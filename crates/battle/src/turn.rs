//! `step()` — one full battle turn (doc 02 v1.1 #1 phase structure).
//!
//! Pure: `(state, actions, rng) → (state', events)`. All policy decisions
//! cite doc 02; engine-level resolutions of caller errors (illegal slot,
//! illegal switch, out-of-range target) are deterministic and documented
//! inline.
//!
//! Doubles (doc 02 v1.5 #2): every per-active mechanism is keyed by a
//! `(side, position)` slot. The singles rng stream is bit-identical to
//! the pre-doubles engine: every ordering helper degenerates to the old
//! two-party comparison, and target resolution consumes no rng.

use undersong_core::moves::{
    Ailment, Effect, EffectTarget, FixedAmount, MoveCategory, MoveFlags, MoveSpec, MoveTarget,
};
use undersong_core::rng::BattleRng;
use undersong_core::stats::Stat;
use undersong_core::types::Type;

use crate::abilities::{Ability, HeldItem};
use crate::actions::{Action, TurnActions};
use crate::catch::attune;
use crate::damage::{DamageContext, compute_damage, crit_chance};
use crate::events::{BattleEvent, Outcome, SideId};
use crate::exp::{apply_exp, exp_gain};
use crate::mote::MajorStatus;
use crate::state::{BattleKind, BattleState, Format};

use crate::stats::{StageStat, acc_stage_factor, stage_multiplied};

/// Maximum turns before a forced draw (doc 03 §2: battles always
/// terminate ≤ 1000 turns).
pub const TURN_LIMIT: u16 = 1000;

/// The built-in no-PP fallback (doc 02 v1.1 #3): 50 power, typeless,
/// physical, never misses, cannot crit, ¼-damage recoil.
fn last_resort_spec() -> MoveSpec {
    MoveSpec {
        id: "last_resort_hum".into(),
        name_key: "move.last_resort_hum".into(),
        // Typeless: represented as Feral for A/D selection but exempted
        // from STAB and type effectiveness by the `typeless` path below.
        r#type: Type::Feral,
        category: MoveCategory::Physical,
        power: 50,
        accuracy: 0,
        pp: 1,
        priority: 0,
        target: MoveTarget::Foe,
        flags: MoveFlags::default(),
        effects: vec![],
    }
}

/// Era "Shift" rule: after a foe trainer's replacement enters, the
/// player may switch for free (no turn passes, no rng consumed beyond
/// none — entry abilities fire). Pure helper for the session layer.
pub fn free_switch(
    state: &BattleState,
    side: crate::events::SideId,
    to: u8,
) -> (BattleState, Vec<crate::events::BattleEvent>) {
    let mut engine = Engine {
        state: state.clone(),
        events: Vec::new(),
        cancelled: [[false; 2]; 2],
    };
    engine.perform_switch_public(side, to);
    (engine.state, engine.events)
}

pub fn step(
    state: &BattleState,
    actions: &TurnActions,
    rng: &mut BattleRng,
) -> (BattleState, Vec<BattleEvent>) {
    let mut engine = Engine {
        state: state.clone(),
        events: Vec::new(),
        cancelled: [[false; 2]; 2],
    };
    if engine.state.is_over() {
        return (engine.state, engine.events);
    }
    engine.run_turn(actions, rng);
    (engine.state, engine.events)
}

/// A `(side, position)` field slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Slot {
    side: SideId,
    pos: u8,
}

/// One position's normalized intent for the turn.
#[derive(Debug, Clone, Copy)]
struct Intent {
    action: Action,
    target: u8,
}

struct Engine {
    state: BattleState,
    events: Vec<BattleEvent>,
    /// `[side][position]`: set when the slot's pending action is consumed
    /// mid-turn (ForceSwitch drag, doc 02 v1.2 #10).
    cancelled: [[bool; 2]; 2],
}

impl Engine {
    // ----- slot plumbing ----------------------------------------------

    fn mote(&self, slot: Slot) -> &crate::mote::BattleMote {
        self.state.side(slot.side).mote_at(slot.pos)
    }

    fn mote_mut(&mut self, slot: Slot) -> &mut crate::mote::BattleMote {
        self.state.side_mut(slot.side).mote_at_mut(slot.pos)
    }

    fn pstate(&self, slot: Slot) -> &crate::state::ActiveState {
        &self.state.side(slot.side).positions[usize::from(slot.pos)].state
    }

    fn pstate_mut(&mut self, slot: Slot) -> &mut crate::state::ActiveState {
        &mut self.state.side_mut(slot.side).positions[usize::from(slot.pos)].state
    }

    fn position_count(&self, side: SideId) -> u8 {
        self.state.side(side).position_count()
    }

    /// All field slots in canonical `(side asc, position asc)` order.
    fn all_slots(&self) -> Vec<Slot> {
        let mut slots = Vec::with_capacity(4);
        for side in 0..2u8 {
            for pos in 0..self.position_count(side) {
                slots.push(Slot { side, pos });
            }
        }
        slots
    }

    fn cancelled(&self, slot: Slot) -> bool {
        self.cancelled[usize::from(slot.side)][usize::from(slot.pos)]
    }

    fn cancel(&mut self, slot: Slot) {
        self.cancelled[usize::from(slot.side)][usize::from(slot.pos)] = true;
    }

    /// Normalizes the submitted actions into a per-slot grid. Duplicate
    /// `(side, position)` declarations resolve first-wins; missing or
    /// out-of-range ones act as `Action::None` (deterministic caller-error
    /// policy, like illegal switches).
    fn normalize(&self, actions: &TurnActions) -> [[Intent; 2]; 2] {
        let mut grid = [[Intent {
            action: Action::None,
            target: 0,
        }; 2]; 2];
        let mut set = [[false; 2]; 2];
        for declared in &actions.actions {
            let (side, pos) = (usize::from(declared.side), usize::from(declared.position));
            if declared.side < 2
                && declared.position < self.position_count(declared.side)
                && !set[side][pos]
            {
                set[side][pos] = true;
                grid[side][pos] = Intent {
                    action: declared.action,
                    target: declared.target_position,
                };
            }
        }
        grid
    }

    fn intent(grid: &[[Intent; 2]; 2], slot: Slot) -> Intent {
        grid[usize::from(slot.side)][usize::from(slot.pos)]
    }

    // ----- the turn ----------------------------------------------------

