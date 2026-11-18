//! The WoW core's mods (docs/ryna-agent-cores-mods.md section 6): which are bundled, where each
//! one's addon folders are, what's installed, and putting an install in step with what's on.
//!
//! A WoW mod is `kind: "addon"`: one or more addon folders (`Ryna_Chronicle`) installed beside the
//! core's `Ryna_Core`, with `## Dependencies: Ryna_Core`. Or it is `kind: "agent"`: no addon folder at all,
//! the work done by the agent itself (the Settings mod, [`crate::settings`]); installing and
//! removing one is nothing, and its toggle only says whether the agent does that work. The bundled ones are compiled in (their
//! `mods/wow/<id>/mod.json`, [`BUNDLED_MOD_JSON`]), so they version with the agent; a test holds the
//! list to the folders in `mods/wow`. Their addon folders come with the core's addon: the app carries
//! them beside `addon/Ryna_Core` (`addon/Ryna_Chronicle`), the release zip beside `Ryna`, and a checkout in
//! `mods/wow/<id>/addon/`. A mod kept in a repository of its own (it has a `pin.json`; Meter) has
//! its folder fetched to that same place from its pinned release (`scripts/fetch-addons.mjs`), and
//! its version is its own.
//!
//! A mod is on when the person turned it on, else when its manifest says `defaultOn`. Installing the
//! core's addon also installs every mod that's on and removes every mod that's off
//! ([`apply`]); a mod's folders are only ever installed into an install that has the core's addon.
//! The game keeps a toggle of its own (its AddOns list, which the core addon's Mods tab changes);
//! [`off_in_game`] reads it from the game's `AddOns.txt` files, so the app can show both.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use anyhow::{Context, Result};
use ryna_agent::mods::{ModManifest, ModSource};
use serde::Serialize;

use crate::addon_install::{self, compare_versions, ADDON_NAME};
use crate::discovery::Install;

/// The bundled mods' manifests, as built into this crate.
pub const BUNDLED_MOD_JSON: &[&str] = &[
    include_str!("../../../../mods/wow/chronicle/mod.json"),
    include_str!("../../../../mods/wow/automation/mod.json"),
    include_str!("../../../../mods/wow/prices/mod.json"),
    include_str!("../../../../mods/wow/fights/mod.json"),
    include_str!("../../../../mods/wow/meter/mod.json"),
    include_str!("../../../../mods/wow/settings/mod.json"),
];

/// The kind whose mods are addon folders, installed beside the core's.
pub const KIND: &str = "addon";
/// The kind whose mods run in the agent: no folder, nothing in the game.
pub const AGENT_KIND: &str = "agent";

/// Whether `m` is an agent mod (no addon folder: install and remove do nothing).
pub fn is_agent_mod(m: &ModManifest) -> bool {
    m.kind == AGENT_KIND
}

/// The bundled mods, parsed once: every one is this core's, an `addon` with folders or an `agent`
/// mod with none, bundled, and works with this core's version (a test holds them to it; one that isn't is left out rather than installed).
pub fn bundled() -> &'static [ModManifest] {
    static MODS: OnceLock<Vec<ModManifest>> = OnceLock::new();
    MODS.get_or_init(|| {
        let core = crate::wow_core::manifest();
        BUNDLED_MOD_JSON
            .iter()
            .filter_map(|json| ModManifest::parse(json).ok())
            .filter(|m| {
                m.core == core.id
                    && ((m.kind == KIND && !m.addons.is_empty())
                        || (m.kind == AGENT_KIND && m.addons.is_empty()))
                    && m.source == ModSource::Bundled
                    && m.works_with_core(&core.version)
                    && m.works_with_api(core.api)
            })
            .collect()
    })
}

/// Whether `m` is on: the person's choice (`chosen`, by mod id), else its `defaultOn`.
pub fn is_on(m: &ModManifest, chosen: &BTreeMap<String, bool>) -> bool {
    chosen.get(&m.id).copied().unwrap_or(m.default_on)
}

