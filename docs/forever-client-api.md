# World of Warcraft: Forever, the client API

The reference for writing addons against Forever's client. Read it before writing or changing any
`Ryna*` addon. Everything here comes from a source named next to it; where nothing settles a
question it says **unknown** and the probe that would settle it is in "Still owed". If a probe in
game contradicts this page, the probe wins: fix the page in the same change.

## Sources

Citations use these keys, with a path or line after. All were read on 2026-11-08.

| Key | What | Where |
|---|---|---|
| Disk | The installed client (read only) | `/Applications/World of Warcraft/_forever_` and its parent |
| ElvUI | ElvUI v16.01, Forever build (custom licence: read, never copy) | `Interface/AddOns/ElvUI*` |
| BenikUI | ElvUI_BenikUI 5.20 (custom licence) | `Interface/AddOns/ElvUI_BenikUI` |
| Questie | Questie 12.6.2 and QuestieDB 1.0.11 (licence in their `LICENSE.md`) | `Interface/AddOns/Questie*` |
| ATT | AllTheThings 5.4.7 (MIT) | `Interface/AddOns/AllTheThings` |
| Pawn | Pawn 2.14.1, VgerCore 1.0.23 (CC BY-NC-ND 3.0) | `Interface/AddOns/Pawn` |
| Doud | Doud's Dungeon Journal 1.1.0 (all rights reserved) | `Interface/AddOns/DoudJournal` |
| BVP | Better Vendor Price 2.1.3 (no licence file) | `Interface/AddOns/BetterVendorPrice` |
| AQ | AutoQuests 3.1.0 (no licence file) | `Interface/AddOns/AutoQuests` |
| Blz | Blizzard's UI source for Forever: Gethe/wow-ui-source, branch `forever`, commit `9dc84f0`, "1.60.1 (70862)" | https://github.com/Gethe/wow-ui-source/tree/forever |
| Docs | Blizzard's generated API docs in that branch | `Interface/AddOns/Blizzard_APIDocumentationGenerated/` |
| W160 | warcraft.wiki.gg, Patch 1.60.1/API changes (Forever against retail 12.1.5) | https://warcraft.wiki.gg/wiki/Patch_1.60.1/API_changes |
| W120 | warcraft.wiki.gg, Patch 12.0.0/API changes | https://warcraft.wiki.gg/wiki/Patch_12.0.0/API_changes |
| WSV | warcraft.wiki.gg, Secret Values | https://warcraft.wiki.gg/wiki/Secret_Values |
| WTOC | warcraft.wiki.gg, TOC format | https://warcraft.wiki.gg/wiki/TOC_format |
| Team | This repo's probes: commit messages, READMEs, code comments | as cited, 2026-11-07 and 08 |
| Details | Details! Damage Meter, source at commit `256a0d7` (2026-11-09), read 2026-11-15; it ships `Details_Camelot.toc` (16001) and is not installed here (all rights reserved: read, never copy) | https://github.com/Tercioo/Details-Damage-Meter |
| Blz70938 | The same `forever` branch at commit `bebcb6b`, "1.60.1 (70938)", read 2026-11-15 for the sections on secrets, the damage meter and threat | as Blz |

The installed third-party addons are evidence of what the API does, nothing more. Their code is
never copied into ours.

## Which client this is

**Forever is Blizzard's own game, not a private server.** The wiki: released 4 November 2026, a
standalone branch "running concurrently with both retail and Classic", developed as "Camelot"
(https://warcraft.wiki.gg/wiki/World_of_Warcraft:_Forever). The install is a Battle.net product:
`.build.info` names product `wowf` and Blizzard's CDN hosts (Disk `../.build.info`).

| Fact | Value | Source |
|---|---|---|
| Version and build | 1.60.1.70862 | Disk `../.build.info`, `_forever_/World of Warcraft.app/Contents/Info.plist` (`CFBundleVersion`) |
| Product, flavour | `wowf` | Disk `.flavor.info`, `../.product.db`, `WTF/Config.wtf` (`agentUID "wowf"`) |
| Engine | Retail (Midnight) 12.1.5 with Forever's changes; it "shares Mainline WoW's UI architecture, including the vast majority of APIs available in 12.1.5" | W160 intro and its "12.1.5 (69952)" to "1.60.1 (70205)" diff |
| Ruleset | Classic | QuestieDB `src/config.lua:66` (`rules = "Classic"`); Pawn treats it with Classic stats and scales (`Pawn/ScaleTemplates.lua:14`) |
| Interface number | **16001** | Every Forever addon's TOC (Disk `Interface/AddOns/*/*.toc`); WTOC's table ("Forever 16001"); https://warcraft.wiki.gg/wiki/GetBuildInfo (row 1.60.1, build 70790, 16001) |
| `WOW_PROJECT_ID` | 18, `WOW_PROJECT_CAMELOT` (also 18) | In game: `/dump WOW_PROJECT_ID, WOW_PROJECT_CAMELOT` gave `18, 18` (Team, 2026-11-08); combat log header `PROJECT_ID,18` (Disk `Logs/WoWCombatLog-110826_113736.txt`, line 1); https://warcraft.wiki.gg/wiki/WOW_PROJECT_ID; W160 ("Added `WOW_PROJECT_CAMELOT` (18) constant", build 70170) |
| `GetBuildInfo()` | `"1.60.1", "70862", "Nov  6 2026", 16001, "", " "`: six values, the retail shape (no Classic seventh `currentVersion`) | In game (Team, 2026-11-08); shape per https://warcraft.wiki.gg/wiki/GetBuildInfo |
| TOC game type | `camelot`, with `forever` as an alias since build 70291; family `mainline` | WTOC; W160 |

Our TOCs say `## Interface: 16001` (Disk `Ryna/Ryna_Core.toc`), which is verified.

### Declaring the interface and loading per flavour

- A TOC may list several numbers. The Forever addons do (`## Interface: 120105, 16001` in
  `AutoQuests.toc`; ATT lists nine). Retail is 120105, its test realm 120107 (WTOC).
- Flavour-specific TOCs: Questie ships `Questie_Forever.toc` and `Questie_Camelot.toc` (16001) and
  a fallback `Questie.toc` with `## Interface: 00000` titled "game client not supported" (Questie
  `Questie.toc:1-6`). The client picks a game-suffixed TOC first, then a family one (`_Mainline`)
  (WTOC).
- Per-file conditions work: `Game\load_forever.xml [AllowLoadGameType camelot][ExcludeLoadGameType standard, classic]`
  (ElvUI `ElvUI.toc:40`, the exact pattern WTOC recommends). Blizzard's own TOCs use the same
  directives (Blz `Blizzard_TrainerUI/Blizzard_TrainerUI.toc`: `[Family]\Blizzard_TrainerUI_Camelot.lua [AllowLoadGameType camelot]`).
- An unrecognised game type in `AllowLoadGameType` makes the condition pass (WTOC), and retail
  doesn't know `forever` (W160): use `camelot` with `ExcludeLoadGameType`.

### Detecting Forever at runtime

What the Forever addons do:

| Addon | Test | Source |
|---|---|---|
| Pawn | `select(4, GetBuildInfo())` > 16000 and < 20000 | Pawn `VgerCore/VgerCore.lua:25` |
| ATT | the same, `>= 16001 and < 20000` | ATT `src/WoW API Wrappers.lua:3` |
| ElvUI's LibDualSpec | the same range | ElvUI `ElvUI_Libraries/Game/Shared/LibDualSpec-1.0/LibDualSpec-1.0.lua:72-75` |
| ElvUI | `WOW_PROJECT_ID == WOW_PROJECT_CAMELOT`, then `E.Modern = E.Retail or E.Forever` | ElvUI `Game/Shared/General/Initialize.lua:111,114` |

W160's own example is the interface range. **Use the interface range** (16000 to 19999) as the
test: it needs no constant that an older or newer build might lack.

What not to key on:

- **`E.Modern` or anything like it** to mean "retail gameplay". Forever has the retail engine and
  Classic rules: hunter pet happiness, loyalty and diet (ElvUI `Game/Shared/Tags/Shared.lua:1267`),
  a ranged slot (Pawn `VgerCore.lua:50`), three talent trees with dual spec. ElvUI uses `E.Modern`
  only for engine questions (which frame exists, whether CLEU may be registered).
- **`WOW_PROJECT_ID` alone, or `== WOW_PROJECT_MAINLINE` as "this is retail"**: before build 70170
  Forever reported retail's id (forum thread cited in W160). It reports 18 now, but don't depend on it.
- **`ClassicExpansionAtLeast(...)`**: "ClassicExpansionAtLeast is always true in Forever" (ElvUI
  `LibDualSpec-1.0.lua:72`).
- **`WOW_PROJECT_ID == WOW_PROJECT_CLASSIC`**: false on Forever; "Classic Era; does not include
  Forever" (Pawn `VgerCore.lua:24`).

## The rules of the engine

Each entry: what we expected, what Forever does, how we know, what to do.

### Combat log and `COMBAT_LOG_EVENT_UNFILTERED`

- **Expected**: Classic addons read CLEU.
- **Forever**: registering it is refused; Ryna 0.2.2 got the "blocked" dialog at login (Team
  `mods/wow/chronicle/addon/Ryna_Chronicle/Chronicle.lua:126`). Retail 12.0 made it an error (W120),
  the event carries `HasRestrictions = true` (Docs `CombatLogDocumentation.lua:132-133`), and ElvUI
  registers it only `not E.Modern` (ElvUI `Game/Shared/Modules/DataTexts/DPS.lua:67`).
- **Do instead**: the game still writes `Logs/WoWCombatLog-*.txt` (header
  `COMBAT_LOG_VERSION,22,ADVANCED_LOG_ENABLED,1,BUILD_VERSION,1.60.1,PROJECT_ID,18`, Disk). The
  agent reads the file; in game, use chat lines, unit events and `UNIT_HEALTH` as Chronicle does.
- The `CombatLogGetCurrentEventInfo` globals are deprecation shims that load only when the
  `loadDeprecationFallbacks` CVar is on (Blz `Blizzard_DeprecatedCombatLog/Deprecated_CombatLog.lua:4-17`).
- Forever adds a swing timer API instead: `C_SwingTimer`, `PLAYER_SWING` (W160).

### Secret values

- **Expected**: Classic has none.
- **Forever**: Midnight's "addon disarmament changes ... including secrets" are active (blue post
  2026-09-15, quoted in W160). Tainted code that compares, does arithmetic on, takes `#` of,
  indexes or uses a secret as a key gets "an immediate Lua error"; storing, passing and
  concatenating are allowed (WSV). `UnitName` of a non-player unit is secret in combat; cast info
  of other units is secret (WSV). Forever adds `GetFrameStrata` returning a secret (W160).
