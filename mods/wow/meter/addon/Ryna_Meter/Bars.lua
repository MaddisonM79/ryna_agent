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
-- after a pull. The pick-list also opens another window (up to four: damage beside healing, DPS beside
-- HPS) and closes one, and locks a window under or beside the one before it, so they sit together and
-- drag as one. Each window's view, tally, lock and place are kept in RynaDB.meter (bars, bars2, ...);
-- a past fight picked from the list is kept only until /reload, since the game's fight numbers start
-- over. Nothing is written elsewhere,
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

-- Several windows can be open at once (damage beside healing, say), each with its own view, tally
-- and place. Every function below takes the window's number last and means the first without one.
M.MAX_WINDOWS = 4
-- The space between two windows locked together.
M.GAP = 2
M.frames = {}
-- A past fight picked for a window, this session: pickedBy[w] = { id, name }.
local pickedBy = {}

-- The key a window's state is kept under in RynaDB.meter: "bars", "bars2", ...
local function keyOf(w)
  return (w or 1) == 1 and "bars" or ("bars" .. w)
end

-- What is kept for a window: { view, session, open, point, rel, x, y }.
local function kept(w)
  local saved = BF.Saved()
  if not saved then return {} end
  local key = keyOf(w)
  if type(saved[key]) ~= "table" then saved[key] = {} end
  return saved[key]
end

-- The first window is open whenever the meter is on; the others when they were opened.
function M.IsOpen(w)
  return (w or 1) == 1 or kept(w).open == true
end

function M.View(w)
  local key = kept(w).view
  for i, v in ipairs(M.VIEWS) do
    if v.key == key then return v, i end
  end
  return M.VIEWS[1], 1
end

-- Which tally: "current", "overall", or "id" (a past fight picked this session).
function M.SessionKey(w)
  if pickedBy[w or 1] then return "id" end
  return kept(w).session == "overall" and "overall" or "current"
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
function M.Read(view, sessionKey, w)
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
  local picked = pickedBy[w or 1]
  if sessionKey == "id" and picked and byId then
    local data = shaped(ns.Try(byId, picked.id, kind))
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

