//! The P3 acceptance gate: a full new game → Badge 1 run, driven through
//! the pure session core. The driver records every input it submits;
//! UPDATE_REPLAYS=1 writes the recording to
//! tests/replays/new_game_to_first_badge.ron, and a second test plays
//! that artifact back through the replay driver (CI lane).

use std::path::{Path, PathBuf};

use game::world::{Input, WorldEvent, WorldState, load_game_world};
use undersong_core::world::Facing::{self, Down, Left, Right, Up};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

const SEED: u64 = 0x0BAD_9E1; // chosen once; the artifact pins it

struct Driver {
    world: WorldState,
    log: Vec<Input>,
    dialogue_lines: u32,
}

impl Driver {
    fn input(&mut self, input: Input) -> Vec<WorldEvent> {
        self.log.push(input);
        let events = self.world.apply(input);
        for event in &events {
            if matches!(event, WorldEvent::DialogueLine { .. }) {
                self.dialogue_lines += 1;
            }
        }
        events
    }

    /// Resolves every modal state: battles (spam slot 0), dialogue
    /// (advance; choices accept the current cursor), evolutions (accept),
    /// learn prompts (skip), shops (close).
    fn drain(&mut self) {
        for _ in 0..600 {
            if let Some(session) = &self.world.battle {
                // Potion when the active Mote drops under 40% and the
                // bag has one — then fight with the T1 policy (max
                // expected damage). Both are deterministic, so the
                // recorded inputs replay exactly.
                let active = session.state.sides[0].active_mote();
                let hurt = u32::from(active.hp) * 5 < u32::from(active.max_hp()) * 2;
                let has_potion = self.world.bag.iter().any(|(id, n)| {
                    *n > 0
                        && self.world.registry.as_ref().is_some_and(|r| {
                            matches!(
                                r.items.get(id).map(|d| &d.kind),
                                Some(data::ItemKind::Potion { .. })
                            )
                        })
                });
                if hurt && has_potion && !active.is_fainted() {
                    self.input(Input::Battle(game::session::BattleCmd::Item));
                    continue;
                }
                let mut probe = undersong_core::rng::BattleRng::from_seed(0);
                let action =
                    battle::ai::choose(battle::ai::AiTier::T1, &session.state, 0, &mut probe);
                let command = match action {
                    battle::Action::Move { slot } => game::session::BattleCmd::Move { slot },
                    battle::Action::Switch { to } => game::session::BattleCmd::Switch { to },
                    _ => game::session::BattleCmd::Move { slot: 0 },
                };
                self.input(Input::Battle(command));
                continue;
            }
            if self.world.dialogue.is_some() {
                self.input(Input::Interact);
                continue;
            }
            if !self.world.pending_evolutions.is_empty() {
                self.input(Input::Evolve { accept: true });
                continue;
            }
            if !self.world.pending_learn_queue.is_empty() {
                self.input(Input::Learn { replace: None });
                continue;
            }
            if self.world.shop.is_some() {
                self.input(Input::ShopClose);
                continue;
            }
            return;
        }
        panic!("drain did not settle — stuck modal state");
    }

    /// Tap-to-turn aware single step; drains whatever the step caused.
    fn step(&mut self, dir: Facing) {
        if self.world.facing != dir && self.world.dialogue.is_none() {
            self.input(Input::Step(dir));
        }
        self.input(Input::Step(dir));
        self.drain();
    }

    fn walk(&mut self, legs: &[(Facing, u32)]) {
        for (dir, count) in legs {
            for _ in 0..*count {
                self.step(*dir);
            }
        }
    }

    fn interact(&mut self) {
        self.input(Input::Interact);
        self.drain();
    }

    /// After a whiteout the party wakes at the rest point; walk back to
    /// the Route 1 patch field at (4,14).
    fn retrek_to_patches(&mut self) {
        if self.world.current_map.as_str() == "pausa_village" {
            // spawn (8,4) → gate (9,13) → route (6,1)
            self.walk(&[(Right, 1), (Up, 9)]);
        }
        if self.world.current_map.as_str() == "prelude_town" {
            // rest stop (10,7) → south doors (10,0) → route (6,28)
            self.walk(&[(Down, 6), (Down, 1)]);
        }
        if self.world.current_map.as_str() == "route_1" {
            let y = self.world.player.1;
            if self.world.player.0 == 6 || self.world.player.0 == 7 {
                if y < 14 {
                    self.walk(&[(Up, 14 - y)]);
                } else if y > 14 {
                    self.walk(&[(Down, y - 14)]);
                }
                while self.world.player.0 > 4 {
                    self.step(Left);
                }
            }
        }
    }

    fn on_patch_field(&self) -> bool {
        self.world.current_map.as_str() == "route_1"
            && (2..=4).contains(&self.world.player.0)
            && (14..=18).contains(&self.world.player.1)
    }

    fn has_bell(&self) -> bool {
        self.world.bag.get(&"fermata".into()).copied().unwrap_or(0) > 0
    }

