//! P6 acceptance gates: three full-game replays, one per ending. One
//! shared runbook drives new game → Badge 8 → Quartet → the Vault floor
//! choice; the recordings diverge only at the finale (and the Chorus
//! run additionally clears all eight echoes and 60% of the Score).

mod common;

use common::{ACT_SEED, Driver, content_root};
use game::world::Input;
use undersong_core::world::Facing::{Down, Right, Up};

fn hall_climb(d: &mut Driver, hall: &str) {
    // Halls 4/7/8 share the two-wall S layout (walls y6 x2..7 and
    // y10 x4..9). Every leg re-checks the map: a mid-climb whiteout
    // must not keep walking the pattern in the wrong town.
    macro_rules! leg {
        ($call:expr) => {
            if d.world.current_map.as_str() != hall {
                return;
            }
            $call;
        };
    }
    if d.world.player.1 > 10 {
        return;
    }
    leg!(d.go_x(6));
    leg!(d.go_y(5));
    leg!(d.go_x(8));
    leg!(d.go_y(9));
    leg!(d.go_x(3));
    leg!(d.go_y(11));
    leg!(d.go_x(6));
}

/// Convert payout cash into potions immediately — items survive
/// whiteouts; the wallet doesn't.
fn bank(d: &mut Driver, town_mart: (u32, u32)) {
    d.open_shop_at(town_mart.0, town_mart.1);
    d.buy_potions(5);
    d.close_shop();
}

/// Mirror of hall_climb: from the maestro back to the door.
fn hall_descend(d: &mut Driver, hall: &str) {
    macro_rules! leg {
        ($call:expr) => {
            if d.world.current_map.as_str() != hall {
                return;
            }
            $call;
        };
    }
    if d.world.player.1 > 10 {
        leg!(d.go_x(3));
        leg!(d.go_y(9));
    }
    if d.world.player.1 > 6 {
        leg!(d.go_x(8));
        leg!(d.go_y(5));
    }
    leg!(d.go_x(6));
    leg!(d.go_y(0));
}