- **Seen in Forever addons**: comparing a secret throws (Doud `Core.lua:13-15`); enemy casts are
  often secret (Doud `Core.lua:163`); Questie skips an NPC whose GUID is secret
  (`Modules/Auto/AutoQuesting.lua:225-227`) and aura reads in combat and instances
  (`Database/QuestXP/QuestieXP.lua:42-45`); BVP avoids money frames in instance combat (BVP
  `BetterVendorPrice.lua:157`); ChatThrottleLib drops secret text (ElvUI
  `ElvUI_Libraries/Game/Shared/Ace3/AceComm-3.0/ChatThrottleLib.lua:282`).
- **Functions**: `issecretvalue`, `issecrettable`, `canaccessvalue`, `canaccesstable`,
  `canaccessallvalues`, `canaccesssecrets`, `hasanysecretvalues`, `scrubsecretvalues`,
  `secretwrap`, `dropsecretaccess` (Docs `FrameScriptDocumentation.lua`), and `C_Secrets.*`
  predicates such as `HasSecretRestrictions()` (retail docs, `SecretPredicateAPIDocumentation.lua`
  on `live`), which exists and returns true in game (Team, 2026-11-08).
- **Do instead**: pass every value from the game through a guard like the core's `IsSecret`
  before comparing it, and keep combat-time reads to a minimum.
- **A secret can be drawn.** "Secret" stops code from computing with a value, not from showing
  it. These take a secret from addon code (`SecretArguments = "AllowedWhenTainted"`, Docs at
  Blz70938): `StatusBar:SetValue` and `SetMinMaxValues`, `FontString:SetText` and
  `SetFormattedText`, `Texture:SetVertexColor`, `SetAlpha`, `SetTexture`, and the formatters
  `AbbreviateNumbers`, `AbbreviateLargeNumbers`, `BreakUpLargeNumbers`. The widget then carries a
  secret aspect (`Enum.SecretAspect.BarValue`, `Text`, ...), so reading it back is refused. Details
  draws its whole in-combat window this way (Details `classes/class_damage.lua`,
  `UpdateBarApocalypseWow`: `statusbar:SetMinMaxValues(0, topValue)`, `SetValue(value)`,
  `AbbreviateNumbers(source.totalAmount, ...)` into `SetText`).
- **A share of a secret** comes from a curve, not from division: `C_CurveUtil.CreateCurve()`,
  `AddPoint(0, 0)`, `AddPoint(total, 100)`, `curve:Evaluate(amount)` (Details
  `frames/window_breakdown_midnight/breakdown_dataprovider.lua:229-233`, in a `pcall`).
- **Which restriction is on**: `C_RestrictedActions.GetAddOnRestrictionState(type)` and
  `IsAddOnRestrictionActive(type)`, with `Enum.AddOnRestrictionType` Combat 0, Encounter 1,
  ChallengeMode 2, PvPMatch 3, Map 4, Chat 5 and states Inactive 0, Activating 1, Active 2; the
  event is `ADDON_RESTRICTION_STATE_CHANGED` (Docs `RestrictedActionsDocumentation.lua`,
  `RestrictedActionsConstantsDocumentation.lua`). Each guarded function names its own rule
  (`SecretWhenInCombat`, `SecretWhenUnitIdentityRestricted`, ...; all listed with their meaning in
  Docs `SecretPredicatesDocumentation.lua`), and `C_Secrets.Should...BeSecret(...)` asks about one
  call before making it. **Read the rule on the exact function before saying it can't be read.**

### Items: `GetItemInfo` and loading

- **Expected**: the global `GetItemInfo`, returning nil for an uncached item.
- **Forever**: the global is nil (Team, probe 2026-11-08, `mods/wow/README.md`). It was namespaced
  in 10.2.6 (https://warcraft.wiki.gg/wiki/API_C_Item.GetItemInfo), and no Blizzard code on the
  `forever` branch calls the global (Blz, grep). `C_Item.GetItemInfo` has 18 returns ending in
  `itemDescription` (Docs `ItemDocumentation.lua`; added in 12.0.0 per the wiki page), and is
  flagged `MayReturnNothing = true`: an unloaded item gives **zero values**, not nil, and a bare id
  does not load it (Team probe; Docs).
- **Verified in game (Team, 2026-11-08)**: for loaded Linen Cloth,
  `/run print(select("#", C_Item.GetItemInfo(2589)), select(11, C_Item.GetItemInfo(2589)))` printed
  `18 13 7 5 0 0 nil true`: 18 returns, `sellPrice` 13 copper, then `classID`, `subclassID`,
  `bindType`, `expansionID`, `setID`, `isCraftingReagent`.
- **Do instead**: `C_Item.RequestLoadItemDataByID`, then `ITEM_DATA_LOAD_RESULT` or
  `GET_ITEM_INFO_RECEIVED`; or `Item:CreateFromItemID(id):ContinueOnItemLoad` (ATT
  `src/UI/Windows/Auctions.lua:57-59`). `C_Item.GetItemInfoInstant` works but has no price (Team).
  Item links start `|cnIQ2:`; read ids with `Hitem:(%d+)` (Team,
  `mods/wow/prices/addon/Ryna_Prices/Tooltip.lua:26`).
- Forever re-rolled some item qualities (Doud `UI.lua:28`) and has items no public database knows
  (Doud `Core.lua:383`).

### Tooltips

- **Forever**: BVP (`BetterVendorPrice.lua:198`), Questie (`Modules/Tooltips/Tooltip.lua:572`) and
  ATT (`src/Modules/Tooltip.lua:909`) use `TooltipDataProcessor.AddTooltipPostCall(Enum.TooltipDataType.Item, ...)`,
  never `OnTooltipSetItem`. `C_TooltipInfo.GetItemByID` lines carry
  `Enum.TooltipDataLineType.SellPrice` (BVP `BetterVendorPrice.lua:140-153`).
- **Do instead**: the data processor, never `HookScript("OnTooltipSetItem")`. Add text with
  `AddLine`, not money frames, in anything that can run in instance combat (BVP, above).

### Containers and the bank

- **Forever**: `C_Container` works (Team; BVP `Restack.lua:12-118`); `GetContainerItemInfo` is
  `MayReturnNothing` (Docs `ContainerDocumentation.lua`).
  There is a **keyring** (`GetKeyRingSize`, `showKeyring` CVar, W160). "Forever bank tabs are bag
  slots" (ElvUI `ElvUI_Options/Game/Shared/Bags.lua:134`), with no `bankTabPool` (ElvUI
  `Game/Shared/Skins/Bags.lua:546`).
- **Do instead**: `C_Container.*` only; walk bank bags as bags; count the keyring separately.

### Merchants

- **Forever**: Blizzard's Forever merchant frame is the mainline one
  (Blz `Blizzard_UIPanels_Game/Blizzard_UIPanels_Game.toc:110`, `[Family]\MerchantFrame.lua [AllowLoadGameType mainline]`)
  and reads `C_MerchantFrame.GetItemInfo(index)`, a table (Blz
  `Blizzard_UIPanels_Game/Mainline/MerchantFrame.lua:290`) with `name, texture, price, stackCount,
  numAvailable, isPurchasable, isUsable, hasExtendedCost, currencyID, spellID, isQuestStartItem`
  (Docs `MerchantFrameDocumentation.lua`). Retail 12.0 **removed**
  `GetMerchantItemInfo` (W120). `C_MerchantFrame.SellAllJunkItems` exists (ElvUI
  `Game/Shared/Modules/Bags/Bags.lua:61`).
