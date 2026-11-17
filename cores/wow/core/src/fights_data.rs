//! The Fights view's file (docs/ryna-agent-cores-mods.md 7 and 7.5, "The fight timeline"): the
//! client refuses the combat log to addons, so the app, which already reads every fight from
//! `Logs/WoWCombatLog-*.txt` ([`crate::combat`]), keeps what the in-game view needs of each one and
//! writes the most recent into `Interface/AddOns/Ryna_Core/Data/fights.lua`, which the game loads at login
//! and `/reload`:
//!
//! ```lua
//! RynaData_fights = {
//!   version = 1, writtenAt = 1762464000, flavor = "wowf",
//!   items = {                                   -- the fights, newest first
//!     {
//!       at = 1762463000, length = 36000,        -- its start (unix seconds) and length (ms)
//!       kind = "trash", zone = "The Deadmines", pulls = 1,   -- kind: boss | trash | world
//!       boss = "Sneed", result = "kill",        -- a boss only; result: kill | wipe (absent: the log didn't say)
//!       step = 900, steps = 40,                 -- the strips' clock: `steps` steps of `step` ms
//!       by = "Player-…",                        -- the logging character (whose fight list it is)
//!       me = { done = 191, taken = 689, heal = 0, died = 29000 },   -- the logger's own (pets in), died: ms of its first death
//!       first = { by = "group", who = "Felcitia Stoutbrew", pet = "Wolf", on = "Goblin Shipbuilder", onNth = 1,
//!                 with = "Flame Shock", t = 8000, miss = true, close = true,
//!                 answer = { by = "enemy", who = "Defias Pirate", whoNth = 1, on = "Maddiep Ryna", t = 10000 } },
//!       deaths = { { "Maddiep Ryna", 26000, 5 }, … },             -- name, ms, its row in players (0: none)
//!       players = {
//!         { name = "Felcitia Stoutbrew", class = "SHAMAN", done = 191, taken = 689, heal = 0,
//!           doneSteps = { 0, 0, 40, … }, takenSteps = { … } },
//!         { name = "Wolf", pet = 1, done = 0, taken = 50, heal = 0, doneSteps = {}, takenSteps = { … } },
//!       },
//!       otherPlayers = { n = 3, done = 120, taken = 40, heal = 0 },
//!       enemies = {
//!         { name = "Goblin Shipbuilder", nth = 1, done = 341, taken = 1693,
//!           targets = { { 1, 0, 2000 }, { 3, 2100, 14000 } }, more = 0 },   -- whom, from ms, for ms
//!       },
//!       otherEnemies = { n = 3, done = 831, taken = 51 },
//!     },
//!   },
//! }
//! ```
//!
//! The rules are the portal's timeline's (the API's `timeline.ts`, docs/wow.md 9.7), ported:
//!
//! - **Sides.** A player, or a player's pet, is the group's side; everyone else is an enemy.
//! - **Pets.** A pet's damage and healing done count in its owner's row, as the meters do; a pet that
//!   took damage has its own row under its owner, holding what it took. A player's pets that share a
//!   name are one pet resummoned (a new GUID each time it is called back), so one row.
//! - **The first blow** is the first hit or miss between the two sides, whichever swung it (no spell
//!   that hurts nobody, no fall): the log has no casts begun, auras or positions, so it can only ever
//!   say who struck first, never who "pulled". `close` when the other side struck within 250 ms (the
//!   log's order may not be the game's). When the group struck first, `answer` is the enemy side's first
//!   hit or miss on the group after it.
//! - **Whom an enemy was attacking**: its melee swings decide; a spell counts only when it swung at
//!   nobody within 1 s either side, and a spell landing on two or more of the group within 100 ms (an
//!   area hit) never counts. A span runs from its first attack on someone to its last before it turns to
//!   someone else, plus 2 s, cut short by its next attack, its death or the fight's end; a pause longer
//!   than 4 s ends it. Spans on the same one in a row are joined here (`for` is their summed length).
//! - **Same-named enemies** are numbered by when they first showed up (`nth`; absent for a name held once).
//!
//! Amounts are what the agent's meters count ([`crate::combat::summary`]): damage less overkill,
//! healing less overheal, absorbs whole; the swing copies the client writes twice are already gone
//! ([`crate::combat::segment::dedupe_swings`]). Steps are shares from 0 to 100 of the fight's biggest step
//! (done and taken each on their own scale, shared by every row), trailing zeros left out.
//!
//! **Caps.** At most [`MAX_FIGHTS`] fights, [`MAX_ROWS`] player and pet rows (the rest folded in
//! `otherPlayers`), [`MAX_ENEMIES`] enemies (the rest in `otherEnemies`; the most damage taken plus
//! done first), [`MAX_SPANS`] targets per enemy (`more` counts the rest) and [`MAX_DEATHS`] deaths; the
//! file stays under [`BUDGET_BYTES`], the oldest fights dropped first. A 5-player fight costs about 2 to
//! 4 KB, a full one (25 rows, 8 enemies with 12 targets each, 40 deaths) about 14 KB (the tests measure
//! both).
//!
//! **Where it comes from, and who it names.** The tailer ([`crate::combat::tail`]) keeps each finished
//! fight's view in `<state>/combat/fights.json`, per install, and rewrites the install's file when a
//! fight finishes and at start-up. The first start with this file reads the newest logs still on disk
//! once ([`backfill`]). Nothing is fetched and nothing is sent: the file is the logger's own combat log,
//! summarised into their own addon folder like a damage meter's saved data. It holds the names of the
//! players who were in those fights and nobody else's, and stays on this computer. Names, not GUIDs,
//! except `by` (the logger's own character, so the addon can tell its own fights from an alt's); the
//! strings are stripped of `|` and control characters, so no name can carry a colour code or a link.

