//! P8 acceptance: the Skalden pack — when present — boots into the
//! same world, its boat docks, and its four-badge mini-arc completes,
//! all with zero engine changes (this test predates the pack).

mod common;

use common::{Driver, content_root};
use undersong_core::world::Facing::Up;

fn skalden_present() -> bool {
    content_root().join("regions/skalden/region.ron").exists()
}

#[test]
fn skalden_mini_arc_completes_as_pure_content() {
    if !skalden_present() {
        eprintln!("skalden pack absent — the thesis test waits");
        return;
    }
    let world = game::world::load_game_world(&content_root(), 0x5CA_1DE0)
        .expect("the union world loads with the pack present");
    let mut driver = Driver::new(world);
    // A Chorus-finished save in miniature (travel unlock condition).
    for flag in [
        "ending.chosen",
        "ending.chorus",
        "credits.chorus",
        "badge.8",
    ] {
        driver.world.vars.flags.insert(flag.into());
    }
    let mut rng = undersong_core::rng::BattleRng::from_seed(58);
    let mut party = Vec::new();
    {
        let registry = driver.world.registry.as_ref().expect("registry");
        for (species, level) in [
            ("maestroar", 70),
            ("gravoross", 68),
            ("orchestrios", 68),
            ("avalanche", 68),
        ] {
            let mut mote = registry
                .wild_individual(&species.into(), level, &mut rng)
                .expect("epilogue mote");
            mote.ot = "player".into();
            party.push(mote);
        }
    }
    driver.world.party = party;
    driver.world.money = 60_000;
    driver.world.bag.insert("potion_x".into(), 8);

    // The boat leaves from Port Calando's quay.
    driver.world.current_map = "port_calando".into();
    driver.world.player = (9, 11);
    driver.go_y(4); // under the echo keeper's row
    driver.go_x(2);
    driver.go_y(3); // the quay trigger
    driver.drain();
    assert_eq!(
        driver.world.current_map.as_str(),
        "port_skald",
        "the boat docks"
    );

    // Four halls, one straight folk road: each badge flag in order.
    for n in 1..=4u8 {
        driver.until_flag(
            &format!("skalden.badge.{n}"),
            8,
            move |d| {
                // Every Skalden hall opens off the main road at marked
                // doorsteps; the pack's STATUS table documents them.
                let (town, door, maestro_y) = match n {
                    1 => ("port_skald", (16, 8), 9),
                    2 => ("varde", (16, 8), 9),
                    3 => ("fenwick_hollow", (16, 8), 9),
                    _ => ("kraghorn", (16, 8), 9),
                };
                if d.world.current_map.as_str() == town {
                    d.go_y(door.1 - 1);
                    d.go_x(door.0);
                    d.go_y(door.1); // hall door
                }
                if d.world.current_map.as_str() == format!("skald_hall_{n}").as_str() {
                    d.go_x(6);
                    d.go_y(maestro_y);
                    d.face(Up);
                    d.interact();
                }
            },
            |d| {
                // Recovery: every Skalden town's rest sits at (9,7),
                // mart at (4,7) — the pack keeps the cantorel grammar.
                let here = d.world.current_map.to_string();
                if here.starts_with("skald_hall") {
                    d.go_x(6);
                    d.go_y(0);
                    return;
                }
                d.go_y(7);
                d.go_x(9);
                d.open_shop_at(4, 7);
                d.buy_potions(4);
                d.close_shop();
            },
        );
        // Surface from the hall, then walk the road east.
        if driver.world.current_map.as_str().starts_with("skald_hall") {
            driver.go_x(6);
            driver.go_y(0);
        }
        if n < 4 {
            driver.go_y(7);
            driver.go_x(driver.world.map().width - 1);
        }
    }
    assert!(
        driver.has_flag("skalden.badge.4"),
        "the folk circuit closes"
    );
}
