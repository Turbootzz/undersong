//! P4 world-system tests: clock/night tables, Mute Charm, item use
//! (vitamins, TMs, stones), friendship evolutions, performances,
//! rematch, Shift — all against real Cantorel content, all pure.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use game::session::Registry;
use game::world::{Input, WorldEvent, WorldState};
use undersong_core::rng::BattleRng;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn void_map() -> data::MapDef {
    data::MapDef {
        id: "void".into(),
        name_key: "map.void".into(),
        width: 8,
        height: 8,
        ground: vec![1; 64],
        decor: vec![],
        overhang: vec![],
        collision: vec![0; 64],
        patches: vec![],
        triggers: vec![],
        npcs: vec![],
        encounters: None,
        music: None,
        weather: None,
        night_encounters: None,
        obstacles: vec![],
        dark: false,
        indoor: false,
    }
}

fn world_with(map: data::MapDef) -> WorldState {
    let content_root = root().join("content");
    let mut world = WorldState::new(
        BTreeMap::from([(map.id.clone(), map.clone())]),
        BTreeMap::new(),
        map.id.clone(),
        (1, 1),
        7,
    );
    let core_content = data::load_core(&content_root).expect("core");
    let items = data::load_items(&content_root).expect("items");
    let pack = data::load_region(&content_root, "cantorel").expect("pack");
    world.registry = Some(Registry::from_content(&core_content, &pack, &items));
    world
}

fn member(world: &WorldState, species: &str, level: u8) -> undersong_core::individual::Individual {
    let registry = world.registry.as_ref().unwrap();
    let mut rng = BattleRng::from_seed(3);
    let mut individual = registry
        .wild_individual(&species.into(), level, &mut rng)
        .unwrap();
    individual.ot = "player".into();
    individual
}

#[test]
fn clock_crosses_into_night_and_back() {
    let mut world = world_with(void_map());
    world.party = vec![member(&world, "fanfyre", 10)];
    let mut crossings = Vec::new();
    for _ in 0..1300 {
        // bounce between two tiles; tap-to-turn means each direction
        // needs a turn tap then a move
        for dir in [
            undersong_core::world::Facing::Right,
            undersong_core::world::Facing::Left,
        ] {
            world.apply(Input::Step(dir)); // turn
            for event in world.apply(Input::Step(dir)) {
                if let WorldEvent::ClockPhase { night } = event {
                    crossings.push(night);
                }
            }
        }
        if crossings.len() >= 2 {
            break;
        }
    }
    assert_eq!(crossings, vec![true, false], "night falls, then morning");
}

#[test]
fn night_uses_the_night_table() {
    let mut map = void_map();
    map.patches = vec![1; 64];
    map.encounters = Some(data::EncounterDef {
        patch_rate_pct: 100,
        slots: vec![
            ("tremole".into(), 5, 5, 20),
            ("tremole".into(), 5, 5, 20),
            ("tremole".into(), 5, 5, 10),
            ("tremole".into(), 5, 5, 10),
            ("tremole".into(), 5, 5, 10),
            ("tremole".into(), 5, 5, 10),
            ("tremole".into(), 5, 5, 5),
            ("tremole".into(), 5, 5, 5),
            ("tremole".into(), 5, 5, 4),
            ("tremole".into(), 5, 5, 4),
            ("tremole".into(), 5, 5, 1),
            ("tremole".into(), 5, 5, 1),
        ],
    });
    map.night_encounters = Some(data::EncounterDef {
        patch_rate_pct: 100,
        slots: vec![
            ("dirgeist".into(), 5, 5, 20),
            ("dirgeist".into(), 5, 5, 20),
            ("dirgeist".into(), 5, 5, 10),
            ("dirgeist".into(), 5, 5, 10),
            ("dirgeist".into(), 5, 5, 10),
            ("dirgeist".into(), 5, 5, 10),
            ("dirgeist".into(), 5, 5, 5),
            ("dirgeist".into(), 5, 5, 5),
            ("dirgeist".into(), 5, 5, 4),
            ("dirgeist".into(), 5, 5, 4),
            ("dirgeist".into(), 5, 5, 1),
            ("dirgeist".into(), 5, 5, 1),
        ],
    });
    let mut world = world_with(map);
    world.party = vec![member(&world, "maestroar", 50)];
    world.clock_ticks = 801; // night
    let mut seen = None;
    for dir in [
        undersong_core::world::Facing::Right,
        undersong_core::world::Facing::Right,
    ] {
        for event in world.apply(Input::Step(dir)) {
            if let WorldEvent::EncounterStarted { species, .. } = event {
                seen = Some(species.to_string());
            }
        }
        if seen.is_some() {
            break;
        }
    }
    assert_eq!(seen.as_deref(), Some("dirgeist"), "night table used");
}

