//! Turn-engine integration tests: phase order, status law, determinism.
//! Specs are built inline so the tests are self-contained and exact.

use battle::{
    Action, BattleEvent, BattleKind, BattleMote, BattleState, MoteBuilder, Outcome, TurnActions,
    step,
};
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

fn move_spec(id: &str, ty: Type, category: MoveCategory, power: u16, priority: i8) -> MoveSpec {
    MoveSpec {
        id: id.into(),
        name_key: format!("move.{id}"),
        r#type: ty,
        category,
        power,
        accuracy: 0, // sure hit unless a test overrides
        pp: 20,
        priority,
        target: MoveTarget::Foe,
        flags: MoveFlags::default(),
        effects: vec![],
    }
}

fn species(id: &str, types: &[Type], spe: u16) -> SpeciesSpec {
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
            spe,
        },
        catch_rate: 100,
        base_exp_yield: 100,
        ev_yield: std::collections::BTreeMap::from([(Stat::Atk, 1u8)]).into(),
        growth_curve: GrowthCurve::MediumFast,
        learnset: vec![],
        abilities: vec![],
        hidden_ability: None,
        tags: vec![],
    }
}

fn mote(spec: &SpeciesSpec, level: u8, moves: Vec<MoveSpec>) -> BattleMote {
    MoteBuilder::new(spec, level).moves(moves).build()
}

fn wild_state(side0: Vec<BattleMote>, side1: Vec<BattleMote>) -> BattleState {
    BattleState::new(BattleKind::Wild, side0, side1, full_neutral_chart())
}

fn trainer_state(side0: Vec<BattleMote>, side1: Vec<BattleMote>) -> BattleState {
    BattleState::new(BattleKind::Trainer, side0, side1, full_neutral_chart())
}

fn both_move(slot0: u8, slot1: u8) -> TurnActions {
    TurnActions::new(Action::Move { slot: slot0 }, Action::Move { slot: slot1 })
}

/// Index of the first event matching the predicate.
fn position(events: &[BattleEvent], pred: impl Fn(&BattleEvent) -> bool) -> Option<usize> {
    events.iter().position(pred)
}

#[test]
fn priority_beats_speed() {
    let fast = species("fast", &[Type::Feral], 200);
    let slow = species("slow", &[Type::Feral], 10);
    let tackle = move_spec("tackle", Type::Feral, MoveCategory::Physical, 40, 0);
    let quick = move_spec("quick_step", Type::Feral, MoveCategory::Physical, 40, 1);

    // Slow side has the priority move; it must act first.
    let state = trainer_state(
        vec![mote(&slow, 20, vec![quick.clone()])],
        vec![mote(&fast, 20, vec![tackle.clone()])],
    );
    let mut rng = BattleRng::from_seed(1);
    let (_, events) = step(&state, &both_move(0, 0), &mut rng);

    let first_move = position(&events, |e| matches!(e, BattleEvent::MoveUsed { .. }));
    let quick_used = position(
        &events,
        |e| matches!(e, BattleEvent::MoveUsed { side: 0, move_id } if move_id.as_str() == "quick_step"),
    );
    assert_eq!(first_move, quick_used, "priority move acts first");
}

#[test]
fn speed_orders_equal_priority_and_paralysis_quarters_speed() {
    let fast = species("fast", &[Type::Feral], 100);
    let slow = species("slow", &[Type::Feral], 30);
    let tackle = move_spec("tackle", Type::Feral, MoveCategory::Physical, 40, 0);

    // Unparalyzed: fast (side 1) first.
    let state = trainer_state(
        vec![mote(&slow, 20, vec![tackle.clone()])],
        vec![mote(&fast, 20, vec![tackle.clone()])],
    );
    let mut rng = BattleRng::from_seed(2);
    let (_, events) = step(&state, &both_move(0, 0), &mut rng);
    let first = position(&events, |e| matches!(e, BattleEvent::MoveUsed { .. })).unwrap();
    assert!(
        matches!(events[first], BattleEvent::MoveUsed { side: 1, .. }),
        "faster side moves first"
    );

    // Paralyze the fast one: 100/4 = 25 < 30 → slow side now first.
    // (Seed chosen so the paralysis full-stop doesn't fire this turn.)
    let mut para_state = state.clone();
    para_state.sides[1].party[0].status = Some(battle::mote::MajorStatus::Paralysis);
    let mut rng = BattleRng::from_seed(11);
    let (_, events) = step(&para_state, &both_move(0, 0), &mut rng);
    let first = position(&events, |e| matches!(e, BattleEvent::MoveUsed { .. })).unwrap();
    assert!(
        matches!(events[first], BattleEvent::MoveUsed { side: 0, .. }),
        "paralysis quarters effective speed"
    );
    // Guard against a vacuous pass: the paralyzed side must still have
    // acted this turn (the seed avoids the 25% full stop).
    assert!(
        events
            .iter()
            .any(|e| matches!(e, BattleEvent::MoveUsed { side: 1, .. })),
        "both sides moved — ordering was actually exercised"
    );
}

