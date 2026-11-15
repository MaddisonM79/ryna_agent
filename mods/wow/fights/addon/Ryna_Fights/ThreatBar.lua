-- ThreatBar: the small frame that shows, while the group is fighting, how close each member is to
-- pulling one enemy (Threat.lua reads it; docs/ryna-agent-cores-mods.md 7.5, "The threat bar"). One
-- row per member, whoever holds the enemy on top, a full bar meaning "pulls". Past 80% a bar turns
-- orange and the frame's edge with it. It is silent: no chat line, no sound. It hides when the fight
-- ends, when alone, and while the setting (fightsThreatBar) is off.
--
-- Where the client hides the numbers (a raid may), the bar is still filled, because a status bar
-- takes a secret value; the rows keep the group's order, the percentage is left out and the words
-- come from the threat state. Nothing secret is compared, measured or kept.
--
-- The frame is dragged with the left button and its place is kept in RynaDB.fights.threatBar.
-- /ryna fights threat shows it with example rows, to be placed outside a fight.

local _, ns = ...
if ns.off then return end
local L = ns.L
local T = ns.Threat

local B = {}
ns.ThreatBar = B

B.KEY = "fightsThreatBar"
B.WIDTH, B.ROW, B.HEAD, B.PAD = 220, 17, 16, 4
B.INTERVAL = 0.25

ns.API.Settings.AddDefaults({ [B.KEY] = true })

local EDGE, EDGE_WARN = { 0.23, 0.21, 0.28 }, { 0.94, 0.54, 0.17 }
local WARN_BAR, PLAIN_BAR, GOLD = { 0.94, 0.54, 0.17 }, { 0.56, 0.54, 0.61 }, { 1, 0.82, 0 }
local TEXTURE = "Interface\\TargetingFrame\\UI-StatusBar"

function B.On()
  return ns.API.Settings.Get(B.KEY) and true or false
end

local function saved()
  local db = ns.API.DB()
  if type(db) ~= "table" then return nil end
  if type(db.fights) ~= "table" then db.fights = {} end
  return db.fights
end

local function fontString(parent, justify)
  local ok, fs = pcall(parent.CreateFontString, parent, nil, "OVERLAY", "GameFontHighlightSmall")
  if not ok or not fs then fs = parent:CreateFontString(nil, "OVERLAY") end
  fs:SetJustifyH(justify)
  if fs.SetWordWrap then fs:SetWordWrap(false) end
  return fs
end

local function colour(tex, c, a)
  tex:SetColorTexture(c[1], c[2], c[3], a or 1)
end

local function place(frame)
  local at = saved()
  at = at and at.threatBar
  frame:ClearAllPoints()
  if type(at) == "table" and type(at.point) == "string" and type(at.x) == "number" and type(at.y) == "number" then
    frame:SetPoint(at.point, _G.UIParent, type(at.rel) == "string" and at.rel or at.point, at.x, at.y)
  else
    frame:SetPoint("CENTER", _G.UIParent, "CENTER", 280, -60)
  end
end

local function newRow(frame, i)
  local bar = CreateFrame("StatusBar", nil, frame)
  bar:SetStatusBarTexture(TEXTURE)
  bar:SetMinMaxValues(0, 100)
  bar:SetHeight(B.ROW - 1)
  bar:SetPoint("TOPLEFT", frame, "TOPLEFT", B.PAD, -(B.PAD + B.HEAD + (i - 1) * B.ROW))
  bar:SetPoint("RIGHT", frame, "RIGHT", -B.PAD, 0)
  bar.back = bar:CreateTexture(nil, "BACKGROUND")
  bar.back:SetAllPoints()
  colour(bar.back, { 0.16, 0.15, 0.21 }, 0.9)
  bar.mine = bar:CreateTexture(nil, "OVERLAY")
  bar.mine:SetPoint("TOPLEFT")
  bar.mine:SetPoint("BOTTOMLEFT")
  bar.mine:SetWidth(2)
  colour(bar.mine, GOLD)
  bar.left = fontString(bar, "LEFT")
  bar.left:SetPoint("LEFT", bar, "LEFT", 5, 0)
  bar.right = fontString(bar, "RIGHT")
  bar.right:SetPoint("RIGHT", bar, "RIGHT", -4, 0)
  bar.left:SetPoint("RIGHT", bar.right, "LEFT", -4, 0)
  bar:Hide()
  return bar
