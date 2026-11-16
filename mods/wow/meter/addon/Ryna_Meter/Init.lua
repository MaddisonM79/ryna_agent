-- Init: Meter registers with the core (Ryna.RegisterMod). Loaded last, so every handler exists. At
-- login it starts the meter (Bars.lua) and the threat bar (ThreatBar.lua). /ryna meter turns the meter
-- on or off; /ryna meter threat shows the threat bar with an example, to be dragged into place.

local _, ns = ...
if ns.off then return end
local L = ns.L

ns.API.RegisterMod({
  id = "meter",
  addon = ns.NAME,
  name = "Meter",
  version = ns.VERSION,
  onLogin = function()
    ns.Run("meter bars", ns.Bars.Register)
    ns.Run("threat bar", ns.ThreatBar.Register)
  end,
  onDefaults = function()
    ns.Run("meter place", ns.Bars.ResetPlace)
    ns.Run("threat bar place", ns.ThreatBar.ResetPlace)
  end,
})

local Slash = ns.API.Slash
if type(Slash) == "table" and type(Slash.Add) == "function" then
  Slash.Add({
    word = "meter",
    order = 72,
    help = L["/ryna meter: the meter on or off; /ryna meter threat: place the threat bar"],
    run = function(arg)
      if type(arg) == "string" and arg:lower() == "threat" then
        if not ns.ThreatBar.On() then
          ns.Print(L["The threat bar is off: turn it on in /ryna, under Meter."])
        elseif ns.ThreatBar.Place() then
          ns.Print(L["The threat bar is showing an example: drag it where you want it, then type "
            .. "/ryna meter threat again."])
        end
        return
      end
      local on = not ns.Bars.On()
      ns.API.Settings.Set(ns.Bars.KEY, on)
      ns.Bars.dirty = true
      ns.Bars.Tick()
      ns.Print(on and L["The meter is on."] or L["The meter is off."])
    end,
  })
end
