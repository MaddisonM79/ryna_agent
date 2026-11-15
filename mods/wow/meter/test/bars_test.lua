-- The meter (Bars.lua, Frame.lua) against the fake client, with a made-up tally.
-- From the repository root, with Lua 5.1 (what the game runs) or later:
--
--   lua5.1 mods/wow/meter/test/bars_test.lua
--
-- It proves: the game is asked for the view and session shown; rows in the game's order with rank,
-- name, amount and class colour, the player's marked; a rate view shows the per-second amount; the
-- player kept in the last row when further down; a click goes to the next view and a right-click
-- between this fight and overall, both kept; in a fight every amount reaches its widget still secret,
-- through the game's formatter, with nothing compared; no read out of a fight unless something
-- changed; nothing yet, and no meter in the client; the setting and /ryna meter; its place
-- kept apart from its view; a read every two seconds when idle and every time while a friend fights; the
-- newest held fight when the current tally is empty; the pick-list's rows, a view and a past fight
-- picked from it, and the client's menu opened on a click; a second window with the partner view, its own
-- rows, view and place, up to four, closed from its list, all hidden by the setting; nothing in chat
-- unasked, no errors.
-- luacheck: ignore 1

local CORE = "cores/wow/addon/Ryna/"
local ADDON = "mods/wow/meter/addon/Ryna_Meter/"
local F = dofile("mods/wow/chronicle/test/fake_client.lua")

local failures, checks = 0, 0
local function check(cond, what)
  checks = checks + 1
  if not cond then
    failures = failures + 1
    print("FAIL: " .. what)
  end
end
local function eq(got, want, what)
  check(got == want, what .. ": got " .. tostring(got) .. ", want " .. tostring(want))
end

-- A secret: raises on arithmetic and ordering, and only issecretvalue knows it.
local SECRET = {}
local function secret(v) return setmetatable({ hiddenValue = v }, SECRET) end
SECRET.__lt = function() error("compared a secret") end
SECRET.__le = SECRET.__lt
SECRET.__add = function() error("arithmetic on a secret") end
SECRET.__sub, SECRET.__mul, SECRET.__div, SECRET.__unm = SECRET.__add, SECRET.__add, SECRET.__add, SECRET.__add

local world = { combat = false, secret = false, available = true, sessions = {}, asked = {} }
local function source(name, class, total, rate, me)
  return { name = name, classFilename = class, totalAmount = total, amountPerSecond = rate,
    isLocalPlayer = me or false }
end
local function hide(v) return world.secret and secret(v) or v end

F.Install({ now = 1800003600 })
_G.issecretvalue = function(v) return getmetatable(v) == SECRET end
_G.UnitGUID = function(unit) if unit == "player" then return "Player-1-0000000D" end end
_G.UnitName = function(unit) if unit == "player" then return "Dela" end end
_G.UnitAffectingCombat = function(unit)
  if unit == "player" then return world.combat end
  return unit == "party1" and world.friendFights or false
end
_G.Enum.DamageMeterType = { DamageDone = 0, Dps = 1, HealingDone = 2, Hps = 3, Absorbs = 4, Interrupts = 5,
  Dispels = 6, DamageTaken = 7 }
_G.Enum.DamageMeterSessionType = { Overall = 0, Current = 1 }
-- The game's formatter: its own default ("short N") without options; with them, the first breakpoint
-- the number reaches, as floor(n / significandDivisor) / fractionDivisor and the abbreviation. It refuses
-- a rule for numbers under a thousand while world.noSmall is set, as a client might.
local function format(n, options)
  local data = options and (options.breakpointData or (options.config and options.config.data))
  if not data then return "short " .. tostring(n) end
  for _, b in ipairs(data) do
    if b.breakpoint < 1 and world.noSmall then error("breakpoint too small") end
    if n >= b.breakpoint then
      return tostring(math.floor(n / b.significandDivisor + 1e-9) / b.fractionDivisor) .. b.abbreviation
    end
  end
  return tostring(math.floor(n))
end
_G.AbbreviateNumbers = function(v, options)
  if getmetatable(v) == SECRET then return secret(format(v.hiddenValue, options)) end
  return format(v, options)