fn has_toc_named(dir: &Path, folder: &str) -> bool {
    dir.join(format!("{folder}.toc")).is_file()
}

/// Where the addon folder `folder` of mod `m` is, next to the core's addon at `core_source` (the
/// `Ryna` folder a front end installs from): beside it (`<…>/Ryna_Chronicle`, the app's resources
/// and the release zip), else in the repository's layout (`cores/wow/addon/Ryna_Core` →
/// `mods/wow/<id>/addon/<folder>`, a checkout).
pub fn addon_source(core_source: &Path, m: &ModManifest, folder: &str) -> Option<PathBuf> {
    let beside = core_source.parent().map(|p| p.join(folder));
    let checkout = core_source.ancestors().nth(4).map(|root| {
        root.join("mods")
            .join(&m.core)
            .join(&m.id)
            .join("addon")
            .join(folder)
    });
    [beside, checkout]
        .into_iter()
        .flatten()
        .find(|d| has_toc_named(d, folder))
}

/// The version a front end would install for `m`: its first addon folder's `## Version:`, else the
/// manifest's.
pub fn bundled_version(core_source: Option<&Path>, m: &ModManifest) -> String {
    core_source
        .zip(m.addons.first())
        .and_then(|(src, folder)| {
            let dir = addon_source(src, m, folder)?;
            addon_install::folder_version_named(&dir, folder)
        })
        .unwrap_or_else(|| m.version.clone())
}

/// How many of the game's `AddOns.txt` files in `install` (one per character, under
/// `WTF/Account/<account>/<realm>/<character>/`, and the account's own) say `<folder>: disabled`.
pub fn off_in_game(install: &Install, folder: &str) -> usize {
    let accounts = install.path.join("WTF").join("Account");
    let mut files = Vec::new();
    let dirs = |p: &Path| -> Vec<PathBuf> {
        std::fs::read_dir(p)
            .map(|rd| {
                rd.filter_map(Result::ok)
                    .map(|e| e.path())
                    .filter(|p| p.is_dir())
                    .collect()
            })
            .unwrap_or_default()
    };
    for account in dirs(&accounts) {
        files.push(account.join("AddOns.txt"));
        for realm in dirs(&account) {
            for character in dirs(&realm) {
                files.push(character.join("AddOns.txt"));
            }
        }
    }
    files
        .iter()
        .filter_map(|f| std::fs::read(f).ok())
        .filter(|bytes| {
            String::from_utf8_lossy(bytes).lines().any(|line| {
                line.split_once(':').is_some_and(|(name, state)| {
                    name.trim().eq_ignore_ascii_case(folder)
                        && state.trim().eq_ignore_ascii_case("disabled")
                })
            })
        })
        .count()
}

/// One install's copy of a mod.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModInstall {
    /// The install (as `InstallStatus::path` has it).
    pub install: String,
    /// Its first addon folder is there.
    pub installed: bool,
    /// That folder's `## Version:`, when it's there and says.
    pub version: Option<String>,
    /// How many characters the game has it turned off for (its AddOns lists).
    pub off_in_game: usize,
}

/// A mod, for a front end: what it is, whether it's on, what's bundled and what's installed where.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModStatus {
    pub id: String,
    pub name: String,
    pub summary: String,
    /// `addon` (folders in the game) or `agent` (the agent's own work, nothing to install).
    pub kind: String,
    pub acts_in_game: bool,
    pub default_on: bool,
    pub on: bool,
    /// The version a front end would install.
    pub bundled: String,
    /// The addon folders it installs.
    pub addons: Vec<String>,
    pub installs: Vec<ModInstall>,
}

impl ModStatus {
    /// This mod in the install at `path`, if it was looked at there.
    pub fn at(&self, path: &str) -> Option<&ModInstall> {
        self.installs.iter().find(|i| i.install == path)
    }
}