use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};
use std::fmt::Write as _;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde::{Deserialize, Serialize};

use crate::agent_status::lua_string;
use crate::combat::digest::{ActorMeta, DigestEvent};
use crate::combat::segment::{Fight, Segmenter};
use crate::discovery::Install;
use crate::lua_write::write_atomic;
use crate::prices_data::{global, write_if_changed, DIR};

pub const FILE: &str = "fights.lua";
/// The table's version: the addon reads only this one (Ryna_Fights Data.lua).
pub const VERSION: u32 = 1;
/// The newest fights kept per install.
pub const MAX_FIGHTS: usize = 20;
/// The file's size budget: the game parses it at every login and `/reload`.
pub const BUDGET_BYTES: usize = 192 * 1024;
/// Steps over a fight: one a second up to this many, then wider.
pub const STEPS: usize = 40;
/// Player and pet rows per fight; the rest are folded.
pub const MAX_ROWS: usize = 25;
/// Enemies per fight; the rest are folded.
pub const MAX_ENEMIES: usize = 8;
/// Targets kept per enemy.
pub const MAX_SPANS: usize = 12;
/// Deaths kept per fight.
pub const MAX_DEATHS: usize = 40;
/// The one backfill reads whole log files, newest first, until it has `MAX_FIGHTS` or has read this much.
pub const BACKFILL_BYTES: u64 = 128 * 1024 * 1024;
/// The store under the state folder's `combat/`.
pub const STORE_FILE: &str = "fights.json";
/// A name's longest form in the file.
const NAME_CHARS: usize = 64;

/// The melee "spell" (`crate::combat::parse::melee`).
const SWING: i64 = 1;
const NO_GUID: &str = "0000000000000000";
const FIRST_BLOW_CONTESTED_MS: i64 = 250;
const TARGET_SWING_MS: i64 = 1000;
const TARGET_SAME_INSTANT_MS: i64 = 100;
const TARGET_GAP_MS: i64 = 4000;
const TARGET_TAIL_MS: i64 = 2000;
/// The most spans one enemy's attacks are read into before they are joined and capped.
const TARGET_SPANS: usize = 200;

/// The logger's own numbers in a fight.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Mine {
    pub done: i64,
    pub taken: i64,
    pub heal: i64,
    pub died: Option<i64>,
}