- **Verified in game (Team, 2026-11-08, a weapons merchant)**: `GetMerchantItemInfo` is **nil**
  (Chronicle's fallback to it is for classic clients only). `C_MerchantFrame.GetItemInfo(1)` gave
  `{ name = "Cutlass", price = 2023, stackCount = 1, numAvailable = -1, isPurchasable = false, ... }`
  with `currencyID` and `spellID` nil. The window showed 20s 23c, so `price` is copper for
  `stackCount` pieces; -1 is unlimited stock. `isPurchasable` was false with 3s 14c on hand, so it
  may mean "can afford now"; **unverified**.

### Trainers

- **Forever**: `GetTrainerServiceInfo(i)`, `GetTrainerServiceCost(i)` and `BuyTrainerService(i)`
  work by index (Team, commits `5228639`, `5f98041`, 2026-11-07). The order is `serviceName,
  serviceType, texture, reqLevel, serviceSubText, category` (Blz
  `Blizzard_TrainerUI/Mainline/Blizzard_TrainerUI.lua:469`). `C_Trainer` has only
  `GetCategorizeTrainerUI`, `SetCategorizeTrainerUI`, `GetTrainerType` (Team `/dump C_Trainer`;
  W160 lists exactly those three as Forever additions; retail's `C_Trainer` has no functions,
  `TrainerDocumentation.lua` on `live`).
- **Resolved in game (Team, 2026-11-08, Brother Joshua, priest trainer, window open)**:
  `/dump GetNumTrainerServices(), GetTrainerServiceInfo(1)` gave `157, "Lesser Heal", "unavailable",
  135929, 10, "Rank 3", ""`, matching Blizzard's own loop (Blz `Blizzard_TrainerUI.lua:460-468`).
  An addon's 0 on 2026-11-07 was probably read at `TRAINER_SHOW`, before the list arrived (a
  guess). Automation's walk by index stays as a safety net.
- The window groups services into categories (`categorizeTrainerUI` CVar) except at pet trainers
  (Blz `Blizzard_TrainerUI/Mainline/Blizzard_TrainerUI_Camelot.lua:9-11`).

### Quests and gossip

- **Forever**: the quest frame is the mainline one and accepts with `AcceptQuest()` on
  `QUEST_DETAIL` (Blz `Blizzard_UIPanels_Game/Mainline/QuestFrame.lua:646`). AQ, built for 16001,
  drives the whole flow with `C_GossipInfo.*` on `GOSSIP_SHOW`, the global
  `GetNumAvailableQuests`/`SelectAvailableQuest` on `QUEST_GREETING`, then `AcceptQuest`,
  `CompleteQuest` and `GetQuestReward` (AQ `main.lua:63-163`). Forever adds
  `C_QuestLog.GetQuestTimers`, `GetTrivialRange`, `IsEliteQuest` (W160). Forever's own quests have
  ids around 86000 and up (Doud `Core.lua:724`).
- **Watch**: the NPC's GUID can be secret, so a quest-giver check by NPC id must fail safe
  (Questie, above). Native quest ids stay readable (Questie `Modules/QuestieCompat.lua:200-202`).
- **The quest log's rows** (Blz, read 2026-11-15 at the `forever` head "1.60.1 (70938)"): Forever's
  quest log is the mainline `QuestMapFrame` (`Blizzard_UIPanels_Game.toc:173-178` loads
  `[Family]\QuestMapFrame.lua [AllowLoadGameType mainline]`, camelot being in that family, with
  Camelot overrides). It walks `C_QuestLog.GetInfo(i)` for `i = 1, C_QuestLog.GetNumQuestLogEntries()`
  (`Mainline/QuestMapFrame.lua:1669-1705`): the log is one flat list where a header row has
  `isHeader = true` and its `title` is the heading (a zone or a category), and each quest after it
  belongs to the last header before it (`info.header = lastHeader`). `QuestInfo` has `title`,
  `questLogIndex`, `questID`, `level`, `isHeader`, `isCollapsed`, `headerSortKey` and more;
  `GetInfo` may return nil (Docs `QuestLogDocumentation.lua:191-204, 1547-1578`). Collapsed headers
  still list their quests there (Blizzard hides them by `info.header.isCollapsed`, line 1655).
  Chronicle records that heading on each active quest (`Quests.lua`); **not yet checked in game**
  (Still owed 14).

### `/who` and `C_FriendList`

- **Forever**: `/who` only from a key press or click (Team, commit `3e50f74`, 2026-11-07);
  `C_FriendList.SendWho` is `HasRestrictions = true` (retail docs) and takes a third argument,
  filters (Blz `Blizzard_GroupFinder_VanillaStyle/Mainline/WhoList.lua:178`). The Who list lives in
  the group finder (W160). Results: `C_FriendList.GetNumWhoResults`, `GetWhoInfo(i)`, a table
  (Blz `WhoList.lua:268`).
- **Do instead**: read `WHO_LIST_UPDATE` after the player's own `/who`; never send.

### Guild roster

- **Forever**: no roster API change is listed against 12.1.5 (W160), so `C_GuildInfo.GuildRoster`
  and `GetGuildRosterInfo` should behave as on retail; Forever adds preferred-play-settings
  functions (W160). Chronicle's guild scan has run on Forever (Team, `cores/wow/addon/README.md`
  "refreshed at login and every 10 minutes"). Return order of `GetGuildRosterInfo`: **unknown on
  Forever**, read by position with care.

### Group, units and names

