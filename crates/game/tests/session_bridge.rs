//! Battle-bridge tests against the real Cantorel content: resolve,
//! fight, catch, payout, learn, evolve — all pure.

use std::path::{Path, PathBuf};

use game::session::{BattleCmd, BattleSession, Registry};
use game::world::{Input, WorldEvent};
use undersong_core::individual::{Individual, LearnedMove};
use undersong_core::rng::BattleRng;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn registry() -> Registry {
    let content_root = root().join("content");
    let core_content = data::load_core(&content_root).expect("core");
    let items = data::load_items(&content_root).expect("items");
    let pack = data::load_region(&content_root, "cantorel").expect("pack");
    Registry::from_content(&core_content, &pack, &items)
}

fn starter(registry: &Registry, species: &str, level: u8) -> Individual {
    let mut rng = BattleRng::from_seed(42);
    let mut individual = registry
        .wild_individual(&species.into(), level, &mut rng)
        .expect("species exists");
    individual.ot = "player".into();
    individual
}

#[test]
fn individuals_resolve_with_stats_moves_and_fold_back() {
    let registry = registry();
    let fanfyre = starter(&registry, "fanfyre", 8);
    let mote = registry.resolve(&fanfyre).expect("resolves");
    assert_eq!(mote.level, 8);
    assert!(mote.max_hp() > 0);
    assert!(!mote.moves.is_empty(), "level-8 fanfyre knows moves");
    assert!(
        mote.moves
            .iter()
            .any(|m| m.spec.id.as_str() == "ember_note"),
        "STAB from the learnset"
    );

    let mut updated = fanfyre.clone();
    let mut harmed = mote.clone();
    harmed.take_damage(5);
    Registry::fold_back(&mut updated, &harmed);
    assert_eq!(updated.hp, Some(mote.max_hp() - 5));
}

#[test]
fn wild_battle_runs_and_can_be_won() {
    let registry = registry();
    let party = vec![starter(&registry, "embaritone", 20)];
    let mut rng = BattleRng::from_seed(7);
    let wild = registry
        .wild_individual(&"tremole".into(), 4, &mut rng)
        .expect("wild");
    let mut session =
        BattleSession::wild(&registry, &party, wild, &mut rng).expect("session starts");

    let mut turns = 0;
    while session.outcome().is_none() && turns < 30 {
        session.turn(BattleCmd::Move { slot: 0 }, None, None, &mut rng);
        turns += 1;
    }
    assert_eq!(
        session.outcome(),
        Some(battle::Outcome::Won { winner: 0 }),
        "a level-20 mid stage beats a level-4 tremole"
    );
}

#[test]
fn full_world_catch_flow_awards_the_mote() {
    let content_root = root().join("content");
    let mut world = game::world::WorldState::new(
        std::collections::BTreeMap::from([(
            "void".into(),
            data::MapDef {
                id: "void".into(),
                name_key: "map.void".into(),
                width: 3,
                height: 3,
                ground: vec![4; 9],
                decor: vec![],
                overhang: vec![],
                collision: vec![0; 9],
                patches: vec![],
                triggers: vec![],
                npcs: vec![],
                encounters: None,
                music: None,
                weather: None,
                night_encounters: None,
                obstacles: vec![],
                dark: false,
            },
        )]),
        std::collections::BTreeMap::new(),
        "void".into(),
        (1, 1),
        99,
    );
    let core_content = data::load_core(&content_root).expect("core");
    let items = data::load_items(&content_root).expect("items");
    let pack = data::load_region(&content_root, "cantorel").expect("pack");
    world.registry = Some(Registry::from_content(&core_content, &pack, &items));
    world.party = vec![{
        let registry = world.registry.as_ref().unwrap();
        let mut rng = BattleRng::from_seed(1);
        let mut s = registry
            .wild_individual(&"embaritone".into(), 25, &mut rng)
            .unwrap();
        s.ot = "player".into();
        s
    }];
    world.bag.insert("maestro_fermata".into(), 5);

    // Force an encounter directly.
    world.pending_encounter = Some(("tremole".into(), 3));
    let mut events = Vec::new();
    // Use the internal hook through a step-free path: weaken then bell.
    {
        // start the battle
        let before = world.battle.is_some();
        assert!(!before);
        // call the private path via apply: a Battle command only works
        // once a battle exists, so trigger via the public seam:
        world.apply(Input::Tick); // no-op, but ensures normal flow works
    }
    // The encounter hook runs inside step(); emulate by invoking the
    // bridge directly through the public battle API:
    let registry = world.registry.as_ref().unwrap();
    let mut rng = BattleRng::from_seed(5);
    let wild = registry
        .wild_individual(&"tremole".into(), 3, &mut rng)
        .unwrap();
    world.battle = Some(BattleSession::wild(registry, &world.party, wild, &mut rng).unwrap());

    // Weaken once, then ring bells until caught (maestro bell, weak foe).
    let mut caught = false;
    for _ in 0..20 {
        let stream_events = world.apply(Input::Battle(BattleCmd::Bell));
        for event in &stream_events {
            if matches!(event, WorldEvent::MoteCaught { .. }) {
                caught = true;
            }
        }
        if caught || world.battle.is_none() {
            break;
        }
        events.extend(stream_events);
    }
    assert!(caught, "maestro bells on a level-3 foe catch quickly");
    assert_eq!(world.party.len(), 2, "caught mote joins the party");
    assert!(
        world
            .bag
            .get(&"maestro_fermata".into())
            .copied()
            .unwrap_or(0)
            < 5
    );
}

