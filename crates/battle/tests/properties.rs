//! Property tests + the 10,000-battle fuzz gate (doc 03 §2).
//!
//! Strategies generate whole species/move spaces — far wider than any
//! shipped content — and drive battles with random legal play.

use battle::stats::{StageStat, Stages};
use battle::{
    Action, BattleKind, BattleMote, BattleState, MoteBuilder, TurnActions, step, turn::TURN_LIMIT,
};
use proptest::prelude::*;
use undersong_core::chart::TypeChart;
use undersong_core::moves::{
    Ailment, Effect, EffectTarget, Frac, MoveCategory, MoveFlags, MoveSpec, MoveTarget,
};
use undersong_core::rng::BattleRng;
use undersong_core::species::{GrowthCurve, SpeciesSpec, StatSpread};
use undersong_core::stats::Stat;
use undersong_core::types::Type;

fn full_neutral_chart() -> TypeChart {
    let mut rows = String::from("TypeChart(entries: {");
    for attacker in Type::ALL {
        rows.push_str(&format!("{attacker:?}: {{"));
        for defender in Type::ALL {
            rows.push_str(&format!("{defender:?}: Neutral,"));
        }
        rows.push_str("},");
    }
    rows.push_str("})");
    ron::from_str(&rows).expect("chart")
}

/// A chart where every cell is drawn from the rng — broader than the
/// shipped chart but always total.
fn random_chart(rng: &mut BattleRng) -> TypeChart {
    let mut rows = String::from("TypeChart(entries: {");
    for attacker in Type::ALL {
        rows.push_str(&format!("{attacker:?}: {{"));
        for defender in Type::ALL {
            let eff = match rng.below(8) {
                0 => "Zero",
                1..=2 => "Half",
                3..=5 => "Neutral",
                _ => "Double",
            };
            rows.push_str(&format!("{defender:?}: {eff},"));
        }
        rows.push_str("},");
    }
    rows.push_str("})");
    ron::from_str(&rows).expect("chart")
}

fn type_at(index: u32) -> Type {
    Type::ALL[usize::try_from(index).expect("index") % Type::COUNT]
}

/// Deterministically derives a random move from the battle rng.
fn random_move(id: u32, rng: &mut BattleRng) -> MoveSpec {
    let category = match rng.below(4) {
        0 => MoveCategory::Status,
        1 => MoveCategory::Physical,
        _ => MoveCategory::Special,
    };
    let power = if matches!(category, MoveCategory::Status) {
        0
    } else {
        u16::try_from(20 + rng.below(101)).expect("20..=120")
    };
    let mut effects = Vec::new();
    match rng.below(10) {
        0 => effects.push(Effect::Status {
            ailment: match rng.below(6) {
                0 => Ailment::Burn,
                1 => Ailment::Poison,
                2 => Ailment::Toxic,
                3 => Ailment::Paralysis,
                4 => Ailment::Sleep,
                _ => Ailment::Freeze,
            },
            chance: u8::try_from(1 + rng.below(100)).expect("1..=100"),
        }),
        1 => effects.push(Effect::StatStage {
            target: if rng.chance(1, 2) {
                EffectTarget::User
            } else {
                EffectTarget::Target
            },
            stat: [Stat::Atk, Stat::Def, Stat::Spa, Stat::Spd, Stat::Spe]
                [usize::try_from(rng.below(5)).expect("0..5")],
            delta: if rng.chance(1, 2) { 1 } else { -1 },
            chance: u8::try_from(1 + rng.below(100)).expect("1..=100"),
        }),
        2 => effects.push(Effect::Drain { frac: Frac(1, 2) }),
        3 => effects.push(Effect::Recoil { frac: Frac(1, 4) }),
        4 => effects.push(Effect::Flinch {
            chance: u8::try_from(1 + rng.below(50)).expect("1..=50"),
        }),
        5 if power > 0 => effects.push(Effect::MultiHit),
        6 => effects.push(Effect::Heal { frac: Frac(1, 4) }),
        7 => effects.push(match rng.below(7) {
            0 => Effect::TwoTurn {
                charge_text: "charging".into(),
            },
            1 => Effect::Protect,
            2 => Effect::Weather {
                kind: match rng.below(4) {
                    0 => undersong_core::moves::WeatherKind::Heatwave,
                    1 => undersong_core::moves::WeatherKind::Downpour,
                    2 => undersong_core::moves::WeatherKind::Flurry,
                    _ => undersong_core::moves::WeatherKind::Dustchord,
                },
            },
            3 => Effect::ForceSwitch,
            4 => Effect::SelfSwitch,
            5 => Effect::Ohko,
            _ => Effect::FixedDamage {
                amount: if rng.chance(1, 2) {
                    undersong_core::moves::FixedAmount::UserLevel
                } else {
                    undersong_core::moves::FixedAmount::Amount(20)
                },
            },
        }),
        _ => {}
    }
    MoveSpec {
        id: format!("fuzz_move_{id}").as_str().into(),
        name_key: format!("move.fuzz_{id}"),
        r#type: type_at(rng.below(12)),
        category,
        power,
        accuracy: if rng.chance(1, 4) {
            0
        } else {
            u8::try_from(50 + rng.below(51)).expect("50..=100")
        },
        pp: u8::try_from(5 + rng.below(36)).expect("5..=40"),
        priority: i8::try_from(rng.below(3)).expect("0..=2") - 1,
        target: MoveTarget::Foe,
        flags: MoveFlags {
            high_crit: rng.chance(1, 8),
            ignore_evasion: rng.chance(1, 16),
            sound: rng.chance(1, 4),
            contact: rng.chance(1, 2),
            ..MoveFlags::default()
        },
        effects,
    }
}

