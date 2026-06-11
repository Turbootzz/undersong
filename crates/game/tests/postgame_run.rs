//! P7 post-game loop, driven headlessly: the Encore ladder pays the
//! Coda, the Coda lands a legendary, and Vesper finally shows her hand.

mod common;

use common::{Driver, content_root};
use game::world::Input;
use undersong_core::rng::BattleRng;
use undersong_core::world::Facing::{Right, Up};

fn postgame_world() -> Driver {
    let world = game::world::load_game_world(&content_root(), 0x00CA_FE77)
        .unwrap_or_else(|_| panic!("game world loads"));
    let mut driver = Driver::new(world);
    // A finished campaign in miniature: endings chosen, badges held,
    // a battle-ready epilogue party.
    for flag in [
        "ending.chosen",
        "ending.chorus",
        "credits.chorus",
        "badge.8",
        "story.quartet.4.defeated",
        "story.descent",
        "story.vesper.met",
    ] {
        driver.world.vars.flags.insert(flag.into());
    }
    let mut rng = BattleRng::from_seed(77);
    let mut party = Vec::new();
    {
        let registry = driver.world.registry.as_ref().expect("registry");
        for (species, level) in [
            ("maestroar", 70),
            ("gravoross", 68),
            ("orchestrios", 68),
            ("velvetide", 68),
        ] {
            let mut mote = registry
                .wild_individual(&species.into(), level, &mut rng)
                .expect("epilogue mote");
            mote.ot = "player".into();
            party.push(mote);
        }
    }
    driver.world.party = party;
    driver.world.money = 50_000;
    driver.world.bag.insert("potion_x".into(), 6);
    // Teleport to the capital (the campaign would have walked).
    driver.world.current_map = "cadenza_city".into();
    driver.world.player = (9, 11);
    driver
}

#[test]
fn encore_ladder_pays_the_coda_and_the_coda_lands_primavoce() {
    let mut driver = postgame_world();

    // Into the Encore hall.
    driver.go_y(9);
    driver.go_x(7);
    driver.go_y(10); // → encore_hall (4,1)
    assert_eq!(driver.world.current_map.as_str(), "encore_hall");

    // Seven curtain calls. Losses reset the ladder, so retry generously.
    for _ in 0..90 {
        if driver.has_flag("encore.complete") {
            break;
        }
        driver.go_x(4);
        driver.go_y(7);
        driver.face(Up);
        driver.interact();
        driver.drain();
    }
    assert!(
        driver.has_flag("encore.complete"),
        "streak flags: {:?} party {:?}",
        (1..=6)
            .filter(|n| driver.has_flag(&format!("encore.streak.{n}")))
            .collect::<Vec<_>>(),
        driver
            .world
            .party
            .iter()
            .map(|p| format!("{} hp{:?}", p.species, p.hp))
            .collect::<Vec<_>>(),
    );
    assert_eq!(
        driver.world.bag.get(&"coda".into()).copied().unwrap_or(0),
        1,
        "the Coda is minted"
    );

    // To the Spire stage: the first voice answers the Chorus.
    driver.go_y(0); // → cadenza (7,9)
    driver.go_y(9);
    driver.go_x(24); // → spire (5,1)
    assert_eq!(driver.world.current_map.as_str(), "quartet_spire");
    driver.go_x(6);
    driver.go_y(25);
    // The last step is raw: the drain inside go_y would fight the
    // legendary instead of letting us ring for it.
    driver.input(Input::Step(Up));
    driver.input(Input::Step(Up));
    for _ in 0..10 {
        if driver.world.battle.is_some() {
            break;
        }
        driver.input(Input::Interact); // pump the static's Say line
    }
    assert!(driver.world.battle.is_some(), "primavoce answers");
    driver.input(Input::Battle(game::session::BattleCmd::Bell));
    driver.drain();
    assert!(
        driver.has_flag("dex.caught.primavoce"),
        "the first voice joins the Score"
    );

    // Down to the Vault: Vesper's honest match.
    driver.go_x(3);
    driver.go_y(27);
    driver.go_x(4);
    driver.go_y(28);
    driver.drain(); // the floor remembers → the_vault (5,1)
    assert_eq!(driver.world.current_map.as_str(), "the_vault");
    driver.go_x(3);
    driver.go_y(23);
    driver.go_x(4);
    driver.go_y(24); // beside Vesper at (5,24)
    driver.face(Right);
    driver.interact();
    driver.drain();
    assert!(
        driver.has_flag("trainer.vesper_epilogue.defeated"),
        "her hand, shown and answered"
    );
}