#[expect(clippy::too_many_lines, reason = "one continuous scripted run")]
fn run_to_finale(chorus_prep: bool) -> Driver {
    let mut driver = common::run_act1();

    // ---------------- Badge 4: Voltaccia --------------------------------
    // From hall_3's exit area, surface to Calando and ride Route 5 north.
    fn back_to_calando(d: &mut Driver) {
        for _ in 0..4 {
            match d.world.current_map.as_str() {
                "hall_3" => {
                    d.go_x(6);
                    d.go_y(0);
                }
                "port_calando" => {
                    d.go_y(11);
                    d.go_x(9); // rest heals
                    return;
                }
                _ => return,
            }
        }
    }
    back_to_calando(&mut driver);

    // Grind to 27 on route_5's west field before Rhea.
    if driver.world.current_map.as_str() == "port_calando" {
        driver.go_x(12); // the free lane east of the rest stop
        driver.go_y(16); // → route_5 (7,1)
    }
    assert_eq!(
        driver.world.current_map.as_str(),
        "route_5",
        "at {:?}",
        driver.world.player
    );
    driver.go_y(6);
    driver.go_x(3);
    driver.grind_until(34, |d| {
        if d.world.current_map.as_str() == "port_calando" {
            d.go_y(11);
            d.go_x(9);
            d.go_x(12);
            d.go_y(16);
            d.go_y(6);
            d.go_x(3);
        } else if d.world.current_map.as_str() == "route_5"
            && !((2..=4).contains(&d.world.player.0) && (4..=8).contains(&d.world.player.1))
        {
            d.go_x(6);
            d.go_y(6);
            d.go_x(3);
        }
    });

    fn back_to_voltaccia(d: &mut Driver) {
        for _ in 0..5 {
            match d.world.current_map.as_str() {
                "port_calando" => {
                    d.go_y(11);
                    d.go_x(9); // rest
                    d.open_shop_at(4, 11);
                    d.buy_potions(4);
                    d.close_shop();
                    d.go_y(11);
                    d.go_x(12);
                    d.go_y(16); // → route_5
                }
                "route_5" => {
                    d.go_y(10); // clear the linesman's row first
                    d.go_x(6);
                    d.go_y(23); // → voltaccia (11,1)
                }
                "voltaccia" => {
                    d.go_y(9);
                    d.go_x(9); // rest doorstep
                    return;
                }
                "hall_4" => {
                    hall_descend(d, "hall_4");
                }
                _ => return,
            }
        }
    }
    back_to_voltaccia(&mut driver);
    driver.until_flag(
        "badge.4",
        6,
        |d| {
            if d.world.current_map.as_str() == "voltaccia" {
                d.go_y(9);
                d.go_x(16);
                d.go_y(10); // → hall_4 (6,1)
            }
            if d.world.current_map.as_str() == "hall_4" {
                hall_climb(d, "hall_4");
                if d.world.current_map.as_str() != "hall_4" {
                    return;
                }
                d.go_y(14);
                d.face(Up);
                d.interact(); // Rhea
            }
        },
        back_to_voltaccia,
    );
    if driver.world.current_map.as_str() == "hall_4" {
        hall_descend(&mut driver, "hall_4"); // surface to bank the payout
    }
    if driver.world.current_map.as_str() == "voltaccia" {
        driver.go_y(9);
        bank(&mut driver, (4, 9));
        driver.go_x(16);
        driver.go_y(10); // back into the hall for the basement door
    }

    // Badge 4's canonical reward: the Exp Share (doc 02 §9). Hang it on
    // the second slot so the backline stops fossilizing.
    driver.input(Input::UseItem {
        item: "exp_share".into(),
        target: 1,
        slot: None,
    });
    if std::env::var_os("DRIVER_DEBUG").is_some() {
        eprintln!(
            "  exp share equipped: bag {:?} holder {:?}",
            driver.world.bag.get(&"exp_share".into()),
            driver.world.party.get(1).and_then(|p| p.held_item.clone()),
        );
    }

    // Beat 6: the maintenance door, past the maestro floor.
    if driver.world.current_map.as_str() == "hall_4" {
        hall_climb(&mut driver, "hall_4");
        driver.go_x(3);
        driver.go_y(13);
        driver.go_x(2);
        driver.go_y(14);
        driver.drain();
    }
    assert!(driver.has_flag("story.door.seen"), "the maintenance door");

    // ---------------- Badge 5: Hollowfen --------------------------------
    fn back_to_hollowfen(d: &mut Driver) {
        for _ in 0..6 {
            match d.world.current_map.as_str() {
                "hall_4" => {
                    hall_descend(d, "hall_4");
                }
                "voltaccia" => {
                    d.go_y(9);
                    d.go_x(9); // rest
                    d.go_y(7);
                    d.go_x(23); // east gate → route_5b (P9 geography)
                }
                "route_5b" => {
                    d.go_y(7);
                    d.go_x(16); // lampman's sight row — fight him once
                    if d.world.current_map.as_str() != "route_5b" {
                        continue; // a loss mid-route re-dispatches
                    }
                    d.go_y(8); // then around his tile
                    d.go_x(24);
                    d.go_y(7);
                    d.go_x(25); // → hollowfen (1,7)
                }
                "hollowfen" => {
                    d.go_x(12); // free lane (works from both gates)
                    d.go_y(7);
                    d.go_x(9); // rest doorstep
                    d.open_shop_at(4, 7);
                    d.buy_potions(4);
                    d.close_shop();
                    d.go_y(7);
                    d.go_x(9);
                    return;
                }
                "hall_5" => {
                    if d.world.player.1 > 9 {
                        d.go_x(8);
                        d.go_y(8);
                    }
                    if d.world.player.1 > 6 && d.world.current_map.as_str() == "hall_5" {
                        d.go_x(2); // the prompter owns (3,7)
                        d.go_y(5);
                    }
                    if d.world.current_map.as_str() == "hall_5" {
                        d.go_x(6);
                        d.go_y(0);
                    }
                }
                _ => return,
            }
        }
    }
    back_to_hollowfen(&mut driver);
    if std::env::var_os("DRIVER_DEBUG").is_some() {
        eprintln!(
            "  pre-graven-grind: map {} at {:?} lead L{}",
            driver.world.current_map,
            driver.world.player,
            driver.world.party.first().map(|p| p.level).unwrap_or(0),
        );
    }
    // Maren's requiemoth sits at 36 — train on Graven's south field
    // first (the pass is open; only its hall needs the badge).
    if driver.world.current_map.as_str() == "hollowfen" {
        driver.go_y(7);
        driver.go_x(12); // free lane east of the rest stop
        driver.go_y(12);
        driver.go_x(10);
        driver.go_y(13); // → graven_pass (6,1)
    }
    if driver.world.current_map.as_str() == "graven_pass" {
        driver.go_y(8);
        driver.go_x(3);
        driver.grind_until(38, |d| {
            if d.world.current_map.as_str() == "hollowfen" {
                d.go_y(7);
                d.go_x(9);
                d.go_y(7);
                d.go_x(12); // free lane east of the rest stop
                d.go_y(12);
                d.go_x(10);
                d.go_y(13);
                d.go_y(8);
                d.go_x(3);
            } else if d.world.current_map.as_str() == "graven_pass"
                && !((2..=4).contains(&d.world.player.0) && (6..=10).contains(&d.world.player.1))
            {
                d.go_x(6);
                d.go_y(8);
                d.go_x(3);
            }
        });
        // back south to Hollowfen
        if driver.world.current_map.as_str() == "graven_pass" {
            driver.go_x(6);
            driver.go_y(0); // → hollowfen (10,12)
        }
    }
    back_to_hollowfen(&mut driver);
    driver.until_flag(
        "badge.5",
        6,
        |d| {
            if d.world.current_map.as_str() == "hollowfen" {
                d.go_y(7);
                d.go_x(16);
                if d.world.player == (16, 7) {
                    d.go_y(8); // → hall_5 (6,1)
                }
            }
            if d.world.current_map.as_str() == "hall_5" {
                macro_rules! leg {
                    ($call:expr) => {
                        if d.world.current_map.as_str() != "hall_5" {
                            return;
                        }
                        $call;
                    };
                }
                leg!(d.go_x(6));
                leg!(d.go_y(5));
                leg!(d.go_x(2)); // the prompter owns (3,7)
                leg!(d.go_y(8));
                leg!(d.go_x(8));
                leg!(d.go_y(12));
                leg!(d.go_x(6));
                leg!(d.go_y(12));
                leg!(d.face(Up));
                leg!(d.interact()); // Maren
            }
        },
        back_to_hollowfen,
    );
    assert!(driver.has_flag("story.maren.letter"), "the sealed letter");

    // ---------------- Beat 8 + Badge 6: Graven Pass ----------------------
    fn back_to_graven(d: &mut Driver) {
        for _ in 0..6 {
            match d.world.current_map.as_str() {
                "hall_5" => {
                    if d.world.player.1 > 9 {
                        d.go_x(8);
                        d.go_y(8);
                    }
                    if d.world.player.1 > 6 && d.world.current_map.as_str() == "hall_5" {
                        d.go_x(2); // the prompter owns (3,7)
                        d.go_y(5);
                    }
                    if d.world.current_map.as_str() == "hall_5" {
                        d.go_x(6);
                        d.go_y(0);
                    }
                }
                "hollowfen" => {
                    d.go_y(7);
                    d.go_x(9); // rest
                    d.go_y(7);
                    d.go_x(12);
                    d.go_y(12);
                    d.go_x(10);
                    d.go_y(13); // → graven (6,1)
                }
                "graven_pass" => {
                    d.go_x(6);
                    d.go_y(12); // waystation rest fires en route
                    return;
                }
                "hall_6" => {
                    if d.world.player.1 > 7 {
                        d.go_x(7); // y7 wall spans x2..6; mason owns (8,5)
                        d.go_y(4);
                    }
                    d.go_x(6);
                    d.go_y(0);
                }
                _ => return,
            }
        }
    }
    back_to_graven(&mut driver);
    // Grind to 38 on the south field before the anchor scene + Orsk.
    driver.go_y(8);
    driver.go_x(3);
    driver.grind_until(38, |d| {
        if d.world.current_map.as_str() == "hollowfen" {
            d.go_y(7);
            d.go_x(9);
            d.go_y(7);
            d.go_x(10);
            d.go_y(13);
            d.go_y(8);
            d.go_x(3);
        } else if d.world.current_map.as_str() == "graven_pass"
            && !((2..=4).contains(&d.world.player.0) && (6..=10).contains(&d.world.player.1))
        {
            d.go_x(6);
            d.go_y(8);
            d.go_x(3);
        }
    });
    driver.until_flag(
        "story.roster.read",
        6,
        |d| {
            if d.world.current_map.as_str() == "graven_pass" {
                d.go_x(6);
                d.go_y(16); // the anchor row: Hush (doubles)
                d.drain();
            }
        },
        back_to_graven,
    );
    driver.until_flag(
        "badge.6",
        6,
        |d| {
            if d.world.current_map.as_str() == "graven_pass" {
                d.go_x(6);
                d.go_y(19);
                d.go_y(20); // hall_6 door at (6,20)
            }
            if d.world.current_map.as_str() == "hall_6" {
                d.go_x(6);
                d.go_y(6);
                d.go_x(8);
                d.go_y(8);
                d.go_x(6);
                d.go_y(11);
                d.face(Up);
                d.interact(); // Orsk
            }
        },
        back_to_graven,
    );

    // ---------------- Badge 7: Frostine (Ilva) ---------------------------
    fn back_to_frostine(d: &mut Driver) {
        for iteration in 0..6 {
            if std::env::var_os("DRIVER_DEBUG").is_some() {
                eprintln!(
                    "  b2f[{iteration}]: {} {:?}",
                    d.world.current_map, d.world.player
                );
            }
            match d.world.current_map.as_str() {
                "hall_6" => {
                    if d.world.player.1 > 7 {
                        d.go_x(7); // y7 wall spans x2..6; mason owns (8,5)
                        d.go_y(4);
                    }
                    d.go_x(6);
                    d.go_y(0);
                }
                "graven_pass" => {
                    if d.world.player.1 < 20 {
                        d.go_y(19);
                        d.go_x(8); // the lane past the hall
                        d.go_y(24);
                    }
                    d.go_x(6);
                    d.go_y(27); // → frostine (11,1)
                }
                "frostine" => {
                    d.go_y(7);
                    d.go_x(9); // rest
                    d.open_shop_at(4, 7);
                    d.buy_potions(4);
                    d.buy_potions(2);
                    d.close_shop();
                    return;
                }
                "hall_7" => {
                    hall_descend(d, "hall_7");
                }
                _ => return,
            }
        }
    }
    back_to_frostine(&mut driver);
    // Ilva is built to be lost to: grind to 47 on graven's north field.
    driver.go_x(11);
    driver.go_y(0); // back into graven (6,26)
    if driver.world.current_map.as_str() == "graven_pass" {
        driver.go_x(10);
        driver.go_y(16);
        driver.grind_until(47, |d| {
            if d.world.current_map.as_str() == "frostine" {
                d.go_y(7);
                d.go_x(9);
                d.go_x(11);
                d.go_y(0);
                d.go_x(10);
                d.go_y(16);
            } else if d.world.current_map.as_str() == "graven_pass"
                && !((9..=11).contains(&d.world.player.0) && (14..=18).contains(&d.world.player.1))
            {
                d.go_x(6);
                d.go_y(15);
                d.go_x(10);
                d.go_y(16);
            }
        });
        driver.go_x(6);
        driver.go_y(27); // → frostine
    }
    back_to_frostine(&mut driver);
    driver.until_flag(
        "badge.7",
        8,
        |d| {
            if d.world.current_map.as_str() == "frostine" {
                d.go_y(7);
                d.go_x(16);
                d.go_y(8); // → hall_7 (6,1)
            }
            if d.world.current_map.as_str() == "hall_7" {
                hall_climb(d, "hall_7");
                if d.world.current_map.as_str() != "hall_7" {
                    return;
                }
                d.go_y(13);
                d.face(Up);
                d.interact(); // Ilva
            }
        },
        back_to_frostine,
    );

    // ---------------- Badge 8: Cadenza (Calder) --------------------------
    fn back_to_cadenza(d: &mut Driver) {
        for _ in 0..6 {
            match d.world.current_map.as_str() {
                "hall_7" => {
                    hall_descend(d, "hall_7");
                }
                "frostine" => {
                    d.go_y(7);
                    d.go_x(9); // rest
                    d.go_y(7);
                    d.go_x(12); // free lane east of the rest stop
                    d.go_y(13); // → route_6 (7,1)
                }
                "route_6" => {
                    d.go_x(6);
                    d.go_y(19); // → cadenza (12,1)
                }
                "cadenza_city" => {
                    d.go_y(11);
                    d.go_x(9); // rest
                    return;
                }
                "hall_8" => {
                    hall_descend(d, "hall_8");
                }
                _ => return,
            }
        }
    }
    back_to_cadenza(&mut driver);
    // Reed's confession lands once the Roster is read (it is).
    driver.go_y(7);
    driver.go_x(12);
    driver.face(Right); // Reed stands at (13,7)
    driver.interact();
    driver.answer_choice(1); // mercy — she was carrying it alone
    driver.drain();
    assert!(
        driver.has_flag("story.reed.confessed"),
        "confession: map {} at {:?}",
        driver.world.current_map,
        driver.world.player,
    );
    driver.until_flag(
        "badge.8",
        8,
        |d| {
            if d.world.current_map.as_str() == "cadenza_city" {
                d.go_y(11);
                d.go_x(18);
                d.go_y(12); // → hall_8 (6,1)
            }
            if d.world.current_map.as_str() == "hall_8" {
                hall_climb(d, "hall_8");
                if d.world.current_map.as_str() != "hall_8" {
                    return;
                }
                d.go_y(13);
                d.face(Up);
                d.interact(); // Provost Calder
            }
        },
        back_to_cadenza,
    );

    // Beat 14: Cade's plea, the night before.
    back_to_cadenza(&mut driver);
    driver.go_y(6);
    driver.go_x(16);
    driver.face(Right); // Cade paces at (17,6)
    driver.interact();
    driver.drain();
    assert!(driver.has_flag("story.cade.plea"), "the plea");

    // ---------------- The Quartet ----------------------------------------
    fn back_to_spire(d: &mut Driver) {
        for _ in 0..5 {
            match d.world.current_map.as_str() {
                "cadenza_city" => {
                    d.go_y(11);
                    d.go_x(9); // rest
                    d.open_shop_at(4, 11);
                    d.buy_potions(4);
                    d.close_shop();
                    d.go_y(9);
                    d.go_x(24); // → spire (5,1)
                }
                "quartet_spire" => return,
                "hall_8" => {
                    hall_descend(d, "hall_8");
                }
                _ => return,
            }
        }
    }
    for (flag, y, x) in [
        ("story.quartet.1.defeated", 5u32, 4u32),
        ("story.quartet.2.defeated", 11, 5),
        ("story.quartet.3.defeated", 17, 4),
        ("story.quartet.4.defeated", 23, 5),
    ] {
        back_to_spire(&mut driver);
        driver.until_flag(
            flag,
            8,
            move |d| {
                if d.world.current_map.as_str() == "quartet_spire" {
                    d.go_x(x);
                    d.go_y(y);
                    d.face(Up);
                    d.interact();
                }
            },
            back_to_spire,
        );
    }

    // The empty stage. The floor opens. (x3 is the only lane the
    // four chairs don't occupy.)
    driver.go_x(3);
    driver.go_y(27);
    driver.go_x(4);
    driver.go_y(28);
    driver.drain();
    assert_eq!(
        driver.world.current_map.as_str(),
        "the_vault",
        "the descent"
    );

    // ---------------- The Vault ------------------------------------------
    fn back_to_vault(d: &mut Driver) {
        // Whiteouts return to Cadenza's rest; climb the spire again
        // (Quartet flags hold) and drop through the open floor.
        for _ in 0..5 {
            match d.world.current_map.as_str() {
                "cadenza_city" => {
                    d.go_y(11);
                    d.go_x(9);
                    d.open_shop_at(4, 11);
                    d.buy_potions(4);
                    d.close_shop();
                    d.go_y(9);
                    d.go_x(24); // → spire
                }
                "quartet_spire" => {
                    d.go_x(4);
                    d.go_y(28);
                    d.drain(); // the floor remembers
                }
                "the_vault" => return,
                _ => return,
            }
        }
    }
    driver.until_flag(
        "trainer.vault_guard_1.defeated",
        6,
        |d| {
            if d.world.current_map.as_str() == "the_vault" {
                d.go_x(4);
                d.go_y(5);
                d.face(Up);
                d.interact();
            }
        },
        back_to_vault,
    );
    driver.until_flag(
        "trainer.vault_hush.defeated",
        6,
        |d| {
            if d.world.current_map.as_str() == "the_vault" {
                d.go_x(5);
                d.go_y(11);
                d.face(Up);
                d.interact();
            }
        },
        back_to_vault,
    );
    driver.until_flag(
        "trainer.vault_guard_2.defeated",
        6,
        |d| {
            if d.world.current_map.as_str() == "the_vault" {
                d.go_x(4);
                d.go_y(17);
                d.face(Up);
                d.interact();
            }
        },
        back_to_vault,
    );
    // The Vesper meeting (not a battle), then the bottom.
    driver.go_x(5);
    driver.go_y(23);
    driver.face(Up);
    driver.interact();
    driver.drain();
    assert!(driver.has_flag("story.vesper.met"));

    // ---------------- Chorus prep (echoes + the Score) -------------------
    // Runs HERE, after the Quartet and the vault guards: their payouts
    // (~55k) are what fund the belling tour. Aria waits.
    if chorus_prep {
        // Surface: vault → spire → the capital (x3 dodges the line of
        // dignitaries still standing in the aisle).
        driver.go_x(3);
        driver.go_y(1);
        driver.go_x(5);
        driver.go_y(0); // → spire (5,27)
        if driver.world.current_map.as_str() == "quartet_spire" {
            driver.go_x(3);
            driver.go_y(2);
            driver.go_x(5);
            driver.go_y(0); // → cadenza (23,9)
        }
        assert_eq!(driver.world.current_map.as_str(), "cadenza_city");
        run_chorus_prep(&mut driver);
        // Back down: spire stage → the open floor → the bottom.
        driver.go_y(9);
        driver.go_x(24); // → spire (5,1)
        if driver.world.current_map.as_str() == "quartet_spire" {
            driver.go_x(3);
            driver.go_y(27);
            driver.go_x(4);
            driver.go_y(28);
            driver.drain(); // the floor remembers
        }
        assert_eq!(driver.world.current_map.as_str(), "the_vault");
        driver.go_x(3);
        driver.go_y(24);
    }
    driver
}