#[test]
fn switch_resolves_before_moves() {
    let sp = species("swapper", &[Type::Feral], 50);
    let tackle = move_spec("tackle", Type::Feral, MoveCategory::Physical, 40, 0);
    let state = trainer_state(
        vec![
            mote(&sp, 20, vec![tackle.clone()]),
            mote(&sp, 20, vec![tackle.clone()]),
        ],
        vec![mote(&sp, 20, vec![tackle.clone()])],
    );
    let mut rng = BattleRng::from_seed(3);
    let (next, events) = step(
        &state,
        &TurnActions::new(Action::Switch { to: 1 }, Action::Move { slot: 0 }),
        &mut rng,
    );

    let switch_at = position(&events, |e| {
        matches!(e, BattleEvent::SwitchedIn { side: 0, .. })
    })
    .expect("switched");
    let move_at = position(&events, |e| {
        matches!(e, BattleEvent::MoveUsed { side: 1, .. })
    })
    .expect("foe moved");
    assert!(switch_at < move_at, "switch resolves before any move");
    assert_eq!(next.sides[0].active, 1);
    // The incoming Mote took the hit.
    assert!(next.sides[0].party[1].hp < next.sides[0].party[1].max_hp());
    assert_eq!(next.sides[0].party[0].hp, next.sides[0].party[0].max_hp());
}

#[test]
fn end_of_turn_tick_order_is_law() {
    // Law v1.1 #2: weather chip → seeded drain → poison → weather count.
    let sp = species("ticker", &[Type::Feral], 50);
    let tackle = move_spec("tackle", Type::Feral, MoveCategory::Physical, 10, 0);
    let mut state = trainer_state(
        vec![mote(&sp, 30, vec![tackle.clone()])],
        vec![mote(&sp, 30, vec![tackle.clone()])],
    );
    state.weather = Some((undersong_core::moves::WeatherKind::Flurry, 3));
    state.sides[0].active_state.seeded = true;
    state.sides[0].party[0].status = Some(battle::mote::MajorStatus::Poison);

    let mut rng = BattleRng::from_seed(4);
    let (next, events) = step(&state, &both_move(0, 0), &mut rng);

    let chip = position(&events, |e| {
        matches!(e, BattleEvent::WeatherChip { target: 0, .. })
    })
    .expect("flurry chips the non-frost active");
    let drain = position(&events, |e| {
        matches!(e, BattleEvent::SeededDrain { from: 0, .. })
    })
    .expect("seeded drain ticks");
    let poison = position(&events, |e| {
        matches!(
            e,
            BattleEvent::StatusTicked {
                target: 0,
                status: Ailment::Poison,
                ..
            }
        )
    })
    .expect("poison ticks");
    assert!(
        chip < drain && drain < poison,
        "EOT order: chip < seed < poison"
    );
    assert_eq!(
        next.weather,
        Some((undersong_core::moves::WeatherKind::Flurry, 2))
    );
}

#[test]
fn status_immunities_hold() {
    let volt = species("volty", &[Type::Volt], 50);
    let ember = species("embery", &[Type::Ember], 50);
    let venom = species("venomy", &[Type::Venom], 50);
    let frost = species("frosty", &[Type::Frost], 50);

    let mut para = move_spec("zap", Type::Volt, MoveCategory::Status, 0, 0);
    para.effects = vec![Effect::Status {
        ailment: Ailment::Paralysis,
        chance: 100,
    }];
    let mut burn = move_spec("scorch", Type::Ember, MoveCategory::Status, 0, 0);
    burn.effects = vec![Effect::Status {
        ailment: Ailment::Burn,
        chance: 100,
    }];
    let mut poison = move_spec("sour", Type::Venom, MoveCategory::Status, 0, 0);
    poison.effects = vec![Effect::Status {
        ailment: Ailment::Poison,
        chance: 100,
    }];
    let mut freeze = move_spec("chill", Type::Frost, MoveCategory::Status, 0, 0);
    freeze.effects = vec![Effect::Status {
        ailment: Ailment::Freeze,
        chance: 100,
    }];

    // (target species, move) pairs that must all fail.
    let cases: Vec<(SpeciesSpec, MoveSpec)> = vec![
        (volt.clone(), para),
        (ember.clone(), burn),
        (venom.clone(), poison.clone()),
        (frost.clone(), freeze),
    ];
    for (target_species, status_move) in cases {
        let attacker = species("caster", &[Type::Feral], 80);
        let state = trainer_state(
            vec![mote(&attacker, 20, vec![status_move.clone()])],
            vec![mote(
                &target_species,
                20,
                vec![move_spec(
                    "tackle",
                    Type::Feral,
                    MoveCategory::Physical,
                    40,
                    0,
                )],
            )],
        );
        let mut rng = BattleRng::from_seed(5);
        let (next, events) = step(&state, &both_move(0, 0), &mut rng);
        assert!(
            next.sides[1].party[0].status.is_none(),
            "immune to its own element"
        );
        assert!(
            events
                .iter()
                .any(|e| matches!(e, BattleEvent::MoveFailed { side: 0 })),
            "pure status move reports failure"
        );
    }

    // Alloy is immune to poison too (doc 02 §5).
    let alloy = species("alloyed", &[Type::Alloy], 50);
    let attacker = species("caster", &[Type::Feral], 80);
    let state = trainer_state(
        vec![mote(&attacker, 20, vec![poison])],
        vec![mote(
            &alloy,
            20,
            vec![move_spec(
                "tackle",
                Type::Feral,
                MoveCategory::Physical,
                40,
                0,
            )],
        )],
    );
    let mut rng = BattleRng::from_seed(6);
    let (next, _) = step(&state, &both_move(0, 0), &mut rng);
    assert!(next.sides[1].party[0].status.is_none());
}

