//! Golden replay corpus (doc 03 §2): five scripted battles whose event
//! streams are locked byte-for-byte. Breaking one means a rules change —
//! bump `battle::REPLAY_VERSION` and regenerate consciously:
//!
//! ```sh
//! UPDATE_GOLDENS=1 cargo test -p battle --test goldens
//! ```

use std::path::PathBuf;

use battle::{
    Action, BattleKind, BattleMote, BattleState, MoteBuilder, PositionAction, TurnActions, step,
};
use undersong_core::chart::TypeChart;
use undersong_core::moves::{
    Ailment, Effect, EffectTarget, Frac, MoveCategory, MoveFlags, MoveSpec, MoveTarget, WeatherKind,
};
use undersong_core::rng::BattleRng;
use undersong_core::species::{GrowthCurve, SpeciesSpec, StatSpread};
use undersong_core::stats::Stat;
use undersong_core::types::Type;

fn goldens_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/goldens")
}

fn chart_with(rows: &str) -> TypeChart {
    // Start fully neutral, override listed cells.
    let mut text = String::from("TypeChart(entries: {");
    for attacker in Type::ALL {
        text.push_str(&format!("{attacker:?}: {{"));
        for defender in Type::ALL {
            text.push_str(&format!("{defender:?}: Neutral,"));
        }
        text.push_str("},");
    }
    text.push_str("})");
    let mut chart: TypeChart = ron::from_str(&text).expect("base chart");
    if !rows.is_empty() {
        let overrides: TypeChart = ron::from_str(rows).expect("override chart");
        for (attacker, row) in overrides.entries.iter() {
            for (defender, eff) in row.iter() {
                chart
                    .entries
                    .get_mut(attacker)
                    .expect("attacker row")
                    .insert(*defender, *eff);
            }
        }
    }
    chart
}

fn species(id: &str, types: &[Type], stats: [u16; 6]) -> SpeciesSpec {
    SpeciesSpec {
        id: id.into(),
        name_key: format!("motif.{id}"),
        types: types.to_vec(),
        base_stats: StatSpread {
            hp: stats[0],
            atk: stats[1],
            def: stats[2],
            spa: stats[3],
            spd: stats[4],
            spe: stats[5],
        },
        catch_rate: 120,
        base_exp_yield: 120,
        ev_yield: std::collections::BTreeMap::from([(Stat::Atk, 1u8)]).into(),
        growth_curve: GrowthCurve::MediumFast,
        learnset: vec![(26, "encore_theme".into())],
        abilities: vec![],
        hidden_ability: None,
        tags: vec![],
    }
}

fn attack(id: &str, ty: Type, category: MoveCategory, power: u16, accuracy: u8) -> MoveSpec {
    MoveSpec {
        id: id.into(),
        name_key: format!("move.{id}"),
        r#type: ty,
        category,
        power,
        accuracy,
        pp: 25,
        priority: 0,
        target: MoveTarget::Foe,
        flags: MoveFlags::default(),
        effects: vec![],
    }
}

fn mote(spec: &SpeciesSpec, level: u8, moves: Vec<MoveSpec>) -> BattleMote {
    MoteBuilder::new(spec, level)
        .ivs(battle::mote::PERFECT_IVS)
        .moves(moves)
        .build()
}

struct Scenario {
    name: &'static str,
    seed: u64,
    state: BattleState,
    /// Per-turn actions; cycled if the battle runs longer.
    script: Vec<TurnActions>,
}