/// Every bundled mod's status across `installs`.
pub fn statuses(
    installs: &[Install],
    core_source: Option<&Path>,
    chosen: &BTreeMap<String, bool>,
) -> Vec<ModStatus> {
    bundled()
        .iter()
        .map(|m| {
            let folder = m.addons.first().map(String::as_str).unwrap_or_default();
            ModStatus {
                id: m.id.clone(),
                name: m.name.clone(),
                summary: m.summary.clone(),
                kind: m.kind.clone(),
                acts_in_game: m.acts_in_game,
                default_on: m.default_on,
                on: is_on(m, chosen),
                bundled: bundled_version(core_source, m),
                addons: m.addons.clone(),
                // An agent mod is in no install: there's nothing of it in the game.
                installs: installs
                    .iter()
                    .filter(|_| !is_agent_mod(m))
                    .map(|i| ModInstall {
                        install: i.path.to_string_lossy().into_owned(),
                        installed: addon_install::addon_dir_named(i, folder).is_dir(),
                        version: addon_install::installed_folder_version(i, folder),
                        off_in_game: off_in_game(i, folder),
                    })
                    .collect(),
            }
        })
        .collect()
}

/// What `apply` did to one addon folder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", tag = "change")]
pub enum ModChange {
    Installed {
        id: String,
        name: String,
        folder: String,
        path: String,
        version: Option<String>,
    },
    Removed {
        id: String,
        name: String,
        folder: String,
        path: String,
    },
}

/// Put `install`'s mods in step with what's on: each on mod's folders copied in from beside
/// `core_source` (when the install has the core's addon), each off mod's folders removed. Only
/// `only` when it's given (one mod, by id). A folder this build doesn't carry is an error, and the
/// mods before it stay done.
pub fn apply(
    install: &Install,
    core_source: &Path,
    chosen: &BTreeMap<String, bool>,
    only: Option<&str>,
) -> Result<Vec<ModChange>> {
    let mut changes = Vec::new();
    let has_core = addon_install::has_addon(install);
    for m in bundled()
        .iter()
        .filter(|m| only.is_none_or(|id| m.id == id) && !is_agent_mod(m))
    {
        for folder in &m.addons {
            if folder == ADDON_NAME {
                continue;
            }
            if is_on(m, chosen) {
                if !has_core {
                    continue;
                }
                let source = addon_source(core_source, m, folder).with_context(|| {
                    format!(
                        "this build doesn't carry {folder}, the {} mod's addon",
                        m.name
                    )
                })?;
                let to = addon_install::install_named(&source, install, folder)?;
                changes.push(ModChange::Installed {
                    id: m.id.clone(),
                    name: m.name.clone(),
                    folder: folder.clone(),
                    path: to.to_string_lossy().into_owned(),
                    version: addon_install::folder_version_named(&source, folder),
                });
            } else if addon_install::remove_named(install, folder)? {
                changes.push(ModChange::Removed {
                    id: m.id.clone(),
                    name: m.name.clone(),
                    folder: folder.clone(),
                    path: addon_install::addon_dir_named(install, folder)
                        .to_string_lossy()
                        .into_owned(),
                });
            }
        }
    }
    Ok(changes)
}

/// Whether `install` has a mod that is on, already installed, and older than the copy beside
/// `core_source`: one a front end should bring up to date by itself, as it does the core's addon
/// ([`addon_install::needs_update`]). Never a mod that isn't installed, and never an install
/// without the core's addon.
pub fn needs_update(
    install: &Install,
    core_source: &Path,
    chosen: &BTreeMap<String, bool>,
) -> bool {
    if !addon_install::has_addon(install) {
        return false;
    }
    bundled()
        .iter()
        .filter(|m| !is_agent_mod(m) && is_on(m, chosen))
        .flat_map(|m| m.addons.iter().map(move |folder| (m, folder)))
        .filter(|(_, folder)| folder.as_str() != ADDON_NAME)
        .any(|(m, folder)| {
            let installed = addon_install::installed_folder_version(install, folder);
            let available = addon_source(core_source, m, folder)
                .and_then(|dir| addon_install::folder_version_named(&dir, folder));
            matches!((installed, available), (Some(i), Some(a)) if compare_versions(&i, &a).is_lt())
        })
}

