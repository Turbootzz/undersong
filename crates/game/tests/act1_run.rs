//! P5 acceptance gate: new game → Badge 3 → the Lull scene, driven
//! through the pure core, recorded to tests/replays/act1_complete.ron
//! (UPDATE_REPLAYS=1) and played back by the second test.

mod common;

use std::path::{Path, PathBuf};

use common::Driver;
use game::world::{Input, load_game_world};
use undersong_core::world::Facing::{Down, Left, Right, Up};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

const SEED: u64 = 0x00AC_71AC;

#[expect(clippy::too_many_lines, reason = "one continuous scripted run")]
fn run_act1() -> Driver {
    let world = load_game_world(&root().join("content"), SEED).expect("game world loads");
    let mut driver = Driver::new(world);

    // ---- Badge 1 (the P3 route, adaptive) --------------------------------
    driver.walk(&[(Left, 2)]); // (6,4)
    driver.go_y(9); // lab door → pausa_lab (4,1)
    assert_eq!(driver.world.current_map.as_str(), "pausa_lab");
    driver.go_y(3); // intro fires at (4,2)
    driver.interact(); // starter choice → fanfyre
    assert!(driver.has_flag("starter.fanfyre"));
    driver.go_y(0); // exit → pausa (6,8)
    driver.go_x(9);
    driver.go_y(13); // → route_1 (6,1)
    assert_eq!(driver.world.current_map.as_str(), "route_1");
    driver.go_y(13); // tuner + busker engage en route
    driver.walk(&[(Up, 1), (Left, 2)]); // (4,14) patch field
    // Catch a teammate, then grind to 11 for Hall 1.
    for _ in 0..400 {
        if driver.world.party.len() >= 2 {
            break;
        }
        if driver.world.battle.is_some() {
            if driver
                .world
                .bag
                .get(&"fermata".into())
                .copied()
                .unwrap_or(0)
                > 0
            {
                driver.input(Input::Battle(game::session::BattleCmd::Bell));
            } else {
                driver.drain();
            }
            continue;
        }
        if driver.world.dialogue.is_some()
            || !driver.world.pending_learn_queue.is_empty()
            || !driver.world.pending_evolutions.is_empty()
        {
            driver.drain();
            continue;
        }
        let dir = if driver.world.player.1.is_multiple_of(2) {
            Up
        } else {
            Down
        };
        driver.face(dir);
        driver.input(Input::Step(dir));
    }
    assert!(driver.world.party.len() >= 2, "teammate attuned");
    driver.grind_until(13, |d| {
        if d.world.current_map.as_str() == "pausa_village" {
            // whiteout recovery: pausa spawn → route_1 patch field
            d.go_x(9);
            d.go_y(13); // gate → route_1 (6,1)
            d.go_y(14);
            d.go_x(4);
        } else if d.world.current_map.as_str() == "route_1"
            && !((2..=4).contains(&d.world.player.0) && (14..=18).contains(&d.world.player.1))
        {
            let x = d.world.player.0;
            if (5..=8).contains(&x) {
                d.go_y(14);
                d.go_x(4);
            }
        }
    });
    driver.go_x(6);
    driver.go_y(28); // percussionist, choirboy, rival1 en route
    assert!(driver.has_flag("story.rival1.defeated"));
    driver.go_y(29); // → prelude (10,1)
    driver.go_y(7); // rest stop heals
    driver.go_x(16);
    driver.go_y(8); // hall_1 (6,1)
    assert_eq!(driver.world.current_map.as_str(), "hall_1");
    driver.go_y(5);
    driver.walk(&[(Left, 1), (Down, 1)]); // aide sight
    driver.walk(&[(Up, 1), (Right, 4), (Up, 3)]); // (9,8)
    driver.walk(&[(Left, 2), (Down, 1)]); // senior sight
    // Duck out to the rest stop before the Maestro — the grind and the
    // hall pair leave nothing in the tank.
    driver.walk(&[(Up, 1)]); // back to (7,8)
    driver.go_x(9);
    driver.go_y(5);
    driver.go_x(6);
    driver.go_y(0); // hall door → prelude (16,7)
    driver.go_x(10); // rest stop heals at (10,7)
    driver.go_x(16);
    driver.go_y(8); // → hall_1 (6,1)
    driver.go_y(5);
    driver.go_x(9); // aide/senior already beaten — clean S-path
    driver.go_y(8);
    driver.go_x(2);
    driver.go_y(10);
    driver.go_x(6);
    driver.go_y(12);
    driver.interact(); // Dario → badge.1
    assert!(
        driver.has_flag("badge.1"),
        "badge1: map {} at {:?} aide {} senior {} party {:?}",
        driver.world.current_map,
        driver.world.player,
        driver.has_flag("trainer.hall_aide.defeated"),
        driver.has_flag("trainer.hall_senior.defeated"),
        driver
            .world
            .party
            .iter()
            .map(|p| format!("{} L{} hp{:?}", p.species, p.level, p.hp))
            .collect::<Vec<_>>(),
    );

    // ---- Beat 1: the theft, back at the lab ------------------------------
    // Reverse the hall_1 S-path: (6,12) → left corridor → bottom door.
    driver.walk(&[(Down, 2)]);
    driver.go_x(2);
    driver.go_y(8);
    driver.go_x(9);
    driver.go_y(5);
    driver.go_x(6);
    driver.go_y(0); // hall → prelude (16,7)
    driver.go_x(10);
    driver.go_y(0); // → route_1 (6,28)
    driver.go_y(0); // walk south the whole route → pausa (9,12)
    driver.go_y(8); // descend beside the lab block
    driver.go_x(6);
    driver.go_y(9); // lab door from below → lab (4,1)
    assert_eq!(driver.world.current_map.as_str(), "pausa_lab");
    driver.walk(&[(Up, 1), (Left, 1), (Up, 1)]); // (3,2): theft trigger
    driver.drain();
    assert!(driver.has_flag("story.theft.seen"), "theft scene fired");
    driver.go_x(4); // the lab exit door sits on x4
    driver.go_y(0); // back out to pausa (6,8)

    // ---- Route 2 → Arbor Vale (beat 3 shipment, hall 2) -------------------
    driver.go_y(6);
    driver.go_x(0); // west gate → route_2 (1,6)
    assert_eq!(driver.world.current_map.as_str(), "route_2");

    // Grind to 17 on the west field FIRST — the shipment grunts and
    // Mirelle's T3 both expect a real party.
    driver.go_x(6);
    driver.go_y(3); // into the patch field (4..8 × 2..4)
    driver.grind_until(17, |d| {
        if d.world.current_map.as_str() == "prelude_town" {
            // whiteout → prelude rest point: walk back west.
            d.go_x(10);
            d.go_y(0); // → route_1 (6,28)
            d.go_y(0); // south → pausa (9,12)
            d.go_y(6);
            d.go_x(0); // west gate → route_2 (1,6)
            d.go_x(6);
            d.go_y(3);
        } else if d.world.current_map.as_str() == "pausa_village" {
            d.go_y(6);
            d.go_x(0);
            d.go_x(6);
            d.go_y(3);
        } else if d.world.current_map.as_str() == "route_2"
            && !((4..=8).contains(&d.world.player.0) && (2..=4).contains(&d.world.player.1))
        {
            d.go_y(6);
            d.go_x(6);
            d.go_y(3);
        }
    });
    // The grind may end on a whiteout — normalize back to route_2.
    for _ in 0..3 {
        match driver.world.current_map.as_str() {
            "prelude_town" => {
                driver.go_x(10);
                driver.go_y(0);
                driver.go_y(0);
                driver.go_y(6);
                driver.go_x(0);
            }
            "pausa_village" => {
                driver.go_y(6);
                driver.go_x(0);
            }
            _ => break,
        }
    }
    assert_eq!(driver.world.current_map.as_str(), "route_2");
    // Rest at Mom's doorstep before the gauntlet — the grind drains PP
    // and Last-Resort recoil loses winnable fights.
    driver.go_y(6);
    driver.go_x(1);
    driver.go_x(0); // hop the west gate back → pausa (1,6)
    if driver.world.current_map.as_str() == "pausa_village" {
        driver.go_x(13);
        driver.go_y(8); // home_rest doorstep heals party + PP
        driver.go_y(6);
        driver.go_x(0); // back west → route_2 (1,6)
    }
    assert_eq!(driver.world.current_map.as_str(), "route_2");
    driver.go_y(7);
    driver.go_x(10); // pass behind the gardener on y7
    driver.go_y(6); // step into his sight line at (10,6)
    driver.drain();
    assert!(
        driver.has_flag("trainer.rt2_gardener.defeated"),
        "gardener: map {} at {:?} party {:?}",
        driver.world.current_map,
        driver.world.player,
        driver
            .world
            .party
            .iter()
            .map(|p| format!("{} L{} hp{:?}", p.species, p.level, p.hp))
            .collect::<Vec<_>>(),
    );
    driver.go_x(12); // TACET shipment row (y5..8)
    driver.drain();
    assert!(
        driver.has_flag("story.tacet.shipment"),
        "shipment: map {} at {:?}, party {:?}, fought {}",
        driver.world.current_map,
        driver.world.player,
        driver
            .world
            .party
            .iter()
            .map(|p| format!("{} L{} hp{:?}", p.species, p.level, p.hp))
            .collect::<Vec<_>>(),
        driver.has_flag("trainer.tacet_grunt_r2.defeated"),
    );
    driver.go_y(7);
    driver.go_x(16);
    driver.go_x(19); // courier sight (17..19,7)
    driver.drain();
    driver.go_y(6); // around the courier's tile
    driver.go_x(29); // east door → arbor_vale (1,7)
    assert_eq!(driver.world.current_map.as_str(), "arbor_vale");

    driver.go_x(10);
    driver.go_y(9); // rest stop doorstep heals
    driver.go_x(17);
    driver.go_y(10); // hall_2 door → (6,1)
    assert_eq!(driver.world.current_map.as_str(), "hall_2");
    driver.go_y(4);
    driver.walk(&[(Left, 1)]); // (5,4): pruner sight (4,4),(5,4)
    assert!(driver.has_flag("trainer.hall2_pruner.defeated"));
    driver.go_x(7);
    driver.go_y(7); // (7,7): arranger sight
    assert!(driver.has_flag("trainer.hall2_arranger.defeated"));
    // Rest in town before the Maestro (attrition discipline).
    driver.go_y(1);
    driver.go_x(6);
    driver.go_y(0); // → arbor (17,9)
    driver.go_x(10); // rest doorstep heals
    driver.go_x(17);
    driver.go_y(10); // → hall_2 (6,1)
    driver.go_y(5);
    driver.go_x(7);
    driver.go_y(8);
    driver.go_x(3);
    driver.go_y(10);
    driver.go_x(6);
    driver.go_y(12);
    driver.interact(); // Mirelle (T3)
    assert!(
        driver.has_flag("badge.2"),
        "Mirelle beaten; party {:?}",
        driver
            .world
            .party
            .iter()
            .map(|p| format!("{} L{} hp{:?}", p.species, p.level, p.hp))
            .collect::<Vec<_>>()
    );
    assert!(driver.has_flag("performance.clearing_chord"));

    // ---- Route 3 (rival 2) → Route 4 (TACET doubles) → Calando ------------
    // Reverse the hall_2 maze: (6,12) → x3 → y8 → x7 → bottom door.
    driver.go_x(3);
    driver.go_y(8);
    driver.go_x(7);
    driver.go_y(1);
    driver.go_x(6);
    driver.go_y(0); // hall → arbor (17,9)
    driver.go_x(13); // free lane between rest stop and hall
    driver.go_y(14);
    driver.go_x(12);
    driver.go_y(15); // north gate → route_3 (6,1)
    assert_eq!(driver.world.current_map.as_str(), "route_3");

    // Train the second slot: bench the lead, grind the partner to 16 on
    // route_3's west field, then bring the lead back (it returns to the
    // rear slot — the trained partner now leads).
    driver.input(Input::BoxDeposit { party_index: 0 });
    driver.go_y(5);
    driver.go_x(3); // patch field (2..4 × 4..8)
    driver.grind_until(16, |d| {
        if d.world.current_map.as_str() == "arbor_vale" {
            d.go_x(13);
            d.go_y(14);
            d.go_x(12);
            d.go_y(15); // → route_3
            d.go_y(5);
            d.go_x(3);
        } else if d.world.current_map.as_str() == "route_3"
            && !((2..=4).contains(&d.world.player.0) && (4..=8).contains(&d.world.player.1))
        {
            d.go_x(6);
            d.go_y(5);
            d.go_x(3);
        }
    });
    driver.input(Input::BoxWithdraw { box_index: 0 });
    assert_eq!(driver.world.party.len(), 2);
    for _ in 0..3 {
        match driver.world.current_map.as_str() {
            "arbor_vale" => {
                driver.go_x(13);
                driver.go_y(14);
                driver.go_x(12);
                driver.go_y(15);
            }
            _ => break,
        }
    }
    assert_eq!(driver.world.current_map.as_str(), "route_3");
    // Flip the order: the L21 lead takes Cade's counter-pair head-on.
    driver.input(Input::BoxDeposit { party_index: 0 });
    driver.input(Input::BoxWithdraw {
        box_index: u32::try_from(driver.world.boxes.len() - 1).unwrap_or(0),
    });

    // Recovery: from wherever a loss dumped us, rest+restock in Arbor
    // and stand back on route_3.
    fn back_to_route3(d: &mut Driver) {
        for _ in 0..4 {
            match d.world.current_map.as_str() {
                "arbor_vale" => {
                    d.go_x(13);
                    d.go_y(9);
                    d.go_x(10); // rest heals
                    d.open_shop_at(5, 9);
                    d.buy("potion_m", 2);
                    d.close_shop();
                    d.go_y(8);
                    d.go_x(13);
                    d.go_y(14);
                    d.go_x(12);
                    d.go_y(15); // → route_3
                }
                "prelude_town" => {
                    d.go_x(10);
                    d.go_y(0);
                    d.go_y(0);
                    d.go_y(6);
                    d.go_x(0); // → route_2 (long way home)
                    d.go_y(6);
                    d.go_x(29); // → arbor
                }
                "route_3" => {
                    d.go_x(6);
                    return;
                }
                _ => return,
            }
        }
    }

    driver.until_flag(
        "trainer.rt3_chorister.defeated",
        4,
        |d| {
            if d.world.current_map.as_str() == "route_3" {
                d.go_x(6);
                d.go_y(15); // drover (y6) engages en route; stop level
                if !d.has_flag("trainer.rt3_chorister.defeated") {
                    // Sight engages once only — re-challenges are direct.
                    d.go_x(8);
                    d.face(Right);
                    d.interact();
                }
            }
        },
        back_to_route3,
    );
    driver.until_flag(
        "story.rival2.defeated",
        4,
        |d| {
            if d.world.current_map.as_str() == "route_3" {
                d.go_x(6);
                d.go_y(20); // the rival row
            }
        },
        back_to_route3,
    );
    back_to_route3(&mut driver);
    driver.go_y(25); // → route_4 (6,1)
    assert_eq!(driver.world.current_map.as_str(), "route_4");

    // Route 4's fields run L14–18 — attune a third voice, then train
    // to 21 before the gauntlet.
    driver.go_y(6);
    driver.go_x(10); // patch field (9..11 × 5..9)
    for _ in 0..400 {
        if driver.world.party.len() >= 3 {
            break;
        }
        if driver.world.battle.is_some() {
            if driver
                .world
                .bag
                .get(&"fermata".into())
                .copied()
                .unwrap_or(0)
                > 0
            {
                driver.input(Input::Battle(game::session::BattleCmd::Bell));
            } else {
                driver.drain();
            }
            continue;
        }
        if driver.world.dialogue.is_some()
            || !driver.world.pending_learn_queue.is_empty()
            || !driver.world.pending_evolutions.is_empty()
        {
            driver.drain();
            continue;
        }
        if driver.world.current_map.as_str() != "route_4" {
            break; // whiteout — the grind retrek below recovers
        }
        let dir = if driver.world.player.1.is_multiple_of(2) {
            Up
        } else {
            Down
        };
        driver.face(dir);
        driver.input(Input::Step(dir));
    }
    driver.grind_until(23, |d| {
        if d.world.current_map.as_str() == "arbor_vale" {
            d.go_x(13);
            d.go_y(14);
            d.go_x(12);
            d.go_y(15); // → route_3
            d.go_x(6);
            d.go_y(25); // → route_4
            d.go_y(6);
            d.go_x(10);
        } else if d.world.current_map.as_str() == "route_4"
            && !((9..=11).contains(&d.world.player.0) && (5..=9).contains(&d.world.player.1))
        {
            d.go_x(6);
            d.go_y(6);
            d.go_x(10);
        }
    });
    // Recovery for the route_4 gauntlet: rest+restock in Arbor, walk
    // back north (rival/chorister rows are inert once beaten).
    fn back_to_route4(d: &mut Driver) {
        for _ in 0..4 {
            match d.world.current_map.as_str() {
                "arbor_vale" => {
                    d.go_x(13);
                    d.go_y(9);
                    d.go_x(10); // rest heals
                    d.open_shop_at(5, 9);
                    d.buy("potion_m", 2);
                    d.buy("potion_s", 2);
                    d.close_shop();
                    d.go_y(8);
                    d.go_x(13);
                    d.go_y(14);
                    d.go_x(12);
                    d.go_y(15); // → route_3
                    d.go_x(6);
                    d.go_y(25); // → route_4
                }
                "prelude_town" => {
                    d.go_x(10);
                    d.go_y(0);
                    d.go_y(0);
                    d.go_y(6);
                    d.go_x(0);
                    d.go_y(6);
                    d.go_x(29); // → arbor
                }
                "route_3" => {
                    d.go_x(6);
                    d.go_y(25);
                }
                "route_4" => {
                    d.go_x(6);
                    return;
                }
                _ => return,
            }
        }
    }

    back_to_route4(&mut driver);
    assert_eq!(driver.world.current_map.as_str(), "route_4");
    driver.until_flag(
        "trainer.rt4_stoker.defeated",
        4,
        |d| {
            if d.world.current_map.as_str() == "route_4" {
                d.go_x(6);
                d.go_y(8); // stoker sight (5..7,8)
                if !d.has_flag("trainer.rt4_stoker.defeated") {
                    d.go_x(5);
                    d.face(Left);
                    d.interact();
                }
            }
        },
        back_to_route4,
    );
    driver.until_flag(
        "trainer.rt4_signaler.defeated",
        4,
        |d| {
            if d.world.current_map.as_str() == "route_4" {
                d.go_x(6);
                d.go_y(12); // signaler sight (6..8,12)
                if !d.has_flag("trainer.rt4_signaler.defeated") {
                    d.go_x(8);
                    d.face(Right);
                    d.interact();
                }
            }
        },
        back_to_route4,
    );
    driver.until_flag(
        "story.tacet.yard",
        4,
        |d| {
            if d.world.current_map.as_str() == "route_4" {
                d.go_x(6);
                d.go_y(18); // the yard row refires until won
                d.drain();
            }
        },
        back_to_route4,
    );
    driver.go_y(21); // → port_calando (11,1)
    assert_eq!(driver.world.current_map.as_str(), "port_calando");

    // Rest, keyshift scene, stock up, hall 3.
    driver.go_y(11);
    driver.go_x(9); // rest doorstep
    driver.go_x(4); // mart doorstep → shop opens via dialogue
    driver.drain();
    // Buy potions for Bram (stock is sorted; find potion_m adaptively).
    driver.input(Input::Step(Down));
    driver.input(Input::Step(Down));
    driver.input(Input::Step(Up));
    driver.input(Input::Step(Up));
    while driver.world.dialogue.is_some() {
        driver.input(Input::Interact);
    }
    if driver.world.shop.is_some() {
        let stock: Vec<String> = driver
            .world
            .shop
            .as_ref()
            .map(|(items, _)| items.iter().map(ToString::to_string).collect())
            .unwrap_or_default();
        if let Some(target) = stock.iter().position(|s| s == "potion_m") {
            loop {
                let cursor = driver.world.shop.as_ref().map(|(_, c)| *c).unwrap_or(0);
                match cursor.cmp(&target) {
                    std::cmp::Ordering::Less => driver.input(Input::ShopCursor(1)),
                    std::cmp::Ordering::Greater => driver.input(Input::ShopCursor(-1)),
                    std::cmp::Ordering::Equal => break,
                };
            }
            for _ in 0..4 {
                driver.input(Input::ShopBuy);
            }
        }
        driver.input(Input::ShopClose);
    }
    driver.go_y(6);
    driver.go_x(19);
    driver.face(Right);
    driver.interact(); // keyshift collector at (20,6)
    assert!(driver.has_flag("story.keyshift.seen"));
    driver.go_x(19);
    driver.go_y(11);
    driver.go_y(12); // hall_3 door → (6,1)
    assert_eq!(driver.world.current_map.as_str(), "hall_3");

    // Hall 3: the doubles gauntlet.
    driver.go_y(5);
    driver.walk(&[(Left, 1)]); // (5,5): duo_a sight
    assert!(driver.has_flag("trainer.hall3_duo_a.defeated"));
    // Rest between the duos (doubles attrition is real).
    driver.go_x(6);
    driver.go_y(0); // → calando (19,11)
    driver.go_x(9); // rest doorstep heals
    driver.go_x(19);
    driver.go_y(12); // → hall_3 (6,1)
    driver.go_y(5);
    driver.go_x(9);
    driver.go_y(9); // (9,9): duo_b sight
    assert!(driver.has_flag("trainer.hall3_duo_b.defeated"));
    // One more rest before the Maestro (descend the x9 channel), and
    // spend the duo payouts on tonics.
    driver.go_y(8);
    driver.go_x(9);
    driver.go_y(5);
    driver.go_x(6);
    driver.go_y(0); // → calando
    driver.go_x(9); // rest heals
    driver.open_shop_at(4, 11);
    driver.buy("potion_m", 4);
    driver.close_shop();
    driver.go_y(11);
    driver.go_x(19);
    driver.go_y(12); // → hall_3
    driver.go_y(5);
    driver.go_x(9);
    driver.go_y(10);
    driver.go_x(3);
    driver.go_y(12);
    driver.go_x(7);
    driver.go_y(14);
    driver.interact(); // Maestro Bram (doubles, T3)
    assert!(
        driver.has_flag("badge.3"),
        "Bram beaten; party {:?}",
        driver
            .world
            .party
            .iter()
            .map(|p| format!("{} L{} hp{:?}", p.species, p.level, p.hp))
            .collect::<Vec<_>>()
    );

    // ---- Beat 5: Lull, on the way out (reverse the gauntlet maze) ---------
    driver.go_x(3);
    driver.go_y(10);
    driver.go_x(9);
    driver.go_y(5);
    driver.go_x(6);
    driver.go_y(2); // the lull_scene trigger
    driver.drain();
    assert!(
        driver.has_flag("story.lull.done"),
        "lull: map {} at {:?} fought {} party {:?}",
        driver.world.current_map,
        driver.world.player,
        driver.has_flag("story.lull.defeated"),
        driver
            .world
            .party
            .iter()
            .map(|p| format!("{} L{} hp{:?}", p.species, p.level, p.hp))
            .collect::<Vec<_>>(),
    );

    driver.input(Input::Save);
    driver
}

