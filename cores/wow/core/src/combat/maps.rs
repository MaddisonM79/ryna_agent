//! Which maps are instances, from the client's own `Map` table (its `InstanceType` column: 0 the
//! open world, 1 a dungeon, 2 a raid, 3 a battleground), Forever build 1.60.1.70790, as the DB2
//! export at wago.tools serves it. `ZONE_CHANGE`'s first field is this map id (36 inside The
//! Deadmines, the same id `ENCOUNTER_START` carries last; 0 and 1 for the two continents).
//!
//! Why a table: Forever writes the last field of `ZONE_CHANGE` (the difficulty) as 0 when the
//! player zones into a dungeon and only writes the real one after a `/reload` inside it, and keeps
//! the old one on the way out (`ZONE_CHANGE,0,"The Deadmines",1`), so that field alone can't say
//! whether a pull is inside an instance. Seen in every Deadmines run in the logs on disk,
//! 2026-11-12 and 14. A map this table doesn't know (a newer build) falls back to that field, and
//! to the maps an `ENCOUNTER_START` has named.

/// `(map id, InstanceType)`, sorted by id.
const MAPS: &[(i64, u8)] = &[
    (0, 0),    // Eastern Kingdoms
    (1, 0),    // Kalimdor
    (13, 1),   // Test Dungeon
    (25, 0),   // Scott Test
    (29, 1),   // CashTest
    (30, 3),   // Alterac Valley
    (33, 1),   // Shadowfang Keep
    (34, 1),   // Stormwind Stockade
    (35, 0),   // <unused>StormwindPrison
    (36, 1),   // Deadmines
    (37, 0),   // Azshara Crater
    (42, 0),   // Collin's Test
    (43, 1),   // Wailing Caverns
    (44, 1),   // <unused> Monastery
    (47, 1),   // Razorfen Kraul
    (48, 1),   // Blackfathom Deeps
    (70, 1),   // Uldaman
    (90, 1),   // Gnomeregan
    (109, 1),  // Sunken Temple
    (129, 1),  // Razorfen Downs
    (169, 2),  // Emerald Dream
    (189, 1),  // Scarlet Monastery
    (209, 1),  // Zul'Farrak
    (229, 1),  // Blackrock Spire
    (230, 1),  // Blackrock Depths
    (249, 2),  // Onyxia's Lair
    (269, 1),  // Caverns of Time
    (289, 1),  // Scholomance
    (309, 2),  // Zul'Gurub
    (329, 1),  // Stratholme
    (349, 1),  // Maraudon
    (369, 0),  // Deeprun Tram
    (389, 1),  // Ragefire Chasm
    (409, 2),  // Molten Core
    (429, 1),  // Dire Maul
    (449, 0),  // Alliance PVP Barracks
    (450, 0),  // Horde PVP Barracks
    (469, 2),  // Blackwing Lair
    (489, 3),  // Warsong Gulch
    (509, 2),  // Ruins of Ahn'Qiraj
    (529, 3),  // Arathi Basin
    (531, 2),  // Ahn'Qiraj Temple
    (533, 2),  // Naxxramas
    (2720, 1), // The Searing Basin
    (2784, 1), // Demon Fall Canyon
    (2789, 2), // The Tainted Scar
    (2791, 2), // Storm Cliffs
    (2804, 2), // The Crystal Vale
    (2806, 1), // Shadow Hold
    (2807, 1), // Burning of Andorhal
    (2817, 1), // Starfall Barrow Den
    (2832, 2), // Nightmare Grove
    (2853, 1), // Deadwind Pass
    (2856, 2), // Scarlet Enclave
    (2868, 0), // Eastern Plaguelands (Scarlet Raid Phase)
    (2875, 1), // Karazhan Crypts
    (2902, 1), // The Scarab Dais
    (2921, 1), // Naxxramas
    (2959, 1), // City of Dalaran
    (2980, 0), // Dalaran City
    (2991, 0), // Zephras Isle
    (2995, 0), // Hyjal Crater
    (2996, 0), // Warsong Gulch (Winter)
    (2997, 3), // Darkspear Islands
    (2998, 1), // Excavation Site: Wetlands
    (2999, 1), // Ruins of Lordaeron
    (3005, 3), // Battle for Gilneas
    (3021, 0), // Eastern Kingdoms Preserved
    (3065, 1), // The Hall of Thanes
    (3104, 0), // nothing to see here
    (3109, 1), // Manor Mistmantle
    (3217, 0), // Dalaran Skycuter
    (3218, 0), // Mulgore Skycuter
];

/// The map's `InstanceType`, or None for a map the table doesn't have.
pub fn instance_type(map_id: i64) -> Option<u8> {
    MAPS.binary_search_by_key(&map_id, |(id, _)| *id)
        .ok()
        .map(|i| MAPS[i].1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_is_sorted_and_knows_the_logs_maps() {
        assert!(MAPS.windows(2).all(|w| w[0].0 < w[1].0));
        // The maps the logs on disk name in ZONE_CHANGE (2026-11-06 to 14).
        assert_eq!(instance_type(0), Some(0));
        assert_eq!(instance_type(1), Some(0));
        assert_eq!(instance_type(2991), Some(0));
        assert_eq!(instance_type(369), Some(0));
        assert_eq!(instance_type(36), Some(1));
        assert_eq!(instance_type(249), Some(2));
        assert_eq!(instance_type(4242), None);
    }
}