#[test]
fn mute_charm_suppresses_lower_level_wilds() {
    let mut map = void_map();
    map.patches = vec![1; 64];
    map.encounters = Some(data::EncounterDef {
        patch_rate_pct: 100,
        slots: vec![
            ("tremole".into(), 2, 2, 20),
            ("tremole".into(), 2, 2, 20),
            ("tremole".into(), 2, 2, 10),
            ("tremole".into(), 2, 2, 10),
            ("tremole".into(), 2, 2, 10),
            ("tremole".into(), 2, 2, 10),
            ("tremole".into(), 2, 2, 5),
            ("tremole".into(), 2, 2, 5),
            ("tremole".into(), 2, 2, 4),
            ("tremole".into(), 2, 2, 4),
            ("tremole".into(), 2, 2, 1),
            ("tremole".into(), 2, 2, 1),
        ],
    });
    let mut world = world_with(map);
    world.party = vec![member(&world, "maestroar", 40)];
    world.bag.insert("mute_charm".into(), 1);
    let events = world.apply(Input::UseItem {
        item: "mute_charm".into(),
        target: 0,
        slot: None,
    });
    assert!(
        events
            .iter()
            .any(|e| matches!(e, WorldEvent::ItemUsed { .. }))
    );
    assert_eq!(world.mute_steps, 200);

    for index in 0..100 {
        let dir = if index % 2 == 0 {
            undersong_core::world::Facing::Right
        } else {
            undersong_core::world::Facing::Left
        };
        world.apply(Input::Step(dir)); // turn tap
        let events = world.apply(Input::Step(dir)); // move
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, WorldEvent::EncounterStarted { .. })),
            "mute charm silences level-2 wilds under a level-40 lead"
        );
        if world.battle.is_some() {
            panic!("no battle should start while muted");
        }
    }
    assert!(world.mute_steps < 200, "steps burned");
}

#[test]
fn vitamins_add_evs_and_respect_the_cap() {
    let mut world = world_with(void_map());
    world.party = vec![member(&world, "fanfyre", 10)];
    world.bag.insert("aria_drop".into(), 30);
    for _ in 0..10 {
        world.apply(Input::UseItem {
            item: "aria_drop".into(),
            target: 0,
            slot: None,
        });
    }
    assert_eq!(
        world.party[0].evs.get(undersong_core::stats::Stat::Spa),
        100,
        "ten doses reach the 100 cap"
    );
    let friendship_before = world.party[0].friendship;
    world.apply(Input::UseItem {
        item: "aria_drop".into(),
        target: 0,
        slot: None,
    });
    assert_eq!(
        world.party[0].evs.get(undersong_core::stats::Stat::Spa),
        100,
        "eleventh dose fails at the cap (doc 02 v1.6 #1)"
    );
    assert_eq!(
        world.party[0].friendship, friendship_before,
        "failed dose grants no friendship"
    );
    assert_eq!(
        world.bag.get(&"aria_drop".into()).copied().unwrap_or(0),
        20,
        "failed dose not consumed"
    );
}

#[test]
fn tms_teach_listed_species_and_are_reusable() {
    let mut world = world_with(void_map());
    world.party = vec![
        member(&world, "fanfyre", 10),
        member(&world, "dirgeist", 10),
    ];
    world.bag.insert("tm02".into(), 1); // cinder_waltz; fanfyre listed
    let events = world.apply(Input::UseItem {
        item: "tm02".into(),
        target: 0,
        slot: None,
    });
    assert!(events.iter().any(|e| matches!(
        e,
        WorldEvent::ItemUsed { message_key, .. } if message_key == "ui.item.tm_taught"
    )));
    assert!(
        world.party[0]
            .moves
            .iter()
            .any(|m| m.id.as_str() == "cinder_waltz")
    );
    assert_eq!(
        world.bag.get(&"tm02".into()).copied().unwrap_or(0),
        1,
        "TMs are reusable (doc 02 v1.6 #2)"
    );

    // dirgeist is not in tm02's set.
    let events = world.apply(Input::UseItem {
        item: "tm02".into(),
        target: 1,
        slot: None,
    });
    assert!(events.iter().any(|e| matches!(
        e,
        WorldEvent::ItemUsed { message_key, .. } if message_key == "ui.item.tm_wrong_species"
    )));
}

