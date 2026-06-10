//! The pure overworld core: grid logic, triggers, NPCs, dialogue,
//! encounters, save snapshots. No Bevy, no I/O, no clock — the Bevy layer
//! renders this state and feeds it inputs, exactly as the battle crate's
//! event stream is rendered (doc 03 §2 discipline applied to the
//! overworld). Headless replays drive this directly.

use std::collections::BTreeMap;

use data::map::{MapDef, NpcBehavior, TriggerKind};
use script::{Cmd, ScriptRunner, ScriptVars, SideEffectReq, StepResult};
use undersong_core::ids::{MapId, SpeciesId};
use undersong_core::rng::BattleRng;
use undersong_core::world::Facing;

/// Replay-file input vocabulary (tests/replays/*.ron).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Input {
    /// Face the direction; step onto the next tile if walkable.
    Step(Facing),
    /// Talk to the faced NPC / advance dialogue / confirm choice.
    Interact,
    /// NPC wander tick (the windowed game fires this on a timer; replays
    /// fire it explicitly so movement stays deterministic).
    Tick,
    /// Save to the autosave slot (the replay harness uses a MemBackend).
    Save,
}

/// What happened during one input application; the Bevy layer turns
/// these into animation/scene work.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorldEvent {
    Stepped { to: (u32, u32) },
    Bumped { at: (u32, u32) },
    Faced(Facing),
    Warped { map: MapId, to: (u32, u32) },
    DialogueLine { who: String, key: String },
    DialogueEnded,
    EncounterStarted { species: SpeciesId, level: u8 },
    NpcMoved { id: String, to: (u32, u32) },
    Saved,
}

/// A live NPC (positions can drift from the map definition via wander).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct NpcState {
    pub id: String,
    pub at: (u32, u32),
    pub spawn: (u32, u32),
    pub facing: Facing,
    pub sprite: String,
    pub script: Option<String>,
    pub behavior: NpcBehavior,
}

/// Dialogue presentation state, pure: the renderer shows `current`,
/// `Interact` advances.
#[derive(Debug, Clone, PartialEq)]
pub struct DialogueState {
    pub runner: ScriptRunner,
    pub current: Option<(String, String)>,
}

pub struct WorldState {
    pub maps: BTreeMap<MapId, MapDef>,
    /// Scripts per (map, path), pre-loaded so the core stays I/O-free.
    pub scripts: BTreeMap<(MapId, String), Vec<Cmd>>,
    pub current_map: MapId,
    pub player: (u32, u32),
    pub facing: Facing,
    pub vars: ScriptVars,
    pub dialogue: Option<DialogueState>,
    /// NPC states per map (only mutated for the current map).
    pub npcs: BTreeMap<MapId, Vec<NpcState>>,
    pub rng: BattleRng,
    pub world_seed: u64,
    pub steps: u64,
    /// Set when an encounter triggers; the scene layer consumes it.
    pub pending_encounter: Option<(SpeciesId, u8)>,
}

impl WorldState {
    pub fn new(
        maps: BTreeMap<MapId, MapDef>,
        scripts: BTreeMap<(MapId, String), Vec<Cmd>>,
        start_map: MapId,
        start: (u32, u32),
        world_seed: u64,
    ) -> Self {
        let npcs = maps
            .iter()
            .map(|(id, map)| {
                let states = map
                    .npcs
                    .iter()
                    .map(|n| NpcState {
                        id: n.id.clone(),
                        at: n.at,
                        spawn: n.at,
                        facing: n.facing,
                        sprite: n.sprite.clone(),
                        script: n.script.clone(),
                        behavior: n.behavior,
                    })
                    .collect();
                (id.clone(), states)
            })
            .collect();
        Self {
            maps,
            scripts,
            current_map: start_map,
            player: start,
            facing: Facing::Down,
            vars: ScriptVars::default(),
            dialogue: None,
            npcs,
            rng: BattleRng::from_seed(world_seed),
            world_seed,
            steps: 0,
            pending_encounter: None,
        }
    }

    pub fn map(&self) -> &MapDef {
        &self.maps[&self.current_map]
    }

    fn npc_blocking(&self, x: u32, y: u32) -> bool {
        self.npcs
            .get(&self.current_map)
            .is_some_and(|npcs| npcs.iter().any(|n| n.at == (x, y)))
    }

    /// Applies one input. Dialogue captures Interact; movement is blocked
    /// while dialogue or an encounter is pending.
    pub fn apply(&mut self, input: Input) -> Vec<WorldEvent> {
        let mut events = Vec::new();
        match input {
            Input::Interact => self.interact(&mut events),
            Input::Step(dir) if self.dialogue.is_none() && self.pending_encounter.is_none() => {
                self.step(dir, &mut events);
            }
            Input::Step(_) => {}
            Input::Tick => self.tick_wander(&mut events),
            Input::Save => events.push(WorldEvent::Saved),
        }
        events
    }

