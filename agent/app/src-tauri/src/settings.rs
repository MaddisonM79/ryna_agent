//! The app's own settings, in its config folder (`settings.json`): only what the core doesn't
//! keep itself. Launch at login lives with the OS (the autostart plugin asks it), not here.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub const FILE: &str = "settings.json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    /// The game folder (or one product folder) the person chose, in place of the OS defaults.
    #[serde(default)]
    pub install_override: Option<PathBuf>,
    /// "Send fight digests": each fight's events go up beside its summary (on by default).
    #[serde(default = "yes")]
    pub send_digests: bool,
    /// "Keep raw logs on ryna.io": each fight's own log lines go up too (off by default).
    #[serde(default)]
    pub keep_raw: bool,
    /// Each mod the person turned on or off, by id; a mod not here is its manifest's `defaultOn`
    /// (Chronicle: on).
    #[serde(default)]
    pub mods: BTreeMap<String, bool>,
    /// Each fetched core the person turned on (or off again) on this computer, by id; a core not
    /// here has never been turned on, and is off.
    #[serde(default)]
    pub cores: BTreeMap<String, bool>,
}

fn yes() -> bool {
    true
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            install_override: None,
            send_digests: true,
            keep_raw: false,
            mods: BTreeMap::new(),
            cores: BTreeMap::new(),
        }
    }
}

impl Settings {
    /// The person's choice for the fetched core `id` on this computer: None when they've never
    /// turned it on (it's off, and its page offers to turn it on).
    pub fn core_choice(&self, id: &str) -> Option<bool> {
        self.cores.get(id).copied()
    }

    /// Whether the fetched core `id` is on: only once turned on here, and until turned off.
    pub fn core_on(&self, id: &str) -> bool {
        self.core_choice(id).unwrap_or(false)
    }

    /// What the watch loop does with the combat logs.
    pub fn combat(&self) -> ryna_wow_core::combat::CombatOptions {
        ryna_wow_core::combat::CombatOptions {
            enabled: true,
            digests: self.send_digests,
            raw: self.keep_raw,
        }
    }
}

impl Settings {
    /// The settings in `dir`, or the defaults when there are none or they can't be read.
    pub fn load(dir: &Path) -> Settings {
        std::fs::read(dir.join(FILE))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }

    /// Written whole through a temp file and a rename.
    pub fn save(&self, dir: &Path) -> std::io::Result<()> {
        std::fs::create_dir_all(dir)?;
        let tmp = dir.join(format!(".{FILE}.tmp"));
        let body = serde_json::to_vec_pretty(self).map_err(std::io::Error::other)?;
        let result =
            std::fs::write(&tmp, body).and_then(|()| std::fs::rename(&tmp, dir.join(FILE)));
        if result.is_err() {
            let _ = std::fs::remove_file(&tmp);
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_and_missing_or_corrupt_is_default() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(Settings::load(dir.path()), Settings::default());
        let s = Settings {
            install_override: Some(PathBuf::from("/Games/World of Warcraft")),
            send_digests: false,
            keep_raw: true,
            mods: BTreeMap::from([("chronicle".to_string(), false)]),
            cores: BTreeMap::from([("palworld".to_string(), false)]),
        };
        assert!(!s.core_on("palworld") && !s.core_on("satisfactory"));
        assert_eq!(s.core_choice("palworld"), Some(false));
        assert_eq!(s.core_choice("satisfactory"), None);
        s.save(dir.path()).unwrap();
        assert_eq!(Settings::load(dir.path()), s);
        std::fs::write(dir.path().join(FILE), "{ not json").unwrap();
        assert_eq!(Settings::load(dir.path()), Settings::default());
        // Settings written before the combat ones: digests on, raw off.
        std::fs::write(dir.path().join(FILE), r#"{ "installOverride": null }"#).unwrap();
        let old = Settings::load(dir.path());
        assert!(old.send_digests && !old.keep_raw);
        // Settings written before mods: none chosen, so each is its default.
        assert!(old.mods.is_empty());
        // And before fetched cores: none turned on yet, so every one off.
        assert!(!old.core_on("palworld"));
        assert_eq!(old.core_choice("palworld"), None);
        let v: serde_json::Value =
            serde_json::from_slice(&serde_json::to_vec(&s).unwrap()).unwrap();
        assert_eq!(v["mods"]["chronicle"], serde_json::json!(false));
    }
}