    fn run_turn(&mut self, actions: &TurnActions, rng: &mut BattleRng) {
        let grid = self.normalize(actions);
        if self.state.turn == 0 {
            // Battle start: leads' entry abilities, fast slots first
            // (doc 02 §10/v1.5 #4).
            let mut order = self.all_slots();
            self.order_by_speed(&mut order, rng);
            for slot in order {
                self.on_entry(slot);
            }
        }
        self.state.turn += 1;
        self.events
            .push(BattleEvent::TurnStarted { n: self.state.turn });
        if self.state.turn > TURN_LIMIT {
            self.end(Outcome::Drawn);
            return;
        }

        // Stale per-turn volatiles from any previous turn.
        for slot in self.all_slots() {
            let st = self.pstate_mut(slot);
            st.flinched = false;
            st.protected = false;
        }

        // Phase a — escape attempts (doc 02 v1.1 #1a; formula §12).
        for slot in self.all_slots() {
            if matches!(Self::intent(&grid, slot).action, Action::Run) {
                self.try_escape(slot.side, rng);
                if self.state.is_over() {
                    return;
                }
            }
        }

        // Phase b — switches, faster slots first (v1.1 #1b).
        let mut switchers: Vec<Slot> = self
            .all_slots()
            .into_iter()
            .filter(|&s| matches!(Self::intent(&grid, s).action, Action::Switch { .. }))
            .collect();
        self.order_by_speed(&mut switchers, rng);
        for slot in switchers {
            if let Action::Switch { to } = Self::intent(&grid, slot).action {
                self.try_switch(slot, to);
            }
        }

        // Phase c — bell use (v1.1 #1c; doc 02 §8). Player side only:
        // attunement is a trainer verb, the wild side has no bells.
        for pos in 0..self.position_count(0) {
            let ringer = Slot { side: 0, pos };
            if let Action::UseBell { mut bell_mod } = Self::intent(&grid, ringer).action {
                // keysmith: ×2 catch-assist (doc 02 §10).
                if self.mote(ringer).ability == Ability::Keysmith {
                    bell_mod = undersong_core::moves::Frac(bell_mod.0 * 2, bell_mod.1);
                }
                if matches!(self.state.kind, BattleKind::Wild) {
                    let result = attune(self.state.side(1).active_mote(), bell_mod, rng);
                    self.events.push(BattleEvent::AttuneAttempt {
                        rings: result.rings,
                        caught: result.caught,
                    });
                    if result.caught {
                        self.end(Outcome::Caught);
                        return;
                    }
                } else {
                    self.events.push(BattleEvent::MoveFailed { side: 0 });
                }
            }
        }
        for pos in 0..self.position_count(1) {
            if matches!(
                Self::intent(&grid, Slot { side: 1, pos }).action,
                Action::UseBell { .. }
            ) {
                self.events.push(BattleEvent::MoveFailed { side: 1 });
            }
        }

        // Phase d — moves (v1.1 #1d): priority desc → speed desc → rng.
        struct Mover {
            slot: Slot,
            move_slot: u8,
            target: u8,
            priority: i8,
        }
        let mut movers: Vec<Mover> = Vec::new();
        for slot in self.all_slots() {
            let intent = Self::intent(&grid, slot);
            if let Action::Move { slot: move_slot } = intent.action {
                let resolved = self.resolve_slot(slot, move_slot);
                let priority = match resolved {
                    Some(s) => self.mote(slot).moves[usize::from(s)].spec.priority,
                    None => 0, // Last Resort Hum
                };
                movers.push(Mover {
                    slot,
                    move_slot: resolved.unwrap_or(u8::MAX),
                    target: intent.target,
                    priority,
                });
            }
        }
        // Stable: equal priority keeps (side, position) order until the
        // speed pass below.
        movers.sort_by_key(|m| std::cmp::Reverse(m.priority));
        // Speed-order each maximal equal-priority run, rng on exact ties.
        let mut start = 0usize;
        while start < movers.len() {
            let mut end = start + 1;
            while end < movers.len() && movers[end].priority == movers[start].priority {
                end += 1;
            }
            if end - start >= 2 {
                let mut run: Vec<Slot> = movers[start..end].iter().map(|m| m.slot).collect();
                self.order_by_speed(&mut run, rng);
                let mut reordered: Vec<Mover> = Vec::with_capacity(end - start);
                for want in &run {
                    let at = movers[start..end]
                        .iter()
                        .position(|m| m.slot == *want)
                        .expect("run member");
                    reordered.push(Mover {
                        slot: movers[start + at].slot,
                        move_slot: movers[start + at].move_slot,
                        target: movers[start + at].target,
                        priority: movers[start + at].priority,
                    });
                }
                movers.splice(start..end, reordered);
            }
            start = end;
        }
        for mover in &movers {
            if self.state.is_over() {
                return;
            }
            if self.mote(mover.slot).is_fainted() || self.cancelled(mover.slot) {
                continue;
            }
            self.act(mover.slot, mover.move_slot, mover.target, rng);
        }
        if self.state.is_over() {
            return;
        }

        // Phase e — end of turn (v1.1 #2).
        self.end_of_turn(rng);
        if self.state.is_over() {
            return;
        }

        // Auto-replace fainted positions (engine policy for the headless
        // sim; the game layer will route a player choice through Switch
        // actions when the presenter exists). End-of-turn in both formats
        // so doc 02 v1.5 #2's mid-turn retarget/fizzle rules stay
        // meaningful; a benchless doubles position keeps its fainted Mote.
        for slot in self.all_slots() {
            if self.mote(slot).is_fainted()
                && let Some(replacement) = self.state.side(slot.side).first_replacement()
            {
                self.perform_switch(slot, replacement);
            }
        }

        self.check_outcome();
    }

    // ----- ordering helpers -------------------------------------------

    /// Effective speed (doc 02 v1.1 #7): stage-modified, then paralysis
    /// quarters it.
    fn effective_speed(&self, slot: Slot) -> u32 {
        let mote = self.mote(slot);
        let mut spe = stage_multiplied(
            u32::from(mote.stats.spe),
            self.pstate(slot).stages.get(StageStat::Spe),
        );
        if matches!(mote.status, Some(MajorStatus::Paralysis))
            && mote.ability != Ability::MetronomeSoul
        {
            spe /= 4;
        }
        spe
    }

