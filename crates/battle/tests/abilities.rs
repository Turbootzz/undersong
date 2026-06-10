//! Ability suite (doc 02 §10) — every launch-24 behavior pinned by a
//! headless test. Fixtures use minimal hand-built species.

use battle::abilities::{Ability, HeldItem};
use battle::{Action, BattleEvent, BattleKind, BattleState, MoteBuilder, TurnActions, step};
use undersong_core::chart::TypeChart;
use undersong_core::ids::SpeciesId;
use undersong_core::moves::{
    Effect, Frac, MoveCategory, MoveFlags, MoveSpec, MoveTarget, WeatherKind,
};
use undersong_core::rng::BattleRng;
use undersong_core::species::{GrowthCurve, SpeciesSpec, StatSpread};
use undersong_core::types::Type;

fn chart() -> TypeChart {
    chart_with(&[])
}

/// Neutral chart with overrides: (attacker, defender, "Double").
fn chart_with(overrides: &[(Type, Type, &str)]) -> TypeChart {
    let mut rows = String::from("TypeChart(entries: {");
    for attacker in Type::ALL {
        rows.push_str(&format!("{attacker:?}: {{"));
        for defender in Type::ALL {
            let eff = overrides
                .iter()
                .find(|(a, d, _)| *a == attacker && *d == defender)
                .map(|(_, _, e)| *e)
                .unwrap_or("Neutral");
            rows.push_str(&format!("{defender:?}: {eff},"));
        }
        rows.push_str("},");
    }
    rows.push_str("})");
    ron::from_str(&rows).expect("chart")
}

fn spread(v: u16) -> StatSpread {
    StatSpread {
        hp: v,
        atk: v,
        def: v,
        spa: v,
        spd: v,
        spe: v,
    }
}

fn species(id: &str, ty: Type, spe: u16) -> SpeciesSpec {
    SpeciesSpec {
        id: SpeciesId::from(id),
        name_key: format!("motif.{id}"),
        types: vec![ty],
        base_stats: StatSpread { spe, ..spread(80) },
        catch_rate: 100,
        base_exp_yield: 60,
        ev_yield: std::collections::BTreeMap::new().into(),
        growth_curve: GrowthCurve::MediumFast,
        learnset: vec![],
        abilities: vec![],
        hidden_ability: None,
        tags: vec![],
    }
}

fn strike(id: &str, ty: Type, power: u16, contact: bool, sound: bool) -> MoveSpec {
    MoveSpec {
        id: id.into(),
        name_key: format!("move.{id}"),
        r#type: ty,
        category: MoveCategory::Physical,
        power,
        accuracy: 100,
        pp: 30,
        priority: 0,
        target: MoveTarget::Foe,
        flags: MoveFlags {
            contact,
            sound,
            ..Default::default()
        },
        effects: vec![],
    }
}

struct Arena {
    state: BattleState,
    rng: BattleRng,
}

fn arena(attacker_ability: Ability, defender_ability: Ability) -> Arena {
    arena_with(attacker_ability, defender_ability, 60, 40, |m| m, |m| m)
}

fn arena_with(
    attacker_ability: Ability,
    defender_ability: Ability,
    spe_a: u16,
    spe_b: u16,
    moves_a: impl Fn(Vec<MoveSpec>) -> Vec<MoveSpec>,
    moves_b: impl Fn(Vec<MoveSpec>) -> Vec<MoveSpec>,
) -> Arena {
    let species_a = species("aaa", Type::Feral, spe_a);
    let species_b = species("bbb", Type::Feral, spe_b);
    let a = MoteBuilder::new(&species_a, 30)
        .moves(moves_a(vec![
            strike("claw", Type::Feral, 50, true, false),
            strike("shout", Type::Feral, 50, false, true),
        ]))
        .ability(attacker_ability)
        .build();
    let b = MoteBuilder::new(&species_b, 30)
        .moves(moves_b(vec![strike(
            "counter_claw",
            Type::Feral,
            50,
            true,
            false,
        )]))
        .ability(defender_ability)
        .build();
    Arena {
        state: BattleState::new(BattleKind::Trainer, vec![a], vec![b], chart()),
        rng: BattleRng::from_seed(0xAB1E),
    }
}

fn one_turn(arena: &mut Arena, my: Action, foe: Action) -> Vec<BattleEvent> {
    let (next, events) = step(&arena.state, &TurnActions::new(my, foe), &mut arena.rng);
    arena.state = next;
    events
}

fn damage_to(events: &[BattleEvent], target: u8) -> u32 {
    events
        .iter()
        .filter_map(|e| match e {
            BattleEvent::DamageDealt {
                target: t, amount, ..
            } if *t == target => Some(u32::from(*amount)),
            _ => None,
        })
        .sum()
}