#[test]
fn toxic_escalates_and_resets_on_switch() {
    let sp = species("toxed", &[Type::Feral], 50);
    let idle = move_spec("hum", Type::Feral, MoveCategory::Status, 0, 0);
    let mut state = trainer_state(
        vec![
            mote(&sp, 50, vec![idle.clone()]),
            mote(&sp, 50, vec![idle.clone()]),
        ],
        vec![mote(&sp, 50, vec![idle.clone()])],
    );
    state.sides[0].party[0].status = Some(battle::mote::MajorStatus::Toxic { n: 1 });

    let max_hp = u32::from(state.sides[0].party[0].max_hp());
    let mut rng = BattleRng::from_seed(7);

    // Turn 1: n=1 → max/16; turn 2: n=2 → 2·max/16.
    let (after1, events1) = step(&state, &both_move(0, 0), &mut rng);
    let tick1 = events1.iter().find_map(|e| match e {
        BattleEvent::StatusTicked {
            target: 0, damage, ..
        } => Some(u32::from(*damage)),
        _ => None,
    });
    assert_eq!(tick1, Some((max_hp / 16).max(1)));
    let (after2, events2) = step(&after1, &both_move(0, 0), &mut rng);
    let tick2 = events2.iter().find_map(|e| match e {
        BattleEvent::StatusTicked {
            target: 0, damage, ..
        } => Some(u32::from(*damage)),
        _ => None,
    });
    assert_eq!(tick2, Some((max_hp * 2 / 16).max(1)));

    // Switch out and back: counter resets to 1.
    let (after3, _) = step(
        &after2,
        &TurnActions::new(Action::Switch { to: 1 }, Action::Move { slot: 0 }),
        &mut rng,
    );
    assert!(matches!(
        after3.sides[0].party[0].status,
        Some(battle::mote::MajorStatus::Toxic { n: 1 })
    ));
}

#[test]
fn sleep_counts_down_on_action_and_wakes() {
    let sp = species("sleeper", &[Type::Feral], 50);
    let tackle = move_spec("tackle", Type::Feral, MoveCategory::Physical, 40, 0);
    let mut state = trainer_state(
        vec![mote(&sp, 20, vec![tackle.clone()])],
        vec![mote(&sp, 20, vec![tackle.clone()])],
    );
    state.sides[0].party[0].status = Some(battle::mote::MajorStatus::Sleep { turns: 2 });

    let mut rng = BattleRng::from_seed(8);
    let (after1, events1) = step(&state, &both_move(0, 0), &mut rng);
    assert!(
        events1.iter().any(|e| matches!(
            e,
            BattleEvent::ActionLost {
                side: 0,
                status: Ailment::Sleep
            }
        )),
        "turn 1: still lulled"
    );
    let (_, events2) = step(&after1, &both_move(0, 0), &mut rng);
    let cured = position(&events2, |e| {
        matches!(
            e,
            BattleEvent::StatusCured {
                target: 0,
                status: Ailment::Sleep
            }
        )
    })
    .expect("turn 2: wakes");
    let moved = position(&events2, |e| {
        matches!(e, BattleEvent::MoveUsed { side: 0, .. })
    })
    .expect("acts on waking (law v1.1 #5)");
    assert!(cured < moved);
}

