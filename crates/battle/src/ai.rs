//! Trainer AI tiers (doc 02 §14, scoring law v1.1 #15–16; T3 per
//! v1.5 #3: 2-ply expectimax over cloned-state turn simulations).

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

/// Picks an action for `side` in a singles battle. Only T0 consumes rng
/// (uniform choice); T1/T2 are deterministic given the state, per the
/// law's tie-breaks. The rng draw pattern is part of the locked replay
/// stream — doubles selection lives in `choose_doubles`.
pub fn choose(tier: AiTier, state: &BattleState, side: SideId, rng: &mut BattleRng) -> Action {
    match tier {
        AiTier::T0 => tier0(state, side, rng),
        AiTier::T1 => tier1(state, side),
        AiTier::T2 => tier2(state, side),
        AiTier::T3 => tier3(state, side),
    }
}

/// Tier 3 (doc 02 v1.5 #3, v1.7 erratum): for each of our legal
/// actions, assume each opposing response in turn (uniform weights),
/// simulate one full turn on a cloned state with a fixed-seed probe
/// rng, and score
/// `Δ(own team HP%) − Δ(foe team HP%) + 10·foe KOs − 10·own KOs`.
/// Picks the action with the highest MEAN score across responses; ties
/// break toward the lower move slot, then Move over Switch. Fully
/// deterministic: the probe rng is fixed-seed and the live rng is never
/// consumed.
fn tier3(state: &BattleState, side: SideId) -> Action {
    let foe: SideId = 1 - side;
    let my_actions = legal_actions(state, side);
    let foe_actions = legal_actions(state, foe);
    // Anchor: tier 2's analytic choice. The expectimax may override it
    // only by a clear margin — fixed-seed probe sims carry correlated
    // noise, and unanchored probe scores measurably lose to T1's exact
    // arithmetic (see doc 02 §14 v1.7 calibration note).
    let anchor = tier2(state, side);

    let team_hp_pct = |s: &BattleState, who: SideId| -> i64 {
        let side_state = s.side(who);
        let mut total = 0i64;
        for mote in &side_state.party {
            total += i64::from(mote.hp) * 100 / i64::from(mote.max_hp().max(1));
        }
        total
    };
    let kos = |before: &BattleState, after: &BattleState, who: SideId| -> i64 {
        let count =
            |s: &BattleState| s.side(who).party.iter().filter(|m| m.is_fainted()).count() as i64;
        count(after) - count(before)
    };

    // Positional matchup term (§14's "hand-tuned weights"): how the
    // post-turn field matchup leans, so a switch that fixes a bad
    // matchup can outscore one turn of lost tempo. Per side: the best
    // chart product any fielded usable move achieves against the foe's
    // fielded mote, mapped 0×→−20, ½×→−8, 1×→0, 2×→+12.
    let matchup_lean = |s: &BattleState| -> i64 {
        let value = |attacker: SideId| -> i64 {
            let me = s.side(attacker).active_mote();
            let them = s.side(1 - attacker).active_mote();
            if me.is_fainted() || them.is_fainted() {
                return 0;
            }
            let mut best = -20i64;
            for battle_move in &me.moves {
                if battle_move.pp == 0 || battle_move.spec.power == 0 {
                    continue;
                }
                let (num, den) = s.chart.product(battle_move.spec.r#type, &them.types);
                let worth = if num == 0 {
                    -20
                } else if num * 2 <= den {
                    -8
                } else if num > den {
                    12
                } else {
                    0
                };
                best = best.max(worth);
            }
            best
        };
        value(side) - value(foe)
    };

    let mut best: Option<(i64, usize)> = None;
    let mut scores: Vec<i64> = Vec::with_capacity(my_actions.len());
    for (index, mine) in my_actions.iter().enumerate() {
        let mut total: i64 = 0;
        for theirs in &foe_actions {
            // Three probe seeds smooth single-sample accuracy/crit noise
            // while staying fully deterministic.
            for probe_seed in 0..3u64 {
                let mut probe = BattleRng::from_seed(probe_seed);
                let actions = if side == 0 {
                    crate::actions::TurnActions::new(*mine, *theirs)
                } else {
                    crate::actions::TurnActions::new(*theirs, *mine)
                };
                let (next, _) = crate::turn::step(state, &actions, &mut probe);
                total += (team_hp_pct(&next, side) - team_hp_pct(state, side))
                    - (team_hp_pct(&next, foe) - team_hp_pct(state, foe))
                    + 10 * kos(state, &next, foe)
                    - 10 * kos(state, &next, side)
                    + matchup_lean(&next);
            }
        }
        // Uniform weights: comparing totals over the same response set
        // is equivalent to comparing means, and stays integer.
        scores.push(total);
        let better = match best {
            None => true,
            Some((best_score, _)) => total > best_score,
        };
        if better {
            best = Some((total, index));
        }
    }
    let anchor_index = my_actions.iter().position(|a| *a == anchor);
    let anchor_score = anchor_index.map(|index| scores[index]);
    match (best, anchor_score) {
        (Some((best_score, index)), Some(anchor_score)) => {
            // Override the anchor only when the probe sims see a clear
            // edge (margin: 25 points per response sample).
            let margin = 25 * i64::try_from(foe_actions.len().max(1) * 3).expect("fits");
            if my_actions[index] != anchor && best_score > anchor_score + margin {
                my_actions[index]
            } else {
                anchor
            }
        }
        (Some((_, index)), None) => my_actions[index],
        _ => anchor,
    }
}

/// Legal singles actions for max-min enumeration: usable move slots in
/// order (lower slot first — the tie-break order), then switches to
/// conscious bench Motes (Move beats Switch on ties because moves
/// enumerate first and ties keep the earlier index).
fn legal_actions(state: &BattleState, side: SideId) -> Vec<Action> {
    let mut actions = Vec::new();
    for slot in usable_slots(state, side) {
        actions.push(Action::Move { slot });
    }
    if actions.is_empty() {
        actions.push(Action::Move { slot: 0 }); // Last Resort path
    }
    let side_state = state.side(side);
    let fielded = side_state.positions[0].party_index;
    for (index, mote) in side_state.party.iter().enumerate() {
        let index = u8::try_from(index).expect("party ≤ 6");
        if index != fielded && !mote.is_fainted() {
            actions.push(Action::Switch { to: index });
        }
    }
    actions
}

/// Picks `(action, target_position)` for one doubles position
/// (doc 02 v1.5 #2: the actor declares a foe slot). T0 draws a uniform
/// usable move and a uniform conscious foe slot; T1/T2 evaluate every
/// `(move, target)` pair with the singles scoring law, ties resolving to
/// the lower move slot, then the lower target slot.
pub fn choose_doubles(
    tier: AiTier,
    state: &BattleState,
    side: SideId,
    position: u8,
    rng: &mut BattleRng,
) -> (Action, u8) {
    let foe: SideId = 1 - side;
    let targets: Vec<u8> = (0..state.side(foe).position_count())
        .filter(|&p| !state.side(foe).mote_at(p).is_fainted())
        .collect();
    match tier {
        AiTier::T0 => {
            let usable = usable_slots_at(state, side, position);
            let slot = if usable.is_empty() {
                0 // engine resolves to Last Resort
            } else {
                usable[usize::try_from(rng.below(u32::try_from(usable.len()).expect("≤ 4")))
                    .expect("index")]
            };
            let target = if targets.is_empty() {
                0
            } else {
                targets[usize::try_from(rng.below(u32::try_from(targets.len()).expect("≤ 2")))
                    .expect("index")]
            };
            (Action::Move { slot }, target)
        }
        AiTier::T1 => tier1_doubles(state, side, position, &targets),
        // Doubles T3 plays the T2 policy: joint-action expectimax over
        // four actors is out of scope at launch (doc 02 v1.5 #3 defines
        // T3 for the singles ladder; the gate measures singles).
        AiTier::T2 | AiTier::T3 => tier2_doubles(state, side, position, &targets),
    }
}

fn usable_slots(state: &BattleState, side: SideId) -> Vec<u8> {
    usable_slots_at(state, side, 0)
}

fn usable_slots_at(state: &BattleState, side: SideId, position: u8) -> Vec<u8> {
    state
        .side(side)
        .mote_at(position)
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
    expected_damage_at(state, side, 0, slot, 0)
}

/// Position-aware expected damage: `(side, position)` attacks the foe's
/// `target` position. `ctx.doubles` follows the format flag so
/// soloist/chorister score correctly (doc 02 §10).
fn expected_damage_at(
    state: &BattleState,
    side: SideId,
    position: u8,
    slot: u8,
    target: u8,
) -> u32 {
    let foe: SideId = 1 - side;
    let attacker_side = state.side(side);
    let defender_side = state.side(foe);
    let spec = &attacker_side.mote_at(position).moves[usize::from(slot)].spec;
    if spec.power == 0 || matches!(spec.category, MoveCategory::Status) {
        return 0;
    }
    // Ability immunities blank the move (doc 02 §10): the AI must not
    // spam into damper/floating walls (turn-limit-draw pathology).
    let defender_ability = defender_side.mote_at(target).ability;
    if (defender_ability == crate::abilities::Ability::Damper && spec.flags.sound)
        || (defender_ability == crate::abilities::Ability::Floating
            && spec.r#type == undersong_core::types::Type::Stone)
    {
        return 0;
    }
    let context = DamageContext {
        attacker: attacker_side.mote_at(position),
        defender: defender_side.mote_at(target),
        attacker_stages: &attacker_side.positions[usize::from(position)].state.stages,
        defender_stages: &defender_side.positions[usize::from(target)].state.stages,
        chart: &state.chart,
        weather: state.weather.map(|(kind, _)| kind),
        crit: false,
        rand: 92,
        spread: false,
        doubles: matches!(state.format, crate::state::Format::Double),
    };
    compute_damage(spec, &context).map_or(0, |outcome| outcome.amount)
}

/// T1 over `(move, target)` pairs: greedy max expected damage; ties →
/// lowest slot, then lowest target; nothing damaging → slot 0 at the
/// first conscious foe slot.
fn tier1_doubles(state: &BattleState, side: SideId, position: u8, targets: &[u8]) -> (Action, u8) {
    let usable = usable_slots_at(state, side, position);
    let mut best: Option<(u8, u8, u32)> = None;
    for &slot in &usable {
        for &target in targets {
            let damage = expected_damage_at(state, side, position, slot, target);
            if damage == 0 {
                continue;
            }
            // Iteration is (slot asc, target asc): strictly-greater keeps
            // the lowest slot, then the lowest target on ties.
            if best.is_none_or(|(_, _, b)| damage > b) {
                best = Some((slot, target, damage));
            }
        }
    }
    match best {
        Some((slot, target, _)) => (Action::Move { slot }, target),
        None => (
            Action::Move { slot: 0 },
            targets.first().copied().unwrap_or(0),
        ),
    }
}

/// T2 over `(move, target)` pairs (law v1.1 #16 scoring per target), with
/// the hard-counter switch rule checked against every conscious foe slot.
fn tier2_doubles(state: &BattleState, side: SideId, position: u8, targets: &[u8]) -> (Action, u8) {
    let foe: SideId = 1 - side;
    let fallback_target = targets.first().copied().unwrap_or(0);

    // Switch rule: any foe slot's best STAB product vs us ≥ 4 and a bench
    // Mote resists it (defensive product ≤ 1) → switch to the first such.
    let our_types = state.side(side).mote_at(position).types.clone();
    let best_threat = targets
        .iter()
        .flat_map(|&t| state.side(foe).mote_at(t).types.clone())
        .map(|t| (t, state.chart.product(t, &our_types)))
        .max_by_key(|&(_, (num, den))| u64::from(num) * 1000 / u64::from(den.max(1)));
    if let Some((threat_type, (num, den))) = best_threat
        && num >= 4 * den
    {
        let s = state.side(side);
        let resists = s.party.iter().enumerate().find(|(i, m)| {
            !s.is_fielded(u8::try_from(*i).expect("party ≤ 6")) && !m.is_fainted() && {
                let (n, d) = resist_product(state, threat_type, &m.types);
                n <= d
            }
        });
        if let Some((index, _)) = resists {
            return (
                Action::Switch {
                    to: u8::try_from(index).expect("party ≤ 6"),
                },
                fallback_target,
            );
        }
    }

    let usable = usable_slots_at(state, side, position);
    if usable.is_empty() {
        return (Action::Move { slot: 0 }, fallback_target);
    }
    let at_full_hp = {
        let m = state.side(side).mote_at(position);
        m.hp == m.max_hp()
    };
    let mut best: Option<(u8, u8, u64)> = None;
    for &slot in &usable {
        let spec = &state.side(side).mote_at(position).moves[usize::from(slot)].spec;
        for &target in targets {
            let target_mote = state.side(foe).mote_at(target);
            let target_hp = u32::from(target_mote.hp);
            let target_healthy = u32::from(target_mote.hp) * 2 > u32::from(target_mote.max_hp());
            let target_unstatused = target_mote.status.is_none();

            let damage = expected_damage_at(state, side, position, slot, target);
            let mut score = (u64::from(damage) * 100)
                .checked_div(u64::from(target_hp))
                .unwrap_or(0)
                .min(100);
            if damage >= target_hp && damage > 0 {
                score += 25; // can KO
            }
            let is_major_status = spec.effects.iter().any(
                |e| matches!(e, undersong_core::moves::Effect::Status { chance, .. } if *chance == 100),
            );
            if matches!(spec.category, MoveCategory::Status)
                && is_major_status
                && target_healthy
                && target_unstatused
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
            // (slot asc, target asc) iteration + strictly-greater: ties
            // keep the lowest slot, then the lowest target.
            if best.is_none_or(|(_, _, b)| score > b) {
                best = Some((slot, target, score));
            }
        }
        // A status move with no conscious target still scores 0 once.
        if targets.is_empty() && best.is_none() {
            best = Some((slot, 0, 0));
        }
    }
    match best {
        Some((slot, target, _)) => (Action::Move { slot }, target),
        None => (Action::Move { slot: 0 }, fallback_target),
    }
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
            !s.is_fielded(u8::try_from(*i).expect("party ≤ 6")) && !m.is_fainted() && {
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
