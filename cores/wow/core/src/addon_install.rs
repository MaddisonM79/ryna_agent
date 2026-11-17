//! Installing the addon: copy an `addon/Ryna_Core` folder into each install's `Interface/AddOns/Ryna_Core`.
//!
//! Before 0.5.0 the folder was `Ryna` ([`LEGACY_ADDON_NAME`]). An install that still has that one
//! counts as having the addon (an old one: [`needs_update`]), the agent's files go on being written
//! into it ([`addon_dir`]), and [`install`] moves it over: the new folder in, each account's saved
//! file copied to the new folder's name (`Ryna.lua` to `Ryna_Core.lua`, the game names the file after
//! the folder), the agent's `Data/` carried across, and the old folder removed. Only with the game
//! closed: a running game still has the old addon in memory and writes the old file at logout.
//!
//! The release puts the addon beside the binary (`<dir>/addon/Ryna_Core` or `<dir>/Ryna`); `--from`
//! names another. The copy is made next to the old one and swapped in, so a failed copy leaves
//! the installed addon as it was. Symbolic links in the source are skipped.
//!
//! `installed_addon_version` reads the installed copy's `## Version:` from its `Ryna_Core.toc`, for the
//! app's install row and the heartbeat; `bundled_addon_version` reads the same line from the copy
//! a front end would install (the app's resources, the CLI's `--from`), and `addon_state` says
//! how the two compare (`compare_versions`: dotted integers).
//!
//! A WoW mod is its own addon folder beside `Ryna` (`Ryna_Chronicle`): `install_named`,
//! `remove_named` and `installed_folder_version` do the same for any folder name; `install` is
//! `install_named` with `Ryna_Core`, and the move from `Ryna` after it.

use std::cmp::Ordering;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};

use crate::discovery::Install;

pub const ADDON_NAME: &str = "Ryna_Core";
/// The folder the core's addon had before 0.5.0.
pub const LEGACY_ADDON_NAME: &str = "Ryna";

/// `<install>/Interface/AddOns/Ryna`, the folder from before 0.5.0, whether or not it's there.
pub fn legacy_addon_dir(install: &Install) -> PathBuf {
    addon_dir_named(install, LEGACY_ADDON_NAME)
}

/// Whether the folder from before 0.5.0 is still in this install.
pub fn has_legacy_addon(install: &Install) -> bool {
    legacy_addon_dir(install).is_dir()
}

/// The core addon's folder in this install: `Interface/AddOns/Ryna_Core`, or the old `Ryna` while
/// that is the only one there (an install not moved over yet), whether or not either is there.
pub fn addon_dir(install: &Install) -> PathBuf {
    let current = addon_dir_named(install, ADDON_NAME);
    if !current.is_dir() && has_legacy_addon(install) {
        return legacy_addon_dir(install);
    }
    current
}

/// The `## Version:` of the addon folder `dir`, from the `.toc` named after the folder.
fn own_toc_version(dir: &Path) -> Option<String> {
    folder_version_named(dir, dir.file_name()?.to_str()?)
}

/// Whether the addon's folder is in this install.
pub fn has_addon(install: &Install) -> bool {
    addon_dir(install).is_dir()
}

/// The `## Version:` line of a `.toc`, trimmed; None when there's none or it's empty.
pub fn parse_toc_version(toc: &str) -> Option<String> {
    toc.trim_start_matches('\u{feff}').lines().find_map(|line| {
        let rest = line.trim().strip_prefix("##")?;
        let (key, value) = rest.split_once(':')?;
        if !key.trim().eq_ignore_ascii_case("version") {
            return None;
        }
        let value = value.trim();
        (!value.is_empty()).then(|| value.to_string())
    })
}

/// The `## Version:` of the `Ryna_Core.toc` in the addon folder `dir`; None when it can't be read or
/// names no version.
fn folder_version(dir: &Path) -> Option<String> {
    folder_version_named(dir, ADDON_NAME)
}

/// The `## Version:` of `<name>.toc` in the addon folder `dir`.
pub fn folder_version_named(dir: &Path, name: &str) -> Option<String> {
    let bytes = std::fs::read(dir.join(format!("{name}.toc"))).ok()?;
    parse_toc_version(&String::from_utf8_lossy(&bytes))
}

/// `<install>/Interface/AddOns/<name>`, whether or not it's there.
pub fn addon_dir_named(install: &Install, name: &str) -> PathBuf {
    install.path.join("Interface").join("AddOns").join(name)
}

/// The installed version of the addon folder `name` (its `<name>.toc`), when it's there and says.
pub fn installed_folder_version(install: &Install, name: &str) -> Option<String> {
    folder_version_named(&addon_dir_named(install, name), name)
}