local function build(w)
  w = w or 1
  if not M.frames[w] then
    M.frames[w] = BF.Create({
      name = w == 1 and "RynaMeter" or ("RynaMeter" .. w), key = keyOf(w), rows = M.ROWS,
      x = 280, y = -220 - (w - 1) * 40,
      onClick = function(button)
        if button == "RightButton" then
          M.Session(w)
        elseif not M.Menu(M.frames[w].head, w) then
          M.Next(w)
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
    if w == 1 then M.frame = M.frames[1] end
  end
  return M.frames[w]
end

-- The open window before `w`, which `w` can be locked to; nil for the first.
function M.Before(w)
  for i = (w or 1) - 1, 1, -1 do
    if M.IsOpen(i) then return i end
  end
  return nil
end

-- How a window is locked to the one before it: "below", "beside", or nil when it is free (or there is
-- no window before it any more).
function M.Lock(w)
  local how = kept(w).lock
  if (how == "below" or how == "beside") and M.Before(w) then return how end
  return nil
end

-- The window a drag of `w` moves: the head of its group.
local function head(w)
  while M.Lock(w) do w = M.Before(w) end
  return w
end

-- Puts a window where its lock says: under or beside the one before it, moving with it and following
-- its height. A window just unlocked stays where it was drawn.
local function position(w, f)
  local how = M.Lock(w)
  f.mover = how and function() return M.frames[head(w)] end or nil
  if how then
    local anchor = M.frames[M.Before(w)]
    if not anchor then return end
    f:ClearAllPoints()
    if how == "below" then
      f:SetPoint("TOPLEFT", anchor, "BOTTOMLEFT", 0, -M.GAP)
    else
      f:SetPoint("TOPLEFT", anchor, "TOPRIGHT", M.GAP, 0)
    end
    f.locked = true
  elseif f.locked then
    f.locked = nil
    -- Its own place again: where it is now when the client can say, else where it was last left.
    local left, top = ns.Try(f.GetLeft, f), ns.Try(f.GetTop, f)
    if type(left) == "number" and type(top) == "number" then
      f:ClearAllPoints()
      f:SetPoint("TOPLEFT", _G.UIParent, "BOTTOMLEFT", left, top)
      local k = kept(w)
      k.point, k.rel, k.x, k.y = "TOPLEFT", "BOTTOMLEFT", left, top
    else
      BF.Place(f)
    end
  end
end

-- Locks a window under or beside the one before it ("below", "beside"), or frees it (nil).
function M.SetLock(w, how)
  if (w or 1) == 1 then return false end
  kept(w).lock = (how == "below" or how == "beside") and how or nil
  M.Draw()
  return true
end

-- Draws one window.
function M.DrawOne(w)
  local f = build(w)
  position(w or 1, f)
  local view = M.View(w)
  local sessionKey = M.SessionKey(w)
  f.title:SetText(L[view.label])
  local data, why = M.Read(view, sessionKey, w)
  local where
  if sessionKey == "id" then
    where = pickedBy[w or 1].name or L["an earlier fight"]
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
end

-- Draws every open window, and puts away any that was closed.
function M.Draw()
  for w = 1, M.MAX_WINDOWS do
    if M.IsOpen(w) then
      M.DrawOne(w)
    elseif M.frames[w] then
      M.frames[w]:Hide()
    end
  end
  M.dirty = false
end

function M.Hide()
  for _, f in pairs(M.frames) do f:Hide() end
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

function M.Pick(key, w)
  kept(w).view = key
  M.Draw()
end

-- A tally: "current" or "overall" (kept), or a past fight by its number (until /reload).
function M.Choose(kind, id, name, w)
  if kind == "id" and type(id) == "number" then
    pickedBy[w or 1] = { id = id, name = name }
  else
    pickedBy[w or 1] = nil
    kept(w).session = kind == "overall" and "overall" or "current"
  end
  M.Draw()
end

function M.Next(w)
  local _, i = M.View(w)
  M.Pick(M.VIEWS[i % #M.VIEWS + 1].key, w)
end

function M.Session(w)
  M.Choose(M.SessionKey(w) == "current" and "overall" or "current", nil, nil, w)
end

-- A view's partner, for what a new window shows beside an open one: damage with healing, DPS with HPS.
local PARTNER = { damage = "healing", healing = "damage", dps = "hps", hps = "dps" }

-- Opens another window: the partner of the first window's view when no open window shows it, else the
-- first view none shows. Returns its number, or nil when every window is open.
function M.Open()
  local shown, free = {}, nil
  for w = 1, M.MAX_WINDOWS do
    if M.IsOpen(w) then shown[M.View(w).key] = true elseif not free then free = w end
  end
  if not free then return nil end
  local view = PARTNER[M.View(1).key]
  if not view or shown[view] then
    view = nil
    for _, v in ipairs(M.VIEWS) do
      if not view and not shown[v.key] then view = v.key end
    end
  end
  local k = kept(free)
  k.open, k.view, k.session = true, view or M.VIEWS[1].key, kept(1).session
  pickedBy[free] = nil
  M.Draw()
  return free
end

-- Closes a window other than the first (the setting turns the meter off).
function M.Close(w)
  if (w or 1) == 1 then return false end
  kept(w).open = nil
  pickedBy[w] = nil
  M.Draw()
  return true
end

-- How many past fights the pick-list offers, newest first.
M.MENU_FIGHTS = 10

-- What the pick-list holds, as rows: { title }, { divider }, or { text, chosen, pick }. Apart from the
-- client's menu so it can be read without one.
function M.MenuRows(w)
  w = w or 1
  local rows = { { title = L["Show"] } }
  local view = M.View(w)
  for _, v in ipairs(M.VIEWS) do
    rows[#rows + 1] = { text = L[v.label], chosen = v.key == view.key, pick = function() M.Pick(v.key, w) end }
  end
  rows[#rows + 1] = { divider = true }
  rows[#rows + 1] = { title = L["From"] }
  local key = M.SessionKey(w)
  rows[#rows + 1] = { text = L["This fight (or the last one)"], chosen = key == "current",
    pick = function() M.Choose("current", nil, nil, w) end }
  rows[#rows + 1] = { text = L["Overall"], chosen = key == "overall",
    pick = function() M.Choose("overall", nil, nil, w) end }
  local fights = M.Fights()
  for i = #fights, math.max(1, #fights - M.MENU_FIGHTS + 1), -1 do
    local f = fights[i]
    local name = f.name or string.format(L["Fight %d"], f.id)
    local length = clock(f.duration)
    local text = length and string.format("%s · %s", name, length) or name
    rows[#rows + 1] = { text = text, chosen = key == "id" and pickedBy[w].id == f.id,
      pick = function() M.Choose("id", f.id, name, w) end }
  end
  -- Windows: another one while there is room, and closing this one when it isn't the first.
  local open = 0
  for i = 1, M.MAX_WINDOWS do
    if M.IsOpen(i) then open = open + 1 end
  end
  if open < M.MAX_WINDOWS or w > 1 then rows[#rows + 1] = { divider = true } end
  if open < M.MAX_WINDOWS then
    rows[#rows + 1] = { text = L["Open another window"], button = true, pick = function() M.Open() end }
  end
  if w > 1 then
    local how = M.Lock(w)
    if how then
      rows[#rows + 1] = { text = L["Unlock this window"], button = true, pick = function() M.SetLock(w, nil) end }
    elseif M.Before(w) then
      rows[#rows + 1] = { text = L["Lock under the window before it"], button = true,
        pick = function() M.SetLock(w, "below") end }
      rows[#rows + 1] = { text = L["Lock beside the window before it"], button = true,
        pick = function() M.SetLock(w, "beside") end }
    end
    rows[#rows + 1] = { text = L["Close this window"], button = true, pick = function() M.Close(w) end }
  end
  return rows
end

-- Opens the pick-list at `owner` with the client's own menu. Returns false on a client without it
-- (the click then goes to the next view).
function M.Menu(owner, w)
  local open = ns.Fn("MenuUtil", "CreateContextMenu")
  if not open then return false end
  local ok = pcall(open, owner, function(_, root)
    for _, row in ipairs(M.MenuRows(w)) do
      if row.title then
        root:CreateTitle(row.title)
      elseif row.divider then
        root:CreateDivider()
      elseif row.button then
        root:CreateButton(row.text, function() ns.Run("meter window", row.pick) end)
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
  for w = 1, M.MAX_WINDOWS do BF.Forget(M.frames[w], keyOf(w)) end
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