- Names: a Forever character has a first and last name, and the last name sits where the realm
  goes (ElvUI `Game/Shared/General/Core.lua:70`; Pawn `Pawn.lua:588`, "unique across all
  rulesets/realms"). Ryna already uses `First-Last` (Team `cores/wow/addon/README.md`).
- GUIDs: "the server ID and spawn ID portions of creature, vehicle, pet and game
  object GUIDs have been replaced with obfuscated values" (blue post 2026-10-16 in W160).
  **Verified in game (Team, 2026-11-08)**: `UnitGUID("target")` on one Defias Cutpurse, three times
  over a minute out of combat, was `"Creature-0-6805-0-2-94-39FF3AAEEF"` each time. It splits on
  `-` as before, with the **NPC id (94)** in field 6. **Unknown**: whether it is secret in combat,
  and whether it survives a `/reload` or matches for another player.
- `UnitName("player")` returning "Unknown" before `PLAYER_LOGIN` was fixed in build 70170 (W160).
- Specs: the `GetSpecialization`-style deprecation globals do not load on Forever
  (`Blizzard_DeprecatedSpecialization.toc`: `AllowLoadGameType: classic, standard`, Blz); use
  `C_SpecializationInfo.*`, as ElvUI does (`Game/Shared/General/API.lua:71-72`).

### Auction house

- **Forever**: the modern auction house. Blizzard's `Blizzard_AuctionHouseUI` declares
  `AllowLoadGameType: mainline, cata, mists` (mainline includes camelot) and loads a Forever
  category file `[Game]\Blizzard_AuctionData.lua [AllowLoadGameType camelot]` (Blz
  `Blizzard_AuctionHouseUI/Blizzard_AuctionHouseUI.toc`). ElvUI skins `Blizzard_AuctionHouseUI` on
  Forever and has a Forever branch because "Forever shows the copper box" in bids (ElvUI
  `Game/Shared/Skins/AuctionHouse.lua:11-15,367`). So `C_AuctionHouse` is the API: `SendBrowseQuery`,
  `GetBrowseResults`, `ReplicateItems`, `GetNumReplicateItems`, `GetReplicateItemInfo`,
  `SupportsCopperValues` and the rest are in Docs `AuctionHouseDocumentation.lua`.
- **Not on Forever, as far as the source shows**: `QueryAuctionItems` and `GetAuctionItemInfo`.
  The only mention of `QueryAuctionItems` on the `forever` branch is the doc string saying
  `ReplicateItems` replaces it (Blz, grep).
- **Verified in game (Team, 2026-11-08, Stormwind auctioneer, 1.60.1.70862):** `C_AuctionHouse` is
  present, `QueryAuctionItems` and `GetAuctionItemInfo` are nil, `SupportsCopperValues()` and
  `FavoritesAreAvailable()` are true. After one search, `GetBrowseResults()[1]` was
  `itemKey { itemID = 1251, itemLevel = 1, ... }, totalQuantity = 13754, minPrice = 10,
  containsOwnerItem = false` (Linen Bandage at 10 copper); before any search it answers nothing.
  `ReplicateItems()`, then a minute later `GetNumReplicateItems()`, gave **192,300** rows: the whole
  house in one pull. `GetReplicateItemInfo(0)` (the index starts at 0) gave `"Simple Branch of
  Power", 135139, 1, 2, false, 10, "REQ_LEVEL_ABBR", 0, 0, 700, 0, nil, nil, nil, nil, 0, 15933,
  true`: the owner fields are **nil**, so the client hands addons no seller names. Prices are per
  listing; divide by `count` for a unit price.
- **Positions** (Docs `AuctionHouseDocumentation.lua`): 18 returns, `name, texture, count, qualityID,
  usable, level, levelType, minBid, minIncrement, buyoutPrice, bidAmount, highBidder,
  bidderFullName, owner, ownerFullName, saleStatus, itemID, hasAllInfo`, so the owner and bidder
  fields are the 12th to 15th. The Prices mod reads only the 3rd, 10th, 17th and 18th. Events:
  `REPLICATE_ITEM_LIST_UPDATE`, `AUCTION_HOUSE_BROWSE_RESULTS_UPDATED` (no payload),
  `AUCTION_HOUSE_BROWSE_RESULTS_ADDED`. The window is `AuctionHouseFrame` with `BrowseResultsFrame`,
  `ItemBuyFrame` and `CommoditiesBuyFrame` (Blz `Blizzard_AuctionHouseUI/Shared/Blizzard_AuctionHouseFrame.xml:4,82,99,107`);
  a browse row's tooltip is GameTooltip owned by the row (Blz `Blizzard_AuctionHouseTableBuilder.lua:79-82`).
- **Still unknown**: the replicate throttle (ATT allows itself one per 15 minutes); whether
  `hasAllInfo` is ever false; whether `minPrice` is per unit for stackable goods (10c reads like it);
  whether `REPLICATE_ITEM_LIST_UPDATE` and `AUCTION_HOUSE_BROWSE_RESULTS_UPDATED` fire on Forever.
  The Prices mod assumes the documented behaviour for each and says so (`mods/wow/README.md`).

### Mail

- **Forever**: the inbox is still the global API: `GetInboxHeaderInfo` (Blz
  `Blizzard_MailFrame/MailFrame.lua:340`), with `C_Mail` holding only `CanCheckInbox`,
  `HasInboxMoney`, `IsCommandPending`, `SetOpeningAll`, `GetCraftingOrderMailInfo` (retail docs,
  `MailInfoDocumentation.lua` on `live`; `C_Mail.CanCheckInbox` at Blz `MailFrame.lua:242`).
  Forever's mail frame differs in layout (ElvUI `Game/Shared/Skins/Mail.lua:81-89`).
- **Verified in game (Team, 2026-11-08)**: `GetInboxHeaderInfo(1)` for an auction-house mail gave
  15 values in Blizzard's order (Blz `MailFrame.lua:340`): icons, sender "Alliance Auction House",
  subject, money 0, CODAmount 0, daysLeft 29.9998 (a fraction), itemCount 1, wasRead false,
  `wasReturned` false, `textCreated` true (`MailFrame.lua:583`), canReply false, isGM false,
  firstItemQuantity 1, and a link with bonus fields after the id
  (`|cnIQ2:|Hitem:2569::::::8:1487:::::::|h[Linen Boots]|h|r`): parse with `Hitem:(%d+)` only.

### Loot and rolls

- **Forever**: Classic loot methods are back: master looter frames, `GroupLootFrame` additions,
  `C_LootFrame.TryAutoLoot`, `C_AutoLoot.SetUseAutoLootToggle`, `UnitHasLootInteraction` (W160).
  `GetLootSourceInfo` is the per-source read (Team, Chronicle `Chronicle.lua:436`).
  `GetLootRollItemInfo`, `RollOnLoot` and `GetLootSourceInfo` all exist (`/dump ... ~= nil` gave
  `true, true, true`, Team, 2026-11-08). Their return shapes on Forever are not yet checked.
- **The loot history is the modern one, by encounter** (game, 2026-11-23): `C_LootHistory` has
  `GetAllEncounterInfos`, `GetInfoForEncounter`, `GetLootHistoryTime`, `GetSortedDropsForEncounter` and
  `GetSortedInfoForDrop`, and `LOOT_HISTORY_UPDATE_ENCOUNTER(encounterID)` and
  `LOOT_HISTORY_UPDATE_DROP(encounterID, lootListKey)` fire as a roll goes. The encounter id is
  `ENCOUNTER_END`'s, so the game itself says which boss dropped what. A drop is `{ itemHyperlink,
  lootListKey, startTime, duration, allPassed, isTied, playerRollState, rollInfos, winner }`; `winner`
  (absent until decided) is `{ playerName, playerClass, playerGUID, roll, state, isSelf, isWinner }`, all
  plain inside a dungeon out of combat. `state` is `Enum.EncounterLootDropRollState`: 0 need, 3 greed,
  4 not yet rolled were seen; 1 off-spec, 2 transmog and 5 pass are the engine's and unseen.
  `LOOT_HISTORY_ROLL_COMPLETE`, `LOOT_HISTORY_ROLL_CHANGED` and `LOOT_HISTORY_FULL_UPDATE` are refused
  (not events here); `GetLootMethod` is missing. `START_LOOT_ROLL(rollID)` and
  `LOOT_ROLLS_COMPLETE(lootListKey)` fire; `LOOT_ITEM_ROLL_WON` didn't for another player's win.
  Chronicle's run drops read it (ryna-agent-cores-mods 7.7).

### Talents and specs

- **Forever**: three trees, dual spec. Blizzard draws them with the modern trait system: the
  Forever talent frame reads `C_Traits.GetGroupDisplayInfoByTreeID` and
  `C_Traits.GetGroupCurrencyInfo` (points per tree) and switches specs with
  `C_SpecializationInfo.SetActiveSpecGroup` (Blz `Blizzard_PlayerSpells/Camelot/ClassTalents/Blizzard_ClassTalentsFrame.lua:95,112`).
  ElvUI skins `PlayerSpellsFrame.TalentsFrame` with Forever-only tree headers and primary/secondary
  tabs (ElvUI `Game/Shared/Skins/PlayerSpells.lua:172-200`).
- **Verified in game (Team, 2026-11-08)**: `GetNumTalentTabs` and `GetTalentTabInfo` are **nil**;
  `C_ClassTalents.GetActiveConfigID()` is `4954925`. Points per tree come from the trait API, as
  Blizzard's frame gets them: `C_Traits.GetConfigInfo(configID).treeIDs[1]` for the tree,
  `C_Traits.GetGroupDisplayInfoByTreeID(treeID)` for each tree's `groupID` and `displayName`, and
  `C_Traits.GetGroupCurrencyInfo(configID, { groupID })`, whose `currencyInfos[1].spent` is the
  tree's points (Docs `SharedTraitsDocumentation.lua`; the tree header shows exactly that, Blz
  `Blizzard_PlayerSpells/Camelot/ClassTalents/Blizzard_ClassTalentsFrame.lua:331-340`). Chronicle
  reads it this way, with the tab globals kept for classic clients (`Character.lua`,
  `C.ScanTalents`; `mods/wow/chronicle/test/chronicle_talents_test.lua`).
- **Not a level effect**: the probe asked whether the functions exist, and the level-8 character
  only affects the values. Blizzard's Forever source defines `GetTalentTabInfo` only as shims in
  `Blizzard_DeprecatedSpecialization`, which declares `AllowLoadGameType: classic, standard` and so
  never loads on Forever (Blz, grep of `Interface`). Those shims wrap
  `C_SpecializationInfo.GetSpecializationInfo`, whose `pointsSpent` may be a second route; not yet
  checked. Expect zeros before the first talent point.

### Professions

- **Forever**: skill lines moved into a namespace: `C_SkillInfo.GetNumSkillLines`,
  `GetSkillLineInfo`, `GetSkillLineInfoByID`, `ExpandSkillHeader`, `AbandonSkill` were added and the
  global `AbandonSkill` removed (W160). The professions book is a Forever frame (ElvUI
  `Game/Shared/Skins/Professions.lua:691`).
- **Verified in game (Team, 2026-11-08)**: `GetProfessions` exists, the global `GetSkillLineInfo`
  is **nil**, and `C_SkillInfo.GetNumSkillLines()` is 21. Chronicle takes the `GetProfessions` path
  (its skill-line fallback is for classic clients). What `GetProfessions()` returns is not yet checked.

### Auras and dispels

- **Docs**: an aura's details are `SecretWhenUnitAuraRestricted` (`C_UnitAuras.GetAuraDataByIndex`,
  `GetAuraDataByAuraInstanceID`, `GetUnitAuras`, whose table has `ConditionalSecretContents`): secret
  "when combat, encounter, challenge mode, or PvP match addon restrictions are in effect", though a
  spell may be flagged never or always secret (Docs `UnitAuraDocumentation.lua`,
  `SecretPredicatesDocumentation.lua`). `GetUnitAuraInstanceIDs` and `IsAuraFilteredOutByInstanceID`
  carry no secret mark. Calls that take an aura instance id are `SecretArguments =
  "AllowedWhenUntainted"`: addon code may not pass them a secret.
- **The filter knows the class.** `"RAID"` with `"HARMFUL"` is "harmful auras the player can
  dispel"; `"RAID_PLAYER_DISPELLABLE"` is "someone in the player's raid can"; `"DISPELLABLE"` is any
  dispellable aura (Blz `Blizzard_FrameXMLUtil/AuraUtil.lua`, `AuraUtil.AuraFilters`). So what a class
  can cure is the game's to answer, not a table of ours.
- **A kind's colour without its name**: `C_UnitAuras.GetAuraDispelTypeColor(unit, auraInstanceID,
  curve)` maps the dispel type id through a colour curve (`C_CurveUtil.CreateColorCurve()`), and its
  result is secret when the aura is (Docs). The type ids used for the curve's points (Magic 1, Curse
  2, Disease 3, Poison 4) are from memory of retail addons, **not checked on Forever**.
- **Forever's cures are the old kind**: a spell per kind of affliction (a priest's Dispel Magic 527
  and Cure Disease 528 or Abolish Disease 552; a druid's Remove Curse 2782 and Abolish Poison 2893;
  a shaman's Cure Poison 526 and Cure Disease 2870, and totems), seen in ElvUI's LibDispel
  (`ElvUI_Libraries/Game/Shared/LibDispel/LibDispel.lua`, which treats Forever as vanilla).
- **Seen in game** (Maddison, a priest, out of combat, 2026-11-18): with Weakened Soul on her,
  `GetUnitAuraInstanceIDs("player","HARMFUL|RAID")` gave 0 and `"HARMFUL"` gave 1, so the filter
  tells "a debuff" from "one I can cure"; the aura's `name`, `icon`, `dispelName` and
  `auraInstanceID` were all plain; and `SetTexture(aura.icon)` drew it.
- **Unknown, all in combat**: whether the `"HARMFUL|RAID"` answer stays plain, which of an aura's
  fields go secret (the instance id above all: a secret one can't be passed to
  `GetAuraDispelTypeColor`), whether a secret icon and name draw, and whether the filter follows the
  class (a curse on a priest). Also whether a cure cast with nothing to cure is refused without a
  global cooldown.
- **Secure buttons** (`SecureActionButtonTemplate`, `RegisterUnitWatch`): a click on one may cast on
  its unit; its attributes, place and visibility can't be changed in combat. Cleanse
  (`Ryna-Mods/cleanse`) is built on that and **has not been run in the game yet**.

### The game's damage meter

- **Forever**: `Blizzard_DamageMeter` is allowed (`AllowLoadGameType: standard, camelot`), and
  `C_DamageMeter` is documented with `Environment = "All"` (Docs `DamageMeterDocumentation.lua`):
  `IsDamageMeterAvailable`, `GetAvailableCombatSessions` (id, name, duration),
  `GetCombatSessionFromID` / `FromType` (sources with `totalAmount`, `amountPerSecond`,
  `classFilename`, `isLocalPlayer`, deaths), `GetCombatSessionSourceFromID` / `FromType` (a source's
  spells), `GetSessionDurationSeconds`, `ResetAllCombatSessions`; events
  `DAMAGE_METER_COMBAT_SESSION_UPDATED`, `DAMAGE_METER_CURRENT_SESSION_UPDATED`, `DAMAGE_METER_RESET`.
  Meter types: damage done, dps, healing done, hps, absorbs, interrupts, dispels, damage taken,
  avoidable damage taken, deaths, enemy damage taken. Session types: overall 0, current 1, expired 2.
- **Watch**: the session reads are `SecretWhenInCombat`; a source's `name` and `sourceGUID` are
  conditionally secret and `specIconID` always is.
- **An addon's calls answer on Forever**: Details' Forever build has no combat-log parser at all
  (`Details_Camelot.toc` loads `core/parser_nocleu*.lua`) and takes everything from these calls
  (`Details:IsUsingBlizzardAPI` is true for `WOW_PROJECT_CAMELOT`, Details
  `classes/class_damage.lua:2151`). Not yet run on our client: probes 10 to 12 below.
- **During a fight** the game hands the sources already sorted, so a live window needs no
  comparison: Details reads `GetCombatSessionFromType` on `DAMAGE_METER_COMBAT_SESSION_UPDATED`
  and draws each source's secret amounts into its own bars (see "A secret can be drawn"). A
  source's spells are live too (`GetCombatSessionSourceFromType`).
- **After a fight** the values turn plain. Details polls once a second until `InCombatLockdown()`
  is false, the Combat restriction state is 0 and no field of the session is secret, then copies
  players, spells and per-target amounts into its own tables (Details `core/parser_nocleu1.lua`,
  `startWaitSecretDropTimer`, `getBlizzardSegmentData`). So a finished fight is in the addon's hands
  seconds after it ends, with no file from the app and no `/reload`.
- **Not in the meter** (Details marks them unsupported on this path, `core/parser_nocleu.lua:20-62`):
  overhealing, healing taken, friendly fire, resources gained, CC breaks, resurrections, cooldowns,
  buff and debuff uptime. Deaths come with a `deathRecapID` for `C_DeathRecap`.
- **Do instead of CLEU**: draw live bars from the session reads, harvest the finished fight when the
  secrets drop, and keep the log file for what the meter has no events for (the timeline, casts).

### Threat

- **Forever**: `UnitThreatSituation(unit[, mob])` and `UnitThreatLeadSituation` are
  `SecretWhenUnitThreatStateRestricted`; `UnitDetailedThreatSituation(unit, mob)` (isTanking, status,
  scaledPercentage, rawPercentage, rawThreat; `MayReturnNothing`) and `UnitThreatPercentageOfLead`
  are `SecretWhenUnitThreatValuesRestricted` (Docs `UnitDocumentation.lua`). The rules: a query
  with one unit, or with the player, their pet or an ally against a target or nameplate, is
  "generally not secret"; for values the docs say "a non-boss or nameplate target" (Docs
  `SecretPredicatesDocumentation.lua:160-167`). Ask first with
  `C_Secrets.ShouldUnitThreatStateBeSecret(unit, mob)` and `ShouldUnitThreatValuesBeSecret(unit, mob)`.
- **Verified in game (Maddison, 2026-11-15, The Deadmines, in combat, a healer)**:
  `ShouldUnitThreatStateBeSecret` and `ShouldUnitThreatValuesBeSecret("player","target")` are both
  false; `UnitDetailedThreatSituation("player","target")` on a Defias mob gave
  `false, 0, 16.92, 22, 47` and a moment later `false, 0, 30.77, 40, 156` (scaled is raw / 1.3, the
  ranged pull line); `("party1","target")` gave `false, 0, 6.15, 8, 36`; on the boss Mr. Smite
  `false, 0, 7.69, 10, 3` with the values rule still false; `UnitThreatSituation("player")` gave 0.
  With a friendly unit as the mob the detailed call returns nothing; a character's name as the
  unit also gave nothing.
- **How the enemy is named decides it (the same day, 16:05, Luna targeted)**: through a compound
  unit the values are secret: `UnitCanAttack("player","targettarget")` was true and
  `UnitDetailedThreatSituation("target","targettarget")` on Captain Greenskin dumped every value
  tagged `<secret>` (true, 3, 100, 255, 234). Through nameplates they are plain:
  `("target","nameplateN")` over the enemy plates printed `false 0 26.36 29 267`,
  `false 0 15.45 17 141` and `false 0 49.09 54 482`, and nothing for a plate whose enemy the unit
  wasn't on. Friendly players have nameplates too: skip them with `UnitCanAttack("player", unit)`.
- **Do**: name the enemy by `target` or walk `nameplate1..40`; never `targettarget`. The game's own
  unit tooltip prints "17% Threat" (seen on Captain Greenskin).
- **Unknown**: a raid boss, where the Encounter restriction may make the values secret. Plan for
  it: ask the `C_Secrets` rule before each read and, when it says secret, draw the value without
  reading it (a bar and the status colour) instead of numbers.
- **The combat log file has no threat events**, so threat over a fight exists only if the addon
  samples it during the fight.
- Details' Tiny Threat plugin ships a Forever TOC and does arithmetic on these values (Details
  `plugins/Details_TinyThreat/Details_TinyThreat.lua:297`).

### Collections, the encounter journal and load on demand

- **Blizzard's UI**: `Blizzard_Collections` and `Blizzard_AchievementUI` are not allowed on Forever's
  game type (their TOCs on the `forever` branch list `wrath, cata, mists, mainline` and `mainline,
  tbc, wrath, cata, mists`); `Blizzard_EncounterJournal` lists `standard, classic`.
- **Verified in game (Team, 2026-11-09)**: the journals answer without that UI.
  `C_MountJournal.GetMountIDs()` lists 136 (the `Mount` table has 144 rows) and the 11th value of
  `GetMountInfoByID` is the collected flag, as on retail (2 collected on a level 17 character).
  `C_PetJournal.GetNumPets()` returns listed and owned, `5, 5`: under the journal's default filters
  it lists owned pets only, so the whole set is the `BattlePetSpecies` table (121 rows).
- **The encounter journal is empty.** `C_AddOns.IsAddOnLoadable("Blizzard_EncounterJournal")` is
  `true, ""` (Forever's game type is inside `classic` for TOC purposes), but `EJ_GetNumTiers()` is 0
  and `EJ_GetInstanceByIndex(1, false)` returns nothing, matching wago.tools serving no
  `JournalEncounter*` or `JournalInstance` table for build 1.60.1.70790. Boss loot and abilities are
  not in the client.
- **Load on demand works** for an addon's own folder: `C_AddOns.IsAddOnLoadOnDemand("ElvUI_Options")`
  is true and `C_AddOns.LoadAddOn("ElvUI_Options")` returned true (Team, 2026-11-09).

### CVars

- `C_CVar.SetCVar` requires a public, writable, non-secure CVar and refuses secret arguments (Docs
  `CVarDocumentation.lua`). Forever adds 138 CVars, among them `showKeyring`, `categorizeTrainerUI`,
  `showQuestLevel`, `showSwingTimer`, and `C_CVar.SetTempCVar` (W160).
- `loadDeprecationFallbacks` decides whether Blizzard's deprecation globals exist at all (Blz,
  above). On the probe character it is **true** (`GetCVarBool`, Team, 2026-11-08), so shims such
  as `CombatLogGetCurrentEventInfo` load there. It is a CVar a player can turn off, so never rely
  on a deprecated global.

### Addon messaging and chat

- `C_ChatInfo.SendAddonMessage` refuses secret arguments (`SecretArguments = "NotAllowed"`);
  `RegisterAddonMessagePrefix` is `AllowedWhenUntainted` (Docs `ChatInfoDocumentation.lua`).
  "Trial accounts can no longer send addon messages" (blue post 2026-10-08 in W160). In dungeons
  and raids chat is in messaging lockdown and chat text can be secret (WSV;
  `C_ChatInfo.InChatMessagingLockdown`, retail docs).

### SavedVariables and `AddOns.txt`

- SavedVariables were broken in early beta and "Fixed SavedVariables not loading" in build 70009
  (W160). Files live at `WTF/Account/<ACCOUNT>/SavedVariables/` (Team).
- A TOC line naming a missing file logs `Error loading <path>` in `Logs/FrameXML.log` and the rest
  loads (Disk: the installed Ryna 0.3.8 lacks `Locales/`, logged 2026-11-08 12:03).
- "An unknown event name aborts the whole file on Forever, so register safely" (Doud
  `Core.lua:845`): retail `RegisterEvent` errors on unknown events. Wrap optional registrations in
  `pcall`.
- **Unknown**: how Forever writes `AddOns.txt`, and whether bindings and macros are server-side
  (Team, `docs/ryna-agent-cores-mods.md`).

### Hardware events and protected actions

- `/who` (above); `SendChatMessage` `"EMOTE"` outside instances (blue post 2026-10-08 in W160).
  Auction buying and posting are `HasRestrictions` (Docs `AuctionHouseDocumentation.lua`): a click.
- Ryna's rule stands: an addon may react to the player's action, never manufacture one.

### `/dump`

- `/dump f()` prints nothing at all when `f` returns zero values (Team, 2026-11-08). That is how
  `C_Item.GetItemInfo` answers for an unloaded item (`MayReturnNothing`, Docs).

## Verified facts

"Game" means checked in game by the team; "Blz" and "Docs" mean Blizzard's Forever source and API
docs; "Addons" means seen in the installed Forever addons. All dated 2026-11-08 unless marked.

| Fact | Status | Source |
|---|---|---|
| Build 1.60.1.70862, product `wowf`, interface 16001 | on disk; Game | Disk `.build.info`, TOCs; WTOC |
| `WOW_PROJECT_ID` 18 = `WOW_PROJECT_CAMELOT`; `GetBuildInfo()` gives six values ending 16001, "", " " | Game | Team; W160 |
| TOC game type `camelot` (alias `forever`) | Addons; documented | ElvUI `ElvUI.toc:40`; WTOC |
| Engine is retail 12.1.5 plus Forever changes | documented | W160 |
| CLEU refused to addons | Game (2026-11-07) | Team 0.2.2; W120; Docs |
| Secret values on; `C_Secrets.HasSecretRestrictions()` true | Game; Addons | Team; Doud, Questie, BVP; W160 |
| `GetItemInfo` global nil; `C_Item.GetItemInfo` 18 returns, nothing when unloaded; Linen Cloth sells for 13c | Game; Docs | Team |
| Links start `\|cnIQ2:`; parse ids with `Hitem:(%d+)` | Game | Team |
| `C_Container.*`, `TooltipDataProcessor`, `C_TooltipInfo` work | Game; Addons | Team; BVP, Questie, ATT |
| `GetMerchantItemInfo` nil; `C_MerchantFrame.GetItemInfo` table, `price` copper per `stackCount`, `numAvailable` -1 unlimited | Game; Blz | Team; Blz `MerchantFrame.lua:290`; W120 |
| `GetNumTrainerServices()` counts with the window open (157); `GetTrainerServiceInfo` order name, kind, texture, level, subText, category; `C_Trainer` has 3 functions | Game; Blz | Team; Blz `Blizzard_TrainerUI.lua:469`; W160 |
| Quests: `AcceptQuest` on `QUEST_DETAIL`, `C_GossipInfo` | Blz; Addons | Blz `QuestFrame.lua:646`; AQ |
| `/who` needs a hardware event; `SendWho` takes filters | Game (2026-11-07); Blz | Team `3e50f74`; Blz `WhoList.lua:178` |
| `C_AuctionHouse` present, `QueryAuctionItems` and `GetAuctionItemInfo` nil; browse results as above; `ReplicateItems()` gives the whole house (192,300 rows) with owner fields nil | Game; Docs | Team; Docs |
| Mail via `GetInboxHeaderInfo`, 15 values | Game; Blz | Team; Blz `MailFrame.lua:340` |
| Talent tab globals nil; talents via `C_Traits` groups; `C_ClassTalents.GetActiveConfigID()` works; spec deprecation globals absent | Game; Blz | Team; Blz Camelot `Blizzard_ClassTalentsFrame.lua:95`, `Blizzard_DeprecatedSpecialization.toc` |
| `GetProfessions` exists, global `GetSkillLineInfo` nil, `C_SkillInfo.GetNumSkillLines()` 21 | Game; documented | Team; W160 |
| `GetLootRollItemInfo`, `RollOnLoot`, `GetLootSourceInfo` exist | Game | Team |
| `loadDeprecationFallbacks` true on the probe character | Game | Team |
| Creature GUID: server and spawn parts obfuscated, readable out of combat, stable over a minute, NPC id in field 6 | Game; documented | Team; W160 (blue post 2026-10-16) |
| Names are First plus Last-as-realm | Addons | ElvUI `Core.lua:70`; Pawn `Pawn.lua:588` |
| Unknown event name errors on register | Addons | Doud `Core.lua:845` |
| `/dump` prints nothing for zero values | Game | Team |
| Combat log `ZONE_CHANGE,<map id>,"<zone>",<difficulty>`: the first field is the map id (36 inside The Deadmines, as `ENCOUNTER_START` names it; 0 and 1 the continents; 2991 Zephras Isle; 369 Deeprun Tram); the difficulty is written 0 on zoning into a dungeon, 1 only after a `/reload` inside, and the old one is kept on the way out (`ZONE_CHANGE,0,"The Deadmines",1`), so the agent decides "in an instance" by map id (the client's `Map` table, `cores/wow/core/src/combat/maps.rs`) | Disk (2026-11-15) | every `ZONE_CHANGE` in the 40 combat logs in `_forever_/Logs`, Nov 6 to 14 |
| The client holds combat-log lines and writes them in bursts: a `/reload` writes its `COMBAT_LOG_VERSION` header at once, in front of held lines up to 300 s older (once in a boss fight), so neither the header's time nor a quiet file means a fight is over | Disk (2026-11-15) | the same 40 logs; `cores/wow/core/src/combat/segment.rs` (`FLUSH_GRACE_MS`) |
| Combat log `ZONE_CHANGE` names the zone as `GetRealZoneText` does (the dungeon inside one), and also a building walked into ("Lion's Pride Inn") and "UNKNOWN AREA" on a loading screen, neither with a `MAP_CHANGE` | Disk (2026-11-10) | 26 logs in `_forever_/Logs`; docs/ryna-agent-cores-mods.md 7.6 |
| `C_MountJournal` lists 136, collected flag is `GetMountInfoByID`'s 11th value; `C_PetJournal.GetNumPets()` is listed, owned (owned only by default); the encounter journal is loadable and empty (`EJ_GetNumTiers()` 0); `C_AddOns.LoadAddOn` loads a load-on-demand addon | Game (2026-11-09); Blz | Team; Blz TOCs on `forever` |
| Threat is readable in combat in a dungeon: `UnitDetailedThreatSituation` gives plain numbers for the player and for a party member against the target, a dungeon boss included; both `C_Secrets.ShouldUnitThreat...BeSecret("player","target")` are false | Game (2026-11-15) | Maddison's probes in The Deadmines; Docs `UnitDocumentation.lua`, `SecretPredicatesDocumentation.lua` |
| Threat through `targettarget` is secret; through `nameplateN` it is plain | Game (2026-11-15) | Maddison's probes in The Deadmines; Ryna_Meter `Threat.lua` |
| Secrets may be passed to `SetValue`, `SetMinMaxValues`, `SetText`, `SetFormattedText`, `SetVertexColor`, `SetAlpha`, `AbbreviateNumbers`; a live meter is drawn that way | Docs; Addons (2026-11-15) | Docs at Blz70938; Details `classes/class_damage.lua` |
| Out of combat, an aura's `name`, `icon`, `dispelName` and `auraInstanceID` are plain, `SetTexture(aura.icon)` draws, and `GetUnitAuraInstanceIDs(unit, "HARMFUL\|RAID")` leaves out what the player can't dispel (Weakened Soul: 0 of 1) | Game (2026-11-18) | Maddison's probes on a priest; the section "Auras and dispels" |
| `C_DamageMeter` answers an addon on Forever, live and after combat (sessions, a source's spells for group members); `AbbreviateNumbers` follows a `breakpointData` rule on secret numbers, the step below a thousand included; an addon's `SendChatMessage` to party posts inside a dungeon out of combat; `MenuUtil.CreateContextMenu` opens a menu for an addon | Game (2026-11-15) | Maddison in The Deadmines with Ryna_Meter (`Bars.lua`) |
| Inside a dungeon the target's `UnitGUID` and `UnitName` are **secret even out of combat** (a mob targeted in the Stormwind Stockade, `InCombatLockdown` false), so an addon can't read a creature's id or the instance copy from a unit there; `UnitXP`, `UnitXPMax` and `GetMoney` are plain there out of combat | Game (2026-11-20) | Maddison, `/ryna probe` (Chronicle 0.6.1). Not yet asked: the same in combat, a loot window's source GUID, and which restriction is in force |
| `IsInInstance()` is `true, "party"` inside a dungeon, `false, "none"` after walking out, and `true, "party"` again after a `/reload` inside. `GetInstanceInfo()` there is `"Stormwind Stockade", "party", 1, "Normal", 5, 0, false, 34, 5, nil, false`: the difficulty is 1 from zoning in (it is the combat log's `ZONE_CHANGE` that writes 0 until a reload), the eighth value is the map id, and nothing in it names the instance copy. Outside: `"Eastern Kingdoms", "none", 0, "", 0, 0, false, 0, 0, nil, false` | Game (2026-11-20) | the same readings |
| Registering `ENCOUNTER_START`, `ENCOUNTER_END`, `BOSS_KILL`, `CHAT_MSG_MONEY`, `CHAT_MSG_SYSTEM` and `PLAYER_REGEN_DISABLED` from an addon's own frame raises no error. `InCombatLockdown()` is still false inside `PLAYER_REGEN_DISABLED`'s handler | Game (2026-11-20) | the same readings |
| In combat inside a dungeon (Blackfathom Deeps, `InCombatLockdown` true): `UnitXP`, `UnitXPMax` and `GetMoney` are plain; the target's GUID and name are secret, as they are out of combat there | Game (2026-11-20) | Maddison, `/ryna probe` typed mid-fight |
| A loot window's source is **not** secret inside a dungeon: `issecretvalue((GetLootSourceInfo(1)))` is `false` with a boss's corpse open (Ghamoo-ra), so a looted corpse gives its GUID where a unit can't: `GetLootSourceInfo(1)` on a Blackfathom Myrmidon's corpse answered `"Creature-0-8452-48-8801871-4807-1C5DBBE380", 67` (map 48, instance copy 8801871, creature 4807; then the quantity) | Game (2026-11-20) | Maddison's two `/dump`s |
| `CHAT_MSG_MONEY` reaches an addon plain; in a group it is the share line, "Your share of the loot is 42 Copper." (the client's `LOOT_MONEY_SPLIT`) | Game (2026-11-20) | `/ryna probe watch`, six lines in Blackfathom Deeps; the solo line (`YOU_LOOT_MONEY`) not yet seen |
| `BOSS_KILL` did not fire for a dungeon boss whose `ENCOUNTER_START` and `ENCOUNTER_END` (success 1) both did: Ghamoo-ra, encounter 2916, Blackfathom Deeps (map 48) | Game (2026-11-20) | `/ryna probe watch`, registered for all three |
| The fifth field of a looted corpse's GUID is **not** one value for a whole instance copy: a run that treated a different value as a new copy started over at loot windows mid-clear (Chronicle 0.7.0, the kill count going back to zero, each time on looting). Thirteen runs split that way in one Blackfathom Deeps visit carried thirteen different values (12688983, 14277873, 4359379, …), so it changes about as often as the corpse does and says nothing about the copy | Game and the API's stored events (2026-11-21) | Maddison, in a dungeon with Chronicle 0.7.0; `/ryna probe watch` prints each looted corpse's GUID from the next Chronicle release, to see how it varies |
| The loot history is by encounter and plain: after Saltspine (encounter 3480, Excavation Site: Wetlands, map 2998) `C_LootHistory.GetSortedInfoForDrop(3480, 9193473)` gave the item's link, `winner = { playerName, playerClass = "WARLOCK", playerGUID, roll = 23, state = 0, isSelf = false, isWinner = true }` and five `rollInfos`; `CHAT_MSG_LOOT` carried each choice ("… has selected Greed for: [item]"), the winning roll ("Need Roll - 23 for [item] by …") and "… won: [item]", all plain, names included | Game (2026-11-23) | Maddison, `/ryna probe watch` and `/ryna probe loot` (Chronicle 0.7.3). Not yet seen: the inside of a `rollInfos` entry (whether a losing roll carries its number), a rolled drop from trash, the history after a `/reload` |
| `C_AddOnProfiler` exists on Forever and answers an addon: `GetAddOnMetric(name, Enum.AddOnProfilerMetric.CountTimeOver50Ms)` per addon and `GetApplicationMetric` for the client (a session in a dungeon: AllTheThings 2, ElvUI_Libraries 8, HealBot 3, the client 28, and no Ryna addon above 0). It is the way to tell whether an addon is stalling frames | Game (2026-11-21) | Maddison's macro, asked because loot windows felt slow with Chronicle 0.7.1; the slowness was the server |
| A dungeon boss sends an addon `ENCOUNTER_START` and `ENCOUNTER_END`: `ENCOUNTER_START 2759 "Dextren Ward" 1 5` (encounter id, name, difficulty, group size) and `ENCOUNTER_END 2759 "Dextren Ward" 1 5 1` then a table (the same four, success 1, and a sixth value that prints as a table, not yet read) | Game (2026-11-20) | Maddison's R3 macro, from her chat frame; whether registering raised a dialog, `BOSS_KILL`, and the Kills tab aren't reported yet |
| The character sheet's reads (Chronicle 0.6.0, `Sheet.lua`; api `docs/wow.md` 19): out of combat `UnitStat`, `GetCritChance` and `UnitHealthMax` are not secret; `C_Traits.GetNodeInfo(configID, nodeID)` gives `posX`, `posY`, `activeRank`, `ranksPurchased`, `maxRanks`, `isVisible` and `groupIDs` (a node of the class tree: `9680 5730 0 0 1 true 11616`); the spellbook lists every rank known, each its own slot, with "Rank N" in both `GetSpellBookItemInfo().subName` and `C_Spell.GetSpellSubtext` (Lesser Heal 2050, 2052, 2053 in slots 18 to 20) and `GetSpellBookItemLevelLearned` (Shoot: empty subName, level 1). Not yet checked: the inactive spec's nodes through `GetCombatConfigIDForSpecGroup`, `C_CurrencyInfo.GetCurrencyListInfo`, `GetPVPLifetimeStats`, the class trainer's list | Game (2026-11-19); Blz 70959 | Maddison's probes on a level 23 priest; Blz `Blizzard_PlayerSpells/Camelot`, `Blizzard_UIPanels_Game/Camelot/PaperDollFrame.lua`, `Blizzard_TokenUI/Camelot` |

## How to probe

- **One value**: `/dump expr`. Nothing printed means zero values; `nil` printed means one nil.
- **Nil or nothing, for sure**: `/run print(select("#", C_Item.GetItemInfo(2589)))` prints 0 for
  nothing and the number of returns otherwise.
- **Does a function exist**: `/dump GetTalentTabInfo ~= nil` (a bare `/dump GetTalentTabInfo`
  prints nothing when it is nil).
- **Enumerate a namespace**: `/dump C_Trainer`; for a long one,
  `/run for k in pairs(C_AuctionHouse) do print(k) end`.
- **Secret or not**: `/dump issecretvalue(UnitGUID("target"))`, in and out of combat.
- **Blizzard's own call** is the cheapest evidence before a probe: clone the `forever` branch
  (`git clone --depth 1 --branch forever --filter=blob:none --sparse https://github.com/Gethe/wow-ui-source.git`)
  and grep it. Its head matches the installed build (1.60.1 70862 on 2026-11-07).
