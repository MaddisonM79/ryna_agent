-- Bars: damage, healing and the rest, live, in a small frame of its own (docs/ryna-agent-cores-mods.md
-- 7.5, "The meter"). The combat log is closed to addons, but the game keeps its own tallies and gives
-- them out (C_DamageMeter; docs/forever-client-api.md, "The game's damage meter"): for this fight or
-- for everything since the last reset, each source's total and per-second amount, already in order.
--
-- During a fight those numbers are secret. They are shown anyway, because a status bar and a label
-- take a secret value and the game's own number formatter does too: every amount goes from the game
-- to the widget untouched, and nothing here compares, adds or keeps one. Which rows to draw needs no
-- comparison either: the game hands the sources sorted, and says which one is the player.
--
--   M.VIEWS            what the meter can show, in the order a click goes through them
--   M.Read(view, session)   { sources, max, duration } from the game, or nil and why
--   M.Draw(), M.Tick()      draw now; the driver's read (twice a second in a fight, every two seconds
--                           out of one, and at once when the game says a tally changed)
--   M.Menu(owner)           the pick-list: what to show, and which fight
--   M.Pick(view), M.Choose(kind, id, name)   what the pick-list does
--   M.Next(), M.Session()   the next view; this fight or overall (the clicks on a client without menus)
--
-- Click the first line for the pick-list, right-click it for this fight or overall, drag to move.
-- "This fight" is the game's current tally; when that is empty (it is cleared between fights on some
-- clients) the newest fight the game still holds is shown instead, so the meter never sits empty
-- after a pull. The view, the tally and the place are kept in RynaDB.meter.bars; a past fight picked
-- from the list is kept only until /reload, since the game's fight numbers start over. Nothing is written elsewhere,
-- nothing is said in chat, and the game's own meter is left as the player set it.

local _, ns = ...
if ns.off then return end
local L = ns.L
local N = ns.N
local BF = ns.BarFrame

local M = {}
ns.Bars = M

M.KEY = "meterBars"
M.ROWS = 8
M.INTERVAL = 0.5

ns.API.Settings.AddDefaults({ [M.KEY] = true })

-- `type` is the name in Enum.DamageMeterType, `value` its number there (used when the client's Enum
-- lacks the name); `rate` shows the per-second amount instead of the total.
M.VIEWS = {
  { key = "damage", type = "DamageDone", value = 0, label = N("Damage") },
  { key = "dps", type = "Dps", value = 1, label = N("DPS"), rate = true },
  { key = "healing", type = "HealingDone", value = 2, label = N("Healing") },
  { key = "hps", type = "Hps", value = 3, label = N("HPS"), rate = true },
  { key = "taken", type = "DamageTaken", value = 7, label = N("Damage taken") },
  { key = "absorbs", type = "Absorbs", value = 4, label = N("Absorbs") },
  { key = "interrupts", type = "Interrupts", value = 5, label = N("Interrupts") },
  { key = "dispels", type = "Dispels", value = 6, label = N("Dispels") },
}
-- Enum.DamageMeterSessionType: overall 0, current 1.
local SESSIONS = { current = { name = "Current", value = 1 }, overall = { name = "Overall", value = 0 } }

function M.On()
  return ns.API.Settings.Get(M.KEY) and true or false
end

-- What is kept: { view, session, point, rel, x, y }.
local function kept()
  local saved = BF.Saved()
  if not saved then return {} end
  if type(saved.bars) ~= "table" then saved.bars = {} end
  return saved.bars
end

function M.View()
  local key = kept().view
  for i, v in ipairs(M.VIEWS) do
    if v.key == key then return v, i end
  end
  return M.VIEWS[1], 1
end

-- Which tally: "current", "overall", or "id" with M.picked = { id, name } (a past fight, this session).
function M.SessionKey()
  if M.picked then return "id" end
  return kept().session == "overall" and "overall" or "current"
end