    /// Sorts slots by effective speed desc (stable: equal speeds keep
    /// submission order), then resolves exact ties with one rng draw per
    /// tied adjacent pair, left to right; a true draw swaps that pair.
    /// With two participants this is exactly the legacy coin flip
    /// (doc 02 v1.1 #1d); with more it is a deterministic, documented
    /// shuffle — not uniform, but stable across replays.
    fn order_by_speed(&self, slots: &mut [Slot], rng: &mut BattleRng) {
        if slots.len() < 2 {
            return;
        }
        slots.sort_by_key(|&s| std::cmp::Reverse(self.effective_speed(s)));
        for i in 0..slots.len() - 1 {
            if self.effective_speed(slots[i]) == self.effective_speed(slots[i + 1])
                && rng.chance(1, 2)
            {
                slots.swap(i, i + 1);
            }
        }
    }

    // ----- phase a: escape --------------------------------------------

    fn try_escape(&mut self, side: SideId, rng: &mut BattleRng) {
        // Running is a player verb in wild battles only (doc 02 §12).
        if !matches!(self.state.kind, BattleKind::Wild) || side != 0 {
            self.events.push(BattleEvent::MoveFailed { side });
            return;
        }
        let a = u32::from(self.state.side(side).active_mote().stats.spe);
        let b = u32::from(self.state.side(1 - side).active_mote().stats.spe);
        // F = floor(A·32 / max(1,B)) + 30·attempts; attempts counts prior
        // failures this battle (first try gets +0).
        let f = a * 32 / b.max(1) + 30 * u32::from(self.state.escape_attempts);
        let fled = u32::from(rng.next_u16() % 256) < f.min(256);
        self.events.push(BattleEvent::EscapeAttempt { side, fled });
        if fled {
            self.end(Outcome::Fled { side });
        } else {
            self.state.escape_attempts = self.state.escape_attempts.saturating_add(1);
        }
    }

    // ----- phase b: switches ------------------------------------------

    fn try_switch(&mut self, slot: Slot, to: u8) {
        let s = self.state.side(slot.side);
        let legal = usize::from(to) < s.party.len()
            && !s.is_fielded(to)
            && !s.party[usize::from(to)].is_fainted()
            && !self.pstate(slot).trapped;
        if legal {
            self.perform_switch(slot, to);
        } else {
            // Illegal switch resolves as a loud no-op, deterministically.
            self.events
                .push(BattleEvent::MoveFailed { side: slot.side });
        }
    }

    /// Entry abilities (doc 02 §10): weather callers, dissonance,
    /// stage_fright. Runs at battle start and on every switch-in.
    fn on_entry(&mut self, slot: Slot) {
        let side = slot.side;
        let ability = self.mote(slot).ability;
        if let Some(kind) = ability.called_weather() {
            // Callers never fail; replace whatever is up (v1.5 #4).
            self.state.weather = Some((kind, 5));
            self.events.push(BattleEvent::AbilityNote { side, ability });
            self.events
                .push(BattleEvent::WeatherChanged { kind: Some(kind) });
        }
        if ability == Ability::Dissonance {
            // Doc 02 §10: "foes' atk −1" — every foe position in doubles.
            let foe_side = 1 - side;
            for foe_pos in 0..self.position_count(foe_side) {
                let foe = Slot {
                    side: foe_side,
                    pos: foe_pos,
                };
                if !self.mote(foe).is_fainted() {
                    let before = self.pstate(foe).stages.get(StageStat::Atk);
                    self.pstate_mut(foe).stages.bump(StageStat::Atk, -1);
                    let after = self.pstate(foe).stages.get(StageStat::Atk);
                    if after != before {
                        self.events.push(BattleEvent::AbilityNote { side, ability });
                        self.events.push(BattleEvent::StatStageChanged {
                            target: foe_side,
                            slot: foe_pos,
                            stat: undersong_core::stats::Stat::Atk,
                            delta: -1,
                            new_stage: after,
                        });
                    }
                }
            }
        }
        if ability == Ability::StageFright && !self.mote(slot).entry_boosted {
            self.mote_mut(slot).entry_boosted = true;
            self.pstate_mut(slot).stages.bump(StageStat::Spe, 1);
            let new_stage = self.pstate(slot).stages.get(StageStat::Spe);
            self.events.push(BattleEvent::AbilityNote { side, ability });
            self.events.push(BattleEvent::StatStageChanged {
                target: side,
                slot: slot.pos,
                stat: undersong_core::stats::Stat::Spe,
                delta: 1,
                new_stage,
            });
        }
    }

    /// Oran Chime (doc 02 v1.5 #1): once per battle at ≤ 1/2 max HP.
    fn check_oran(&mut self, slot: Slot) {
        let mote = self.mote(slot);
        if mote.is_fainted() || u32::from(mote.hp) * 2 > u32::from(mote.max_hp()) {
            return;
        }
        if let HeldItem::OranChime { used: false } = mote.held {
            let mote = self.mote_mut(slot);
            mote.held = HeldItem::OranChime { used: true };
            let healed = mote.heal(20);
            if healed > 0 {
                self.events.push(BattleEvent::ItemNote {
                    side: slot.side,
                    item: HeldItem::OranChime { used: true },
                });
                self.events.push(BattleEvent::Healed {
                    target: slot.side,
                    slot: slot.pos,
                    amount: healed,
                });
            }
        }
    }

    fn perform_switch_public(&mut self, side: SideId, to: u8) {
        // Validity: bench, conscious, not already fielded.
        let side_state = self.state.side(side);
        let fielded: Vec<u8> = side_state.positions.iter().map(|p| p.party_index).collect();
        let valid = usize::from(to) < side_state.party.len()
            && !side_state.party[usize::from(to)].is_fainted()
            && !fielded.contains(&to);
        if valid {
            self.perform_switch(Slot { side, pos: 0 }, to);
        }
    }

    fn perform_switch(&mut self, slot: Slot, to: u8) {
        // Toxic's counter resets on switch-out (doc 02 v1.1 #9).
        if let Some(MajorStatus::Toxic { n }) = &mut self.mote_mut(slot).status {
            *n = 1;
        }
        let position = &mut self.state.side_mut(slot.side).positions[usize::from(slot.pos)];
        position.state = Default::default();
        position.party_index = to;
        let species = self.mote(slot).species.clone();
        self.events.push(BattleEvent::SwitchedIn {
            side: slot.side,
            slot: to,
            species,
        });
        self.on_entry(slot);
    }

    // ----- phase d: acting --------------------------------------------