#[test]
fn amplify_boosts_sound_only() {
    let mut plain = arena(Ability::None, Ability::None);
    let base = damage_to(
        &one_turn(&mut plain, Action::Move { slot: 1 }, Action::None),
        1,
    );
    let mut amped = arena(Ability::Amplify, Ability::None);
    let boosted = damage_to(
        &one_turn(&mut amped, Action::Move { slot: 1 }, Action::None),
        1,
    );
    assert!(boosted > base, "amplify sound: {boosted} > {base}");

    let mut amped_contact = arena(Ability::Amplify, Ability::None);
    let contact = damage_to(
        &one_turn(&mut amped_contact, Action::Move { slot: 0 }, Action::None),
        1,
    );
    let mut plain_contact = arena(Ability::None, Ability::None);
    let contact_base = damage_to(
        &one_turn(&mut plain_contact, Action::Move { slot: 0 }, Action::None),
        1,
    );
    assert_eq!(contact, contact_base, "non-sound untouched");
}

#[test]
fn damper_blanks_sound_moves() {
    let mut fight = arena(Ability::None, Ability::Damper);
    let events = one_turn(&mut fight, Action::Move { slot: 1 }, Action::None);
    assert_eq!(damage_to(&events, 1), 0);
    assert!(events.iter().any(|e| matches!(
        e,
        BattleEvent::AbilityNote {
            ability: Ability::Damper,
            ..
        }
    )));
}

#[test]
fn floating_blanks_stone_moves() {
    let mut fight = arena_with(
        Ability::None,
        Ability::Floating,
        60,
        40,
        |_| vec![strike("rock", Type::Stone, 50, false, false)],
        |m| m,
    );
    let events = one_turn(&mut fight, Action::Move { slot: 0 }, Action::None);
    assert_eq!(damage_to(&events, 1), 0);
}

#[test]
fn live_wire_can_paralyze_contacters() {
    // 30% on contact: run turns until it procs (deterministic seed).
    let mut fight = arena(Ability::None, Ability::LiveWire);
    let mut paralyzed = false;
    for _ in 0..20 {
        let events = one_turn(&mut fight, Action::Move { slot: 0 }, Action::None);
        if events.iter().any(|e| {
            matches!(
                e,
                BattleEvent::StatusApplied {
                    target: 0,
                    status: undersong_core::moves::Ailment::Paralysis
                }
            )
        }) {
            paralyzed = true;
            break;
        }
        if fight.state.outcome.is_some() {
            break;
        }
    }
    assert!(paralyzed, "live_wire procs within 20 contact turns");
}

#[test]
fn thorn_coat_recoils_contacters_for_an_eighth() {
    let mut fight = arena(Ability::None, Ability::ThornCoat);
    let max_hp = fight.state.sides[0].active_mote().max_hp();
    let events = one_turn(&mut fight, Action::Move { slot: 0 }, Action::None);
    let recoil: u32 = events
        .iter()
        .filter_map(|e| match e {
            BattleEvent::Recoiled { side: 0, amount } => Some(u32::from(*amount)),
            _ => None,
        })
        .sum();
    assert_eq!(recoil, u32::from(max_hp) / 8);
}

#[test]
fn weather_callers_set_their_weather_on_entry() {
    for (ability, expected) in [
        (Ability::HeatHaze, WeatherKind::Heatwave),
        (Ability::RainCaller, WeatherKind::Downpour),
        (Ability::FlurryCaller, WeatherKind::Flurry),
        (Ability::DustCaller, WeatherKind::Dustchord),
    ] {
        let mut fight = arena(ability, Ability::None);
        one_turn(&mut fight, Action::None, Action::None);
        assert_eq!(
            fight.state.weather.map(|(k, _)| k),
            Some(expected),
            "{ability:?}"
        );
    }
}

#[test]
fn dissonance_drops_foe_attack_on_entry() {
    let mut fight = arena(Ability::Dissonance, Ability::None);
    let events = one_turn(&mut fight, Action::None, Action::None);
    assert!(events.iter().any(|e| matches!(
        e,
        BattleEvent::StatStageChanged {
            target: 1,
            stat: undersong_core::stats::Stat::Atk,
            delta: -1,
            ..
        }
    )));
}

#[test]
fn stage_fright_boosts_speed_once_per_battle() {
    let mut fight = arena(Ability::StageFright, Ability::None);
    let events = one_turn(&mut fight, Action::None, Action::None);
    let boosts = events
        .iter()
        .filter(|e| {
            matches!(
                e,
                BattleEvent::StatStageChanged {
                    target: 0,
                    stat: undersong_core::stats::Stat::Spe,
                    delta: 1,
                    ..
                }
            )
        })
        .count();
    assert_eq!(boosts, 1);
    assert!(fight.state.sides[0].active_mote().entry_boosted);
}

