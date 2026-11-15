//! The Fights file against synthetic fights only: every name, GUID and number here is made up.

use std::collections::HashMap;

use serde_json::{json, Value};

use super::*;
use crate::combat::digest::{Header, SpellMeta};
use crate::combat::segment::FightKind;
use crate::combat::summary::Extras;
use crate::lua;

// The group: five players, a hunter's wolf (called twice), and the enemies.
const ASTER: &str = "Player-1-0000000A";
const BRYN: &str = "Player-1-0000000B";
const CORIN: &str = "Player-1-0000000C";
const DELA: &str = "Player-1-0000000D";
const EMBER: &str = "Player-1-0000000E";
const WOLF: &str = "Pet-0-1-1-1-900-00000000F1";
const WOLF_AGAIN: &str = "Pet-0-1-1-1-900-00000000F2";
const BRUTE: &str = "Creature-0-1-1-1-501-0000000B01";
const CUTTER: &str = "Creature-0-1-1-1-502-0000000B02";
const CUTTER_TWO: &str = "Creature-0-1-1-1-502-0000000B03";
const CALLER: &str = "Creature-0-1-1-1-503-0000000B04";

const FLAME: i64 = 9001;
const MEND: i64 = 9002;
const FROST: i64 = 9003;
const STORM: i64 = 9004;
const BOLT: i64 = 9005;

fn actor(name: &str, kind: &str, owner: Option<&str>) -> ActorMeta {
    ActorMeta {
        name: Some(name.into()),
        kind: kind.into(),
        class: None,
        owner: owner.map(str::to_string),
    }
}

fn header() -> Header {
    let mut h = Header::default();
    for (g, n) in [
        (ASTER, "Aster-Vale"),
        (BRYN, "Bryn-Holloway"),
        (CORIN, "Corin-Ashby"),
        (DELA, "Dela-Marsh"),
        (EMBER, "Ember-Quill"),
    ] {
        h.actors.insert(g.into(), actor(n, "player", None));
    }
    h.actors
        .insert(WOLF.into(), actor("Wolf", "pet", Some(EMBER)));
    h.actors
        .insert(WOLF_AGAIN.into(), actor("Wolf", "pet", Some(EMBER)));
    h.actors
        .insert(BRUTE.into(), actor("Harbor Brute", "npc", None));
    h.actors
        .insert(CUTTER.into(), actor("Dockside Cutter", "npc", None));
    h.actors
        .insert(CUTTER_TWO.into(), actor("Dockside Cutter", "npc", None));
    h.actors
        .insert(CALLER.into(), actor("Tide Caller", "npc", None));
    for (id, name) in [
        (1, "Melee"),
        (FLAME, "Flame Lash"),
        (MEND, "Mending Light"),
        (FROST, "Frost Needle"),
        (STORM, "Hail Storm"),
        (BOLT, "Shadow Dart"),
    ] {
        h.spells.insert(
            id,
            SpellMeta {
                name: name.into(),
                school: 1,
            },
        );
    }
    h
}

fn ev(t: i64, e: &str, s: &str, d: &str, sp: i64, a: i64) -> DigestEvent {
    DigestEvent {
        t,
        e: e.into(),
        s: (!s.is_empty()).then(|| s.to_string()),
        d: Some(d.into()),
        sp: (e != "X").then_some(sp),
        a: (e != "X" && e != "M").then_some(a),
        o: None,
        sc: None,
        f: None,
        m: (e == "M").then(|| "DODGE".to_string()),
        twin: false,
    }
}

fn hit(t: i64, s: &str, d: &str, sp: i64, a: i64) -> DigestEvent {
    ev(t, "D", s, d, sp, a)
}

fn died(t: i64, d: &str) -> DigestEvent {
    ev(t, "X", "", d, 0, 0)
}

fn fight(kind: FightKind, length: i64, digest: Vec<DigestEvent>) -> Fight {
    Fight {
        kind,
        encounter_id: None,
        name: (kind == FightKind::Boss).then(|| "Captain Gull".to_string()),
        zone: Some("Saltmire Docks".into()),
        instance: Some("Saltmire Docks".into()),
        difficulty: None,
        ui_map_id: None,
        started_at: 1_800_000_000_000,
        ended_at: 1_800_000_000_000 + length,
        success: (kind == FightKind::Boss).then_some(true),
        pulls: 1,
        advanced: true,
        events: digest.len() as u64,
        header: header(),
        digest,
        extras: Extras::default(),
        raw: Vec::new(),
        realm_id: Some(1),
        start_offset: 0,
        logger: Some(DELA.into()),
    }
}

