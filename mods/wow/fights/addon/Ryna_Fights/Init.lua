-- Init: Fights registers with the core (Ryna.RegisterMod). Loaded last, so every handler exists. At
-- login it reads the app's fights file (Data.lua) and adds its section to Ryna's window (Window.lua);
-- /ryna fights opens it there. It also starts the threat bar (ThreatBar.lua), which reads the game while
-- the group is fighting; /ryna fights threat shows that bar with an example, to be placed.

local _, ns = ...
if ns.off then return end
local L = ns.L

ns.API.RegisterMod({
  id = "fights",
  addon = ns.NAME,
  name = "Fights",
  version = ns.VERSION,
  onLogin = function()
    ns.Run("fights file", ns.Data.Load)
    ns.Run("fights window", ns.Window.Register)
    ns.Run("threat bar", ns.ThreatBar.Register)
    ns.Debug("fights: %d in the file (%s)", #ns.Data.list, tostring(ns.Data.why or "read"))
  end,
  onDefaults = function() ns.Run("threat bar place", ns.ThreatBar.ResetPlace) end,
})

local Slash = ns.API.Slash
if type(Slash) == "table" and type(Slash.Add) == "function" then
  Slash.Add({
    word = "fights",
    order = 70,
    help = L["/ryna fights: your recent fights in Ryna's window"],
    run = function(arg)
      -- /ryna fights threat: the bar with example rows, to be dragged into place; again to put it away.
      if type(arg) == "string" and arg:lower() == "threat" then
        if not ns.ThreatBar.On() then
          ns.Print(L["The threat bar is off: turn it on in /ryna, under Fights."])
        elseif ns.ThreatBar.Place() then
          ns.Print(L["The threat bar is showing an example: drag it where you want it, then type "
            .. "/ryna fights threat again."])
        end
        return
      end
      if not ns.Window.Open() then ns.Print(L["Fights needs a newer Ryna core: update the Ryna app."]) end
    end,
  })
end
