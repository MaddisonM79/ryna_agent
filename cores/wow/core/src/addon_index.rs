//! The addon index (docs/ryna-agent-cores-mods.md 6.2): the one signed file that says which
//! version of each addon is the newest, and the rule for what the app takes from it.
//!
//! The app carries a copy of every addon it was built with. The index (`addons.json` on the latest
//! release of `Ryna-Mods/ryna`) can offer a newer one of a folder this build already knows, and
//! (Maddison, 2026-11-26) **a mod this build has never heard of**, when its entry says what the mod
//! is (`mod`: its id, name, summary and whether it acts in the game). Such a mod is fetched like an
//! update, listed as new, and is always off until the person turns it on. A folder the index names
//! without saying what it is stays ignored. This module is the format and the choice, with no file
//! or network in it:
//! [`parse`] reads an index whose signature the caller has already checked ([`crate::addon_store`]),
//! and [`choose`] picks, for each folder, the carried copy or the index's.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::addon_install::compare_versions;

/// The index's file name; its signature is `<this>.sig`.
pub const INDEX_FILE: &str = "addons.json";
/// Where the index is: the latest release of the bundle repository.
pub const INDEX_URL: &str = "https://github.com/Ryna-Mods/ryna/releases/latest/download";
/// Points the app at a folder or another https URL holding the index and the zips (development).
pub const INDEX_ENV: &str = "RYNA_ADDON_INDEX";
/// The only place an entry's zip may be.
pub const ALLOWED_PREFIX: &str = "https://github.com/Ryna-Mods/";
/// The largest zip an entry may name.
pub const MAX_ZIP_BYTES: u64 = 128 * 1024 * 1024;
/// The format this build reads.
pub const SCHEMA: u32 = 1;

/// The folder name a mod the index introduces must start with: what the core's manifest says the
/// app writes (`Interface/AddOns/Ryna_*/`).
pub const LEARNED_PREFIX: &str = "Ryna_";

/// What a mod is, for an app that doesn't carry it: enough to list it in Mods and say what it does.
/// Signed with the index, like everything in it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModInfo {
    /// Short, lowercase: `guild`.
    pub id: String,
    pub name: String,
    /// What it does, as the Mods list says it.
    pub summary: String,
    /// It does something in the game for the player: said before it can be turned on.
    #[serde(default)]
    pub acts_in_game: bool,
}

/// One addon in the index.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    /// The addon's folder (`Ryna_Core`, `Ryna_Meter`).
    pub folder: String,
    pub version: String,
    /// The core: the API level it offers. A mod: the level it needs.
    pub api: u32,
    /// The core only: the lowest level it still serves.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_min: Option<u32>,
    /// The oldest app that can read what this addon writes; absent: any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_app: Option<String>,
    pub url: String,
    /// Lowercase hex.
    pub sha256: String,
    pub size: u64,
    /// A mod's own description, which lets an app that doesn't carry the folder offer it. Absent
    /// for the core, and for a mod only apps that carry it should install.
    #[serde(default, rename = "mod", skip_serializing_if = "Option::is_none")]
    pub mod_info: Option<Box<ModInfo>>,
}

/// An index that was read: the entries that are well formed, and why each other one was dropped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Index {
    pub serial: u64,
    pub entries: Vec<Entry>,
    pub dropped: Vec<String>,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum IndexError {
    #[error("the addon index isn't readable ({0})")]
    Format(String),
    #[error("the addon index is format {0}, which this version of Ryna doesn't read")]
    Schema(u32),
}

#[derive(Deserialize)]
struct Raw {
    schema: u32,
    serial: u64,
    #[serde(default)]
    addons: Vec<serde_json::Value>,
}