#[test]
fn ember_hit_thaws_frozen_target() {
    let sp = species("frozen_solid", &[Type::Feral], 10);
    let pyro = species("pyro", &[Type::Ember], 90);
    let ember_jab = move_spec("ember_jab", Type::Ember, MoveCategory::Special, 40, 0);
    let tackle = move_spec("tackle", Type::Feral, MoveCategory::Physical, 40, 0);
    let mut state = trainer_state(
        vec![mote(&pyro, 20, vec![ember_jab])],
        vec![mote(&sp, 20, vec![tackle])],
    );
    state.sides[1].party[0].status = Some(battle::mote::MajorStatus::Freeze);

    let mut rng = BattleRng::from_seed(9);
    let (next, events) = step(&state, &both_move(0, 0), &mut rng);
    assert!(
        next.sides[1].party[0].status.is_none(),
        "thawed by ember move"
    );
    assert!(events.iter().any(|e| matches!(
        e,
        BattleEvent::StatusCured {
            target: 1,
            status: Ailment::Freeze
        }
    )));
}

#[test]
fn flinch_costs_the_target_its_action() {
    let fast = species("flincher", &[Type::Feral], 200);
    let slow = species("victim", &[Type::Feral], 10);
    let mut fang = move_spec("fang", Type::Feral, MoveCategory::Physical, 30, 0);
    fang.effects = vec![Effect::Flinch { chance: 100 }];
    let tackle = move_spec("tackle", Type::Feral, MoveCategory::Physical, 40, 0);

    let state = trainer_state(
        vec![mote(&fast, 20, vec![fang])],
        vec![mote(&slow, 20, vec![tackle])],
    );
    let mut rng = BattleRng::from_seed(10);
    let (_, events) = step(&state, &both_move(0, 0), &mut rng);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, BattleEvent::Flinched { side: 1 }))
    );
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, BattleEvent::MoveUsed { side: 1, .. })),
        "flinched side never moves"
    );
}

#[test]
fn drain_heals_half_of_damage_dealt() {
    let sp = species("drained", &[Type::Feral], 10);
    let healer = species("healer", &[Type::Bloom], 90);
    let mut chord = move_spec("root_chord", Type::Bloom, MoveCategory::Special, 75, 0);
    chord.effects = vec![Effect::Drain { frac: Frac(1, 2) }];
    let tackle = move_spec("tackle", Type::Feral, MoveCategory::Physical, 40, 0);

    let mut state = trainer_state(
        vec![mote(&healer, 30, vec![chord])],
        vec![mote(&sp, 30, vec![tackle])],
    );
    // Hurt the healer so the drain has room to heal.
    state.sides[0].party[0].hp = 10;

    let mut rng = BattleRng::from_seed(12);
    let (next, events) = step(&state, &both_move(0, 0), &mut rng);
    let dealt = events
        .iter()
        .find_map(|e| match e {
            BattleEvent::DamageDealt {
                target: 1, amount, ..
            } => Some(*amount),
            _ => None,
        })
        .expect("damage dealt");
    let drained = events
        .iter()
        .find_map(|e| match e {
            BattleEvent::Drained { from: 1, amount } => Some(*amount),
            _ => None,
        })
        .expect("drained");
    assert_eq!(u32::from(drained), u32::from(dealt) / 2);
    // Final HP: healed by the drain, then hit by the victim's tackle.
    let counter_hit = events
        .iter()
        .find_map(|e| match e {
            BattleEvent::DamageDealt {
                target: 0, amount, ..
            } => Some(*amount),
            _ => None,
        })
        .expect("victim hit back");
    assert_eq!(next.sides[0].party[0].hp, 10 + drained - counter_hit);
}

#[test]
fn no_pp_falls_back_to_last_resort_with_recoil() {
    let sp = species("exhausted", &[Type::Feral], 50);
    let tackle = move_spec("tackle", Type::Feral, MoveCategory::Physical, 40, 0);
    let mut state = trainer_state(
        vec![mote(&sp, 20, vec![tackle.clone()])],
        vec![mote(&sp, 20, vec![tackle.clone()])],
    );
    state.sides[0].party[0].moves[0].pp = 0;

    let mut rng = BattleRng::from_seed(13);
    let (next, events) = step(&state, &both_move(0, 0), &mut rng);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, BattleEvent::LastResortUsed { side: 0 }))
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e, BattleEvent::Recoiled { side: 0, .. })),
        "last resort recoils ¼ of damage dealt"
    );
    assert_eq!(next.sides[0].party[0].moves[0].pp, 0, "no PP consumed");
}