    fn step(&mut self, dir: Facing, events: &mut Vec<WorldEvent>) {
        if self.facing != dir {
            self.facing = dir;
            events.push(WorldEvent::Faced(dir));
            // Era feel: turning and stepping are one input; the step
            // still proceeds this tick.
        }
        let (dx, dy) = dir.delta();
        let nx = self.player.0.checked_add_signed(dx);
        let ny = self.player.1.checked_add_signed(dy);
        let (Some(nx), Some(ny)) = (nx, ny) else {
            events.push(WorldEvent::Bumped { at: self.player });
            return;
        };
        if self.map().is_solid(nx, ny) || self.npc_blocking(nx, ny) {
            events.push(WorldEvent::Bumped { at: (nx, ny) });
            return;
        }
        self.player = (nx, ny);
        self.steps += 1;
        events.push(WorldEvent::Stepped { to: (nx, ny) });

        // Triggers fire on entering the tile (doc 04 §2).
        if let Some(trigger) = self.map().trigger_at(nx, ny).cloned() {
            if let Some(flag) = &trigger.once_flag
                && self.vars.flags.contains(flag)
            {
                // consumed once-trigger
            } else {
                match trigger.kind {
                    TriggerKind::Warp { map, to, facing } => {
                        if let Some(flag) = trigger.once_flag {
                            self.vars.flags.insert(flag);
                        }
                        self.current_map = map.clone();
                        self.player = to;
                        self.facing = facing;
                        events.push(WorldEvent::Warped { map, to });
                        return; // no encounter roll on the warp step
                    }
                    TriggerKind::Script { path } => {
                        if let Some(flag) = trigger.once_flag {
                            self.vars.flags.insert(flag);
                        }
                        self.start_script(&path, events);
                        return;
                    }
                }
            }
        }

        // Resonance patch roll (doc 02 §12): P per step, then a 12-slot
        // weighted species pick and a uniform level in the slot's range.
        if self.map().is_patch(nx, ny)
            && let Some(encounters) = self.map().encounters.clone()
            && self.rng.chance(u32::from(encounters.patch_rate_pct), 100)
        {
            let roll = self.rng.below(100);
            let mut cumulative = 0u32;
            for (species, lo, hi, weight) in &encounters.slots {
                cumulative += u32::from(*weight);
                if roll < cumulative {
                    let level = u8::try_from(
                        self.rng
                            .range_inclusive(u32::from(*lo), u32::from(*hi.max(lo))),
                    )
                    .expect("level fits u8");
                    self.pending_encounter = Some((species.clone(), level));
                    events.push(WorldEvent::EncounterStarted {
                        species: species.clone(),
                        level,
                    });
                    break;
                }
            }
        }
    }

    fn interact(&mut self, events: &mut Vec<WorldEvent>) {
        // Advancing dialogue?
        if self.dialogue.is_some() {
            self.advance_dialogue(events);
            return;
        }
        // Facing an NPC?
        let (dx, dy) = self.facing.delta();
        let (Some(tx), Some(ty)) = (
            self.player.0.checked_add_signed(dx),
            self.player.1.checked_add_signed(dy),
        ) else {
            return;
        };
        let npc = self
            .npcs
            .get(&self.current_map)
            .and_then(|npcs| npcs.iter().position(|n| n.at == (tx, ty)));
        let Some(index) = npc else { return };

        // NPC turns to face the player.
        let player_facing = self.facing;
        let map_id = self.current_map.clone();
        if let Some(npcs) = self.npcs.get_mut(&map_id) {
            npcs[index].facing = player_facing.opposite();
        }
        let script_path = self.npcs[&map_id][index].script.clone();
        if let Some(path) = script_path {
            self.start_script(&path, events);
        }
    }

    fn start_script(&mut self, path: &str, events: &mut Vec<WorldEvent>) {
        let key = (self.current_map.clone(), path.to_string());
        let Some(cmds) = self.scripts.get(&key).cloned() else {
            // Validator guarantees this for shipped content; tolerate in
            // case of dev hot-edits.
            return;
        };
        self.dialogue = Some(DialogueState {
            runner: ScriptRunner::new(cmds),
            current: None,
        });
        self.advance_dialogue(events);
    }

    /// Pumps the script until the next ShowText (rendered, waits for
    /// Interact) or completion. Non-dialogue effects apply immediately.
    fn advance_dialogue(&mut self, events: &mut Vec<WorldEvent>) {
        let Some(mut dialogue) = self.dialogue.take() else {
            return;
        };
        loop {
            match dialogue.runner.step(&mut self.vars) {
                StepResult::Effect(SideEffectReq::ShowText { who, key }) => {
                    events.push(WorldEvent::DialogueLine {
                        who: who.clone(),
                        key: key.clone(),
                    });
                    dialogue.current = Some((who, key));
                    self.dialogue = Some(dialogue);
                    return;
                }
                StepResult::Effect(SideEffectReq::Warp { map, x, y }) => {
                    self.current_map = map.clone();
                    self.player = (x, y);
                    events.push(WorldEvent::Warped { map, to: (x, y) });
                }
                StepResult::Effect(SideEffectReq::HealParty) => {
                    // Party heal lands with the party system in P3.
                }
                StepResult::Effect(_other) => {
                    // Remaining effects (items, battles, music…) arrive
                    // with their systems in P3+; the interpreter contract
                    // is already exercised by the script crate tests.
                }
                StepResult::Done => {
                    events.push(WorldEvent::DialogueEnded);
                    self.dialogue = None;
                    return;
                }
            }
        }
    }