/// The installed addon's version, from `Interface/AddOns/Ryna_Core/Ryna_Core.toc` (or the old
/// `Ryna/Ryna.toc`, see [`addon_dir`]); None when the addon isn't installed, the file can't be read,
/// or it names no version.
pub fn installed_addon_version(install: &Install) -> Option<String> {
    own_toc_version(&addon_dir(install))
}

/// The version of the addon `from` names (resolved as `resolve_source` does: the folder itself,
/// `Ryna/` or `addon/Ryna_Core/` inside it); None when there's no addon there or its `.toc` names no
/// version.
pub fn bundled_addon_version(from: &Path) -> Option<String> {
    folder_version(&resolve_source(from).ok()?)
}

/// Two versions as dotted integers: `0.10.0` is after `0.9.1`, and `0.1` equals `0.1.0`. A part
/// that isn't a number counts its leading digits (`2-beta` as 2, `x` as 0).
pub fn compare_versions(a: &str, b: &str) -> Ordering {
    fn parts(v: &str) -> Vec<u64> {
        let mut parts: Vec<u64> = v
            .trim()
            .trim_start_matches(['v', 'V'])
            .split('.')
            .map(|p| {
                let digits: String = p.trim().chars().take_while(char::is_ascii_digit).collect();
                digits.parse().unwrap_or(0)
            })
            .collect();
        while parts.last() == Some(&0) {
            parts.pop();
        }
        parts
    }
    parts(a).cmp(&parts(b))
}

/// How an install's addon compares with the one a front end would install.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum AddonState {
    /// `Interface/AddOns/Ryna_Core` isn't there.
    NotInstalled,
    /// The same version as the bundled one.
    Current,
    /// Older than the bundled one: an update is available.
    Older,
    /// Newer than the bundled one (installed from elsewhere).
    Newer,
    /// Installed, but one side names no version (or nothing is bundled), so they can't be
    /// compared.
    Unknown,
}

/// `installed` (`None` with `has_addon` false: not there) against `bundled`.
pub fn addon_state(has_addon: bool, installed: Option<&str>, bundled: Option<&str>) -> AddonState {
    if !has_addon {
        return AddonState::NotInstalled;
    }
    match (installed, bundled) {
        (Some(i), Some(b)) => match compare_versions(i, b) {
            Ordering::Equal => AddonState::Current,
            Ordering::Less => AddonState::Older,
            Ordering::Greater => AddonState::Newer,
        },
        _ => AddonState::Unknown,
    }
}

/// Whether a front end should update this install's addon by itself: it is there, both sides name
/// a version, and the installed one is older than `bundled`. Never where the addon isn't
/// installed (that is the person's to choose), never a newer copy (installed from elsewhere), and
/// never one that names no version. An install with the folder from before 0.5.0 always does,
/// whatever its version says: that folder is older than any addon this build carries.
pub fn needs_update(install: &Install, bundled: Option<&str>) -> bool {
    (has_legacy_addon(install) && bundled.is_some())
        || addon_state(
            has_addon(install),
            installed_addon_version(install).as_deref(),
            bundled,
        ) == AddonState::Older
}

fn has_toc(dir: &Path) -> bool {
    std::fs::read_dir(dir)
        .map(|rd| {
            rd.filter_map(Result::ok).any(|e| {
                e.path()
                    .extension()
                    .is_some_and(|x| x.eq_ignore_ascii_case("toc"))
            })
        })
        .unwrap_or(false)
}

/// The addon folder `from` names: `from` itself when it holds a `.toc`, else `from/Ryna`, else
/// `from/addon/Ryna_Core`.
pub fn resolve_source(from: &Path) -> Result<PathBuf> {
    for candidate in [
        from.to_path_buf(),
        from.join(ADDON_NAME),
        from.join("addon").join(ADDON_NAME),
    ] {
        if has_toc(&candidate) {
            return Ok(candidate);
        }
    }
    bail!(
        "no addon in {}: expected a folder holding Ryna_Core.toc (or Ryna_Core/ or addon/Ryna_Core/ inside it)",
        from.display()
    )
}

/// The addon beside the running binary, if the release layout put it there.
pub fn beside_binary() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    resolve_source(exe.parent()?).ok()
}

fn copy_dir(src: &Path, dst: &Path) -> Result<()> {
    std::fs::create_dir_all(dst).with_context(|| format!("creating {}", dst.display()))?;
    for entry in std::fs::read_dir(src).with_context(|| format!("reading {}", src.display()))? {
        let entry = entry?;
        let kind = entry.file_type()?;
        let to = dst.join(entry.file_name());
        if kind.is_dir() {
            copy_dir(&entry.path(), &to)?;
        } else if kind.is_file() {
            std::fs::copy(entry.path(), &to)
                .with_context(|| format!("copying to {}", to.display()))?;
        }
    }
    Ok(())
}