#[test]
fn trainer_battle_pays_out_and_sets_flag() {
    let trainer = data::Trainer {
        id: "test_rival".into(),
        class: "Rival".into(),
        name_key: "npc.rival".into(),
        ai_tier: 1,
        payout_base: 20,
        double_battle: false,
        party: vec![data::TrainerMote {
            species: "tremole".into(),
            level: 5,
            moves: None,
            ivs: None,
            held_item: None,
        }],
        defeat_flag: "rival.1.defeated".into(),
        reward_items: vec![],
        intro_key: "battle.rival.intro".into(),
        defeat_key: "battle.rival.defeat".into(),
    };

    let content_root = root().join("content");
    let mut world = game::world::WorldState::new(
        std::collections::BTreeMap::from([(
            "void".into(),
            data::MapDef {
                id: "void".into(),
                name_key: "map.void".into(),
                width: 3,
                height: 3,
                ground: vec![4; 9],
                decor: vec![],
                overhang: vec![],
                collision: vec![0; 9],
                patches: vec![],
                triggers: vec![],
                npcs: vec![],
                encounters: None,
                music: None,
                weather: None,
                night_encounters: None,
                obstacles: vec![],
                dark: false,
            },
        )]),
        std::collections::BTreeMap::new(),
        "void".into(),
        (1, 1),
        21,
    );
    let core_content = data::load_core(&content_root).expect("core");
    let items = data::load_items(&content_root).expect("items");
    let mut pack = data::load_region(&content_root, "cantorel").expect("pack");
    pack.trainers.insert(trainer.id.clone(), trainer.clone());
    world.registry = Some(Registry::from_content(&core_content, &pack, &items));
    world.party = vec![{
        let registry = world.registry.as_ref().unwrap();
        let mut rng = BattleRng::from_seed(2);
        let mut s = registry
            .wild_individual(&"vinebrato".into(), 20, &mut rng)
            .unwrap();
        s.ot = "player".into();
        s
    }];
    let money_before = world.money;

    world.start_trainer_battle(&"test_rival".into());
    assert!(world.battle.is_some());
    let mut finished = false;
    for _ in 0..40 {
        let events = world.apply(Input::Battle(BattleCmd::Move { slot: 0 }));
        if events
            .iter()
            .any(|e| matches!(e, WorldEvent::BattleFinished { .. }))
        {
            finished = true;
            break;
        }
    }
    assert!(finished);
    assert!(world.vars.flags.contains("rival.1.defeated"));
    assert_eq!(
        world.money,
        money_before + 20 * 5,
        "payout = base × ace level"
    );

    // Re-challenging a defeated trainer is a no-op.
    world.start_trainer_battle(&"test_rival".into());
    assert!(world.battle.is_none());
}

#[test]
fn level_evolution_prompts_and_resolves() {
    let registry = registry();
    let mut world = game::world::WorldState::new(
        std::collections::BTreeMap::from([(
            "void".into(),
            data::MapDef {
                id: "void".into(),
                name_key: "map.void".into(),
                width: 3,
                height: 3,
                ground: vec![4; 9],
                decor: vec![],
                overhang: vec![],
                collision: vec![0; 9],
                patches: vec![],
                triggers: vec![],
                npcs: vec![],
                encounters: None,
                music: None,
                weather: None,
                night_encounters: None,
                obstacles: vec![],
                dark: false,
            },
        )]),
        std::collections::BTreeMap::new(),
        "void".into(),
        (1, 1),
        3,
    );
    world.registry = Some(registry);
    let registry = world.registry.as_ref().unwrap();
    let mut rng = BattleRng::from_seed(11);
    let mut fanfyre = registry
        .wild_individual(&"fanfyre".into(), 16, &mut rng)
        .unwrap();
    fanfyre.ot = "player".into();
    world.party = vec![fanfyre];
    world.pending_evolutions = vec![(0, "embaritone".into())];

    let events = world.apply(Input::Evolve { accept: true });
    assert!(events.iter().any(|e| matches!(
        e,
        WorldEvent::Evolved { into, .. } if into.as_str() == "embaritone"
    )));
    assert_eq!(world.party[0].species.as_str(), "embaritone");
}

#[test]
fn learn_prompt_replaces_a_move_when_answered() {
    let registry = registry();
    let mut world = game::world::WorldState::new(
        std::collections::BTreeMap::from([(
            "void".into(),
            data::MapDef {
                id: "void".into(),
                name_key: "map.void".into(),
                width: 3,
                height: 3,
                ground: vec![4; 9],
                decor: vec![],
                overhang: vec![],
                collision: vec![0; 9],
                patches: vec![],
                triggers: vec![],
                npcs: vec![],
                encounters: None,
                music: None,
                weather: None,
                night_encounters: None,
                obstacles: vec![],
                dark: false,
            },
        )]),
        std::collections::BTreeMap::new(),
        "void".into(),
        (1, 1),
        4,
    );
    world.registry = Some(registry);
    let registry = world.registry.as_ref().unwrap();
    let mut rng = BattleRng::from_seed(12);
    let mut subject = registry
        .wild_individual(&"maestroar".into(), 38, &mut rng)
        .unwrap();
    subject.ot = "player".into();
    // Give it a full moveset, then prompt resonate into slot 2.
    subject.moves = ["tackle", "ember_note", "dampen", "gale_riff"]
        .into_iter()
        .map(|id| LearnedMove {
            id: id.into(),
            pp: 10,
            pp_ups: 0,
        })
        .collect();
    world.party = vec![subject];
    world.pending_learn_queue = vec![(0, "resonate".into())];

    let events = world.apply(Input::Learn { replace: Some(2) });
    assert!(events.iter().any(|e| matches!(
        e,
        WorldEvent::MoveLearned { move_id, .. } if move_id.as_str() == "resonate"
    )));
    assert_eq!(world.party[0].moves[2].id.as_str(), "resonate");
}