fn classes() -> HashMap<String, String> {
    // The app knows the logger's own account's characters (here two of them).
    HashMap::from([
        (DELA.to_string(), "PRIEST".to_string()),
        (ASTER.to_string(), "SHAMAN".to_string()),
    ])
}

/// The agreed mockup's fight: an early heal, the group opens with a spell at 0:08, an enemy answers
/// on the healer at 0:10, three deaths.
fn mockup() -> Fight {
    let mut d = vec![
        ev(0, "H", DELA, ASTER, MEND, 40),
        hit(8000, ASTER, BRUTE, FLAME, 120),
        hit(10_000, CUTTER, DELA, 1, 60),
        hit(10_500, BRUTE, ASTER, 1, 30),
        hit(11_000, BRYN, BRUTE, BOLT, 200),
        hit(12_000, BRUTE, ASTER, 1, 30),
        hit(12_400, EMBER, BRUTE, 1, 80),
        hit(13_000, WOLF, BRUTE, 1, 25),
        hit(14_000, CUTTER, WOLF, 1, 20),
        hit(15_000, BRUTE, EMBER, 1, 40),
        hit(16_000, BRUTE, EMBER, 1, 40),
        hit(17_000, CORIN, CUTTER, 1, 90),
        hit(18_000, CUTTER, DELA, 1, 70),
        hit(19_000, BRUTE, CORIN, 1, 50),
        hit(20_000, BRUTE, CORIN, 1, 50),
        hit(21_000, CUTTER, DELA, 1, 80),
        hit(24_000, CUTTER, DELA, 1, 90),
        hit(25_000, CUTTER, DELA, 1, 300),
        died(26_000, DELA),
        hit(27_000, CUTTER, ASTER, 1, 200),
        hit(28_500, CUTTER, ASTER, 1, 400),
        died(29_000, ASTER),
        hit(30_000, BRUTE, CORIN, 1, 300),
        hit(31_500, BRUTE, CORIN, 1, 400),
        died(32_000, CORIN),
        hit(33_000, EMBER, BRUTE, 1, 500),
        hit(34_000, BRUTE, EMBER, 1, 100),
        hit(35_000, EMBER, BRUTE, 1, 500),
        died(35_500, BRUTE),
    ];
    // The wolf, dismissed and called again: a new GUID with the same name.
    d.push(hit(35_600, CUTTER, WOLF_AGAIN, 1, 35));
    d.push(hit(35_700, WOLF_AGAIN, CUTTER, 1, 15));
    d.sort_by_key(|e| e.t);
    fight(FightKind::Trash, 36_000, d)
}