fn is_folder_name(s: &str) -> bool {
    !s.is_empty() && s.len() <= 64 && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

fn is_version(s: &str) -> bool {
    let parts: Vec<&str> = s.split('.').collect();
    (2..=4).contains(&parts.len())
        && parts
            .iter()
            .all(|p| !p.is_empty() && p.len() <= 9 && p.bytes().all(|b| b.is_ascii_digit()))
}

/// Text an app may show: something in it, no longer than `max` characters, and no control or
/// format character (a bidi override, a zero-width space).
fn is_plain_text(s: &str, max: usize) -> bool {
    !s.trim().is_empty()
        && s.chars().count() <= max
        && !s.chars().any(|c| {
            c.is_control() || matches!(c, '\u{200B}'..='\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}' | '\u{FEFF}')
        })
}

fn is_mod_id(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 32
        && s.as_bytes()[0].is_ascii_lowercase()
        && s.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// Why `e` can't be used, or None when it is well formed.
fn entry_fault(e: &Entry) -> Option<String> {
    if let Some(m) = &e.mod_info {
        if !is_mod_id(&m.id) || !is_plain_text(&m.name, 60) || !is_plain_text(&m.summary, 1000) {
            return Some("what it says of its mod isn't an id, a name and a summary".into());
        }
        if !e.folder.starts_with(LEARNED_PREFIX) || e.api_min.is_some() {
            return Some(format!(
                "a mod's folder starts with {LEARNED_PREFIX}, and only the core has a lowest level"
            ));
        }
    }
    if !is_folder_name(&e.folder) {
        return Some("its folder isn't a plain name".into());
    }
    if !is_version(&e.version) || e.min_app.as_deref().is_some_and(|v| !is_version(v)) {
        return Some("a version isn't numbers and dots".into());
    }
    if !e.url.starts_with(ALLOWED_PREFIX)
        || !e.url.ends_with(".zip")
        || e.url.contains("..")
        || e.url.contains(['?', '#', '\\', ' '])
    {
        return Some(format!("its zip isn't under {ALLOWED_PREFIX}"));
    }
    if e.sha256.len() != 64
        || !e
            .sha256
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Some("its sha256 isn't 64 lowercase hex characters".into());
    }
    if e.size == 0 || e.size > MAX_ZIP_BYTES {
        return Some("its size is nothing or too large".into());
    }
    if e.api == 0 || e.api_min.is_some_and(|min| min == 0 || min > e.api) {
        return Some("its API level isn't a level".into());
    }
    None
}

/// Read an index. Call only with bytes whose signature has been checked. An entry that isn't well
/// formed is dropped and the rest are kept; a second entry for a folder is dropped.
pub fn parse(bytes: &[u8]) -> Result<Index, IndexError> {
    let raw: Raw = serde_json::from_slice(bytes).map_err(|e| IndexError::Format(e.to_string()))?;
    if raw.schema != SCHEMA {
        return Err(IndexError::Schema(raw.schema));
    }
    let mut entries: Vec<Entry> = Vec::new();
    let mut dropped = Vec::new();
    for value in raw.addons {
        let name = value
            .get("folder")
            .and_then(|f| f.as_str())
            .unwrap_or("an entry")
            .to_string();
        match serde_json::from_value::<Entry>(value) {
            Err(e) => dropped.push(format!("{name}: {e}")),
            Ok(e) => match entry_fault(&e) {
                Some(why) => dropped.push(format!("{name}: {why}")),
                None if entries.iter().any(|have| have.folder == e.folder) => {
                    dropped.push(format!("{name}: listed twice"))
                }
                None => entries.push(e),
            },
        }
    }
    Ok(Index {
        serial: raw.serial,
        entries,
        dropped,
    })
}

/// A mod's copy that the app carries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Carried {
    pub version: String,
    /// The API level it needs.
    pub api: u32,
}

/// Everything this build of the app carries: the core's addon and each addon mod's folders.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Known {
    pub core_folder: String,
    pub core_version: String,
    /// The level the carried core offers, and the lowest it serves.
    pub core_api: u32,
    pub core_api_min: u32,
    /// By folder.
    pub mods: BTreeMap<String, Carried>,
}

/// Which copy of a folder to install from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pick {
    /// The copy the app carries.
    Carried,
    /// The index's, which is newer.
    Fetched(Entry),
}

fn fits(mod_api: u32, core_api: u32, core_api_min: u32) -> bool {
    core_api_min <= mod_api && mod_api <= core_api
}

/// Whether the index's `e` may replace a carried copy at `carried`: newer, and this app is new
/// enough to read what it writes.
fn candidate(e: &Entry, carried: &str, app_version: &str) -> bool {
    compare_versions(&e.version, carried).is_gt()
        && e.min_app
            .as_deref()
            .is_none_or(|min| compare_versions(app_version, min).is_ge())
}