- After a probe, put the answer in this page's table with the date.

### Still owed

The first fourteen were answered on 2026-11-08 and the journal probes on 2026-11-09, all folded in above. Next, one line each:

1. Talent points per tree (what Chronicle now reads; zeros before the first point), then `/dump C_SpecializationInfo.GetSpecializationInfo(1)`:
   `/run local c=C_ClassTalents.GetActiveConfigID() local t=C_Traits.GetConfigInfo(c).treeIDs[1] for _,d in ipairs(C_Traits.GetGroupDisplayInfoByTreeID(t)) do local g=C_Traits.GetGroupCurrencyInfo(c,{d.groupID})[1] local i=g and g.currencyInfos and g.currencyInfos[1] print(d.displayName, i and i.spent, i and i.spentInTree) end`
2. `/dump issecretvalue(UnitGUID("target")), issecretvalue(UnitName("target"))` (as a macro, in combat with a mob)
3. `/dump UnitGUID("target")` on the same mob after a `/reload` (does the obfuscated spawn part survive a reload?)
4. `/dump GetProfessions()` then `/dump C_SkillInfo.GetSkillLineInfo(1)`
5. `/run C_AuctionHouse.ReplicateItems()` again within 15 minutes of the last one, then `/dump C_AuctionHouse.GetNumReplicateItems()` (is there a throttle?)
6. `/dump GetGuildRosterInfo(1)` (needs a character in a guild)
7. `/dump C_MerchantFrame.GetItemInfo(1).isPurchasable` at a vendor with enough money for item 1 (does it mean "can afford"?)
8. With Prices on and `/ryna debug on`, open the auction house: a "prices: auction snapshot: ..." line within a minute means `REPLICATE_ITEM_LIST_UPDATE` fired; note any stutter
9. `/run local f=CreateFrame("Frame") f:RegisterEvent("AUCTION_HOUSE_BROWSE_RESULTS_UPDATED") f:SetScript("OnEvent",function(_,e) local r=C_AuctionHouse.GetBrowseResults() print(e,#r,r[1] and r[1].minPrice,r[1] and r[1].totalQuantity) end)`, then search for a stackable item whose cheapest listing you can see (is `minPrice` per unit?)
10. `/dump C_DamageMeter ~= nil, C_DamageMeter and C_DamageMeter.IsDamageMeterAvailable()`
11. After a fight, out of combat: `/dump C_DamageMeter.GetAvailableCombatSessions()`
12. Then: `/run local s=C_DamageMeter.GetCombatSessionFromType(1,0) print(#s.combatSources, s.totalAmount, s.durationSeconds) local c=s.combatSources[1] print(c and c.name, c and c.totalAmount, c and c.amountPerSecond)` (the current session's damage done: readable out of combat?)
13. Ryna's window (2026-11-09) uses these, each with a fallback, none yet seen on Forever:
    `/dump C_Item.GetItemIconByID ~= nil, C_CurrencyInfo and C_CurrencyInfo.GetCoinTextureString ~= nil, GetItemIcon ~= nil`;
    `/dump C_XMLUtil.GetTemplateInfo("WowStyle1DropdownTemplate") ~= nil, C_XMLUtil.GetTemplateInfo("SearchBoxTemplate") ~= nil`
    (the character picker falls back to a list of its own, the search box to a plain one);
    `/dump RAID_CLASS_COLORS.MAGE, ITEM_QUALITY_COLORS[2], FACTION_BAR_COLORS and FACTION_BAR_COLORS[5], FACTION_STANDING_LABEL5, LOCALIZED_CLASS_NAMES_MALE and LOCALIZED_CLASS_NAMES_MALE.MAGE, HEADSLOT, RANGEDSLOT`
14. On a raid boss, in combat: `/dump C_Secrets.ShouldUnitThreatValuesBeSecret("player","target"), UnitDetailedThreatSituation("player","target")` (do the values go secret in a raid?)
15. In combat with a mob: `/dump C_RestrictedActions.GetAddOnRestrictionState(0), C_RestrictedActions.GetAddOnRestrictionState(1), C_RestrictedActions.GetAddOnRestrictionState(4)` (Combat, Encounter, Map), and again inside a dungeon
16. In combat: `/run local s=C_DamageMeter.GetCombatSessionFromType(1,0) local c=s.combatSources[1] print(#s.combatSources, c and issecretvalue(c.totalAmount), c and issecretvalue(c.name))` (the live session answers, and which fields are secret)
14. The quest log's header rows as Chronicle reads them (Blizzard's source, not yet seen in game), with a few
    quests in the log: `/run for i=1,C_QuestLog.GetNumQuestLogEntries() do local q=C_QuestLog.GetInfo(i) if q then print(i, q.isHeader, q.title, q.questID) end end`
    (each header row should print `true` and the heading the quest log shows, its quests after it with `false`);
    then `/dump issecretvalue(C_QuestLog.GetInfo(1).title)` (expect `false`)