local function groupUnits()
  local units = { "player" }
  if ns.Try(ns.Fn("IsInRaid")) == true then
    for i = 1, 40 do units[#units + 1] = "raid" .. i end
  else
    for i = 1, 4 do units[#units + 1] = "party" .. i end
  end
  return units
end

-- Whether the player or anyone in the group is fighting: a healer who hasn't cast yet still watches.
function M.Fighting()
  local inCombat = ns.Fn("UnitAffectingCombat")
  if not inCombat then return false end
  for _, unit in ipairs(groupUnits()) do
    if ns.Try(inCombat, unit) == true then return true end
  end
  return false
end

-- The fights the game still holds, oldest first: { id, name, duration }.
function M.Fights()
  local list = ns.Try(ns.Fn("C_DamageMeter", "GetAvailableCombatSessions"))
  local out = {}
  if type(list) ~= "table" or ns.IsSecret(list) then return out end
  for _, f in ipairs(list) do
    if type(f) == "table" and type(f.sessionID) == "number" and not ns.IsSecret(f.sessionID) then
      out[#out + 1] = { id = f.sessionID, name = ns.NonEmpty(f.name), duration = f.durationSeconds }
    end
  end
  return out
end

local function enum(group, name, fallback)
  local e = _G.Enum
  local g = type(e) == "table" and e[group]
  local v = type(g) == "table" and g[name]
  if type(v) == "number" then return v end
  return fallback
end

-- The game's tally for a view and session. Returns nil and "absent" (no such API), "unavailable" (the
-- game says the meter can't be used) or "empty" (nothing yet).
function M.Read(view, sessionKey)
  local get = ns.Fn("C_DamageMeter", "GetCombatSessionFromType")
  if not get then return nil, "absent" end
  local available = ns.Try(ns.Fn("C_DamageMeter", "IsDamageMeterAvailable"))
  if available == false then return nil, "unavailable" end
  local kind = enum("DamageMeterType", view.type, view.value)
  local function shaped(session)
    if type(session) ~= "table" then return nil end
    local sources = session.combatSources
    if type(sources) ~= "table" or ns.IsSecret(sources) then return nil end
    local n = ns.Try(function() return #sources end)
    if type(n) ~= "number" or n == 0 then return nil end
    return { sources = sources, count = n, max = session.maxAmount, duration = session.durationSeconds }
  end
  local byId = ns.Fn("C_DamageMeter", "GetCombatSessionFromID")
  if sessionKey == "id" and M.picked and byId then
    local data = shaped(ns.Try(byId, M.picked.id, kind))
    if data then return data end
    return nil, "empty"
  end
  local s = SESSIONS[sessionKey] or SESSIONS.current
  local data = shaped(ns.Try(get, enum("DamageMeterSessionType", s.name, s.value), kind))
  if data then return data end
  -- The current tally is empty: the newest fight the game still holds, if it has one.
  if s == SESSIONS.current and byId then
    local fights = M.Fights()
    local last = fights[#fights]
    data = last and shaped(ns.Try(byId, last.id, kind))
    if data then
      data.last = true
      return data
    end
  end
  return nil, "empty"
end

-- A number for a label. A secret is never touched: it goes to the game's own formatter, or to the
-- label as it is.
function M.Short(v)
  local abbreviate = ns.Fn("AbbreviateNumbers") or ns.Fn("AbbreviateLargeNumbers")
  if abbreviate then
    local out = ns.Try(abbreviate, v)
    if ns.IsSecret(out) or out ~= nil then return out end
  end
  if ns.IsSecret(v) or type(v) ~= "number" then return v end
  if v >= 1e6 then return string.format("%.1fM", v / 1e6) end
  if v >= 1e4 then return string.format("%.1fK", v / 1e3) end
  return string.format("%d", math.floor(v + 0.5))
end

-- "1:05" for a fight's length; nil when the game won't say or it is secret.
local function clock(seconds)
  if ns.IsSecret(seconds) or type(seconds) ~= "number" or seconds <= 0 then return nil end
  local s = math.floor(seconds + 0.5)
  return string.format("%d:%02d", math.floor(s / 60), s % 60)
end

-- The rows to draw: the first M.ROWS sources, with the player's own in the last row when it is
-- further down. `isLocalPlayer` is never secret, so finding it compares nothing hidden.
function M.Rows(data)
  local rows, mine = {}, nil
  for i = 1, data.count do
    local src = data.sources[i]
    if type(src) == "table" then
      if i <= M.ROWS then rows[#rows + 1] = { rank = i, src = src } end
      if src.isLocalPlayer == true and i > M.ROWS then mine = { rank = i, src = src } end
    end
  end
  if mine then rows[M.ROWS] = mine end
  return rows
end

local function build()
  if not M.frame then
    M.frame = BF.Create({
      name = "RynaMeter", key = "bars", rows = M.ROWS, x = 280, y = -220,
      onClick = function(button)
        if button == "RightButton" then
          M.Session()
        elseif not M.Menu(M.frame.head) then
          M.Next()
        end
      end,
      onEnter = function(owner)
        local tip = _G.GameTooltip
        if not tip or not tip.SetOwner then return end
        tip:SetOwner(owner, "ANCHOR_TOP")
        tip:AddLine(L["Click: choose what to show. Right-click: this fight or overall. Drag to move."], 1, 1, 1, true)
        tip:Show()
      end,
    })
  end
  return M.frame
end

function M.Draw()
  local f = build()
  local view = M.View()
  local sessionKey = M.SessionKey()
  f.title:SetText(L[view.label])
  local data, why = M.Read(view, sessionKey)
  local where
  if sessionKey == "id" then
    where = M.picked.name or L["an earlier fight"]
  elseif sessionKey == "overall" then
    where = L["overall"]
  elseif (data and data.last) or not M.Fighting() then
    where = L["last fight"]
  else
    where = L["this fight"]
  end
  local length = data and clock(data.duration)
  f.holder:SetText(length and string.format("%s · %s", where, length) or where)

  local rows = data and M.Rows(data) or {}
  for i, bar in ipairs(f.rows) do
    local r = rows[i]
    if r then
      local src = r.src
      -- The name can be secret in a fight: the rank is added only if the client will format it.
      local ok, label = pcall(string.format, "%d. %s", r.rank, src.name)
      bar.left:SetText(ok and label or src.name)
      bar.right:SetText(M.Short(view.rate and src.amountPerSecond or src.totalAmount))
      bar:SetMinMaxValues(0, data.max)
      bar:SetValue(src.totalAmount)
      bar:SetStatusBarColor(BF.ClassRGB(src.classFilename))
      if src.isLocalPlayer == true then bar.mine:Show() else bar.mine:Hide() end
      bar:Show()
    else
      bar:Hide()
    end
  end

  local foot
  if why == "absent" or why == "unavailable" then
    foot = L["The game's meter isn't available here."]
  elseif why == "empty" then
    foot = L["Nothing yet."]
  end
  BF.Foot(f, foot)
  BF.Fit(f, #rows, foot)
  f:Show()
  M.dirty = false
end

function M.Hide()
  if M.frame then M.frame:Hide() end
end

-- The driver's read: every time while anyone in the group is fighting or the game said a tally
-- changed, and otherwise every fourth time (two seconds), so it can never sit on an old fight.
local idle = 0
function M.Tick()
  if not M.On() then return M.Hide() end
  if M.frame and M.frame:IsShown() and not M.dirty and not M.Fighting() then
    idle = idle + 1
    if idle < 4 then return end
  end
  idle = 0
  M.Draw()
end

function M.Pick(key)
  kept().view = key
  M.Draw()
end

-- A tally: "current" or "overall" (kept), or a past fight by its number (until /reload).
function M.Choose(kind, id, name)
  if kind == "id" and type(id) == "number" then
    M.picked = { id = id, name = name }
  else
    M.picked = nil
    kept().session = kind == "overall" and "overall" or "current"
  end
  M.Draw()
end

function M.Next()
  local _, i = M.View()
  M.Pick(M.VIEWS[i % #M.VIEWS + 1].key)
end

function M.Session()
  M.Choose(M.SessionKey() == "current" and "overall" or "current")
end

-- How many past fights the pick-list offers, newest first.
M.MENU_FIGHTS = 10

-- What the pick-list holds, as rows: { title }, { divider }, or { text, chosen, pick }. Apart from the
-- client's menu so it can be read without one.
function M.MenuRows()
  local rows = { { title = L["Show"] } }
  local view = M.View()
  for _, v in ipairs(M.VIEWS) do
    rows[#rows + 1] = { text = L[v.label], chosen = v.key == view.key, pick = function() M.Pick(v.key) end }
  end
  rows[#rows + 1] = { divider = true }
  rows[#rows + 1] = { title = L["From"] }
  local key = M.SessionKey()
  rows[#rows + 1] = { text = L["This fight (or the last one)"], chosen = key == "current",
    pick = function() M.Choose("current") end }
  rows[#rows + 1] = { text = L["Overall"], chosen = key == "overall", pick = function() M.Choose("overall") end }
  local fights = M.Fights()
  for i = #fights, math.max(1, #fights - M.MENU_FIGHTS + 1), -1 do
    local f = fights[i]
    local name = f.name or string.format(L["Fight %d"], f.id)
    local length = clock(f.duration)
    local text = length and string.format("%s · %s", name, length) or name
    rows[#rows + 1] = { text = text, chosen = key == "id" and M.picked.id == f.id,
      pick = function() M.Choose("id", f.id, name) end }
  end
  return rows
end

-- Opens the pick-list at `owner` with the client's own menu. Returns false on a client without it
-- (the click then goes to the next view).
function M.Menu(owner)
  local open = ns.Fn("MenuUtil", "CreateContextMenu")
  if not open then return false end
  local ok = pcall(open, owner, function(_, root)
    for _, row in ipairs(M.MenuRows()) do
      if row.title then
        root:CreateTitle(row.title)
      elseif row.divider then
        root:CreateDivider()
      else
        root:CreateRadio(row.text, function() return row.chosen end, function()
          ns.Run("meter pick", row.pick)
        end)
      end
    end
  end)
  return ok
end

function M.ResetPlace()
  BF.Forget(M.frame, "bars")
end

function M.Register()
  if M.driver then return end
  local driver = CreateFrame("Frame")
  local since = 0
  driver:SetScript("OnUpdate", function(_, elapsed)
    since = since + (tonumber(elapsed) or 0)
    if since < M.INTERVAL then return end
    since = 0
    ns.Run("meter bars", M.Tick)
  end)
  M.driver = driver
  M.dirty = true
  local function changed() M.dirty = true end
  for _, event in ipairs({ "DAMAGE_METER_COMBAT_SESSION_UPDATED", "DAMAGE_METER_CURRENT_SESSION_UPDATED",
    "DAMAGE_METER_RESET", "PLAYER_REGEN_ENABLED", "PLAYER_REGEN_DISABLED" }) do
    ns.API.On(event, changed)
  end
end