    /// Bounces on a patch tile until a second party member is caught
    /// (or bells run out). Wild battles ring a bell immediately —
    /// route-common catch rates make full-HP catches likely; anything
    /// that escapes the bells gets KO'd by the T1 policy for exp.
    fn catch_one_teammate(&mut self) {
        for _ in 0..400 {
            if self.world.party.len() >= 2 {
                return;
            }
            if self.world.battle.is_some() {
                if self.has_bell() {
                    self.input(Input::Battle(game::session::BattleCmd::Bell));
                } else {
                    self.drain(); // no bells left: fight it out for exp
                }
                continue;
            }
            if self.world.dialogue.is_some()
                || !self.world.pending_learn_queue.is_empty()
                || !self.world.pending_evolutions.is_empty()
            {
                self.drain();
                continue;
            }
            if !self.on_patch_field() {
                self.retrek_to_patches();
                continue;
            }
            // Bounce between two patch tiles — raw inputs, NOT step():
            // step() drains, and drain's T1 policy would KO the wild
            // before the bell logic ever saw it.
            let dir = if self.world.player.1 % 2 == 0 {
                Up
            } else {
                Down
            };
            if self.world.facing != dir {
                self.input(Input::Step(dir));
            }
            self.input(Input::Step(dir));
        }
        panic!("could not catch a teammate (bells: {:?})", self.world.bag);
    }
}

fn run_to_badge() -> Driver {
    let world = load_game_world(&root().join("content"), SEED).expect("game world loads");
    let mut driver = Driver {
        world,
        log: Vec::new(),
        dialogue_lines: 0,
    };

    // Pausa Village spawn (8,4) → Reed's lab door (6,9).
    driver.walk(&[(Left, 2), (Up, 5)]); // (6,9) warps into the lab (4,1)
    assert_eq!(driver.world.current_map.as_str(), "pausa_lab");

    // Intro trigger at (4,2), then talk to Reed at (4,4): the choice
    // cursor starts on fanfyre — accept it.
    driver.walk(&[(Up, 2)]); // (4,3); the (4,2) step fires the intro
    driver.interact(); // Reed: offer → choice(fanfyre) → bells → Cade
    assert!(driver.world.vars.flags.contains("starter.fanfyre"));
    assert_eq!(driver.world.party.len(), 1);

    // Leave the lab, cross the village to the Route 1 gate (9,13).
    driver.walk(&[(Down, 3)]); // (4,0) → village (6,8)
    assert_eq!(driver.world.current_map.as_str(), "pausa_village");
    driver.walk(&[(Right, 3), (Up, 5)]); // (9,13) → route_1 (6,1)
    assert_eq!(driver.world.current_map.as_str(), "route_1");

    // March north past the first two trainers, then detour into the
    // middle patch field to attune a teammate (the percussionist's
    // timpanite walls a solo ember starter — by design).
    driver.walk(&[(Up, 12)]); // (6,13): tuner + busker engaged en route
    assert!(
        driver
            .world
            .vars
            .flags
            .contains("trainer.rt1_tuner.defeated")
    );
    driver.walk(&[(Up, 1), (Left, 2)]); // (4,14): patch field
    driver.catch_one_teammate();
    assert!(driver.world.party.len() >= 2, "a teammate joined");
    // Era ritual: grind the patch field until the starter hits L11
    // before challenging the hall (drain()'s T1 policy KOs wilds).
    for _ in 0..2500 {
        if driver.world.party[0].level >= 11 {
            break;
        }
        if !driver.on_patch_field() {
            driver.retrek_to_patches();
            continue;
        }
        let dir = if driver.world.player.1 % 2 == 0 {
            Up
        } else {
            Down
        };
        driver.step(dir);
    }
    assert!(driver.world.party[0].level >= 11, "grind target reached");
    driver.retrek_to_patches(); // normalize to the field if a whiteout hit
    // Return to the path at whatever row the bouncing ended on.
    while driver.world.player.1 > 14 {
        driver.step(Down);
    }
    driver.walk(&[(Right, 2)]); // back to x6
    let remaining = 28 - driver.world.player.1;
    driver.walk(&[(Up, remaining)]); // (6,28): percussionist, choirboy, rival
    assert!(driver.world.vars.flags.contains("story.rival1.defeated"));

    // Into Prelude Town; the rest-stop doorstep heals on the way.
    driver.walk(&[(Up, 1)]); // (6,29) → prelude (10,1)
    assert_eq!(driver.world.current_map.as_str(), "prelude_town");
    driver.walk(&[(Up, 6)]); // x10 → (10,7): rest stop heals

    // Stock up at the mart for the hall (exercises the shop path).
    driver.walk(&[(Left, 5)]); // (5,7): mart doorstep → clerk → shop
    // drain() closed the shop; reopen by stepping off/on with raw
    // inputs and buy three mid potions (stock index 4 = potion_m).
    driver.input(Input::Step(Down));
    driver.input(Input::Step(Down)); // (5,6)
    driver.input(Input::Step(Up));
    driver.input(Input::Step(Up)); // (5,7): clerk line again
    while driver.world.dialogue.is_some() {
        driver.input(Input::Interact);
    }
    assert!(driver.world.shop.is_some(), "mart open");
    // Buy what the wallet allows: prefer mid potions, fall back to
    // small ones (whiteouts on the route may have halved the funds).
    let stock: Vec<String> = driver
        .world
        .shop
        .as_ref()
        .map(|(items, _)| items.iter().map(ToString::to_string).collect())
        .unwrap_or_default();
    let buy = |driver: &mut Driver, name: &str, count: u32| {
        let Some(target) = stock.iter().position(|s| s == name) else {
            return;
        };
        loop {
            let cursor = driver.world.shop.as_ref().map(|(_, c)| *c).unwrap_or(0);
            match cursor.cmp(&target) {
                std::cmp::Ordering::Less => driver.input(Input::ShopCursor(1)),
                std::cmp::Ordering::Greater => driver.input(Input::ShopCursor(-1)),
                std::cmp::Ordering::Equal => break,
            };
        }
        for _ in 0..count {
            driver.input(Input::ShopBuy); // silently no-ops when broke
        }
    };
    buy(&mut driver, "potion_m", 3);
    buy(&mut driver, "potion_s", 3);
    let potions = driver
        .world
        .bag
        .get(&"potion_m".into())
        .copied()
        .unwrap_or(0)
        + driver
            .world
            .bag
            .get(&"potion_s".into())
            .copied()
            .unwrap_or(0);
    assert!(
        potions >= 2,
        "stocked at least two potions (money {})",
        driver.world.money
    );
    driver.input(Input::ShopClose);
    driver.drain();

    driver.walk(&[(Right, 5)]); // back to (10,7): rest heals again
    driver.walk(&[(Right, 6), (Up, 1)]); // (16,8) → hall_1 (6,1)
    assert_eq!(driver.world.current_map.as_str(), "hall_1");

    // The S-path puzzle, engaging both hall trainers on the way.
    driver.walk(&[(Up, 4)]); // (6,5)
    driver.walk(&[(Left, 1), (Down, 1)]); // (5,4): aide's sight line
    assert!(
        driver
            .world
            .vars
            .flags
            .contains("trainer.hall_aide.defeated")
    );
    driver.walk(&[(Up, 1), (Right, 4), (Up, 3)]); // (9,8)
    driver.walk(&[(Left, 2), (Down, 1)]); // (7,7): senior's sight line
    assert!(
        driver
            .world
            .vars
            .flags
            .contains("trainer.hall_senior.defeated")
    );
    driver.walk(&[(Up, 1), (Left, 5), (Up, 2), (Right, 4), (Up, 2)]); // (6,12)
    driver.interact(); // Dario: dialogue → battle → badge ceremony
    assert!(
        driver.world.vars.flags.contains("hall.1.cleared"),
        "the Maestro fight must be won (flags: {:?}, party: {:?})",
        driver.world.vars.flags,
        driver
            .world
            .party
            .iter()
            .map(|p| format!("{} L{} hp{:?}", p.species, p.level, p.hp))
            .collect::<Vec<_>>()
    );
    assert!(driver.world.vars.flags.contains("badge.1"));
    assert!(driver.world.vars.flags.contains("performance.lumen_hum"));
    driver
}

