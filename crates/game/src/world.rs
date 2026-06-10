//! The pure overworld core: grid logic, triggers, NPCs, dialogue,
//! encounters, save snapshots. No Bevy, no I/O, no clock — the Bevy layer
//! renders this state and feeds it inputs, exactly as the battle crate's
//! event stream is rendered (doc 03 §2 discipline applied to the
//! overworld). Headless replays drive this directly.

use std::collections::BTreeMap;

use data::map::{MapDef, NpcBehavior, TriggerKind};
use script::{Cmd, ScriptRunner, ScriptVars, SideEffectReq, StepResult};
use undersong_core::ids::{ItemId, MapId, SpeciesId};
use undersong_core::individual::Individual;
use undersong_core::rng::BattleRng;
use undersong_core::world::Facing;

use crate::session::{BattleCmd, BattleContext, BattleSession, Registry};

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
    /// A battle command while a battle session is active.
    Battle(BattleCmd),
    /// Answer a pending learn prompt: replace this move slot (None skips).
    Learn {
        replace: Option<u8>,
    },
    /// Accept or refuse a pending evolution.
    Evolve {
        accept: bool,
    },
    /// Move the shop cursor / buy / leave.
    ShopCursor(i8),
    ShopBuy,
    ShopClose,
}

/// What happened during one input application; the Bevy layer turns
/// these into animation/scene work.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorldEvent {
    Stepped {
        to: (u32, u32),
    },
    Bumped {
        at: (u32, u32),
    },
    Faced(Facing),
    Warped {
        map: MapId,
        to: (u32, u32),
    },
    DialogueLine {
        who: String,
        key: String,
    },
    /// A choice list opened; the renderer shows options + cursor.
    DialogueChoice {
        key: String,
        options: Vec<String>,
    },
    DialogueEnded,
    /// A sighted trainer NPC spotted the player (doc 02 v1.3 #3).
    Engaged {
        npc: String,
    },
    EncounterStarted {
        species: SpeciesId,
        level: u8,
    },
    NpcMoved {
        id: String,
        to: (u32, u32),
    },
    Saved,
    /// A battle turn resolved; the presenter renders this stream.
    Battle(Vec<battle::BattleEvent>),
    BattleFinished {
        outcome: battle::Outcome,
    },
    MoteCaught {
        species: SpeciesId,
    },
    MoteJoined {
        species: SpeciesId,
    },
    LearnPrompt {
        species: SpeciesId,
        move_id: undersong_core::ids::MoveId,
    },
    MoveLearned {
        species: SpeciesId,
        move_id: undersong_core::ids::MoveId,
    },
    EvolutionPrompt {
        from: SpeciesId,
        into: SpeciesId,
    },
    Evolved {
        from: SpeciesId,
        into: SpeciesId,
    },
    MoneyChanged {
        money: u32,
    },
    ShopOpened,
    ItemBought {
        item: ItemId,
    },
    /// Party wiped: half money, heal, return to the rest point
    /// (doc 02 §15).
    Whiteout,
    /// An illegal battle selection was refused without spending the
    /// turn or the item (doc 02 v1.4 #3).
    ActionRejected {
        reason_key: String,
    },
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
    /// Trainer sight range in tiles; 0 = never engages (doc 02 §12).
    pub sight_range: u8,
}