#[test]
fn thick_hide_never_takes_crits() {
    // High-crit move spam vs thick_hide: zero crit events in 60 turns.
    let mut fight = arena_with(
        Ability::None,
        Ability::ThickHide,
        60,
        40,
        |_| {
            let mut spec = strike("razor", Type::Feral, 10, true, false);
            spec.flags.high_crit = true;
            vec![spec]
        },
        |m| m,
    );
    for _ in 0..60 {
        let events = one_turn(&mut fight, Action::Move { slot: 0 }, Action::None);
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, BattleEvent::DamageDealt { crit: true, .. })),
            "thick_hide blocked the crit"
        );
        if fight.state.outcome.is_some() {
            break;
        }
    }
}

#[test]
fn metronome_soul_ignores_paralysis_speed_penalty_and_drops() {
    let species_fast = species("fast", Type::Feral, 60);
    let mut subject = MoteBuilder::new(&species_fast, 30)
        .moves(vec![strike("claw", Type::Feral, 50, true, false)])
        .ability(Ability::MetronomeSoul)
        .build();
    subject.status = Some(battle::mote::MajorStatus::Paralysis);
    let species_slow = species("slow", Type::Feral, 55);
    let foe = MoteBuilder::new(&species_slow, 30)
        .moves(vec![strike("claw", Type::Feral, 50, true, false)])
        .build();
    let state = BattleState::new(BattleKind::Trainer, vec![subject], vec![foe], chart());
    let mut rng = BattleRng::from_seed(3);
    let (_, events) = step(
        &state,
        &TurnActions::new(Action::Move { slot: 0 }, Action::Move { slot: 0 }),
        &mut rng,
    );
    // The paralyzed metronome_soul holder still outspeeds: its MoveUsed
    // appears before the foe's.
    let first_mover = events.iter().find_map(|e| match e {
        BattleEvent::MoveUsed { side, .. } => Some(*side),
        _ => None,
    });
    assert_eq!(first_mover, Some(0));
}

#[test]
fn vigor_is_immune_to_sleep() {
    let lull = MoveSpec {
        effects: vec![Effect::Status {
            ailment: undersong_core::moves::Ailment::Sleep,
            chance: 100,
        }],
        category: MoveCategory::Status,
        power: 0,
        ..strike("lullaby", Type::Resonant, 0, false, true)
    };
    let mut fight = arena_with(
        Ability::None,
        Ability::Vigor,
        60,
        40,
        |_| vec![lull.clone()],
        |m| m,
    );
    let events = one_turn(&mut fight, Action::Move { slot: 0 }, Action::None);
    assert!(!events.iter().any(|e| matches!(
        e,
        BattleEvent::StatusApplied {
            target: 1,
            status: undersong_core::moves::Ailment::Sleep
        }
    )));
}

#[test]
fn iron_ear_cannot_flinch() {
    let flincher = MoveSpec {
        effects: vec![Effect::Flinch { chance: 100 }],
        ..strike("slam", Type::Feral, 30, true, false)
    };
    let mut fight = arena_with(
        Ability::None,
        Ability::IronEar,
        60,
        40,
        |_| vec![flincher.clone()],
        |m| m,
    );
    one_turn(
        &mut fight,
        Action::Move { slot: 0 },
        Action::Move { slot: 0 },
    );
    assert!(!fight.state.sides[1].active_state.flinched);
}

#[test]
fn encore_heart_heals_in_weather() {
    let mut fight = arena(Ability::RainCaller, Ability::None);
    // Burn some HP first so there is room to heal.
    fight.state.sides[0].active_mote_mut().take_damage(30);
    let events = one_turn(&mut fight, Action::None, Action::None);
    let _ = events;
    let mut fight2 = arena(Ability::EncoreHeart, Ability::None);
    fight2.state.weather = Some((WeatherKind::Downpour, 5));
    fight2.state.sides[0].active_mote_mut().take_damage(30);
    let events = one_turn(&mut fight2, Action::None, Action::None);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, BattleEvent::Healed { target: 0, .. }))
    );
}