/// A blow between the sides: the first, or the enemy's answer to it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Blow {
    /// `group` or `enemy`: which side swung it.
    pub by: String,
    /// The group's player (a pet's owner), or the enemy.
    pub who: String,
    pub who_nth: Option<u32>,
    /// When a player's pet swung it, the pet.
    pub pet: Option<String>,
    pub on: String,
    pub on_nth: Option<u32>,
    /// The spell's name; none for a melee swing.
    pub with: Option<String>,
    pub t: i64,
    pub miss: bool,
    pub close: bool,
    pub answer: Option<Box<Blow>>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Death {
    pub name: String,
    pub t: i64,
    /// Its row in `players` (1-based), 0 when it has none.
    pub row: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Row {
    pub name: String,
    pub class: Option<String>,
    /// A pet's owner's row (1-based); 0 for a player.
    pub pet: usize,
    pub done: i64,
    pub taken: i64,
    pub heal: i64,
    pub done_steps: Vec<u8>,
    pub taken_steps: Vec<u8>,
}

/// One stretch of an enemy attacking someone of the group.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Span {
    /// The row attacked (1-based; a pet without a row of its own is its owner's), 0 when it has none.
    pub row: usize,
    /// Its name when `row` is 0.
    pub name: String,
    pub from: i64,
    pub ms: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Enemy {
    pub name: String,
    pub nth: Option<u32>,
    pub done: i64,
    pub taken: i64,
    pub targets: Vec<Span>,
    /// Targets past `MAX_SPANS`.
    pub more: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Folded {
    pub n: usize,
    pub done: i64,
    pub taken: i64,
    pub heal: i64,
}

/// What the in-game view keeps of one fight.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FightView {
    /// Unix ms (the file says seconds).
    pub started_at: i64,
    pub length: i64,
    pub kind: String,
    pub zone: Option<String>,
    pub boss: Option<String>,
    pub result: Option<String>,
    pub pulls: u32,
    pub step: i64,
    pub steps: usize,
    pub by: Option<String>,
    pub me: Option<Mine>,
    pub first: Option<Blow>,
    pub deaths: Vec<Death>,
    pub players: Vec<Row>,
    pub other_players: Option<Folded>,
    pub enemies: Vec<Enemy>,
    pub other_enemies: Option<Folded>,
}

fn is_player(g: &str) -> bool {
    g.starts_with("Player-")
}

fn real(g: &Option<String>) -> Option<&str> {
    g.as_deref().filter(|g| !g.is_empty() && *g != NO_GUID)
}

/// The player whose numbers a group actor's fold into: a player itself, a player's pet its owner;
/// None for anyone not of the group.
fn group_owner(g: &str, actors: &BTreeMap<String, ActorMeta>) -> Option<String> {
    if is_player(g) {
        return Some(g.to_string());
    }
    actors
        .get(g)
        .and_then(|m| m.owner.clone())
        .filter(|o| is_player(o))
}

/// A name as the file holds it: no `|` or control character, at most `NAME_CHARS` long.
fn clean(s: &str) -> String {
    s.chars()
        .filter(|c| *c != '|' && !c.is_control())
        .take(NAME_CHARS)
        .collect::<String>()
        .trim()
        .to_string()
}

/// An actor's name: a player's as the portal shows it ("First-Last" as "First Last"), else the log's.
fn name_of(g: &str, actors: &BTreeMap<String, ActorMeta>) -> String {
    match actors.get(g).and_then(|m| m.name.as_deref()) {
        Some(n) if is_player(g) => clean(&n.replace('-', " ")),
        Some(n) => clean(n),
        None => String::new(),
    }
}

/// A player's pets that share a name are one pet resummoned: each such GUID mapped to the one that
/// showed up first.
fn pet_aliases(
    events: &[DigestEvent],
    actors: &BTreeMap<String, ActorMeta>,
) -> HashMap<String, String> {
    let mut first_seen: HashMap<&str, i64> = HashMap::new();
    for e in events {
        for g in [real(&e.s), real(&e.d)].into_iter().flatten() {
            first_seen.entry(g).or_insert(e.t);
        }
    }
    let mut by_name: BTreeMap<(String, String), Vec<&str>> = BTreeMap::new();
    for (g, m) in actors {
        let (Some(name), Some(owner)) = (m.name.as_deref(), m.owner.as_deref()) else {
            continue;
        };
        if is_player(g) || !is_player(owner) || name.is_empty() {
            continue;
        }
        by_name
            .entry((owner.to_string(), name.to_string()))
            .or_default()
            .push(g);
    }
    let mut out = HashMap::new();
    for mut list in by_name.into_values() {
        if list.len() < 2 {
            continue;
        }
        list.sort_by_key(|g| (first_seen.get(g).copied().unwrap_or(i64::MAX), *g));
        let keep = list[0].to_string();
        for g in &list[1..] {
            out.insert((*g).to_string(), keep.clone());
        }
    }
    out
}

#[derive(Debug, Clone, Default)]
struct Acc {
    done: Vec<i64>,
    taken: Vec<i64>,
    heal: Vec<i64>,
}

fn sum(v: &[i64]) -> i64 {
    v.iter().sum()
}

/// Steps as shares of `max`, 0 to 100 (anything above nothing at least 1), trailing zeros left out.
fn shares(v: &[i64], max: i64) -> Vec<u8> {
    let mut out: Vec<u8> = v
        .iter()
        .map(|&x| {
            if x <= 0 || max <= 0 {
                0
            } else {
                ((x * 100 + max / 2) / max).clamp(1, 100) as u8
            }
        })
        .collect();
    while out.last() == Some(&0) {
        out.pop();
    }
    out
}

fn spell_of(e: &DigestEvent) -> i64 {
    e.sp.unwrap_or(SWING)
}

/// Whom one enemy was attacking, as `(target, from, to)` in ms from the start: `attacks` are its hits
/// and misses on the group, oldest first; `end_ms` is its death or the fight's end.
fn target_spans(attacks: &[&DigestEvent], end_ms: i64) -> Vec<(String, i64, i64)> {
    let near = |i: usize, ms: i64, hit: &dyn Fn(&DigestEvent) -> bool| -> bool {
        let at = attacks[i].t;
        let before = attacks[..i]
            .iter()
            .rev()
            .take_while(|o| at - o.t <= ms)
            .any(|o| hit(o));
        before
            || attacks[i + 1..]
                .iter()
                .take_while(|o| o.t - at <= ms)
                .any(|o| hit(o))
    };
    let deciding: Vec<&DigestEvent> = attacks
        .iter()
        .enumerate()
        .filter(|(i, e)| {
            let sp = spell_of(e);
            if sp == SWING {
                return true;
            }
            if near(*i, TARGET_SWING_MS, &|o| spell_of(o) == SWING) {
                return false;
            }
            // An area spell: the same spell on someone else of the group at the same instant.
            !near(*i, TARGET_SAME_INSTANT_MS, &|o| {
                spell_of(o) == sp && o.d != e.d
            })
        })
        .map(|(_, e)| *e)
        .collect();
    let mut spans: Vec<(String, i64, i64)> = Vec::new();
    // (target, from, last attack)
    let mut cur: Option<(String, i64, i64)> = None;
    let close = |cur: &mut Option<(String, i64, i64)>, spans: &mut Vec<_>, until: i64| {
        if let Some((target, from, last)) = cur.take() {
            let to = (last + TARGET_TAIL_MS).min(until).min(end_ms);
            spans.push((target, from, to.max(from)));
        }
    };
    for e in deciding {
        let t = e.t;
        if t > end_ms {
            break;
        }
        let target = e.d.clone().unwrap_or_default();
        if let Some((ct, _, last)) = cur.as_mut() {
            if *ct == target && t - *last <= TARGET_GAP_MS {
                *last = t;
                continue;
            }
        }
        close(&mut cur, &mut spans, t);
        if spans.len() >= TARGET_SPANS {
            break;
        }
        cur = Some((target, t.max(0), t));
    }
    if spans.len() < TARGET_SPANS {
        close(&mut cur, &mut spans, end_ms);
    }
    spans
}

/// The view of one fight. `classes` names the class of the characters the app knows (the accounts'
/// own, by GUID): the log itself never says.
pub fn view(fight: &Fight, classes: &HashMap<String, String>) -> FightView {
    let actors = &fight.header.actors;
    let alias = pet_aliases(&fight.digest, actors);
    let mut events: Vec<DigestEvent> = fight
        .digest
        .iter()
        .map(|e| {
            let mut e = e.clone();
            for g in [&mut e.s, &mut e.d] {
                if let Some(to) = g.as_ref().and_then(|x| alias.get(x)) {
                    *g = Some(to.clone());
                }
            }
            e
        })
        .collect();
    events.sort_by_key(|e| e.t);

    let length = fight.duration_ms().max(1);
    let steps = ((length + 999) / 1000).clamp(1, STEPS as i64) as usize;
    let step = ((length + steps as i64 - 1) / steps as i64).max(1);
    let index = |t: i64| ((t.max(0) / step) as usize).min(steps - 1);

    let mut accs: BTreeMap<String, Acc> = BTreeMap::new();
    let blank = Acc {
        done: vec![0; steps],
        taken: vec![0; steps],
        heal: vec![0; steps],
    };
    let mut first_seen: HashMap<String, i64> = HashMap::new();
    let mut died: HashMap<String, i64> = HashMap::new();
    let mut engaged: BTreeSet<String> = BTreeSet::new();
    let mut attacks: BTreeMap<String, Vec<&DigestEvent>> = BTreeMap::new();
    let mut deaths: Vec<(String, i64)> = Vec::new();
    let owner = |g: &str| group_owner(g, actors);

    for e in &events {
        for g in [real(&e.s), real(&e.d)].into_iter().flatten() {
            first_seen.entry(g.to_string()).or_insert(e.t);
        }
        let (s, d) = (real(&e.s), real(&e.d));
        if e.e == "X" {
            if let Some(d) = d {
                died.entry(d.to_string()).or_insert(e.t);
                if owner(d).is_some() {
                    deaths.push((d.to_string(), e.t));
                }
            }
            continue;
        }
        if !matches!(e.e.as_str(), "D" | "H" | "A" | "M") {
            continue;
        }
        let so = s.and_then(owner);
        let dg = d.and_then(owner);
        if let (Some(s), Some(d), true) = (s, d, e.e == "D" || e.e == "M") {
            if so.is_some() && dg.is_none() {
                engaged.insert(d.to_string());
            }
            if so.is_none() && dg.is_some() {
                engaged.insert(s.to_string());
                if spell_of(e) >= 0 {
                    attacks.entry(s.to_string()).or_default().push(e);
                }
            }
        }
        if e.e == "M" {
            continue;
        }
        let amount = e.effective();
        let i = index(e.t);
        if let Some(s) = s {
            let who = so.clone().unwrap_or_else(|| s.to_string());
            let a = accs.entry(who).or_insert_with(|| blank.clone());
            if e.e == "D" {
                a.done[i] += amount;
            } else {
                a.heal[i] += amount;
            }
        }
        if let (Some(d), "D") = (d, e.e.as_str()) {
            accs.entry(d.to_string())
                .or_insert_with(|| blank.clone())
                .taken[i] += amount;
        }
    }

    // Same-named enemies, numbered by when they first showed up.
    let mut nth: HashMap<String, u32> = HashMap::new();
    let mut by_name: BTreeMap<String, Vec<&String>> = BTreeMap::new();
    for g in &engaged {
        let n = name_of(g, actors);
        if !n.is_empty() {
            by_name.entry(n).or_default().push(g);
        }
    }
    for list in by_name.values_mut() {
        if list.len() < 2 {
            continue;
        }
        list.sort_by_key(|g| {
            (
                first_seen.get(*g).copied().unwrap_or(i64::MAX),
                (*g).clone(),
            )
        });
        for (i, g) in list.iter().enumerate() {
            nth.insert((*g).clone(), i as u32 + 1);
        }
    }

    // The first blow, and the enemy's answer when the group struck first.
    let is_group = |g: &str| owner(g).is_some();
    let eligible = |e: &DigestEvent| -> bool {
        let (Some(s), Some(d)) = (real(&e.s), real(&e.d)) else {
            return false;
        };
        (e.e == "D" || e.e == "M") && spell_of(e) >= 0 && is_group(s) != is_group(d)
    };
    let spell_name = |e: &DigestEvent| -> Option<String> {
        let sp = spell_of(e);
        if sp == SWING {
            return None;
        }
        fight
            .header
            .spells
            .get(&sp)
            .map(|s| clean(&s.name))
            .filter(|n| !n.is_empty())
    };
    let blow = |e: &DigestEvent| -> Blow {
        let s = real(&e.s).unwrap_or_default();
        let d = real(&e.d).unwrap_or_default();
        let group = is_group(s);
        let (who, pet) = if group {
            let o = owner(s).unwrap_or_else(|| s.to_string());
            let pet = (o != s).then(|| name_of(s, actors));
            (name_of(&o, actors), pet)
        } else {
            (name_of(s, actors), None)
        };
        Blow {
            by: if group { "group" } else { "enemy" }.to_string(),
            who,
            who_nth: if group { None } else { nth.get(s).copied() },
            pet,
            on: name_of(d, actors),
            on_nth: if group { nth.get(d).copied() } else { None },
            with: spell_name(e),
            t: e.t.max(0),
            miss: e.e == "M",
            close: false,
            answer: None,
        }
    };
    let first = events.iter().position(eligible).map(|i| {
        let e = &events[i];
        let mut b = blow(e);
        let by_group = b.by == "group";
        b.close = events[i + 1..]
            .iter()
            .take_while(|o| o.t - e.t <= FIRST_BLOW_CONTESTED_MS)
            .any(|o| eligible(o) && is_group(real(&o.s).unwrap_or_default()) != by_group);
        if by_group {
            b.answer = events[i + 1..]
                .iter()
                .find(|o| eligible(o) && !is_group(real(&o.s).unwrap_or_default()))
                .map(|o| Box::new(blow(o)));
        }
        (
            b,
            real(&e.s).unwrap_or_default().to_string(),
            real(&e.d).unwrap_or_default().to_string(),
        )
    });

    // The rows: every player the fight names or touched, the first blow's player first, then by
    // damage done; under each, its pets that took damage.
    let mut players: BTreeSet<String> = actors.keys().filter(|g| is_player(g)).cloned().collect();
    players.extend(accs.keys().filter(|g| is_player(g)).cloned());
    let total = |g: &str, f: fn(&Acc) -> &Vec<i64>| accs.get(g).map_or(0, |a| sum(f(a)));
    let first_player = first.as_ref().and_then(
        |(b, s, d)| {
            if b.by == "group" {
                owner(s)
            } else {
                owner(d)
            }
        },
    );
    let mut ordered: Vec<String> = players.iter().cloned().collect();
    ordered.sort_by(|a, b| {
        let fa = first_player.as_deref() == Some(a.as_str());
        let fb = first_player.as_deref() == Some(b.as_str());
        fb.cmp(&fa)
            .then_with(|| total(b, |x| &x.done).cmp(&total(a, |x| &x.done)))
            .then_with(|| name_of(a, actors).cmp(&name_of(b, actors)))
            .then_with(|| a.cmp(b))
    });
    let mut pets: Vec<String> = accs
        .keys()
        .filter(|g| !is_player(g) && owner(g).is_some() && total(g, |x| &x.taken) > 0)
        .cloned()
        .collect();
    pets.sort_by(|a, b| {
        total(b, |x| &x.taken)
            .cmp(&total(a, |x| &x.taken))
            .then_with(|| a.cmp(b))
    });
    let mut lanes: Vec<String> = Vec::new();
    for p in &ordered {
        lanes.push(p.clone());
        lanes.extend(
            pets.iter()
                .filter(|x| owner(x).as_deref() == Some(p.as_str()))
                .cloned(),
        );
    }
    lanes.extend(
        pets.iter()
            .filter(|x| !owner(x).is_some_and(|o| players.contains(&o)))
            .cloned(),
    );
    let kept: Vec<String> = lanes.iter().take(MAX_ROWS).cloned().collect();
    let row_of: HashMap<String, usize> = kept
        .iter()
        .enumerate()
        .map(|(i, g)| (g.clone(), i + 1))
        .collect();
    let max_done = kept
        .iter()
        .filter_map(|g| accs.get(g))
        .flat_map(|a| a.done.iter().copied())
        .max()
        .unwrap_or(0);
    let max_taken = kept
        .iter()
        .filter_map(|g| accs.get(g))
        .flat_map(|a| a.taken.iter().copied())
        .max()
        .unwrap_or(0);
    let rows: Vec<Row> = kept
        .iter()
        .map(|g| {
            let a = accs.get(g).cloned().unwrap_or_else(|| blank.clone());
            let pet = if is_player(g) {
                0
            } else {
                owner(g).and_then(|o| row_of.get(&o).copied()).unwrap_or(0)
            };
            Row {
                name: name_of(g, actors),
                class: classes.get(g).map(|c| clean(c)).filter(|c| !c.is_empty()),
                pet,
                done: sum(&a.done),
                taken: sum(&a.taken),
                heal: sum(&a.heal),
                done_steps: shares(&a.done, max_done),
                taken_steps: shares(&a.taken, max_taken),
            }
        })
        .collect();
    let rest: Vec<&String> = lanes.iter().skip(MAX_ROWS).collect();
    let other_players = (!rest.is_empty()).then(|| Folded {
        n: rest.len(),
        done: rest.iter().map(|g| total(g, |x| &x.done)).sum(),
        taken: rest.iter().map(|g| total(g, |x| &x.taken)).sum(),
        heal: rest.iter().map(|g| total(g, |x| &x.heal)).sum(),
    });
    // The row an attack on `g` shows on: its own, else (a pet without one) its owner's.
    let row_for = |g: &str| -> usize {
        row_of
            .get(g)
            .copied()
            .or_else(|| owner(g).and_then(|o| row_of.get(&o).copied()))
            .unwrap_or(0)
    };

    // Enemies: the most damage taken plus done first.
    let mut ranked: Vec<&String> = engaged.iter().collect();
    ranked.sort_by(|a, b| {
        (total(b, |x| &x.taken) + total(b, |x| &x.done))
            .cmp(&(total(a, |x| &x.taken) + total(a, |x| &x.done)))
            .then_with(|| name_of(a, actors).cmp(&name_of(b, actors)))
            .then_with(|| nth.get(*a).cmp(&nth.get(*b)))
            .then_with(|| a.cmp(b))
    });
    let enemies: Vec<Enemy> = ranked
        .iter()
        .take(MAX_ENEMIES)
        .map(|g| {
            let end = died.get(*g).copied().unwrap_or(length).min(length);
            let list = attacks.get(*g).map(Vec::as_slice).unwrap_or(&[]);
            let mut joined: Vec<Span> = Vec::new();
            for (target, from, to) in target_spans(list, end) {
                let row = row_for(&target);
                let name = if row == 0 {
                    name_of(&target, actors)
                } else {
                    String::new()
                };
                match joined.last_mut() {
                    Some(last) if last.row == row && last.name == name => last.ms += to - from,
                    _ => joined.push(Span {
                        row,
                        name,
                        from,
                        ms: to - from,
                    }),
                }
            }
            let more = joined.len().saturating_sub(MAX_SPANS);
            joined.truncate(MAX_SPANS);
            Enemy {
                name: name_of(g, actors),
                nth: nth.get(*g).copied(),
                done: total(g, |x| &x.done),
                taken: total(g, |x| &x.taken),
                targets: joined,
                more,
            }
        })
        .collect();
    let rest: Vec<&&String> = ranked.iter().skip(MAX_ENEMIES).collect();
    let other_enemies = (!rest.is_empty()).then(|| Folded {
        n: rest.len(),
        done: rest.iter().map(|g| total(g, |x| &x.done)).sum(),
        taken: rest.iter().map(|g| total(g, |x| &x.taken)).sum(),
        heal: 0,
    });

    let deaths: Vec<Death> = deaths
        .iter()
        .take(MAX_DEATHS)
        .map(|(g, t)| Death {
            name: name_of(g, actors),
            t: *t,
            row: row_of.get(g).copied().unwrap_or(0),
        })
        .collect();
    let me = fight.logger.as_deref().map(|g| Mine {
        done: total(g, |x| &x.done),
        taken: total(g, |x| &x.taken),
        heal: total(g, |x| &x.heal),
        died: died.get(g).copied(),
    });
    let boss = fight.kind == crate::combat::segment::FightKind::Boss;
    FightView {
        started_at: fight.started_at,
        length,
        kind: fight.kind.as_str().to_string(),
        zone: fight.zone.as_deref().map(clean).filter(|z| !z.is_empty()),
        boss: if boss {
            fight.name.as_deref().map(clean).filter(|n| !n.is_empty())
        } else {
            None
        },
        result: if boss {
            fight
                .success
                .map(|s| if s { "kill" } else { "wipe" }.to_string())
        } else {
            None
        },
        pulls: fight.pulls,
        step,
        steps,
        by: fight.logger.clone(),
        me,
        first: first.map(|(b, _, _)| b),
        deaths,
        players: rows,
        other_players,
        enemies,
        other_enemies,
    }
}

// The file ---------------------------------------------------------------------------------------

fn opt_str(out: &mut String, key: &str, v: Option<&str>) {
    if let Some(v) = v {
        let _ = write!(out, ", {key} = {}", lua_string(v));
    }
}

fn opt_int(out: &mut String, key: &str, v: Option<i64>) {
    if let Some(v) = v {
        let _ = write!(out, ", {key} = {v}");
    }
}

fn steps_lua(v: &[u8]) -> String {
    if v.is_empty() {
        return "{}".into();
    }
    let parts: Vec<String> = v.iter().map(u8::to_string).collect();
    format!("{{ {} }}", parts.join(", "))
}

fn folded_lua(f: &Folded, heal: bool) -> String {
    let mut s = format!("{{ n = {}, done = {}, taken = {}", f.n, f.done, f.taken);
    if heal {
        let _ = write!(s, ", heal = {}", f.heal);
    }
    s.push_str(" }");
    s
}

fn blow_lua(b: &Blow) -> String {
    let mut s = format!(
        "{{ by = {}, who = {}",
        lua_string(&b.by),
        lua_string(&b.who)
    );
    opt_int(&mut s, "whoNth", b.who_nth.map(i64::from));
    opt_str(&mut s, "pet", b.pet.as_deref());
    let _ = write!(s, ", on = {}", lua_string(&b.on));
    opt_int(&mut s, "onNth", b.on_nth.map(i64::from));
    opt_str(&mut s, "with", b.with.as_deref());
    let _ = write!(s, ", t = {}", b.t);
    if b.miss {
        s.push_str(", miss = true");
    }
    if b.close {
        s.push_str(", close = true");
    }
    if let Some(a) = &b.answer {
        let _ = write!(s, ", answer = {}", blow_lua(a));
    }
    s.push_str(" }");
    s
}

/// One fight as the file writes it (an entry of `items`, with its trailing comma).
pub fn fight_lua(f: &FightView) -> String {
    let mut out = String::new();
    out.push_str("\t\t{\n");
    let _ = write!(
        out,
        "\t\t\tat = {}, length = {}, kind = {}",
        f.started_at.div_euclid(1000),
        f.length,
        lua_string(&f.kind)
    );
    opt_str(&mut out, "zone", f.zone.as_deref());
    let _ = write!(
        out,
        ", pulls = {}, step = {}, steps = {}",
        f.pulls, f.step, f.steps
    );
    opt_str(&mut out, "boss", f.boss.as_deref());
    opt_str(&mut out, "result", f.result.as_deref());
    opt_str(&mut out, "by", f.by.as_deref());
    out.push_str(",\n");
    if let Some(m) = &f.me {
        let _ = write!(
            out,
            "\t\t\tme = {{ done = {}, taken = {}, heal = {}",
            m.done, m.taken, m.heal
        );
        opt_int(&mut out, "died", m.died);
        out.push_str(" },\n");
    }
    if let Some(b) = &f.first {
        let _ = writeln!(out, "\t\t\tfirst = {},", blow_lua(b));
    }
    out.push_str("\t\t\tdeaths = {");
    for d in &f.deaths {
        let _ = write!(out, " {{ {}, {}, {} }},", lua_string(&d.name), d.t, d.row);
    }
    out.push_str(" },\n\t\t\tplayers = {\n");
    for r in &f.players {
        let _ = write!(out, "\t\t\t\t{{ name = {}", lua_string(&r.name));
        opt_str(&mut out, "class", r.class.as_deref());
        if r.pet > 0 {
            let _ = write!(out, ", pet = {}", r.pet);
        }
        let _ = writeln!(
            out,
            ", done = {}, taken = {}, heal = {}, doneSteps = {}, takenSteps = {} }},",
            r.done,
            r.taken,
            r.heal,
            steps_lua(&r.done_steps),
            steps_lua(&r.taken_steps)
        );
    }
    out.push_str("\t\t\t},\n");
    if let Some(o) = &f.other_players {
        let _ = writeln!(out, "\t\t\totherPlayers = {},", folded_lua(o, true));
    }
    out.push_str("\t\t\tenemies = {\n");
    for e in &f.enemies {
        let _ = write!(out, "\t\t\t\t{{ name = {}", lua_string(&e.name));
        opt_int(&mut out, "nth", e.nth.map(i64::from));
        let _ = write!(
            out,
            ", done = {}, taken = {}, targets = {{",
            e.done, e.taken
        );
        for s in &e.targets {
            let who = if s.row > 0 {
                s.row.to_string()
            } else {
                lua_string(&s.name)
            };
            let _ = write!(out, " {{ {who}, {}, {} }},", s.from, s.ms);
        }
        let _ = writeln!(out, " }}, more = {} }},", e.more);
    }
    out.push_str("\t\t\t},\n");
    if let Some(o) = &f.other_enemies {
        let _ = writeln!(out, "\t\t\totherEnemies = {},", folded_lua(o, false));
    }
    out.push_str("\t\t},\n");
    out
}

/// `Data/fights.lua`'s text for `fights` (newest first), inside the budget: over it, the oldest go.
/// Returns the text and how many fights it holds.
pub fn fights_lua(fights: &[FightView], written_at: i64, flavor: &str) -> (String, usize) {
    let mut head = String::new();
    head.push_str(
        "-- Written by the Ryna companion app from its own reading of your combat log. Edits are overwritten.\n",
    );
    head.push_str("-- Data/fights.lua: your most recent fights, newest first, for Ryna's Fights section. It names\n");
    head.push_str("-- the players who were in them, and stays on this computer.\n");
    let _ = writeln!(head, "{} = {{", global("fights"));
    let _ = writeln!(head, "\tversion = {VERSION},");
    let _ = writeln!(head, "\twrittenAt = {written_at},");
    let _ = writeln!(head, "\tflavor = {},", lua_string(flavor));
    head.push_str("\titems = {\n");
    let foot = "\t},\n}\n";
    let mut size = head.len() + foot.len();
    let mut body = String::new();
    let mut n = 0;
    for f in fights.iter().take(MAX_FIGHTS) {
        let text = fight_lua(f);
        if n > 0 && size + text.len() > BUDGET_BYTES {
            break;
        }
        size += text.len();
        body.push_str(&text);
        n += 1;
    }
    (format!("{head}{body}{foot}"), n)
}

// The store and the install's file ---------------------------------------------------------------

/// One install's fights, oldest first, and whether the logs on disk were read for it once.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct InstallFights {
    pub backfilled: bool,
    pub fights: Vec<FightView>,
}