end

local function build()
  if B.frame then return B.frame end
  local f = CreateFrame("Frame", "RynaThreatBar", _G.UIParent)
  f:SetSize(B.WIDTH, B.HEAD + B.ROW + 2 * B.PAD)
  f:SetFrameStrata("MEDIUM")
  f:SetClampedToScreen(true)
  f:SetMovable(true)
  f:EnableMouse(true)
  f:RegisterForDrag("LeftButton")
  f:SetScript("OnDragStart", function(self) self:StartMoving() end)
  f:SetScript("OnDragStop", function(self)
    self:StopMovingOrSizing()
    ns.Run("threat bar place", function()
      local point, _, rel, x, y = self:GetPoint(1)
      local at = saved()
      if at and type(point) == "string" then at.threatBar = { point = point, rel = rel, x = x, y = y } end
    end)
  end)

  f.back = f:CreateTexture(nil, "BACKGROUND")
  f.back:SetAllPoints()
  colour(f.back, { 0.08, 0.07, 0.1 }, 0.88)
  f.edges = {}
  for i, points in ipairs({
    { "TOPLEFT", "TOPRIGHT", "h" }, { "BOTTOMLEFT", "BOTTOMRIGHT", "h" },
    { "TOPLEFT", "BOTTOMLEFT", "v" }, { "TOPRIGHT", "BOTTOMRIGHT", "v" },
  }) do
    local e = f:CreateTexture(nil, "BORDER")
    e:SetPoint(points[1])
    e:SetPoint(points[2])
    if points[3] == "h" then e:SetHeight(1) else e:SetWidth(1) end
    f.edges[i] = e
  end

  f.title = fontString(f, "LEFT")
  f.title:SetPoint("TOPLEFT", f, "TOPLEFT", B.PAD + 2, -B.PAD)
  f.title:SetTextColor(GOLD[1], GOLD[2], GOLD[3])
  f.holder = fontString(f, "RIGHT")
  f.holder:SetPoint("TOPRIGHT", f, "TOPRIGHT", -(B.PAD + 2), -B.PAD)
  f.holder:SetTextColor(0.62, 0.59, 0.67)
  f.title:SetPoint("RIGHT", f.holder, "LEFT", -6, 0)
  f.foot = fontString(f, "LEFT")
  f.foot:SetPoint("BOTTOMLEFT", f, "BOTTOMLEFT", B.PAD + 2, B.PAD)
  f.foot:SetPoint("RIGHT", f, "RIGHT", -B.PAD, 0)
  f.foot:SetTextColor(0.62, 0.59, 0.67)

  f.rows = {}
  for i = 1, T.MAX_ROWS do f.rows[i] = newRow(f, i) end
  place(f)
  f:Hide()
  B.frame = f
  return f
end

local function classRGB(class)
  local colors = _G.RAID_CLASS_COLORS
  local c = class and type(colors) == "table" and colors[class]
  if type(c) == "table" and type(c.r) == "number" then return c.r, c.g, c.b end
  return PLAIN_BAR[1], PLAIN_BAR[2], PLAIN_BAR[3]
end

-- What a row says on the right: "has it", a percentage, or (numbers hidden) "close" or nothing.
function B.Label(r)
  if T.Holds(r) then return L["has it"] end
  if r.hidden then return T.Close(r) and L["close"] or "" end
  return string.format("%d%%", math.floor(r.pct + 0.5))
end

-- An enemy's name for a label. In combat it can be secret: it is passed on to the label untouched,
-- and only a missing one is replaced.
local function mobName(unit)
  local name = ns.Try(ns.Fn("UnitName"), unit)
  if ns.IsSecret(name) then return name end
  if type(name) == "string" and name ~= "" then return name end
  return L["An enemy"]