/// Copy `source` (the `Ryna_Core` folder) into `install`'s `Interface/AddOns/Ryna_Core`, replacing
/// what's there, and move an install over from the folder it had before 0.5.0. Returns the installed
/// folder. Refused while the game runs when there is an old folder to move.
pub fn install(source: &Path, install: &Install) -> Result<PathBuf> {
    install_checked(source, install, &|| {
        crate::game_process::game_running_at(&install.path)
    })
}

/// [`install`], with the question "is the game running?" passed in.
pub fn install_checked(
    source: &Path,
    install: &Install,
    game_running: &dyn Fn() -> bool,
) -> Result<PathBuf> {
    let moving = has_legacy_addon(install);
    if moving && game_running() {
        bail!(
            "close the game first: this update moves the addon from the folder {LEGACY_ADDON_NAME} to \
             {ADDON_NAME}, and the game still has the old one open"
        );
    }
    let had_current = addon_dir_named(install, ADDON_NAME).is_dir();
    let target = install_named(source, install, ADDON_NAME)?;
    if moving {
        retire_legacy(install, &target, had_current)?;
    }
    Ok(target)
}

/// The saved file the game wrote for the folder `Ryna`, and the one it reads for `Ryna_Core`.
const LEGACY_SAVED: &str = crate::discovery::LEGACY_SAVED_VARIABLES_FILE;
const SAVED: &str = crate::discovery::SAVED_VARIABLES_FILE;

/// Finish the move from the old folder, with the new one already in place at `target`: each
/// account's saved file, then the agent's `Data/`, then the old folder goes. A saved file that
/// can't be copied stops it with the old folder still there, so the next install tries again and
/// nothing recorded is left behind under a name the game no longer reads.
fn retire_legacy(install: &Install, target: &Path, had_current: bool) -> Result<()> {
    let accounts = install.path.join("WTF").join("Account");
    if let Ok(rd) = std::fs::read_dir(&accounts) {
        for account in rd.filter_map(Result::ok) {
            let saved = account.path().join("SavedVariables");
            adopt_saved_file(&saved.join(LEGACY_SAVED), &saved.join(SAVED))?;
        }
    }
    let legacy = legacy_addon_dir(install);
    // The agent's tables, unless the new folder already had its own (both were there).
    let kept = legacy.join(crate::prices_data::DIR);
    if !had_current && kept.is_dir() {
        let into = target.join(crate::prices_data::DIR);
        let _ = std::fs::remove_dir_all(&into);
        if let Err(e) = std::fs::rename(&kept, &into) {
            eprintln!("couldn't keep {}: {e}", into.display());
        }
    }
    std::fs::remove_dir_all(&legacy).with_context(|| format!("removing {}", legacy.display()))
}

/// Copy the old folder's saved file to the new folder's name, keeping its modified time (which is
/// when the game saved it, and what the agent goes by). Not when the new one is there and no
/// older: the game has written it since. The old file stays, for someone who goes back a version.
fn adopt_saved_file(old: &Path, new: &Path) -> Result<()> {
    let Ok(old_meta) = std::fs::metadata(old) else {
        return Ok(());
    };
    if !old_meta.is_file() {
        return Ok(());
    }
    let old_time = old_meta.modified().ok();
    if let Ok(new_meta) = std::fs::metadata(new) {
        match (old_time, new_meta.modified().ok()) {
            (Some(o), Some(n)) if o > n => {}
            _ => return Ok(()),
        }
    }
    let staging = new.with_extension("lua.ryna.tmp");
    let copy = || -> std::io::Result<()> {
        std::fs::copy(old, &staging)?;
        if let Some(t) = old_time {
            std::fs::File::options()
                .write(true)
                .open(&staging)?
                .set_modified(t)?;
        }
        std::fs::rename(&staging, new)
    };
    copy().map_err(|e| {
        let _ = std::fs::remove_file(&staging);
        anyhow::anyhow!("copying {} to {} failed: {e}", old.display(), new.display())
    })
}