/// The status card's words for a mod in the install at `path`.
pub fn row_words(m: &ModStatus, path: &str) -> (String, ryna_agent::core::Chip) {
    use ryna_agent::core::Chip;
    if m.kind == AGENT_KIND {
        return if m.on {
            ("On · runs in the app".to_string(), Chip::Pushed)
        } else {
            ("Off".to_string(), Chip::Waiting)
        };
    }
    let here = m.at(path);
    let installed = here.filter(|i| i.installed);
    let mut words = match (m.on, installed) {
        (true, Some(i)) => match i.version.as_deref() {
            Some(v) if compare_versions(v, &m.bundled).is_lt() => (
                format!("On · {v} installed, {} available", m.bundled),
                Chip::Update,
            ),
            Some(v) => (format!("On · {v} installed"), Chip::Pushed),
            None => ("On · installed".to_string(), Chip::Pushed),
        },
        (true, None) => (
            "On · not installed yet: install the addon".to_string(),
            Chip::Waiting,
        ),
        (false, Some(_)) => ("Off · still installed".to_string(), Chip::Waiting),
        (false, None) => ("Off".to_string(), Chip::Waiting),
    };
    if let Some(n) = here
        .map(|i| i.off_in_game)
        .filter(|n| *n > 0 && installed.is_some())
    {
        let who = if n == 1 {
            "1 character".to_string()
        } else {
            format!("{n} characters")
        };
        words.0.push_str(&format!(" · off in the game for {who}"));
        words.1 = Chip::Waiting;
    }
    words
}

#[cfg(test)]
mod tests {
    use super::*;
    use ryna_agent::core::Chip;

