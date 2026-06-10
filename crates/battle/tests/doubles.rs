//! Doubles-format tests (doc 02 v1.5 #2, §10): declared targets,
//! retarget/fizzle, understudy, soloist/chorister format reads, and the
//! §9 exp split.

use battle::abilities::Ability;
use battle::{
    Action, BattleEvent, BattleKind, BattleMote, BattleState, MoteBuilder, PositionAction,
    TurnActions, step,
};
use undersong_core::chart::TypeChart;
use undersong_core::moves::{Effect, Frac, MoveCategory, MoveFlags, MoveSpec, MoveTarget};
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

fn species(id: &str, spe: u16) -> SpeciesSpec {
    SpeciesSpec {
        id: id.into(),
        name_key: format!("motif.{id}"),
        types: vec![Type::Feral],
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

/// Sure-hit physical move (accuracy 0 = never misses).
fn move_spec(id: &str, power: u16) -> MoveSpec {
    MoveSpec {
        id: id.into(),
        name_key: format!("move.{id}"),
        r#type: Type::Feral,
        category: MoveCategory::Physical,
        power,
        accuracy: 0,
        pp: 20,
        priority: 0,
        target: MoveTarget::Foe,
        flags: MoveFlags::default(),
        effects: vec![],
    }
}

fn mote(spec: &SpeciesSpec, moves: Vec<MoveSpec>) -> BattleMote {
    MoteBuilder::new(spec, 20).moves(moves).build()
}

fn declare(side: u8, position: u8, action: Action, target: u8) -> PositionAction {
    PositionAction {
        side,
        position,
        action,
        target_position: target,
    }
}

fn move_at(side: u8, position: u8, target: u8) -> PositionAction {
    declare(side, position, Action::Move { slot: 0 }, target)
}

#[test]
fn new_double_fields_first_two_conscious() {
    let sp = species("bench_check", 50);
    let jab = move_spec("jab", 40);
    let mut fainted = mote(&sp, vec![jab.clone()]);
    fainted.hp = 0;
    let state = BattleState::new_double(
        BattleKind::Trainer,
        vec![
            fainted,
            mote(&sp, vec![jab.clone()]),
            mote(&sp, vec![jab.clone()]),
        ],
        vec![mote(&sp, vec![jab.clone()]), mote(&sp, vec![jab])],
        full_neutral_chart(),
    );
    let fielded: Vec<u8> = state.sides[0]
        .positions
        .iter()
        .map(|p| p.party_index)
        .collect();
    assert_eq!(fielded, vec![1, 2], "skips the fainted lead");
    assert_eq!(state.sides[1].position_count(), 2);
}

#[test]
#[should_panic(expected = "wild battles are always Single")]
fn wild_doubles_do_not_exist_at_launch() {
    let sp = species("wild_check", 50);
    let jab = move_spec("jab", 40);
    let _ = BattleState::new_double(
        BattleKind::Wild,
        vec![mote(&sp, vec![jab.clone()])],
        vec![mote(&sp, vec![jab])],
        full_neutral_chart(),
    );
}

#[test]
fn understudy_triggers_on_ally_faint() {
    let frail = species("frail", 30);
    let study = species("study", 40);
    let striker = species("striker", 90);
    let nuke = move_spec("nuke", 250);
    let jab = move_spec("jab", 40);

    let mut doomed = mote(&frail, vec![jab.clone()]);
    doomed.hp = 1;
    let survivor = MoteBuilder::new(&study, 20)
        .moves(vec![jab.clone()])
        .ability(Ability::Understudy)
        .build();
    let state = BattleState::new_double(
        BattleKind::Trainer,
        vec![doomed, survivor],
        vec![mote(&striker, vec![nuke]), mote(&striker, vec![jab])],
        full_neutral_chart(),
    );
    let mut rng = BattleRng::from_seed(31);
    let (next, events) = step(
        &state,
        &TurnActions::doubles(vec![
            declare(0, 0, Action::None, 0),
            declare(0, 1, Action::None, 0),
            move_at(1, 0, 0), // nuke the understudy's ally
            declare(1, 1, Action::None, 0),
        ]),
        &mut rng,
    );

    let faint = events
        .iter()
        .position(|e| matches!(e, BattleEvent::Fainted { target: 0, slot: 0 }))
        .expect("ally fainted");
    let note = events
        .iter()
        .position(|e| {
            matches!(
                e,
                BattleEvent::AbilityNote {
                    side: 0,
                    ability: Ability::Understudy
                }
            )
        })
        .expect("understudy announced");
    assert!(faint < note, "boost follows the faint");
    for stat in [Stat::Atk, Stat::Spa] {
        assert!(
            events.iter().any(|e| matches!(
                e,
                BattleEvent::StatStageChanged {
                    target: 0,
                    slot: 1,
                    stat: s,
                    delta: 1,
                    new_stage: 1,
                } if *s == stat
            )),
            "{stat:?} +1 on the survivor"
        );
    }
    use battle::stats::StageStat;
    let stages = &next.sides[0].positions[1].state.stages;
    assert_eq!(stages.get(StageStat::Atk), 1);
    assert_eq!(stages.get(StageStat::Spa), 1);
}

#[test]
fn retarget_hits_the_survivor_when_declared_target_fainted() {
    let fast = species("fast", 90);
    let slow = species("slow", 50);
    let foe_sp = species("foe", 40);
    let nuke = move_spec("nuke", 250);
    let jab = move_spec("jab", 40);

    let mut frail_foe = mote(&foe_sp, vec![jab.clone()]);
    frail_foe.hp = 1;
    let state = BattleState::new_double(
        BattleKind::Trainer,
        vec![mote(&fast, vec![nuke]), mote(&slow, vec![jab.clone()])],
        vec![frail_foe, mote(&foe_sp, vec![jab])],
        full_neutral_chart(),
    );
    let mut rng = BattleRng::from_seed(32);
    let (_, events) = step(
        &state,
        &TurnActions::doubles(vec![
            move_at(0, 0, 0), // KOs foe slot 0
            move_at(0, 1, 0), // declared at slot 0 — must retarget to 1
            declare(1, 0, Action::None, 0),
            declare(1, 1, Action::None, 0),
        ]),
        &mut rng,
    );

    let faint = events
        .iter()
        .position(|e| matches!(e, BattleEvent::Fainted { target: 1, slot: 0 }))
        .expect("declared target fainted first");
    let retargeted = events
        .iter()
        .position(|e| {
            matches!(
                e,
                BattleEvent::DamageDealt {
                    target: 1,
                    target_slot: 1,
                    ..
                }
            )
        })
        .expect("second strike lands on the survivor");
    assert!(faint < retargeted, "retarget happens after the KO");
}

#[test]
fn fizzle_when_both_foes_down_mid_turn() {
    let fast = species("fast", 200);
    let slow = species("slow", 50);
    let kamikaze_sp = species("kamikaze", 150);
    let laggard = species("laggard", 10);
    let nuke = move_spec("nuke", 250);
    let jab = move_spec("jab", 40);
    let mut crash = move_spec("crash", 60);
    crash.effects = vec![Effect::Recoil { frac: Frac(1, 1) }];

    let mut frail_foe = mote(&laggard, vec![jab.clone()]);
    frail_foe.hp = 1;
    let mut kamikaze = mote(&kamikaze_sp, vec![crash]);
    kamikaze.hp = 1; // its own recoil KOs it
    let state = BattleState::new_double(
        BattleKind::Trainer,
        vec![mote(&fast, vec![nuke]), mote(&slow, vec![jab.clone()])],
        // Third member benched: the battle must NOT end mid-turn.
        vec![frail_foe, kamikaze, mote(&laggard, vec![jab])],
        full_neutral_chart(),
    );
    let mut rng = BattleRng::from_seed(33);
    let (next, events) = step(
        &state,
        &TurnActions::doubles(vec![
            move_at(0, 0, 0), // fastest: KOs foe slot 0
            move_at(0, 1, 0), // slowest: by then both foe slots are down
            declare(1, 0, Action::None, 0),
            move_at(1, 1, 0), // recoil self-KO before our slow mover acts
        ]),
        &mut rng,
    );

    // Our slow mover still announces its move, then it fizzles.
    let slow_move = events
        .iter()
        .position(|e| {
            matches!(
                e,
                BattleEvent::MoveUsed {
                    side: 0,
                    slot: 1,
                    ..
                }
            )
        })
        .expect("slow mover acts");
    let fizzle = events
        .iter()
        .skip(slow_move)
        .position(|e| matches!(e, BattleEvent::MoveFailed { side: 0 }))
        .expect("move fizzles with no surviving target");
    assert!(fizzle > 0, "fizzle follows the MoveUsed");
    assert!(
        !events
            .iter()
            .skip(slow_move)
            .any(|e| matches!(e, BattleEvent::DamageDealt { target: 1, .. })),
        "no damage lands after both foes are down"
    );
    assert!(
        next.outcome.is_none(),
        "the foe bench keeps the battle alive"
    );
    assert!(
        events.iter().any(|e| matches!(
            e,
            BattleEvent::SwitchedIn {
                side: 1,
                slot: 2,
                ..
            }
        )),
        "end-of-turn auto-replace fields the bench Mote"
    );
}

/// Runs one scripted strike and returns the damage dealt to foe slot 0.
fn first_strike_damage(double: bool, ability: Ability) -> u32 {
    let attacker_sp = species("atk", 90);
    let defender_sp = species("def", 40);
    let blow = move_spec("blow", 80);
    let jab = move_spec("jab", 40);
    let attacker = MoteBuilder::new(&attacker_sp, 20)
        .moves(vec![blow])
        .ability(ability)
        .build();
    let defender = mote(&defender_sp, vec![jab]);
    let chart = full_neutral_chart();
    let mut rng = BattleRng::from_seed(34);
    let events = if double {
        let state =
            BattleState::new_double(BattleKind::Trainer, vec![attacker], vec![defender], chart);
        let (_, events) = step(
            &state,
            &TurnActions::doubles(vec![move_at(0, 0, 0), declare(1, 0, Action::None, 0)]),
            &mut rng,
        );
        events
    } else {
        let state = BattleState::new(BattleKind::Trainer, vec![attacker], vec![defender], chart);
        let (_, events) = step(
            &state,
            &TurnActions::new(Action::Move { slot: 0 }, Action::None),
            &mut rng,
        );
        events
    };
    events
        .iter()
        .find_map(|e| match e {
            BattleEvent::DamageDealt {
                target: 1, amount, ..
            } => Some(u32::from(*amount)),
            _ => None,
        })
        .expect("strike landed")
}

#[test]
fn chorister_boosts_in_doubles_only() {
    let base_single = first_strike_damage(false, Ability::None);
    let base_double = first_strike_damage(true, Ability::None);
    assert_eq!(
        first_strike_damage(false, Ability::Chorister),
        base_single,
        "chorister is inert in singles"
    );
    assert!(
        first_strike_damage(true, Ability::Chorister) > base_double,
        "chorister ×1.2 in doubles"
    );
}

#[test]
fn soloist_softens_to_nine_tenths_in_doubles() {
    let base_single = first_strike_damage(false, Ability::None);
    let base_double = first_strike_damage(true, Ability::None);
    assert!(
        first_strike_damage(false, Ability::Soloist) > base_single,
        "soloist ×1.3 in singles"
    );
    assert!(
        first_strike_damage(true, Ability::Soloist) < base_double,
        "soloist ×0.9 in doubles"
    );
}

#[test]
fn exp_splits_evenly_between_conscious_player_positions() {
    let strong = species("victor", 90);
    let foe_sp = species("fodder", 10);
    let nuke = move_spec("nuke", 250);
    let jab = move_spec("jab", 40);

    let mut fodder = mote(&foe_sp, vec![jab.clone()]);
    fodder.hp = 1;
    let state = BattleState::new_double(
        BattleKind::Trainer,
        vec![mote(&strong, vec![nuke]), mote(&strong, vec![jab])],
        vec![fodder],
        full_neutral_chart(),
    );
    let mut rng = BattleRng::from_seed(35);
    let (next, events) = step(
        &state,
        &TurnActions::doubles(vec![
            move_at(0, 0, 0),
            declare(0, 1, Action::None, 0),
            declare(1, 0, Action::None, 0),
        ]),
        &mut rng,
    );

    // §9: floor(100·20/7) = 285 → ÷2 participants = 142 → ×1.5 trainer = 213.
    let expected = battle::exp::exp_gain(100, 20, 2, true, false);
    assert_eq!(expected, 213, "hand vector");
    for slot in [0u8, 1u8] {
        assert!(
            events.iter().any(|e| matches!(
                e,
                BattleEvent::ExpGained {
                    side: 0,
                    slot: s,
                    amount,
                } if *s == slot && *amount == expected
            )),
            "position {slot} gets the even split"
        );
    }
    // Both participants take the full EV yield (1 atk each).
    assert_eq!(next.sides[0].party[0].evs.atk, 1);
    assert_eq!(next.sides[0].party[1].evs.atk, 1);
}

#[test]
fn choose_doubles_t1_picks_target_it_can_hurt_most() {
    let attacker_sp = species("brain", 60);
    let tank_sp = {
        let mut sp = species("tank", 40);
        sp.base_stats.def = 250;
        sp
    };
    let frail_sp = species("paper", 40);
    let blow = move_spec("blow", 80);
    let jab = move_spec("jab", 40);
    let state = BattleState::new_double(
        BattleKind::Trainer,
        vec![
            mote(&attacker_sp, vec![blow]),
            mote(&attacker_sp, vec![jab.clone()]),
        ],
        vec![
            mote(&tank_sp, vec![jab.clone()]),
            mote(&frail_sp, vec![jab]),
        ],
        full_neutral_chart(),
    );
    let mut rng = BattleRng::from_seed(36);
    let (action, target) =
        battle::ai::choose_doubles(battle::ai::AiTier::T1, &state, 0, 0, &mut rng);
    assert_eq!(action, Action::Move { slot: 0 });
    assert_eq!(target, 1, "T1 aims at the squishier foe slot");
}
