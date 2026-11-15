-- German (deDE) strings for Meter by Ryna. Loaded on every client, applied only when the game
-- runs in deDE: GetLocale() is "deDE".
--
-- How to translate:
--   - Each line is L["English"] = nil. Replace nil with your text in double quotes:
--       L["Take part in the census"] = "..."
--     A line left as nil shows the English, so a partial translation is fine.
--   - Keep every %s and %d, in the same order (WoW's string.format can't reorder them).
--   - Keep "||" as it is: it prints one "|" in chat. Inside a command (/ryna debug on||off||toggle,
--     /who, /reload) keep the words in English: they are what the player types.
--   - Plurals: a pair like "%d player" / "%d players" is used for 1 / everything else.
--     For a language with more forms, use the wording that reads best for most numbers.
--   - Keys are the English text and must stay exactly as they are; only change the values.
--   - Test: set the client's language (Battle.net -> Settings -> Game Settings -> Text Language,
--     or SET textLocale "deDE" in WTF/Config.wtf, with that language installed), log in, then
--     /ryna, /ryna status and /ryna help.
-- This file is generated from enUS.lua by cores/wow/addon/tools/strings.mjs --templates, which keeps the
-- translations already here; run it after enUS.lua changes.

local _, ns = ...
if ns.off then return end
if ns.Locale() ~= "deDE" then return end
local L = ns.L

-- Init.lua
-- /ryna's help line for the command.
L["/ryna meter: the meter on or off; /ryna meter threat: place the threat bar"] = nil
-- Answers to /ryna meter.
L["The meter is on."] = nil
L["The meter is off."] = nil
-- Answers to /ryna meter threat.
L["The threat bar is off: turn it on in /ryna, under Meter."] = nil
L["The threat bar is showing an example: drag it where you want it, then type /ryna meter threat again."] = nil

-- Tab.lua: the Meter tab of /ryna.
L["Meter"] = nil
L["Meter shows a fight as it happens: a meter of what the game itself counts, and a threat bar while your "
  .. "group is fighting. It reads the game only: it does nothing for you, keeps nothing and says nothing in "
  .. "chat."] = nil
L["Show the meter"] = nil
L["Damage, healing and more for everyone in the fight, as the game counts them, live. Click its first line "
  .. "to choose what to show (damage, DPS, healing, HPS, damage taken, absorbs, interrupts, dispels) and "
  .. "which fight, right-click it for this fight or overall, and drag it to move it."] = nil
L["Show the threat bar while my group is fighting"] = nil
L["A small bar per group member for the enemy you have targeted: how close each is to pulling it. With a "
  .. "friend targeted, it follows the enemy that friend is on, from the nameplates on screen. Past 80% a bar "
  .. "turns orange. Hidden when you are alone. Drag it to move it; /ryna meter threat shows it with an "
  .. "example so you can place it outside a fight."] = nil

-- Bars.lua: the meter's views, as its first line names them.
L["Damage"] = nil
L["DPS"] = nil
L["Healing"] = nil
L["HPS"] = nil
L["Damage taken"] = nil
L["Absorbs"] = nil
L["Interrupts"] = nil
L["Dispels"] = nil
-- Beside the view: which tally, and (after " · ") the fight's length.
L["this fight"] = nil
L["last fight"] = nil
L["an earlier fight"] = nil
L["overall"] = nil
-- The hint on the first line.
L["Click: choose what to show. Right-click: this fight or overall. Drag to move."] = nil
-- The pick-list a click opens: its two headings, the two standing tallies, and a past fight the game
-- gave no name for. %d: the game's number for that fight.
L["Show"] = nil
L["From"] = nil
L["This fight (or the last one)"] = nil
L["Overall"] = nil
L["Fight %d"] = nil
-- Under the rows when there is nothing to show.
L["The game's meter isn't available here."] = nil
L["Nothing yet."] = nil

-- ThreatBar.lua: the bar on screen during a fight.
-- A row's right side for whoever the enemy is attacking, and (numbers hidden) for someone near pulling it.
L["has it"] = nil
L["close"] = nil
-- Beside the enemy's name: who it is on. %s: a group member's name.
L["on %s"] = nil
-- Stand-ins for a name the game didn't give.
L["An enemy"] = nil
L["Someone"] = nil
-- Under the rows, where the game won't give threat as numbers (a raid may not).
L["The game hides the numbers here."] = nil
-- Under the rows: the player's highest threat on another enemy. %s: that enemy; %d: percent.
L["Your highest: %s, %d%%"] = nil
L["Your highest elsewhere: %d%%"] = nil
-- The example the bar shows for /ryna meter threat.
L["Example enemy"] = nil
L["An example. Drag the bar where you want it."] = nil
L["The tank"] = nil
L["A damage dealer"] = nil
L["You"] = nil