/// Whether the index's `e` is a mod this app doesn't carry and may take: it says what the mod is,
/// it fits the core being installed, and this app is new enough for it.
fn new_mod(e: &Entry, known: &Known, api: u32, api_min: u32, app_version: &str) -> bool {
    e.mod_info.is_some()
        && e.folder != known.core_folder
        && !known.mods.contains_key(&e.folder)
        && fits(e.api, api, api_min)
        && e.min_app
            .as_deref()
            .is_none_or(|min| compare_versions(app_version, min).is_ge())
}

/// For each folder this app knows, the carried copy or the index's (docs 6.2, "What it picks").
/// Every known folder is in the answer. A folder the index names that this app doesn't know is in
/// it only when the entry says what mod it is and it fits the core picked ([`new_mod`]): always the
/// index's copy, there being no other.
pub fn choose(index: &Index, known: &Known, app_version: &str) -> BTreeMap<String, Pick> {
    let entry = |folder: &str| index.entries.iter().find(|e| e.folder == folder);
    // A mod's best copy for a core at (api, api_min), or None when no copy of it fits that core.
    let mod_pick = |folder: &str, carried: &Carried, api: u32, api_min: u32| -> Option<Pick> {
        match entry(folder) {
            Some(e) if candidate(e, &carried.version, app_version) && fits(e.api, api, api_min) => {
                Some(Pick::Fetched(e.clone()))
            }
            _ => fits(carried.api, api, api_min).then_some(Pick::Carried),
        }
    };
    let newer_core = entry(&known.core_folder)
        .filter(|e| candidate(e, &known.core_version, app_version))
        .and_then(|e| e.api_min.map(|min| (e, min)));
    if let Some((core, api_min)) = newer_core {
        let mods: Option<Vec<(String, Pick)>> = known
            .mods
            .iter()
            .map(|(folder, carried)| {
                mod_pick(folder, carried, core.api, api_min).map(|p| (folder.clone(), p))
            })
            .collect();
        // Taken only when every mod has a copy that fits it.
        if let Some(mods) = mods {
            let mut picks: BTreeMap<String, Pick> = mods.into_iter().collect();
            picks.insert(known.core_folder.clone(), Pick::Fetched(core.clone()));
            for e in &index.entries {
                if new_mod(e, known, core.api, api_min, app_version) {
                    picks.insert(e.folder.clone(), Pick::Fetched(e.clone()));
                }
            }
            return picks;
        }
    }
    let mut picks: BTreeMap<String, Pick> = known
        .mods
        .iter()
        .map(|(folder, carried)| {
            let pick = mod_pick(folder, carried, known.core_api, known.core_api_min)
                // The carried mods fit the carried core by construction (a test holds them to it).
                .unwrap_or(Pick::Carried);
            (folder.clone(), pick)
        })
        .collect();
    picks.insert(known.core_folder.clone(), Pick::Carried);
    for e in &index.entries {
        if new_mod(e, known, known.core_api, known.core_api_min, app_version) {
            picks.insert(e.folder.clone(), Pick::Fetched(e.clone()));
        }
    }
    picks
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(folder: &str, version: &str, api: u32) -> Entry {
        Entry {
            folder: folder.into(),
            version: version.into(),
            api,
            api_min: (folder == "Ryna_Core").then_some(api),
            min_app: None,
            url: format!("{ALLOWED_PREFIX}x/releases/download/v{version}/{folder}-{version}.zip"),
            sha256: "ab".repeat(32),
            size: 10,
            mod_info: None,
        }
    }

    fn known() -> Known {
        Known {
            core_folder: "Ryna_Core".into(),
            core_version: "0.5.0".into(),
            core_api: 2,
            core_api_min: 2,
            mods: [("Ryna_Meter", "0.5.0"), ("Ryna_Prices", "0.5.0")]
                .into_iter()
                .map(|(f, v)| {
                    (
                        f.to_string(),
                        Carried {
                            version: v.into(),
                            api: 2,
                        },
                    )
                })
                .collect(),
        }
    }

    fn index(entries: Vec<Entry>) -> Index {
        Index {
            serial: 1,
            entries,
            dropped: vec![],
        }
    }

    fn fetched(picks: &BTreeMap<String, Pick>) -> Vec<(String, String)> {
        picks
            .iter()
            .filter_map(|(f, p)| match p {
                Pick::Fetched(e) => Some((f.clone(), e.version.clone())),
                Pick::Carried => None,
            })
            .collect()
    }

    #[test]
    fn parse_keeps_good_entries_and_says_why_it_dropped_the_rest() {
        let good = serde_json::to_value(entry("Ryna_Meter", "1.0.1", 2)).unwrap();
        let mut elsewhere = good.clone();
        elsewhere["folder"] = "Ryna_Prices".into();
        elsewhere["url"] = "https://example.com/Ryna_Prices.zip".into();
        let mut bad_hash = good.clone();
        bad_hash["folder"] = "Ryna_Fights".into();
        bad_hash["sha256"] = "XYZ".into();
        let mut escaping = good.clone();
        escaping["folder"] = "../Ryna".into();
        let json = serde_json::json!({
            "schema": 1, "serial": 7, "generated": "2026-11-17T00:00:00Z",
            "addons": [good, elsewhere, bad_hash, escaping, good, { "folder": "Ryna_Chronicle" }],
        });
        let i = parse(json.to_string().as_bytes()).unwrap();
        assert_eq!(i.serial, 7);
        assert_eq!(i.entries, vec![entry("Ryna_Meter", "1.0.1", 2)]);
        assert_eq!(i.dropped.len(), 5, "{:?}", i.dropped);
        assert!(i.dropped[0].starts_with("Ryna_Prices: its zip isn't under"));
        assert!(i.dropped[3].ends_with("listed twice"));
    }

    #[test]
    fn parse_refuses_another_format_and_what_isnt_json() {
        assert_eq!(
            parse(br#"{"schema":2,"serial":1,"addons":[]}"#),
            Err(IndexError::Schema(2))
        );
        assert!(matches!(parse(b"<html>"), Err(IndexError::Format(_))));
    }

    #[test]
    fn a_newer_mod_is_taken_and_an_older_or_equal_one_is_not() {
        let i = index(vec![
            entry("Ryna_Meter", "1.0.1", 2),
            entry("Ryna_Prices", "0.5.0", 2),
            entry("Ryna_Unknown", "9.0.0", 2),
        ]);
        let picks = choose(&i, &known(), "0.5.0");
        assert_eq!(fetched(&picks), [("Ryna_Meter".into(), "1.0.1".into())]);
        assert_eq!(picks.len(), 3, "every known folder, and no unknown one");
        assert_eq!(picks["Ryna_Core"], Pick::Carried);
    }

    #[test]
    fn an_addon_that_needs_a_newer_app_waits_for_it() {
        let mut e = entry("Ryna_Meter", "1.0.1", 2);
        e.min_app = Some("0.6.0".into());
        let i = index(vec![e]);
        assert!(fetched(&choose(&i, &known(), "0.5.9")).is_empty());
        assert_eq!(fetched(&choose(&i, &known(), "0.6.0")).len(), 1);
    }

    #[test]
    fn a_mod_that_needs_a_higher_level_waits_for_the_core_that_has_it() {
        // Meter 2.0.0 needs level 3; alone it isn't taken, with a core at 3 that still serves 2 it is.
        let meter = entry("Ryna_Meter", "2.0.0", 3);
        assert!(fetched(&choose(&index(vec![meter.clone()]), &known(), "0.5.0")).is_empty());
        let mut core = entry("Ryna_Core", "0.6.0", 3);
        core.api_min = Some(2);
        let picks = choose(&index(vec![meter, core]), &known(), "0.5.0");
        assert_eq!(
            fetched(&picks),
            [
                ("Ryna_Core".into(), "0.6.0".into()),
                ("Ryna_Meter".into(), "2.0.0".into())
            ]
        );
        assert_eq!(picks["Ryna_Prices"], Pick::Carried, "level 2 still fits");
    }

    #[test]
    fn a_core_that_would_strand_a_mod_is_not_taken() {
        // Core 1.0.0 serves only level 3. Meter has a level 3 release; Prices doesn't yet.
        let core = entry("Ryna_Core", "1.0.0", 3);
        let meter = entry("Ryna_Meter", "2.0.0", 3);
        let picks = choose(&index(vec![core.clone(), meter.clone()]), &known(), "0.5.0");
        assert!(fetched(&picks).is_empty(), "the carried set stays whole");
        // Once Prices has one too, all three move together.
        let prices = entry("Ryna_Prices", "2.0.0", 3);
        let picks = choose(&index(vec![core, meter, prices]), &known(), "0.5.0");
        assert_eq!(fetched(&picks).len(), 3);
    }

    #[test]
    fn a_core_entry_without_its_lowest_level_is_not_taken() {
        let mut core = entry("Ryna_Core", "0.6.0", 2);
        core.api_min = None;
        assert!(fetched(&choose(&index(vec![core]), &known(), "0.5.0")).is_empty());
    }
    fn guild(version: &str, api: u32) -> Entry {
        Entry {
            mod_info: Some(Box::new(ModInfo {
                id: "guild".into(),
                name: "Guild".into(),
                summary: "Your guild's roster, and what changed.".into(),
                acts_in_game: false,
            })),
            api_min: None,
            ..entry("Ryna_Guild", version, api)
        }
    }

    #[test]
    fn a_mod_this_app_never_carried_is_taken_when_the_index_says_what_it_is() {
        let picks = choose(&index(vec![guild("0.1.0", 2)]), &known(), "0.5.0");
        assert_eq!(picks["Ryna_Guild"], Pick::Fetched(guild("0.1.0", 2)));
        assert_eq!(picks.len(), 4, "the three it carries, and the new one");
        // A folder the index only names, without saying what it is, stays ignored.
        let bare = entry("Ryna_Bare", "1.0.0", 2);
        assert!(!choose(&index(vec![bare]), &known(), "0.5.0").contains_key("Ryna_Bare"));
        // One that needs a level this core doesn't serve, or a newer app, waits.
        assert!(
            !choose(&index(vec![guild("0.1.0", 3)]), &known(), "0.5.0").contains_key("Ryna_Guild")
        );
        let mut newer = guild("0.1.0", 2);
        newer.min_app = Some("0.6.0".into());
        assert!(!choose(&index(vec![newer.clone()]), &known(), "0.5.0").contains_key("Ryna_Guild"));
        assert!(choose(&index(vec![newer]), &known(), "0.6.0").contains_key("Ryna_Guild"));
    }

    #[test]
    fn a_new_mod_follows_the_core_that_is_picked() {
        // Core 1.0.0 serves only level 3, and both carried mods have a level 3 release.
        let set = vec![
            entry("Ryna_Core", "1.0.0", 3),
            entry("Ryna_Meter", "2.0.0", 3),
            entry("Ryna_Prices", "2.0.0", 3),
        ];
        let with = |g: Entry| choose(&index([set.clone(), vec![g]].concat()), &known(), "0.5.0");
        assert!(
            with(guild("0.2.0", 3)).contains_key("Ryna_Guild"),
            "it fits the new core"
        );
        let old = with(guild("0.1.0", 2));
        assert_eq!(fetched(&old).len(), 3, "the core still moves");
        assert!(
            !old.contains_key("Ryna_Guild"),
            "a level 2 mod is left out, not installed broken"
        );
    }

    #[test]
    fn what_an_entry_says_of_its_mod_is_checked() {
        let json = |m: &str, folder: &str| {
            format!(
                r#"{{"schema":1,"serial":7,"addons":[{{"folder":"{folder}","version":"0.1.0","api":2,
                "url":"{ALLOWED_PREFIX}guild/releases/download/v0.1.0/x.zip","sha256":"{}","size":10,"mod":{m}}}]}}"#,
                "ab".repeat(32)
            )
        };
        let good = r#"{"id":"guild","name":"Guild","summary":"The roster.","actsInGame":true}"#;
        let read = parse(json(good, "Ryna_Guild").as_bytes()).unwrap();
        assert_eq!(read.entries[0].mod_info.as_ref().unwrap().id, "guild");
        assert!(read.entries[0].mod_info.as_ref().unwrap().acts_in_game);
        for (bad, folder) in [
            (
                r#"{"id":"Guild","name":"Guild","summary":"x"}"#,
                "Ryna_Guild",
            ),
            (r#"{"id":"guild","name":"","summary":"x"}"#, "Ryna_Guild"),
            (
                r#"{"id":"guild","name":"Guild","summary":"a\u202Eb"}"#,
                "Ryna_Guild",
            ),
            (good, "Other_Guild"),
        ] {
            let read = parse(json(bad, folder).as_bytes()).unwrap();
            assert!(
                read.entries.is_empty() && read.dropped.len() == 1,
                "{bad} {folder}"
            );
        }
    }
}