/// Copy `source` into `install`'s `Interface/AddOns/<name>`, replacing what's there, the same way
/// (staged beside it and swapped in). Returns the installed folder.
pub fn install_named(source: &Path, install: &Install, name: &str) -> Result<PathBuf> {
    let addons = install.path.join("Interface").join("AddOns");
    std::fs::create_dir_all(&addons).with_context(|| format!("creating {}", addons.display()))?;
    let target = addons.join(name);
    let staging = addons.join(format!(".{name}.installing"));
    let old = addons.join(format!(".{name}.old"));
    // An earlier install that couldn't put the previous addon back left it as `.<name>.old` with no
    // `<name>`: that copy is the addon now (its Data/ too), so it goes back before anything else.
    if !target.exists() && old.is_dir() {
        std::fs::rename(&old, &target).with_context(|| {
            format!(
                "{} is the previous addon, left by an install that failed; rename it to {name} by hand",
                old.display()
            )
        })?;
    }
    for leftover in [&staging, &old] {
        if leftover.exists() {
            std::fs::remove_dir_all(leftover)
                .with_context(|| format!("removing {}", leftover.display()))?;
        }
    }
    if let Err(e) = copy_dir(source, &staging) {
        let _ = std::fs::remove_dir_all(&staging);
        return Err(e);
    }
    if target.exists() {
        std::fs::rename(&target, &old)
            .with_context(|| format!("moving the old addon aside in {}", addons.display()))?;
    }
    swap_in(&staging, &target, &old, &|from, to| {
        std::fs::rename(from, to)
    })?;
    if old.exists() {
        // The core's Data/ folder is the agent's, not the release's (prices_data): carry it over,
        // so a reinstall doesn't lose the pooled prices until the next sync. The stubs the release
        // ships under Data/ go; the agent's files take their place.
        if name == ADDON_NAME {
            let kept = old.join(crate::prices_data::DIR);
            if kept.is_dir() {
                let into = target.join(crate::prices_data::DIR);
                let _ = std::fs::remove_dir_all(&into);
                if let Err(e) = std::fs::rename(&kept, &into) {
                    // Lost until the next sync rewrites it; the stubs are gone with the old copy.
                    eprintln!("couldn't keep {}: {e}", into.display());
                }
            }
        }
        std::fs::remove_dir_all(&old).with_context(|| format!("removing {}", old.display()))?;
    }
    Ok(target)
}

/// Move `staging` to `target`, where the previous addon (if any) already sits aside as `old`. When
/// that fails, `old` is put back; when that fails too, the error keeps the first failure and says
/// where the previous addon is now and how to restore it, since the folder is then missing.
fn swap_in(
    staging: &Path,
    target: &Path,
    old: &Path,
    rename: &dyn Fn(&Path, &Path) -> std::io::Result<()>,
) -> Result<()> {
    let Err(e) = rename(staging, target) else {
        return Ok(());
    };
    let first = format!("putting the addon in {} failed: {e}", target.display());
    if !old.exists() {
        bail!("{first}");
    }
    match rename(old, target) {
        Ok(()) => bail!("{first}; the previous addon was put back as it was"),
        Err(back) => bail!(
            "{first}; putting the previous addon back failed too ({back}){}: the previous one is \
             at {old} (rename that folder to {name}, in the same folder, to restore it; the next \
             install puts it back first if {name} is still missing)",
            if target.exists() {
                ""
            } else {
                ", so there is no addon folder now"
            },
            name = target
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
            old = old.display(),
        ),
    }
}

