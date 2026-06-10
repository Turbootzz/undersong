//! The P2 acceptance gate and pure-core behavior tests
//! (docs/06-ROADMAP.md Gate P2; doc 02 v1.3 rulings).

use std::path::{Path, PathBuf};

use game::world::{Input, WorldEvent, load_dev_world};
use undersong_core::world::Facing::{Down, Up};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn walk_talk_warp_save_replay_passes() {
    let outcome = game::replay::run_replay_file(
        &root().join("content"),
        &root().join("tests/replays/walk_talk_warp_save.ron"),
    )
    .expect("gate replay must pass");
    assert!(outcome.dialogue_lines >= 2);
    assert!(outcome.warps >= 1);
    assert_eq!(outcome.saves, 1);
}

#[test]
fn replay_is_deterministic_across_runs() {
    let run = || {
        let outcome = game::replay::run_replay_file(
            &root().join("content"),
            &root().join("tests/replays/walk_talk_warp_save.ron"),
        )
        .expect("replay passes");
        (outcome.dialogue_lines, outcome.warps, outcome.saves)
    };
    assert_eq!(run(), run());
}

#[test]
fn encounters_roll_deterministically_on_patches() {
    // Bounce inside the west patch column (x4, y6..7): every move lands
    // on a patch tile, so the roll fires quickly at 12%/step.
    let run = |seed: u64| -> Vec<(String, u8, u32)> {
        let mut world = load_dev_world(&root().join("content"), seed).expect("world");
        world.player = (4, 7); // walkable patch-adjacent start
        let mut hits = Vec::new();
        let mut steps = 0u32;
        let cycle = [
            Input::Step(Down), // move (4,6)
            Input::Step(Up),   // turn
            Input::Step(Up),   // move (4,7)
            Input::Step(Down), // turn
        ];
        'outer: for _ in 0..60 {
            for input in cycle.clone() {
                for event in world.apply(input) {
                    if let WorldEvent::EncounterStarted { species, level } = event {
                        hits.push((species.to_string(), level, steps));
                        break 'outer;
                    }
                }
                steps += 1;
            }
        }
        hits
    };
    let first = run(99);
    let second = run(99);
    assert!(
        !first.is_empty(),
        "walker bounces on patch tiles — an encounter must fire"
    );
    assert_eq!(first, second, "same seed ⇒ same encounter at the same step");
    let different = run(100);
    // Different seeds may coincide, but the test documents the intent.
    let _ = different;
}

#[test]
fn sighted_sentry_engages_once_and_choice_flow_works() {
    // Doc 02 v1.3 #3 + Choice handling: walk into the sentry's sight
    // line in debug_annex, get engaged, answer the challenge.
    let mut world = load_dev_world(&root().join("content"), 7).expect("world");
    world.current_map = "debug_annex".into();
    world.player = (5, 3);
    world.facing = Down;

    let events = world.apply(Input::Step(Down)); // move to (5,2): sighted
    assert!(
        events
            .iter()
            .any(|e| matches!(e, WorldEvent::Engaged { npc } if npc == "sentry")),
        "{events:?}"
    );
    assert!(events.iter().any(
        |e| matches!(e, WorldEvent::DialogueLine { key, .. } if key == "debug.sentry.spotted")
    ));
    assert!(world.vars.flags.contains("engaged.debug_annex.sentry"));

    // Advance into the choice, move the cursor down, answer.
    let events = world.apply(Input::Interact);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, WorldEvent::DialogueChoice { options, .. } if options.len() == 2))
    );
    let _ = world.apply(Input::Step(Down)); // cursor → decline
    let events = world.apply(Input::Interact);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, WorldEvent::DialogueEnded)),
        "{events:?}"
    );
    assert!(world.vars.flags.contains("sentry.declined"));
    assert!(!world.vars.flags.contains("sentry.accepted"));

    // Walking the line again must not re-engage (flag-gated).
    world.player = (5, 3);
    world.facing = Down;
    let events = world.apply(Input::Step(Down));
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, WorldEvent::Engaged { .. })),
        "engagement is once per flag"
    );
}

#[test]
fn tap_to_turn_first_step_only_faces() {
    let mut world = load_dev_world(&root().join("content"), 1).expect("world");
    assert_eq!(world.facing, Down);
    let start = world.player;
    let events = world.apply(Input::Step(Up));
    assert_eq!(world.player, start, "direction change only turns (v1.3 #1)");
    assert!(events.iter().any(|e| matches!(e, WorldEvent::Faced(Up))));
    let _ = world.apply(Input::Step(Up));
    assert_ne!(
        world.player, start,
        "second step in the faced direction moves"
    );
}

#[test]
fn wander_pauses_during_dialogue_in_the_core() {
    // Doc 02 v1.3 #4: ticks while talking consume no rng and move no one.
    let mut world = load_dev_world(&root().join("content"), 5).expect("world");
    // Stand before the greeter and start dialogue.
    world.player = (7, 4);
    world.facing = Up;
    let _ = world.apply(Input::Interact);
    assert!(world.dialogue.is_some());
    let rng_before = format!("{:?}", world.rng);
    let npc_before = world.npcs[&world.current_map].clone();
    for _ in 0..5 {
        let events = world.apply(Input::Tick);
        assert!(events.is_empty());
    }
    assert_eq!(format!("{:?}", world.rng), rng_before, "no rng consumed");
    assert_eq!(world.npcs[&world.current_map], npc_before, "no one moved");
}