#[test]
fn the_mockup_s_fight_tells_its_story() {
    let v = view(&mockup(), &classes());
    assert_eq!(v.kind, "trash");
    assert_eq!(v.zone.as_deref(), Some("Saltmire Docks"));
    assert_eq!((v.steps, v.step, v.length), (36, 1000, 36_000));
    // The first blow is the group's spell at 0:08, not the heal at 0:00.
    let b = v.first.as_ref().unwrap();
    assert_eq!(b.by, "group");
    assert_eq!(b.who, "Aster Vale");
    assert_eq!(b.with.as_deref(), Some("Flame Lash"));
    assert_eq!(b.on, "Harbor Brute");
    assert_eq!(b.on_nth, None, "one Harbor Brute");
    assert_eq!(b.t, 8000);
    assert!(!b.miss && !b.close);
    // The enemy's answer: a swing on the healer at 0:10.
    let a = b.answer.as_ref().unwrap();
    assert_eq!(a.by, "enemy");
    assert_eq!(a.who, "Dockside Cutter");
    assert_eq!(
        a.who_nth, None,
        "the other Dockside Cutter never fought: one of its name"
    );
    assert_eq!(a.on, "Dela Marsh");
    assert_eq!(a.with, None, "a melee swing names no spell");
    assert_eq!(a.t, 10_000);
    // Deaths in order, each with its row.
    let deaths: Vec<(&str, i64)> = v.deaths.iter().map(|d| (d.name.as_str(), d.t)).collect();
    assert_eq!(
        deaths,
        vec![
            ("Dela Marsh", 26_000),
            ("Aster Vale", 29_000),
            ("Corin Ashby", 32_000)
        ]
    );
    for d in &v.deaths {
        assert_eq!(v.players[d.row - 1].name, d.name);
    }
    // Rows: the first blow's player first, then by damage done; the wolf under its owner as one
    // row (both GUIDs), holding what it took; its damage is in its owner's row.
    let names: Vec<&str> = v.players.iter().map(|r| r.name.as_str()).collect();
    assert_eq!(
        names,
        vec![
            "Aster Vale",
            "Ember Quill",
            "Wolf",
            "Bryn Holloway",
            "Corin Ashby",
            "Dela Marsh"
        ]
    );
    let wolf = &v.players[2];
    assert_eq!(wolf.pet, 2);
    assert_eq!(wolf.taken, 20 + 35);
    assert_eq!(wolf.done, 0);
    assert_eq!(v.players[1].done, 80 + 25 + 500 + 500 + 15);
    // The class where the app knows it, none where the log alone can't say.
    assert_eq!(v.players[0].class.as_deref(), Some("SHAMAN"));
    assert_eq!(v.players[5].class.as_deref(), Some("PRIEST"));
    assert_eq!(v.players[3].class, None);
    // The healer's healing, and the logger's own numbers for the list.
    assert_eq!(v.players[5].heal, 40);
    assert_eq!(
        v.me,
        Some(Mine {
            done: 0,
            taken: 60 + 70 + 80 + 90 + 300,
            heal: 40,
            died: Some(26_000)
        })
    );
    // Strips: one step a second; done on one scale, taken on another, shared by every row.
    let aster = &v.players[0];
    assert_eq!(aster.done_steps.len(), 9, "trailing zeros left out");
    assert!(aster.done_steps[8] > 0 && aster.done_steps[..8].iter().all(|s| *s == 0));
    // Ember's last step holds the new wolf's 15 too: 515, the biggest done step.
    assert_eq!(v.players[1].done_steps[35], 100, "the biggest done step");
    assert_eq!(v.players[1].done_steps[33], 97, "500 of 515");
    assert_eq!(v.players[1].done_steps[12], 16, "80 of 515");
    let corin = &v.players[4];
    assert_eq!(corin.taken_steps[31], 100, "the biggest taken step");
    assert_eq!(corin.taken_steps[19], 13, "50 of 400");
    // Enemies: the most taken plus done first; whom each was attacking, in order.
    let brute = &v.enemies[0];
    assert_eq!(brute.name, "Harbor Brute");
    assert_eq!(brute.taken, 120 + 200 + 80 + 25 + 500 + 500);
    let whom: Vec<&str> = brute
        .targets
        .iter()
        .map(|s| v.players[s.row - 1].name.as_str())
        .collect();
    assert_eq!(
        whom,
        vec!["Aster Vale", "Ember Quill", "Corin Ashby", "Ember Quill"]
    );
    // Aster from 10.5 s to the last swing at 12 s plus 2 s, cut at the turn to Ember (15 s).
    assert_eq!((brute.targets[0].from, brute.targets[0].ms), (10_500, 3500));
    let cutter = v
        .enemies
        .iter()
        .find(|e| e.name == "Dockside Cutter")
        .unwrap();
    assert_eq!(cutter.nth, None);
    let whom: Vec<&str> = cutter
        .targets
        .iter()
        .map(|s| v.players[s.row - 1].name.as_str())
        .collect();
    assert_eq!(
        whom,
        vec!["Dela Marsh", "Wolf", "Dela Marsh", "Aster Vale", "Wolf"]
    );
    assert!(v.other_players.is_none() && v.other_enemies.is_none());
}

