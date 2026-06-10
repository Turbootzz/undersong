//! Integration tests for the real `content/core` files.
//!
//! The chart test re-transcribes doc 02 §1's compact 2×/½×/0× lists,
//! independently of the fully explicit RON matrix, so the two
//! transcriptions cross-validate each other: a slip in either one shows
//! up as a mismatch here.

use std::path::{Path, PathBuf};

use data::{load_core, validate_core};
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