#[test]
fn performance_gates_check_badge_then_tag() {
    let mut map = void_map();
    map.obstacles = vec![data::Obstacle {
        at: (2, 1),
        kind: data::ObstacleKind::Brush,
    }];
    let mut world = world_with(map);
    world.party = vec![member(&world, "solfawn", 10)]; // performer.clear
    // Face the brush from (1,1).
    world.apply(Input::Step(undersong_core::world::Facing::Right)); // turn

    // No badge yet.
    let events = world.apply(Input::Interact);
    assert!(events.iter().any(|e| matches!(
        e,
        WorldEvent::ItemUsed { message_key, .. } if message_key == "ui.perform.no_badge"
    )));

    // Badge but no carrier.
    world.vars.flags.insert("badge.2".into());
    world.party = vec![member(&world, "timpanite", 10)];
    let events = world.apply(Input::Interact);
    assert!(events.iter().any(|e| matches!(
        e,
        WorldEvent::ItemUsed { message_key, .. } if message_key == "ui.perform.no_tag"
    )));

    // Badge + carrier clears it, permanently (flag).
    world.party = vec![member(&world, "solfawn", 10)];
    let events = world.apply(Input::Interact);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, WorldEvent::Performed { .. }))
    );
    let stepped = world.apply(Input::Step(undersong_core::world::Facing::Right));
    assert!(
        stepped
            .iter()
            .any(|e| matches!(e, WorldEvent::Stepped { to: (2, 1) }))
    );
    assert!(world.vars.flags.contains("cleared.void.2.1"));
}

#[test]
fn water_blocks_without_ferry_song_and_carries_with_it() {
    let mut map = void_map();
    map.ground[8 + 2] = 5; // water at (2,1)
    let mut world = world_with(map);
    world.party = vec![member(&world, "rippeggio", 10)]; // tide starter is no ferry
    world.apply(Input::Step(undersong_core::world::Facing::Right)); // face

    let events = world.apply(Input::Step(undersong_core::world::Facing::Right));
    assert!(
        events
            .iter()
            .any(|e| matches!(e, WorldEvent::Bumped { .. }))
    );

    // tidalegro carries performer.ferry; badge 4 unlocks.
    world.vars.flags.insert("badge.4".into());
    world.party = vec![member(&world, "tidalegro", 20)];
    let events = world.apply(Input::Step(undersong_core::world::Facing::Right));
    assert!(
        events
            .iter()
            .any(|e| matches!(e, WorldEvent::Performed { .. }))
    );
    assert!(world.surfing);
    assert_eq!(world.player, (2, 1));

    // Stepping back onto land ends the ride.
    world.apply(Input::Step(undersong_core::world::Facing::Left)); // turn
    world.apply(Input::Step(undersong_core::world::Facing::Left)); // move
    assert!(!world.surfing);
}

#[test]
fn duet_stone_evolves_the_flagged_species() {
    // No launch species uses DuetStone yet — synthesize one through the
    // registry to prove the path.
    let mut world = world_with(void_map());
    world.party = vec![member(&world, "fanfyre", 10)];
    if let Some(registry) = &mut world.registry {
        registry.all_evolutions.insert(
            "fanfyre".into(),
            vec![(data::EvolutionMethod::DuetStone, "embaritone".into())],
        );
    }
    world.bag.insert("duet_stone".into(), 1);
    let events = world.apply(Input::UseItem {
        item: "duet_stone".into(),
        target: 0,
        slot: None,
    });
    assert!(events.iter().any(|e| matches!(
        e,
        WorldEvent::Evolved { into, .. } if into.as_str() == "embaritone"
    )));
    assert_eq!(world.party[0].species.as_str(), "embaritone");
    assert!(
        !world.bag.contains_key(&"duet_stone".into()),
        "stone consumed"
    );
}

#[test]
fn rematch_trainers_re_engage_after_defeat() {
    let mut world = world_with(void_map());
    world.party = vec![member(&world, "maestroar", 50)];
    if let Some(registry) = &mut world.registry {
        let mut trainer = registry.trainers.get(&"rt1_tuner".into()).cloned().unwrap();
        trainer.rematch = true;
        registry.trainers.insert(trainer.id.clone(), trainer);
    }
    world.vars.flags.insert("trainer.rt1_tuner.defeated".into());
    world.start_trainer_battle(&"rt1_tuner".into());
    assert!(world.battle.is_some(), "rematch trainer re-engages");
}
