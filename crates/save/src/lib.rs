//! Save format, versioning, and backends (docs/03-ARCHITECTURE.md §4).
//!
//! Saves are human-readable RON on purpose during development. The
//! `SaveBackend` trait exists from day one so the P7 web build is a
//! backend (LocalStorage), not a refactor. Every format change adds a
//! `migrate_vN_to_vN+1` and keeps fixture saves loading forever.

#![forbid(unsafe_code)]

pub mod backend;
pub mod model;

pub use backend::{FsBackend, MemBackend, SaveBackend};
#[cfg(target_arch = "wasm32")]
pub use backend::LocalStorageBackend;
pub use model::{
    Facing, Position, SAVE_VERSION, SaveError, SaveFile, SaveHeader, Settings, SlotId,
};

use model::migrate;

/// Slot file names: three manual slots plus the rotating autosave
/// (doc 03 §4).
fn slot_name(slot: SlotId) -> &'static str {
    match slot {
        SlotId::Slot1 => "slot1.ron",
        SlotId::Slot2 => "slot2.ron",
        SlotId::Slot3 => "slot3.ron",
        SlotId::Auto => "auto.ron",
    }
}

/// Saves to a slot through any backend.
pub fn save(backend: &mut dyn SaveBackend, slot: SlotId, file: &SaveFile) -> Result<(), SaveError> {
    let text = ron::ser::to_string_pretty(file, ron::ser::PrettyConfig::default())
        .map_err(|e| SaveError::Encode(e.to_string()))?;
    backend.write(slot_name(slot), &text)
}

/// Loads a slot; `Ok(None)` when empty. Migrates older versions.
pub fn load(backend: &dyn SaveBackend, slot: SlotId) -> Result<Option<SaveFile>, SaveError> {
    let Some(text) = backend.read(slot_name(slot))? else {
        return Ok(None);
    };
    let file = migrate(&text)?;
    Ok(Some(file))
}