/// `<state>/combat/fights.json`: every install's fights, by install path.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Store {
    pub installs: BTreeMap<String, InstallFights>,
}

fn key(install: &Path) -> String {
    install.to_string_lossy().into_owned()
}

impl Store {
    /// The store on disk; an unreadable one (or one from another shape) is empty.
    pub fn load(state_dir: &Path) -> Store {
        std::fs::read(state_dir.join("combat").join(STORE_FILE))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, state_dir: &Path) -> std::io::Result<()> {
        let dir = state_dir.join("combat");
        std::fs::create_dir_all(&dir)?;
        let body = serde_json::to_vec(self).map_err(std::io::Error::other)?;
        write_atomic(&dir.join(STORE_FILE), &body)
    }

    pub fn get(&self, install: &Path) -> Option<&InstallFights> {
        self.installs.get(&key(install))
    }

    /// Keep `view` for `install`: a fight with the same start replaces the one kept (a log read
    /// twice), and only the newest `MAX_FIGHTS` stay.
    pub fn add(&mut self, install: &Path, view: FightView) {
        let entry = self.installs.entry(key(install)).or_default();
        entry.fights.retain(|f| f.started_at != view.started_at);
        entry.fights.push(view);
        entry.fights.sort_by_key(|f| f.started_at);
        let excess = entry.fights.len().saturating_sub(MAX_FIGHTS);
        entry.fights.drain(..excess);
    }