    /// One wander tick: each Wander NPC on the current map has a 1-in-3
    /// chance to stroll one tile, staying within its radius, off solids,
    /// off the player, off other NPCs.
    fn tick_wander(&mut self, events: &mut Vec<WorldEvent>) {
        let map_id = self.current_map.clone();
        let Some(count) = self.npcs.get(&map_id).map(Vec::len) else {
            return;
        };
        for index in 0..count {
            let NpcBehavior::Wander { radius } = self.npcs[&map_id][index].behavior else {
                continue;
            };
            if !self.rng.chance(1, 3) {
                continue;
            }
            let dir = match self.rng.below(4) {
                0 => Facing::Up,
                1 => Facing::Down,
                2 => Facing::Left,
                _ => Facing::Right,
            };
            let npc = &self.npcs[&map_id][index];
            let (dx, dy) = dir.delta();
            let (Some(nx), Some(ny)) = (
                npc.at.0.checked_add_signed(dx),
                npc.at.1.checked_add_signed(dy),
            ) else {
                continue;
            };
            let within = nx.abs_diff(npc.spawn.0) <= u32::from(radius)
                && ny.abs_diff(npc.spawn.1) <= u32::from(radius);
            let blocked = self.maps[&map_id].is_solid(nx, ny)
                || (nx, ny) == self.player
                || self.npcs[&map_id]
                    .iter()
                    .enumerate()
                    .any(|(i, other)| i != index && other.at == (nx, ny));
            if within && !blocked {
                let npcs = self.npcs.get_mut(&map_id).expect("map npcs");
                npcs[index].at = (nx, ny);
                npcs[index].facing = dir;
                events.push(WorldEvent::NpcMoved {
                    id: npcs[index].id.clone(),
                    to: (nx, ny),
                });
            }
        }
    }

    /// Snapshot into a SaveFile (the game layer adds timestamps).
    pub fn to_save(&self, name: &str, playtime_s: u64, created_epoch_s: u64) -> save::SaveFile {
        save::SaveFile {
            header: save::SaveHeader {
                version: save::SAVE_VERSION,
                created_epoch_s,
                playtime_s,
                region: "dev".into(),
                badge_bits: 0,
                score_pct: 0,
            },
            player: save::model::Player {
                name: name.into(),
                money: 0,
                position: save::Position {
                    map: self.current_map.clone(),
                    x: self.player.0,
                    y: self.player.1,
                    facing: self.facing,
                },
                settings: save::Settings::default(),
            },
            party: vec![],
            boxes: vec![],
            bag: BTreeMap::new(),
            flags: self.vars.flags.clone(),
            vars: self.vars.vars.clone(),
            counters: BTreeMap::from([("steps".to_string(), self.steps)]),
            world_seed: self.world_seed,
        }
    }

    /// Restores position/flags/counters from a save into a fresh world
    /// built over the same content.
    pub fn restore(&mut self, file: &save::SaveFile) {
        self.current_map = file.player.position.map.clone();
        self.player = (file.player.position.x, file.player.position.y);
        self.facing = file.player.position.facing;
        self.vars.flags = file.flags.clone();
        self.vars.vars = file.vars.clone();
        self.steps = file.counters.get("steps").copied().unwrap_or(0);
        self.world_seed = file.world_seed;
        self.rng = BattleRng::from_seed(file.world_seed);
        self.dialogue = None;
        self.pending_encounter = None;
    }
}

/// Loads the dev world (maps + scripts) from a content root. The only
/// I/O in this module, used by both the windowed game and the replay
/// driver before the pure loop starts.
pub fn load_dev_world(content_root: &std::path::Path, seed: u64) -> Result<WorldState, String> {
    let maps_root = content_root.join("dev/maps");
    let maps = data::load_maps(&maps_root).map_err(|e| e.to_string())?;
    let mut scripts = BTreeMap::new();
    for (id, map) in &maps {
        let mut paths: Vec<String> = map.npcs.iter().filter_map(|n| n.script.clone()).collect();
        for trigger in &map.triggers {
            if let TriggerKind::Script { path } = &trigger.kind {
                paths.push(path.clone());
            }
        }
        for path in paths {
            let file = maps_root.join(id.as_str()).join("scripts").join(&path);
            let text =
                std::fs::read_to_string(&file).map_err(|e| format!("{}: {e}", file.display()))?;
            let cmds: Vec<Cmd> =
                ron::from_str(&text).map_err(|e| format!("{}: {e}", file.display()))?;
            scripts.insert((id.clone(), path), cmds);
        }
    }
    Ok(WorldState::new(
        maps,
        scripts,
        "debug_rehearsal".into(),
        (5, 2),
        seed,
    ))
}