15. Gold at logout (what `GetMoney()` answers during `PLAYER_LOGOUT` is unknown; saved records had 0): log
    character A out to the character list, log in character B, and open Ryna's window, Characters: A's gold
    should be what A carried, not 0
16. Auras in combat, with something on you that your class can cure (a magic effect or a disease on a
    priest), one macro each:
    `/run local f=C_UnitAuras.GetUnitAuraInstanceIDs local t,a=f("player","HARMFUL|RAID"),f("player","HARMFUL") print("RynaP1 cure",issecretvalue(t) or #t,"all",issecretvalue(a) or #a,"combat",InCombatLockdown())`
    (is the count plain? with a curse or poison on a priest, is "cure" 0?);
    `/run print("RynaP2",pcall(function() local a=C_UnitAuras.GetUnitAuras("player","HARMFUL",1)[1] local s=issecretvalue return a and s(a.name),a and s(a.icon),a and s(a.dispelName),a and s(a.auraInstanceID) end))`
    (which of name, icon, kind and instance id are secret);
    `/run local a=C_UnitAuras.GetUnitAuras("player","HARMFUL",1)[1] print("RynaP3",a~=nil) local f=CreateFrame("Frame",nil,UIParent) f:SetSize(48,48) f:SetPoint("CENTER") local t=f:CreateTexture() t:SetAllPoints() t:SetTexture(a.icon) print("RynaP3 ok")`
    (does a secret icon draw?)
