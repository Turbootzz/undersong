//! Headless replay driver (doc 03 §6 integration lane).
//!
//! A replay file is a RON list of [`crate::world::Input`] plus expected
//! end-state assertions. The driver runs the pure world, performs `Save`
//! inputs against a MemBackend, then reloads from that save into a fresh
//! world and re-asserts — so the save/reload half of the P2 gate is part
//! of the same artifact.

use std::collections::BTreeSet;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::world::{Input, WorldState, load_dev_world, load_game_world};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum WorldKind {
    /// The P2 dev testbed (content/dev/maps).
    #[default]
    Dev,
    /// The real game world: Cantorel pack + battle registry.
    Game,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayFile {
    pub seed: u64,
    #[serde(default)]
    pub world: WorldKind,
    pub inputs: Vec<Input>,
    pub expect: Expectations,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Expectations {
    pub map: String,
    pub position: (u32, u32),
    /// Flags that must be set at the end.
    pub flags: BTreeSet<String>,
    /// Minimum dialogue lines seen across the run.
    pub min_dialogue_lines: u32,
    /// Whether at least one warp must have happened.
    pub warped: bool,
    /// Party levels in order (doc 03 §6: replays assert party too).
    #[serde(default)]
    pub party_levels: Option<Vec<u8>>,
    /// Final money, asserted end-of-inputs and after reload.
    #[serde(default)]
    pub money: Option<u32>,
}

pub struct ReplayOutcome {
    pub dialogue_lines: u32,
    pub warps: u32,
    pub saves: u32,
}

/// Runs a replay against the dev world. Returns Err with a readable
/// message on any assertion failure (the CLI prints it; tests unwrap).
pub fn run_replay(content_root: &Path, file: &ReplayFile) -> Result<ReplayOutcome, String> {
    let mut world = match file.world {
        WorldKind::Dev => load_dev_world(content_root, file.seed)?,
        WorldKind::Game => load_game_world(content_root, file.seed)?,
    };
    let mut backend = save::MemBackend::default();
    let mut outcome = ReplayOutcome {
        dialogue_lines: 0,
        warps: 0,
        saves: 0,
    };

    for (index, input) in file.inputs.iter().enumerate() {
        let events = world.apply(*input);
        for event in &events {
            match event {
                crate::world::WorldEvent::DialogueLine { .. } => outcome.dialogue_lines += 1,
                crate::world::WorldEvent::Warped { .. } => outcome.warps += 1,
                crate::world::WorldEvent::Saved => {
                    let snapshot = world.to_save("replay", 0, 0);
                    save::save(&mut backend, save::SlotId::Auto, &snapshot)
                        .map_err(|e| format!("input {index}: save failed: {e}"))?;
                    outcome.saves += 1;
                }
                _ => {}
            }
        }
    }

    let assert_world = |world: &WorldState, stage: &str| -> Result<(), String> {
        if world.current_map.as_str() != file.expect.map {
            return Err(format!(
                "{stage}: expected map `{}`, got `{}`",
                file.expect.map, world.current_map
            ));
        }
        if world.player != file.expect.position {
            return Err(format!(
                "{stage}: expected position {:?}, got {:?}",
                file.expect.position, world.player
            ));
        }
        for flag in &file.expect.flags {
            if !world.vars.flags.contains(flag) {
                return Err(format!("{stage}: flag `{flag}` not set"));
            }
        }
        if let Some(expected) = &file.expect.party_levels {
            let actual: Vec<u8> = world.party.iter().map(|p| p.level).collect();
            if &actual != expected {
                return Err(format!(
                    "{stage}: party levels {actual:?}, expected {expected:?}"
                ));
            }
        }
        if let Some(expected) = file.expect.money
            && world.money != expected
        {
            return Err(format!(
                "{stage}: money {}, expected {expected}",
                world.money
            ));
        }
        Ok(())
    };

    assert_world(&world, "end of inputs")?;
    if outcome.dialogue_lines < file.expect.min_dialogue_lines {
        return Err(format!(
            "expected ≥ {} dialogue lines, saw {}",
            file.expect.min_dialogue_lines, outcome.dialogue_lines
        ));
    }
    if file.expect.warped && outcome.warps == 0 {
        return Err("expected at least one warp".into());
    }

    // Save/reload leg: restore the last autosave into a fresh world and
    // re-assert position + flags (the P2 gate's reload half).
    if outcome.saves > 0 {
        let restored = save::load(&backend, save::SlotId::Auto)
            .map_err(|e| format!("reload failed: {e}"))?
            .ok_or("autosave missing after save")?;
        let mut fresh = match file.world {
            WorldKind::Dev => load_dev_world(content_root, file.seed)?,
            WorldKind::Game => load_game_world(content_root, file.seed)?,
        };
        fresh.restore(&restored);
        assert_world(&fresh, "after reload")?;
    }

    Ok(outcome)
}

/// Loads and runs a replay file from disk.
pub fn run_replay_file(content_root: &Path, replay_path: &Path) -> Result<ReplayOutcome, String> {
    let text = std::fs::read_to_string(replay_path)
        .map_err(|e| format!("{}: {e}", replay_path.display()))?;
    let file: ReplayFile =
        ron::from_str(&text).map_err(|e| format!("{}: {e}", replay_path.display()))?;
    run_replay(content_root, &file)
}