#[test]
fn an_enemy_that_strikes_first_has_the_first_blow_and_no_answer() {
    let d = vec![
        ev(0, "H", DELA, DELA, MEND, 10),
        ev(2000, "M", CUTTER, CORIN, 1, 0),
        hit(2100, CORIN, CUTTER, 1, 50),
        hit(3000, CUTTER, CORIN, 1, 20),
    ];
    let v = view(&fight(FightKind::Trash, 4000, d), &classes());
    let b = v.first.unwrap();
    assert_eq!(b.by, "enemy");
    assert_eq!(b.who, "Dockside Cutter");
    assert_eq!(b.on, "Corin Ashby");
    assert!(b.miss, "a dodged swing still strikes first");
    assert!(b.close, "Corin's swing 100 ms later: too close to call");
    assert!(b.answer.is_none());
    // Corin, struck first, leads the rows.
    assert_eq!(v.players[0].name, "Corin Ashby");
    // No blow between the sides at all: no first blow.
    let quiet = view(
        &fight(
            FightKind::World,
            3000,
            vec![ev(0, "H", DELA, DELA, MEND, 10)],
        ),
        &classes(),
    );
    assert!(quiet.first.is_none());
}

#[test]
fn same_named_enemies_are_numbered_by_when_they_showed_up() {
    // CUTTER_TWO's GUID sorts after CUTTER's but shows up first.
    let d = vec![
        hit(1000, CUTTER_TWO, BRYN, 1, 10),
        hit(1500, BRYN, CUTTER_TWO, BOLT, 900),
        hit(2000, CUTTER, BRYN, 1, 10),
        hit(2500, BRYN, CUTTER, BOLT, 100),
    ];
    let v = view(&fight(FightKind::Trash, 3000, d), &classes());
    let b = v.first.unwrap();
    assert_eq!((b.who.as_str(), b.who_nth), ("Dockside Cutter", Some(1)));
    let nth: Vec<(i64, Option<u32>)> = v.enemies.iter().map(|e| (e.taken, e.nth)).collect();
    assert_eq!(nth, vec![(900, Some(1)), (100, Some(2))]);
}

#[test]
fn an_area_spell_never_turns_an_enemy() {
    // A caster with no swings: its single-target spells say whom it is attacking; a storm landing
    // on three of the group within 100 ms says nothing.
    let mut d = vec![
        hit(0, CORIN, CALLER, 1, 10),
        hit(1000, CALLER, CORIN, FROST, 30),
        hit(3000, CALLER, CORIN, FROST, 30),
        hit(6000, CALLER, ASTER, STORM, 20),
        hit(6020, CALLER, BRYN, STORM, 20),
        hit(6050, CALLER, DELA, STORM, 20),
        hit(7000, CALLER, CORIN, FROST, 30),
    ];
    let v = view(&fight(FightKind::Trash, 9000, d.clone()), &classes());
    let caller = &v.enemies[0];
    let whom: Vec<&str> = caller
        .targets
        .iter()
        .map(|s| v.players[s.row - 1].name.as_str())
        .collect();
    assert_eq!(whom, vec!["Corin Ashby"]);
    assert_eq!((caller.targets[0].from, caller.targets[0].ms), (1000, 8000));
    // The same spell on one of them alone would have turned it.
    d.retain(|e| !(e.t == 6020 || e.t == 6050));
    let v = view(&fight(FightKind::Trash, 9000, d), &classes());
    let whom: Vec<&str> = v.enemies[0]
        .targets
        .iter()
        .map(|s| v.players[s.row - 1].name.as_str())
        .collect();
    assert_eq!(whom, vec!["Corin Ashby", "Aster Vale", "Corin Ashby"]);
    // A spell within a second of a swing doesn't decide either.
    let d = vec![
        hit(0, BRUTE, CORIN, 1, 10),
        hit(500, BRUTE, ASTER, FROST, 10),
        hit(1500, BRUTE, CORIN, 1, 10),
        hit(1600, CORIN, BRUTE, 1, 10),
    ];
    let v = view(&fight(FightKind::Trash, 3000, d), &classes());
    assert_eq!(v.enemies[0].targets.len(), 1);
}

