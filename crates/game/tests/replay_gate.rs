//! The P2 acceptance gate, as a plain test: the walk/talk/warp/save
//! replay must pass headlessly (docs/06-ROADMAP.md Gate P2).

use std::path::Path;

#[test]
fn walk_talk_warp_save_replay_passes() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let outcome = game::replay::run_replay_file(
        &root.join("content"),
        &root.join("tests/replays/walk_talk_warp_save.ron"),
    )
    .expect("gate replay must pass");
    assert!(outcome.dialogue_lines >= 2);
    assert!(outcome.warps >= 1);
    assert_eq!(outcome.saves, 1);
}

#[test]
fn replay_is_deterministic_across_runs() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let run = || {
        let outcome = game::replay::run_replay_file(
            &root.join("content"),
            &root.join("tests/replays/walk_talk_warp_save.ron"),
        )
        .expect("replay passes");
        (outcome.dialogue_lines, outcome.warps, outcome.saves)
    };
    assert_eq!(run(), run());
}

#[test]
fn encounters_roll_deterministically_on_patches() {
    // March through the patch field until an encounter fires; same seed
    // ⇒ same species/level/step.
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let run = |seed: u64| {
        let mut world = game::world::load_dev_world(&root.join("content"), seed).expect("world");
        let mut walk = Vec::new();
        // Zig-zag over the west patch block (x3..4, y6..8).
        use undersong_core::world::Facing::{Down, Left, Right, Up};
        let mut steps = 0u32;
        for _ in 0..40 {
            for dir in [Up, Left, Right, Down] {
                for event in world.apply(game::world::Input::Step(dir)) {
                    if let game::world::WorldEvent::EncounterStarted { species, level } = event {
                        walk.push((species.to_string(), level, steps));
                    }
                }
                steps += 1;
                if !walk.is_empty() {
                    return walk;
                }
            }
        }
        walk
    };
    // Position the walker inside the patches first: directly construct.
    let first = run(99);
    let second = run(99);
    assert_eq!(first, second, "same seed ⇒ same encounter");
}