#[test]
fn crescendo_kicks_in_below_a_third() {
    let ember_species = species("emb", Type::Ember, 60);
    let target_species = species("tgt", Type::Feral, 40);
    let build = |hp_fraction_low: bool| {
        let mut attacker = MoteBuilder::new(&ember_species, 30)
            .moves(vec![strike("flame", Type::Ember, 50, false, false)])
            .ability(Ability::CrescendoEmber)
            .build();
        if hp_fraction_low {
            let max = attacker.max_hp();
            attacker.take_damage(u32::from(max) * 3 / 4);
        }
        let defender = MoteBuilder::new(&target_species, 30)
            .moves(vec![strike("claw", Type::Feral, 50, true, false)])
            .build();
        BattleState::new(BattleKind::Trainer, vec![attacker], vec![defender], chart())
    };
    let mut rng_high = BattleRng::from_seed(9);
    let (_, events_high) = step(
        &build(false),
        &TurnActions::new(Action::Move { slot: 0 }, Action::None),
        &mut rng_high,
    );
    let mut rng_low = BattleRng::from_seed(9);
    let (_, events_low) = step(
        &build(true),
        &TurnActions::new(Action::Move { slot: 0 }, Action::None),
        &mut rng_low,
    );
    assert!(
        damage_to(&events_low, 1) > damage_to(&events_high, 1),
        "crescendo at low HP out-damages full HP (same seed)"
    );
}

#[test]
fn tuning_fork_strikes_feral_as_resonant_with_a_boost() {
    // Chart where resonant hits feral-type defenders at 2× while feral
    // hits at 1× — the shifted type must use the resonant row.
    let custom_chart = chart_with(&[(Type::Resonant, Type::Feral, "Double")]);

    let attacker_species = species("fork", Type::Gale, 60);
    let defender_species = species("tgtf", Type::Feral, 40);
    let attacker = MoteBuilder::new(&attacker_species, 30)
        .moves(vec![strike("strum", Type::Feral, 50, false, false)])
        .ability(Ability::TuningFork)
        .build();
    let defender = MoteBuilder::new(&defender_species, 30)
        .moves(vec![strike("claw", Type::Feral, 50, true, false)])
        .build();
    let state = BattleState::new(
        BattleKind::Trainer,
        vec![attacker],
        vec![defender],
        custom_chart,
    );
    let mut rng = BattleRng::from_seed(4);
    let (_, events) = step(
        &state,
        &TurnActions::new(Action::Move { slot: 0 }, Action::None),
        &mut rng,
    );
    assert!(events.iter().any(|e| matches!(
        e,
        BattleEvent::DamageDealt {
            effectiveness: undersong_core::types::Eff::Double,
            ..
        }
    )));
}

#[test]
fn oran_chime_triggers_once_at_half() {
    let holder_species = species("hold", Type::Feral, 40);
    let striker_species = species("hit", Type::Feral, 60);
    let holder = {
        let mut mote = MoteBuilder::new(&holder_species, 30)
            .moves(vec![strike("claw", Type::Feral, 50, true, false)])
            .held(HeldItem::OranChime { used: false })
            .build();
        let max = mote.max_hp();
        mote.take_damage(u32::from(max) / 2 - 5); // just above half
        mote
    };
    let striker = MoteBuilder::new(&striker_species, 30)
        .moves(vec![strike("claw", Type::Feral, 40, true, false)])
        .build();
    let state = BattleState::new(BattleKind::Trainer, vec![striker], vec![holder], chart());
    let mut rng = BattleRng::from_seed(8);
    let (next, events) = step(
        &state,
        &TurnActions::new(Action::Move { slot: 0 }, Action::None),
        &mut rng,
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e, BattleEvent::ItemNote { side: 1, .. })),
        "chime rang"
    );
    assert!(matches!(
        next.sides[1].active_mote().held,
        HeldItem::OranChime { used: true }
    ));
}

#[test]
fn keysmith_doubles_bell_assist() {
    // Wild battle: keysmith holder rings a base bell; catch odds match a
    // 2× bell for a keysmith-less ringer (same seed, same target).
    let player_species = species("smith", Type::Alloy, 60);
    let wild_species = species("wildy", Type::Feral, 40);
    let run = |ability: Ability, bell: Frac| {
        let player = MoteBuilder::new(&player_species, 30)
            .moves(vec![strike("claw", Type::Feral, 50, true, false)])
            .ability(ability)
            .build();
        let wild = MoteBuilder::new(&wild_species, 10)
            .moves(vec![strike("claw", Type::Feral, 50, true, false)])
            .build();
        let state = BattleState::new(BattleKind::Wild, vec![player], vec![wild], chart());
        let mut rng = BattleRng::from_seed(0x0CA7C);
        let (next, _) = step(
            &state,
            &TurnActions::new(Action::UseBell { bell_mod: bell }, Action::None),
            &mut rng,
        );
        next.outcome
    };
    assert_eq!(
        run(Ability::Keysmith, Frac(1, 1)),
        run(Ability::None, Frac(2, 1)),
        "keysmith ×2 equals a doubled bell"
    );
}
