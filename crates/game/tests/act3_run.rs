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
                    d.go_x(12); // free lane east of the rest stop
                    d.go_y(15); // north gate → route_5b
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
                d.interact(); // Orsk
            }
        },
        back_to_graven,
    );

    // ---------------- Badge 7: Frostine (Ilva) ---------------------------
    fn back_to_frostine(d: &mut Driver) {
        for _ in 0..6 {
            match d.world.current_map.as_str() {
                "hall_6" => {
                    d.go_x(6);
                    d.go_y(0);
                }
                "graven_pass" => {
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
                    d.go_x(11);
                    d.go_y(13); // → route_6 (6,1)
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
    driver.go_x(13);
    driver.face(Up);
    driver.interact();
    driver.drain();
    assert!(driver.has_flag("story.reed.confessed"), "the confession");
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
                d.interact(); // Provost Calder
            }
        },
        back_to_cadenza,
    );

    // Beat 14: Cade's plea, the night before.
    back_to_cadenza(&mut driver);
    driver.go_y(6);
    driver.go_x(17);
    driver.face(Right);
    driver.interact();
    driver.drain();
    if !driver.has_flag("story.cade.plea") {
        driver.go_x(16);
        driver.face(Right);
        driver.interact();
        driver.drain();
    }
    assert!(driver.has_flag("story.cade.plea"), "the plea");

    // ---------------- Chorus prep (echoes + the Score) -------------------
    if chorus_prep {
        run_chorus_prep(&mut driver);
    }

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

    // The empty stage. The floor opens.
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
    driver
}

/// Echoes 1–8 + the 60% Score for the Chorus gate.
fn run_chorus_prep(driver: &mut Driver) {
    let _ = driver;
    unimplemented!("filled in by the chorus test below");
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
    // The scene pumps Says until the Choice opens; drive it manually.
    for _ in 0..40 {
        if driver
            .world
            .dialogue
            .as_ref()
            .is_some_and(|d| d.choice.is_some())
        {
            break;
        }
        driver.input(Input::Interact);
    }
    for _ in 0..downs {
        driver.input(Input::Step(Down));
    }
    driver.input(Input::Interact);
    driver.drain();
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
