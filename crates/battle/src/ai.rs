//! Trainer AI tiers (doc 02 §14, scoring law v1.1 #15–16).
//!
//! Tier 3 (2-ply expectimax) is a P5 deliverable; until then it returns
//! tier 2's choice (roadmap P1).

use undersong_core::moves::MoveCategory;
use undersong_core::rng::BattleRng;
use undersong_core::types::Type;

use crate::actions::Action;
use crate::damage::{DamageContext, compute_damage};
use crate::events::SideId;
use crate::state::BattleState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiTier {
    T0,
    T1,
    T2,
    T3,
}

/// Picks an action for `side`. Only T0 consumes rng (uniform choice);
/// T1/T2 are deterministic given the state, per the law's tie-breaks.
pub fn choose(tier: AiTier, state: &BattleState, side: SideId, rng: &mut BattleRng) -> Action {
    match tier {
        AiTier::T0 => tier0(state, side, rng),
        AiTier::T1 => tier1(state, side),
        // T3 stub returns tier 2 until P5 (roadmap).
        AiTier::T2 | AiTier::T3 => tier2(state, side),
    }
}

fn usable_slots(state: &BattleState, side: SideId) -> Vec<u8> {
    state
        .side(side)
        .active_mote()
        .moves
        .iter()
        .enumerate()
        .filter(|(_, m)| m.pp > 0)
        .map(|(i, _)| u8::try_from(i).expect("≤ 4"))
        .collect()
}

/// T0: uniform random usable move (doc 02 §14).
fn tier0(state: &BattleState, side: SideId, rng: &mut BattleRng) -> Action {
    let usable = usable_slots(state, side);
    if usable.is_empty() {
        return Action::Move { slot: 0 }; // engine resolves to Last Resort
    }
    let pick = usable
        [usize::try_from(rng.below(u32::try_from(usable.len()).expect("≤ 4"))).expect("index")];
    Action::Move { slot: pick }
}

/// Expected damage for T1/T2 (law v1.1 #15): full pipeline, rand fixed
/// at 92, no crit.
fn expected_damage(state: &BattleState, side: SideId, slot: u8) -> u32 {
    let foe: SideId = 1 - side;
    let attacker_side = state.side(side);
    let defender_side = state.side(foe);
    let spec = &attacker_side.active_mote().moves[usize::from(slot)].spec;
    if spec.power == 0 || matches!(spec.category, MoveCategory::Status) {
        return 0;
    }
    let context = DamageContext {
        attacker: attacker_side.active_mote(),
        defender: defender_side.active_mote(),
        attacker_stages: &attacker_side.active_state.stages,
        defender_stages: &defender_side.active_state.stages,
        chart: &state.chart,
        weather: state.weather.map(|(kind, _)| kind),
        crit: false,
        rand: 92,
        spread: false,
    };
    compute_damage(spec, &context).map_or(0, |outcome| outcome.amount)
}

/// T1: greedy max expected damage; never uses status moves; no damaging
/// PP left → slot 0 (law v1.1 #15).
fn tier1(state: &BattleState, side: SideId) -> Action {
    let usable = usable_slots(state, side);
    let best = usable
        .iter()
        .map(|&slot| (slot, expected_damage(state, side, slot)))
        .filter(|&(_, damage)| damage > 0)
        .max_by(|a, b| a.1.cmp(&b.1).then(b.0.cmp(&a.0))); // ties → lowest slot
    match best {
        Some((slot, _)) => Action::Move { slot },
        None => Action::Move { slot: 0 },
    }
}

/// T2 (law v1.1 #16): 1-ply scoring + hard-counter switching.
fn tier2(state: &BattleState, side: SideId) -> Action {
    let foe: SideId = 1 - side;

    // Switch rule: foe's best STAB product vs us ≥ 4 and a bench Mote
    // resists it (defensive product ≤ 1) → switch to the first such.
    let our_types = state.side(side).active_mote().types.clone();
    let foe_types = state.side(foe).active_mote().types.clone();
    let best_threat = foe_types
        .iter()
        .map(|&t| (t, state.chart.product(t, &our_types)))
        .max_by_key(|&(_, (num, den))| u64::from(num) * 1000 / u64::from(den.max(1)));
    if let Some((threat_type, (num, den))) = best_threat
        && num >= 4 * den
    {
        let s = state.side(side);
        let resists = s.party.iter().enumerate().find(|(i, m)| {
            *i != usize::from(s.active) && !m.is_fainted() && {
                let (n, d) = resist_product(state, threat_type, &m.types);
                n <= d
            }
        });
        if let Some((index, _)) = resists {
            return Action::Switch {
                to: u8::try_from(index).expect("party ≤ 6"),
            };
        }
    }

    // Move scoring.
    let foe_hp = u32::from(state.side(foe).active_mote().hp);
    let foe_healthy = {
        let m = state.side(foe).active_mote();
        u32::from(m.hp) * 2 > u32::from(m.max_hp())
    };
    let foe_unstatused = state.side(foe).active_mote().status.is_none();
    let at_full_hp = {
        let m = state.side(side).active_mote();
        m.hp == m.max_hp()
    };

    let usable = usable_slots(state, side);
    if usable.is_empty() {
        return Action::Move { slot: 0 };
    }
    let mut best: Option<(u8, u32)> = None;
    for &slot in &usable {
        let spec = &state.side(side).active_mote().moves[usize::from(slot)].spec;
        let damage = expected_damage(state, side, slot);
        // damage as % of foe's current HP, capped 100
        let mut score = (damage * 100).checked_div(foe_hp).unwrap_or(0).min(100);
        if damage >= foe_hp && damage > 0 {
            score += 25; // can KO
        }
        let is_major_status = spec.effects.iter().any(
            |e| matches!(e, undersong_core::moves::Effect::Status { chance, .. } if *chance == 100),
        );
        if matches!(spec.category, MoveCategory::Status)
            && is_major_status
            && foe_healthy
            && foe_unstatused
        {
            score += 15;
        }
        let is_self_setup = spec.effects.iter().any(|e| {
            matches!(
                e,
                undersong_core::moves::Effect::StatStage {
                    target: undersong_core::moves::EffectTarget::User,
                    delta,
                    ..
                } if *delta > 0
            )
        });
        if matches!(spec.category, MoveCategory::Status) && is_self_setup && at_full_hp {
            score += 10;
        }
        let better = match best {
            None => true,
            Some((best_slot, best_score)) => {
                score > best_score || (score == best_score && slot < best_slot)
            }
        };
        if better {
            best = Some((slot, score));
        }
    }
    Action::Move {
        slot: best.expect("usable nonempty").0,
    }
}

/// Defensive product of `attacker_type` vs a defender's types.
fn resist_product(state: &BattleState, attacker_type: Type, defender_types: &[Type]) -> (u32, u32) {
    state.chart.product(attacker_type, defender_types)
}
