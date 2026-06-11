//! P5 acceptance gate: new game → Badge 3 → the Lull scene, driven
//! through the pure core, recorded to tests/replays/act1_complete.ron
//! (UPDATE_REPLAYS=1) and played back by the second test.

mod common;

use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

use common::ACT_SEED as SEED;

#[test]
fn act1_reaches_badge_three_and_lull() {
    let driver = common::run_act1();
    if std::env::var_os("UPDATE_REPLAYS").is_some() {
        let file = game::replay::ReplayFile {
            seed: SEED,
            world: game::replay::WorldKind::Game,
            inputs: driver.log.clone(),
            expect: game::replay::Expectations {
                map: driver.world.current_map.to_string(),
                position: driver.world.player,
                flags: [
                    "badge.1",
                    "badge.2",
                    "badge.3",
                    "story.theft.seen",
                    "story.tacet.shipment",
                    "story.tacet.yard",
                    "story.rival2.defeated",
                    "story.keyshift.seen",
                    "story.lull.done",
                    "performance.clearing_chord",
                    "performance.tunneling_bass",
                ]
                .into_iter()
                .map(String::from)
                .collect(),
                min_dialogue_lines: driver.dialogue_lines,
                warped: true,
                party_levels: Some(driver.world.party.iter().map(|p| p.level).collect()),
                money: Some(driver.world.money),
            },
        };
        let text = ron::ser::to_string_pretty(&file, ron::ser::PrettyConfig::default())
            .expect("serialize");
        std::fs::write(
            root().join("tests/replays/act1_complete.ron"),
            format!(
                "// P5 gate replay — recorded by act1_run.rs (UPDATE_REPLAYS=1).\n// {} inputs; do not hand-edit.\n{text}\n",
                file.inputs.len()
            ),
        )
        .expect("write replay");
    }
}

#[test]
fn recorded_act1_replay_plays_back() {
    if std::env::var_os("UPDATE_REPLAYS").is_some() {
        return;
    }
    let path = root().join("tests/replays/act1_complete.ron");
    if !path.exists() {
        panic!("missing act1 replay — run UPDATE_REPLAYS=1 cargo test -p game --test act1_run");
    }
    let outcome = game::replay::run_replay_file(&root().join("content"), &path)
        .expect("the recorded act-1 run must replay cleanly");
    assert!(outcome.warps >= 12, "the run crosses many gates");
}