/// Dialogue presentation state, pure: the renderer shows `current`,
/// `Interact` advances.
#[derive(Debug, Clone, PartialEq)]
pub struct DialogueState {
    pub runner: ScriptRunner,
    pub current: Option<(String, String)>,
    /// Open choice list: (prompt key, options, cursor).
    pub choice: Option<(String, Vec<String>, usize)>,
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
    /// Set when an encounter triggers; consumed by the battle bridge
    /// when a registry is present, by the placeholder scene otherwise.
    pub pending_encounter: Option<(SpeciesId, u8)>,
    /// Content registry; None in the registry-less dev world (P2 tests).
    pub registry: Option<Registry>,
    pub party: Vec<Individual>,
    pub boxes: Vec<Individual>,
    pub bag: std::collections::BTreeMap<ItemId, u32>,
    pub money: u32,
    pub battle: Option<BattleSession>,
    /// Open mart: (item ids, cursor).
    pub shop: Option<(Vec<ItemId>, usize)>,
    /// Queued evolution prompts: (party index, target species).
    pub pending_evolutions: Vec<(usize, SpeciesId)>,
    /// Learn prompts awaiting an answer: (party index, move).
    pub pending_learn_queue: Vec<(usize, undersong_core::ids::MoveId)>,
    /// Respawn point after a whiteout (map, position).
    pub heal_point: (MapId, (u32, u32)),
    /// Merged string table (core + region), golden rule 6's flavored face.
    pub strings: undersong_core::collections::UniqueMap<String, String>,
    /// Region id for saves and asset paths ("dev" in the testbed).
    pub region_id: String,
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
                        sight_range: n.sight_range,
                    })
                    .collect();
                (id.clone(), states)
            })
            .collect();
        Self {
            maps,
            scripts,
            current_map: start_map.clone(),
            player: start,
            facing: Facing::Down,
            vars: ScriptVars::default(),
            dialogue: None,
            npcs,
            rng: BattleRng::from_seed(world_seed),
            world_seed,
            steps: 0,
            pending_encounter: None,
            registry: None,
            party: Vec::new(),
            boxes: Vec::new(),
            bag: std::collections::BTreeMap::new(),
            money: 3000,
            battle: None,
            shop: None,
            pending_evolutions: Vec::new(),
            pending_learn_queue: Vec::new(),
            heal_point: (start_map, start),
            strings: std::collections::BTreeMap::new().into(),
            region_id: "dev".into(),
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
        // Battle mode captures its own vocabulary first.
        if self.battle.is_some() {
            if let Input::Battle(command) = input {
                self.battle_turn(command, &mut events);
            }
            return events;
        }
        if !self.pending_evolutions.is_empty() || !self.pending_learn_queue.is_empty() {
            match input {
                Input::Evolve { accept } if !self.pending_evolutions.is_empty() => {
                    self.resolve_evolution(accept, &mut events);
                }
                Input::Learn { replace } if !self.pending_learn_queue.is_empty() => {
                    self.answer_learn(replace, &mut events);
                }
                _ => {}
            }
            return events;
        }
        if self.shop.is_some() {
            match input {
                Input::ShopCursor(delta) => {
                    if let Some((items, cursor)) = &mut self.shop {
                        let len = items.len() as i32;
                        let next = (*cursor as i32 + i32::from(delta)).clamp(0, len - 1);
                        *cursor = usize::try_from(next).unwrap_or(0);
                    }
                }
                Input::ShopBuy => self.shop_buy(&mut events),
                Input::ShopClose | Input::Interact => {
                    self.shop = None;
                }
                _ => {}
            }
            return events;
        }
        match input {
            Input::Interact if self.pending_encounter.is_none() => self.interact(&mut events),
            Input::Interact => {}
            Input::Step(dir) if self.dialogue.is_some() => {
                // While a choice list is open, Up/Down move the cursor.
                if let Some(dialogue) = &mut self.dialogue
                    && let Some((_, options, cursor)) = &mut dialogue.choice
                {
                    match dir {
                        Facing::Up if *cursor > 0 => *cursor -= 1,
                        Facing::Down if *cursor + 1 < options.len() => *cursor += 1,
                        _ => {}
                    }
                }
            }
            Input::Step(dir) if self.pending_encounter.is_none() => {
                self.step(dir, &mut events);
            }
            Input::Step(_) => {}
            // Wander pauses during dialogue / pending encounters — a core
            // rule, not a renderer courtesy (doc 02 v1.3 #4).
            Input::Tick if self.dialogue.is_none() && self.pending_encounter.is_none() => {
                self.tick_wander(&mut events);
            }
            Input::Tick => {}
            Input::Save => events.push(WorldEvent::Saved),
            Input::Learn { replace } => {
                let mut learn_events = Vec::new();
                self.answer_learn(replace, &mut learn_events);
                events.extend(learn_events);
            }
            // Battle/shop/prompt vocabulary outside its mode: no-op.
            _ => {}
        }
        events
    }

    fn step(&mut self, dir: Facing, events: &mut Vec<WorldEvent>) {
        // Tap-to-turn (doc 02 v1.3 #1): a direction change only turns;
        // the windowed layer's held keys become repeated Steps.
        if self.facing != dir {
            self.facing = dir;
            events.push(WorldEvent::Faced(dir));
            return;
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

        // Line-of-sight engagement preempts encounter rolls
        // (doc 02 v1.3 #3).
        if self.try_engage(events) {
            return;
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
                    self.maybe_start_wild_battle(events);
                    break;
                }
            }
        }
    }

    /// Scans for an unflagged sighted trainer whose facing covers the
    /// player along a clear straight line (doc 02 v1.3 #3). Engages the
    /// first match in NPC order; returns true if one engaged.
    fn try_engage(&mut self, events: &mut Vec<WorldEvent>) -> bool {
        let map_id = self.current_map.clone();
        let Some(npcs) = self.npcs.get(&map_id) else {
            return false;
        };
        let mut engaged: Option<usize> = None;
        for (index, npc) in npcs.iter().enumerate() {
            if npc.sight_range == 0 {
                continue;
            }
            let flag = format!("engaged.{map_id}.{}", npc.id);
            if self.vars.flags.contains(&flag) {
                continue;
            }
            let (dx, dy) = npc.facing.delta();
            let mut clear = true;
            let mut seen = false;
            let (mut cx, mut cy) = npc.at;
            for _ in 0..npc.sight_range {
                let (Some(nx), Some(ny)) = (cx.checked_add_signed(dx), cy.checked_add_signed(dy))
                else {
                    break;
                };
                if (nx, ny) == self.player {
                    seen = true;
                    break;
                }
                if self.maps[&map_id].is_solid(nx, ny) {
                    clear = false;
                    break;
                }
                (cx, cy) = (nx, ny);
            }
            if seen && clear {
                engaged = Some(index);
                break;
            }
        }
        let Some(index) = engaged else { return false };
        let npc = &self.npcs[&map_id][index];
        let id = npc.id.clone();
        let script = npc.script.clone();
        self.vars.flags.insert(format!("engaged.{map_id}.{id}"));
        events.push(WorldEvent::Engaged { npc: id });
        if let Some(path) = script {
            self.start_script(&path, events);
        }
        true
    }

    fn interact(&mut self, events: &mut Vec<WorldEvent>) {
        // Advancing dialogue (or answering an open choice)?
        if let Some(dialogue) = &mut self.dialogue {
            if let Some((_, _, cursor)) = dialogue.choice.take() {
                dialogue.runner.resume_choice(cursor);
            }
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
            choice: None,
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
                StepResult::Effect(SideEffectReq::AskChoice { key, options }) => {
                    events.push(WorldEvent::DialogueChoice {
                        key: key.clone(),
                        options: options.clone(),
                    });
                    dialogue.choice = Some((key, options, 0));
                    self.dialogue = Some(dialogue);
                    return;
                }
                StepResult::Effect(SideEffectReq::Warp { map, x, y }) => {
                    self.current_map = map.clone();
                    self.player = (x, y);
                    events.push(WorldEvent::Warped { map, to: (x, y) });
                }
                StepResult::Effect(SideEffectReq::HealParty) => {
                    self.heal_party();
                    self.heal_point = (self.current_map.clone(), self.player);
                }
                StepResult::Effect(SideEffectReq::GiveMote { species, level }) => {
                    if let Some(registry) = &self.registry
                        && self.party.len() < 6
                        && let Some(mut given) =
                            registry.wild_individual(&species, level, &mut self.rng)
                    {
                        given.ot = "player".into();
                        self.party.push(given);
                        events.push(WorldEvent::MoteJoined { species });
                    }
                }
                StepResult::Effect(SideEffectReq::GiveItem { id, n }) => {
                    *self.bag.entry(id).or_insert(0) += n;
                }
                StepResult::Effect(SideEffectReq::StartBattle { trainer }) => {
                    // The battle takes over; the script resumes after.
                    self.dialogue = Some(dialogue);
                    self.start_trainer_battle(&trainer);
                    return;
                }
                StepResult::Effect(SideEffectReq::OpenShop { table }) => {
                    if let Some(registry) = &self.registry {
                        // P3 mart: every priced item; per-table stock in P4.
                        let _ = table;
                        let mut stock: Vec<ItemId> = registry
                            .items
                            .values()
                            .filter(|def| def.price > 0)
                            .map(|def| def.id.clone())
                            .collect();
                        stock.sort();
                        self.shop = Some((stock, 0));
                        events.push(WorldEvent::ShopOpened);
                    }
                }
                StepResult::Effect(_other) => {
                    // Music / cries / camera effects are presentation-only;
                    // the Bevy layer subscribes to them in its own pass.
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
                || self.maps[&map_id].trigger_at(nx, ny).is_some()
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

    /// Resolves a string key to its flavored text; missing keys show
    /// the key itself prefixed so playtests spot them instantly.
    pub fn text(&self, key: &str) -> String {
        self.strings
            .get(key)
            .cloned()
            .unwrap_or_else(|| format!("⟨{key}⟩"))
    }

    /// Starts the wild battle for `pending_encounter` if content is
    /// loaded and the player has a party.
    fn maybe_start_wild_battle(&mut self, events: &mut Vec<WorldEvent>) {
        let Some(registry) = &self.registry else {
            return;
        };
        let Some((species, level)) = self.pending_encounter.clone() else {
            return;
        };
        let Some(wild) = registry.wild_individual(&species, level, &mut self.rng) else {
            return;
        };
        if let Some(session) = BattleSession::wild(registry, &self.party, wild, &mut self.rng) {
            self.battle = Some(session);
        } else {
            // No conscious party — should not happen outside dev worlds.
            self.pending_encounter = None;
        }
        let _ = events;
    }

    /// Starts a trainer battle by id (script `StartBattle`).
    pub fn start_trainer_battle(&mut self, trainer_id: &undersong_core::ids::TrainerId) {
        let Some(registry) = &self.registry else {
            return;
        };
        let Some(trainer) = registry.trainers.get(trainer_id).cloned() else {
            return;
        };
        if self.vars.flags.contains(&trainer.defeat_flag) {
            return; // one-time fights stay won
        }
        if let Some(session) =
            BattleSession::trainer(registry, &self.party, &trainer, &mut self.rng)
        {
            self.battle = Some(session);
        }
    }

    fn battle_turn(&mut self, command: BattleCmd, events: &mut Vec<WorldEvent>) {
        let Some(mut session) = self.battle.take() else {
            return;
        };
        // Doc 02 v1.4 #3: illegal selections reject without consuming
        // the item or the turn (and without touching the rng stream).
        match command {
            BattleCmd::Bell => {
                let is_wild = matches!(session.context, BattleContext::Wild { .. });
                if !is_wild {
                    events.push(WorldEvent::ActionRejected {
                        reason_key: "ui.reject.bell_trainer".into(),
                    });
                    self.battle = Some(session);
                    return;
                }
                if self.peek_best_bell().is_none() {
                    events.push(WorldEvent::ActionRejected {
                        reason_key: "ui.reject.no_bells".into(),
                    });
                    self.battle = Some(session);
                    return;
                }
            }
            BattleCmd::Item => {
                let active = session.state.sides[0].active_mote();
                if self.peek_best_potion().is_none() {
                    events.push(WorldEvent::ActionRejected {
                        reason_key: "ui.reject.no_tonics".into(),
                    });
                    self.battle = Some(session);
                    return;
                }
                if active.hp >= active.max_hp() {
                    events.push(WorldEvent::ActionRejected {
                        reason_key: "ui.reject.full_hp".into(),
                    });
                    self.battle = Some(session);
                    return;
                }
            }
            _ => {}
        }
        let bell = if matches!(command, BattleCmd::Bell) {
            self.consume_best_bell()
        } else {
            None
        };
        let heal = if matches!(command, BattleCmd::Item) {
            self.consume_best_potion()
        } else {
            None
        };
        let stream = session.turn(command, bell, heal, &mut self.rng);

        // Learn prompts: queue events the engine surfaced this turn.
        for event in &stream {
            if let battle::BattleEvent::MoveLearnable { slot, move_id, .. } = event {
                let party_index = session
                    .party_map
                    .get(usize::from(*slot))
                    .copied()
                    .unwrap_or(0);
                session.pending_learn.push((party_index, move_id.clone()));
            }
        }
        events.push(WorldEvent::Battle(stream));

        match session.outcome() {
            None => {
                self.battle = Some(session);
            }
            Some(outcome) => {
                self.finish_battle(session, outcome, events);
            }
        }
    }

    fn peek_best_bell(&self) -> Option<()> {
        let registry = self.registry.as_ref()?;
        self.bag
            .iter()
            .any(|(id, n)| {
                *n > 0
                    && matches!(
                        registry.items.get(id).map(|d| &d.kind),
                        Some(data::ItemKind::Bell { .. })
                    )
            })
            .then_some(())
    }

    fn peek_best_potion(&self) -> Option<()> {
        let registry = self.registry.as_ref()?;
        self.bag
            .iter()
            .any(|(id, n)| {
                *n > 0
                    && matches!(
                        registry.items.get(id).map(|d| &d.kind),
                        Some(data::ItemKind::Potion { .. })
                    )
            })
            .then_some(())
    }

    fn consume_best_bell(&mut self) -> Option<undersong_core::moves::Frac> {
        let registry = self.registry.as_ref()?;
        // Best owned bell by multiplier.
        let mut best: Option<(ItemId, undersong_core::moves::Frac, u64)> = None;
        for (item_id, count) in &self.bag {
            if *count == 0 {
                continue;
            }
            if let Some(def) = registry.items.get(item_id)
                && let data::ItemKind::Bell { catch_mod } = &def.kind
            {
                let strength = u64::from(catch_mod.0) * 1000 / u64::from(catch_mod.1.max(1));
                if best.as_ref().is_none_or(|(_, _, s)| strength > *s) {
                    best = Some((item_id.clone(), *catch_mod, strength));
                }
            }
        }
        let (item_id, frac, _) = best?;
        if let Some(count) = self.bag.get_mut(&item_id) {
            *count -= 1;
            if *count == 0 {
                self.bag.remove(&item_id);
            }
        }
        Some(frac)
    }

    /// Consumes the weakest potion that exists (era bag etiquette).
    fn consume_best_potion(&mut self) -> Option<u16> {
        let registry = self.registry.as_ref()?;
        let mut best: Option<(ItemId, u16)> = None;
        for (item_id, count) in &self.bag {
            if *count == 0 {
                continue;
            }
            if let Some(def) = registry.items.get(item_id)
                && let data::ItemKind::Potion { hp } = &def.kind
                && best.as_ref().is_none_or(|(_, smallest)| hp < smallest)
            {
                best = Some((item_id.clone(), *hp));
            }
        }
        let (item_id, hp) = best?;
        if let Some(count) = self.bag.get_mut(&item_id) {
            *count -= 1;
            if *count == 0 {
                self.bag.remove(&item_id);
            }
        }
        Some(hp)
    }

    fn finish_battle(
        &mut self,
        session: BattleSession,
        outcome: battle::Outcome,
        events: &mut Vec<WorldEvent>,
    ) {
        // Fold survivors back into the party.
        for (battle_slot, party_index) in session.party_map.iter().enumerate() {
            if let (Some(mote), Some(individual)) = (
                session.state.sides[0].party.get(battle_slot),
                self.party.get_mut(*party_index),
            ) {
                Registry::fold_back(individual, mote);
            }
        }
        self.pending_encounter = None;
        events.push(WorldEvent::BattleFinished { outcome });

        match outcome {
            battle::Outcome::Caught => {
                if let Some(mut wild) = session.wild {
                    // Carry the battle-end condition into the caught Mote.
                    if let Some(foe) = session.state.sides[1].party.first() {
                        wild.hp = Some(foe.hp.max(1));
                        wild.status = foe.status.map(battle::mote::MajorStatus::ailment);
                        for learned in &mut wild.moves {
                            if let Some(battle_move) =
                                foe.moves.iter().find(|m| m.spec.id == learned.id)
                            {
                                learned.pp = battle_move.pp;
                            }
                        }
                    }
                    wild.ot = "player".into();
                    events.push(WorldEvent::MoteCaught {
                        species: wild.species.clone(),
                    });
                    if self.party.len() < 6 {
                        self.party.push(wild);
                    } else {
                        self.boxes.push(wild);
                    }
                }
            }
            battle::Outcome::Won { winner: 0 } => {
                if let BattleContext::Trainer { id } = &session.context
                    && let Some(registry) = &self.registry
                    && let Some(trainer) = registry.trainers.get(id)
                {
                    // Payout = class base × ace level (doc 02 §15).
                    let ace = trainer.party.iter().map(|m| m.level).max().unwrap_or(1);
                    let payout = trainer.payout_base * u32::from(ace);
                    self.money = self.money.saturating_add(payout);
                    self.vars.flags.insert(trainer.defeat_flag.clone());
                    for (item, count) in &trainer.reward_items {
                        *self.bag.entry(item.clone()).or_insert(0) += count;
                    }
                    events.push(WorldEvent::MoneyChanged { money: self.money });
                }
            }
            battle::Outcome::Won { .. } | battle::Outcome::Drawn => {
                // Loss: half money, heal, return to the rest point
                // (doc 02 §15).
                self.money /= 2;
                self.heal_party();
                self.current_map = self.heal_point.0.clone();
                self.player = self.heal_point.1;
                // Doc 02 v1.4 #5: loss wipes suspended scripts/shops.
                self.dialogue = None;
                self.shop = None;
                events.push(WorldEvent::Whiteout);
                events.push(WorldEvent::MoneyChanged { money: self.money });
            }
            battle::Outcome::Fled { .. } => {}
        }

        // Surface queued learn prompts (auto-learn when a slot is free).
        for (party_index, move_id) in session.pending_learn {
            let Some(individual) = self.party.get_mut(party_index) else {
                continue;
            };
            let species = individual.species.clone();
            if individual.moves.len() < 4 {
                if let Some(registry) = &self.registry
                    && let Some(spec) = registry.moves.get(&move_id)
                {
                    individual
                        .moves
                        .push(undersong_core::individual::LearnedMove {
                            id: move_id.clone(),
                            pp: spec.pp,
                            pp_ups: 0,
                        });
                    events.push(WorldEvent::MoveLearned { species, move_id });
                }
            } else {
                self.pending_learn_queue
                    .push((party_index, move_id.clone()));
                events.push(WorldEvent::LearnPrompt { species, move_id });
            }
        }

        // Queue level evolutions for members that LEVELED this battle
        // (doc 02 v1.4 #2: prompts on level-up only, never for fresh
        // catches already past the threshold).
        let mut leveled: Vec<usize> = Vec::new();
        for event in &session.last_events_all {
            if let battle::BattleEvent::LeveledUp { side: 0, slot, .. } = event
                && let Some(party_index) = session.party_map.get(usize::from(*slot))
            {
                leveled.push(*party_index);
            }
        }
        if let Some(registry) = &self.registry {
            for index in leveled {
                let Some(individual) = self.party.get(index) else {
                    continue;
                };
                if let Some((at_level, target)) = registry.evolutions.get(&individual.species)
                    && individual.level >= *at_level
                    && !self.pending_evolutions.iter().any(|(i, _)| *i == index)
                {
                    self.pending_evolutions.push((index, target.clone()));
                    events.push(WorldEvent::EvolutionPrompt {
                        from: individual.species.clone(),
                        into: target.clone(),
                    });
                }
            }
        }
    }

    /// Answers the oldest learn prompt outside battle.
    pub fn answer_learn(&mut self, replace: Option<u8>, events: &mut Vec<WorldEvent>) {
        let Some((party_index, move_id)) = self.pending_learn_queue.first().cloned() else {
            return;
        };
        self.pending_learn_queue.remove(0);
        let Some(individual) = self.party.get_mut(party_index) else {
            return;
        };
        if let Some(slot) = replace
            && let Some(registry) = &self.registry
            && let Some(spec) = registry.moves.get(&move_id)
            && let Some(learned) = individual.moves.get_mut(usize::from(slot))
        {
            *learned = undersong_core::individual::LearnedMove {
                id: move_id.clone(),
                pp: spec.pp,
                pp_ups: 0,
            };
            events.push(WorldEvent::MoveLearned {
                species: individual.species.clone(),
                move_id,
            });
        }
    }

    fn resolve_evolution(&mut self, accept: bool, events: &mut Vec<WorldEvent>) {
        let Some((party_index, target)) = self.pending_evolutions.first().cloned() else {
            return;
        };
        self.pending_evolutions.remove(0);
        if !accept {
            return;
        }
        if let Some(individual) = self.party.get_mut(party_index) {
            // Doc 02 v1.4 #1: evolution preserves current HP and status —
            // resolve clamps against the new max; no free heal.
            let from = individual.species.clone();
            individual.species = target.clone();
            events.push(WorldEvent::Evolved { from, into: target });
        }
    }

    fn shop_buy(&mut self, events: &mut Vec<WorldEvent>) {
        let Some(registry) = &self.registry else {
            return;
        };
        let Some((items, cursor)) = &self.shop else {
            return;
        };
        let Some(item_id) = items.get(*cursor).cloned() else {
            return;
        };
        let Some(def) = registry.items.get(&item_id) else {
            return;
        };
        if def.price == 0 || self.money < def.price {
            return;
        }
        self.money -= def.price;
        *self.bag.entry(item_id.clone()).or_insert(0) += 1;
        events.push(WorldEvent::ItemBought { item: item_id });
        events.push(WorldEvent::MoneyChanged { money: self.money });
    }

    pub fn heal_party(&mut self) {
        for individual in &mut self.party {
            individual.hp = None; // full at next resolve
            individual.status = None;
            if let Some(registry) = &self.registry {
                for learned in &mut individual.moves {
                    if let Some(spec) = registry.moves.get(&learned.id) {
                        learned.pp = spec.pp;
                    }
                }
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
                region: self.region_id.clone(),
                badge_bits: (1..=8u8).fold(0, |bits, n| {
                    if self.vars.flags.contains(&format!("badge.{n}")) {
                        bits | (1 << (n - 1))
                    } else {
                        bits
                    }
                }),
                score_pct: 0,
            },
            player: save::model::Player {
                name: name.into(),
                money: self.money,
                position: save::Position {
                    map: self.current_map.clone(),
                    x: self.player.0,
                    y: self.player.1,
                    facing: self.facing,
                },
                settings: save::Settings::default(),
            },
            party: self.party.clone(),
            boxes: vec![self.boxes.clone()],
            bag: BTreeMap::from([(
                "items".to_string(),
                self.bag.iter().map(|(id, n)| (id.clone(), *n)).collect(),
            )]),
            flags: self.vars.flags.clone(),
            vars: self.vars.vars.clone(),
            counters: BTreeMap::from([("steps".to_string(), self.steps)]),
            world_seed: self.world_seed,
            heal_point: Some((self.heal_point.0.to_string(), self.heal_point.1)),
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
        self.party = file.party.clone();
        self.boxes = file.boxes.first().cloned().unwrap_or_default();
        self.money = file.player.money;
        self.bag = file
            .bag
            .get("items")
            .map(|items| items.iter().cloned().collect())
            .unwrap_or_default();
        if let Some((map, at)) = &file.heal_point {
            self.heal_point = (map.as_str().into(), *at);
        }
        self.world_seed = file.world_seed;
        self.rng = BattleRng::from_seed(file.world_seed);
        self.dialogue = None;
        self.pending_encounter = None;

        // NPC positions are not persisted (wander drift is cosmetic);
        // if a spawn coincides with the restored player tile, nudge the
        // NPC deterministically to its first free neighbor.
        let map_id = self.current_map.clone();
        let player = self.player;
        if let Some(map) = self.maps.get(&map_id).cloned()
            && let Some(npcs) = self.npcs.get_mut(&map_id)
        {
            let taken: Vec<(u32, u32)> = npcs.iter().map(|n| n.at).collect();
            for npc in npcs.iter_mut() {
                if npc.at != player {
                    continue;
                }
                for dir in [Facing::Up, Facing::Down, Facing::Left, Facing::Right] {
                    let (dx, dy) = dir.delta();
                    if let (Some(nx), Some(ny)) = (
                        npc.at.0.checked_add_signed(dx),
                        npc.at.1.checked_add_signed(dy),
                    ) && !map.is_solid(nx, ny)
                        && map.trigger_at(nx, ny).is_none()
                        && (nx, ny) != player
                        && !taken.contains(&(nx, ny))
                    {
                        npc.at = (nx, ny);
                        break;
                    }
                }
            }
        }
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

/// Loads the real game world: the Cantorel region pack + core content,
/// with the battle registry attached. Maps and scripts come from the
/// pack (doc 04 §1 layout).
pub fn load_game_world(content_root: &std::path::Path, seed: u64) -> Result<WorldState, String> {
    let core_content = data::load_core(content_root).map_err(|e| e.to_string())?;
    let items = data::load_items(content_root).map_err(|e| e.to_string())?;
    let pack = data::load_region(content_root, "cantorel").map_err(|e| e.to_string())?;

    let maps_root = content_root.join("regions/cantorel/maps");
    let mut scripts = BTreeMap::new();
    for (id, map) in &pack.maps {
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

    let registry = Registry::from_content(&core_content, &pack, &items);
    let core_strings = data::load_core_strings(content_root).map_err(|e| e.to_string())?;
    let mut world = WorldState::new(
        pack.maps.clone(),
        scripts,
        pack.def.entry_map.clone(),
        pack.def.entry_spawn,
        seed,
    );
    let mut merged: std::collections::BTreeMap<String, String> = core_strings
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    for (key, value) in pack.strings.iter() {
        merged.insert(key.clone(), value.clone());
    }
    world.strings = merged.into();
    world.region_id = pack.def.id.clone();
    world.registry = Some(registry);
    Ok(world)
}