17. Out of combat, with nothing to cure: cast a cure on yourself. Refused or cast? Mana, global cooldown?

**For dungeon runs** (docs/ryna-agent-cores-mods.md 7.7; asked 2026-11-20). All inside a dungeon, one line each:

From the Chronicle release after 0.6.0 these are one command (`Probe.lua`, 2026-11-20): `/ryna probe watch` once
on zoning in (R3's events and whether registering them is refused, R5's coin line, a reset's system line, and a
reading at the start of the next three fights, which is R4 and probe 2), `/ryna probe` with a mob targeted
wherever R1 and R2 ask for a dump, and `/ryna probe show` for every line in a box to copy. The watch survives
the `/reload`. The macros below ask the same things on a client without it.

- **R1.** (Answered 2026-11-20, and it is a no: the target's GUID is secret inside a dungeon even out of combat, "Verified facts". A loot window's source is plain there, the same day; and its GUID has the usual shape with the copy in the fifth field; left of it: is the copy the same after walking out and back in, and different after a reset.) Out of combat, with a mob targeted: `/dump UnitGUID("target"), GetInstanceInfo()` (the GUID's fifth
  field is the instance copy; `GetInstanceInfo` should give the name, `"party"`, the difficulty and, eighth, the
  map id, 36 in The Deadmines). Walk out and back in without a reset and dump again: is the fifth field the
  same? Reset the dungeon, go in, dump again: is it different? Probe 2 above (is the GUID secret in combat)
  matters here too.
