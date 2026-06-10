//! Integration tests for the real `content/core` files.
//!
//! The chart test re-transcribes doc 02 §1's compact 2×/½×/0× lists,
//! independently of the fully explicit RON matrix, so the two
//! transcriptions cross-validate each other: a slip in either one shows
//! up as a mismatch here.

use std::path::{Path, PathBuf};

use data::{load_core, validate_core};
use undersong_core::moves::{Ailment, Effect, EffectTarget, Frac, MoveCategory};
use undersong_core::stats::Stat;
use undersong_core::types::{
    Eff,
    Type::{
        self, Alloy, Bloom, Ember, Feral, Frost, Gale, Phantom, Resonant, Stone, Tide, Venom, Volt,
    },
};

fn content_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content")
}

#[test]
fn repo_core_content_loads_and_validates_clean() {
    let content = load_core(&content_root()).expect("content/core must parse");
    let findings = validate_core(&content);
    assert!(
        findings.is_empty(),
        "validation findings:\n{}",
        findings
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n")
    );
}

/// One doc-table row: (attacker, 2× list, ½× list, 0× list).
type DocRow = (Type, &'static [Type], &'static [Type], &'static [Type]);

/// Doc 02 §1, transcribed as compact effectiveness lists;
/// everything unlisted is neutral.
const DOC_TABLE: &[DocRow] = &[
    (Feral, &[], &[Stone, Alloy], &[Phantom]),
    (Ember, &[Bloom, Frost, Alloy], &[Ember, Tide, Stone], &[]),
    (Tide, &[Ember, Stone, Alloy], &[Tide, Bloom], &[]),
    (Bloom, &[Tide, Stone], &[Bloom, Ember, Gale, Venom], &[]),
    (Volt, &[Tide, Gale], &[Volt, Bloom], &[Stone]),
    (Gale, &[Bloom, Venom], &[Volt, Stone, Alloy], &[]),
    (
        Stone,
        &[Ember, Volt, Gale, Frost, Venom],
        &[Bloom, Alloy],
        &[],
    ),
    (
        Frost,
        &[Bloom, Gale, Stone],
        &[Frost, Tide, Alloy, Ember],
        &[],
    ),
    (
        Venom,
        &[Bloom, Resonant],
        &[Stone, Venom, Phantom],
        &[Alloy],
    ),
    (Phantom, &[Phantom, Resonant], &[], &[Feral]),
    (Alloy, &[Frost, Stone], &[Ember, Tide, Volt, Alloy], &[]),
    (Resonant, &[Phantom, Frost], &[Resonant, Venom], &[]),
];

#[test]
fn chart_matches_design_doc_table() {
    let content = load_core(&content_root()).expect("content/core must parse");
    assert_eq!(DOC_TABLE.len(), Type::COUNT, "one row per attacker");
    let attackers: std::collections::BTreeSet<Type> =
        DOC_TABLE.iter().map(|&(attacker, ..)| attacker).collect();
    assert_eq!(
        attackers.len(),
        Type::COUNT,
        "DOC_TABLE attackers must be distinct, or a duplicated row would \
         leave one attacker's twelve cells value-unchecked"
    );

    for &(attacker, double, half, zero) in DOC_TABLE {
        for defender in Type::ALL {
            let expected = if double.contains(&defender) {
                Eff::Double
            } else if half.contains(&defender) {
                Eff::Half
            } else if zero.contains(&defender) {
                Eff::Zero
            } else {
                Eff::Neutral
            };
            assert_eq!(
                content.typechart.eff(attacker, defender),
                Some(expected),
                "{attacker:?} attacking {defender:?}"
            );
        }
    }
}

/// Doc 02 §3's 5×5 grid in canonical index order (row = boosted stat,
/// column = hindered, over [atk, def, spa, spd, spe]). Index order is
/// mechanics law — a transposed key would silently change which stat a
/// temperament boosts once P1 wires up the multiplier — so all 25 entries
/// are asserted, not just the diagonal.
const DOC_GRID: [&str; 25] = [
    "nature.marcato",
    "nature.forte",
    "nature.bravura",
    "nature.sforzando",
    "nature.pesante",
    "nature.tenuto",
    "nature.fermo",
    "nature.solido",
    "nature.grave",
    "nature.largo",
    "nature.brillante",
    "nature.estro",
    "nature.lucido",
    "nature.acuto",
    "nature.rubato",
    "nature.placido",
    "nature.sereno",
    "nature.velato",
    "nature.calmo",
    "nature.adagio",
    "nature.vivace",
    "nature.presto",
    "nature.agile",
    "nature.scherzo",
    "nature.moderato",
];

/// Doc 02 §6's canon-move table, re-transcribed:
/// (id, type, category, power, accuracy, pp, priority).
const DOC_MOVES: [(&str, Type, MoveCategory, u16, u8, u8, i8); 18] = [
    ("tackle", Feral, MoveCategory::Physical, 40, 100, 35, 0),
    ("quick_step", Feral, MoveCategory::Physical, 40, 100, 30, 1),
    ("ember_note", Ember, MoveCategory::Special, 40, 100, 25, 0),
    ("flare_brass", Ember, MoveCategory::Special, 90, 100, 15, 0),
    ("ripple", Tide, MoveCategory::Special, 40, 100, 25, 0),
    ("undertow", Tide, MoveCategory::Special, 80, 100, 15, 0),
    ("leaf_pick", Bloom, MoveCategory::Physical, 55, 95, 25, 0),
    ("root_chord", Bloom, MoveCategory::Special, 75, 100, 10, 0),
    ("volt_pluck", Volt, MoveCategory::Special, 65, 100, 20, 0),
    ("gale_riff", Gale, MoveCategory::Special, 60, 100, 20, 0),
    ("stone_toll", Stone, MoveCategory::Physical, 75, 90, 15, 0),
    ("venom_trill", Venom, MoveCategory::Special, 65, 100, 20, 0),
    (
        "phantom_rest",
        Phantom,
        MoveCategory::Special,
        80,
        100,
        15,
        0,
    ),
    ("alloy_clang", Alloy, MoveCategory::Physical, 80, 100, 15, 0),
    ("frost_lull", Frost, MoveCategory::Status, 0, 75, 10, 0),
    ("resonate", Resonant, MoveCategory::Special, 85, 100, 10, 0),
    ("crescendo", Resonant, MoveCategory::Status, 0, 0, 20, 0),
    ("dampen", Feral, MoveCategory::Status, 0, 100, 20, 0),
];

#[test]
fn canon_moves_match_design_doc_table() {
    let content = load_core(&content_root()).expect("content/core must parse");
    let moves = &content.moves;
    assert_eq!(
        moves.moves.len(),
        DOC_MOVES.len(),
        "exactly the 18 canon moves"
    );

    for (id, ty, category, power, accuracy, pp, priority) in DOC_MOVES {
        let m = moves
            .get(&id.into())
            .unwrap_or_else(|| panic!("move `{id}` present"));
        assert_eq!(m.r#type, ty, "{id} type");
        assert_eq!(m.category, category, "{id} category");
        assert_eq!(m.power, power, "{id} power");
        assert_eq!(m.accuracy, accuracy, "{id} accuracy");
        assert_eq!(m.pp, pp, "{id} pp");
        assert_eq!(m.priority, priority, "{id} priority");
    }

    // Flags and effects, per the doc rows that carry them.
    let get = |id: &str| moves.get(&id.into()).expect("present");
    for sound in [
        "ember_note",
        "flare_brass",
        "gale_riff",
        "venom_trill",
        "alloy_clang",
        "frost_lull",
        "resonate",
        "dampen",
    ] {
        assert!(get(sound).flags.sound, "{sound} is a sound move");
    }
    for contact in ["tackle", "quick_step", "leaf_pick"] {
        assert!(get(contact).flags.contact, "{contact} is contact");
    }
    assert!(get("leaf_pick").flags.high_crit);
    assert!(get("resonate").flags.ignore_evasion);
    assert_eq!(
        get("ember_note").effects,
        vec![Effect::Status {
            ailment: Ailment::Burn,
            chance: 10
        }]
    );
    assert_eq!(
        get("flare_brass").effects,
        vec![Effect::Status {
            ailment: Ailment::Burn,
            chance: 10
        }]
    );
    // Absence is law too: the doc's effect-free rows must stay empty, and
    // the sound-move set must match the doc exactly (no stray flags).
    for effect_free in [
        "tackle",
        "quick_step",
        "ripple",
        "leaf_pick",
        "gale_riff",
        "stone_toll",
        "phantom_rest",
        "resonate",
    ] {
        assert!(
            get(effect_free).effects.is_empty(),
            "{effect_free} must carry no effects per doc 02 §6"
        );
    }
    let doc_sound: std::collections::BTreeSet<&str> = [
        "ember_note",
        "flare_brass",
        "gale_riff",
        "venom_trill",
        "alloy_clang",
        "frost_lull",
        "resonate",
        "dampen",
    ]
    .into();
    let actual_sound: std::collections::BTreeSet<&str> = moves
        .moves
        .iter()
        .filter(|m| m.flags.sound)
        .map(|m| m.id.as_str())
        .collect();
    assert_eq!(
        actual_sound, doc_sound,
        "sound flag set matches the doc table"
    );
    assert_eq!(
        get("undertow").effects,
        vec![Effect::StatStage {
            target: EffectTarget::Target,
            stat: Stat::Spe,
            delta: -1,
            chance: 20
        }]
    );
    assert_eq!(
        get("root_chord").effects,
        vec![Effect::Drain { frac: Frac(1, 2) }]
    );
    assert_eq!(
        get("volt_pluck").effects,
        vec![Effect::Status {
            ailment: Ailment::Paralysis,
            chance: 10
        }]
    );
    assert_eq!(
        get("venom_trill").effects,
        vec![Effect::Status {
            ailment: Ailment::Poison,
            chance: 30
        }]
    );
    assert_eq!(
        get("alloy_clang").effects,
        vec![Effect::StatStage {
            target: EffectTarget::User,
            stat: Stat::Def,
            delta: 1,
            chance: 10
        }]
    );
    assert_eq!(
        get("frost_lull").effects,
        vec![Effect::Status {
            ailment: Ailment::Sleep,
            chance: 100
        }]
    );
    assert_eq!(
        get("crescendo").effects,
        vec![
            Effect::StatStage {
                target: EffectTarget::User,
                stat: Stat::Spa,
                delta: 1,
                chance: 100
            },
            Effect::StatStage {
                target: EffectTarget::User,
                stat: Stat::Spe,
                delta: 1,
                chance: 100
            },
        ]
    );
    assert_eq!(
        get("dampen").effects,
        vec![Effect::StatStage {
            target: EffectTarget::Target,
            stat: Stat::Atk,
            delta: -1,
            chance: 100
        }]
    );
}

#[test]
fn natures_follow_the_doc_grid() {
    let content = load_core(&content_root()).expect("content/core must parse");
    let keys = &content.natures.name_keys;
    assert_eq!(keys.len(), DOC_GRID.len());
    for (index, expected) in DOC_GRID.iter().enumerate() {
        assert_eq!(
            keys[index],
            *expected,
            "temperament at index {index} (boost {}, hinder {})",
            index / 5,
            index % 5
        );
    }
}
