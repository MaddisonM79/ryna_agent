-- Meter: damage, healing and the rest, live, in a small frame of its own (docs/ryna-agent-cores-mods.md
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
--   M.Draw(), M.Tick()      draw now; the driver's read (twice a second in a fight, else when told)
--   M.Next(), M.Session()   the next view; this fight or overall
--
-- Click the first line for the next view, right-click it for this fight or overall, drag to move.
-- The view, the session and the place are kept in RynaDB.fights.meter. Nothing is written elsewhere,
-- nothing is said in chat, and the game's own meter is left as the player set it.

local _, ns = ...
if ns.off then return end
local L = ns.L
local N = ns.N
local BF = ns.BarFrame

local M = {}
ns.Meter = M

M.KEY = "fightsMeter"
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
  if type(saved.meter) ~= "table" then saved.meter = {} end
  return saved.meter
end

function M.View()
  local key = kept().view
  for i, v in ipairs(M.VIEWS) do
    if v.key == key then return v, i end
  end
  return M.VIEWS[1], 1
end

function M.SessionKey()
  return kept().session == "overall" and "overall" or "current"
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
  local s = SESSIONS[sessionKey] or SESSIONS.current
  local session = ns.Try(get, enum("DamageMeterSessionType", s.name, s.value),
    enum("DamageMeterType", view.type, view.value))
  if type(session) ~= "table" then return nil, "empty" end
  local sources = session.combatSources
  if type(sources) ~= "table" or ns.IsSecret(sources) then return nil, "empty" end
  local n = ns.Try(function() return #sources end)
  if type(n) ~= "number" or n == 0 then return nil, "empty" end
  return { sources = sources, count = n, max = session.maxAmount, duration = session.durationSeconds }
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
      name = "RynaMeter", key = "meter", rows = M.ROWS, x = 280, y = -220,
      onClick = function(button)
        if button == "RightButton" then M.Session() else M.Next() end
      end,
      onEnter = function(owner)
        local tip = _G.GameTooltip
        if not tip or not tip.SetOwner then return end
        tip:SetOwner(owner, "ANCHOR_TOP")
        tip:AddLine(L["Click: the next view. Right-click: this fight or overall. Drag to move."], 1, 1, 1, true)
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
  local where = sessionKey == "overall" and L["overall"] or L["this fight"]
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

local function fighting()
  return ns.Try(ns.Fn("UnitAffectingCombat"), "player") == true
end

-- The driver's read: every time in a fight, otherwise only when something changed.
function M.Tick()
  if not M.On() then return M.Hide() end
  if M.frame and M.frame:IsShown() and not M.dirty and not fighting() then return end
  M.Draw()
end

function M.Next()
  local _, i = M.View()
  kept().view = M.VIEWS[i % #M.VIEWS + 1].key
  M.Draw()
end

function M.Session()
  kept().session = M.SessionKey() == "overall" and "current" or "overall"
  M.Draw()
end

function M.ResetPlace()
  BF.Forget(M.frame, "meter")
end

function M.Register()
  if M.driver then return end
  local driver = CreateFrame("Frame")
  local since = 0
  driver:SetScript("OnUpdate", function(_, elapsed)
    since = since + (tonumber(elapsed) or 0)
    if since < M.INTERVAL then return end
    since = 0
    ns.Run("meter", M.Tick)
  end)
  M.driver = driver
  M.dirty = true
  local function changed() M.dirty = true end
  for _, event in ipairs({ "DAMAGE_METER_COMBAT_SESSION_UPDATED", "DAMAGE_METER_CURRENT_SESSION_UPDATED",
    "DAMAGE_METER_RESET", "PLAYER_REGEN_ENABLED", "PLAYER_REGEN_DISABLED" }) do
    ns.API.On(event, changed)
  end
end