    /// Resolves the chosen slot to a usable one: chosen if usable, else
    /// the first slot with PP, else `None` = Last Resort Hum
    /// (doc 02 v1.1 #3). Deterministic so fuzzed actions stay replayable.
    fn resolve_slot(&self, slot: Slot, move_slot: u8) -> Option<u8> {
        let moves = &self.mote(slot).moves;
        if let Some(battle_move) = moves.get(usize::from(move_slot))
            && battle_move.pp > 0
        {
            return Some(move_slot);
        }
        moves
            .iter()
            .position(|m| m.pp > 0)
            .map(|i| u8::try_from(i).expect("≤ 4 moves"))
    }

    /// Resolves the declared target (doc 02 v1.5 #2): the declared foe
    /// slot; if that Mote has fainted by execution, retarget to the
    /// surviving foe slot; else `None` — the move fizzles. Allies cannot
    /// be targeted at launch. Singles keeps the locked pre-doubles
    /// semantics: always foe position 0, even if it fainted mid-turn (the
    /// damage loop and effects no-op against it) — the golden corpus pins
    /// this, and the rng stream must not shift.
    fn resolve_target(&self, foe_side: SideId, declared: u8) -> Option<Slot> {
        if !matches!(self.state.format, Format::Double) {
            return Some(Slot {
                side: foe_side,
                pos: 0,
            });
        }
        let count = self.position_count(foe_side);
        // Out-of-range declarations resolve to slot 0 (deterministic
        // caller-error policy).
        let declared = if declared < count { declared } else { 0 };
        let slot = Slot {
            side: foe_side,
            pos: declared,
        };
        if !self.mote(slot).is_fainted() {
            return Some(slot);
        }
        (0..count)
            .find(|&p| {
                p != declared
                    && !self
                        .mote(Slot {
                            side: foe_side,
                            pos: p,
                        })
                        .is_fainted()
            })
            .map(|p| Slot {
                side: foe_side,
                pos: p,
            })
    }

