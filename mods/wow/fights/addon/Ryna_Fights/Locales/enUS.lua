-- English (enUS): every string Ryna Fights shows, keyed by the English text itself. `true` means "the
-- text is the key"; ns.L returns the key for anything no locale file translates, so English is always
-- the fallback. The other Locales/*.lua files are templates for translators, generated from this one:
-- after adding, changing or removing a string here, run (from the repository root)
--
--   node cores/wow/addon/tools/strings.mjs --templates
--
-- which rewrites them with the same keys, keeping every translation already in them. Without
-- --templates it only checks that this list matches what the code uses (L["..."] and N("...")).
-- Debug lines (ns.Debug) are English on purpose and are not listed. A key the core addon's
-- Locales/enUS.lua already has isn't repeated here: Fights' L falls back to it. The Fights section
-- never says "threat" (its enemy rows say whom an enemy was attacking); only the threat bar does.

local _, ns = ...
if ns.off then return end
local L = ns.L

-- Init.lua
-- /ryna's help line for the command.
L["/ryna fights: your recent fights in Ryna's window"] = true
-- Said when /ryna fights is typed with a core whose window can't draw the section.
L["Fights needs a newer Ryna core: update the Ryna app."] = true
-- Answers to /ryna fights threat.
L["The threat bar is off: turn it on in /ryna, under Fights."] = true
L["The threat bar is showing an example: drag it where you want it, then type /ryna fights threat again."] = true

-- Tab.lua: the Fights tab of /ryna.
L["Fights shows your recent fights in Ryna's window, a live meter, and a threat bar while your group is "
  .. "fighting. It reads the game and the fights the companion app found in your combat log: it does nothing "
  .. "for you and says nothing in chat."] = true
L["Show the threat bar while my group is fighting"] = true
L["A small bar per group member for the enemy you have targeted: how close each is to pulling it. With "
  .. "a friend targeted, it follows the enemy that friend is on, from the nameplates on screen. Past 80% "
  .. "a bar turns orange. Hidden when you are alone. Drag it to move it; /ryna fights threat shows it "
  .. "with an example so you can place it outside a fight."] = true

L["Show the meter"] = true
L["Damage, healing and more for everyone in the fight, as the game counts them, live. Click its first line "
  .. "for the next view (damage, DPS, healing, HPS, damage taken, absorbs, interrupts, dispels), right-click "
  .. "it for this fight or overall, and drag it to move it. It only shows what the game counted: nothing is "
  .. "kept or sent."] = true
-- Answers to /ryna fights meter.
L["The meter is on."] = true
L["The meter is off."] = true

-- Meter.lua: the meter's views, as its first line names them.
L["Damage"] = true
L["DPS"] = true
L["Healing"] = true
L["HPS"] = true
L["Absorbs"] = true
L["Interrupts"] = true
L["Dispels"] = true
-- Beside the view: which tally, and (after " · ") the fight's length.
L["this fight"] = true
L["overall"] = true
-- The hint on the first line.
L["Click: the next view. Right-click: this fight or overall. Drag to move."] = true
-- Under the rows when there is nothing to show.
L["The game's meter isn't available here."] = true
L["Nothing yet."] = true

-- ThreatBar.lua: the bar on screen during a fight.
-- A row's right side for whoever the enemy is attacking, and (numbers hidden) for someone near pulling it.
L["has it"] = true
L["close"] = true
-- Beside the enemy's name: who it is on. %s: a group member's name.
L["on %s"] = true
-- Stand-ins for a name the game didn't give.
L["An enemy"] = true
L["Someone"] = true
-- Under the rows, where the game won't give threat as numbers (a raid may not).
L["The game hides the numbers here."] = true
-- Under the rows: the player's highest threat on another enemy. %s: that enemy; %d: percent.
L["Your highest: %s, %d%%"] = true
L["Your highest elsewhere: %d%%"] = true
-- The example the bar shows for /ryna fights threat.
L["Example enemy"] = true
L["An example. Drag the bar where you want it."] = true
L["The tank"] = true
L["A damage dealer"] = true
L["You"] = true

-- Window.lua: the section's name in the rail, its title, and a fight's title. %s: what the fight was.
L["Fights"] = true
L["Fights › %s"] = true
-- A boss's name and its result ("Captain Gull, kill"). %s: the name; %s: the result, lower case.
L["%s, %s"] = true
-- What a fight was when it wasn't a boss. %s: the zone.
L["%s, trash"] = true
L["%s, open world"] = true
-- The zone when the log never named it.
L["somewhere"] = true
-- A boss fight's result.
L["Kill"] = true
L["Wipe"] = true
-- The list's columns.
L["When"] = true
L["What"] = true
L["Length"] = true
L["Result"] = true
L["Done"] = true
L["Taken"] = true
-- The list's tiles. "of %d tries": how many boss fights there were.
L["Bosses killed"] = true
L["of %d try"] = true
L["of %d tries"] = true
L["Your deaths"] = true
-- A list row's tooltip.
L["Players"] = true
L["Your damage done"] = true
L["Your damage taken"] = true
L["Your healing done"] = true
L["Deaths"] = true
-- The header on a fight: its length and how many players; then the buttons.
L["%d player"] = true
L["%d players"] = true
L["‹ Earlier"] = true
L["Later ›"] = true
L["All fights"] = true
-- A fight's rows: players past the file's 25 rows, folded; the enemies' heading.
L["%d more player"] = true
L["%d more players"] = true
L["Enemies"] = true
L["Was attacking, in order"] = true
-- Enemies past the three shown (a click lists them), or past the eight the file keeps.
L["%d more enemy"] = true
L["%d more enemies"] = true
L["Click to list them."] = true
L["Not kept: the file keeps the eight that mattered most."] = true
-- Whom an enemy was attacking: a name and for how many seconds ("Corin 9s"); more than the file keeps.
L["%s %ds"] = true
L["%d more"] = true
L["nobody of the group"] = true
-- A player's and an enemy's tooltip.
L["%s's pet: its damage done is in its owner's row."] = true
L["Damage done"] = true
L["Damage taken"] = true
L["Healing done"] = true
L["Died at"] = true
L["Was attacking, in order:"] = true
-- From one time to another in the fight ("0:10 to 0:19").
L["%s to %s"] = true
L["and %d more"] = true
-- The footer: where the numbers come from and how old the file is. %s: the core's "2 hours ago".
L["From your combat log, as the Ryna app read it, as of %s; newer fights appear after /reload."] = true
L["From your combat log, as the Ryna app reads it; newer fights appear after /reload."] = true
L["an unknown time"] = true
L["The fights file is from another version: update the Ryna app and the addon."] = true
-- The list with nothing in it.
L["Fights appear here once the Ryna app has read your combat log and the game is reloaded."] = true
L["None of these fights was logged by this character."] = true

-- Story.lua: a fight in sentences. Times are "0:08" (minutes and seconds from the fight's start).
-- The first blow: who struck, with what (a spell's name, or "a melee swing"), on whom, when. It says who
-- struck first, never who pulled: the log can't tell.
L["%s opened with %s on %s at %s."] = true
L["%s opened with %s on %s at %s, and missed."] = true
L["a melee swing"] = true
-- A player's pet: "Ember Quill's Wolf". %s: the player; %s: the pet.
L["%s's %s"] = true
-- An enemy with its number among enemies of the same name ("Dockside Cutter 2").
L["%s %d"] = true
-- After the first blow when the other side struck within a quarter second (the log's order may not be the game's).
L["The enemy struck back within a quarter second."] = true
L["The group struck back within a quarter second."] = true
L["The log shows no blow between the group and an enemy."] = true
-- The enemy's answer when the group struck first. %s: the enemy; %s: whom; %s: when.
L["%s went for %s at %s."] = true
-- Deaths in order: "Dela died at 0:26" then ", Aster at 0:29" for each next one, then ", and 3 more".
L["%s died at %s"] = true
L[", %s at %s"] = true
L[", and %d more"] = true
L["Nobody died."] = true
-- A name the log didn't give.
L["someone"] = true
