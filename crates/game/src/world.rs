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
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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
    /// Use a bag item on a party member (potion/status/vitamin/stone) or
    /// the field (mute charm); TMs pass the replace slot.
    UseItem {
        item: ItemId,
        target: u8,
        slot: Option<u8>,
    },
    /// Answer an open Shift offer: switch to this bench index, or
    /// decline (None). Free — no battle turn passes (era Shift rule).
    Shift(Option<u8>),
    /// Fly to a visited town (Skybridge Aria, badge 6 + performer.sky).
    FlyTo(MapId),
    /// Repertoire moves (pure — the Box screen routes through these).
    BoxDeposit {
        party_index: u8,
    },
    BoxWithdraw {
        box_index: u32,
    },
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
    ItemUsed {
        item: ItemId,
        message_key: String,
    },
    /// Night fell / morning came (clock threshold crossings).
    ClockPhase {
        night: bool,
    },
    /// A Performance fired (key = ui.perform.*).
    Performed {
        performance: String,
    },
    /// The foe sent a replacement and Shift is on offer.
    ShiftOffered,
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
    /// Overworld clock in ticks; 1 step = 1 tick, 1200 = a day, night =
    /// the last third (doc 02 v1.6 #5).
    pub clock_ticks: u64,
    /// Mute Charm steps remaining (doc 02 v1.6 #3).
    pub mute_steps: u16,
    /// Ferry Song state: currently riding water tiles.
    pub surfing: bool,
    /// Era Shift rule: a free switch is on offer (doc 06 P4 Set/Shift;
    /// the presenter decides whether to surface it per settings).
    pub pending_shift: bool,
    /// Cached badge count for the +1 friendship-per-badge hook.
    badge_count: u8,
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
            clock_ticks: 0,
            mute_steps: 0,
            surfing: false,
            pending_shift: false,
            badge_count: 0,
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
            match input {
                Input::Battle(command) => self.battle_turn(command, &mut events),
                Input::Shift(choice) => self.answer_shift(choice, &mut events),
                _ => {}
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
                    if let Some((items, cursor)) = &mut self.shop
                        && !items.is_empty()
                    {
                        let last = items.len() as i32 - 1;
                        let next = (*cursor as i32 + i32::from(delta)).clamp(0, last);
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
            Input::UseItem { item, target, slot } => {
                self.use_item(&item, target, slot, &mut events);
            }
            Input::BoxDeposit { party_index } => {
                let index = usize::from(party_index);
                let conscious = self.party.iter().filter(|m| m.hp != Some(0)).count();
                if index < self.party.len()
                    && self.party.len() > 1
                    && (self.party[index].hp == Some(0) || conscious > 1)
                {
                    let member = self.party.remove(index);
                    self.boxes.push(member);
                }
            }
            Input::BoxWithdraw { box_index } => {
                let index = usize::try_from(box_index).unwrap_or(usize::MAX);
                if index < self.boxes.len() && self.party.len() < 6 {
                    let member = self.boxes.remove(index);
                    self.party.push(member);
                }
            }
            Input::FlyTo(destination) => {
                self.fly_to(&destination, &mut events);
            }
            Input::Learn { replace } => {
                let mut learn_events = Vec::new();
                self.answer_learn(replace, &mut learn_events);
                events.extend(learn_events);
            }
            // Battle/shop/prompt vocabulary outside its mode: no-op.
            _ => {}
        }
        // Chorus gate (doc 01 §6): Score ≥ 60% transcribed + all eight
        // Anchor Echoes. Reed tracks it; the Vault branch reads it.
        if !self.vars.flags.contains("chorus.ready") {
            let echoes = (1..=8u8).all(|n| self.vars.flags.contains(&format!("anchor_echo.{n}")));
            if echoes && let Some(registry) = &self.registry {
                let dex_total = registry.species.len().max(1);
                let caught = registry
                    .species
                    .keys()
                    .filter(|s| self.vars.flags.contains(&format!("dex.caught.{s}")))
                    .count();
                if caught * 100 >= dex_total * 60 {
                    self.vars.flags.insert("chorus.ready".into());
                }
            }
        }

        // Badge friendship hook (doc 02 v1.5 #5): +1 to the whole party
        // per badge earned while in it.
        let badges = (1..=8u8)
            .filter(|n| self.vars.flags.contains(&format!("badge.{n}")))
            .count() as u8;
        if badges > self.badge_count {
            for member in &mut self.party {
                member.friendship = member.friendship.saturating_add(1);
            }
            self.badge_count = badges;
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
        // Performance obstacles block until cleared (doc 02 §11);
        // pushed boulders block at their new spot.
        if self.obstacle_at(nx, ny).is_some() || self.boulder_pushed_to(nx, ny) {
            events.push(WorldEvent::Bumped { at: (nx, ny) });
            return;
        }
        // Water (ground id 5): Ferry Song surfs it, land walks end it.
        let entering_water = self.map().ground_at(nx, ny) == Some(5);
        if entering_water && !self.surfing {
            if self.vars.flags.contains("badge.4") && self.party_has_tag("performer.ferry") {
                self.surfing = true;
                events.push(WorldEvent::Performed {
                    performance: "ui.perform.ferry_song".into(),
                });
            } else {
                events.push(WorldEvent::Bumped { at: (nx, ny) });
                return;
            }
        }
        if !entering_water && self.surfing {
            self.surfing = false;
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
                        // Visited registry for Skybridge Aria (§11).
                        self.vars.flags.insert(format!("visited.{map}"));
                        self.vars.vars.insert(format!("spawn.{map}.x"), to.0 as i32);
                        self.vars.vars.insert(format!("spawn.{map}.y"), to.1 as i32);
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

        // Clock (doc 02 v1.6 #5): one tick per step; surface phase
        // crossings so the presenter can tint.
        let was_night = self.is_night();
        self.clock_ticks += 1;
        if self.is_night() != was_night {
            events.push(WorldEvent::ClockPhase {
                night: self.is_night(),
            });
        }
        // Mute Charm (v1.6 #3) burns a step regardless of patches.
        if self.mute_steps > 0 {
            self.mute_steps -= 1;
        }

        // Resonance patch roll (doc 02 §12): P per step, then a 12-slot
        // weighted species pick and a uniform level in the slot's range.
        // Night uses the map's night table when present (v1.6 #5).
        let table = if self.is_night() {
            self.map()
                .night_encounters
                .clone()
                .or_else(|| self.map().encounters.clone())
        } else {
            self.map().encounters.clone()
        };
        if self.map().is_patch(nx, ny)
            && let Some(encounters) = table
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
                    // Mute Charm: no engagement below the lead's level
                    // (doc 02 §12; rng already consumed — stream-stable).
                    let lead_level = self.party.first().map(|p| p.level).unwrap_or(0);
                    if self.mute_steps > 0 && level < lead_level {
                        break;
                    }
                    self.pending_encounter = Some((species.clone(), level));
                    // Score (dex) registry: seen on encounter.
                    self.vars.flags.insert(format!("dex.seen.{species}"));
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

    /// The uncleared obstacle on a tile, if any (cleared ones persist
    /// as namespaced flags so saves carry them).
    fn obstacle_at(&self, x: u32, y: u32) -> Option<data::ObstacleKind> {
        let map_id = self.current_map.clone();
        self.maps[&map_id]
            .obstacles
            .iter()
            .find(|o| o.at == (x, y))
            .map(|o| o.kind)
            .filter(|_| {
                !self
                    .vars
                    .flags
                    .contains(&format!("cleared.{map_id}.{x}.{y}"))
            })
    }

    /// A boulder pushed to this tile earlier (blocks like an obstacle).
    fn boulder_pushed_to(&self, x: u32, y: u32) -> bool {
        self.vars
            .flags
            .contains(&format!("pushed.{}.{x}.{y}", self.current_map))
    }

    /// Facing-tile Performance interactions (doc 02 §11).
    fn try_perform(&mut self, events: &mut Vec<WorldEvent>) -> bool {
        let (dx, dy) = self.facing.delta();
        let Some(tx) = self.player.0.checked_add_signed(dx) else {
            return false;
        };
        let Some(ty) = self.player.1.checked_add_signed(dy) else {
            return false;
        };
        let Some(kind) = self.obstacle_at(tx, ty) else {
            return false;
        };
        let (badge, tag, key) = match kind {
            data::ObstacleKind::Brush => {
                ("badge.2", "performer.clear", "ui.perform.clearing_chord")
            }
            data::ObstacleKind::CrackedRock => {
                ("badge.3", "performer.smash", "ui.perform.tunneling_bass")
            }
            data::ObstacleKind::Boulder => ("badge.5", "performer.lift", "ui.perform.lift_motif"),
        };
        if !self.vars.flags.contains(badge) {
            events.push(WorldEvent::ItemUsed {
                item: "performance".into(),
                message_key: "ui.perform.no_badge".into(),
            });
            return true;
        }
        if !self.party_has_tag(tag) {
            events.push(WorldEvent::ItemUsed {
                item: "performance".into(),
                message_key: "ui.perform.no_tag".into(),
            });
            return true;
        }
        match kind {
            data::ObstacleKind::Boulder => {
                // Push one tile along the facing if free. The boulder
                // KEEPS blocking at its new spot: the cleared flag frees
                // the old tile and a pushed.<map>.<x>.<y> flag blocks
                // the new one (doc 02 §11 anchor-boulder puzzles).
                let Some(bx) = tx.checked_add_signed(dx) else {
                    return true;
                };
                let Some(by) = ty.checked_add_signed(dy) else {
                    return true;
                };
                let free = self.maps[&self.current_map].in_bounds(bx, by)
                    && !self.maps[&self.current_map].is_solid(bx, by)
                    && self.obstacle_at(bx, by).is_none()
                    && !self.boulder_pushed_to(bx, by)
                    && !self.npc_blocking(bx, by);
                if free {
                    let map_id = self.current_map.clone();
                    self.vars
                        .flags
                        .insert(format!("cleared.{map_id}.{tx}.{ty}"));
                    self.vars.flags.insert(format!("pushed.{map_id}.{bx}.{by}"));
                    events.push(WorldEvent::Performed {
                        performance: key.into(),
                    });
                }
            }
            _ => {
                let map_id = self.current_map.clone();
                self.vars
                    .flags
                    .insert(format!("cleared.{map_id}.{tx}.{ty}"));
                events.push(WorldEvent::Performed {
                    performance: key.into(),
                });
            }
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
        // Performance obstacles claim the facing tile first (doc 02 §11).
        if self.try_perform(events) {
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
                        self.vars.flags.insert(format!("dex.seen.{species}"));
                        self.vars.flags.insert(format!("dex.caught.{species}"));
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

    /// Night = the last third of the 1200-tick day (doc 02 v1.6 #5).
    pub fn is_night(&self) -> bool {
        self.clock_ticks % 1200 >= 800
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
        if let Some(mut session) = BattleSession::wild(registry, &self.party, wild, &mut self.rng) {
            // Ambient weather zone (doc 02 v1.6 #6) + the night flag for
            // Vesper bells (v1.6 #4).
            if let Some(kind) = self.map().weather {
                session.state.weather = Some((kind, 5));
            }
            session.night = self.is_night();
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
        if self.vars.flags.contains(&trainer.defeat_flag) && !trainer.rematch {
            return; // one-time fights stay won (rematch trainers re-engage)
        }
        if let Some(mut session) =
            BattleSession::trainer(registry, &self.party, &trainer, &mut self.rng)
        {
            if let Some(kind) = self.map().weather {
                session.state.weather = Some((kind, 5));
            }
            session.night = self.is_night();
            for foe in &session.state.sides[1].party {
                self.vars.flags.insert(format!("dex.seen.{}", foe.species));
            }
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
            BattleCmd::Run => {
                if matches!(session.context, BattleContext::Trainer { .. }) {
                    events.push(WorldEvent::ActionRejected {
                        reason_key: "ui.reject.run_trainer".into(),
                    });
                    self.battle = Some(session);
                    return;
                }
            }
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
                // In doubles the second declaration heals position 1 —
                // gate against the declaring position's mote.
                let declaring = usize::from(u8::from(session.pending_declaration.is_some()));
                let active = match session.state.sides[0].positions.get(declaring) {
                    Some(position) => {
                        &session.state.sides[0].party[usize::from(position.party_index)]
                    }
                    None => session.state.sides[0].active_mote(),
                };
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
        // Zone weather refreshes every round (doc 02 v1.6 #6).
        if let Some(kind) = self.map().weather
            && session.outcome().is_none()
        {
            session.state.weather = Some((kind, 5));
        }
        // Era Shift: foe replacement entered → offer a free switch when
        // a conscious bench exists (presenter hides it in Set mode).
        let foe_replaced_after_ko = stream
            .iter()
            .any(|e| matches!(e, battle::BattleEvent::SwitchedIn { side: 1, .. }))
            && stream
                .iter()
                .any(|e| matches!(e, battle::BattleEvent::Fainted { target: 1, .. }));
        if session.outcome().is_none()
            && session.state.format == battle::Format::Single
            && matches!(session.context, BattleContext::Trainer { .. })
            && foe_replaced_after_ko
        {
            let bench: Vec<u8> = (0..session.state.sides[0].party.len() as u8)
                .filter(|i| {
                    *i != session.state.sides[0].positions[0].party_index
                        && !session.state.sides[0].party[usize::from(*i)].is_fainted()
                })
                .collect();
            if !bench.is_empty() {
                self.pending_shift = true;
                events.push(WorldEvent::ShiftOffered);
            }
        }

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
                        Some(
                            data::ItemKind::Bell { .. }
                                | data::ItemKind::OvertureBell
                                | data::ItemKind::CradleBell
                                | data::ItemKind::VesperBell
                                | data::ItemKind::Coda
                        )
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
        use undersong_core::moves::Frac;
        let registry = self.registry.as_ref()?;
        // Conditional context (doc 02 v1.6 #4): first turn? target
        // lulled/frosted? night?
        let (first_turn, target_lulled) = match &self.battle {
            Some(session) => {
                let foe = session.state.sides[1].active_mote();
                (
                    session.state.turn == 0,
                    matches!(
                        foe.status,
                        Some(battle::mote::MajorStatus::Sleep { .. })
                            | Some(battle::mote::MajorStatus::Freeze)
                    ),
                )
            }
            None => (false, false),
        };
        let night = self.battle.as_ref().map(|s| s.night).unwrap_or(false);
        let effective = |kind: &data::ItemKind| -> Option<Frac> {
            match kind {
                data::ItemKind::Bell { catch_mod } => Some(*catch_mod),
                data::ItemKind::OvertureBell => {
                    Some(if first_turn { Frac(4, 1) } else { Frac(1, 1) })
                }
                data::ItemKind::CradleBell => Some(if target_lulled {
                    Frac(7, 2)
                } else {
                    Frac(1, 1)
                }),
                data::ItemKind::VesperBell => Some(if night { Frac(7, 2) } else { Frac(1, 1) }),
                // Coda: certainty as an overwhelming rational (catch.rs
                // clamps a ≥ 255 to an immediate catch).
                data::ItemKind::Coda => Some(Frac(1000, 1)),
                _ => None,
            }
        };
        let mut best: Option<(ItemId, Frac, u64)> = None;
        for (item_id, count) in &self.bag {
            if *count == 0 {
                continue;
            }
            if let Some(def) = registry.items.get(item_id)
                && let Some(frac) = effective(&def.kind)
            {
                let strength = u64::from(frac.0) * 1000 / u64::from(frac.1.max(1));
                if best.as_ref().is_none_or(|(_, _, s)| strength > *s) {
                    best = Some((item_id.clone(), frac, strength));
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
        self.pending_shift = false;
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
                    self.vars.flags.insert(format!("dex.seen.{}", wild.species));
                    self.vars
                        .flags
                        .insert(format!("dex.caught.{}", wild.species));
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
                    let first_win = !self.vars.flags.contains(&trainer.defeat_flag);
                    self.vars.flags.insert(trainer.defeat_flag.clone());
                    if first_win {
                        for (item, count) in &trainer.reward_items {
                            *self.bag.entry(item.clone()).or_insert(0) += count;
                        }
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

        // Friendship (doc 02 v1.5 #5): +2 per level-up, −5 per faint.
        for event in &session.last_events_all {
            match event {
                battle::BattleEvent::LeveledUp { side: 0, slot, .. } => {
                    if let Some(party_index) = session.party_map.get(usize::from(*slot))
                        && let Some(member) = self.party.get_mut(*party_index)
                    {
                        member.friendship = member.friendship.saturating_add(2);
                    }
                }
                battle::BattleEvent::Fainted {
                    target: 0,
                    party_index,
                    ..
                } => {
                    // party_index is stable across switches; the position
                    // slot must NOT be used for fold-back mapping.
                    if let Some(world_index) = session.party_map.get(usize::from(*party_index))
                        && let Some(member) = self.party.get_mut(*world_index)
                    {
                        member.friendship = member.friendship.saturating_sub(5);
                    }
                }
                _ => {}
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
                let Some(methods) = registry.all_evolutions.get(&individual.species) else {
                    continue;
                };
                // Level-up checks Level(n) and Friendship≥220 (doc 02
                // §9, v1.5 #5); item methods fire from the bag.
                let target = methods.iter().find_map(|(method, into)| match method {
                    data::EvolutionMethod::Level(at_level) if individual.level >= *at_level => {
                        Some(into.clone())
                    }
                    data::EvolutionMethod::Friendship if individual.friendship >= 220 => {
                        Some(into.clone())
                    }
                    _ => None,
                });
                if let Some(target) = target
                    && !self.pending_evolutions.iter().any(|(i, _)| *i == index)
                {
                    self.pending_evolutions.push((index, target.clone()));
                    events.push(WorldEvent::EvolutionPrompt {
                        from: individual.species.clone(),
                        into: target,
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
            self.vars.flags.insert(format!("dex.seen.{target}"));
            self.vars.flags.insert(format!("dex.caught.{target}"));
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

    /// Bag item on a party member or the field (doc 02 v1.6).
    fn use_item(
        &mut self,
        item: &ItemId,
        target: u8,
        slot: Option<u8>,
        events: &mut Vec<WorldEvent>,
    ) {
        let Some(registry) = &self.registry else {
            return;
        };
        let Some(def) = registry.items.get(item).cloned() else {
            return;
        };
        let owned = self.bag.get(item).copied().unwrap_or(0);
        if owned == 0 {
            return;
        }
        let target_index = usize::from(target);
        let mut consumed = false;
        let mut message = String::new();
        match &def.kind {
            data::ItemKind::Potion { hp } => {
                if let Some(member) = self.party.get_mut(target_index) {
                    let max = registry
                        .resolve(member)
                        .map(|m| m.max_hp())
                        .unwrap_or(u16::MAX);
                    let current = member.hp.unwrap_or(max);
                    if current < max {
                        member.hp = Some((current + hp).min(max));
                        consumed = true;
                        message = "ui.item.heal".into();
                    }
                }
            }
            data::ItemKind::StatusHeal => {
                if let Some(member) = self.party.get_mut(target_index)
                    && member.status.is_some()
                {
                    member.status = None;
                    consumed = true;
                    message = "ui.item.status_heal".into();
                }
            }
            data::ItemKind::Vitamin { stat } => {
                if let Some(member) = self.party.get_mut(target_index) {
                    // +10 EVs, fail ≥100 in-stat or 510 total (v1.6 #1).
                    let current = member.evs.get(*stat);
                    let total: u32 = undersong_core::stats::Stat::ALL
                        .into_iter()
                        .map(|s| u32::from(member.evs.get(s)))
                        .sum();
                    if current < 100 && total + 10 <= 510 {
                        member.evs.set(*stat, current + 10);
                        member.friendship = member.friendship.saturating_add(5);
                        consumed = true;
                        message = "ui.item.vitamin_used".into();
                    } else {
                        message = "ui.item.vitamin_fail".into();
                    }
                }
            }
            data::ItemKind::Tm { move_id } => {
                if let Some(member) = self.party.get_mut(target_index) {
                    let allowed = registry.species.contains_key(&member.species)
                        && registry
                            .tm_sets
                            .get(&member.species)
                            .is_some_and(|set| set.iter().any(|tm| tm == item));
                    if !allowed {
                        message = "ui.item.tm_wrong_species".into();
                    } else if member.moves.iter().any(|m| &m.id == move_id) {
                        message = "ui.item.tm_taught".into(); // already known: no-op
                    } else {
                        let pp = registry.moves.get(move_id).map(|m| m.pp).unwrap_or(10);
                        let learned = undersong_core::individual::LearnedMove {
                            id: move_id.clone(),
                            pp,
                            pp_ups: 0,
                        };
                        if member.moves.len() < 4 {
                            member.moves.push(learned);
                            message = "ui.item.tm_taught".into();
                        } else if let Some(slot) = slot
                            && let Some(existing) = member.moves.get_mut(usize::from(slot))
                        {
                            *existing = learned;
                            message = "ui.item.tm_taught".into();
                        } else {
                            message = "ui.item.tm_no_slot".into();
                        }
                    }
                    // TMs are reusable (v1.6 #2): never consumed.
                }
            }
            data::ItemKind::MuteCharm => {
                self.mute_steps = 200;
                consumed = true;
                message = "ui.item.mute_charm".into();
            }
            data::ItemKind::Held => {
                // Equip: bag → held slot; any current held item returns
                // to the bag (era swap).
                if let Some(member) = self.party.get_mut(target_index) {
                    let previous = member.held_item.replace(item.clone());
                    consumed = true;
                    if let Some(previous) = previous {
                        *self.bag.entry(previous).or_insert(0) += 1;
                    }
                    message = "ui.item.held".into();
                }
            }
            data::ItemKind::Key => {
                // Duet Stone & friends: item-method evolutions (doc 02 §9).
                if let Some(member) = self.party.get(target_index) {
                    let evolution =
                        registry
                            .all_evolutions
                            .get(&member.species)
                            .and_then(|methods| {
                                methods.iter().find(|(method, _)| match method {
                                    data::EvolutionMethod::Item(needed) => needed == item,
                                    data::EvolutionMethod::DuetStone => {
                                        item.as_str() == "duet_stone"
                                    }
                                    _ => false,
                                })
                            });
                    if let Some((_, into)) = evolution.cloned() {
                        let from = member.species.clone();
                        if let Some(member) = self.party.get_mut(target_index) {
                            member.species = into.clone();
                        }
                        self.vars.flags.insert(format!("dex.seen.{into}"));
                        self.vars.flags.insert(format!("dex.caught.{into}"));
                        consumed = true;
                        events.push(WorldEvent::Evolved { from, into });
                    }
                }
            }
            _ => {}
        }
        if consumed && let Some(count) = self.bag.get_mut(item) {
            *count -= 1;
            if *count == 0 {
                self.bag.remove(item);
            }
        }
        if !message.is_empty() {
            events.push(WorldEvent::ItemUsed {
                item: item.clone(),
                message_key: message,
            });
        }
    }

    /// Era Shift rule: free switch while the offer stands.
    fn answer_shift(&mut self, choice: Option<u8>, events: &mut Vec<WorldEvent>) {
        if !self.pending_shift {
            return;
        }
        self.pending_shift = false;
        let Some(session) = &mut self.battle else {
            return;
        };
        if let Some(to) = choice {
            let (next, stream) = battle::turn::free_switch(&session.state, 0, to);
            session.state = next;
            events.push(WorldEvent::Battle(stream));
        }
    }

    /// Skybridge Aria (doc 02 §11): fly to a visited town.
    fn fly_to(&mut self, destination: &MapId, events: &mut Vec<WorldEvent>) {
        let allowed = self.vars.flags.contains("badge.6")
            && self.party_has_tag("performer.sky")
            && self.vars.flags.contains(&format!("visited.{destination}"))
            && self.maps.contains_key(destination);
        if !allowed {
            return;
        }
        self.current_map = destination.clone();
        // Land at the map's first walkable tile ring from center — towns
        // register a spawn via the visited flag's recorded position.
        let spawn = self
            .vars
            .vars
            .get(&format!("spawn.{destination}.x"))
            .copied()
            .zip(
                self.vars
                    .vars
                    .get(&format!("spawn.{destination}.y"))
                    .copied(),
            )
            .map(|(x, y)| (x.max(0) as u32, y.max(0) as u32))
            .unwrap_or((1, 1));
        self.player = spawn;
        events.push(WorldEvent::Performed {
            performance: "ui.perform.skybridge_aria".into(),
        });
        events.push(WorldEvent::Warped {
            map: destination.clone(),
            to: spawn,
        });
    }

    /// Any party member carries the tag (doc 02 §11: party-wide skills).
    pub fn party_has_tag(&self, tag: &str) -> bool {
        let Some(registry) = &self.registry else {
            return false;
        };
        self.party.iter().any(|member| {
            registry
                .species
                .get(&member.species)
                .is_some_and(|spec| spec.tags.iter().any(|t| t == tag))
        })
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
            counters: BTreeMap::from([
                ("steps".to_string(), self.steps),
                ("clock_ticks".to_string(), self.clock_ticks),
                ("mute_steps".to_string(), u64::from(self.mute_steps)),
                ("surfing".to_string(), u64::from(self.surfing)),
            ]),
            world_seed: self.world_seed,
            heal_point: Some((self.heal_point.0.to_string(), self.heal_point.1)),
        }
    }

    /// Restores position/flags/counters from a save into a fresh world
    /// built over the same content.
    /// Contract: saves are only written outside battles/shops/prompts
    /// (rest points, menu, post-battle), so those modal fields stay at
    /// their fresh-world defaults here by design.
    pub fn restore(&mut self, file: &save::SaveFile) {
        self.current_map = file.player.position.map.clone();
        self.player = (file.player.position.x, file.player.position.y);
        self.facing = file.player.position.facing;
        self.vars.flags = file.flags.clone();
        self.vars.vars = file.vars.clone();
        self.steps = file.counters.get("steps").copied().unwrap_or(0);
        self.clock_ticks = file.counters.get("clock_ticks").copied().unwrap_or(0);
        self.surfing = file.counters.get("surfing").copied().unwrap_or(0) == 1;
        self.badge_count = (1..=8u8)
            .filter(|n| self.vars.flags.contains(&format!("badge.{n}")))
            .count() as u8;
        self.mute_steps =
            u16::try_from(file.counters.get("mute_steps").copied().unwrap_or(0)).unwrap_or(0);
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
