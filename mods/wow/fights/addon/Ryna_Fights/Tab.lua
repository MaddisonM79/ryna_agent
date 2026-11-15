-- Tab: Fights' tab in the core's panel (Ryna.Panel.AddTab), after Prices and before Messages, built
-- with the core's widgets: what the mod does, the meter's checkbox and the threat bar's.

local _, ns = ...
if ns.off then return end
local L = ns.L
local B = ns.ThreatBar

local W = ns.API.Panel and ns.API.Panel.Widgets
if type(W) ~= "table" or type(W.checkbox) ~= "function" then return end

local P = {}

local function build(f)
  P.intro = W.text(f, "GameFontHighlightSmall", 11)
  P.intro:SetText(L["Fights shows your recent fights in Ryna's window, a live meter, and a threat bar while your group "
    .. "is fighting. It reads the game and the fights the companion app found in your combat log: it does "
    .. "nothing for you and says nothing in chat."])
  P.bar = W.checkbox(f, L["Show the threat bar while my group is fighting"],
    L["A small bar per group member for the enemy you have targeted: how close each is to pulling it. With a "
      .. "friend targeted, it follows the enemy that friend is on, from the nameplates on screen. Past 80% a "
      .. "bar turns orange. Hidden when you are alone. Drag it to move it; /ryna fights threat shows it with "
      .. "an example so you can place it outside a fight."])
  W.toggle(P.bar, B.KEY, function()
    if not B.On() then B.Hide() end
    ns.API.Panel.RefreshIfShown()
  end)
  P.meter = W.checkbox(f, L["Show the meter"],
    L["Damage, healing and more for everyone in the fight, as the game counts them, live. Click its first "
      .. "line for the next view (damage, DPS, healing, HPS, damage taken, absorbs, interrupts, dispels), "
      .. "right-click it for this fight or overall, and drag it to move it. It only shows what the game "
      .. "counted: nothing is kept or sent."])
  W.toggle(P.meter, ns.Meter.KEY, function()
    ns.Meter.dirty = true
    ns.Meter.Tick()
    ns.API.Panel.RefreshIfShown()
  end)
end

local function layout(s, settings)
  s.text(P.intro, 0, 12)
  P.bar:SetChecked(settings[B.KEY] and true or false)
  P.meter:SetChecked(settings[ns.Meter.KEY] and true or false)
  P.meter.place(s, 4)
  P.bar.place(s, 4)
end

ns.API.Panel.AddTab({ key = "fights", label = L["Fights"], order = 39, build = build, layout = layout })