#[test]
fn a_boss_says_its_name_and_result() {
    let d = vec![
        hit(0, ASTER, BRUTE, FLAME, 10),
        hit(2500, BRUTE, ASTER, 1, 5),
    ];
    let v = view(&fight(FightKind::Boss, 3000, d), &classes());
    assert_eq!(v.boss.as_deref(), Some("Captain Gull"));
    assert_eq!(v.result.as_deref(), Some("kill"));
    let mut f = fight(FightKind::Boss, 3000, vec![hit(0, ASTER, BRUTE, FLAME, 10)]);
    f.success = Some(false);
    assert_eq!(view(&f, &classes()).result.as_deref(), Some("wipe"));
    f.success = None;
    assert_eq!(view(&f, &classes()).result, None);
    // A long fight: 40 steps, wider than a second.
    let long = view(
        &fight(
            FightKind::World,
            600_000,
            vec![hit(599_999, ASTER, BRUTE, FLAME, 10)],
        ),
        &classes(),
    );
    assert_eq!((long.steps, long.step), (40, 15_000));
    assert_eq!(long.players[0].done_steps.len(), 40);
}

/// 30 players, each struck in turn by 12 enemies that turn 20 times, 45 deaths: past every cap.
fn crowded() -> Fight {
    let mut f = fight(FightKind::Boss, 120_000, Vec::new());
    f.header.actors.clear();
    let players: Vec<String> = (0..30).map(|i| format!("Player-1-1{i:07X}")).collect();
    let enemies: Vec<String> = (0..12)
        .map(|i| format!("Creature-0-1-1-1-600-2{i:09X}"))
        .collect();
    for (i, p) in players.iter().enumerate() {
        f.header.actors.insert(
            p.clone(),
            actor(&format!("Raider{i:02}-Longname{i:02}"), "player", None),
        );
    }
    for (i, e) in enemies.iter().enumerate() {
        f.header.actors.insert(
            e.clone(),
            actor(&format!("Deep Ward Sentinel {i}"), "npc", None),
        );
    }
    let mut d = Vec::new();
    for step in 0..120 {
        let t = step * 1000;
        for (i, p) in players.iter().enumerate() {
            let e = &enemies[(i + step as usize) % enemies.len()];
            d.push(hit(
                t + i as i64,
                p,
                e,
                BOLT,
                1000 + (i as i64 * 37 + t) % 900,
            ));
        }
        for (j, e) in enemies.iter().enumerate() {
            // Each enemy turns every 6 s, so 20 targets over the fight.
            let p = &players[(j + (step as usize / 6)) % players.len()];
            d.push(hit(
                t + 500 + j as i64,
                e,
                p,
                1,
                300 + (j as i64 * 13 + t) % 500,
            ));
        }
    }
    for (i, p) in players.iter().enumerate().take(45.min(players.len())) {
        d.push(died(100_000 + i as i64 * 100, p));
    }
    for (i, p) in players.iter().enumerate().take(15) {
        d.push(died(110_000 + i as i64 * 100, p));
    }
    d.sort_by_key(|e| e.t);
    f.digest = d;
    f.logger = Some(players[29].clone());
    f
}

#[test]
fn the_caps_hold_and_the_rest_is_folded() {
    let v = view(&crowded(), &HashMap::new());
    assert_eq!(v.players.len(), MAX_ROWS);
    let other = v.other_players.as_ref().unwrap();
    assert_eq!(other.n, 30 - MAX_ROWS);
    assert_eq!(
        v.players.iter().map(|r| r.done).sum::<i64>() + other.done,
        v.enemies.iter().map(|e| e.taken).sum::<i64>() + v.other_enemies.as_ref().unwrap().taken
    );
    assert_eq!(v.enemies.len(), MAX_ENEMIES);
    assert_eq!(v.other_enemies.as_ref().unwrap().n, 12 - MAX_ENEMIES);
    for e in &v.enemies {
        assert_eq!(e.targets.len(), MAX_SPANS);
        assert_eq!(e.more, 20 - MAX_SPANS);
    }
    assert_eq!(v.deaths.len(), MAX_DEATHS);
    // A folded player's death has no row; the logger, folded or not, still has its own numbers.
    assert!(v.deaths.iter().any(|d| d.row == 0));
    assert!(v.me.as_ref().unwrap().done > 0);
    // An attack on a folded player names them.
    assert!(v
        .enemies
        .iter()
        .flat_map(|e| &e.targets)
        .any(|s| s.row == 0 && s.name.starts_with("Raider")));
}

fn parsed(text: &str) -> Value {
    lua::parse(text).expect("the file is Lua the reader reads")["RynaData_fights"].clone()
}