/// Echoes 1–8 + the 60% Score for the Chorus gate: a southbound tour
/// from the capital, ringing every keeper and belling every field.
#[expect(clippy::too_many_lines, reason = "one continuous tour")]
fn run_chorus_prep(driver: &mut Driver) {
    fn must_reach(d: &mut Driver, target: &str) {
        d.tour_goto(target);
        if d.world.current_map.as_str() != target {
            d.tour_goto(target); // one more lap for compound whiteouts
        }
        assert_eq!(
            d.world.current_map.as_str(),
            target,
            "tour stranded at {:?}",
            d.world.player
        );
    }

    /// North towns ration the budget — the south fields are where the
    /// cheap catches live.
    fn restock_n(d: &mut Driver, mart: (u32, u32), overtures: u32, fermatas: u32) {
        d.open_shop_at(mart.0, mart.1);
        d.buy("overture_bell", overtures); // ×4 on turn 1 — the engine
        d.buy("fermata", fermatas);
        d.buy("potion_m", 3);
        d.close_shop();
        if std::env::var_os("DRIVER_DEBUG").is_some() {
            eprintln!(
                "  restock_n @{}: ovt {} fer {} money {}",
                d.world.current_map,
                d.world
                    .bag
                    .get(&"overture_bell".into())
                    .copied()
                    .unwrap_or(0),
                d.world.bag.get(&"fermata".into()).copied().unwrap_or(0),
                d.world.money,
            );
        }
        if std::env::var_os("DRIVER_DEBUG").is_some() {
            eprintln!(
                "  restock @{}: bells {} money {}",
                d.world.current_map,
                d.world.bag.get(&"fermata".into()).copied().unwrap_or(0),
                d.world.money,
            );
        }
    }
    fn echo(d: &mut Driver, x: u32, y: u32, side: undersong_core::world::Facing) {
        // Keepers stand still; talk from one tile beside.
        match side {
            Up => {
                d.go_x(x + 1); // adjacent lane — never path through them
                d.go_y(y - 1);
                d.go_x(x);
            }
            _ => {
                d.go_y(y - 1); // approach on the row below…
                d.go_x(x - 1);
                d.go_y(y); // …then step up beside them
            }
        }
        d.face(side);
        d.interact();
        d.drain();
        if matches!(side, Up) {
            d.go_x(x + 1); // step out of the keeper's column
        }
    }

    // Cadenza: echo 8, then the WHOLE tour's chest — cash halves on a
    // whiteout, a bag of bells doesn't.
    driver.go_y(11);
    driver.go_x(9); // rest
    restock_n(driver, (4, 11), 40, 60);
    echo(driver, 5, 5, Up);
    assert!(driver.has_flag("anchor_echo.8"), "echo 8");

    // Route 6 sweep, then Frostine: echo 7.
    driver.go_y(11);
    driver.go_x(12);
    driver.go_y(0); // south gate → route_6 (6,18) (P9 geography)
    driver.go_x(6);
    driver.go_y(7);
    // The Virtuoso takes all comers, every day, for stake money — the
    // era's rich rematch. Twelve sets of ten encores (with rests between:
    // ninety straight bouts once ended in Last-Resort recoil).
    for _ in 0..12 {
        for _ in 0..10 {
            if driver.world.current_map.as_str() != "route_6" {
                break;
            }
            driver.go_x(5);
            driver.go_y(8);
            driver.face(undersong_core::world::Facing::Left);
            driver.interact();
            driver.drain();
        }
        if driver.world.current_map.as_str() == "route_6" {
            driver.go_x(6);
            driver.go_y(19); // → cadenza
        }
        if driver.world.current_map.as_str() == "cadenza_city" {
            driver.go_y(11);
            driver.go_x(9); // rest: hp + pp
            driver.go_y(11);
            driver.go_x(12);
            driver.go_y(0); // south gate → route_6
            driver.go_x(6);
            driver.go_y(7);
        }
    }
    if std::env::var_os("DRIVER_DEBUG").is_some() {
        eprintln!("  war chest: {}", driver.world.money);
    }
    // First order of business: a frost_lull singer (Shiverine, 20%
    // here) — sleep-and-ring is what makes the Score affordable.
    driver.go_x(10);
    for _ in 0..600 {
        let have_singer = driver
            .world
            .party
            .iter()
            .any(|m| m.moves.iter().any(|mv| mv.id.as_str() == "frost_lull"));
        if have_singer {
            break;
        }
        if driver.world.battle.is_some() {
            let is_shiverine =
                driver.world.battle.as_ref().is_some_and(|b| {
                    b.state.sides[1].active_mote().species.as_str() == "shiverine"
                });
            let bells = driver
                .world
                .bag
                .get(&"fermata".into())
                .copied()
                .unwrap_or(0);
            if is_shiverine && bells > 0 {
                driver.input(Input::Battle(game::session::BattleCmd::Bell));
            } else {
                driver.input(Input::Battle(game::session::BattleCmd::Run));
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
        if driver.world.current_map.as_str() != "route_6" {
            break;
        }
        let dir = if driver.world.player.1 <= 5 { Up } else { Down };
        driver.face(dir);
        driver.input(Input::Step(dir));
    }
    // If the singer ended up boxed (full party), make room and pull
    // it out — the whole tour's economics ride on frost_lull.
    if !driver
        .world
        .party
        .iter()
        .any(|m| m.moves.iter().any(|mv| mv.id.as_str() == "frost_lull"))
    {
        let boxed_singer = driver
            .world
            .boxes
            .iter()
            .position(|m| m.moves.iter().any(|mv| mv.id.as_str() == "frost_lull"));
        if let Some(index) = boxed_singer {
            if driver.world.party.len() >= 6 {
                let bench = u8::try_from(driver.world.party.len() - 1).unwrap_or(5);
                driver.input(Input::BoxDeposit { party_index: bench });
            }
            driver.input(Input::BoxWithdraw {
                box_index: u32::try_from(index).unwrap_or(0),
            });
        }
    }
    if std::env::var_os("DRIVER_DEBUG").is_some() {
        eprintln!(
            "  singer aboard: {}",
            driver
                .world
                .party
                .iter()
                .any(|m| m.moves.iter().any(|mv| mv.id.as_str() == "frost_lull")),
        );
    }
    driver.sweep_catch(9, 5, 11, 9, 2500); // deep: the capital rares
    must_reach(driver, "frostine");
    driver.go_x(12); // free lane down past the rest stop
    driver.go_y(7);
    driver.go_x(9); // rest
    restock_n(driver, (4, 7), 90, 160); // EVERYTHING — cash halves, bells don't
    echo(driver, 6, 4, Up);
    assert!(driver.has_flag("anchor_echo.7"), "echo 7");

    // Graven: echo 6 + the mountain sweep.
    driver.go_y(7);
    driver.go_x(11);
    driver.go_y(0); // → graven (6,26)
    driver.go_x(8);
    driver.go_y(19);
    driver.go_x(6);
    driver.go_y(12); // waystation rest en route
    echo(driver, 9, 9, Right);
    assert!(driver.has_flag("anchor_echo.6"), "echo 6");
    driver.go_x(6);
    driver.go_y(8);
    driver.sweep_catch(2, 6, 4, 10, 2500);
    for _ in 0..2 {
        if driver.world.current_map.as_str() == "graven_pass" {
            driver.go_x(6);
            driver.go_y(0); // → hollowfen (10,12)
        }
        if driver.world.current_map.as_str() == "hollowfen" {
            break;
        }
    }

    // Hollowfen: echo 5, then the fen road sweep west.
    must_reach(driver, "hollowfen");
    driver.go_x(12);
    driver.go_y(7);
    driver.go_x(9); // rest (the budget stays south)
    echo(driver, 6, 4, Up);
    assert!(driver.has_flag("anchor_echo.5"), "echo 5");
    driver.go_y(7);
    driver.go_x(0); // → route_5b (24,7)
    driver.go_y(8); // around the lampman's tile
    driver.go_x(17);
    driver.go_y(3);
    driver.sweep_catch(16, 2, 20, 4, 2000);
    for _ in 0..2 {
        if driver.world.current_map.as_str() == "route_5b" {
            driver.go_y(7); // the bogger owns (6,6)
            driver.go_x(0); // → voltaccia (11,14)
        }
        if driver.world.current_map.as_str() == "voltaccia" {
            break;
        }
    }

    // Voltaccia: echo 4, then Route 5 down to Calando: echo 1.
    must_reach(driver, "voltaccia");
    driver.go_x(12); // free lane down past the rest stop
    driver.go_y(9);
    driver.go_x(9); // rest (the budget stays south)
    echo(driver, 5, 5, Up);
    assert!(driver.has_flag("anchor_echo.4"), "echo 4");
    must_reach(driver, "route_5");
    driver.go_y(16);
    driver.go_x(3);
    driver.sweep_catch(2, 14, 4, 18, 2000);
    must_reach(driver, "port_calando");
    driver.go_y(15); // off the door row first — (12,16) warps back
    driver.go_x(12); // free lane down past the rest stop
    driver.go_y(11);
    driver.go_x(9); // rest
    restock_n(driver, (4, 11), 25, 50); // the south chest
    driver.trace.clear();
    eprintln!(
        "    pre-echo1: map {} at {:?}",
        driver.world.current_map, driver.world.player
    );
    echo(driver, 5, 5, Up);
    if !driver.has_flag("anchor_echo.1") {
        for line in &driver.trace {
            eprintln!("    | {line}");
        }
        eprintln!(
            "    pos {:?} map {}",
            driver.world.player, driver.world.current_map
        );
    }
    assert!(driver.has_flag("anchor_echo.1"), "echo 1");

    // Route 4 + the Quiet Coast (echo 3) + Route 3 + Arbor (echo 2).
    driver.go_x(11);
    driver.go_y(0); // → route_4 (6,20)
    driver.go_y(7);
    driver.go_x(10);
    driver.sweep_catch(9, 5, 11, 9, 700);
    for _ in 0..2 {
        if driver.world.current_map.as_str() == "route_4" {
            driver.go_x(6);
            driver.go_y(0); // → route_3 (6,24)
        }
        if driver.world.current_map.as_str() == "route_3" {
            break;
        }
    }
    must_reach(driver, "route_3");
    driver.go_y(13);
    driver.go_x(0); // west doors → quiet_coast (22,6)?
    if driver.world.current_map.as_str() == "quiet_coast" {
        echo(driver, 19, 8, Right);
        assert!(driver.has_flag("anchor_echo.3"), "echo 3");
        driver.go_x(16);
        driver.go_y(4);
        driver.sweep_catch(15, 4, 18, 5, 1600);
        for _ in 0..2 {
            if driver.world.current_map.as_str() == "quiet_coast" {
                driver.go_y(6); // the old fisher owns (8,7)
                driver.go_x(22);
                driver.go_y(7);
                driver.go_x(23); // east doors (23,7)/(23,8) → route_3
            }
            if driver.world.current_map.as_str() == "route_3" {
                break;
            }
        }
    }
    must_reach(driver, "route_3");
    driver.go_x(3);
    driver.go_y(6);
    driver.sweep_catch(2, 4, 4, 8, 700);
    for _ in 0..2 {
        if driver.world.current_map.as_str() == "route_3" {
            driver.go_y(10); // the drover owns (4,6)
            driver.go_x(6);
            driver.go_y(0); // → arbor (11,14)
        }
        if driver.world.current_map.as_str() == "arbor_vale" {
            break;
        }
    }
    must_reach(driver, "arbor_vale");
    driver.go_x(13);
    driver.go_y(9);
    driver.go_x(10); // rest
    restock_n(driver, (5, 9), 25, 50);
    echo(driver, 13, 5, Up);
    assert!(driver.has_flag("anchor_echo.2"), "echo 2");

    // Route 2 sweep finishes the Score if the coast didn't.
    driver.go_y(8);
    driver.go_x(2);
    driver.go_y(7);
    driver.go_x(0); // → route_2 (29,?)
    if driver.world.current_map.as_str() == "route_2" {
        driver.go_y(3);
        driver.go_x(24);
        driver.sweep_catch(22, 2, 26, 4, 2000);
    }
    // Route 1's early commons round out the Score.
    driver.tour_goto("pausa_village");
    if driver.world.current_map.as_str() == "pausa_village" {
        driver.go_y(6);
        driver.go_x(9);
        driver.go_y(13); // → route_1 (6,1)
    }
    if driver.world.current_map.as_str() == "route_1" {
        driver.go_y(14);
        driver.go_x(4);
        driver.sweep_catch(3, 13, 5, 15, 1500);
    }

    // Northbound: back to the capital for the Spire.
    for _ in 0..12 {
        match driver.world.current_map.as_str() {
            "route_2" => {
                driver.go_y(6);
                driver.go_x(29);
            }
            "arbor_vale" => {
                driver.go_x(13);
                driver.go_y(14);
                driver.go_x(12);
                driver.go_y(15);
            }
            "route_3" => {
                driver.go_x(6);
                driver.go_y(25);
            }
            "route_4" => {
                driver.go_x(6);
                driver.go_y(21);
            }
            "port_calando" => {
                driver.go_y(11);
                driver.go_x(9);
                driver.go_x(12);
                driver.go_y(16);
            }
            "route_5" => {
                driver.go_y(10);
                driver.go_x(6);
                driver.go_y(23);
            }
            "voltaccia" => {
                driver.go_y(9);
                driver.go_x(9);
                driver.go_y(7);
                driver.go_x(23); // east gate (P9 geography)
            }
            "route_5b" => {
                driver.go_y(7);
                driver.go_x(16);
                if driver.world.current_map.as_str() != "route_5b" {
                    continue;
                }
                driver.go_y(8);
                driver.go_x(24);
                driver.go_y(7);
                driver.go_x(25);
            }
            "hollowfen" => {
                driver.go_x(12);
                driver.go_y(7);
                driver.go_x(9);
                driver.go_x(12);
                driver.go_y(12);
                driver.go_x(10);
                driver.go_y(13);
            }
            "graven_pass" => {
                if driver.world.player.1 < 20 {
                    driver.go_y(19);
                    driver.go_x(8);
                    driver.go_y(24);
                }
                driver.go_x(6);
                driver.go_y(27);
            }
            "frostine" => {
                driver.go_y(7);
                driver.go_x(9);
                driver.go_x(12);
                driver.go_y(13);
            }
            "route_6" => {
                driver.go_x(6);
                driver.go_y(19);
            }
            "cadenza_city" => break,
            _ => break,
        }
    }
    must_reach(driver, "cadenza_city");

    if std::env::var_os("DRIVER_DEBUG").is_some() {
        let mut uncaught: Vec<String> = driver
            .world
            .registry
            .as_ref()
            .map(|r| {
                r.species
                    .keys()
                    .filter(|sp| {
                        !driver
                            .world
                            .vars
                            .flags
                            .contains(&format!("dex.caught.{sp}"))
                    })
                    .map(ToString::to_string)
                    .collect()
            })
            .unwrap_or_default();
        uncaught.sort();
        eprintln!("  uncaught ({}): {:?}", uncaught.len(), uncaught);
    }
    eprintln!(
        "chorus prep: caught {} echoes {:?} chorus.ready {}",
        driver.caught_count(),
        (1..=8u8)
            .filter(|n| driver.has_flag(&format!("anchor_echo.{n}")))
            .count(),
        driver.has_flag("chorus.ready"),
    );
}

fn record(driver: &Driver, name: &str, flags: &[&str]) {
    if std::env::var_os("UPDATE_REPLAYS").is_none() {
        return;
    }
    let file = game::replay::ReplayFile {
        seed: ACT_SEED,
        world: game::replay::WorldKind::Game,
        inputs: driver.log.clone(),
        expect: game::replay::Expectations {
            map: driver.world.current_map.to_string(),
            position: driver.world.player,
            flags: flags.iter().map(ToString::to_string).collect(),
            min_dialogue_lines: driver.dialogue_lines,
            warped: true,
            party_levels: Some(driver.world.party.iter().map(|p| p.level).collect()),
            money: Some(driver.world.money),
        },
    };
    let text =
        ron::ser::to_string_pretty(&file, ron::ser::PrettyConfig::default()).expect("serialize");
    std::fs::write(
        content_root().join(format!("../tests/replays/{name}.ron")),
        format!(
            "// P6 gate replay — recorded by act3_run.rs (UPDATE_REPLAYS=1).\n// {} inputs; do not hand-edit.\n{text}\n",
            file.inputs.len()
        ),
    )
    .expect("write replay");
}

/// At the finale Choice: cursor starts at option 0. Two-option menus are
/// [dacapo, tacet]; three-option (chorus.ready) are [chorus, dacapo,
/// tacet]. Down moves the cursor; Interact confirms.
fn choose_finale(driver: &mut Driver, downs: u32) {
    driver.go_x(4);
    driver.go_y(31);
    driver.answer_choice(downs);
}

#[test]
fn ending_dacapo_reaches_credits() {
    let mut driver = run_to_finale(false);
    choose_finale(&mut driver, 0); // two-option menu: dacapo first
    assert!(driver.has_flag("credits.dacapo"), "flags: {:?}", {
        let mut f: Vec<_> = driver
            .world
            .vars
            .flags
            .iter()
            .filter(|f| f.starts_with("ending") || f.starts_with("credits"))
            .collect();
        f.sort();
        f
    });
    driver.input(Input::Save);
    record(
        &driver,
        "ending_dacapo",
        &["badge.8", "story.quartet.4.defeated", "credits.dacapo"],
    );
}

#[test]
fn ending_tacet_reaches_credits() {
    let mut driver = run_to_finale(false);
    choose_finale(&mut driver, 1); // two-option menu: tacet second
    assert!(driver.has_flag("credits.tacet"));
    driver.input(Input::Save);
    record(
        &driver,
        "ending_tacet",
        &["badge.8", "story.vesper.met", "credits.tacet"],
    );
}

#[test]
fn ending_chorus_reaches_credits() {
    let mut driver = run_to_finale(true);
    choose_finale(&mut driver, 0); // three-option menu: chorus first
    assert!(
        driver.has_flag("credits.chorus"),
        "chorus gate: caught {} ready {}",
        driver.caught_count(),
        driver.has_flag("chorus.ready"),
    );
    driver.input(Input::Save);
    record(
        &driver,
        "ending_chorus",
        &["badge.8", "anchor_echo.8", "chorus.ready", "credits.chorus"],
    );
}

#[test]
fn chorus_replay_plays_back() {
    if std::env::var_os("UPDATE_REPLAYS").is_some() {
        return;
    }
    let path = content_root().join("../tests/replays/ending_chorus.ron");
    if !path.exists() {
        panic!("missing ending_chorus.ron — record with UPDATE_REPLAYS=1");
    }
    game::replay::run_replay_file(&content_root(), &path).expect("chorus replays");
}

#[test]
fn dacapo_replay_plays_back() {
    if std::env::var_os("UPDATE_REPLAYS").is_some() {
        return;
    }
    let path = content_root().join("../tests/replays/ending_dacapo.ron");
    if !path.exists() {
        panic!("missing ending_dacapo.ron — record with UPDATE_REPLAYS=1");
    }
    game::replay::run_replay_file(&content_root(), &path).expect("dacapo replays");
}

#[test]
fn tacet_replay_plays_back() {
    if std::env::var_os("UPDATE_REPLAYS").is_some() {
        return;
    }
    let path = content_root().join("../tests/replays/ending_tacet.ron");
    if !path.exists() {
        panic!("missing ending_tacet.ron — record with UPDATE_REPLAYS=1");
    }
    game::replay::run_replay_file(&content_root(), &path).expect("tacet replays");
}