fn scenarios() -> Vec<Scenario> {
    let mut list = Vec::new();

    // 1 — duel: straight STAB exchange to a KO.
    {
        let ember = species("golden_ember", &[Type::Ember], [60, 55, 50, 80, 60, 70]);
        let tide = species("golden_tide", &[Type::Tide], [65, 50, 65, 75, 70, 55]);
        let singe = attack("singe", Type::Ember, MoveCategory::Special, 65, 100);
        let surge = attack("surge", Type::Tide, MoveCategory::Special, 65, 100);
        let chart =
            chart_with("TypeChart(entries: { Ember: { Tide: Half }, Tide: { Ember: Double } })");
        list.push(Scenario {
            name: "duel",
            seed: 0x0001_D0E1,
            state: BattleState::new(
                BattleKind::Trainer,
                vec![mote(&ember, 25, vec![singe])],
                vec![mote(&tide, 25, vec![surge])],
                chart,
            ),
            script: vec![TurnActions::new(
                Action::Move { slot: 0 },
                Action::Move { slot: 0 },
            )],
        });
    }

    // 2 — status war: burn vs paralysis vs sleep.
    {
        let caster = species("golden_caster", &[Type::Frost], [70, 50, 60, 70, 75, 60]);
        let bruiser = species("golden_bruiser", &[Type::Feral], [75, 80, 65, 40, 55, 65]);
        let mut lull = attack("lull", Type::Frost, MoveCategory::Status, 0, 75);
        lull.effects = vec![Effect::Status {
            ailment: Ailment::Sleep,
            chance: 100,
        }];
        let mut spark = attack("numbing_spark", Type::Volt, MoveCategory::Special, 50, 100);
        spark.effects = vec![Effect::Status {
            ailment: Ailment::Paralysis,
            chance: 30,
        }];
        let mut maul = attack("maul", Type::Feral, MoveCategory::Physical, 70, 100);
        maul.flags.contact = true;
        list.push(Scenario {
            name: "status_war",
            seed: 0x0002_57A7,
            state: BattleState::new(
                BattleKind::Trainer,
                vec![mote(&caster, 28, vec![lull, spark])],
                vec![mote(&bruiser, 28, vec![maul])],
                chart_with(""),
            ),
            script: vec![
                TurnActions::new(Action::Move { slot: 0 }, Action::Move { slot: 0 }),
                TurnActions::new(Action::Move { slot: 1 }, Action::Move { slot: 0 }),
            ],
        });
    }

    // 3 — pivot: type-driven switching.
    {
        let volt = species("golden_volt", &[Type::Volt], [55, 55, 50, 80, 55, 95]);
        let stone = species("golden_stone", &[Type::Stone], [80, 85, 90, 40, 60, 35]);
        let gale = species("golden_gale", &[Type::Gale], [60, 55, 55, 75, 60, 85]);
        let bolt = attack("bolt", Type::Volt, MoveCategory::Special, 70, 100);
        let toll = attack("toll", Type::Stone, MoveCategory::Physical, 75, 90);
        let riff = attack("riff", Type::Gale, MoveCategory::Special, 60, 100);
        let chart = chart_with(
            "TypeChart(entries: {
                Volt:  { Gale: Double, Stone: Zero },
                Stone: { Volt: Double, Gale: Double },
                Gale:  { Stone: Half, Volt: Half },
            })",
        );
        list.push(Scenario {
            name: "pivot",
            seed: 0x0003_9170,
            state: BattleState::new(
                BattleKind::Trainer,
                vec![
                    mote(&gale, 26, vec![riff.clone()]),
                    mote(&stone, 26, vec![toll.clone()]),
                ],
                vec![mote(&volt, 26, vec![bolt.clone()])],
                chart,
            ),
            script: vec![
                TurnActions::new(Action::Switch { to: 1 }, Action::Move { slot: 0 }),
                TurnActions::new(Action::Move { slot: 0 }, Action::Move { slot: 0 }),
            ],
        });
    }

    // 4 — wild hunt: chip, failed escape odds, bells, rings.
    {
        let hunter = species("golden_hunter", &[Type::Feral], [70, 75, 65, 45, 60, 75]);
        let quarry = species("golden_quarry", &[Type::Bloom], [70, 70, 70, 60, 70, 50]);
        let jab = attack("jab", Type::Feral, MoveCategory::Physical, 40, 100);
        let pick = attack("pick", Type::Bloom, MoveCategory::Physical, 55, 95);
        list.push(Scenario {
            name: "wild_hunt",
            seed: 0x0004_CA7C,
            state: BattleState::new(
                BattleKind::Wild,
                vec![mote(&hunter, 24, vec![jab])],
                vec![mote(&quarry, 22, vec![pick])],
                chart_with(""),
            ),
            script: vec![
                TurnActions::new(Action::Move { slot: 0 }, Action::Move { slot: 0 }),
                TurnActions::new(Action::Run, Action::Move { slot: 0 }),
                TurnActions::new(
                    Action::UseBell {
                        bell_mod: Frac(1, 1),
                    },
                    Action::Move { slot: 0 },
                ),
                TurnActions::new(Action::Move { slot: 0 }, Action::Move { slot: 0 }),
                TurnActions::new(
                    Action::UseBell {
                        bell_mod: Frac(3, 2),
                    },
                    Action::Move { slot: 0 },
                ),
            ],
        });
    }

    // 5 — chaos: drain, weather, stat stages, multihit, thin PP into
    // last resort.
    {
        let weaver = species("golden_weaver", &[Type::Bloom], [65, 60, 60, 75, 65, 60]);
        let storm = species("golden_storm", &[Type::Ember], [60, 65, 55, 75, 55, 75]);
        let mut chord = attack("chord", Type::Bloom, MoveCategory::Special, 70, 100);
        chord.pp = 5;
        chord.effects = vec![Effect::Drain { frac: Frac(1, 2) }];
        let mut swell = attack("swell", Type::Resonant, MoveCategory::Status, 0, 0);
        swell.target = MoveTarget::User;
        swell.pp = 5;
        swell.effects = vec![
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
        let mut flurry_hits = attack("flurry_hits", Type::Ember, MoveCategory::Physical, 18, 100);
        flurry_hits.pp = 5;
        flurry_hits.effects = vec![Effect::MultiHit];
        let mut kindle = attack("kindle", Type::Ember, MoveCategory::Status, 0, 0);
        kindle.target = MoveTarget::User;
        kindle.pp = 5;
        kindle.effects = vec![Effect::Weather {
            kind: WeatherKind::Heatwave,
        }];
        let chart =
            chart_with("TypeChart(entries: { Ember: { Bloom: Double }, Bloom: { Ember: Half } })");
        list.push(Scenario {
            name: "chaos",
            seed: 0x0005_CA05,
            state: BattleState::new(
                BattleKind::Trainer,
                vec![mote(&weaver, 27, vec![chord, swell])],
                vec![mote(&storm, 27, vec![flurry_hits, kindle])],
                chart,
            ),
            script: vec![
                TurnActions::new(Action::Move { slot: 1 }, Action::Move { slot: 1 }),
                TurnActions::new(Action::Move { slot: 0 }, Action::Move { slot: 0 }),
            ],
        });
    }

    // 6 — tandem (doubles, doc 02 v1.5 #2): declared targets, a mid-turn
    // KO, and the retarget-to-survivor rule.
    {
        let lead = species("golden_lead", &[Type::Ember], [60, 70, 55, 70, 55, 90]);
        let anchor = species("golden_anchor", &[Type::Tide], [70, 60, 70, 60, 70, 40]);
        let frail = species("golden_frail", &[Type::Bloom], [45, 55, 40, 55, 40, 60]);
        let bulwark = species("golden_bulwark", &[Type::Stone], [85, 70, 90, 40, 70, 20]);
        let singe = attack("singe", Type::Ember, MoveCategory::Special, 70, 100);
        let surge = attack("surge", Type::Tide, MoveCategory::Special, 60, 100);
        let pick = attack("pick", Type::Bloom, MoveCategory::Physical, 50, 100);
        let toll = attack("toll", Type::Stone, MoveCategory::Physical, 60, 90);
        let chart = chart_with(
            "TypeChart(entries: {
                Ember: { Bloom: Double, Stone: Half },
                Tide:  { Stone: Double },
                Stone: { Ember: Double },
            })",
        );
        // Both player positions pile onto foe slot 0: if the first strike
        // KOs, the second retargets to the survivor (v1.5 #2).
        let pile_on = TurnActions::doubles(vec![
            PositionAction {
                side: 0,
                position: 0,
                action: Action::Move { slot: 0 },
                target_position: 0,
            },
            PositionAction {
                side: 0,
                position: 1,
                action: Action::Move { slot: 0 },
                target_position: 0,
            },
            PositionAction {
                side: 1,
                position: 0,
                action: Action::Move { slot: 0 },
                target_position: 1,
            },
            PositionAction {
                side: 1,
                position: 1,
                action: Action::Move { slot: 0 },
                target_position: 0,
            },
        ]);
        list.push(Scenario {
            name: "tandem",
            seed: 0x0006_D0B1,
            state: BattleState::new_double(
                BattleKind::Trainer,
                vec![mote(&lead, 25, vec![singe]), mote(&anchor, 25, vec![surge])],
                vec![mote(&frail, 25, vec![pick]), mote(&bulwark, 25, vec![toll])],
                chart,
            ),
            script: vec![pile_on],
        });
    }

    list
}