#[test]
fn act1_reaches_badge_three_and_lull() {
    let driver = run_act1();
    if std::env::var_os("UPDATE_REPLAYS").is_some() {
        let file = game::replay::ReplayFile {
            seed: SEED,
            world: game::replay::WorldKind::Game,
            inputs: driver.log.clone(),
            expect: game::replay::Expectations {
                map: driver.world.current_map.to_string(),
                position: driver.world.player,
                flags: [
                    "badge.1",
                    "badge.2",
                    "badge.3",
                    "story.theft.seen",
                    "story.tacet.shipment",
                    "story.tacet.yard",
                    "story.rival2.defeated",
                    "story.keyshift.seen",
                    "story.lull.done",
                    "performance.clearing_chord",
                    "performance.tunneling_bass",
                ]
                .into_iter()
                .map(String::from)
                .collect(),
                min_dialogue_lines: driver.dialogue_lines,
                warped: true,
                party_levels: Some(driver.world.party.iter().map(|p| p.level).collect()),
                money: Some(driver.world.money),
            },
        };
        let text = ron::ser::to_string_pretty(&file, ron::ser::PrettyConfig::default())
            .expect("serialize");
        std::fs::write(
            root().join("tests/replays/act1_complete.ron"),
            format!(
                "// P5 gate replay — recorded by act1_run.rs (UPDATE_REPLAYS=1).\n// {} inputs; do not hand-edit.\n{text}\n",
                file.inputs.len()
            ),
        )
        .expect("write replay");
    }
}

#[test]
fn recorded_act1_replay_plays_back() {
    if std::env::var_os("UPDATE_REPLAYS").is_some() {
        return;
    }
    let path = root().join("tests/replays/act1_complete.ron");
    if !path.exists() {
        panic!("missing act1 replay — run UPDATE_REPLAYS=1 cargo test -p game --test act1_run");
    }
    let outcome = game::replay::run_replay_file(&root().join("content"), &path)
        .expect("the recorded act-1 run must replay cleanly");
    assert!(outcome.warps >= 12, "the run crosses many gates");
}