#[test]
fn new_game_reaches_badge_one() {
    let driver = run_to_badge();

    // Record the artifact when asked (gate replay for CI).
    if std::env::var_os("UPDATE_REPLAYS").is_some() {
        let file = game::replay::ReplayFile {
            seed: SEED,
            world: game::replay::WorldKind::Game,
            inputs: driver.log.clone(),
            expect: game::replay::Expectations {
                map: driver.world.current_map.to_string(),
                position: driver.world.player,
                flags: [
                    "starter.fanfyre",
                    "story.rival1.defeated",
                    "hall.1.cleared",
                    "badge.1",
                    "performance.lumen_hum",
                ]
                .into_iter()
                .map(String::from)
                .collect(),
                min_dialogue_lines: 10,
                warped: true,
            },
        };
        let text = ron::ser::to_string_pretty(&file, ron::ser::PrettyConfig::default())
            .expect("serialize replay");
        let path = root().join("tests/replays/new_game_to_first_badge.ron");
        std::fs::write(
            path,
            format!(
                "// P3 gate replay — recorded by badge_run.rs (UPDATE_REPLAYS=1).\n// {} inputs from one seed; do not hand-edit.\n{text}\n",
                file.inputs.len()
            ),
        )
        .expect("write replay");
    }
}

#[test]
fn recorded_badge_replay_plays_back() {
    let path = root().join("tests/replays/new_game_to_first_badge.ron");
    if !path.exists() {
        panic!("missing gate replay — run UPDATE_REPLAYS=1 cargo test -p game --test badge_run");
    }
    let outcome = game::replay::run_replay_file(&root().join("content"), &path)
        .expect("the recorded badge run must replay cleanly");
    assert!(outcome.dialogue_lines >= 10);
    assert!(outcome.warps >= 4, "lab, route, town, hall transitions");
}
