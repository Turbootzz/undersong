//! TEMPORARY review probe — deleted after the review run.

use std::path::{Path, PathBuf};

use game::world::{Input, load_game_world};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn p4_review_probe() {
    // 1. Run the badge replay manually, measure clock_ticks vs steps.
    let text =
        std::fs::read_to_string(root().join("tests/replays/new_game_to_first_badge.ron")).unwrap();
    let file: game::replay::ReplayFile = ron::from_str(&text).unwrap();
    let mut world = load_game_world(&root().join("content"), file.seed).unwrap();
    for input in &file.inputs {
        let _ = world.apply(input.clone());
    }
    eprintln!(
        "PROBE clock_ticks={} steps={} is_night={} badge1={}",
        world.clock_ticks,
        world.steps,
        world.is_night(),
        world.vars.flags.contains("badge.1"),
    );

    // 2. badge_count restore bug: save → restore into fresh world → one
    //    harmless input → party friendship should NOT change.
    let friendship_before: Vec<u8> = world.party.iter().map(|p| p.friendship).collect();
    let snapshot = world.to_save("probe", 0, 0);
    let mut fresh = load_game_world(&root().join("content"), file.seed).unwrap();
    fresh.restore(&snapshot);
    let restored: Vec<u8> = fresh.party.iter().map(|p| p.friendship).collect();
    let _ = fresh.apply(Input::Tick);
    let after_one_input: Vec<u8> = fresh.party.iter().map(|p| p.friendship).collect();
    eprintln!(
        "PROBE friendship live={friendship_before:?} restored={restored:?} after_one_input={after_one_input:?}"
    );

    // 3. Repeat load cycles compound it.
    let snapshot2 = fresh.to_save("probe", 0, 0);
    let mut fresh2 = load_game_world(&root().join("content"), file.seed).unwrap();
    fresh2.restore(&snapshot2);
    let _ = fresh2.apply(Input::Tick);
    let after_second_cycle: Vec<u8> = fresh2.party.iter().map(|p| p.friendship).collect();
    eprintln!("PROBE after_second_cycle={after_second_cycle:?}");
}