/// Remove `install`'s `Interface/AddOns/<name>` (a mod's folder; never the core's own, under either
/// of its names). Returns whether there was one to remove.
pub fn remove_named(install: &Install, name: &str) -> Result<bool> {
    if name == ADDON_NAME
        || name == LEGACY_ADDON_NAME
        || name.is_empty()
        || name.contains(['/', '\\'])
        || name.starts_with('.')
    {
        bail!("not a mod's addon folder: {name:?}");
    }
    let target = addon_dir_named(install, name);
    if !target.is_dir() {
        return Ok(false);
    }
    std::fs::remove_dir_all(&target).with_context(|| format!("removing {}", target.display()))?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn addon(dir: &Path) -> PathBuf {
        let a = dir.join("release/addon/Ryna_Core");
        std::fs::create_dir_all(a.join("Libs")).unwrap();
        std::fs::write(a.join("Ryna_Core.toc"), "## Interface: 11600\nCore.lua\n").unwrap();
        std::fs::write(a.join("Core.lua"), "-- core\n").unwrap();
        std::fs::write(a.join("Libs/Lib.lua"), "-- lib\n").unwrap();
        a
    }

    #[test]
    fn resolves_the_release_layout_and_the_folder_itself() {
        let dir = tempfile::tempdir().unwrap();
        let a = addon(dir.path());
        assert_eq!(resolve_source(&dir.path().join("release")).unwrap(), a);
        assert_eq!(
            resolve_source(&dir.path().join("release/addon")).unwrap(),
            a
        );
        assert_eq!(resolve_source(&a).unwrap(), a);
        assert!(resolve_source(dir.path()).is_err());
    }

    #[test]
    fn reads_the_installed_version() {
        assert_eq!(
            parse_toc_version("## Interface: 16001\n## Title: Ryna\n## Version: 0.1.0\n")
                .as_deref(),
            Some("0.1.0")
        );
        assert_eq!(
            parse_toc_version("\u{feff}##version:  1.2.3 \r\n").as_deref(),
            Some("1.2.3")
        );
        assert_eq!(parse_toc_version("## Version:\n## Title: x"), None);
        assert_eq!(parse_toc_version("Core.lua"), None);

        let dir = tempfile::tempdir().unwrap();
        let inst = Install {
            path: dir.path().join("_forever_"),
            flavor: Some("wowf".into()),
        };
        assert!(!has_addon(&inst));
        assert_eq!(installed_addon_version(&inst), None);
        let a = addon(dir.path());
        std::fs::write(
            a.join("Ryna_Core.toc"),
            "## Interface: 16001\n## Version: 0.1.0\n",
        )
        .unwrap();
        install(&a, &inst).unwrap();
        assert!(has_addon(&inst));
        assert_eq!(installed_addon_version(&inst).as_deref(), Some("0.1.0"));
    }

    #[test]
    fn compares_versions_as_dotted_integers() {
        use std::cmp::Ordering::*;
        assert_eq!(compare_versions("0.1.0", "0.1.0"), Equal);
        assert_eq!(compare_versions("0.1", "0.1.0"), Equal);
        assert_eq!(compare_versions("v1.2.0", "1.2"), Equal);
        assert_eq!(compare_versions("0.1.0", "0.2.0"), Less);
        assert_eq!(
            compare_versions("0.10.0", "0.9.1"),
            Greater,
            "not as strings"
        );
        assert_eq!(compare_versions("1.0.0", "0.99.99"), Greater);
        assert_eq!(compare_versions("0.2-beta", "0.2"), Equal);
        assert_eq!(compare_versions("0.1.1", "0.1"), Greater);
    }

    #[test]
    fn says_how_the_installed_addon_compares() {
        use AddonState::*;
        assert_eq!(addon_state(false, None, Some("0.2.0")), NotInstalled);
        assert_eq!(addon_state(false, None, None), NotInstalled);
        assert_eq!(addon_state(true, Some("0.2.0"), Some("0.2.0")), Current);
        assert_eq!(addon_state(true, Some("0.1.0"), Some("0.2.0")), Older);
        assert_eq!(addon_state(true, Some("0.3.0"), Some("0.2.0")), Newer);
        assert_eq!(addon_state(true, None, Some("0.2.0")), Unknown);
        assert_eq!(addon_state(true, Some("0.2.0"), None), Unknown);
        assert_eq!(serde_json::to_value(NotInstalled).unwrap(), "notInstalled");
    }

    #[test]
    fn only_an_older_installed_addon_is_updated_unasked() {
        let dir = tempfile::tempdir().unwrap();
        let a = addon(dir.path());
        let inst = Install {
            path: dir.path().join("_forever_"),
            flavor: Some("wowf".into()),
        };
        // Not installed: installing is the person's to choose.
        assert!(!needs_update(&inst, Some("0.4.2")));
        // Installed with no version: nothing to compare.
        install(&a, &inst).unwrap();
        assert!(!needs_update(&inst, Some("0.4.2")));
        std::fs::write(a.join("Ryna_Core.toc"), "## Version: 0.3.6\n").unwrap();
        install(&a, &inst).unwrap();
        assert!(needs_update(&inst, Some("0.4.2")));
        assert!(needs_update(&inst, Some("0.10.0")));
        assert!(!needs_update(&inst, Some("0.3.6")));
        // A newer copy (installed from elsewhere) is left alone, and so is a build with no addon.
        assert!(!needs_update(&inst, Some("0.3.5")));
        assert!(!needs_update(&inst, None));
    }

    #[test]
    fn reads_the_bundled_version() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(bundled_addon_version(dir.path()), None, "no addon there");
        let a = addon(dir.path());
        assert_eq!(
            bundled_addon_version(&dir.path().join("release")),
            None,
            "a toc with no version"
        );
        std::fs::write(
            a.join("Ryna_Core.toc"),
            "## Interface: 16001\n## Version: 0.2.0\nCore.lua\n",
        )
        .unwrap();
        for from in ["release", "release/addon", "release/addon/Ryna_Core"] {
            assert_eq!(
                bundled_addon_version(&dir.path().join(from)).as_deref(),
                Some("0.2.0"),
                "{from}"
            );
        }
    }

    #[test]
    fn a_mod_folder_installs_beside_the_core_and_comes_out_again() {
        let dir = tempfile::tempdir().unwrap();
        let a = addon(dir.path());
        let m = dir.path().join("release/addon/Ryna_Chronicle");
        std::fs::create_dir_all(m.join("Locales")).unwrap();
        std::fs::write(m.join("Ryna_Chronicle.toc"), "## Version: 0.3.0\n").unwrap();
        std::fs::write(m.join("Locales/enUS.lua"), "-- strings\n").unwrap();
        let inst = Install {
            path: dir.path().join("_forever_"),
            flavor: Some("wowf".into()),
        };
        install(&a, &inst).unwrap();
        assert_eq!(installed_folder_version(&inst, "Ryna_Chronicle"), None);
        let to = install_named(&m, &inst, "Ryna_Chronicle").unwrap();
        assert_eq!(to, addon_dir_named(&inst, "Ryna_Chronicle"));
        assert!(to.join("Locales/enUS.lua").is_file());
        assert_eq!(
            installed_folder_version(&inst, "Ryna_Chronicle").as_deref(),
            Some("0.3.0")
        );
        assert!(remove_named(&inst, "Ryna_Chronicle").unwrap());
        assert!(!to.exists());
        assert!(has_addon(&inst), "the core's folder is left alone");
        assert!(
            !remove_named(&inst, "Ryna_Chronicle").unwrap(),
            "nothing left to remove"
        );
        for refused in ["Ryna_Core", "Ryna", "", "../Ryna", ".Ryna.old"] {
            assert!(remove_named(&inst, refused).is_err(), "{refused}");
        }
        assert!(has_addon(&inst));
    }

    #[test]
    fn a_failed_swap_says_where_the_previous_addon_is() {
        let dir = tempfile::tempdir().unwrap();
        let addons = dir.path();
        let staging = addons.join(".Ryna_Core.installing");
        let target = addons.join("Ryna_Core");
        let old = addons.join(".Ryna_Core.old");
        std::fs::create_dir_all(&staging).unwrap();
        std::fs::create_dir_all(&old).unwrap();
        let refuse = |_: &Path, _: &Path| -> std::io::Result<()> {
            Err(std::io::Error::other("disk says no"))
        };
        // Both renames fail: the first error is kept, and the way back is spelled out.
        let e = swap_in(&staging, &target, &old, &refuse)
            .unwrap_err()
            .to_string();
        assert!(e.starts_with("putting the addon in "), "{e}");
        assert!(e.contains("failed: disk says no"), "{e}");
        assert!(
            e.contains("putting the previous addon back failed too"),
            "{e}"
        );
        assert!(e.contains(&old.display().to_string()), "{e}");
        assert!(e.contains("rename that folder to Ryna"), "{e}");
        // Only the first fails: the previous addon is back, and the error says so.
        let calls = std::cell::Cell::new(0);
        let once = |from: &Path, to: &Path| {
            calls.set(calls.get() + 1);
            if calls.get() == 1 {
                Err(std::io::Error::other("disk says no"))
            } else {
                std::fs::rename(from, to)
            }
        };
        let e = swap_in(&staging, &target, &old, &once)
            .unwrap_err()
            .to_string();
        assert!(e.contains("put back as it was"), "{e}");
        assert!(target.is_dir() && !old.exists());
        // Nothing aside (a first install): just the first failure.
        let e = swap_in(
            &staging,
            &addons.join("New"),
            &addons.join(".New.old"),
            &refuse,
        )
        .unwrap_err()
        .to_string();
        assert!(e.ends_with("failed: disk says no"), "{e}");
    }

    #[test]
    fn a_stranded_previous_addon_is_put_back_before_the_next_install() {
        // A failed rollback left `.Ryna_Core.old` (with the agent's Data/) and no `Ryna_Core`.
        let dir = tempfile::tempdir().unwrap();
        let src = addon(dir.path());
        let install = Install {
            path: dir.path().join("_forever_"),
            flavor: Some("wowf".into()),
        };
        let addons = install.path.join("Interface/AddOns");
        let stranded = addons.join(".Ryna_Core.old");
        std::fs::create_dir_all(stranded.join(crate::prices_data::DIR)).unwrap();
        std::fs::write(stranded.join(crate::prices_data::DIR).join("kept.lua"), "x").unwrap();
        let target = install_named(&src, &install, ADDON_NAME).unwrap();
        assert!(target.join("Core.lua").is_file());
        assert_eq!(
            std::fs::read_to_string(target.join(crate::prices_data::DIR).join("kept.lua")).unwrap(),
            "x"
        );
        assert!(!stranded.exists());
    }

    /// An install as 0.4.4 left it: the folder `Ryna` (with the agent's Data/) and two accounts, one
    /// with a saved file.
    fn legacy_install(dir: &Path) -> Install {
        let install = Install {
            path: dir.join("_forever_"),
            flavor: Some("wowf".into()),
        };
        let old = install.path.join("Interface/AddOns/Ryna");
        std::fs::create_dir_all(old.join("Data")).unwrap();
        std::fs::write(old.join("Ryna.toc"), "## Version: 0.4.4\n").unwrap();
        std::fs::write(old.join("Data/vendors.lua"), "RynaData_vendors = {}").unwrap();
        let sv = install.path.join("WTF/Account/ONE/SavedVariables");
        std::fs::create_dir_all(&sv).unwrap();
        std::fs::write(sv.join("Ryna.lua"), "RynaDB = { old = true }\n").unwrap();
        std::fs::create_dir_all(install.path.join("WTF/Account/TWO/SavedVariables")).unwrap();
        install
    }

    fn set_mtime(path: &Path, secs: u64) {
        let t = std::time::UNIX_EPOCH + std::time::Duration::from_secs(secs);
        std::fs::File::options()
            .write(true)
            .open(path)
            .unwrap()
            .set_modified(t)
            .unwrap();
    }

    fn mtime(path: &Path) -> std::time::SystemTime {
        std::fs::metadata(path).unwrap().modified().unwrap()
    }

    #[test]
    fn an_install_with_the_old_folder_has_the_addon_and_needs_the_update() {
        let dir = tempfile::tempdir().unwrap();
        let install = legacy_install(dir.path());
        assert!(has_addon(&install) && has_legacy_addon(&install));
        assert_eq!(addon_dir(&install), legacy_addon_dir(&install));
        assert_eq!(installed_addon_version(&install).as_deref(), Some("0.4.4"));
        // Whatever the versions say, even the same one: the old folder is always behind.
        assert!(needs_update(&install, Some("0.5.0")));
        assert!(needs_update(&install, Some("0.4.4")));
        assert!(
            !needs_update(&install, None),
            "a build with no addon to give"
        );
    }

    #[test]
    fn installing_moves_the_old_folder_over_with_its_saved_files_and_data() {
        let dir = tempfile::tempdir().unwrap();
        let src = addon(dir.path());
        let install = legacy_install(dir.path());
        let sv = install.path.join("WTF/Account/ONE/SavedVariables");
        set_mtime(&sv.join("Ryna.lua"), 1_700_000_000);

        let target = install_checked(&src, &install, &|| false).unwrap();
        assert_eq!(target, install.path.join("Interface/AddOns/Ryna_Core"));
        assert!(target.join("Ryna_Core.toc").is_file());
        assert!(!has_legacy_addon(&install), "the old folder is gone");
        assert_eq!(addon_dir(&install), target);
        // The agent's tables came across.
        assert_eq!(
            std::fs::read_to_string(target.join("Data/vendors.lua")).unwrap(),
            "RynaData_vendors = {}"
        );
        // The saved file is under the new folder's name, as it was saved and when it was saved; the
        // old one is left for someone who goes back a version.
        assert_eq!(
            std::fs::read_to_string(sv.join("Ryna_Core.lua")).unwrap(),
            "RynaDB = { old = true }\n"
        );
        assert_eq!(
            mtime(&sv.join("Ryna_Core.lua")),
            mtime(&sv.join("Ryna.lua"))
        );
        assert!(sv.join("Ryna.lua").is_file());
        // An account with no saved file gets none, and nothing is left half-written.
        let two = install.path.join("WTF/Account/TWO/SavedVariables");
        assert_eq!(std::fs::read_dir(&two).unwrap().count(), 0);
        assert_eq!(std::fs::read_dir(&sv).unwrap().count(), 2);
        assert!(
            !needs_update(&install, Some("0.4.4")),
            "moved: no longer behind"
        );
    }

    #[test]
    fn the_move_waits_for_the_game_to_close() {
        let dir = tempfile::tempdir().unwrap();
        let src = addon(dir.path());
        let install = legacy_install(dir.path());
        let e = install_checked(&src, &install, &|| true)
            .unwrap_err()
            .to_string();
        assert!(e.contains("close the game first"), "{e}");
        // Nothing was touched.
        assert!(has_legacy_addon(&install));
        assert!(!install.path.join("Interface/AddOns/Ryna_Core").exists());
        assert!(!install
            .path
            .join("WTF/Account/ONE/SavedVariables/Ryna_Core.lua")
            .exists());
        // With no old folder a running game doesn't stop an install (it needs a /reload, as before).
        let fresh = Install {
            path: dir.path().join("_fresh_"),
            flavor: Some("wowf".into()),
        };
        assert!(install_checked(&src, &fresh, &|| true).is_ok());
    }

    #[test]
    fn a_saved_file_the_game_has_written_since_is_never_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        let src = addon(dir.path());
        let install = legacy_install(dir.path());
        let sv = install.path.join("WTF/Account/ONE/SavedVariables");
        // Both folders were there for a while (unpacked by hand): the new one's file is the newer.
        std::fs::create_dir_all(install.path.join("Interface/AddOns/Ryna_Core/Data")).unwrap();
        std::fs::write(
            install
                .path
                .join("Interface/AddOns/Ryna_Core/Data/vendors.lua"),
            "newer tables",
        )
        .unwrap();
        std::fs::write(sv.join("Ryna_Core.lua"), "RynaDB = { new = true }\n").unwrap();
        set_mtime(&sv.join("Ryna.lua"), 1_700_000_000);
        set_mtime(&sv.join("Ryna_Core.lua"), 1_700_000_500);
        let target = install_checked(&src, &install, &|| false).unwrap();
        assert_eq!(
            std::fs::read_to_string(sv.join("Ryna_Core.lua")).unwrap(),
            "RynaDB = { new = true }\n"
        );
        // And the new folder's own tables are the ones kept.
        assert_eq!(
            std::fs::read_to_string(target.join("Data/vendors.lua")).unwrap(),
            "newer tables"
        );
        assert!(!has_legacy_addon(&install));

        // The other way round (the old addon was the one still running): the old file is the truth.
        let again = legacy_install(&dir.path().join("again"));
        let sv = again.path.join("WTF/Account/ONE/SavedVariables");
        std::fs::write(sv.join("Ryna_Core.lua"), "RynaDB = { stale = true }\n").unwrap();
        set_mtime(&sv.join("Ryna_Core.lua"), 1_700_000_000);
        set_mtime(&sv.join("Ryna.lua"), 1_700_000_500);
        install_checked(&src, &again, &|| false).unwrap();
        assert_eq!(
            std::fs::read_to_string(sv.join("Ryna_Core.lua")).unwrap(),
            "RynaDB = { old = true }\n"
        );
    }

    #[test]
    fn installs_and_replaces() {
        let dir = tempfile::tempdir().unwrap();
        let a = addon(dir.path());
        let product = dir.path().join("_forever_");
        std::fs::create_dir_all(product.join("Interface/AddOns/Ryna_Core")).unwrap();
        std::fs::write(product.join("Interface/AddOns/Ryna_Core/Stale.lua"), "old").unwrap();
        // The agent's price tables (prices_data) live under the core's Data/: kept across a reinstall.
        std::fs::create_dir_all(product.join("Interface/AddOns/Ryna_Core/Data")).unwrap();
        std::fs::write(
            product.join("Interface/AddOns/Ryna_Core/Data/vendors.lua"),
            "RynaData_vendors = { version = 1 }",
        )
        .unwrap();
        std::fs::create_dir_all(a.join("Data")).unwrap();
        std::fs::write(a.join("Data/vendors.lua"), "-- stub\n").unwrap();
        std::fs::create_dir_all(product.join("Interface/AddOns/Other")).unwrap();
        let inst = Install {
            path: product.clone(),
            flavor: Some("wowf".into()),
        };

        let target = install(&a, &inst).unwrap();
        assert_eq!(target, product.join("Interface/AddOns/Ryna_Core"));
        assert_eq!(
            std::fs::read_to_string(target.join("Libs/Lib.lua")).unwrap(),
            "-- lib\n"
        );
        assert!(target.join("Ryna_Core.toc").is_file());
        assert!(
            !target.join("Stale.lua").exists(),
            "the old copy is replaced, not merged"
        );
        assert_eq!(
            std::fs::read_to_string(target.join("Data/vendors.lua")).unwrap(),
            "RynaData_vendors = { version = 1 }",
            "the agent's Data/ tables survive a reinstall, over the release's stubs"
        );
        assert!(
            product.join("Interface/AddOns/Other").is_dir(),
            "other addons are left alone"
        );
        let names: Vec<String> = std::fs::read_dir(product.join("Interface/AddOns"))
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names.len(), 2, "{names:?}");

        // A fresh install with no Interface folder yet.
        let fresh = Install {
            path: dir.path().join("_forever_ptr_"),
            flavor: None,
        };
        assert!(install(&a, &fresh).unwrap().join("Core.lua").is_file());
    }
}