    fn act(&mut self, user: Slot, move_slot: u8, declared_target: u8, rng: &mut BattleRng) {
        let side = user.side;
        // Volatile gate order (doc 02 v1.1 #8):
        // flinch → sleep → freeze → paralysis → confusion.
        if self.pstate(user).flinched {
            self.events.push(BattleEvent::Flinched {
                side,
                slot: user.pos,
            });
            self.pstate_mut(user).flinched = false;
            return;
        }
        match self.mote(user).status {
            Some(MajorStatus::Sleep { turns }) => {
                let turns = turns.saturating_sub(1);
                if turns == 0 {
                    self.mote_mut(user).status = None;
                    self.events.push(BattleEvent::StatusCured {
                        target: side,
                        status: Ailment::Sleep,
                    });
                } else {
                    self.mote_mut(user).status = Some(MajorStatus::Sleep { turns });
                    self.events.push(BattleEvent::ActionLost {
                        side,
                        slot: user.pos,
                        status: Ailment::Sleep,
                    });
                    return;
                }
            }
            Some(MajorStatus::Freeze) => {
                if rng.chance(1, 5) {
                    self.mote_mut(user).status = None;
                    self.events.push(BattleEvent::StatusCured {
                        target: side,
                        status: Ailment::Freeze,
                    });
                } else {
                    self.events.push(BattleEvent::ActionLost {
                        side,
                        slot: user.pos,
                        status: Ailment::Freeze,
                    });
                    return;
                }
            }
            _ => {}
        }
        if matches!(self.mote(user).status, Some(MajorStatus::Paralysis)) && rng.chance(1, 4) {
            self.events.push(BattleEvent::ActionLost {
                side,
                slot: user.pos,
                status: Ailment::Paralysis,
            });
            return;
        }
        if self.pstate(user).confusion > 0 {
            let confusion = self.pstate(user).confusion - 1;
            self.pstate_mut(user).confusion = confusion;
            if confusion == 0 {
                self.events
                    .push(BattleEvent::ConfusionEnded { target: side });
            } else if rng.chance(1, 3) {
                // Self-hit (doc 02 v1.1 #4): 40 power, own atk vs own def,
                // deterministic base damage only.
                let mote = self.mote(user);
                let level_term = 2 * u32::from(mote.level) / 5 + 2;
                let atk = u32::from(mote.stats.atk);
                let def = u32::from(mote.stats.def).max(1);
                let damage = (level_term * 40 * atk / def / 50 + 2).max(1);
                let dealt = self.mote_mut(user).take_damage(damage);
                self.events.push(BattleEvent::HurtItselfInConfusion {
                    side,
                    damage: dealt,
                });
                self.faint_check(user);
                return;
            }
        }

        // Resolve the move. A committed charge (doc 02 v1.2 #5) forces the
        // stored slot: no second PP cost, no second MoveUsed. Otherwise
        // resolve the submitted slot (or the no-PP fallback), and a
        // two-turn move's first use charges: 1 PP, MoveUsed +
        // ChargeStarted, action over.
        let committed = self.pstate(user).charging;
        let (spec, typeless) = if let Some(committed_slot) = committed {
            self.pstate_mut(user).charging = None;
            let spec = self.mote(user).moves[usize::from(committed_slot)]
                .spec
                .clone();
            (spec, false)
        } else {
            match self.resolve_slot(user, move_slot) {
                Some(s) => {
                    let battle_move = &mut self.mote_mut(user).moves[usize::from(s)];
                    battle_move.pp -= 1;
                    let spec = battle_move.spec.clone();
                    self.events.push(BattleEvent::MoveUsed {
                        side,
                        slot: user.pos,
                        move_id: spec.id.clone(),
                    });
                    if spec
                        .effects
                        .iter()
                        .any(|e| matches!(e, Effect::TwoTurn { .. }))
                    {
                        self.pstate_mut(user).charging = Some(s);
                        self.events.push(BattleEvent::ChargeStarted { side });
                        return;
                    }
                    (spec, false)
                }
                None => {
                    self.events.push(BattleEvent::LastResortUsed { side });
                    (last_resort_spec(), true)
                }
            }
        };

        // Doubles targeting (doc 02 v1.5 #2): declared slot, retarget to
        // the survivor, else fizzle. Consumes no rng.
        let foe_side: SideId = 1 - side;
        let Some(foe) = self.resolve_target(foe_side, declared_target) else {
            self.events.push(BattleEvent::MoveFailed { side });
            return;
        };

        // Protect (engine support; no canon P1 move sets it).
        if spec.flags.protectable && self.pstate(foe).protected {
            self.events.push(BattleEvent::MoveFailed { side });
            return;
        }

        // Accuracy (doc 02 §4): acc 0 never misses; stages per §3, with
        // resonate-style evasion bypass (v1.1 #18). Flurry makes frost
        // moves skip the roll entirely (doc 02 §7, v1.2 #9).
        let flurry_frost = spec.r#type == Type::Frost
            && matches!(
                self.state.weather,
                Some((undersong_core::moves::WeatherKind::Flurry, _))
            );
        if spec.accuracy > 0 && matches!(spec.target, MoveTarget::Foe) && !flurry_frost {
            let user_stage = self.pstate(user).stages.get(StageStat::Acc);
            let eva_stage =
                if spec.flags.ignore_evasion || self.mote(user).ability == Ability::PerfectPitch {
                    0
                } else {
                    self.pstate(foe).stages.get(StageStat::Eva)
                };
            let (acc_n, acc_d) = acc_stage_factor(user_stage);
            let (eva_n, eva_d) = acc_stage_factor(eva_stage);
            let threshold = (u32::from(spec.accuracy) * acc_n * eva_d / (acc_d * eva_n)).min(100);
            if rng.below(100) >= threshold {
                self.events.push(BattleEvent::MoveMissed { side });
                return;
            }
        }

        // Ability immunities (doc 02 §10): damper blanks sound moves,
        // floating blanks stone moves — turn consumed, nothing happens.
        {
            let defender_ability = self.mote(foe).ability;
            let blanked = (defender_ability == Ability::Damper && spec.flags.sound)
                || (defender_ability == Ability::Floating && spec.r#type == Type::Stone);
            if blanked && matches!(spec.target, MoveTarget::Foe) {
                self.events.push(BattleEvent::AbilityNote {
                    side: foe.side,
                    ability: defender_ability,
                });
                self.events.push(BattleEvent::DamageDealt {
                    target: foe.side,
                    target_slot: foe.pos,
                    amount: 0,
                    crit: false,
                    effectiveness: undersong_core::types::Eff::Zero,
                });
                return;
            }
        }

        // Damage.
        let doubles = matches!(self.state.format, Format::Double);
        let mut total_dealt: u32 = 0;
        let mut immune = false;
        if spec.power > 0 && !matches!(spec.category, MoveCategory::Status) {
            let multi = spec.effects.iter().any(|e| matches!(e, Effect::MultiHit));
            let planned_hits = if multi {
                // 2:3/8, 3:3/8, 4:1/8, 5:1/8 (doc 02 §6).
                match rng.below(8) {
                    0..=2 => 2,
                    3..=5 => 3,
                    6 => 4,
                    _ => 5,
                }
            } else {
                1
            };
            let mut landed: u8 = 0;
            for _ in 0..planned_hits {
                if self.mote(foe).is_fainted() {
                    break;
                }
                let crit_stage = u8::from(spec.flags.high_crit);
                let (crit_n, crit_d) = crit_chance(crit_stage);
                let crit_roll = rng.chance(crit_n, crit_d);
                // thick_hide: the roll still consumes rng (stream-stable)
                // but can never land (doc 02 §10).
                let crit = !typeless && crit_roll && self.mote(foe).ability != Ability::ThickHide;
                let rand_roll =
                    u8::try_from(rng.range_inclusive(85, 100)).expect("85..=100 fits u8");

                let (amount, effectiveness, product_zero) = if typeless {
                    // Last Resort Hum / typeless: product 1, no STAB, no
                    // crit, base pipeline with the rand roll only.
                    let attacker = self.mote(user);
                    let level_term = 2 * u32::from(attacker.level) / 5 + 2;
                    let a = stage_multiplied(
                        u32::from(attacker.stats.atk),
                        self.pstate(user).stages.get(StageStat::Atk),
                    );
                    let d = stage_multiplied(
                        u32::from(self.mote(foe).stats.def),
                        self.pstate(foe).stages.get(StageStat::Def),
                    )
                    .max(1);
                    let mut damage = level_term * u32::from(spec.power) * a / d / 50 + 2;
                    damage = damage * u32::from(rand_roll) / 100;
                    if matches!(self.mote(user).status, Some(MajorStatus::Burn)) {
                        damage /= 2;
                    }
                    (damage.max(1), undersong_core::types::Eff::Neutral, false)
                } else {
                    let context = DamageContext {
                        attacker: self.mote(user),
                        defender: self.mote(foe),
                        attacker_stages: &self.pstate(user).stages,
                        defender_stages: &self.pstate(foe).stages,
                        chart: &self.state.chart,
                        weather: self.state.weather.map(|(kind, _)| kind),
                        crit,
                        rand: rand_roll,
                        spread: false,
                        doubles,
                    };
                    let outcome = compute_damage(&spec, &context).expect("damaging move");
                    (
                        outcome.amount,
                        outcome.effectiveness,
                        outcome.type_product.0 == 0,
                    )
                };

                if product_zero {
                    immune = true;
                    self.events.push(BattleEvent::DamageDealt {
                        target: foe.side,
                        target_slot: foe.pos,
                        amount: 0,
                        crit: false,
                        effectiveness,
                    });
                    break;
                }

                let dealt = self.mote_mut(foe).take_damage(amount);
                total_dealt += u32::from(dealt);
                landed += 1;
                self.events.push(BattleEvent::DamageDealt {
                    target: foe.side,
                    target_slot: foe.pos,
                    amount: dealt,
                    crit,
                    effectiveness,
                });

                // Ember moves thaw a frozen target (doc 02 §5).
                if spec.r#type == Type::Ember
                    && matches!(self.mote(foe).status, Some(MajorStatus::Freeze))
                {
                    self.mote_mut(foe).status = None;
                    self.events.push(BattleEvent::StatusCured {
                        target: foe.side,
                        status: Ailment::Freeze,
                    });
                }
            }
            if multi && landed > 1 {
                self.events.push(BattleEvent::MultiHit { hits: landed });
            }

            // Contact aftermath (doc 02 §10): live_wire / thorn_coat
            // punish contacters; oran chime may trigger on the target.
            if total_dealt > 0 {
                self.check_oran(foe);
            }
            if spec.flags.contact && total_dealt > 0 && !self.mote(user).is_fainted() {
                let defender_ability = self.mote(foe).ability;
                if defender_ability == Ability::LiveWire
                    && rng.chance(3, 10)
                    && self.try_apply_status(user, Ailment::Paralysis, rng)
                {
                    self.events.push(BattleEvent::AbilityNote {
                        side: foe.side,
                        ability: defender_ability,
                    });
                }
                if defender_ability == Ability::ThornCoat {
                    let recoil = u32::from(self.mote(user).max_hp()) / 8;
                    if recoil > 0 {
                        let dealt = self.mote_mut(user).take_damage(recoil);
                        self.events.push(BattleEvent::AbilityNote {
                            side: foe.side,
                            ability: defender_ability,
                        });
                        self.events.push(BattleEvent::Recoiled {
                            side,
                            slot: user.pos,
                            amount: dealt,
                        });
                        self.check_oran(user);
                    }
                }
            }
        }

        // Effects in list order (doc 02 §6), skipped entirely on immunity.
        if !immune {
            for effect in spec.effects.clone() {
                self.apply_effect(&spec, &effect, user, foe, total_dealt, rng);
            }
        }

        // Last Resort recoil (v1.1 #3): floor(damage/4).
        if typeless && total_dealt > 0 {
            let recoil = total_dealt / 4;
            if recoil > 0 {
                let dealt = self.mote_mut(user).take_damage(recoil);
                self.events.push(BattleEvent::Recoiled {
                    side,
                    slot: user.pos,
                    amount: dealt,
                });
            }
        }

        self.faint_check(foe);
        self.faint_check(user);
        self.check_outcome();
    }

    fn apply_effect(
        &mut self,
        spec: &MoveSpec,
        effect: &Effect,
        user: Slot,
        foe: Slot,
        total_dealt: u32,
        rng: &mut BattleRng,
    ) {
        let side = user.side;
        match effect {
            Effect::StatStage {
                target,
                stat,
                delta,
                chance,
            } => {
                if !rng.chance(u32::from(*chance), 100) {
                    return;
                }
                let target_slot = match target {
                    EffectTarget::User => user,
                    EffectTarget::Target => foe,
                };
                if self.mote(target_slot).is_fainted() {
                    return;
                }
                // Drop guards (doc 02 §10): metronome_soul pins speed.
                // (perfect_pitch's accuracy pin is structural: the
                // StatStage effect carries core::Stat, which has no Acc
                // variant — no launch move can lower accuracy.)
                if *delta < 0 {
                    let guard = self.mote(target_slot).ability;
                    let blocked = guard == Ability::MetronomeSoul
                        && *stat == undersong_core::stats::Stat::Spe;
                    if blocked {
                        self.events.push(BattleEvent::AbilityNote {
                            side: target_slot.side,
                            ability: guard,
                        });
                        return;
                    }
                }
                let Some(stage_stat) = StageStat::from_stat(*stat) else {
                    return; // HP has no stage
                };
                let (new_stage, clamped) =
                    self.pstate_mut(target_slot).stages.bump(stage_stat, *delta);
                self.events.push(if clamped {
                    BattleEvent::StatStageClamped {
                        target: target_slot.side,
                        stat: *stat,
                    }
                } else {
                    BattleEvent::StatStageChanged {
                        target: target_slot.side,
                        slot: target_slot.pos,
                        stat: *stat,
                        delta: *delta,
                        new_stage,
                    }
                });
            }
            Effect::Status { ailment, chance } => {
                if !rng.chance(u32::from(*chance), 100) {
                    return;
                }
                let applied = self.try_apply_status(foe, *ailment, rng);
                // A pure status move that does nothing tells the player so.
                if !applied && matches!(spec.category, MoveCategory::Status) {
                    self.events.push(BattleEvent::MoveFailed { side });
                }
            }
            Effect::Heal { frac } => {
                let max_hp = u32::from(self.mote(user).max_hp());
                let healed = self.mote_mut(user).heal(frac.apply(max_hp));
                if healed > 0 {
                    self.events.push(BattleEvent::Healed {
                        target: side,
                        slot: user.pos,
                        amount: healed,
                    });
                }
            }
            Effect::Drain { frac } => {
                let amount = frac.apply(total_dealt);
                if amount > 0 {
                    let healed = self.mote_mut(user).heal(amount);
                    if healed > 0 {
                        self.events.push(BattleEvent::Drained {
                            from: foe.side,
                            amount: healed,
                        });
                    }
                }
            }
            Effect::Recoil { frac } => {
                let amount = frac.apply(total_dealt);
                if amount > 0 {
                    let dealt = self.mote_mut(user).take_damage(amount);
                    self.events.push(BattleEvent::Recoiled {
                        side,
                        slot: user.pos,
                        amount: dealt,
                    });
                }
            }
            Effect::Flinch { chance } => {
                // keysmith: +10% flinch on sound moves (doc 02 §10).
                let mut chance = u32::from(*chance);
                if self.mote(user).ability == Ability::Keysmith && spec.flags.sound {
                    chance += 10;
                }
                if rng.chance(chance, 100)
                    && !self.mote(foe).is_fainted()
                    && self.mote(foe).ability != Ability::IronEar
                {
                    self.pstate_mut(foe).flinched = true;
                }
            }
            Effect::Weather { kind } => {
                self.state.weather = Some((*kind, 5));
                self.events
                    .push(BattleEvent::WeatherChanged { kind: Some(*kind) });
            }
            Effect::Protect => {
                self.pstate_mut(user).protected = true;
            }
            Effect::ForceSwitch => {
                // A KO from this same move wins over the drag: the faint
                // must reach the event stream before any switch could
                // hide it (doc 03 §2; v1.2 #1/#3 award rules).
                self.faint_check(foe);
                if self.mote(foe).is_fainted() {
                    return;
                }
                let bench: Vec<u8> = {
                    let s = self.state.side(foe.side);
                    s.party
                        .iter()
                        .enumerate()
                        .map(|(i, m)| (u8::try_from(i).expect("party ≤ 6"), m))
                        .filter(|(i, m)| !s.is_fielded(*i) && !m.is_fainted())
                        .map(|(i, _)| i)
                        .collect()
                };
                if !bench.is_empty() {
                    let pick =
                        bench[usize::try_from(rng.below(u32::try_from(bench.len()).expect("≤ 6")))
                            .expect("index")];
                    self.perform_switch(foe, pick);
                    // v1.2 #10: the dragged-in Mote does not act with its
                    // predecessor's queued action.
                    self.cancel(foe);
                }
            }
            Effect::SelfSwitch => {
                // Same faint-before-switch rule as ForceSwitch.
                self.faint_check(user);
                if self.mote(user).is_fainted() {
                    return;
                }
                if let Some(replacement) = self.state.side(user.side).first_replacement() {
                    self.perform_switch(user, replacement);
                }
            }
            Effect::Ohko => {
                let hp = u32::from(self.mote(foe).hp);
                if hp > 0 {
                    let dealt = self.mote_mut(foe).take_damage(hp);
                    self.events.push(BattleEvent::DamageDealt {
                        target: foe.side,
                        target_slot: foe.pos,
                        amount: dealt,
                        crit: false,
                        effectiveness: undersong_core::types::Eff::Neutral,
                    });
                }
            }
            Effect::FixedDamage { amount } => {
                let raw = match amount {
                    FixedAmount::Amount(n) => u32::from(*n),
                    FixedAmount::UserLevel => u32::from(self.mote(user).level),
                };
                let dealt = self.mote_mut(foe).take_damage(raw);
                self.events.push(BattleEvent::DamageDealt {
                    target: foe.side,
                    target_slot: foe.pos,
                    amount: dealt,
                    crit: false,
                    effectiveness: undersong_core::types::Eff::Neutral,
                });
            }
            // Handled in the act() flow, not as post-damage effects.
            Effect::MultiHit | Effect::TwoTurn { .. } => {}
        }
    }

    /// Applies a major status respecting one-at-a-time and the type
    /// immunities of doc 02 §5. Returns whether it stuck.
    fn try_apply_status(&mut self, target: Slot, ailment: Ailment, rng: &mut BattleRng) -> bool {
        // vigor: immune to sleep (doc 02 §10).
        if ailment == Ailment::Sleep && self.mote(target).ability == Ability::Vigor {
            return false;
        }
        let mote = self.mote(target);
        if mote.is_fainted() || mote.status.is_some() {
            return false;
        }
        let immune = match ailment {
            Ailment::Burn => mote.types.contains(&Type::Ember),
            Ailment::Poison | Ailment::Toxic => {
                mote.types.contains(&Type::Venom) || mote.types.contains(&Type::Alloy)
            }
            Ailment::Paralysis => mote.types.contains(&Type::Volt),
            Ailment::Freeze => mote.types.contains(&Type::Frost),
            Ailment::Sleep => false,
        };
        if immune {
            return false;
        }
        let status = match ailment {
            Ailment::Burn => MajorStatus::Burn,
            Ailment::Poison => MajorStatus::Poison,
            Ailment::Toxic => MajorStatus::Toxic { n: 1 },
            Ailment::Paralysis => MajorStatus::Paralysis,
            Ailment::Sleep => MajorStatus::Sleep {
                turns: u8::try_from(rng.range_inclusive(1, 3)).expect("1..=3"),
            },
            Ailment::Freeze => MajorStatus::Freeze,
        };
        self.mote_mut(target).status = Some(status);
        self.events.push(BattleEvent::StatusApplied {
            target: target.side,
            slot: target.pos,
            status: ailment,
        });
        true
    }

    // ----- end of turn ---------------------------------------------------

    fn end_of_turn(&mut self, _rng: &mut BattleRng) {
        use undersong_core::moves::WeatherKind;

        // 1) Weather chip (v1.1 #2.1), side 0's positions first. All EOT
        // fraction damage has a 1 HP minimum (v1.2 #7).
        if let Some((kind, _)) = self.state.weather {
            for slot in self.all_slots() {
                let mote = self.mote(slot);
                if mote.is_fainted() {
                    continue;
                }
                let exempt = match kind {
                    WeatherKind::Flurry => mote
                        .types
                        .iter()
                        .any(|t| matches!(t, Type::Frost | Type::Alloy | Type::Stone)),
                    WeatherKind::Dustchord => mote
                        .types
                        .iter()
                        .any(|t| matches!(t, Type::Stone | Type::Alloy)),
                    _ => true,
                };
                if !exempt {
                    let chip = u32::from(mote.max_hp()) / 16;
                    let dealt = self.mote_mut(slot).take_damage(chip.max(1));
                    self.events.push(BattleEvent::WeatherChip {
                        target: slot.side,
                        amount: dealt,
                    });
                    self.faint_check(slot);
                }
            }
        }

        // 2) Seeded drain (doc 02 §5: 1/8 to the opposer). In doubles the
        // drain heals the foe's first conscious position (engine policy:
        // the seeded volatile does not track its planter).
        for slot in self.all_slots() {
            if !self.pstate(slot).seeded {
                continue;
            }
            let mote = self.mote(slot);
            if mote.is_fainted() {
                continue;
            }
            let amount = (u32::from(mote.max_hp()) / 8).max(1);
            let dealt = self.mote_mut(slot).take_damage(amount);
            self.events.push(BattleEvent::SeededDrain {
                from: slot.side,
                amount: dealt,
            });
            let foe_side = 1 - slot.side;
            let drinker = (0..self.position_count(foe_side))
                .map(|p| Slot {
                    side: foe_side,
                    pos: p,
                })
                .find(|&s| !self.mote(s).is_fainted());
            if let Some(drinker) = drinker {
                self.mote_mut(drinker).heal(u32::from(dealt));
            }
            self.faint_check(slot);
        }

        // 3) Burn / poison / toxic (doc 02 §5; order v1.1 #2.3).
        for slot in self.all_slots() {
            let mote = self.mote(slot);
            if mote.is_fainted() {
                continue;
            }
            let max_hp = u32::from(mote.max_hp());
            let (ailment, amount, next): (Ailment, u32, Option<MajorStatus>) = match mote.status {
                Some(MajorStatus::Burn) => (Ailment::Burn, max_hp / 16, Some(MajorStatus::Burn)),
                Some(MajorStatus::Poison) => {
                    (Ailment::Poison, max_hp / 8, Some(MajorStatus::Poison))
                }
                Some(MajorStatus::Toxic { n }) => (
                    Ailment::Toxic,
                    max_hp * u32::from(n) / 16,
                    Some(MajorStatus::Toxic {
                        n: n.saturating_add(1),
                    }),
                ),
                _ => continue,
            };
            let dealt = self.mote_mut(slot).take_damage(amount.max(1));
            self.mote_mut(slot).status = next;
            self.events.push(BattleEvent::StatusTicked {
                target: slot.side,
                status: ailment,
                damage: dealt,
            });
            self.faint_check(slot);
        }

        // 4) Weather countdown.
        // encore_heart: 1/16 max HP each turn in any weather (doc 02 §10).
        if self.state.weather.is_some() {
            for slot in self.all_slots() {
                let mote = self.mote(slot);
                if !mote.is_fainted() && mote.ability == Ability::EncoreHeart {
                    let amount = u32::from(mote.max_hp()) / 16;
                    let healed = self.mote_mut(slot).heal(amount);
                    if healed > 0 {
                        self.events.push(BattleEvent::AbilityNote {
                            side: slot.side,
                            ability: Ability::EncoreHeart,
                        });
                        self.events.push(BattleEvent::Healed {
                            target: slot.side,
                            slot: slot.pos,
                            amount: healed,
                        });
                    }
                }
            }
        }
        if let Some((kind, turns)) = self.state.weather {
            let turns = turns.saturating_sub(1);
            if turns == 0 {
                self.state.weather = None;
                self.events.push(BattleEvent::WeatherChanged { kind: None });
            } else {
                self.state.weather = Some((kind, turns));
            }
        }
    }

    // ----- faints, exp, outcome ----------------------------------------

    /// Emits Fainted (once per faint, tracked by the position's
    /// `fainted_emitted` flag — reset on switch-in), fires understudy on
    /// the surviving ally (doc 02 §10), and awards exp/EVs to the
    /// opposing fielded Motes (doc 02 §9, v1.1 #13–14; doubles split
    /// evenly among conscious player positions, floor).
    fn faint_check(&mut self, slot: Slot) {
        if !self.mote(slot).is_fainted() || self.pstate(slot).fainted_emitted {
            return;
        }
        self.pstate_mut(slot).fainted_emitted = true;
        self.events.push(BattleEvent::Fainted {
            target: slot.side,
            slot: slot.pos,
        });

        // understudy (doc 02 §10): +1 atk/+1 spa to the surviving ally,
        // per ally faint.
        for ally_pos in 0..self.position_count(slot.side) {
            if ally_pos == slot.pos {
                continue;
            }
            let ally = Slot {
                side: slot.side,
                pos: ally_pos,
            };
            if self.mote(ally).is_fainted() || self.mote(ally).ability != Ability::Understudy {
                continue;
            }
            self.events.push(BattleEvent::AbilityNote {
                side: slot.side,
                ability: Ability::Understudy,
            });
            for stat in [Stat::Atk, Stat::Spa] {
                let stage_stat = StageStat::from_stat(stat).expect("atk/spa have stages");
                let (new_stage, clamped) = self.pstate_mut(ally).stages.bump(stage_stat, 1);
                self.events.push(if clamped {
                    BattleEvent::StatStageClamped {
                        target: slot.side,
                        stat,
                    }
                } else {
                    BattleEvent::StatStageChanged {
                        target: slot.side,
                        slot: ally_pos,
                        stat,
                        delta: 1,
                        new_stage,
                    }
                });
            }
        }

        let victor: SideId = 1 - slot.side;
        // Exp/EV awards are player-side only (doc 02 v1.2 #1); the Fainted
        // event above is unconditional.
        if victor != 0 {
            return;
        }
        let (yield_base, level, ev_yield) = {
            let fainted = self.mote(slot);
            (
                fainted.base_exp_yield,
                fainted.level,
                fainted.ev_yield.clone(),
            )
        };
        let trainer = matches!(self.state.kind, BattleKind::Trainer);
        // Participants (doc 02 §9): the player positions on the field and
        // conscious when the foe fainted; the gain splits evenly (floor).
        let recipients: Vec<Slot> = (0..self.position_count(victor))
            .map(|p| Slot {
                side: victor,
                pos: p,
            })
            .filter(|&s| !self.mote(s).is_fainted())
            .collect();
        let participants = u32::try_from(recipients.len()).expect("≤ 2");
        let gained = exp_gain(yield_base, level, participants.max(1), trainer, false);
        for recipient in recipients {
            let victor_slot =
                self.state.side(victor).positions[usize::from(recipient.pos)].party_index;
            let victor_mote = self.mote_mut(recipient);

            // EVs first (v1.1 #14): per-stat cap 252, total cap 510,
            // excess dropped in canonical stat order. Every participant
            // receives the full yield (doc 02 §9 split covers exp only).
            for (stat, amount) in &ev_yield {
                let total = victor_mote.ev_sum();
                if total >= 510 {
                    break;
                }
                let room_total = 510 - total;
                let current = victor_mote.evs.get(*stat);
                let room_stat = u32::from(252u16.saturating_sub(current));
                let grant = u32::from(*amount).min(room_total).min(room_stat);
                let new = current + u16::try_from(grant).expect("≤ 252");
                match stat {
                    Stat::Hp => victor_mote.evs.hp = new,
                    Stat::Atk => victor_mote.evs.atk = new,
                    Stat::Def => victor_mote.evs.def = new,
                    Stat::Spa => victor_mote.evs.spa = new,
                    Stat::Spd => victor_mote.evs.spd = new,
                    Stat::Spe => victor_mote.evs.spe = new,
                }
            }
            // EVs take effect at the next level-up recompute (v1.2 #4); no
            // mid-battle stat bump from the award itself.

            if gained > 0 {
                self.events.push(BattleEvent::ExpGained {
                    side: victor,
                    slot: victor_slot,
                    amount: gained,
                });
                let ups = apply_exp(self.mote_mut(recipient), gained);
                for up in ups {
                    self.events.push(BattleEvent::LeveledUp {
                        side: victor,
                        slot: victor_slot,
                        level: up.new_level,
                    });
                    for move_id in up.learnable {
                        self.events.push(BattleEvent::MoveLearnable {
                            side: victor,
                            slot: victor_slot,
                            move_id,
                        });
                    }
                }
            }
        }
    }

    fn check_outcome(&mut self) {
        if self.state.is_over() {
            return;
        }
        let side0 = self.state.side(0).has_conscious();
        let side1 = self.state.side(1).has_conscious();
        match (side0, side1) {
            (true, true) => {}
            (true, false) => self.end(Outcome::Won { winner: 0 }),
            (false, true) => self.end(Outcome::Won { winner: 1 }),
            (false, false) => self.end(Outcome::Drawn),
        }
    }

    fn end(&mut self, outcome: Outcome) {
        self.state.outcome = Some(outcome);
        self.events.push(BattleEvent::BattleEnded { outcome });
    }
}