#[test]
fn wild_catch_emits_rings_and_can_end_battle() {
    let sp = species("catchable", &[Type::Feral], 50);
    let tackle = move_spec("tackle", Type::Feral, MoveCategory::Physical, 40, 0);
    let mut state = wild_state(
        vec![mote(&sp, 20, vec![tackle.clone()])],
        vec![mote(&sp, 20, vec![tackle.clone()])],
    );
    // Weak + asleep target, strong bell → instant catch path (a ≥ 255).
    state.sides[1].party[0].hp = 1;
    state.sides[1].party[0].status = Some(battle::mote::MajorStatus::Sleep { turns: 3 });

    let mut rng = BattleRng::from_seed(14);
    let (next, events) = step(
        &state,
        &TurnActions::new(
            Action::UseBell {
                bell_mod: Frac(4, 1),
            },
            Action::Move { slot: 0 },
        ),
        &mut rng,
    );
    assert!(events.iter().any(|e| matches!(
        e,
        BattleEvent::AttuneAttempt {
            rings: 4,
            caught: true
        }
    )));
    assert_eq!(next.outcome, Some(Outcome::Caught));
}

#[test]
fn bell_fails_in_trainer_battles() {
    let sp = species("owned", &[Type::Feral], 50);
    let tackle = move_spec("tackle", Type::Feral, MoveCategory::Physical, 40, 0);
    let state = trainer_state(
        vec![mote(&sp, 20, vec![tackle.clone()])],
        vec![mote(&sp, 20, vec![tackle.clone()])],
    );
    let mut rng = BattleRng::from_seed(15);
    let (next, events) = step(
        &state,
        &TurnActions::new(
            Action::UseBell {
                bell_mod: Frac(1, 1),
            },
            Action::Move { slot: 0 },
        ),
        &mut rng,
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e, BattleEvent::MoveFailed { side: 0 }))
    );
    assert!(next.outcome.is_none());
}

#[test]
fn faint_awards_exp_and_levels_up() {
    let strong = species("victor", &[Type::Feral], 90);
    let weak = species("fodder", &[Type::Feral], 10);
    let nuke = move_spec("nuke", Type::Feral, MoveCategory::Physical, 250, 0);
    let tackle = move_spec("tackle", Type::Feral, MoveCategory::Physical, 40, 0);

    let mut victor = mote(&strong, 10, vec![nuke]);
    victor.learnset = vec![(11, "tackle".into())];
    let mut state = trainer_state(vec![victor], vec![mote(&weak, 30, vec![tackle])]);
    // Guarantee the KO regardless of level math: fodder is on its last legs.
    state.sides[1].party[0].hp = 1;
    let mut rng = BattleRng::from_seed(16);
    let (next, events) = step(&state, &both_move(0, 0), &mut rng);

    assert!(
        events
            .iter()
            .any(|e| matches!(e, BattleEvent::Fainted { target: 1 }))
    );
    // ΔExp = floor(100·30/7)·1.5 (trainer) = 428·1.5 = 642
    assert!(events.iter().any(|e| matches!(
        e,
        BattleEvent::ExpGained {
            side: 0,
            amount: 642,
            ..
        }
    )));
    assert!(
        events.iter().any(|e| matches!(
            e,
            BattleEvent::LeveledUp {
                side: 0,
                level: 11,
                ..
            }
        )),
        "level 10 → 11 (1000→1331 needs 331 < 642)"
    );
    assert!(events.iter().any(|e| matches!(
        e,
        BattleEvent::MoveLearnable { side: 0, move_id, .. } if move_id.as_str() == "tackle"
    )));
    // EV award: fodder yields 1 atk EV.
    assert_eq!(next.sides[0].party[0].evs.atk, 1);
    assert_eq!(next.outcome, Some(Outcome::Won { winner: 0 }));
}

#[test]
fn run_can_end_wild_battle() {
    let cheetah = species("runner", &[Type::Feral], 200);
    let snail = species("snail", &[Type::Feral], 5);
    let tackle = move_spec("tackle", Type::Feral, MoveCategory::Physical, 40, 0);
    let state = wild_state(
        vec![mote(&cheetah, 20, vec![tackle.clone()])],
        vec![mote(&snail, 20, vec![tackle.clone()])],
    );
    // F = floor(A·32/B) ≥ 256 with these speeds → guaranteed flight.
    let mut rng = BattleRng::from_seed(17);
    let (next, events) = step(
        &state,
        &TurnActions::new(Action::Run, Action::Move { slot: 0 }),
        &mut rng,
    );
    assert!(events.iter().any(|e| matches!(
        e,
        BattleEvent::EscapeAttempt {
            side: 0,
            fled: true
        }
    )));
    assert_eq!(next.outcome, Some(Outcome::Fled { side: 0 }));
}

