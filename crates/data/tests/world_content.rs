//! Integration tests for palette + dev maps (P2 world content).

use std::path::{Path, PathBuf};

use data::{load_maps, load_palette, load_species_pool, validate_maps, validate_palette};
use undersong_core::types::Type;

fn content_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content")
}

fn script_exists(map: &undersong_core::ids::MapId, path: &str) -> bool {
    content_root()
        .join("dev/maps")
        .join(map.as_str())
        .join("scripts")
        .join(path)
        .exists()
}

#[test]
fn palette_loads_and_validates_clean() {
    let palette = load_palette(&content_root()).expect("palette parses");
    let findings = validate_palette(&palette);
    assert!(findings.is_empty(), "{findings:?}");
    // Doc 05 §2 spot checks.
    assert_eq!(palette.ink, "#1a1822");
    assert_eq!(palette.parchment, "#f2e9d8");
    assert_eq!(palette.gilt, "#c9a227");
    assert_eq!(palette.type_colors.len(), Type::COUNT);
}

#[test]
fn dev_maps_load_and_validate_clean() {
    let maps = load_maps(&content_root().join("dev/maps")).expect("maps load");
    assert!(maps.contains_key(&"debug_rehearsal".into()));
    assert!(maps.contains_key(&"debug_annex".into()));
    let pool = load_species_pool(&content_root().join("dev/testbed.ron")).expect("pool");
    let findings = validate_maps(&maps, &pool, &script_exists);
    assert!(
        findings.is_empty(),
        "{}",
        findings
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn debug_map_geometry_is_coherent() {
    let maps = load_maps(&content_root().join("dev/maps")).expect("maps load");
    let rehearsal = &maps[&"debug_rehearsal".into()];

    // South gate tiles are walkable warp triggers.
    for x in [5u32, 6] {
        assert!(!rehearsal.is_solid(x, 0), "gate at ({x},0) walkable");
        assert!(rehearsal.trigger_at(x, 0).is_some(), "warp at ({x},0)");
    }
    // The greeter stands on open floor next to the solid sign.
    assert!(!rehearsal.is_solid(7, 5));
    assert!(rehearsal.is_solid(7, 6), "sign tile is solid");
    assert!(rehearsal.npc_at(7, 5).is_some());
    // Patches exist and sit on walkable grass.
    let mut patch_count = 0;
    for y in 0..rehearsal.height {
        for x in 0..rehearsal.width {
            if rehearsal.is_patch(x, y) {
                patch_count += 1;
                assert!(!rehearsal.is_solid(x, y), "patch ({x},{y}) walkable");
            }
        }
    }
    assert!(patch_count >= 8, "enough patch tiles to test encounters");

    // Round trip: rehearsal gate → annex landing → annex door → rehearsal.
    let annex = &maps[&"debug_annex".into()];
    assert!(
        !annex.is_solid(3, 4) && !annex.is_solid(4, 4),
        "annex landing clear"
    );
    assert!(annex.trigger_at(3, 5).is_some(), "annex exit door");
    assert!(
        !rehearsal.is_solid(5, 1) && !rehearsal.is_solid(6, 1),
        "return landing clear"
    );
}

#[test]
fn broken_warp_targets_are_flagged() {
    let mut maps = load_maps(&content_root().join("dev/maps")).expect("maps load");
    // Point a warp at a solid tile.
    let rehearsal = maps.get_mut(&"debug_rehearsal".into()).expect("map");
    if let data::TriggerKind::Warp { to, .. } = &mut rehearsal.triggers[0].kind {
        *to = (0, 0); // annex (0,0) is solid wall
    }
    let pool = load_species_pool(&content_root().join("dev/testbed.ron")).expect("pool");
    let findings = validate_maps(&maps, &pool, &script_exists);
    assert!(
        findings
            .iter()
            .any(|f| f.rule == "map.warp_target" && f.message.contains("solid")),
        "{findings:?}"
    );
}