end
_G.CreateAbbreviateConfig = function(data) return { data = data } end
_G.C_DamageMeter = {
  IsDamageMeterAvailable = function() return world.available, "" end,
  GetAvailableCombatSessions = function() return world.fights or {} end,
  GetCombatSessionFromID = function(id, kind)
    world.asked[#world.asked + 1] = { "id", id, kind }
    local s = world.sessions["id" .. id .. ":" .. kind]
    if not s then return { combatSources = {}, maxAmount = 0, totalAmount = 0 } end
    local out = { combatSources = {}, maxAmount = s.max, totalAmount = s.max, durationSeconds = s.duration }
    for i, src in ipairs(s) do out.combatSources[i] = src end
    return out
  end,
  GetCombatSessionFromType = function(session, kind)
    world.asked[#world.asked + 1] = { session, kind }
    local s = world.sessions[session .. ":" .. kind]
    if not s then return { combatSources = {}, maxAmount = 0, totalAmount = 0 } end
    local out = { combatSources = {}, maxAmount = hide(s.max), totalAmount = hide(s.max),
      durationSeconds = hide(s.duration) }
    for i, src in ipairs(s) do
      out.combatSources[i] = { name = hide(src.name), classFilename = src.classFilename,
        totalAmount = hide(src.totalAmount), amountPerSecond = hide(src.amountPerSecond),
        isLocalPlayer = src.isLocalPlayer }
    end
    return out
  end,
}
_G.C_AddOns = { GetAddOnMetadata = function() return "0.0.0" end }
_G.RAID_CLASS_COLORS.MAGE = { r = 0.25, g = 0.78, b = 0.92 }
_G.RAID_CLASS_COLORS.PRIEST = { r = 1, g = 1, b = 1 }
_G.RynaDB = { schema = 1, settings = {}, journal = {}, characters = {} }

world.sessions["1:0"] = { max = 1200, duration = 65,
  source("Ilse", "MAGE", 1200, 18.5), source("Brom", "WARRIOR", 800, 12.3), source("Dela", "PRIEST", 90, 1.4, true) }
world.sessions["1:1"] = world.sessions["1:0"]
world.sessions["0:0"] = { max = 9000, duration = 600, source("Brom", "WARRIOR", 9000, 15) }
world.sessions["0:1"] = world.sessions["0:0"]

local core, mod = {}, {}
F.LoadAddon(CORE, "Ryna", core)
F.LoadAddon(ADDON, "Ryna_Meter", mod)
F.Fire("ADDON_LOADED", "Ryna")
F.Fire("PLAYER_LOGIN")
F.DropTimers()

local M = mod.Bars
check(M ~= nil and M.driver ~= nil, "the meter loaded and its driver started")
eq(_G.Ryna.Settings.Get("meterBars"), true, "on by default")

-- Out of a fight: drawn once -------------------------------------------------------------------------

M.driver._scripts.OnUpdate(M.driver, 0.6)
local f = M.frame
check(f ~= nil and f:IsShown(), "the meter shows")
eq(f.title._text, "Damage", "the first view")
eq(f.holder._text, "last fight · 1:05", "which tally and its length, out of a fight")
local last = world.asked[#world.asked]
check(last[1] == 1 and last[2] == 0, "the game was asked for the current fight's damage")
eq(f.rows[1].left._text, "1. Ilse", "rank and name")
eq(f.rows[1].right._text, "1.20K", "the total to two decimals")
eq(f.rows[1]._max, 1200, "the bar's top is the highest amount")
eq(f.rows[1]._value, 1200, "the bar's value")
eq(f.rows[1]._barColor[1], 0.25, "the class colour")
eq(f.rows[2].left._text, "2. Brom", "the game's order is kept")
check(f.rows[3].mine:IsShown() and not f.rows[1].mine:IsShown(), "the player's row is marked")
check(not f.rows[4]:IsShown(), "unused rows are hidden")
check(not f.foot:IsShown(), "no line under the rows")
local asks = #world.asked
M.driver._scripts.OnUpdate(M.driver, 0.6)
eq(#world.asked, asks, "nothing changed and no fight: no read")
F.Fire("DAMAGE_METER_COMBAT_SESSION_UPDATED", 0, 1)
M.driver._scripts.OnUpdate(M.driver, 0.6)
check(#world.asked > asks, "the game said it changed: a read")

-- The next view, and overall; both kept -------------------------------------------------------------

F.Click(f.head, "LeftButton")
eq(f.title._text, "DPS", "a click: the next view")
eq(f.rows[1].right._text, "18.50", "a rate view shows the per-second amount, to two decimals")
eq(f.rows[1]._value, 1200, "its bar is still the total")
eq(_G.RynaDB.meter.bars.view, "dps", "the view is kept")
F.Click(f.head, "RightButton")
eq(f.holder._text, "overall · 10:00", "a right-click: overall")
eq(_G.RynaDB.meter.bars.session, "overall", "the session is kept")
F.Click(f.head, "RightButton")
for _ = 1, #M.VIEWS - 1 do F.Click(f.head, "LeftButton") end
eq(f.title._text, "Damage", "the views go round")

-- In a fight: secrets go to the widgets untouched ---------------------------------------------------

world.combat, world.secret = true, true
asks = #world.asked
M.driver._scripts.OnUpdate(M.driver, 0.6)
check(#world.asked > asks, "in a fight: read every time")
eq(core.errors.count, 0, "no error with secret amounts: " .. tostring(core.errors.last))
check(_G.issecretvalue(f.rows[1]._value), "the secret total went to the bar")
check(_G.issecretvalue(f.rows[1]._max), "the secret top went to the bar")
check(_G.issecretvalue(f.rows[1].right._text), "the formatted amount is still the game's secret")
eq(f.rows[1].right._text.hiddenValue, "1.2K", "and it came through the game's formatter, given the two-decimal rule")
check(M.optionsSmall == true, "the formatter was found to take the rule for small numbers too")
eq(M.Short(secret(18.57)).hiddenValue, "18.57", "a secret rate keeps its two decimals")
eq(M.Short(secret(4567890)).hiddenValue, "4.56M", "a secret in the millions")
eq(M.Short(90), "90", "a whole number under a thousand has no decimals")
eq(M.Short(4567890), "4.57M", "a plain number in the millions")
M.options, world.noSmall = nil, true
eq(M.Short(secret(1234.5)).hiddenValue, "1.23K", "a client that refuses the small rule still gets thousands")
check(M.optionsSmall == false, "found without the small rule")
world.noSmall = nil
local honest = _G.AbbreviateNumbers
_G.AbbreviateNumbers = function(v)
  if getmetatable(v) == SECRET then return secret("short " .. tostring(v.hiddenValue)) end
  return "short " .. tostring(v)
end
M.options = nil
eq(M.Short(secret(1234.5)).hiddenValue, "short 1234.5", "a formatter that follows no options is used as it is")
eq(M.options, false, "and that is remembered")
_G.AbbreviateNumbers, M.options = honest, nil
eq(f.holder._text, "this fight", "a secret length is left out")
check(f.rows[3].mine:IsShown(), "the player is still found: that flag is never secret")
world.combat, world.secret = false, false
F.Fire("PLAYER_REGEN_ENABLED")
M.Tick()
eq(f.rows[1].right._text, "1.20K", "plain again after the fight")

-- The player further down than the rows -------------------------------------------------------------

local many = { max = 5000, duration = 30 }
for i = 1, 11 do many[i] = source("Player" .. i, "WARRIOR", 5000 - i * 100, 10) end
many[10] = source("Dela", "PRIEST", 4000, 9, true)
world.sessions["1:0"] = many
M.dirty = true
M.Tick()
eq(f.rows[7].left._text, "7. Player7", "the rows before the last")
eq(f.rows[8].left._text, "10. Dela", "the player in the last row, with their own rank")
check(f.rows[8].mine:IsShown(), "and marked")

-- Nothing yet; no meter in the client ----------------------------------------------------------------

world.sessions["1:0"] = nil
M.dirty = true
M.Tick()
check(not f.rows[1]:IsShown(), "no rows with nothing counted")
eq(f.foot._text, "Nothing yet.", "and it says so")
world.available = false
M.dirty = true
M.Tick()
eq(f.foot._text, "The game's meter isn't available here.", "the game says the meter can't be used")
world.available = true
local kept = _G.C_DamageMeter
_G.C_DamageMeter = nil
M.dirty = true
M.Tick()
eq(f.foot._text, "The game's meter isn't available here.", "a client without the meter")
_G.C_DamageMeter = kept
eq(core.errors.count, 0, "no errors without it: " .. tostring(core.errors.last))

-- Its place, apart from its view ---------------------------------------------------------------------

_G.RynaDB.meter.bars.view = "healing"
f:ClearAllPoints()
f:SetPoint("TOPLEFT", _G.UIParent, "TOPLEFT", 12, -40)
f.head._scripts.OnDragStop(f.head)
eq(_G.RynaDB.meter.bars.x, 12, "its place is kept when dragged by the first line")
M.ResetPlace()
eq(_G.RynaDB.meter.bars.x, nil, "Defaults forgets the place")
eq(_G.RynaDB.meter.bars.view, "healing", "and keeps the view")

-- Idle: read again every two seconds; a friend fighting counts as a fight ------------------------------

M.Pick("damage")
world.sessions["1:0"] = { max = 300, duration = 20, source("Brom", "WARRIOR", 300, 15) }
M.dirty = true
M.Tick()
asks = #world.asked
for _ = 1, 3 do M.Tick() end
eq(#world.asked, asks, "idle: three reads skipped")
M.Tick()
check(#world.asked > asks, "idle: the fourth reads, so it never sits on an old fight")
world.friendFights = true
asks = #world.asked
M.Tick()
check(#world.asked > asks, "a group member fighting: read every time")
eq(f.holder._text, "this fight · 0:20", "and it is this fight")
world.friendFights = false

-- The current tally empty: the newest fight the game still holds ------------------------------------

world.sessions["1:0"] = nil
world.fights = { { sessionID = 3, name = "Quay Rat", durationSeconds = 41 },
  { sessionID = 4, name = "Captain Gull", durationSeconds = 95 } }
world.sessions["id4:0"] = { max = 700, duration = 95, source("Ilse", "MAGE", 700, 7.4) }
world.sessions["id3:1"] = { max = 50, duration = 41, source("Dela", "PRIEST", 50, 1.2, true) }
M.dirty = true
M.Tick()
eq(f.rows[1].left._text, "1. Ilse", "current is empty: the newest fight is shown")
eq(f.holder._text, "last fight · 1:35", "and named the last fight")

-- The pick-list ---------------------------------------------------------------------------------------

local rows = M.MenuRows()
local texts = {}
for _, r in ipairs(rows) do texts[#texts + 1] = r.title and ("[" .. r.title .. "]") or r.text or "-" end
eq(table.concat(texts, "|"), "[Show]|Damage|DPS|Healing|HPS|Damage taken|Absorbs|Interrupts|Dispels|-|[From]|"
  .. "This fight (or the last one)|Overall|Captain Gull · 1:35|Quay Rat · 0:41|-|Open another window",
  "what the pick-list holds, newest fight first")
check(rows[2].chosen and not rows[3].chosen, "the view shown is the chosen one")
check(rows[12].chosen, "this fight is the chosen tally")
rows[3].pick()
eq(f.title._text, "DPS", "picking a view shows it")
eq(_G.RynaDB.meter.bars.view, "dps", "and keeps it")
rows[15].pick()
eq(f.holder._text, "Quay Rat · 0:41", "picking a past fight shows it by name")
eq(f.rows[1].left._text, "1. Dela", "with that fight's rows")
check(M.MenuRows()[15].chosen, "and it is the chosen one in the list")
eq(_G.RynaDB.meter.bars.session, "current", "a past fight isn't what is kept for after /reload")
F.Click(f.head, "RightButton")
eq(M.SessionKey(), "current", "a right-click leaves a past fight for this fight")
local opened
_G.MenuUtil = { CreateContextMenu = function(owner, build)
  opened = { owner = owner, n = 0 }
  local root = { CreateTitle = function() end, CreateDivider = function() end,
    CreateButton = function(_, text) opened.buttons = (opened.buttons or 0) + 1; opened.button = text end,
    CreateRadio = function(_, text, chosen, pick) opened.n = opened.n + 1; opened.last = { text, chosen, pick } end }
  build(owner, root)
end }
F.Click(f.head, "LeftButton")
check(opened and opened.owner == f.head and opened.n == 12, "a click opens the client's menu with every choice")
check(opened.buttons == 1 and opened.button == "Open another window", "and the way to another window")
eq(f.title._text, "DPS", "and no longer goes to the next view")
opened.last[3]()
eq(f.holder._text, "Quay Rat · 0:41", "a choice in the client's menu is made")
_G.MenuUtil = nil
M.Choose("current")
M.Pick("damage")
world.fights = nil
world.sessions["1:0"] = { max = 300, duration = 20, source("Brom", "WARRIOR", 300, 15) }

-- A second window: DPS beside HPS -----------------------------------------------------------------------

world.sessions["1:1"] = { max = 300, duration = 20, source("Brom", "WARRIOR", 300, 15) }
world.sessions["1:3"] = { max = 120, duration = 20, source("Dela", "PRIEST", 120, 6, true) }
M.Pick("dps")
eq(M.Open(), 2, "another window opens as the second")
local f2 = M.frames[2]
check(f2 ~= nil and f2:IsShown() and f2 ~= f, "it is a frame of its own")
eq(f2.title._text, "HPS", "beside DPS it shows HPS")
eq(f.title._text, "DPS", "the first keeps its view")
eq(f2.rows[1].left._text, "1. Dela", "with its own rows")
eq(f2.rows[1].right._text, "6", "and its own amounts")
check(_G.RynaDB.meter.bars2.open == true and _G.RynaDB.meter.bars2.view == "hps",
  "its view and that it is open are kept")
M.Pick("healing", 2)
eq(f2.title._text, "Healing", "a view picked in the second window changes the second")
eq(f.title._text, "DPS", "and leaves the first")
local list2 = M.MenuRows(2)
eq(list2[#list2].text, "Close this window", "the second window's list can close it")
eq(M.MenuRows(1)[#M.MenuRows(1)].text, "Open another window", "the first can't be closed, only joined")
f2:ClearAllPoints()
f2:SetPoint("TOPLEFT", _G.UIParent, "TOPLEFT", 300, -90)
f2.head._scripts.OnDragStop(f2.head)
eq(_G.RynaDB.meter.bars2.x, 300, "the second window's place is its own")
-- Locked together: under the one before it, dragged as one
local names = {}
for _, r in ipairs(M.MenuRows(2)) do if r.button then names[#names + 1] = r.text end end
eq(table.concat(names, "|"), "Open another window|Lock under the window before it|"
  .. "Lock beside the window before it|Close this window", "a free window's list offers both locks")
M.SetLock(2, "below")
local pt = f2._points[1]
check(#f2._points == 1 and pt[1] == "TOPLEFT" and pt[2] == f and pt[3] == "BOTTOMLEFT" and pt[5] == -2,
  "locked below: its top on the first window's bottom")
eq(_G.RynaDB.meter.bars2.lock, "below", "the lock is kept")
names = {}
for _, r in ipairs(M.MenuRows(2)) do if r.button then names[#names + 1] = r.text end end
eq(table.concat(names, "|"), "Open another window|Unlock this window|Close this window",
  "a locked one offers to unlock")
f:ClearAllPoints()
f:SetPoint("TOPLEFT", _G.UIParent, "TOPLEFT", 77, -11)
f2.head._scripts.OnDragStart(f2.head)
f2.head._scripts.OnDragStop(f2.head)
eq(_G.RynaDB.meter.bars.x, 77, "dragging the locked window moves the first, whose place is kept")
eq(_G.RynaDB.meter.bars2.x, 300, "and its own free place is left for when it is unlocked")
M.SetLock(2, "beside")
pt = f2._points[1]
check(pt[2] == f and pt[3] == "TOPRIGHT" and pt[4] == 2, "locked beside: its left on the first window's right")
M.SetLock(2, nil)
check(f2._points[1][2] == _G.UIParent and f2.mover == nil, "unlocked: on its own again")
eq(M.SetLock(1, "below"), false, "the first window has nothing to lock to")
M.SetLock(2, "below")
eq(M.Open(), 3, "a third")
M.SetLock(3, "below")
check(M.frames[3]._points[1][2] == f2 and M.frames[3].mover() == f,
  "a chain: the third under the second, all moved by the first")
M.SetLock(3, nil)
M.SetLock(2, nil)
eq(M.Open(), 4, "a fourth")
eq(M.Open(), nil, "and no more than four")
check(M.MenuRows(1)[#M.MenuRows(1)].text ~= "Open another window", "with four open the list stops offering one")
M.Close(4)
M.Close(3)
list2[#list2].pick()
check(not f2:IsShown() and _G.RynaDB.meter.bars2.open == nil, "closing puts the window away and forgets it was open")
eq(M.Close(1), false, "the first window isn't closed this way")
M.Open()
_G.Ryna.Settings.Set("meterBars", false)
M.Tick()
check(not f:IsShown() and not M.frames[2]:IsShown(), "the setting off hides every window")
_G.Ryna.Settings.Set("meterBars", true)
M.dirty = true
M.Tick()
check(f:IsShown() and M.frames[2]:IsShown(), "and on brings back the ones that were open")
M.Close(2)
M.Pick("damage")
world.sessions["1:1"] = world.sessions["1:0"]

-- The setting, and /ryna meter ---------------------------------------------------------------

check(#F.chat == 0, "nothing in chat unasked: " .. table.concat(F.chat, " / "))
_G.SlashCmdList.RYNA("meter")
check(not f:IsShown() and _G.Ryna.Settings.Get("meterBars") == false, "the command turns it off")
eq(#F.chat, 1, "and answers once")
M.driver._scripts.OnUpdate(M.driver, 0.6)
check(not f:IsShown(), "it stays away while off")
_G.SlashCmdList.RYNA("meter")
check(f:IsShown(), "and on again")

eq(core.errors.count, 0, "no errors caught: " .. tostring(core.errors.last))
print(string.format("bars: %d checks, %d failed", checks, failures))
os.exit(failures == 0 and 0 or 1)