/// Drives a scenario to completion (or 40 turns), returning the full
/// event stream serialized as pretty RON.
fn play(scenario: &Scenario) -> String {
    let mut rng = BattleRng::from_seed(scenario.seed);
    let mut state = scenario.state.clone();
    let mut events = Vec::new();
    let mut turn = 0usize;
    while state.outcome.is_none() && turn < 40 {
        let actions = &scenario.script[turn % scenario.script.len()];
        let (next, step_events) = step(&state, actions, &mut rng);
        events.extend(step_events);
        state = next;
        turn += 1;
    }
    let header = format!(
        "// golden replay `{}` — REPLAY_VERSION {}\n// regenerate: UPDATE_GOLDENS=1 cargo test -p battle --test goldens\n",
        scenario.name,
        battle::REPLAY_VERSION
    );
    let body = ron::ser::to_string_pretty(&events, ron::ser::PrettyConfig::default())
        .expect("serialize events");
    format!("{header}{body}\n")
}

#[test]
fn golden_replays_are_locked() {
    let update = std::env::var_os("UPDATE_GOLDENS").is_some();
    let dir = goldens_dir();
    if update {
        std::fs::create_dir_all(&dir).expect("create goldens dir");
    }
    let mut failures = Vec::new();
    for scenario in scenarios() {
        let produced = play(&scenario);
        let path = dir.join(format!("{}.ron", scenario.name));
        if update {
            std::fs::write(&path, &produced).expect("write golden");
            continue;
        }
        let expected = std::fs::read_to_string(&path)
            .unwrap_or_else(|_| panic!("missing golden {} — run UPDATE_GOLDENS=1", scenario.name));
        if produced != expected {
            failures.push(scenario.name);
        }
    }
    assert!(
        failures.is_empty(),
        "golden replays diverged: {failures:?} — a rules change must bump \
         REPLAY_VERSION and regenerate goldens consciously (doc 03 §2)"
    );
}

#[test]
fn corpus_has_six_scenarios() {
    // Doc 06 P1 locked five singles battles; doubles (P4) adds `tandem`.
    assert_eq!(scenarios().len(), 6, "5 singles + 1 doubles");
}
