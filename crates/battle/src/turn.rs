//! `step()` — one full battle turn (doc 02 v1.1 #1 phase structure).
//!
//! Pure: `(state, actions, rng) → (state', events)`. All policy decisions
//! cite doc 02; engine-level resolutions of caller errors (illegal slot,
//! illegal switch) are deterministic and documented inline.

use undersong_core::moves::{
    Ailment, Effect, EffectTarget, FixedAmount, MoveCategory, MoveFlags, MoveSpec, MoveTarget,
};
use undersong_core::rng::BattleRng;
use undersong_core::stats::Stat;
use undersong_core::types::Type;

use crate::actions::{Action, TurnActions};
use crate::catch::attune;
use crate::damage::{DamageContext, compute_damage, crit_chance};
use crate::events::{BattleEvent, Outcome, SideId};
use crate::exp::{apply_exp, exp_gain};
use crate::mote::MajorStatus;
use crate::state::{BattleKind, BattleState};
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

pub fn step(
    state: &BattleState,
    actions: &TurnActions,
    rng: &mut BattleRng,
) -> (BattleState, Vec<BattleEvent>) {
    let mut engine = Engine {
        state: state.clone(),
        events: Vec::new(),
        cancelled: [false; 2],
    };
    if engine.state.is_over() {
        return (engine.state, engine.events);
    }
    engine.run_turn(actions, rng);
    (engine.state, engine.events)
}

struct Engine {
    state: BattleState,
    events: Vec<BattleEvent>,
    /// Set when a side's pending action is consumed mid-turn
    /// (ForceSwitch drag, doc 02 v1.2 #10).
    cancelled: [bool; 2],
}