#[test]
fn confusion_can_cause_deterministic_self_hit() {
    let sp = species("confused", &[Type::Feral], 50);
    let tackle = move_spec("tackle", Type::Feral, MoveCategory::Physical, 40, 0);
    let mut state = trainer_state(
        vec![mote(&sp, 20, vec![tackle.clone()])],
        vec![mote(&sp, 20, vec![tackle.clone()])],
    );
    state.sides[0].active_state.confusion = 4;

    // Find a seed where the 1/3 self-hit fires, then assert the v1.1 #4
    // deterministic damage: floor(floor(2·20/5+2)·40·atk/def/50)+2 with
    // atk=def → floor(10·40/50)+2 = 10.
    let mut found = false;
    for seed in 0..50u64 {
        let mut rng = BattleRng::from_seed(seed);
        let (_, events) = step(&state, &both_move(0, 0), &mut rng);
        if let Some(BattleEvent::HurtItselfInConfusion { damage, .. }) = events
            .iter()
            .find(|e| matches!(e, BattleEvent::HurtItselfInConfusion { .. }))
        {
            assert_eq!(*damage, 10);
            found = true;
            break;
        }
    }
    assert!(found, "self-hit fires within 50 seeds at 1/3 odds");
}

#[test]
fn same_seed_same_actions_byte_identical_event_stream() {
    let a = species("alpha", &[Type::Ember], 70);
    let b = species("beta", &[Type::Tide], 70);
    let singe = move_spec("singe", Type::Ember, MoveCategory::Special, 60, 0);
    let splash = move_spec("splash_hit", Type::Tide, MoveCategory::Special, 60, 0);

    let state = trainer_state(
        vec![mote(&a, 25, vec![singe.clone()])],
        vec![mote(&b, 25, vec![splash.clone()])],
    );

    let run = || {
        let mut rng = BattleRng::from_seed(0xCAFE);
        let mut current = state.clone();
        let mut all_events = Vec::new();
        for _ in 0..50 {
            if current.is_over() {
                break;
            }
            let (next, events) = step(&current, &both_move(0, 0), &mut rng);
            all_events.extend(events);
            current = next;
        }
        ron::to_string(&all_events).expect("serialize")
    };

    assert_eq!(run(), run(), "replay invariant: byte-identical streams");
}

#[test]
fn battle_state_serialization_round_trips() {
    let sp = species("roundtrip", &[Type::Feral, Type::Gale], 50);
    let tackle = move_spec("tackle", Type::Feral, MoveCategory::Physical, 40, 0);
    let state = wild_state(
        vec![mote(&sp, 20, vec![tackle.clone()])],
        vec![mote(&sp, 22, vec![tackle.clone()])],
    );
    let text = ron::to_string(&state).expect("serialize");
    let back: BattleState = ron::from_str(&text).expect("deserialize");
    assert_eq!(back, state);
}

#[test]
fn stat_stage_effects_apply_and_clamp() {
    let sp = species("stager", &[Type::Feral], 50);
    let mut boost = move_spec("crescendo_like", Type::Resonant, MoveCategory::Status, 0, 0);
    boost.target = MoveTarget::User;
    boost.accuracy = 0;
    boost.effects = vec![
        Effect::StatStage {
            target: EffectTarget::User,
            stat: Stat::Spa,
            delta: 1,
            chance: 100,
        },
        Effect::StatStage {
            target: EffectTarget::User,
            stat: Stat::Spe,
            delta: 1,
            chance: 100,
        },
    ];
    let tackle = move_spec("tackle", Type::Feral, MoveCategory::Physical, 10, 0);
    let mut state = trainer_state(
        vec![mote(&sp, 20, vec![boost])],
        vec![mote(&sp, 20, vec![tackle])],
    );

    let mut rng = BattleRng::from_seed(18);
    for _ in 0..7 {
        let (next, _) = step(&state, &both_move(0, 0), &mut rng);
        state = next;
    }
    use battle::stats::StageStat;
    assert_eq!(state.sides[0].active_state.stages.get(StageStat::Spa), 6);
    assert_eq!(state.sides[0].active_state.stages.get(StageStat::Spe), 6);
}

#[test]
fn turn_limit_forces_draw() {
    let sp = species("staller", &[Type::Feral], 50);
    let idle = move_spec("hum", Type::Feral, MoveCategory::Status, 0, 0);
    let mut state = trainer_state(
        vec![mote(&sp, 20, vec![idle.clone()])],
        vec![mote(&sp, 20, vec![idle.clone()])],
    );
    state.turn = 1000;
    let mut rng = BattleRng::from_seed(19);
    let (next, events) = step(&state, &both_move(0, 0), &mut rng);
    assert_eq!(next.outcome, Some(Outcome::Drawn));
    assert!(events.iter().any(|e| matches!(
        e,
        BattleEvent::BattleEnded {
            outcome: Outcome::Drawn
        }
    )));
}