#[test]
fn the_file_is_lua_of_the_documented_shape() {
    let views = vec![view(&mockup(), &classes())];
    let (text, n) = fights_lua(&views, 1_800_000_100, "wowf");
    assert_eq!(n, 1);
    assert!(text.starts_with("-- Written by the Ryna companion app"));
    assert!(text.contains("stays on this computer"));
    assert!(!text.to_lowercase().contains("threat"));
    let t = parsed(&text);
    assert_eq!(t["version"], json!(VERSION));
    assert_eq!(t["writtenAt"], json!(1_800_000_100));
    assert_eq!(t["flavor"], json!("wowf"));
    let f = &t["items"][0];
    assert_eq!(f["at"], json!(1_800_000_000));
    assert_eq!(f["length"], json!(36_000));
    assert_eq!(f["kind"], json!("trash"));
    assert_eq!(f["zone"], json!("Saltmire Docks"));
    assert_eq!(
        (f["step"].clone(), f["steps"].clone()),
        (json!(1000), json!(36))
    );
    assert_eq!(f["by"], json!(DELA));
    assert_eq!(f["me"]["died"], json!(26_000));
    assert_eq!(f["first"]["by"], json!("group"));
    assert_eq!(f["first"]["who"], json!("Aster Vale"));
    assert_eq!(f["first"]["with"], json!("Flame Lash"));
    assert_eq!(f["first"]["t"], json!(8000));
    assert_eq!(f["first"]["answer"]["who"], json!("Dockside Cutter"));
    assert!(f["first"]["answer"].get("whoNth").is_none());
    assert!(f["first"]["answer"].get("with").is_none());
    assert_eq!(f["deaths"][0], json!(["Dela Marsh", 26_000, 6]));
    assert_eq!(f["players"][2]["name"], json!("Wolf"));
    assert_eq!(f["players"][2]["pet"], json!(2));
    assert_eq!(f["players"][0]["class"], json!("SHAMAN"));
    assert!(f["players"][3].get("class").is_none());
    assert_eq!(f["players"][1]["doneSteps"][35], json!(100));
    assert_eq!(f["enemies"][0]["targets"][0], json!([1, 10_500, 3500]));
    assert_eq!(f["enemies"][0]["more"], json!(0));
    assert!(f.get("otherPlayers").is_none());
    // A name can't carry a colour code or a link.
    let mut odd = mockup();
    odd.header.actors.get_mut(ASTER).unwrap().name = Some("Aster|cffff0000-Vale\u{7}".into());
    let (text, _) = fights_lua(&[view(&odd, &classes())], 1, "wowf");
    assert_eq!(
        parsed(&text)["items"][0]["players"][0]["name"],
        json!("Astercffff0000 Vale")
    );
    // Nothing yet: a valid, empty table.
    let (empty, n) = fights_lua(&[], 1, "wowf");
    assert_eq!(n, 0);
    assert_eq!(parsed(&empty)["items"], json!([]));
}

#[test]
fn a_typical_and_a_worst_fight_s_cost_and_the_budget() {
    let typical = fight_lua(&view(&mockup(), &classes())).len();
    let worst = fight_lua(&view(&crowded(), &HashMap::new())).len();
    eprintln!("bytes: typical {typical}, worst {worst}");
    assert!(typical < 4 * 1024, "{typical}");
    assert!(worst < 16 * 1024, "{worst}");
    // Twenty of the worst: over the budget, so the oldest go and the newest stay.
    let mut views: Vec<FightView> = (0..MAX_FIGHTS as i64)
        .map(|i| {
            let mut v = view(&crowded(), &HashMap::new());
            v.started_at -= i * 1_000_000;
            v
        })
        .collect();
    let (text, n) = fights_lua(&views, 1, "wowf");
    assert!(text.len() <= BUDGET_BYTES, "{}", text.len());
    assert!((10..MAX_FIGHTS).contains(&n), "{n}");
    let items = parsed(&text)["items"].as_array().unwrap().len();
    assert_eq!(items, n);
    assert_eq!(
        parsed(&text)["items"][0]["at"],
        json!(views[0].started_at / 1000),
        "the newest first"
    );
    // Twenty typical ones fit whole.
    views = (0..MAX_FIGHTS as i64)
        .map(|i| {
            let mut v = view(&mockup(), &classes());
            v.started_at -= i * 1_000_000;
            v
        })
        .collect();
    let (text, n) = fights_lua(&views, 1, "wowf");
    assert_eq!(n, MAX_FIGHTS);
    eprintln!("bytes: twenty typical {}", text.len());
}