fn random_species(id: u32, rng: &mut BattleRng) -> SpeciesSpec {
    let stat = |rng: &mut BattleRng| u16::try_from(30 + rng.below(91)).expect("30..=120");
    let mut types = vec![type_at(rng.below(12))];
    if rng.chance(1, 3) {
        let second = type_at(rng.below(12));
        if second != types[0] {
            types.push(second);
        }
    }
    SpeciesSpec {
        id: format!("fuzz_species_{id}").as_str().into(),
        name_key: format!("motif.fuzz_{id}"),
        types,
        base_stats: StatSpread {
            hp: stat(rng),
            atk: stat(rng),
            def: stat(rng),
            spa: stat(rng),
            spd: stat(rng),
            spe: stat(rng),
        },
        catch_rate: u8::try_from(3 + rng.below(253)).expect("3..=255"),
        base_exp_yield: u16::try_from(50 + rng.below(200)).expect("50..=249"),
        ev_yield: std::collections::BTreeMap::from([(Stat::Atk, 1u8)]).into(),
        growth_curve: GrowthCurve::MediumFast,
        learnset: vec![],
        abilities: vec![],
        hidden_ability: None,
        tags: vec![],
    }
}

fn random_battle_mote(id: u32, rng: &mut BattleRng) -> BattleMote {
    let spec = random_species(id, rng);
    let level = u8::try_from(5 + rng.below(60)).expect("5..=64");
    let move_count = 1 + rng.below(4);
    let moves: Vec<MoveSpec> = (0..move_count)
        .map(|i| random_move(id * 8 + i, rng))
        .collect();
    let ivs = StatSpread {
        hp: u16::try_from(rng.below(32)).expect("iv"),
        atk: u16::try_from(rng.below(32)).expect("iv"),
        def: u16::try_from(rng.below(32)).expect("iv"),
        spa: u16::try_from(rng.below(32)).expect("iv"),
        spd: u16::try_from(rng.below(32)).expect("iv"),
        spe: u16::try_from(rng.below(32)).expect("iv"),
    };
    MoteBuilder::new(&spec, level)
        .ivs(ivs)
        .nature(u8::try_from(rng.below(25)).expect("nature"))
        .moves(moves)
        .build()
}

/// Random legal-ish action: mostly moves, sometimes switches, in wild
/// battles occasionally run/bell. Illegal choices are fine — the engine
/// must resolve them deterministically without panicking.
fn random_action(state: &BattleState, side: u8, rng: &mut BattleRng) -> Action {
    let wild = matches!(state.kind, BattleKind::Wild);
    match rng.below(if wild && side == 0 { 12 } else { 10 }) {
        0..=7 => Action::Move {
            slot: u8::try_from(rng.below(4)).expect("0..4"),
        },
        8..=9 => Action::Switch {
            to: u8::try_from(rng.below(3)).expect("0..3"),
        },
        10 => Action::Run,
        _ => Action::UseBell {
            bell_mod: Frac(1, 1),
        },
    }
}

/// Drives one fully random battle; returns turns played. Asserts the
/// core invariants every step.
fn run_random_battle(seed: u64) -> u16 {
    let mut rng = BattleRng::from_seed(seed);
    let chart = if rng.chance(1, 2) {
        random_chart(&mut rng)
    } else {
        full_neutral_chart()
    };
    let party = |rng: &mut BattleRng, base: u32| -> Vec<BattleMote> {
        (0..1 + rng.below(3))
            .map(|i| random_battle_mote(base + i, rng))
            .collect()
    };
    let kind = if rng.chance(1, 3) {
        BattleKind::Wild
    } else {
        BattleKind::Trainer
    };
    let side0 = party(&mut rng, 0);
    let side1 = if matches!(kind, BattleKind::Wild) {
        vec![random_battle_mote(100, &mut rng)]
    } else {
        party(&mut rng, 100)
    };
    let mut state = BattleState::new(kind, side0, side1, chart);

    let mut steps = 0u32;
    while state.outcome.is_none() {
        let actions = TurnActions::new(
            random_action(&state, 0, &mut rng),
            random_action(&state, 1, &mut rng),
        );
        let (next, _) = step(&state, &actions, &mut rng);
        state = next;
        steps += 1;
        assert!(
            steps <= u32::from(TURN_LIMIT) + 1,
            "battle failed to terminate (seed {seed})"
        );
        // Invariants after every step:
        for side in &state.sides {
            for mote in &side.party {
                assert!(mote.hp <= mote.max_hp(), "hp ≤ max (seed {seed})");
                assert!(mote.ev_sum() <= 510, "EV sum ≤ 510 (seed {seed})");
                assert!(mote.level <= 100, "level cap (seed {seed})");
            }
            for stage in side.active_state.stages.0 {
                assert!((-6..=6).contains(&stage), "stage clamp (seed {seed})");
            }
        }
    }
    state.turn
}