impl Engine {
    fn run_turn(&mut self, actions: &TurnActions, rng: &mut BattleRng) {
        self.state.turn += 1;
        self.events
            .push(BattleEvent::TurnStarted { n: self.state.turn });
        if self.state.turn > TURN_LIMIT {
            self.end(Outcome::Drawn);
            return;
        }

        // Stale per-turn volatiles from any previous turn.
        for side in 0..2u8 {
            let st = &mut self.state.side_mut(side).active_state;
            st.flinched = false;
            st.protected = false;
        }

        // Phase a — escape attempts (doc 02 v1.1 #1a; formula §12).
        for side in 0..2u8 {
            if matches!(actions.get(side), Action::Run) {
                self.try_escape(side, rng);
                if self.state.is_over() {
                    return;
                }
            }
        }

        // Phase b — switches, faster side first (v1.1 #1b).
        let mut switchers: Vec<SideId> = (0..2u8)
            .filter(|&s| matches!(actions.get(s), Action::Switch { .. }))
            .collect();
        if switchers.len() == 2 {
            self.order_by_speed(&mut switchers, rng);
        }
        for side in switchers {
            if let Action::Switch { to } = actions.get(side) {
                self.try_switch(side, to);
            }
        }

        // Phase c — bell use (v1.1 #1c; doc 02 §8). Player side only:
        // attunement is a trainer verb, the wild side has no bells.
        if let Action::UseBell { bell_mod } = actions.get(0) {
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
        if matches!(actions.get(1), Action::UseBell { .. }) {
            self.events.push(BattleEvent::MoveFailed { side: 1 });
        }

        // Phase d — moves (v1.1 #1d): priority desc → speed desc → rng.
        let mut movers: Vec<(SideId, u8, i8)> = (0..2u8)
            .filter_map(|side| match actions.get(side) {
                Action::Move { slot } => {
                    let slot = self.resolve_slot(side, slot);
                    let priority = match slot {
                        Some(s) => {
                            self.state.side(side).active_mote().moves[usize::from(s)]
                                .spec
                                .priority
                        }
                        None => 0, // Last Resort Hum
                    };
                    Some((side, slot.unwrap_or(u8::MAX), priority))
                }
                _ => None,
            })
            .collect();
        movers.sort_by_key(|&(_, _, priority)| std::cmp::Reverse(priority));
        if movers.len() == 2 && movers[0].2 == movers[1].2 {
            let mut order: Vec<SideId> = movers.iter().map(|&(s, ..)| s).collect();
            self.order_by_speed(&mut order, rng);
            if order[0] != movers[0].0 {
                movers.swap(0, 1);
            }
        }
        for (side, slot, _) in movers {
            if self.state.is_over() {
                return;
            }
            if self.state.side(side).active_mote().is_fainted() || self.cancelled[usize::from(side)]
            {
                continue;
            }
            self.act(side, slot, rng);
        }
        if self.state.is_over() {
            return;
        }

        // Phase e — end of turn (v1.1 #2).
        self.end_of_turn(rng);
        if self.state.is_over() {
            return;
        }

        // Auto-replace fainted actives (engine policy for the headless
        // sim; the game layer will route a player choice through Switch
        // actions when the presenter exists).
        for side in 0..2u8 {
            if self.state.side(side).active_mote().is_fainted()
                && let Some(replacement) = self.state.side(side).first_replacement()
            {
                self.perform_switch(side, replacement);
            }
        }

        self.check_outcome();
    }

    // ----- ordering helpers -------------------------------------------

    /// Effective speed (doc 02 v1.1 #7): stage-modified, then paralysis
    /// quarters it.
    fn effective_speed(&self, side: SideId) -> u32 {
        let s = self.state.side(side);
        let mote = s.active_mote();
        let mut spe = stage_multiplied(
            u32::from(mote.stats.spe),
            s.active_state.stages.get(StageStat::Spe),
        );
        if matches!(mote.status, Some(MajorStatus::Paralysis)) {
            spe /= 4;
        }
        spe
    }

    /// Sorts side ids by effective speed desc; exact ties get one rng
    /// coin flip (doc 02 v1.1 #1d).
    fn order_by_speed(&mut self, sides: &mut [SideId], rng: &mut BattleRng) {
        if sides.len() < 2 {
            return;
        }
        let speed0 = self.effective_speed(sides[0]);
        let speed1 = self.effective_speed(sides[1]);
        let swap = match speed0.cmp(&speed1) {
            std::cmp::Ordering::Less => true,
            std::cmp::Ordering::Greater => false,
            std::cmp::Ordering::Equal => rng.chance(1, 2),
        };
        if swap {
            sides.swap(0, 1);
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

    fn try_switch(&mut self, side: SideId, to: u8) {
        let s = self.state.side(side);
        let legal = usize::from(to) < s.party.len()
            && to != s.active
            && !s.party[usize::from(to)].is_fainted()
            && !s.active_state.trapped;
        if legal {
            self.perform_switch(side, to);
        } else {
            // Illegal switch resolves as a loud no-op, deterministically.
            self.events.push(BattleEvent::MoveFailed { side });
        }
    }

    fn perform_switch(&mut self, side: SideId, to: u8) {
        let s = self.state.side_mut(side);
        // Toxic's counter resets on switch-out (doc 02 v1.1 #9).
        if let Some(MajorStatus::Toxic { n }) = &mut s.active_mote_mut().status {
            *n = 1;
        }
        s.active_state = Default::default();
        s.active = to;
        let species = s.active_mote().species.clone();
        self.events.push(BattleEvent::SwitchedIn {
            side,
            slot: to,
            species,
        });
    }

    // ----- phase d: acting --------------------------------------------

    /// Resolves the chosen slot to a usable one: chosen if usable, else
    /// the first slot with PP, else `None` = Last Resort Hum
    /// (doc 02 v1.1 #3). Deterministic so fuzzed actions stay replayable.
    fn resolve_slot(&self, side: SideId, slot: u8) -> Option<u8> {
        let moves = &self.state.side(side).active_mote().moves;
        if let Some(battle_move) = moves.get(usize::from(slot))
            && battle_move.pp > 0
        {
            return Some(slot);
        }
        moves
            .iter()
            .position(|m| m.pp > 0)
            .map(|i| u8::try_from(i).expect("≤ 4 moves"))
    }

    fn act(&mut self, side: SideId, slot: u8, rng: &mut BattleRng) {
        // Volatile gate order (doc 02 v1.1 #8):
        // flinch → sleep → freeze → paralysis → confusion.
        if self.state.side(side).active_state.flinched {
            self.events.push(BattleEvent::Flinched { side });
            self.state.side_mut(side).active_state.flinched = false;
            return;
        }
        match self.state.side(side).active_mote().status {
            Some(MajorStatus::Sleep { turns }) => {
                let turns = turns.saturating_sub(1);
                if turns == 0 {
                    self.state.side_mut(side).active_mote_mut().status = None;
                    self.events.push(BattleEvent::StatusCured {
                        target: side,
                        status: Ailment::Sleep,
                    });
                } else {
                    self.state.side_mut(side).active_mote_mut().status =
                        Some(MajorStatus::Sleep { turns });
                    self.events.push(BattleEvent::ActionLost {
                        side,
                        status: Ailment::Sleep,
                    });
                    return;
                }
            }
            Some(MajorStatus::Freeze) => {
                if rng.chance(1, 5) {
                    self.state.side_mut(side).active_mote_mut().status = None;
                    self.events.push(BattleEvent::StatusCured {
                        target: side,
                        status: Ailment::Freeze,
                    });
                } else {
                    self.events.push(BattleEvent::ActionLost {
                        side,
                        status: Ailment::Freeze,
                    });
                    return;
                }
            }
            _ => {}
        }
        if matches!(
            self.state.side(side).active_mote().status,
            Some(MajorStatus::Paralysis)
        ) && rng.chance(1, 4)
        {
            self.events.push(BattleEvent::ActionLost {
                side,
                status: Ailment::Paralysis,
            });
            return;
        }
        if self.state.side(side).active_state.confusion > 0 {
            let confusion = self.state.side(side).active_state.confusion - 1;
            self.state.side_mut(side).active_state.confusion = confusion;
            if confusion == 0 {
                self.events
                    .push(BattleEvent::ConfusionEnded { target: side });
            } else if rng.chance(1, 3) {
                // Self-hit (doc 02 v1.1 #4): 40 power, own atk vs own def,
                // deterministic base damage only.
                let mote = self.state.side(side).active_mote();
                let level_term = 2 * u32::from(mote.level) / 5 + 2;
                let atk = u32::from(mote.stats.atk);
                let def = u32::from(mote.stats.def).max(1);
                let damage = (level_term * 40 * atk / def / 50 + 2).max(1);
                let dealt = self
                    .state
                    .side_mut(side)
                    .active_mote_mut()
                    .take_damage(damage);
                self.events.push(BattleEvent::HurtItselfInConfusion {
                    side,
                    damage: dealt,
                });
                self.faint_check(side);
                return;
            }
        }

        // Resolve the move. A committed charge (doc 02 v1.2 #5) forces the
        // stored slot: no second PP cost, no second MoveUsed. Otherwise
        // resolve the submitted slot (or the no-PP fallback), and a
        // two-turn move's first use charges: 1 PP, MoveUsed +
        // ChargeStarted, action over.
        let committed = self.state.side(side).active_state.charging;
        let (spec, typeless) = if let Some(committed_slot) = committed {
            self.state.side_mut(side).active_state.charging = None;
            let spec = self.state.side(side).active_mote().moves[usize::from(committed_slot)]
                .spec
                .clone();
            (spec, false)
        } else {
            match self.resolve_slot(side, slot) {
                Some(s) => {
                    let battle_move =
                        &mut self.state.side_mut(side).active_mote_mut().moves[usize::from(s)];
                    battle_move.pp -= 1;
                    let spec = battle_move.spec.clone();
                    self.events.push(BattleEvent::MoveUsed {
                        side,
                        move_id: spec.id.clone(),
                    });
                    if spec
                        .effects
                        .iter()
                        .any(|e| matches!(e, Effect::TwoTurn { .. }))
                    {
                        self.state.side_mut(side).active_state.charging = Some(s);
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

        let foe: SideId = 1 - side;

        // Protect (engine support; no canon P1 move sets it).
        if spec.flags.protectable && self.state.side(foe).active_state.protected {
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
            let user_stage = self
                .state
                .side(side)
                .active_state
                .stages
                .get(StageStat::Acc);
            let eva_stage = if spec.flags.ignore_evasion {
                0
            } else {
                self.state.side(foe).active_state.stages.get(StageStat::Eva)
            };
            let (acc_n, acc_d) = acc_stage_factor(user_stage);
            let (eva_n, eva_d) = acc_stage_factor(eva_stage);
            let threshold = (u32::from(spec.accuracy) * acc_n * eva_d / (acc_d * eva_n)).min(100);
            if rng.below(100) >= threshold {
                self.events.push(BattleEvent::MoveMissed { side });
                return;
            }
        }

        // Damage.
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
                if self.state.side(foe).active_mote().is_fainted() {
                    break;
                }
                let crit_stage = u8::from(spec.flags.high_crit);
                let (crit_n, crit_d) = crit_chance(crit_stage);
                let crit = !typeless && rng.chance(crit_n, crit_d);
                let rand_roll =
                    u8::try_from(rng.range_inclusive(85, 100)).expect("85..=100 fits u8");

                let (amount, effectiveness, product_zero) = if typeless {
                    // Last Resort Hum / typeless: product 1, no STAB, no
                    // crit, base pipeline with the rand roll only.
                    let attacker = self.state.side(side).active_mote();
                    let defender_stats = self.state.side(foe);
                    let level_term = 2 * u32::from(attacker.level) / 5 + 2;
                    let a = stage_multiplied(
                        u32::from(attacker.stats.atk),
                        self.state
                            .side(side)
                            .active_state
                            .stages
                            .get(StageStat::Atk),
                    );
                    let d = stage_multiplied(
                        u32::from(defender_stats.active_mote().stats.def),
                        defender_stats.active_state.stages.get(StageStat::Def),
                    )
                    .max(1);
                    let mut damage = level_term * u32::from(spec.power) * a / d / 50 + 2;
                    damage = damage * u32::from(rand_roll) / 100;
                    if matches!(
                        self.state.side(side).active_mote().status,
                        Some(MajorStatus::Burn)
                    ) {
                        damage /= 2;
                    }
                    (damage.max(1), undersong_core::types::Eff::Neutral, false)
                } else {
                    let attacker_side = self.state.side(side);
                    let defender_side = self.state.side(foe);
                    let context = DamageContext {
                        attacker: attacker_side.active_mote(),
                        defender: defender_side.active_mote(),
                        attacker_stages: &attacker_side.active_state.stages,
                        defender_stages: &defender_side.active_state.stages,
                        chart: &self.state.chart,
                        weather: self.state.weather.map(|(kind, _)| kind),
                        crit,
                        rand: rand_roll,
                        spread: false,
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
                        target: foe,
                        amount: 0,
                        crit: false,
                        effectiveness,
                    });
                    break;
                }

                let dealt = self
                    .state
                    .side_mut(foe)
                    .active_mote_mut()
                    .take_damage(amount);
                total_dealt += u32::from(dealt);
                landed += 1;
                self.events.push(BattleEvent::DamageDealt {
                    target: foe,
                    amount: dealt,
                    crit,
                    effectiveness,
                });

                // Ember moves thaw a frozen target (doc 02 §5).
                if spec.r#type == Type::Ember
                    && matches!(
                        self.state.side(foe).active_mote().status,
                        Some(MajorStatus::Freeze)
                    )
                {
                    self.state.side_mut(foe).active_mote_mut().status = None;
                    self.events.push(BattleEvent::StatusCured {
                        target: foe,
                        status: Ailment::Freeze,
                    });
                }
            }
            if multi && landed > 1 {
                self.events.push(BattleEvent::MultiHit { hits: landed });
            }
        }

        // Effects in list order (doc 02 §6), skipped entirely on immunity.
        if !immune {
            for effect in spec.effects.clone() {
                self.apply_effect(&spec, &effect, side, foe, total_dealt, typeless, rng);
            }
        }

        // Last Resort recoil (v1.1 #3): floor(damage/4).
        if typeless && total_dealt > 0 {
            let recoil = total_dealt / 4;
            if recoil > 0 {
                let dealt = self
                    .state
                    .side_mut(side)
                    .active_mote_mut()
                    .take_damage(recoil);
                self.events.push(BattleEvent::Recoiled {
                    side,
                    amount: dealt,
                });
            }
        }

        self.faint_check(foe);
        self.faint_check(side);
        self.check_outcome();
    }

    #[allow(clippy::too_many_arguments, reason = "internal dispatcher, not API")]
    fn apply_effect(
        &mut self,
        spec: &MoveSpec,
        effect: &Effect,
        side: SideId,
        foe: SideId,
        total_dealt: u32,
        typeless: bool,
        rng: &mut BattleRng,
    ) {
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
                let target_side = match target {
                    EffectTarget::User => side,
                    EffectTarget::Target => foe,
                };
                if self.state.side(target_side).active_mote().is_fainted() {
                    return;
                }
                let Some(stage_stat) = StageStat::from_stat(*stat) else {
                    return; // HP has no stage
                };
                let (new_stage, clamped) = self
                    .state
                    .side_mut(target_side)
                    .active_state
                    .stages
                    .bump(stage_stat, *delta);
                self.events.push(if clamped {
                    BattleEvent::StatStageClamped {
                        target: target_side,
                        stat: *stat,
                    }
                } else {
                    BattleEvent::StatStageChanged {
                        target: target_side,
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
                let max_hp = u32::from(self.state.side(side).active_mote().max_hp());
                let healed = self
                    .state
                    .side_mut(side)
                    .active_mote_mut()
                    .heal(frac.apply(max_hp));
                if healed > 0 {
                    self.events.push(BattleEvent::Healed {
                        target: side,
                        amount: healed,
                    });
                }
            }
            Effect::Drain { frac } => {
                let amount = frac.apply(total_dealt);
                if amount > 0 {
                    let healed = self.state.side_mut(side).active_mote_mut().heal(amount);
                    if healed > 0 {
                        self.events.push(BattleEvent::Drained {
                            from: foe,
                            amount: healed,
                        });
                    }
                }
            }
            Effect::Recoil { frac } => {
                let amount = frac.apply(total_dealt);
                if amount > 0 {
                    let dealt = self
                        .state
                        .side_mut(side)
                        .active_mote_mut()
                        .take_damage(amount);
                    self.events.push(BattleEvent::Recoiled {
                        side,
                        amount: dealt,
                    });
                }
            }
            Effect::Flinch { chance } => {
                if rng.chance(u32::from(*chance), 100)
                    && !self.state.side(foe).active_mote().is_fainted()
                {
                    self.state.side_mut(foe).active_state.flinched = true;
                }
            }
            Effect::Weather { kind } => {
                self.state.weather = Some((*kind, 5));
                self.events
                    .push(BattleEvent::WeatherChanged { kind: Some(*kind) });
            }
            Effect::Protect => {
                self.state.side_mut(side).active_state.protected = true;
            }
            Effect::ForceSwitch => {
                // A KO from this same move wins over the drag: the faint
                // must reach the event stream before any switch could
                // hide it (doc 03 §2; v1.2 #1/#3 award rules).
                self.faint_check(foe);
                if self.state.side(foe).active_mote().is_fainted() {
                    return;
                }
                let bench: Vec<u8> = {
                    let s = self.state.side(foe);
                    s.party
                        .iter()
                        .enumerate()
                        .filter(|(i, m)| *i != usize::from(s.active) && !m.is_fainted())
                        .map(|(i, _)| u8::try_from(i).expect("party ≤ 6"))
                        .collect()
                };
                if !bench.is_empty() {
                    let pick =
                        bench[usize::try_from(rng.below(u32::try_from(bench.len()).expect("≤ 6")))
                            .expect("index")];
                    self.perform_switch(foe, pick);
                    // v1.2 #10: the dragged-in Mote does not act with its
                    // predecessor's queued action.
                    self.cancelled[usize::from(foe)] = true;
                }
            }
            Effect::SelfSwitch => {
                // Same faint-before-switch rule as ForceSwitch.
                self.faint_check(side);
                if self.state.side(side).active_mote().is_fainted() {
                    return;
                }
                if let Some(replacement) = self.state.side(side).first_replacement() {
                    self.perform_switch(side, replacement);
                }
            }
            Effect::Ohko => {
                let hp = u32::from(self.state.side(foe).active_mote().hp);
                if hp > 0 {
                    let dealt = self.state.side_mut(foe).active_mote_mut().take_damage(hp);
                    self.events.push(BattleEvent::DamageDealt {
                        target: foe,
                        amount: dealt,
                        crit: false,
                        effectiveness: undersong_core::types::Eff::Neutral,
                    });
                }
            }
            Effect::FixedDamage { amount } => {
                let raw = match amount {
                    FixedAmount::Amount(n) => u32::from(*n),
                    FixedAmount::UserLevel => u32::from(self.state.side(side).active_mote().level),
                };
                let dealt = self.state.side_mut(foe).active_mote_mut().take_damage(raw);
                self.events.push(BattleEvent::DamageDealt {
                    target: foe,
                    amount: dealt,
                    crit: false,
                    effectiveness: undersong_core::types::Eff::Neutral,
                });
            }
            // Handled in the act() flow, not as post-damage effects.
            Effect::MultiHit | Effect::TwoTurn { .. } => {}
        }
        let _ = typeless;
    }

    /// Applies a major status respecting one-at-a-time and the type
    /// immunities of doc 02 §5. Returns whether it stuck.
    fn try_apply_status(&mut self, target: SideId, ailment: Ailment, rng: &mut BattleRng) -> bool {
        let mote = self.state.side(target).active_mote();
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
        self.state.side_mut(target).active_mote_mut().status = Some(status);
        self.events.push(BattleEvent::StatusApplied {
            target,
            status: ailment,
        });
        true
    }

    // ----- end of turn ---------------------------------------------------

    fn end_of_turn(&mut self, _rng: &mut BattleRng) {
        use undersong_core::moves::WeatherKind;

        // 1) Weather chip (v1.1 #2.1), side 0's active first. All EOT
        // fraction damage has a 1 HP minimum (v1.2 #7).
        if let Some((kind, _)) = self.state.weather {
            for side in 0..2u8 {
                let mote = self.state.side(side).active_mote();
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
                    let dealt = self
                        .state
                        .side_mut(side)
                        .active_mote_mut()
                        .take_damage(chip.max(1));
                    self.events.push(BattleEvent::WeatherChip {
                        target: side,
                        amount: dealt,
                    });
                    self.faint_check(side);
                }
            }
        }

        // 2) Seeded drain (doc 02 §5: 1/8 to the opposer).
        for side in 0..2u8 {
            if !self.state.side(side).active_state.seeded {
                continue;
            }
            let mote = self.state.side(side).active_mote();
            if mote.is_fainted() {
                continue;
            }
            let amount = (u32::from(mote.max_hp()) / 8).max(1);
            let dealt = self
                .state
                .side_mut(side)
                .active_mote_mut()
                .take_damage(amount);
            self.events.push(BattleEvent::SeededDrain {
                from: side,
                amount: dealt,
            });
            let foe = 1 - side;
            if !self.state.side(foe).active_mote().is_fainted() {
                self.state
                    .side_mut(foe)
                    .active_mote_mut()
                    .heal(u32::from(dealt));
            }
            self.faint_check(side);
        }

        // 3) Burn / poison / toxic (doc 02 §5; order v1.1 #2.3).
        for side in 0..2u8 {
            let mote = self.state.side(side).active_mote();
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
            let dealt = self
                .state
                .side_mut(side)
                .active_mote_mut()
                .take_damage(amount.max(1));
            self.state.side_mut(side).active_mote_mut().status = next;
            self.events.push(BattleEvent::StatusTicked {
                target: side,
                status: ailment,
                damage: dealt,
            });
            self.faint_check(side);
        }

        // 4) Weather countdown.
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

    /// Emits Fainted (once) and awards exp/EVs to the opposing active
    /// (doc 02 §9, v1.1 #13–14).
    fn faint_check(&mut self, side: SideId) {
        let mote = self.state.side(side).active_mote();
        if !mote.is_fainted() {
            return;
        }
        // Already emitted for this faint?
        let already =
            self.events
                .iter()
                .rev()
                .any(|e| matches!(e, BattleEvent::Fainted { target } if *target == side))
                && {
                    // A new switch-in resets the "already fainted" detection.
                    let last_switch = self.events.iter().rposition(
                        |e| matches!(e, BattleEvent::SwitchedIn { side: s, .. } if *s == side),
                    );
                    let last_faint = self.events.iter().rposition(
                        |e| matches!(e, BattleEvent::Fainted { target } if *target == side),
                    );
                    match (last_faint, last_switch) {
                        (Some(f), Some(s)) => f > s,
                        (Some(_), None) => true,
                        _ => false,
                    }
                };
        if already {
            return;
        }
        self.events.push(BattleEvent::Fainted { target: side });

        let victor: SideId = 1 - side;
        // Exp/EV awards are player-side only (doc 02 v1.2 #1); the Fainted
        // event above is unconditional.
        if victor != 0 {
            return;
        }
        let (yield_base, level, ev_yield) = {
            let fainted = self.state.side(side).active_mote();
            (
                fainted.base_exp_yield,
                fainted.level,
                fainted.ev_yield.clone(),
            )
        };
        let trainer = matches!(self.state.kind, BattleKind::Trainer);
        let victor_slot = self.state.side(victor).active;
        let victor_mote = self.state.side_mut(victor).active_mote_mut();
        if victor_mote.is_fainted() {
            return;
        }

        // EVs first (v1.1 #14): per-stat cap 252, total cap 510, excess
        // dropped in canonical stat order.
        for (stat, amount) in ev_yield {
            let total = victor_mote.ev_sum();
            if total >= 510 {
                break;
            }
            let room_total = 510 - total;
            let current = victor_mote.evs.get(stat);
            let room_stat = u32::from(252u16.saturating_sub(current));
            let grant = u32::from(amount).min(room_total).min(room_stat);
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

        let gained = exp_gain(yield_base, level, 1, trainer, false);
        if gained > 0 {
            self.events.push(BattleEvent::ExpGained {
                side: victor,
                slot: victor_slot,
                amount: gained,
            });
            let ups = apply_exp(self.state.side_mut(victor).active_mote_mut(), gained);
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