/// The addon's tests read this file (mods/wow/fights/test/fixtures/fights.lua): the app's own output,
/// so the two sides can't drift. `RYNA_UPDATE_FIXTURES=1 cargo test` rewrites it.
#[test]
fn the_addon_s_fixture_is_this_writer_s_output() {
    let mut boss = view(
        &fight(
            FightKind::Boss,
            4000,
            vec![
                ev(0, "M", CUTTER, CORIN, 1, 0),
                hit(2000, CORIN, CUTTER, 1, 50),
                hit(3000, CUTTER, CORIN, 1, 20),
            ],
        ),
        &classes(),
    );
    boss.started_at += 600_000;
    let views = vec![boss, view(&mockup(), &classes())];
    let (text, _) = fights_lua(&views, 1_800_000_900, "wowf");
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../mods/wow/fights/test/fixtures/fights.lua");
    if std::env::var_os("RYNA_UPDATE_FIXTURES").is_some() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, &text).unwrap();
    }
    let on_disk = std::fs::read_to_string(&path).unwrap_or_default();
    assert_eq!(
        on_disk, text,
        "the addon's fixture is stale: RYNA_UPDATE_FIXTURES=1 cargo test -p ryna-wow-core fights_data"
    );
}

#[test]
fn the_store_keeps_the_newest_and_replaces_a_fight_read_twice() {
    let dir = tempfile::tempdir().unwrap();
    let install = Path::new("/wow/_forever_");
    let mut store = Store::default();
    for i in 0..(MAX_FIGHTS as i64 + 5) {
        let mut v = view(&mockup(), &classes());
        v.started_at = 1_000 + i;
        store.add(install, v);
    }
    let kept = &store.get(install).unwrap().fights;
    assert_eq!(kept.len(), MAX_FIGHTS);
    assert_eq!(kept[0].started_at, 1_005);
    let mut again = kept[3].clone();
    again.length = 1;
    store.add(install, again);
    let kept = &store.get(install).unwrap().fights;
    assert_eq!(kept.len(), MAX_FIGHTS);
    assert_eq!(kept[3].length, 1);
    store.save(dir.path()).unwrap();
    assert_eq!(Store::load(dir.path()), store);
    // A broken file reads as empty, never an error.
    std::fs::write(dir.path().join("combat").join(STORE_FILE), "{").unwrap();
    assert_eq!(Store::load(dir.path()), Store::default());
}

// The tailer writes the file ----------------------------------------------------------------------

fn line(t: &str, body: &str) -> String {
    format!("11/12/2026 21:{t}-6  {body}\n")
}

/// A synthetic pull in a dungeon: the logger opens with a spell, the enemy swings back.
fn pull(at: &str, back: &str) -> String {
    let mut s = line(
        at,
        "SPELL_DAMAGE,Player-1-0F00C001,\"Wren-Tallow-Normal-US\",0x511,0x80000000,Creature-0-1-36-3-642-0F00D001,\"Quay Rat\",0xa48,0x80000000,403,\"Spark\",0x8,Creature-0-1-36-3-642-0F00D001,0000000000000000,0,100,1,0,1,0,0,0,0,1,1,0,1.0,1.0,291,1.0,63,30,30,-1,8,0,0,0,nil,nil,nil,ST",
    );
    s += &line(
        back,
        "SWING_DAMAGE,Creature-0-1-36-3-642-0F00D001,\"Quay Rat\",0xa48,0x80000000,Player-1-0F00C001,\"Wren-Tallow-Normal-US\",0x511,0x80000000,Creature-0-1-36-3-642-0F00D001,0000000000000000,71,71,6,0,46,0,0,0,1,0,0,0,1.0,1.0,291,5.4978,3,3,3,-1,1,0,0,0,nil,1,nil",
    );
    s
}

