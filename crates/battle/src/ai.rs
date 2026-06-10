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
    let mut best: Option<(u8, u64)> = None;
    for &slot in &usable {
        let spec = &state.side(side).active_mote().moves[usize::from(slot)].spec;
        let damage = expected_damage(state, side, slot);
        // damage as % of foe's current HP, capped 100 (u64: the product
        // cannot overflow even for absurd generated content)
        let mut score = (u64::from(damage) * 100)
            .checked_div(u64::from(foe_hp))
            .unwrap_or(0)
            .min(100);
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

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use undersong_core::chart::TypeChart;
    use undersong_core::moves::{Ailment, Effect, MoveFlags, MoveSpec, MoveTarget};
    use undersong_core::species::{GrowthCurve, SpeciesSpec, StatSpread};
    use undersong_core::stats::Stat;

    use super::*;
    use crate::mote::MoteBuilder;
    use crate::state::BattleKind;

    fn neutral_chart() -> TypeChart {
        let mut text = String::from("TypeChart(entries: {");
        for attacker in Type::ALL {
            text.push_str(&format!("{attacker:?}: {{"));
            for defender in Type::ALL {
                text.push_str(&format!("{defender:?}: Neutral,"));
            }
            text.push_str("},");
        }
        text.push_str("})");
        ron::from_str(&text).expect("chart")
    }

    fn species(id: &str, types: &[Type]) -> SpeciesSpec {
        SpeciesSpec {
            id: id.into(),
            name_key: format!("motif.{id}"),
            types: types.to_vec(),
            base_stats: StatSpread {
                hp: 60,
                atk: 60,
                def: 60,
                spa: 60,
                spd: 60,
                spe: 60,
            },
            catch_rate: 100,
            base_exp_yield: 100,
            ev_yield: BTreeMap::from([(Stat::Atk, 1u8)]).into(),
            growth_curve: GrowthCurve::MediumFast,
            learnset: vec![],
            abilities: vec![],
            hidden_ability: None,
            tags: vec![],
        }
    }

    fn mk(id: &str, ty: Type, category: MoveCategory, power: u16) -> MoveSpec {
        MoveSpec {
            id: id.into(),
            name_key: format!("move.{id}"),
            r#type: ty,
            category,
            power,
            accuracy: 100,
            pp: 20,
            priority: 0,
            target: MoveTarget::Foe,
            flags: MoveFlags::default(),
            effects: vec![],
        }
    }

    fn duel(side0_moves: Vec<MoveSpec>, chart: TypeChart) -> BattleState {
        let a = species("ai_user", &[Type::Feral]);
        let b = species("ai_foe", &[Type::Feral]);
        BattleState::new(
            BattleKind::Trainer,
            vec![MoteBuilder::new(&a, 30).moves(side0_moves).build()],
            vec![
                MoteBuilder::new(&b, 30)
                    .moves(vec![mk("tackle", Type::Feral, MoveCategory::Physical, 40)])
                    .build(),
            ],
            chart,
        )
    }

    #[test]
    fn t1_equal_damage_ties_pick_lowest_slot() {
        // Slots 1 and 2 are identical; slot 0 is weaker (law v1.1 #15).
        let state = duel(
            vec![
                mk("weak", Type::Feral, MoveCategory::Physical, 20),
                mk("strong_a", Type::Feral, MoveCategory::Physical, 80),
                mk("strong_b", Type::Feral, MoveCategory::Physical, 80),
            ],
            neutral_chart(),
        );
        let mut rng = BattleRng::from_seed(1);
        assert_eq!(
            choose(AiTier::T1, &state, 0, &mut rng),
            Action::Move { slot: 1 }
        );
    }

    #[test]
    fn t1_never_picks_status_when_damaging_usable() {
        let mut lull = mk("lull", Type::Frost, MoveCategory::Status, 0);
        lull.effects = vec![Effect::Status {
            ailment: Ailment::Sleep,
            chance: 100,
        }];
        let state = duel(
            vec![lull, mk("jab", Type::Feral, MoveCategory::Physical, 40)],
            neutral_chart(),
        );
        let mut rng = BattleRng::from_seed(2);
        assert_eq!(
            choose(AiTier::T1, &state, 0, &mut rng),
            Action::Move { slot: 1 }
        );
    }

    #[test]
    fn t1_all_immune_falls_back_to_slot_zero() {
        // Foe is Phantom; only move is Feral (0x) → no damaging candidate
        // (law v1.2 #11) → slot 0.
        let mut chart_text = String::from("TypeChart(entries: {");
        for attacker in Type::ALL {
            chart_text.push_str(&format!("{attacker:?}: {{"));
            for defender in Type::ALL {
                let eff = if attacker == Type::Feral && defender == Type::Phantom {
                    "Zero"
                } else {
                    "Neutral"
                };
                chart_text.push_str(&format!("{defender:?}: {eff},"));
            }
            chart_text.push_str("},");
        }
        chart_text.push_str("})");
        let chart: TypeChart = ron::from_str(&chart_text).expect("chart");

        let a = species("ai_user", &[Type::Feral]);
        let b = species("ai_ghost", &[Type::Phantom]);
        let state = BattleState::new(
            BattleKind::Trainer,
            vec![
                MoteBuilder::new(&a, 30)
                    .moves(vec![
                        mk("futile", Type::Feral, MoveCategory::Physical, 90),
                        mk("also_futile", Type::Feral, MoveCategory::Physical, 120),
                    ])
                    .build(),
            ],
            vec![
                MoteBuilder::new(&b, 30)
                    .moves(vec![mk("haunt", Type::Phantom, MoveCategory::Special, 40)])
                    .build(),
            ],
            chart,
        );
        let mut rng = BattleRng::from_seed(3);
        assert_eq!(
            choose(AiTier::T1, &state, 0, &mut rng),
            Action::Move { slot: 0 }
        );
    }

    #[test]
    fn t2_ko_bonus_beats_higher_percent() {
        // Foe weakened so the weaker move still KOs: both cap at 100, the
        // +25 KO bonus ties, lowest slot wins; then verify a non-KO strong
        // move loses to a KO-capable weak one.
        let mut state = duel(
            vec![
                mk("chip", Type::Feral, MoveCategory::Physical, 25),
                mk("blast", Type::Feral, MoveCategory::Physical, 90),
            ],
            neutral_chart(),
        );
        // Weaken foe so chip's expected damage covers its HP.
        state.sides[1].party[0].hp = 5;
        let mut rng = BattleRng::from_seed(4);
        assert_eq!(
            choose(AiTier::T2, &state, 0, &mut rng),
            Action::Move { slot: 0 },
            "both KO → score ties at 125 → lowest slot"
        );
    }

    #[test]
    fn t2_status_bonus_on_healthy_unstatused_foe() {
        // Status move (15) must beat a tiny damaging move (< 15% of HP).
        let mut lull = mk("lull", Type::Frost, MoveCategory::Status, 0);
        lull.effects = vec![Effect::Status {
            ailment: Ailment::Sleep,
            chance: 100,
        }];
        let state = duel(
            vec![mk("pebble", Type::Feral, MoveCategory::Physical, 5), lull],
            neutral_chart(),
        );
        let mut rng = BattleRng::from_seed(5);
        assert_eq!(
            choose(AiTier::T2, &state, 0, &mut rng),
            Action::Move { slot: 1 },
            "+15 status bonus beats single-digit chip percent"
        );
    }

    #[test]
    fn t2_switches_to_resist_on_hard_counter() {
        // Foe's Ember STAB hits our dual Bloom/Gale active at 2x·2x = 4x;
        // the Tide bench Mote resists it (½x).
        let mut chart_text = String::from("TypeChart(entries: {");
        for attacker in Type::ALL {
            chart_text.push_str(&format!("{attacker:?}: {{"));
            for defender in Type::ALL {
                let eff = match (attacker, defender) {
                    (Type::Ember, Type::Bloom) => "Double",
                    (Type::Ember, Type::Gale) => "Double",
                    (Type::Ember, Type::Tide) => "Half",
                    _ => "Neutral",
                };
                chart_text.push_str(&format!("{defender:?}: {eff},"));
            }
            chart_text.push_str("},");
        }
        chart_text.push_str("})");
        let chart: TypeChart = ron::from_str(&chart_text).expect("chart");

        let frail = species("ai_frail", &[Type::Bloom, Type::Gale]);
        let wall = species("ai_wall", &[Type::Tide]);
        let pyro = species("ai_pyro", &[Type::Ember]);
        let state = BattleState::new(
            BattleKind::Trainer,
            vec![
                MoteBuilder::new(&frail, 30)
                    .moves(vec![mk("pick", Type::Bloom, MoveCategory::Physical, 55)])
                    .build(),
                MoteBuilder::new(&wall, 30)
                    .moves(vec![mk("surge", Type::Tide, MoveCategory::Special, 60)])
                    .build(),
            ],
            vec![
                MoteBuilder::new(&pyro, 30)
                    .moves(vec![mk("singe", Type::Ember, MoveCategory::Special, 60)])
                    .build(),
            ],
            chart,
        );
        let mut rng = BattleRng::from_seed(6);
        assert_eq!(
            choose(AiTier::T2, &state, 0, &mut rng),
            Action::Switch { to: 1 },
            "4x threat + resisting bench → switch (law v1.1 #16)"
        );
    }

    #[test]
    fn t3_stub_matches_t2() {
        let state = duel(
            vec![
                mk("a", Type::Feral, MoveCategory::Physical, 40),
                mk("b", Type::Feral, MoveCategory::Physical, 80),
            ],
            neutral_chart(),
        );
        let mut rng_a = BattleRng::from_seed(7);
        let mut rng_b = BattleRng::from_seed(7);
        assert_eq!(
            choose(AiTier::T3, &state, 0, &mut rng_a),
            choose(AiTier::T2, &state, 0, &mut rng_b)
        );
    }
}
