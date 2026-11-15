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
-- kept apart from its view; nothing in chat unasked, no errors.
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
_G.UnitAffectingCombat = function() return world.combat end
_G.Enum.DamageMeterType = { DamageDone = 0, Dps = 1, HealingDone = 2, Hps = 3, Absorbs = 4, Interrupts = 5,
  Dispels = 6, DamageTaken = 7 }
_G.Enum.DamageMeterSessionType = { Overall = 0, Current = 1 }
_G.AbbreviateNumbers = function(v)
  if getmetatable(v) == SECRET then return secret("short " .. tostring(v.hiddenValue)) end
  return "short " .. tostring(v)
end
_G.C_DamageMeter = {
  IsDamageMeterAvailable = function() return world.available, "" end,
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
eq(f.holder._text, "this fight · 1:05", "which tally and its length")
local last = world.asked[#world.asked]
check(last[1] == 1 and last[2] == 0, "the game was asked for the current fight's damage")
eq(f.rows[1].left._text, "1. Ilse", "rank and name")
eq(f.rows[1].right._text, "short 1200", "the total through the game's formatter")
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
eq(f.rows[1].right._text, "short 18.5", "a rate view shows the per-second amount")
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
eq(f.rows[1].right._text.hiddenValue, "short 1200", "and it came through the game's formatter")
eq(f.holder._text, "this fight", "a secret length is left out")
check(f.rows[3].mine:IsShown(), "the player is still found: that flag is never secret")
world.combat, world.secret = false, false
F.Fire("PLAYER_REGEN_ENABLED")
M.Tick()
eq(f.rows[1].right._text, "short 1200", "plain again after the fight")

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
