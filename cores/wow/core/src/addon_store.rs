//! Fetched addons on disk (docs/ryna-agent-cores-mods.md 6.2): reading the signed index, taking
//! the zips it names, and laying out the folder the installer copies from.
//!
//! Under `<state>/addons/`:
//! - `index.json`: the last serial taken, and the entry of each addon held in `fetched/`.
//! - `fetched/<Folder>/`: one version of an addon, unpacked whole and checked before it lands.
//! - `current/`: every addon's chosen copy side by side (`current/Ryna_Core`, `current/Ryna_Meter`),
//!   the same layout as the app's own resources, built only while something is fetched.
//!
//! [`refresh`] is the one call that touches the network. [`source_dir`] is what a front end
//! installs from: the app's own copy when nothing is held, else `current/Ryna_Core`. It decides
//! again from what is held every time ([`crate::addon_index::choose`]), so an app that now carries
//! something newer than a held copy stops using the held one without being told.

use std::collections::BTreeMap;
use std::io::{Cursor, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::Mutex;

use anyhow::{anyhow, bail, Context, Result};
use ryna_agent::package::{self, Source};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::addon_index::{self, Carried, Entry, Index, Known, Pick, INDEX_FILE};
use crate::addon_install::{self, ADDON_NAME};
use crate::mods;

/// The most an addon's zip may unpack to, and the most files it may hold.
const MAX_UNPACKED: u64 = 512 * 1024 * 1024;
const MAX_FILES: usize = 20_000;
/// The core's TOC fields for its API level and the lowest level it serves; a mod's for the level
/// it needs.
pub const TOC_API: &str = "X-Ryna-API";
pub const TOC_API_MIN: &str = "X-Ryna-API-Min";

/// One at a time: two callers must never rebuild `current/` or swap `fetched/` together.
static LOCK: Mutex<()> = Mutex::new(());

/// Where the index is read from: `RYNA_ADDON_INDEX` when set, else the bundle's latest release.
pub fn index_source() -> Result<Source> {
    match std::env::var(addon_index::INDEX_ENV)
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
    {
        Some(v) => Ok(Source::parse(&v)?),
        None => Ok(Source::Url(addon_index::INDEX_URL.to_string())),
    }
}

/// `## <field>: value` in a TOC.
pub fn toc_field(toc: &str, field: &str) -> Option<String> {
    toc.trim_start_matches('\u{feff}').lines().find_map(|line| {
        let (key, value) = line.trim().strip_prefix("##")?.split_once(':')?;
        (key.trim().eq_ignore_ascii_case(field) && !value.trim().is_empty())
            .then(|| value.trim().to_string())
    })
}

fn toc_level(dir: &Path, folder: &str, field: &str) -> Option<u32> {
    let bytes = std::fs::read(dir.join(format!("{folder}.toc"))).ok()?;
    toc_field(&String::from_utf8_lossy(&bytes), field)?
        .parse()
        .ok()
}

/// What the app carries, read from the copies themselves: each folder's version and level, and
/// where it is.
#[derive(Debug, Clone)]
pub struct Carry {
    pub known: Known,
    pub dirs: BTreeMap<String, PathBuf>,
}

/// The carried addons beside `core_source` (the carried `Ryna_Core` folder). None when the core's
/// own TOC doesn't say its version and level.
pub fn carry(core_source: &Path) -> Option<Carry> {
    let core_version = addon_install::folder_version_named(core_source, ADDON_NAME)?;
    let core_api = toc_level(core_source, ADDON_NAME, TOC_API)?;
    let core_api_min = toc_level(core_source, ADDON_NAME, TOC_API_MIN).unwrap_or(core_api);
    let mut dirs = BTreeMap::from([(ADDON_NAME.to_string(), core_source.to_path_buf())]);
    let mut carried = BTreeMap::new();
    for m in mods::bundled().iter().filter(|m| !mods::is_agent_mod(m)) {
        for folder in m.addons.iter().filter(|f| f.as_str() != ADDON_NAME) {
            let Some(dir) = mods::addon_source(core_source, m, folder) else {
                continue;
            };
            let Some(version) = addon_install::folder_version_named(&dir, folder) else {
                continue;
            };
            let api = toc_level(&dir, folder, TOC_API)
                .or(m.requires.api)
                .unwrap_or(core_api);
            carried.insert(folder.clone(), Carried { version, api });
            dirs.insert(folder.clone(), dir);
        }
    }
    Some(Carry {
        known: Known {
            core_folder: ADDON_NAME.to_string(),
            core_version,
            core_api,
            core_api_min,
            mods: carried,
        },
        dirs,
    })
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Record {
    #[serde(default)]
    serial: u64,
    /// By folder: the entry whose zip is unpacked in `fetched/<folder>`.
    #[serde(default)]
    fetched: BTreeMap<String, Entry>,
}

fn root(state: &Path) -> PathBuf {
    state.join("addons")
}

fn fetched_dir(state: &Path, folder: &str) -> PathBuf {
    root(state).join("fetched").join(folder)
}

fn load_record(state: &Path) -> Record {
    std::fs::read(root(state).join("index.json"))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

fn save_record(state: &Path, record: &Record) -> Result<()> {
    let dir = root(state);
    std::fs::create_dir_all(&dir)?;
    let tmp = dir.join("index.json.new");
    std::fs::write(&tmp, serde_json::to_vec_pretty(record)?)?;
    std::fs::rename(&tmp, dir.join("index.json")).context("saving the addon index's record")?;
    Ok(())
}

/// Whether `fetched/<folder>` holds exactly `e`.
fn held(state: &Path, record: &Record, e: &Entry) -> bool {
    record.fetched.get(&e.folder) == Some(e)
        && addon_install::folder_version_named(&fetched_dir(state, &e.folder), &e.folder).as_deref()
            == Some(e.version.as_str())
}

/// What to install from, decided from what is held alone (no network).
fn held_picks(state: &Path, known: &Known, app_version: &str) -> BTreeMap<String, Pick> {
    let record = load_record(state);
    let entries = record
        .fetched
        .values()
        .filter(|e| held(state, &record, e))
        .cloned()
        .collect();
    let index = Index {
        serial: record.serial,
        entries,
        dropped: Vec::new(),
    };
    addon_index::choose(&index, known, app_version)
}

/// The key `current/` was built for: the app, and each folder's chosen copy.
fn set_key(carry: &Carry, picks: &BTreeMap<String, Pick>, app_version: &str) -> String {
    let mut key = format!("app {app_version}\n");
    for (folder, pick) in picks {
        let line = match pick {
            Pick::Fetched(e) => format!("{folder} fetched {} {}\n", e.version, e.sha256),
            Pick::Carried => {
                let version = if folder == ADDON_NAME {
                    carry.known.core_version.as_str()
                } else {
                    carry.known.mods.get(folder).map_or("", |c| &c.version)
                };
                format!("{folder} carried {version}\n")
            }
        };
        key.push_str(&line);
    }
    key
}

/// Lay out `current/` for `picks` unless it already is, and return its core folder. With nothing
/// fetched there is nothing to lay out: the carried core folder is the answer.
fn assemble(
    state: &Path,
    carry: &Carry,
    picks: &BTreeMap<String, Pick>,
    app_version: &str,
) -> Result<PathBuf> {
    let carried_core = carry.dirs[ADDON_NAME].clone();
    let current = root(state).join("current");
    if !picks.values().any(|p| matches!(p, Pick::Fetched(_))) {
        let _ = std::fs::remove_dir_all(&current);
        return Ok(carried_core);
    }
    let key = set_key(carry, picks, app_version);
    let core = current.join(ADDON_NAME);
    let built = std::fs::read_to_string(current.join(".set")).ok();
    if built.as_deref() == Some(key.as_str()) && core.join(format!("{ADDON_NAME}.toc")).is_file() {
        return Ok(core);
    }
    let new = root(state).join("current.new");
    let _ = std::fs::remove_dir_all(&new);
    for (folder, pick) in picks {
        let from = match pick {
            Pick::Fetched(_) => fetched_dir(state, folder),
            Pick::Carried => carry
                .dirs
                .get(folder)
                .cloned()
                .ok_or_else(|| anyhow!("this build doesn't carry {folder}"))?,
        };
        addon_install::copy_dir(&from, &new.join(folder))?;
    }
    std::fs::write(new.join(".set"), &key)?;
    let _ = std::fs::remove_dir_all(&current);
    std::fs::rename(&new, &current).context("laying out the addons to install")?;
    Ok(core)
}

/// The folder to install the core's addon from, its mods beside it: the app's own copy
/// (`core_source`) when nothing newer is held, else `<state>/addons/current/Ryna_Core`. Any
/// trouble reading what is held is the app's own copy.
pub fn source_dir(state: &Path, core_source: &Path, app_version: &str) -> PathBuf {
    let Some(carry) = carry(core_source) else {
        return core_source.to_path_buf();
    };
    let _one = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let picks = held_picks(state, &carry.known, app_version);
    assemble(state, &carry, &picks, app_version).unwrap_or_else(|_| core_source.to_path_buf())
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// The zip an entry names: from its URL, or by its file name from a folder source (development).
fn get_zip(source: &Source, e: &Entry) -> Result<Vec<u8>> {
    let bytes = match source {
        Source::Folder(dir) => {
            let name = e.url.rsplit('/').next().unwrap_or_default();
            std::fs::read(dir.join(name)).with_context(|| format!("reading {name}"))?
        }
        Source::Url(_) => ryna_agent::http::download(&e.url, e.size, &mut |_, _| {})
            .map_err(|why| anyhow!("couldn't download {}: {why}", e.url))?,
    };
    if bytes.len() as u64 != e.size || sha256_hex(&bytes) != e.sha256 {
        bail!(
            "{} {} isn't the file the index names; nothing was installed",
            e.folder,
            e.version
        );
    }
    Ok(bytes)
}

/// Unpack an addon's zip into `dest` (which becomes the addon's folder). The zip must hold one top
/// folder, `folder`, and nothing that would land outside it.
fn unpack_zip(bytes: &[u8], folder: &str, dest: &Path) -> Result<()> {
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).context("it isn't a zip")?;
    if zip.len() > MAX_FILES {
        bail!("it holds too many files");
    }
    let mut total = 0u64;
    for i in 0..zip.len() {
        let mut file = zip.by_index(i)?;
        let shown = file
            .name()
            .map(|n| n.into_owned())
            .unwrap_or_else(|_| "a name that can't be read".into());
        let outside = || anyhow!("it holds {shown}, which isn't in {folder}/");
        let name = file.enclosed_name().ok_or_else(outside)?;
        let mut parts = name.components();
        if parts.next() != Some(Component::Normal(folder.as_ref()))
            || !parts.clone().all(|c| matches!(c, Component::Normal(_)))
        {
            return Err(outside());
        }
        if file.unix_mode().is_some_and(|m| m & 0o170000 == 0o120000) {
            bail!("it holds a link ({shown})");
        }
        let to = dest.join(parts.as_path());
        if file.is_dir() {
            std::fs::create_dir_all(&to)?;
            continue;
        }
        if to == dest {
            return Err(outside());
        }
        if let Some(parent) = to.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut out = std::fs::File::create(&to)?;
        let left = MAX_UNPACKED - total;
        let wrote = std::io::copy(&mut file.by_ref().take(left + 1), &mut out)?;
        total += wrote;
        if total > MAX_UNPACKED {
            bail!("it unpacks to more than {MAX_UNPACKED} bytes");
        }
        out.flush()?;
    }
    Ok(())
}

/// The unpacked addon in `dir` says what its entry says: its version and its levels.
fn check_unpacked(dir: &Path, e: &Entry) -> Result<()> {
    let version = addon_install::folder_version_named(dir, &e.folder);
    if version.as_deref() != Some(e.version.as_str()) {
        bail!(
            "its {}.toc says version {}, not {}",
            e.folder,
            version.as_deref().unwrap_or("nothing"),
            e.version
        );
    }
    if toc_level(dir, &e.folder, TOC_API) != Some(e.api) {
        bail!("its {}.toc doesn't say API level {}", e.folder, e.api);
    }
    if e.folder == ADDON_NAME && toc_level(dir, &e.folder, TOC_API_MIN) != e.api_min {
        bail!(
            "its {}.toc doesn't say the lowest level the index does",
            e.folder
        );
    }
    Ok(())
}

/// What [`refresh`] came to.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Refreshed {
    /// Each addon it downloaded just now: folder and version.
    pub fetched: Vec<(String, String)>,
    /// Each held addon it stopped holding (the app carries as new or newer, or the index moved on).
    pub released: Vec<String>,
    /// Entries of the index that weren't usable, and why.
    pub dropped: Vec<String>,
}

impl Refreshed {
    /// Whether what [`source_dir`] gives may have changed.
    pub fn changed(&self) -> bool {
        !self.fetched.is_empty() || !self.released.is_empty()
    }
}

/// Read the index from `source`, check it against `pubkey`, and bring `fetched/` in step with what
/// it picks for this app. All of it or none: every zip is downloaded, checked and unpacked before
/// the first one replaces what was held, and any failure leaves what was held as it was.
pub fn refresh(
    state: &Path,
    source: &Source,
    pubkey: &str,
    core_source: &Path,
    app_version: &str,
) -> Result<Refreshed> {
    let carry = carry(core_source).context("this build's addon doesn't say its version")?;
    let mut quiet = |_: u64, _: Option<u64>| {};
    let sig = source.get(&format!("{INDEX_FILE}.sig"), &mut quiet)?;
    let sig = String::from_utf8(sig).map_err(|_| anyhow!("the index's .sig isn't text"))?;
    let bytes = source.get(INDEX_FILE, &mut quiet)?;
    package::verify_file(&bytes, &sig, pubkey, INDEX_FILE)?;
    let index = addon_index::parse(&bytes)?;

    let _one = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut record = load_record(state);
    if index.serial < record.serial {
        bail!(
            "the addon index is older (serial {}) than the last one taken ({}); nothing was changed",
            index.serial,
            record.serial
        );
    }
    let picks = addon_index::choose(&index, &carry.known, app_version);
    let staging = root(state).join("staging");
    let _ = std::fs::remove_dir_all(&staging);
    let staged = (|| -> Result<Vec<&Entry>> {
        let mut staged = Vec::new();
        for pick in picks.values() {
            let Pick::Fetched(e) = pick else { continue };
            if held(state, &record, e) {
                continue;
            }
            let zip = get_zip(source, e)?;
            let to = staging.join(&e.folder);
            unpack_zip(&zip, &e.folder, &to)
                .and_then(|()| check_unpacked(&to, e))
                .with_context(|| format!("{} {} was refused", e.folder, e.version))?;
            staged.push(e);
        }
        Ok(staged)
    })()
    .inspect_err(|_| {
        let _ = std::fs::remove_dir_all(&staging);
    })?;

    let mut out = Refreshed {
        dropped: index.dropped.clone(),
        ..Default::default()
    };
    for e in staged {
        let to = fetched_dir(state, &e.folder);
        let _ = std::fs::remove_dir_all(&to);
        if let Some(parent) = to.parent() {
            std::fs::create_dir_all(parent)?;
        }
        // The record forgets the folder first, so a failed swap is fetched again, never trusted.
        record.fetched.remove(&e.folder);
        std::fs::rename(staging.join(&e.folder), &to)
            .with_context(|| format!("keeping {}", e.folder))?;
        record.fetched.insert(e.folder.clone(), e.clone());
        out.fetched.push((e.folder.clone(), e.version.clone()));
    }
    let _ = std::fs::remove_dir_all(&staging);
    let unpicked: Vec<String> = record
        .fetched
        .keys()
        .filter(|f| !matches!(picks.get(*f), Some(Pick::Fetched(_))))
        .cloned()
        .collect();
    for folder in unpicked {
        record.fetched.remove(&folder);
        let _ = std::fs::remove_dir_all(fetched_dir(state, &folder));
        out.released.push(folder);
    }
    record.serial = index.serial;
    save_record(state, &record)?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ryna_agent::package::testing::TestKey;

    const BASE: &str = "https://github.com/Ryna-Mods/x/releases/download/v1";

    fn toc(folder: &str, version: &str, api: u32) -> String {
        let min = if folder == ADDON_NAME {
            format!("## {TOC_API_MIN}: {api}\n")
        } else {
            String::new()
        };
        format!("## Title: {folder}\n## Version: {version}\n## {TOC_API}: {api}\n{min}Main.lua\n")
    }

    /// The addons an app carries: `<dir>/addon/Ryna_Core` and `Ryna_Meter` beside it.
    fn carried(dir: &Path, core: &str, meter: &str) -> PathBuf {
        for (folder, version) in [(ADDON_NAME, core), ("Ryna_Meter", meter)] {
            let d = dir.join("addon").join(folder);
            std::fs::create_dir_all(&d).unwrap();
            std::fs::write(d.join(format!("{folder}.toc")), toc(folder, version, 2)).unwrap();
            std::fs::write(d.join("Main.lua"), format!("-- carried {version}")).unwrap();
        }
        dir.join("addon").join(ADDON_NAME)
    }

    fn zip_of(files: &[(&str, &str)]) -> Vec<u8> {
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default();
        for (name, text) in files {
            zip.start_file(*name, options).unwrap();
            zip.write_all(text.as_bytes()).unwrap();
        }
        zip.finish().unwrap().into_inner()
    }

    fn addon_zip(folder: &str, version: &str, api: u32) -> Vec<u8> {
        zip_of(&[
            (
                &format!("{folder}/{folder}.toc"),
                &toc(folder, version, api),
            ),
            (
                &format!("{folder}/Main.lua"),
                &format!("-- fetched {version}"),
            ),
            (&format!("{folder}/Locales/enUS.lua"), "-- words"),
        ])
    }

    /// A release folder: the zips, and an index of them signed by `key`.
    struct Release {
        dir: PathBuf,
        key: TestKey,
    }

    impl Release {
        fn new(dir: &Path) -> Self {
            let dir = dir.join("release");
            std::fs::create_dir_all(&dir).unwrap();
            Release {
                dir,
                key: TestKey::new(),
            }
        }

        fn entry(&self, folder: &str, version: &str, api: u32, zip: &[u8]) -> serde_json::Value {
            let name = format!("{folder}-{version}.zip");
            std::fs::write(self.dir.join(&name), zip).unwrap();
            serde_json::json!({
                "folder": folder, "version": version, "api": api,
                "apiMin": (folder == ADDON_NAME).then_some(api),
                "url": format!("{BASE}/{name}"),
                "sha256": sha256_hex(zip), "size": zip.len(),
            })
        }

        fn publish(&self, serial: u64, addons: Vec<serde_json::Value>) {
            let json = serde_json::json!({ "schema": 1, "serial": serial, "addons": addons });
            let bytes = serde_json::to_vec(&json).unwrap();
            let sig = self
                .key
                .sign(&bytes, &format!("timestamp:1\tfile:{INDEX_FILE}"));
            std::fs::write(self.dir.join(INDEX_FILE), &bytes).unwrap();
            std::fs::write(self.dir.join(format!("{INDEX_FILE}.sig")), sig).unwrap();
        }

        fn refresh(&self, state: &Path, core: &Path) -> Result<Refreshed> {
            let source = Source::Folder(self.dir.clone());
            refresh(state, &source, &self.key.public(), core, "0.5.0")
        }
    }

    fn version_at(core_dir: &Path, folder: &str) -> Option<String> {
        addon_install::folder_version_named(&core_dir.parent().unwrap().join(folder), folder)
    }

    #[test]
    fn a_newer_mod_is_fetched_and_installed_from_beside_the_carried_core() {
        let tmp = tempfile::tempdir().unwrap();
        let state = tmp.path().join("state");
        let core = carried(tmp.path(), "0.5.0", "0.5.0");
        assert_eq!(source_dir(&state, &core, "0.5.0"), core, "nothing held");

        let r = Release::new(tmp.path());
        let meter = r.entry(
            "Ryna_Meter",
            "1.0.1",
            2,
            &addon_zip("Ryna_Meter", "1.0.1", 2),
        );
        r.publish(1, vec![meter.clone()]);
        let done = r.refresh(&state, &core).unwrap();
        assert_eq!(done.fetched, [("Ryna_Meter".into(), "1.0.1".into())]);
        assert!(done.changed());

        let from = source_dir(&state, &core, "0.5.0");
        assert_eq!(from, state.join("addons/current").join(ADDON_NAME));
        assert_eq!(version_at(&from, ADDON_NAME).as_deref(), Some("0.5.0"));
        assert_eq!(version_at(&from, "Ryna_Meter").as_deref(), Some("1.0.1"));
        let lua = from.parent().unwrap().join("Ryna_Meter/Locales/enUS.lua");
        assert!(lua.is_file(), "the zip's folders came too");

        // The same index again downloads nothing; a later one with the zip gone still holds it.
        std::fs::remove_file(r.dir.join("Ryna_Meter-1.0.1.zip")).unwrap();
        r.publish(2, vec![meter]);
        let again = r.refresh(&state, &core).unwrap();
        assert!(!again.changed(), "{again:?}");
    }

    #[test]
    fn an_app_that_now_carries_newer_stops_using_the_held_copy() {
        let tmp = tempfile::tempdir().unwrap();
        let state = tmp.path().join("state");
        let core = carried(tmp.path(), "0.5.0", "0.5.0");
        let r = Release::new(tmp.path());
        let meter = r.entry(
            "Ryna_Meter",
            "1.0.1",
            2,
            &addon_zip("Ryna_Meter", "1.0.1", 2),
        );
        r.publish(1, vec![meter]);
        r.refresh(&state, &core).unwrap();
        assert_ne!(source_dir(&state, &core, "0.5.0"), core);

        // The next app carries Meter 1.0.2: with no network at all, it installs its own.
        let core = carried(tmp.path(), "0.5.0", "1.0.2");
        assert_eq!(source_dir(&state, &core, "0.6.0"), core);
        assert!(!state.join("addons/current").exists());
        // And the next refresh lets the held copy go.
        let done = r.refresh(&state, &core).unwrap();
        assert_eq!(done.released, ["Ryna_Meter"]);
        assert!(!state.join("addons/fetched/Ryna_Meter").exists());
    }

    #[test]
    fn a_core_and_the_mod_that_needs_it_arrive_together_or_not_at_all() {
        let tmp = tempfile::tempdir().unwrap();
        let state = tmp.path().join("state");
        let core = carried(tmp.path(), "0.5.0", "0.5.0");
        let r = Release::new(tmp.path());
        // Core 0.6.0 offers level 3 and still serves 2; Meter 2.0.0 needs 3; its zip is damaged.
        let core_zip = zip_of(&[
            (
                "Ryna_Core/Ryna_Core.toc",
                "## Version: 0.6.0\n## X-Ryna-API: 3\n## X-Ryna-API-Min: 2\n",
            ),
            ("Ryna_Core/Core.lua", "-- core"),
        ]);
        let mut core_entry = r.entry(ADDON_NAME, "0.6.0", 3, &core_zip);
        core_entry["apiMin"] = 2.into();
        let meter = r.entry(
            "Ryna_Meter",
            "2.0.0",
            3,
            &addon_zip("Ryna_Meter", "2.0.0", 3),
        );
        std::fs::write(r.dir.join("Ryna_Meter-2.0.0.zip"), b"not the zip").unwrap();
        r.publish(1, vec![core_entry.clone(), meter]);
        let err = r.refresh(&state, &core).unwrap_err();
        assert!(format!("{err:#}").contains("isn't the file the index names"));
        assert_eq!(
            source_dir(&state, &core, "0.5.0"),
            core,
            "the core didn't land alone"
        );
        assert!(!state.join("addons/fetched").join(ADDON_NAME).exists());

        // Republished whole, both land.
        let meter = r.entry(
            "Ryna_Meter",
            "2.0.0",
            3,
            &addon_zip("Ryna_Meter", "2.0.0", 3),
        );
        r.publish(2, vec![core_entry, meter]);
        assert_eq!(r.refresh(&state, &core).unwrap().fetched.len(), 2);
        let from = source_dir(&state, &core, "0.5.0");
        assert_eq!(version_at(&from, ADDON_NAME).as_deref(), Some("0.6.0"));
        assert_eq!(version_at(&from, "Ryna_Meter").as_deref(), Some("2.0.0"));
    }

    #[test]
    fn an_index_nobody_signed_or_an_older_one_changes_nothing() {
        let tmp = tempfile::tempdir().unwrap();
        let state = tmp.path().join("state");
        let core = carried(tmp.path(), "0.5.0", "0.5.0");
        let r = Release::new(tmp.path());
        let meter = r.entry(
            "Ryna_Meter",
            "1.0.1",
            2,
            &addon_zip("Ryna_Meter", "1.0.1", 2),
        );
        r.publish(5, vec![meter.clone()]);

        // Another key's signature.
        let other = TestKey::new();
        let source = Source::Folder(r.dir.clone());
        let err = refresh(&state, &source, &other.public(), &core, "0.5.0").unwrap_err();
        assert!(format!("{err:#}").contains("signature"), "{err:#}");
        // The right key, but signed as some other file.
        let bytes = std::fs::read(r.dir.join(INDEX_FILE)).unwrap();
        let sig = r
            .key
            .sign(&bytes, "timestamp:1\tfile:Ryna_0.5.0_x64.app.tar.gz");
        std::fs::write(r.dir.join(format!("{INDEX_FILE}.sig")), sig).unwrap();
        let err = r.refresh(&state, &core).unwrap_err();
        assert!(
            format!("{err:#}").contains("wasn't signed as addons.json"),
            "{err:#}"
        );
        assert!(!state.join("addons").exists(), "nothing was written");

        r.publish(5, vec![meter.clone()]);
        r.refresh(&state, &core).unwrap();
        r.publish(4, vec![meter]);
        let err = r.refresh(&state, &core).unwrap_err();
        assert!(format!("{err:#}").contains("older (serial 4)"), "{err:#}");
        // No index at all (before the first bundle release): an error, and the held copy stays.
        std::fs::remove_file(r.dir.join(INDEX_FILE)).unwrap();
        assert!(r.refresh(&state, &core).is_err());
        assert_ne!(source_dir(&state, &core, "0.5.0"), core);
    }

    #[test]
    fn a_zip_that_isnt_one_addon_folder_is_refused() {
        let tmp = tempfile::tempdir().unwrap();
        let state = tmp.path().join("state");
        let core = carried(tmp.path(), "0.5.0", "0.5.0");
        let r = Release::new(tmp.path());
        let good_toc = toc("Ryna_Meter", "1.0.1", 2);
        let cases: Vec<(&str, Vec<u8>)> = vec![
            (
                "isn't in Ryna_Meter/",
                zip_of(&[
                    ("Ryna_Meter/Ryna_Meter.toc", &good_toc),
                    ("Ryna_Core/Core.lua", "-- over the core"),
                ]),
            ),
            (
                "isn't in Ryna_Meter/",
                zip_of(&[
                    ("Ryna_Meter/Ryna_Meter.toc", &good_toc),
                    ("Ryna_Meter/../../outside.lua", "-- out"),
                ]),
            ),
            (
                "says version 9.9.9, not 1.0.1",
                addon_zip("Ryna_Meter", "9.9.9", 2),
            ),
            (
                "doesn't say API level 2",
                addon_zip("Ryna_Meter", "1.0.1", 3),
            ),
            ("it isn't a zip", b"plain text".to_vec()),
        ];
        for (n, (why, zip)) in cases.into_iter().enumerate() {
            let meter = r.entry("Ryna_Meter", "1.0.1", 2, &zip);
            r.publish(n as u64 + 1, vec![meter]);
            let err = r.refresh(&state, &core).unwrap_err();
            assert!(format!("{err:#}").contains(why), "{why}: {err:#}");
            assert_eq!(source_dir(&state, &core, "0.5.0"), core);
            assert!(!tmp.path().join("outside.lua").exists());
            assert!(!state.join("addons/staging").exists());
        }
    }

    #[test]
    fn toc_fields_are_read_whatever_the_line_endings() {
        let toc = "\u{feff}## Title: X\r\n## X-Ryna-API: 2\r\n## x-ryna-api-min:  1 \r\n";
        assert_eq!(toc_field(toc, TOC_API).as_deref(), Some("2"));
        assert_eq!(toc_field(toc, TOC_API_MIN).as_deref(), Some("1"));
        assert_eq!(toc_field(toc, "Version"), None);
    }
}