- **R2.** (Answered 2026-11-20 but for the ghost: inside, outside on foot and after a `/reload`, "Verified facts".) `/dump IsInInstance()` inside; again as a ghost outside after a death; again inside after a `/reload`,
  with `/dump GetInstanceInfo()` (the difficulty is expected to change from 0 to 1 on the reload).
- **R3.** (Answered 2026-11-20 but for the Kills tab: both encounter events arrive in a dungeon, registering raises no error, `BOSS_KILL` doesn't fire, "Verified facts".) Before a boss:
  `/run local f=CreateFrame("Frame") for _,e in ipairs({"ENCOUNTER_START","ENCOUNTER_END","BOSS_KILL"}) do f:RegisterEvent(e) end f:SetScript("OnEvent",function(_,e,...) print("RynaR3",e,...) end)`
  (does registering raise the "blocked" dialog; what prints on the pull and on the kill?). After the boss dies,
  open Ryna's window, Kills: is the boss there (the question 7.4 still owes)?
- **R4.** (Answered 2026-11-20: all three plain in combat, "Verified facts".) In combat, as a macro: `/dump issecretvalue(UnitXP("player")), issecretvalue(UnitXPMax("player")), issecretvalue(GetMoney())`
  (expect three `false`).
- **R5.** (Answered 2026-11-20 for a group: plain, the share line, "Verified facts". The solo line is still to see.) Before looting coin from a corpse, in a group if possible:
  `/run local f=CreateFrame("Frame") f:RegisterEvent("CHAT_MSG_MONEY") f:SetScript("OnEvent",function(_,_,m) print("RynaR5",issecretvalue(m) or m) end)`
  (the line as an addon receives it: your own share, and plain?).
- **R6.** Does Forever cap instances per hour, as Classic did at five? A blue post or a refusal seen in game
  answers it; if neither, reset and re-enter a dungeon six times inside an hour and note what the sixth says.

## Links

The Sources table has the main ones. Beyond it:

- Retail UI source (`live`, 12.1.5 70783 on 2026-11-04): https://github.com/Gethe/wow-ui-source/tree/live ;
  Forever's diff against it is linked from W160 (`compare/12.1.5..forever` on Gethe).
- Wiki pages cited inline: https://warcraft.wiki.gg/wiki/World_of_Warcraft:_Forever ,
  https://warcraft.wiki.gg/wiki/WOW_PROJECT_ID , https://warcraft.wiki.gg/wiki/GetBuildInfo ,
  https://warcraft.wiki.gg/wiki/API_C_Item.GetItemInfo
- This repo's probes: `mods/wow/README.md`, `cores/wow/addon/README.md`,
  `docs/ryna-agent-cores-mods.md`, `mods/wow/automation/addon/Ryna_Automation/Trainer.lua`.
