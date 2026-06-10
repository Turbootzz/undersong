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

#[test]
fn natures_follow_the_doc_grid() {
    let content = load_core(&content_root()).expect("content/core must parse");
    let keys = &content.natures.name_keys;
    assert_eq!(keys.len(), 25);
    // The five diagonal (neutral) temperaments from the doc 02 §3 grid,
    // at indices where n / 5 == n % 5.
    for (index, name) in [
        (0, "nature.marcato"),
        (6, "nature.fermo"),
        (12, "nature.lucido"),
        (18, "nature.calmo"),
        (24, "nature.moderato"),
    ] {
        assert_eq!(keys[index], name, "neutral diagonal at index {index}");
    }
}