    fn root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..")
    }

    #[test]
    fn every_mod_folder_in_the_repository_is_bundled_and_valid() {
        let mut on_disk: Vec<String> = std::fs::read_dir(root().join("mods/wow"))
            .unwrap()
            .filter_map(Result::ok)
            .filter(|e| e.path().join("mod.json").is_file())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        on_disk.sort();
        assert_eq!(BUNDLED_MOD_JSON.len(), on_disk.len());
        let mut ids: Vec<String> = bundled().iter().map(|m| m.id.clone()).collect();
        ids.sort();
        assert_eq!(
            ids, on_disk,
            "every mod.json parses, is wow's and works with this core"
        );
        let core = crate::wow_core::manifest();
        // The core's API level is written three times, and each must say the same: core.json, the
        // addon's Mods.lua (what a mod reads in the game) and the addon's TOC.
        let level = core.api.expect("core.json names the API level");
        let core_addon = root().join("cores/wow/addon").join(ADDON_NAME);
        let mods_lua = std::fs::read_to_string(core_addon.join("Mods.lua")).unwrap();
        // By line, never by "\n": a Windows checkout has CRLF endings.
        assert!(
            mods_lua.lines().any(|l| l == format!("R.API = {level}")),
            "Mods.lua's R.API is core.json's api ({level})"
        );
        let toc_level = |toc: &str| -> Option<u32> {
            toc.lines()
                .find_map(|l| l.strip_prefix("## X-Ryna-API:"))
                .and_then(|v| v.trim().parse().ok())
        };
        let core_toc =
            std::fs::read_to_string(core_addon.join(format!("{ADDON_NAME}.toc"))).unwrap();
        assert_eq!(toc_level(&core_toc), Some(level), "{ADDON_NAME}.toc");
        // The lowest level it serves, the same three places (docs 6.2).
        let lowest = core.api_min.expect("core.json names the lowest level");
        assert!((1..=level).contains(&lowest));
        assert!(
            mods_lua
                .lines()
                .any(|l| l == format!("R.API_MIN = {lowest}")),
            "Mods.lua's R.API_MIN is core.json's apiMin ({lowest})"
        );
        let toc_lowest = crate::addon_store::toc_field(&core_toc, crate::addon_store::TOC_API_MIN);
        assert_eq!(toc_lowest, Some(lowest.to_string()), "{ADDON_NAME}.toc");
        for m in bundled() {
            // A mod kept in a repository of its own has a pin.json: its version is its pin's tag, and
            // its folder is here only once scripts/fetch-addons.mjs has run (CI's addon checks and
            // every build of the app do; a bare checkout hasn't).
            let pin = std::fs::read_to_string(root().join("mods/wow").join(&m.id).join("pin.json"))
                .ok()
                .map(|json| serde_json::from_str::<serde_json::Value>(&json).unwrap());
            match &pin {
                Some(pin) => assert_eq!(
                    pin["tag"].as_str(),
                    Some(format!("v{}", m.version).as_str()),
                    "{}: a pinned mod's version is its pin's tag",
                    m.id
                ),
                None => assert_eq!(
                    m.version, core.version,
                    "{}: a bundled mod takes the agent's version",
                    m.id
                ),
            }
            let source = root().join("cores/wow/addon/Ryna_Core");
            for folder in &m.addons {
                let Some(dir) = addon_source(&source, m, folder) else {
                    assert!(pin.is_some(), "{folder}: a mod kept here has its folder");
                    continue;
                };
                assert_eq!(
                    addon_install::folder_version_named(&dir, folder).as_deref(),
                    Some(m.version.as_str()),
                    "{folder}.toc"
                );
                let toc = std::fs::read_to_string(dir.join(format!("{folder}.toc"))).unwrap();
                assert!(
                    toc.lines()
                        .any(|l| l == format!("## Dependencies: {ADDON_NAME}")),
                    "{folder}.toc depends on the core's folder"
                );
                // What the mod reads in the game (its TOC) is what its manifest says.
                assert!(
                    m.requires.api.is_some(),
                    "{}: an addon mod names a level",
                    m.id
                );
                assert_eq!(toc_level(&toc), m.requires.api, "{folder}.toc X-Ryna-API");
                assert!(
                    m.requires.api <= Some(level) && m.requires.api >= Some(lowest),
                    "{}: a level this core serves",
                    m.id
                );
            }
        }
        let chronicle = bundled().iter().find(|m| m.id == "chronicle").unwrap();
        assert!(chronicle.default_on && !chronicle.acts_in_game);
        assert_eq!(chronicle.addons, ["Ryna_Chronicle"]);
        // Automation acts in the game, so it's off until the person turns it on.
        let automation = bundled().iter().find(|m| m.id == "automation").unwrap();
        assert!(!automation.default_on && automation.acts_in_game);
        assert_eq!(automation.addons, ["Ryna_Automation"]);
        // Prices only reads the game's item data: off by default like every new mod, and acts on nothing.
        let prices = bundled().iter().find(|m| m.id == "prices").unwrap();
        assert!(!prices.default_on && !prices.acts_in_game);
        assert_eq!(prices.addons, ["Ryna_Prices"]);
        // Fights only reads the app's file of the combat log: off by default, acts on nothing.
        let fights = bundled().iter().find(|m| m.id == "fights").unwrap();
        assert!(!fights.default_on && !fights.acts_in_game);
        assert_eq!(fights.addons, ["Ryna_Fights"]);
        // Meter only reads and draws what the game says of a fight: off by default, acts on nothing.
        let meter = bundled().iter().find(|m| m.id == "meter").unwrap();
        assert!(!meter.default_on && !meter.acts_in_game);
        assert_eq!(meter.addons, ["Ryna_Meter"]);
        // Settings runs in the agent: no folder, nothing in the game, off until turned on.
        let settings = bundled().iter().find(|m| m.id == "settings").unwrap();
        assert!(is_agent_mod(settings) && settings.addons.is_empty());
        assert!(!settings.default_on && !settings.acts_in_game);
    }

    /// A release layout: `<dir>/Ryna_Core` and each bundled mod's folder beside it, each with its toc.
    fn release(dir: &Path, version: &str) -> PathBuf {
        for name in [
            ADDON_NAME,
            "Ryna_Chronicle",
            "Ryna_Automation",
            "Ryna_Prices",
            "Ryna_Fights",
            "Ryna_Meter",
        ] {
            let a = dir.join("release").join(name);
            std::fs::create_dir_all(&a).unwrap();
            std::fs::write(
                a.join(format!("{name}.toc")),
                format!("## Version: {version}\n"),
            )
            .unwrap();
        }
        dir.join("release").join(ADDON_NAME)
    }

    fn forever(dir: &Path) -> Install {
        Install {
            path: dir.join("_forever_"),
            flavor: Some("wowf".into()),
        }
    }

    #[test]
    fn an_installed_mod_older_than_the_copy_to_install_needs_an_update() {
        let dir = tempfile::tempdir().unwrap();
        let source = release(dir.path(), "0.5.0");
        let inst = forever(dir.path());
        let none = BTreeMap::new();
        assert!(!needs_update(&inst, &source, &none), "no core addon there");
        addon_install::install(&source, &inst).unwrap();
        apply(&inst, &source, &none, None).unwrap();
        assert!(
            !needs_update(&inst, &source, &none),
            "all as new as the source"
        );

        // Chronicle (on by default) moves on alone; the core stays where it is.
        let toc = dir.path().join("release/Ryna_Chronicle/Ryna_Chronicle.toc");
        std::fs::write(&toc, "## Version: 0.5.1\n").unwrap();
        assert!(needs_update(&inst, &source, &none));
        // A mod that is off, or on and not installed, never asks for one.
        let off = BTreeMap::from([("chronicle".to_string(), false)]);
        assert!(!needs_update(&inst, &source, &off));
        let meter = dir.path().join("release/Ryna_Meter/Ryna_Meter.toc");
        std::fs::write(&meter, "## Version: 9.0.0\n").unwrap();
        let meter_on = BTreeMap::from([
            ("chronicle".to_string(), false),
            ("meter".to_string(), true),
        ]);
        assert!(
            !needs_update(&inst, &source, &meter_on),
            "not installed yet"
        );
        // And once brought up to date it is quiet again.
        apply(&inst, &source, &none, None).unwrap();
        assert!(!needs_update(&inst, &source, &none));
    }

    #[test]
    fn on_mods_go_in_with_the_core_and_off_ones_come_out() {
        let dir = tempfile::tempdir().unwrap();
        let source = release(dir.path(), "0.3.0");
        let inst = forever(dir.path());
        let none = BTreeMap::new();

        // No core addon yet: nothing goes in.
        assert!(apply(&inst, &source, &none, None).unwrap().is_empty());
        addon_install::install(&source, &inst).unwrap();

        // Chronicle is on by default.
        let changes = apply(&inst, &source, &none, None).unwrap();
        assert!(
            matches!(&changes[..], [ModChange::Installed { id, version: Some(v), .. }] if id == "chronicle" && v == "0.3.0"),
            "{changes:?}"
        );
        let s = statuses(std::slice::from_ref(&inst), Some(&source), &none);
        let here = s[0].at(&inst.path.to_string_lossy()).unwrap();
        assert!(s[0].on && here.installed && here.version.as_deref() == Some("0.3.0"));
        assert_eq!(s[0].bundled, "0.3.0");

        // Turned off: its folder comes out, the core's stays.
        let off = BTreeMap::from([("chronicle".to_string(), false)]);
        let changes = apply(&inst, &source, &off, Some("chronicle")).unwrap();
        assert!(
            matches!(&changes[..], [ModChange::Removed { folder, .. }] if folder == "Ryna_Chronicle")
        );
        assert!(addon_install::has_addon(&inst));
        assert!(!addon_install::addon_dir_named(&inst, "Ryna_Chronicle").exists());
        assert!(
            apply(&inst, &source, &off, None).unwrap().is_empty(),
            "already out"
        );
        let s = statuses(std::slice::from_ref(&inst), Some(&source), &off);
        assert!(!s[0].on && !s[0].installs[0].installed);

        // `only` names another mod: nothing happens to this one.
        assert!(apply(&inst, &source, &none, Some("nope"))
            .unwrap()
            .is_empty());

        // A build that doesn't carry the folder says so.
        std::fs::remove_dir_all(dir.path().join("release/Ryna_Chronicle")).unwrap();
        let err = apply(&inst, &source, &none, None).unwrap_err().to_string();
        assert!(err.contains("Ryna_Chronicle"), "{err}");
    }

    #[test]
    fn a_mod_off_by_default_stays_out_until_turned_on_and_leaves_when_turned_off() {
        let dir = tempfile::tempdir().unwrap();
        let source = release(dir.path(), "0.3.0");
        let inst = forever(dir.path());
        addon_install::install(&source, &inst).unwrap();
        let none = BTreeMap::new();
        let here = inst.path.to_string_lossy().into_owned();
        let of = |s: &[ModStatus], id: &str| s.iter().find(|m| m.id == id).unwrap().clone();

        // Both are listed; only Chronicle (on by default) goes in.
        let changes = apply(&inst, &source, &none, None).unwrap();
        assert!(
            matches!(&changes[..], [ModChange::Installed { id, .. }] if id == "chronicle"),
            "{changes:?}"
        );
        let s = statuses(std::slice::from_ref(&inst), Some(&source), &none);
        assert_eq!(s.len(), BUNDLED_MOD_JSON.len());
        let automation = of(&s, "automation");
        assert!(!automation.on && !automation.default_on && automation.acts_in_game);
        assert!(!automation.at(&here).unwrap().installed);
        assert_eq!(row_words(&automation, &here).0, "Off");
        assert!(!addon_install::addon_dir_named(&inst, "Ryna_Automation").exists());

        // Turned on: its folder goes in, Chronicle's is untouched.
        let on = BTreeMap::from([("automation".to_string(), true)]);
        let changes = apply(&inst, &source, &on, Some("automation")).unwrap();
        assert!(
            matches!(&changes[..], [ModChange::Installed { id, folder, version: Some(v), .. }]
                if id == "automation" && folder == "Ryna_Automation" && v == "0.3.0"),
            "{changes:?}"
        );
        let s = statuses(std::slice::from_ref(&inst), Some(&source), &on);
        assert!(of(&s, "automation").on && of(&s, "automation").at(&here).unwrap().installed);
        assert!(of(&s, "chronicle").at(&here).unwrap().installed);
        // A full apply (the addon installed again) keeps both.
        let changes = apply(&inst, &source, &on, None).unwrap();
        assert_eq!(changes.len(), 2, "{changes:?}");

        // Turned off again: only its folder comes out.
        let off = BTreeMap::from([("automation".to_string(), false)]);
        let changes = apply(&inst, &source, &off, Some("automation")).unwrap();
        assert!(
            matches!(&changes[..], [ModChange::Removed { folder, .. }] if folder == "Ryna_Automation"),
            "{changes:?}"
        );
        assert!(!addon_install::addon_dir_named(&inst, "Ryna_Automation").exists());
        assert!(addon_install::addon_dir_named(&inst, "Ryna_Chronicle").is_dir());
        assert!(addon_install::has_addon(&inst));
    }

    #[test]
    fn the_games_own_addon_list_is_read() {
        let dir = tempfile::tempdir().unwrap();
        let inst = forever(dir.path());
        assert_eq!(off_in_game(&inst, "Ryna_Chronicle"), 0);
        let acct = inst.path.join("WTF/Account/ACCOUNT1");
        for (who, text) in [
            (
                "Realm/Aerin",
                "Ryna: enabled\r\nRyna_Chronicle: disabled\r\n",
            ),
            ("Realm/Bryn", "ryna_chronicle : Disabled\n"),
            ("Realm/Cael", "Ryna_Chronicle: enabled\n"),
            ("Other/Dara", "Ryna_ChronicleX: disabled\n"),
        ] {
            let d = acct.join(who);
            std::fs::create_dir_all(&d).unwrap();
            std::fs::write(d.join("AddOns.txt"), text).unwrap();
        }
        assert_eq!(off_in_game(&inst, "Ryna_Chronicle"), 2);
    }

    #[test]
    fn an_agent_mod_installs_nothing_and_its_row_is_on_or_off() {
        let dir = tempfile::tempdir().unwrap();
        let source = release(dir.path(), "0.3.0");
        let inst = forever(dir.path());
        addon_install::install(&source, &inst).unwrap();
        let on = BTreeMap::from([("settings".to_string(), true)]);
        // On: nothing goes into the game for it, and no folder is looked for.
        assert!(apply(&inst, &source, &on, Some("settings"))
            .unwrap()
            .is_empty());
        let before: Vec<_> = std::fs::read_dir(inst.path.join("Interface/AddOns"))
            .unwrap()
            .filter_map(Result::ok)
            .map(|e| e.file_name())
            .collect();
        let off = BTreeMap::from([("settings".to_string(), false)]);
        assert!(apply(&inst, &source, &off, Some("settings"))
            .unwrap()
            .is_empty());
        let after: Vec<_> = std::fs::read_dir(inst.path.join("Interface/AddOns"))
            .unwrap()
            .filter_map(Result::ok)
            .map(|e| e.file_name())
            .collect();
        assert_eq!(before, after);
        let here = inst.path.to_string_lossy().into_owned();
        let s = statuses(std::slice::from_ref(&inst), Some(&source), &on);
        let settings = s.iter().find(|m| m.id == "settings").unwrap();
        assert_eq!(settings.kind, AGENT_KIND);
        assert!(settings.on && settings.installs.is_empty());
        assert_eq!(
            row_words(settings, &here),
            ("On · runs in the app".to_string(), Chip::Pushed)
        );
        let s = statuses(std::slice::from_ref(&inst), Some(&source), &BTreeMap::new());
        let settings = s.iter().find(|m| m.id == "settings").unwrap();
        assert!(!settings.on);
        assert_eq!(row_words(settings, &here).0, "Off");
    }

    #[test]
    fn the_rows_say_on_off_installed_and_off_in_the_game() {
        let m = |on: bool, installed: bool, version: Option<&str>, off: usize| ModStatus {
            id: "chronicle".into(),
            name: "Chronicle".into(),
            summary: "x".into(),
            kind: KIND.into(),
            acts_in_game: false,
            default_on: true,
            on,
            bundled: "0.3.0".into(),
            addons: vec!["Ryna_Chronicle".into()],
            installs: vec![ModInstall {
                install: "/wow/_forever_".into(),
                installed,
                version: version.map(str::to_string),
                off_in_game: off,
            }],
        };
        let words = |s: ModStatus| row_words(&s, "/wow/_forever_");
        assert_eq!(
            words(m(true, true, Some("0.3.0"), 0)),
            ("On · 0.3.0 installed".into(), Chip::Pushed)
        );
        assert_eq!(
            words(m(true, true, Some("0.2.6"), 0)),
            ("On · 0.2.6 installed, 0.3.0 available".into(), Chip::Update)
        );
        assert_eq!(
            words(m(true, false, None, 0)),
            (
                "On · not installed yet: install the addon".into(),
                Chip::Waiting
            )
        );
        assert_eq!(
            words(m(false, false, None, 0)),
            ("Off".into(), Chip::Waiting)
        );
        assert_eq!(
            words(m(false, true, Some("0.3.0"), 0)).0,
            "Off · still installed"
        );
        assert_eq!(
            words(m(true, true, Some("0.3.0"), 2)),
            (
                "On · 0.3.0 installed · off in the game for 2 characters".into(),
                Chip::Waiting
            )
        );
        assert_eq!(
            row_words(&m(true, false, None, 0), "/elsewhere").1,
            Chip::Waiting
        );
    }
}