fn install_with(dir: &Path, log: &str, addon: bool) -> (PathBuf, PathBuf) {
    let root = dir.join("World of Warcraft");
    let p = crate::discovery::testing::product(&root, "_forever_", "wowf", &[]);
    std::fs::create_dir_all(p.join("Logs")).unwrap();
    std::fs::write(p.join("Logs/WoWCombatLog-111226_210000.txt"), log).unwrap();
    if addon {
        std::fs::create_dir_all(p.join("Interface/AddOns/Ryna")).unwrap();
    }
    (root, p)
}

#[test]
fn the_tailer_writes_the_file_when_a_fight_ends_and_at_start_up() {
    use crate::combat::tail::Tailer;
    use crate::combat::CombatOptions;
    let dir = tempfile::tempdir().unwrap();
    let mut log = line("00:00.000", "ZONE_CHANGE,36,\"The Deadmines\",0");
    log += &pull("00:01.000", "00:04.000");
    log += &pull("00:30.000", "00:33.000");
    let (root, product) = install_with(dir.path(), &log, true);
    let found = crate::discovery::discover(Some(&root));
    let state = dir.path().join("state");
    let file = product.join("Interface/AddOns/Ryna/Data").join(FILE);
    let now = 1_900_000_000;
    let mut tailer = Tailer::new(&state, CombatOptions::default());
    tailer.poll(&found, None, now, &mut |_| {});
    let t = parsed(&std::fs::read_to_string(&file).unwrap());
    let items = t["items"].as_array().unwrap();
    assert_eq!(
        items.len(),
        2,
        "both pulls, from the backfill and the read alike"
    );
    assert_eq!(items[0]["kind"], json!("trash"));
    assert_eq!(items[0]["zone"], json!("The Deadmines"));
    assert_eq!(items[0]["first"]["who"], json!("Wren Tallow"));
    assert_eq!(items[0]["first"]["answer"]["who"], json!("Quay Rat"));
    assert!(
        items[0]["at"].as_i64() > items[1]["at"].as_i64(),
        "newest first"
    );
    assert_eq!(t["writtenAt"], json!(now));
    // A new fight: the file is rewritten with it on top.
    log += &pull("01:30.000", "01:33.000");
    std::fs::write(product.join("Logs/WoWCombatLog-111226_210000.txt"), &log).unwrap();
    tailer.poll(&found, None, now + 600, &mut |_| {});
    let t = parsed(&std::fs::read_to_string(&file).unwrap());
    assert_eq!(t["items"].as_array().unwrap().len(), 3);
    // At start-up the file is written again from what's kept, without reading the logs again.
    std::fs::remove_file(&file).unwrap();
    let mut again = Tailer::new(&state, CombatOptions::default());
    again.poll(&found, None, now + 700, &mut |_| {});
    let t = parsed(&std::fs::read_to_string(&file).unwrap());
    assert_eq!(t["items"].as_array().unwrap().len(), 3);
    // An install the tailer knows only from before this file (its logs already read): the logs on
    // disk are read once for it.
    std::fs::remove_file(&file).unwrap();
    std::fs::remove_file(state.join("combat").join(STORE_FILE)).unwrap();
    let mut upgraded = Tailer::new(&state, CombatOptions::default());
    upgraded.poll(&found, None, now + 800, &mut |_| {});
    let t = parsed(&std::fs::read_to_string(&file).unwrap());
    assert_eq!(t["items"].as_array().unwrap().len(), 3);
    assert!(Store::load(&state).get(&product).unwrap().backfilled);
}

#[test]
fn no_file_without_the_addon_s_folder() {
    use crate::combat::tail::Tailer;
    use crate::combat::CombatOptions;
    let dir = tempfile::tempdir().unwrap();
    let mut log = line("00:00.000", "ZONE_CHANGE,36,\"The Deadmines\",0");
    log += &pull("00:01.000", "00:04.000");
    let (root, product) = install_with(dir.path(), &log, false);
    let found = crate::discovery::discover(Some(&root));
    let state = dir.path().join("state");
    let mut tailer = Tailer::new(&state, CombatOptions::default());
    tailer.poll(&found, None, 1_900_000_000, &mut |_| {});
    assert!(
        !product.join("Interface").exists(),
        "nothing written into the game"
    );
    // The fight is still kept for when the addon arrives.
    assert_eq!(Store::load(&state).get(&product).unwrap().fights.len(), 1);
}