end

function B.Draw(snap)
  local f = build()
  f.title:SetText(snap.name or mobName(snap.mob))

  local holder
  for _, r in ipairs(snap.rows) do
    if not holder and T.Holds(r) then holder = r end
  end
  f.holder:SetText(holder and holder.name and string.format(L["on %s"], holder.name) or "")

  for i, bar in ipairs(f.rows) do
    local r = snap.rows[i]
    if r then
      bar.left:SetText(r.name or L["Someone"])
      bar.right:SetText(B.Label(r))
      if r.hidden then
        -- A secret goes straight to the bar: the client fills it, and nothing here reads it.
        bar:SetValue(r.pct)
      else
        bar:SetValue(r.tanking and 100 or math.min(r.pct, 100))
      end
      if T.Close(r) then
        bar:SetStatusBarColor(WARN_BAR[1], WARN_BAR[2], WARN_BAR[3])
      else
        bar:SetStatusBarColor(classRGB(r.class))
      end
      if r.me then bar.mine:Show() else bar.mine:Hide() end
      bar:Show()
    else
      bar:Hide()
    end
  end

  local foot
  if snap.note then
    foot = snap.note
  elseif snap.hidden then
    foot = L["The game hides the numbers here."]
  elseif snap.top then
    local pct = math.floor(snap.top.pct + 0.5)
    -- The enemy's name may be secret; if the client won't format it, the line goes without it.
    local ok, line = pcall(string.format, L["Your highest: %s, %d%%"], mobName(snap.top.mob), pct)
    foot = ok and line or string.format(L["Your highest elsewhere: %d%%"], pct)
  end
  if foot then
    f.foot:SetText(foot)
    f.foot:Show()
  else
    f.foot:Hide()
  end

  local edge = snap.warn and EDGE_WARN or EDGE
  for _, e in ipairs(f.edges) do colour(e, edge) end
  f.warn = snap.warn and true or false
  f:SetHeight(B.HEAD + #snap.rows * B.ROW + 2 * B.PAD + (foot and 14 or 0))
  f:Show()
end

function B.Hide()
  if B.frame then B.frame:Hide() end
end

-- One read and draw; the driver calls it four times a second.
function B.Tick()
  if B.placing then return end
  if not B.On() then return B.Hide() end
  local snap = T.Snapshot()
  if not snap then return B.Hide() end
  B.Draw(snap)
end

-- The bar with example rows, to be dragged into place outside a fight. Returns whether it is now
-- showing them. A fight starting ends it.
function B.Place()
  if B.placing then
    B.placing = false
    B.Hide()
    return false
  end
  B.placing = true
  local _, class = ns.Try(ns.Fn("UnitClass"), "player")
  B.Draw({
    name = L["Example enemy"],
    note = L["An example. Drag the bar where you want it."],
    warn = true,
    rows = {
      { name = L["The tank"], class = "WARRIOR", tanking = true, pct = 100, status = 3 },
      { name = L["A damage dealer"], class = "MAGE", tanking = false, pct = 86, status = 0 },
      { name = ns.NonEmpty(ns.Try(ns.Fn("UnitName"), "player")) or L["You"], class = class, me = true,
        tanking = false, pct = 31, status = 0 },
    },
  })
  return true
end

function B.ResetPlace()
  local at = saved()
  if at then at.threatBar = nil end
  if B.frame then place(B.frame) end
end

function B.Register()
  if B.driver then return end
  local driver = CreateFrame("Frame")
  local since = 0
  driver:SetScript("OnUpdate", function(_, elapsed)
    since = since + (tonumber(elapsed) or 0)
    if since < B.INTERVAL then return end
    since = 0
    ns.Run("threat bar", B.Tick)
  end)
  B.driver = driver
  ns.API.On("PLAYER_REGEN_DISABLED", function() B.placing = false end)
end