/// Header peek for the save-select screen: parses just enough.
pub fn peek_header(
    backend: &dyn SaveBackend,
    slot: SlotId,
) -> Result<Option<SaveHeader>, SaveError> {
    Ok(load(backend, slot)?.map(|f| f.header))
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use undersong_core::individual::{Individual, LearnedMove};
    use undersong_core::species::StatSpread;

    use super::*;
    use crate::model::{Facing, Position, Settings};

    fn sample() -> SaveFile {
        let mut flags = BTreeSet::new();
        flags.insert("met.fisher_old".to_string());
        let mut vars = BTreeMap::new();
        vars.insert("story.act".to_string(), 1);
        let mut bag = BTreeMap::new();
        bag.insert("items".to_string(), vec![("fermata".into(), 5u32)]);

        SaveFile {
            header: SaveHeader {
                version: SAVE_VERSION,
                created_epoch_s: 1_780_000_000,
                playtime_s: 4_321,
                region: "cantorel".into(),
                badge_bits: 0b0000_0001,
                score_pct: 4,
            },
            player: crate::model::Player {
                name: "Junie".into(),
                money: 3_000,
                position: Position {
                    map: "debug_rehearsal".into(),
                    x: 5,
                    y: 7,
                    facing: Facing::Down,
                },
                settings: Settings::default(),
            },
            party: vec![Individual {
                species: "testbed_ember".into(),
                level: 12,
                exp: 1_728,
                ivs: StatSpread {
                    hp: 31,
                    atk: 12,
                    def: 18,
                    spa: 31,
                    spd: 22,
                    spe: 31,
                },
                evs: StatSpread {
                    hp: 0,
                    atk: 0,
                    def: 0,
                    spa: 8,
                    spd: 0,
                    spe: 4,
                },
                nature: 10,
                ability_slot: 0,
                moves: vec![LearnedMove {
                    id: "ember_note".into(),
                    pp: 25,
                    pp_ups: 0,
                }],
                status: None,
                held_item: None,
                friendship: 90,
                keyshifted: false,
                ot: "Junie".into(),
                nickname: None,
                hp: None,
                uses_hidden_ability: false,
            }],
            boxes: vec![vec![]],
            bag,
            flags,
            vars,
            counters: BTreeMap::from([("steps".to_string(), 812u64)]),
            world_seed: 0x00A1_10F5,
            heal_point: Some(("debug_rehearsal".into(), (5, 2))),
        }
    }

    #[test]
    fn round_trips_through_memory_backend() {
        let mut backend = MemBackend::default();
        let file = sample();
        save(&mut backend, SlotId::Slot1, &file).expect("save");
        let loaded = load(&backend, SlotId::Slot1)
            .expect("load")
            .expect("present");
        assert_eq!(loaded, file);
    }

    #[test]
    fn empty_slots_load_as_none() {
        let backend = MemBackend::default();
        assert!(load(&backend, SlotId::Slot2).expect("load").is_none());
    }

    #[test]
    fn slots_are_independent_and_autosave_rotates() {
        let mut backend = MemBackend::default();
        let mut a = sample();
        a.player.name = "A".into();
        let mut b = sample();
        b.player.name = "B".into();
        save(&mut backend, SlotId::Slot1, &a).expect("save");
        save(&mut backend, SlotId::Auto, &b).expect("save");
        // Autosave overwrites in place.
        let mut b2 = b.clone();
        b2.header.playtime_s += 60;
        save(&mut backend, SlotId::Auto, &b2).expect("save");

        assert_eq!(
            load(&backend, SlotId::Slot1).unwrap().unwrap().player.name,
            "A"
        );
        assert_eq!(
            load(&backend, SlotId::Auto)
                .unwrap()
                .unwrap()
                .header
                .playtime_s,
            b.header.playtime_s + 60
        );
        assert!(load(&backend, SlotId::Slot3).unwrap().is_none());
    }

    #[test]
    fn header_peek_matches_full_load() {
        let mut backend = MemBackend::default();
        save(&mut backend, SlotId::Slot3, &sample()).expect("save");
        let header = peek_header(&backend, SlotId::Slot3).unwrap().unwrap();
        assert_eq!(header.badge_bits, 1);
        assert_eq!(header.region, "cantorel");
    }

    #[test]
    fn future_versions_are_rejected_loudly() {
        let mut file = sample();
        file.header.version = SAVE_VERSION + 1;
        let text = ron::to_string(&file).expect("serialize");
        let mut backend = MemBackend::default();
        backend.write("slot1.ron", &text).expect("write");
        let err = load(&backend, SlotId::Slot1).expect_err("must refuse");
        assert!(matches!(err, SaveError::UnsupportedVersion(_)));
    }

    #[test]
    fn corrupt_saves_error_instead_of_panicking() {
        let mut backend = MemBackend::default();
        backend
            .write("slot1.ron", "not ron at all {")
            .expect("write");
        assert!(load(&backend, SlotId::Slot1).is_err());
    }

    #[test]
    fn v1_fixture_remains_loadable_forever() {
        // The committed fixture pins the v1 wire format (doc 03 §4:
        // never break old saves). It is READ-ONLY — new versions get
        // their own fixture below.
        let path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/slot.v1.ron");
        let text = std::fs::read_to_string(&path).expect("fixture slot.v1.ron must stay committed");
        let loaded = migrate(&text).expect("v1 fixture loads in every future build");
        assert_eq!(loaded.header.version, SAVE_VERSION, "migrated to current");
        assert_eq!(
            loaded.heal_point, None,
            "v1 had no rest point — entry fallback"
        );
        assert_eq!(loaded.player.name, "Junie");
        assert_eq!(loaded.party.len(), 1);
        assert!(loaded.flags.contains("met.fisher_old"));
    }

    #[test]
    fn v2_fixture_round_trips() {
        // Current-version wire pin. Regenerate when creating a NEW
        // version's fixture: UPDATE_FIXTURES=1 cargo test -p save.
        let path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/slot.v2.ron");
        if std::env::var_os("UPDATE_FIXTURES").is_some() {
            let text = ron::ser::to_string_pretty(&sample(), ron::ser::PrettyConfig::default())
                .expect("serialize");
            std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
            std::fs::write(&path, text).expect("write fixture");
        }
        let text = std::fs::read_to_string(&path).expect("fixture slot.v2.ron must stay committed");
        let loaded = migrate(&text).expect("v2 fixture loads");
        assert_eq!(loaded.header.version, SAVE_VERSION);
        assert_eq!(
            loaded.heal_point,
            Some(("debug_rehearsal".to_string(), (5, 2)))
        );
    }

    #[test]
    fn fs_backend_round_trips_in_a_temp_dir() {
        let dir = std::env::temp_dir().join(format!("undersong-save-test-{}", std::process::id()));
        let mut backend = FsBackend::at(dir.clone());
        let file = sample();
        save(&mut backend, SlotId::Slot1, &file).expect("save");
        let loaded = load(&backend, SlotId::Slot1)
            .expect("load")
            .expect("present");
        assert_eq!(loaded, file);
        std::fs::remove_dir_all(dir).ok();
    }
}
