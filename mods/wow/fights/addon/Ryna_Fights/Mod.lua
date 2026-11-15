-- Mod: Fights' namespace, built from the core's mod API (the `Ryna` table, the core addon's Mods.lua).
-- Fights is the Fights section of Ryna's window (docs/ryna-agent-cores-mods.md 7.5, "Fights, in the
-- manner of Details" and "The fight timeline"): the client refuses the combat log to addons, so the
-- Ryna app, which reads every fight from the log file, writes the recent ones into the core's
-- Data/fights.lua, which the game loads at login and /reload (Data.lua reads it). The list shows them
-- newest first; a fight's detail tells its story in sentences (Story.lua) and draws a row per player
-- and per enemy (Window.lua). It writes nothing (not even settings) and never prints to chat unasked.
--
-- Every file starts with `if ns.off then return end`: without the mod API (a core addon from before
-- it), Fights loads nothing at all, and says nothing either.

local addonName, ns = ...

local R = _G.Ryna
if type(R) ~= "table" or type(R.API) ~= "number" or R.API < 1 or type(R.RegisterMod) ~= "function" then
  ns.off = true
  return
end

ns.NAME = addonName
ns.API = R

-- The version is the TOC's, so there is no second copy to keep in step.
local metadata = (_G.C_AddOns and _G.C_AddOns.GetAddOnMetadata) or _G.GetAddOnMetadata
ns.VERSION = R.Try(metadata, addonName, "Version")

for _, name in ipairs({ "Try", "Fn", "IsSecret", "NonEmpty", "Now", "Print", "Debug", "Run", "ReportError",
  "Ago", "Plain", "Plural", "N", "Locale" }) do
  ns[name] = R[name]
end

-- Fights' strings (Locales/), falling back to the core's (Ryna.L), which fall back to the key.
ns.L = setmetatable({}, {
  __index = function(_, key) return R.L[key] end,
  __newindex = function(t, key, value)
    if type(value) == "string" and value ~= "" then rawset(t, key, value) end
  end,
})