    /// The backfill's fights for `install` (oldest first), merged with what's kept, and the install
    /// marked as backfilled.
    pub fn backfilled(&mut self, install: &Path, views: Vec<FightView>) {
        for v in views {
            self.add(install, v);
        }
        self.installs.entry(key(install)).or_default().backfilled = true;
    }
}

/// Write `install`'s `Data/fights.lua` from `fights` (oldest first, as kept) when the core addon is
/// there (an install the discovery found; never a guessed path), and only when the rows changed.
/// Returns the path when it was written.
pub fn write(
    install: &Install,
    fights: &[FightView],
    now: i64,
) -> std::io::Result<Option<PathBuf>> {
    let addon = crate::addon_install::addon_dir(install);
    if !addon.is_dir() {
        return Ok(None);
    }
    let newest: Vec<FightView> = fights.iter().rev().cloned().collect();
    let (text, _) = fights_lua(&newest, now, install.flavor.as_deref().unwrap_or("wowf"));
    let path = addon.join(DIR).join(FILE);
    Ok(write_if_changed(&path, &text)?.then_some(path))
}

/// The once-only read of the logs still on disk for an install the store has nothing from: whole
/// files, newest first, until `MAX_FIGHTS` fights or `BACKFILL_BYTES` read. Oldest first.
pub fn backfill(
    install: &Path,
    classes: &HashMap<String, String>,
    now: SystemTime,
) -> Vec<FightView> {
    let mut per_file: Vec<VecDeque<FightView>> = Vec::new();
    let (mut read, mut found) = (0u64, 0usize);
    for (path, _) in crate::combat::tail::logs(install, now) {
        if found >= MAX_FIGHTS || read >= BACKFILL_BYTES {
            break;
        }
        let Ok(file) = std::fs::File::open(&path) else {
            continue;
        };
        let mut reader = BufReader::new(file);
        let mut seg = Segmenter::new(false);
        let mut views: VecDeque<FightView> = VecDeque::new();
        let keep = |views: &mut VecDeque<FightView>, seg: &mut Segmenter| {
            for f in seg.take() {
                views.push_back(view(&f, classes));
                if views.len() > MAX_FIGHTS {
                    views.pop_front();
                }
            }
        };
        let mut line = Vec::new();
        let mut offset = 0u64;
        loop {
            line.clear();
            match reader.read_until(b'\n', &mut line) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    seg.feed(&String::from_utf8_lossy(&line), offset);
                    offset += n as u64;
                    keep(&mut views, &mut seg);
                }
            }
        }
        read += offset;
        seg.finish();
        keep(&mut views, &mut seg);
        found += views.len();
        per_file.push(views);
    }
    let all: Vec<FightView> = per_file.into_iter().rev().flatten().collect();
    let skip = all.len().saturating_sub(MAX_FIGHTS);
    all.into_iter().skip(skip).collect()
}

#[cfg(test)]
mod tests;