#[test]
fn two_turn_charges_once_commits_slot_and_costs_one_pp() {
    let sp = species("breather", &[Type::Feral], 80);
    let mut big_breath = move_spec("big_breath", Type::Feral, MoveCategory::Physical, 90, 0);
    big_breath.pp = 5;
    big_breath.effects = vec![Effect::TwoTurn {
        charge_text: "draws breath".into(),
    }];
    let tackle = move_spec("tackle", Type::Feral, MoveCategory::Physical, 40, 0);

    let state = trainer_state(
        vec![mote(&sp, 20, vec![big_breath, tackle.clone()])],
        vec![mote(&sp, 20, vec![tackle.clone()])],
    );
    let mut rng = BattleRng::from_seed(21);

    // Turn 1: charge — MoveUsed + ChargeStarted, 1 PP, no damage from us.
    let (mid, events1) = step(&state, &both_move(0, 0), &mut rng);
    assert!(
        events1
            .iter()
            .any(|e| matches!(e, BattleEvent::ChargeStarted { side: 0 }))
    );
    assert!(
        !events1
            .iter()
            .any(|e| matches!(e, BattleEvent::DamageDealt { target: 1, .. })),
        "no strike on the charge turn"
    );
    assert_eq!(mid.sides[0].party[0].moves[0].pp, 4, "1 PP at charge");
    assert_eq!(mid.sides[0].active_state.charging, Some(0));

    // Turn 2: submit the OTHER slot — the committed slot strikes anyway,
    // no second PP cost, no second MoveUsed (doc 02 v1.2 #5).
    let (after, events2) = step(&mid, &both_move(1, 0), &mut rng);
    assert!(
        events2
            .iter()
            .any(|e| matches!(e, BattleEvent::DamageDealt { target: 1, .. })),
        "release strikes"
    );
    assert!(
        !events2
            .iter()
            .any(|e| matches!(e, BattleEvent::MoveUsed { side: 0, .. })),
        "no second MoveUsed on release"
    );
    assert_eq!(after.sides[0].party[0].moves[0].pp, 4, "no second PP cost");
    assert_eq!(
        after.sides[0].party[0].moves[1].pp, tackle.pp,
        "tackle untouched"
    );
    assert_eq!(after.sides[0].active_state.charging, None);
}

#[test]
fn two_turn_switch_cancels_the_charge() {
    let sp = species("breather", &[Type::Feral], 80);
    let mut big_breath = move_spec("big_breath", Type::Feral, MoveCategory::Physical, 90, 0);
    big_breath.effects = vec![Effect::TwoTurn {
        charge_text: "draws breath".into(),
    }];
    let tackle = move_spec("tackle", Type::Feral, MoveCategory::Physical, 40, 0);

    let state = trainer_state(
        vec![
            mote(&sp, 20, vec![big_breath]),
            mote(&sp, 20, vec![tackle.clone()]),
        ],
        vec![mote(&sp, 20, vec![tackle.clone()])],
    );
    let mut rng = BattleRng::from_seed(22);
    let (mid, _) = step(&state, &both_move(0, 0), &mut rng);
    assert_eq!(mid.sides[0].active_state.charging, Some(0));

    let (after, events) = step(
        &mid,
        &TurnActions::new(Action::Switch { to: 1 }, Action::Move { slot: 0 }),
        &mut rng,
    );
    assert_eq!(
        after.sides[0].active_state.charging, None,
        "volatiles clear on exit"
    );
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, BattleEvent::DamageDealt { target: 1, .. })),
        "no phantom strike after switching out"
    );
}

#[test]
fn force_switch_does_not_eclipse_a_faint() {
    let strong = species("dragger", &[Type::Feral], 90);
    let weak = species("dragged", &[Type::Feral], 30);
    let mut roar_smash = move_spec("roar_smash", Type::Feral, MoveCategory::Physical, 120, 0);
    roar_smash.effects = vec![Effect::ForceSwitch];
    let tackle = move_spec("tackle", Type::Feral, MoveCategory::Physical, 40, 0);

    let mut state = trainer_state(
        vec![mote(&strong, 30, vec![roar_smash])],
        vec![
            mote(&weak, 20, vec![tackle.clone()]),
            mote(&weak, 20, vec![tackle.clone()]),
        ],
    );
    state.sides[1].party[0].hp = 1;

    let mut rng = BattleRng::from_seed(23);
    let (next, events) = step(&state, &both_move(0, 0), &mut rng);

    let faint = position(&events, |e| matches!(e, BattleEvent::Fainted { target: 1 }))
        .expect("the KO is visible in the stream");
    assert!(
        events
            .iter()
            .any(|e| matches!(e, BattleEvent::ExpGained { side: 0, .. })),
        "the victor is awarded"
    );
    // The replacement arrives via auto-replace, after the faint.
    let switched = position(&events, |e| {
        matches!(e, BattleEvent::SwitchedIn { side: 1, .. })
    })
    .expect("replacement arrives");
    assert!(faint < switched);
    assert_eq!(next.sides[1].active, 1);
}