// ---- the 10,000-battle fuzz gate (doc 03 §2) ---------------------------

/// Gate P1: 10,000 random seeded battles — zero panics, all terminate.
/// Plain loop (not proptest) so the count is exact and the seeds stable.
#[test]
fn fuzz_ten_thousand_battles_no_panics_all_terminate() {
    for seed in 0..10_000u64 {
        run_random_battle(seed);
    }
}

// ---- proptest properties (doc 03 §2 list) ------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    /// Same seed ⇒ byte-identical event streams, across the whole
    /// generated battle space.
    #[test]
    fn replays_are_deterministic(seed in 0u64..1_000_000) {
        let run = |seed: u64| {
            let mut rng = BattleRng::from_seed(seed);
            let chart = random_chart(&mut rng);
            // Half the runs are wild so the run/bell/catch rng paths fall
            // under the byte-identical-stream property too.
            let kind = if seed.is_multiple_of(2) {
                BattleKind::Trainer
            } else {
                BattleKind::Wild
            };
            let side0 = vec![random_battle_mote(0, &mut rng)];
            let side1 = vec![random_battle_mote(1, &mut rng)];
            let mut state = BattleState::new(kind, side0, side1, chart);
            let mut events = Vec::new();
            while state.outcome.is_none() {
                let actions = TurnActions::new(
                    random_action(&state, 0, &mut rng),
                    random_action(&state, 1, &mut rng),
                );
                let (next, step_events) = step(&state, &actions, &mut rng);
                events.extend(step_events);
                state = next;
            }
            ron::to_string(&events).expect("serialize")
        };
        prop_assert_eq!(run(seed), run(seed));
    }

    /// Battle states round-trip through RON unchanged.
    #[test]
    fn state_serialization_round_trips(seed in 0u64..1_000_000) {
        let mut rng = BattleRng::from_seed(seed);
        let chart = random_chart(&mut rng);
        let side0 = vec![random_battle_mote(0, &mut rng), random_battle_mote(2, &mut rng)];
        let side1 = vec![random_battle_mote(1, &mut rng)];
        let state = BattleState::new(BattleKind::Wild, side0, side1, chart);
        let text = ron::to_string(&state).expect("serialize");
        let back: BattleState = ron::from_str(&text).expect("deserialize");
        prop_assert_eq!(back, state);
    }

    /// Damage is ≥ 1 whenever the type product is positive (doc 02 §4),
    /// across random stats, stages, levels, and rolls.
    #[test]
    fn damage_at_least_one_when_effective(
        seed in 0u64..1_000_000,
        attacker_level in 1u8..=100,
        power in 1u16..=150,
        rand_roll in 85u8..=100,
    ) {
        use battle::damage::{DamageContext, compute_damage};

        let mut rng = BattleRng::from_seed(seed);
        let chart = random_chart(&mut rng);
        let mut attacker = random_battle_mote(0, &mut rng);
        attacker.level = attacker_level;
        attacker.recompute_stats();
        let defender = random_battle_mote(1, &mut rng);
        let spec = MoveSpec {
            power,
            category: MoveCategory::Physical,
            ..random_move(7, &mut rng)
        };
        let stages = (Stages::default(), Stages::default());
        let context = DamageContext {
            attacker: &attacker,
            defender: &defender,
            attacker_stages: &stages.0,
            defender_stages: &stages.1,
            chart: &chart,
            weather: None,
            crit: false,
            rand: rand_roll,
            spread: false,
        };
        let outcome = compute_damage(&spec, &context).expect("damaging");
        if outcome.type_product.0 > 0 {
            prop_assert!(outcome.amount >= 1);
        } else {
            prop_assert_eq!(outcome.amount, 0);
        }
    }

    /// Stage bumps never leave −6..=+6 regardless of sequence.
    #[test]
    fn stages_always_clamped(deltas in proptest::collection::vec(-3i8..=3, 0..64)) {
        let mut stages = Stages::default();
        for (i, delta) in deltas.iter().enumerate() {
            let stat = [
                StageStat::Atk, StageStat::Def, StageStat::Spa, StageStat::Spd,
                StageStat::Spe, StageStat::Acc, StageStat::Eva,
            ][i % StageStat::COUNT];
            stages.bump(stat, *delta);
            for value in stages.0 {
                prop_assert!((-6..=6).contains(&value));
            }
        }
    }
}