#[test]
fn self_switch_after_lethal_recoil_faints_the_user() {
    let frail = species("kamikaze", &[Type::Feral], 90);
    let tank = species("tank", &[Type::Feral], 30);
    let mut crash_out = move_spec("crash_out", Type::Feral, MoveCategory::Physical, 80, 0);
    crash_out.effects = vec![Effect::Recoil { frac: Frac(1, 1) }, Effect::SelfSwitch];
    let tackle = move_spec("tackle", Type::Feral, MoveCategory::Physical, 40, 0);

    let mut state = trainer_state(
        vec![
            mote(&frail, 25, vec![crash_out]),
            mote(&frail, 25, vec![tackle.clone()]),
        ],
        vec![mote(&tank, 40, vec![tackle.clone()])],
    );
    state.sides[0].party[0].hp = 1;

    let mut rng = BattleRng::from_seed(24);
    let (_, events) = step(&state, &both_move(0, 0), &mut rng);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, BattleEvent::Fainted { target: 0 })),
        "recoil faint is visible, not hidden by the self-switch"
    );
}

#[test]
fn failed_escape_consumes_the_action_and_escalates() {
    let snail = species("snail", &[Type::Feral], 1);
    let cheetah = species("cheetah", &[Type::Feral], 250);
    let tackle = move_spec("tackle", Type::Feral, MoveCategory::Physical, 40, 0);
    let state = wild_state(
        vec![mote(&snail, 10, vec![tackle.clone()])],
        vec![mote(&cheetah, 40, vec![tackle.clone()])],
    );
    // A = 5 (snail spe at L10), B = 205 (cheetah at L40):
    // F = floor(5·32/205) + 30·0 = 0 → guaranteed failure.
    let mut rng = BattleRng::from_seed(25);
    let (next, events) = step(
        &state,
        &TurnActions::new(Action::Run, Action::Move { slot: 0 }),
        &mut rng,
    );
    assert!(events.iter().any(|e| matches!(
        e,
        BattleEvent::EscapeAttempt {
            side: 0,
            fled: false
        }
    )));
    assert_eq!(next.escape_attempts, 1, "doc 02 §12: +30 per prior failure");
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, BattleEvent::MoveUsed { side: 0, .. })),
        "a failed escape consumes the action (v1.1 #1a)"
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e, BattleEvent::MoveUsed { side: 1, .. })),
        "the wild still acts"
    );
    // (The wild may well KO the snail afterwards — irrelevant here.)
}

#[test]
fn flurry_makes_frost_moves_skip_the_accuracy_roll() {
    let caster = species("rimecaller", &[Type::Feral], 80);
    let target = species("dodger", &[Type::Feral], 40);
    // 30% accuracy frost move: without flurry it usually misses; under
    // flurry it must never miss (doc 02 §7 / v1.2 #9).
    let mut rime_dart = move_spec("rime_dart", Type::Frost, MoveCategory::Special, 40, 0);
    rime_dart.accuracy = 30;
    let tackle = move_spec("tackle", Type::Feral, MoveCategory::Physical, 40, 0);

    let mut flurry_state = trainer_state(
        vec![mote(&caster, 20, vec![rime_dart.clone()])],
        vec![mote(&target, 20, vec![tackle.clone()])],
    );
    flurry_state.weather = Some((undersong_core::moves::WeatherKind::Flurry, 5));

    let mut misses_under_flurry = 0;
    let mut misses_dry = 0;
    for seed in 100..150u64 {
        let mut rng = BattleRng::from_seed(seed);
        let (_, events) = step(&flurry_state, &both_move(0, 0), &mut rng);
        if events
            .iter()
            .any(|e| matches!(e, BattleEvent::MoveMissed { side: 0 }))
        {
            misses_under_flurry += 1;
        }

        let mut dry = flurry_state.clone();
        dry.weather = None;
        let mut rng = BattleRng::from_seed(seed);
        let (_, events) = step(&dry, &both_move(0, 0), &mut rng);
        if events
            .iter()
            .any(|e| matches!(e, BattleEvent::MoveMissed { side: 0 }))
        {
            misses_dry += 1;
        }
    }
    assert_eq!(misses_under_flurry, 0, "frost never misses in flurry");
    assert!(
        misses_dry > 10,
        "the 30%-accuracy control actually misses dry"
    );
}
