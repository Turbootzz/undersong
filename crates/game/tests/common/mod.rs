#![allow(dead_code)] // shared across gate tests; not all use every helper
//! Shared adaptive driver for gate replays (badge run, act 1).

use game::session::BattleCmd;
use game::world::{Input, WorldEvent, WorldState};
use undersong_core::world::Facing::{self, Down, Left, Right, Up};

pub struct Driver {
    pub world: WorldState,
    pub log: Vec<Input>,
    pub dialogue_lines: u32,
    pub trace: Vec<String>,
    battle_turns: u32,
}

impl Driver {
    pub fn new(world: WorldState) -> Self {
        Driver {
            world,
            log: Vec::new(),
            dialogue_lines: 0,
            trace: Vec::new(),
            battle_turns: 0,
        }
    }

    pub fn input(&mut self, input: Input) -> Vec<WorldEvent> {
        self.log.push(input.clone());
        let events = self.world.apply(input);
        for event in &events {
            if matches!(event, WorldEvent::DialogueLine { .. }) {
                self.dialogue_lines += 1;
            }
            if std::env::var_os("DRIVER_TRACE").is_some() {
                self.trace.push(format!("{event:?}"));
                if self.trace.len() > 500 {
                    self.trace.remove(0);
                }
            }
        }
        events
    }

    /// Battle policy: potion under 40%, otherwise T1's pick; doubles
    /// declare per position with choose_doubles. Deterministic.
    pub fn battle_policy(&mut self) {
        let Some(session) = &self.world.battle else {
            self.battle_turns = 0;
            return;
        };
        if self.world.pending_shift {
            self.input(Input::Shift(None));
            return;
        }
        self.battle_turns += 1;
        // Stalled wild fight (healer loops, mutual walls): just leave.
        if self.battle_turns > 60
            && matches!(session.context, game::session::BattleContext::Wild { .. })
        {
            self.input(Input::Battle(BattleCmd::Run));
            return;
        }
        if session.state.format == battle::Format::Double {
            let position = u8::from(session.pending_declaration.is_some());
            // Potion the declaring position when it's hurting.
            let declaring = &session.state.sides[0];
            if let Some(slot) = declaring.positions.get(usize::from(position)) {
                let mote = &declaring.party[usize::from(slot.party_index)];
                let hurt = u32::from(mote.hp) * 5 < u32::from(mote.max_hp()) * 2;
                let has_potion = self.world.bag.iter().any(|(id, n)| {
                    *n > 0
                        && self.world.registry.as_ref().is_some_and(|r| {
                            matches!(
                                r.items.get(id).map(|d| &d.kind),
                                Some(data::ItemKind::Potion { .. })
                            )
                        })
                });
                if hurt && has_potion && !mote.is_fainted() {
                    self.input(Input::Battle(BattleCmd::Item));
                    return;
                }
            }
            let mut probe = undersong_core::rng::BattleRng::from_seed(0);
            let (action, target) = battle::ai::choose_doubles(
                battle::ai::AiTier::T2,
                &session.state,
                0,
                position,
                &mut probe,
            );
            let command = match action {
                battle::Action::Move { slot } => BattleCmd::MoveAt { slot, target },
                battle::Action::Switch { to } => BattleCmd::Switch { to },
                _ => BattleCmd::MoveAt { slot: 0, target: 0 },
            };
            self.input(Input::Battle(command));
            return;
        }
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
            self.input(Input::Battle(BattleCmd::Item));
            return;
        }
        // Critical with an empty bag in a WILD fight: flee — a faint
        // costs half the wallet, a flight costs nothing.
        let critical = u32::from(active.hp) * 4 < u32::from(active.max_hp());
        if critical
            && !has_potion
            && matches!(session.context, game::session::BattleContext::Wild { .. })
        {
            self.input(Input::Battle(BattleCmd::Run));
            return;
        }
        // Damper walls: if the active mote's every attacking move is
        // blanked by the foe's ability, no tier of move-picking helps —
        // switch to a teammate who can actually touch it.
        let foe_ability = session.state.sides[1].active_mote().ability;
        let blanked = |mote: &battle::BattleMote| {
            mote.moves.iter().all(|m| {
                m.spec.power == 0
                    || (foe_ability == battle::abilities::Ability::Damper && m.spec.flags.sound)
                    || (foe_ability == battle::abilities::Ability::Floating
                        && m.spec.r#type == undersong_core::types::Type::Stone)
            })
        };
        let our_side = &session.state.sides[0];
        if blanked(our_side.active_mote()) {
            let bench = our_side.party.iter().enumerate().find(|(i, m)| {
                *i != usize::from(our_side.positions[0].party_index)
                    && !m.is_fainted()
                    && !blanked(m)
            });
            if let Some((to, _)) = bench {
                self.input(Input::Battle(BattleCmd::Switch {
                    to: u8::try_from(to).unwrap_or(0),
                }));
                return;
            }
        }
        let mut probe = undersong_core::rng::BattleRng::from_seed(0);
        let action = battle::ai::choose(battle::ai::AiTier::T3, &session.state, 0, &mut probe);
        let command = match action {
            battle::Action::Move { slot } => BattleCmd::Move { slot },
            battle::Action::Switch { to } => BattleCmd::Switch { to },
            _ => BattleCmd::Move { slot: 0 },
        };
        self.input(Input::Battle(command));
    }

    pub fn drain(&mut self) {
        for _ in 0..2000 {
            if self.world.battle.is_some() {
                self.battle_policy();
                continue;
            }
            self.battle_turns = 0;
            // Prompt order mirrors the core's input gate: evolutions and
            // learn prompts outrank dialogue (which can't advance while
            // they're queued).
            if !self.world.pending_evolutions.is_empty() {
                self.input(Input::Evolve { accept: true });
                continue;
            }
            if !self.world.pending_learn_queue.is_empty() {
                // Keep kits current: replace the weakest current move
                // (status moves first), never skip — frozen movesets
                // walk into damper walls with four sound moves.
                let replace = self
                    .world
                    .pending_learn_queue
                    .first()
                    .and_then(|(party_index, _)| self.world.party.get(*party_index))
                    .map(|member| {
                        let power = |id: &undersong_core::ids::MoveId| {
                            self.world
                                .registry
                                .as_ref()
                                .and_then(|r| r.moves.get(id))
                                .map_or(0, |spec| spec.power)
                        };
                        member
                            .moves
                            .iter()
                            .enumerate()
                            .min_by_key(|(_, m)| power(&m.id))
                            .map_or(0, |(i, _)| u8::try_from(i).unwrap_or(0))
                    });
                self.input(Input::Learn { replace });
                continue;
            }
            if self.world.dialogue.is_some() {
                // Open choice lists are the caller's decision — confirming
                // blind would pick option 0 (it once silently chose an
                // ENDING that way).
                if self
                    .world
                    .dialogue
                    .as_ref()
                    .is_some_and(|d| d.choice.is_some())
                {
                    return;
                }
                self.input(Input::Interact);
                continue;
            }
            if self.world.shop.is_some() {
                self.input(Input::ShopClose);
                continue;
            }
            return;
        }
        panic!(
            "drain did not settle: map {} at {:?} battle {:?} party {:?}",
            self.world.current_map,
            self.world.player,
            self.world.battle.as_ref().map(|s| format!(
                "{:?} us {} hp{} vs {} hp{}",
                s.context,
                s.state.sides[0].active_mote().species,
                s.state.sides[0].active_mote().hp,
                s.state.sides[1].active_mote().species,
                s.state.sides[1].active_mote().hp,
            )),
            self.world
                .party
                .iter()
                .map(|p| format!(
                    "{} L{} hp{:?} pp{:?}",
                    p.species,
                    p.level,
                    p.hp,
                    p.moves.iter().map(|m| m.pp).collect::<Vec<_>>()
                ))
                .collect::<Vec<_>>(),
        );
    }

    pub fn step(&mut self, dir: Facing) {
        if self.world.facing != dir && self.world.dialogue.is_none() {
            self.input(Input::Step(dir));
        }
        self.input(Input::Step(dir));
        self.drain();
    }

    pub fn walk(&mut self, legs: &[(Facing, u32)]) {
        for (dir, count) in legs {
            for _ in 0..*count {
                self.step(*dir);
            }
        }
    }

    /// Adaptive straight-line move along one axis until the coordinate
    /// matches (panics if blocked twice on the same tile — bad plan).
    /// Stops early when a warp changes the map (door semantics).
    pub fn go_y(&mut self, y: u32) {
        let map = self.world.current_map.clone();
        let mut stuck = 0;
        while self.world.player.1 != y && self.world.current_map == map {
            let before = self.world.player;
            let dir = if self.world.player.1 < y { Up } else { Down };
            self.step(dir);
            if self.world.player == before && self.world.current_map == map {
                stuck += 1;
                // Wandering NPCs are transient blocks: let them move.
                for _ in 0..4 {
                    self.input(Input::Tick);
                }
                assert!(
                    stuck < 8,
                    "go_y stuck at {:?} heading {y} in {}",
                    before,
                    self.world.current_map
                );
            } else {
                stuck = 0;
            }
        }
    }

    pub fn go_x(&mut self, x: u32) {
        let map = self.world.current_map.clone();
        let mut stuck = 0;
        while self.world.player.0 != x && self.world.current_map == map {
            let before = self.world.player;
            let dir = if self.world.player.0 < x { Right } else { Left };
            self.step(dir);
            if self.world.player == before && self.world.current_map == map {
                stuck += 1;
                for _ in 0..4 {
                    self.input(Input::Tick);
                }
                assert!(
                    stuck < 8,
                    "go_x stuck at {:?} heading {x} in {}",
                    before,
                    self.world.current_map
                );
            } else {
                stuck = 0;
            }
        }
    }

    pub fn interact(&mut self) {
        self.input(Input::Interact);
        self.drain();
    }

    pub fn face(&mut self, dir: Facing) {
        if self.world.facing != dir {
            self.input(Input::Step(dir));
        }
    }

    /// Buys up to `count` of an item from an OPEN shop (cursor walk),
    /// then closes it. No-ops politely when broke or absent.
    pub fn buy(&mut self, item: &str, count: u32) {
        if self.world.shop.is_none() {
            return;
        }
        let stock: Vec<String> = self
            .world
            .shop
            .as_ref()
            .map(|(items, _)| items.iter().map(ToString::to_string).collect())
            .unwrap_or_default();
        if let Some(target) = stock.iter().position(|s| s == item) {
            loop {
                let cursor = self.world.shop.as_ref().map(|(_, c)| *c).unwrap_or(0);
                match cursor.cmp(&target) {
                    std::cmp::Ordering::Less => self.input(Input::ShopCursor(1)),
                    std::cmp::Ordering::Greater => self.input(Input::ShopCursor(-1)),
                    std::cmp::Ordering::Equal => break,
                };
            }
            for _ in 0..count {
                self.input(Input::ShopBuy);
            }
        }
    }

    /// Buys best affordable potions, walking down tiers until either
    /// `count` landed or even small ones beyond wallet. Shop must be open.
    pub fn buy_potions(&mut self, count: u32) {
        let mut bought = 0;
        for tier in ["potion_x", "potion_l", "potion_m", "potion_s"] {
            while bought < count {
                let before: u32 = self.world.bag.get(&tier.into()).copied().unwrap_or(0);
                self.buy(tier, 1);
                let after: u32 = self.world.bag.get(&tier.into()).copied().unwrap_or(0);
                if after > before {
                    bought += 1;
                } else {
                    break; // can't afford this tier — drop down
                }
            }
        }
    }

    pub fn close_shop(&mut self) {
        if self.world.shop.is_some() {
            self.input(Input::ShopClose);
        }
    }

    /// Steps onto a mart doorstep tile from one tile south, pumping the
    /// clerk line until the shop opens.
    pub fn open_shop_at(&mut self, x: u32, y: u32) {
        self.go_x(x);
        self.go_y(y);
        // The doorstep trigger fired during the walk; the drain inside
        // step() may have closed it — step off and back on with raw
        // inputs, then pump dialogue manually.
        self.input(Input::Step(Down));
        self.input(Input::Step(Down));
        self.input(Input::Step(Up));
        self.input(Input::Step(Up));
        for _ in 0..10 {
            if self.world.shop.is_some() {
                return;
            }
            self.input(Input::Interact);
        }
    }

    /// Era retry loop: run `attempt` until `flag` is set, invoking
    /// `recover` after each failed try (whiteouts re-heal; defeat flags
    /// only set on wins, so re-engaging is always legal).
    pub fn until_flag(
        &mut self,
        flag: &str,
        tries: u32,
        mut attempt: impl FnMut(&mut Driver),
        mut recover: impl FnMut(&mut Driver),
    ) {
        for attempt_no in 0..tries {
            if self.world.vars.flags.contains(flag) {
                return;
            }
            if attempt_no > 0 {
                recover(self);
            }
            attempt(self);
            self.drain();
            if !self.world.vars.flags.contains(flag) && std::env::var_os("DRIVER_DEBUG").is_some() {
                eprintln!(
                    "  try {attempt_no} failed for {flag}: map {} at {:?} party {:?} bag potions l{:?} x{:?} money {}",
                    self.world.current_map,
                    self.world.player,
                    self.world
                        .party
                        .iter()
                        .map(|p| format!("{} L{} hp{:?}", p.species, p.level, p.hp))
                        .collect::<Vec<_>>(),
                    self.world.bag.get(&"potion_l".into()),
                    self.world.bag.get(&"potion_x".into()),
                    self.world.money,
                );
            }
        }
        if !self.world.vars.flags.contains(flag) && std::env::var_os("DRIVER_TRACE").is_some() {
            for line in &self.trace {
                eprintln!("    | {line}");
            }
        }
        assert!(
            self.world.vars.flags.contains(flag),
            "until_flag({flag}) exhausted {tries} tries: map {} at {:?} party {:?}",
            self.world.current_map,
            self.world.player,
            self.world
                .party
                .iter()
                .map(|p| format!("{} L{} hp{:?}", p.species, p.level, p.hp))
                .collect::<Vec<_>>(),
        );
    }

    /// Pumps dialogue to the next open choice and confirms option
    /// `index` (Down × index from the top).
    pub fn answer_choice(&mut self, index: u32) {
        for _ in 0..60 {
            if self
                .world
                .dialogue
                .as_ref()
                .is_some_and(|d| d.choice.is_some())
            {
                break;
            }
            if self.world.dialogue.is_none() {
                return; // nothing asked
            }
            self.input(Input::Interact);
        }
        for _ in 0..index {
            self.input(Input::Step(Down));
        }
        self.input(Input::Interact);
        self.drain();
    }

    /// Catch-sweep: bounce a patch field, belling every species not
    /// yet in the caught Score, until `idle_cap` iterations pass with
    /// no new catch (or the bells run out).
    pub fn sweep_catch(&mut self, x0: u32, y0: u32, x1: u32, y1: u32, idle_cap: u32) {
        let mut idle = 0u32;
        let mut caught = self.caught_count();
        let start_caught = caught;
        let start_map = self.world.current_map.clone();
        let mut battles_seen = 0u32;
        let mut bells_rung = 0u32;
        macro_rules! exit_log {
            ($reason:expr) => {
                if std::env::var_os("DRIVER_DEBUG").is_some() {
                    eprintln!(
                        "  sweep {} ({},{}): +{} caught (total {}), bells {}, battles {}, rung {}, exit: {}",
                        start_map, x0, y0,
                        self.caught_count() - start_caught,
                        self.caught_count(),
                        self.world.bag.get(&"fermata".into()).copied().unwrap_or(0),
                        battles_seen,
                        bells_rung,
                        $reason,
                    );
                }
            };
        }
        for _ in 0..6000 {
            if idle > idle_cap {
                exit_log!("idle");
                return;
            }
            if self.world.battle.is_some() {
                battles_seen += 1;
                let bells: u32 = [
                    "fermata",
                    "grand_fermata",
                    "maestro_fermata",
                    "overture_bell",
                    "vesper_bell",
                    "cradle_bell",
                ]
                .iter()
                .map(|id| self.world.bag.get(&(*id).into()).copied().unwrap_or(0))
                .sum();
                // Sip a potion when the fielded mote runs low — a faint
                // mid-sweep costs the wallet and the tour position.
                let hurt = self.world.battle.as_ref().is_some_and(|session| {
                    let us = session.state.sides[0].active_mote();
                    u32::from(us.hp) * 5 < u32::from(us.max_hp()) * 2
                });
                let has_potion = ["potion_x", "potion_l", "potion_m", "potion_s"]
                    .iter()
                    .any(|t| self.world.bag.get(&(*t).into()).copied().unwrap_or(0) > 0);
                if hurt && has_potion {
                    self.input(Input::Battle(BattleCmd::Item));
                    idle += 1;
                    continue;
                }
                if bells == 0 {
                    // Nothing left to ring — finish this fight and stop
                    // burning field time.
                    self.battle_policy();
                    idle += idle_cap / 8;
                    continue;
                }
                let (foe_new, foe_weak) = self
                    .world
                    .battle
                    .as_ref()
                    .map(|session| {
                        let foe = session.state.sides[1].active_mote();
                        (
                            !self
                                .world
                                .vars
                                .flags
                                .contains(&format!("dex.caught.{}", foe.species)),
                            u32::from(foe.hp) * 3 <= u32::from(foe.max_hp()),
                        )
                    })
                    .unwrap_or((false, false));
                if foe_new && bells > 0 {
                    let session = self.world.battle.as_ref().expect("battle open");
                    let our = &session.state.sides[0];
                    let active_index = usize::from(our.positions[0].party_index);
                    let foe_asleep = session.state.sides[1].active_mote().status.is_some();
                    let lull_slot = our.party[active_index]
                        .moves
                        .iter()
                        .position(|m| m.spec.id.as_str() == "frost_lull" && m.pp > 0);
                    let turn_zero = self
                        .world
                        .battle
                        .as_ref()
                        .is_some_and(|b| b.state.turn == 0);
                    let overtures = self
                        .world
                        .bag
                        .get(&"overture_bell".into())
                        .copied()
                        .unwrap_or(0);
                    if foe_asleep || foe_weak {
                        bells_rung += 1;
                        self.input(Input::Battle(BattleCmd::Bell));
                    } else if turn_zero && overtures > 0 {
                        // The opening ring at ×4 — the only full-HP bell
                        // worth its price.
                        self.input(Input::Battle(BattleCmd::Bell));
                    } else if let Some(slot) = lull_slot {
                        self.input(Input::Battle(BattleCmd::Move {
                            slot: u8::try_from(slot).unwrap_or(0),
                        }));
                    } else {
                        // Keep ringing — fermatas land eventually, and
                        // the war chest covers the spread.
                        self.input(Input::Battle(BattleCmd::Bell));
                    }
                } else {
                    self.input(Input::Battle(BattleCmd::Run));
                }
                idle += 1;
                continue;
            }
            if self.world.dialogue.is_some()
                || !self.world.pending_learn_queue.is_empty()
                || !self.world.pending_evolutions.is_empty()
            {
                self.drain();
                continue;
            }
            if self.world.current_map != start_map {
                exit_log!("left map");
                return;
            }
            let now = self.caught_count();
            if now > caught {
                caught = now;
                idle = 0;
            }
            if !((x0..=x1).contains(&self.world.player.0)
                && (y0..=y1).contains(&self.world.player.1))
            {
                let cx = (x0 + x1) / 2;
                self.go_x(cx);
                self.go_y(y0);
            }
            // Oscillate between two in-field rows — parity bouncing
            // leaks out of odd-anchored fields, and every encounter
            // that fires inside a recenter leg gets policy-killed
            // instead of belled.
            let dir = if self.world.player.1 <= y0 { Up } else { Down };
            self.face(dir);
            self.input(Input::Step(dir));
            idle += 1;
        }
        exit_log!("budget");
    }

    /// Walks the region chain one hop toward `target`, from wherever a
    /// whiteout left us. Returns true when standing in `target`.
    pub fn tour_hop(&mut self, target: &str) -> bool {
        let here = self.world.current_map.to_string();
        if here == target {
            return true;
        }
        const CHAIN: [&str; 12] = [
            "pausa_village",
            "route_2",
            "arbor_vale",
            "route_3",
            "route_4",
            "port_calando",
            "route_5",
            "voltaccia",
            "route_5b",
            "hollowfen",
            "graven_pass",
            "frostine",
        ];
        let index_of = |m: &str| CHAIN.iter().position(|c| *c == m);
        // Off-chain maps first: drop to their town.
        match here.as_str() {
            "prelude_town" => {
                self.go_x(10);
                self.go_y(0);
                return false;
            }
            "route_1" => {
                self.go_x(6);
                self.go_y(0);
                return false;
            }
            "quiet_coast" => {
                self.go_y(6);
                self.go_x(22);
                self.go_y(7);
                self.go_x(23);
                return false;
            }
            "route_6" | "cadenza_city" | "quartet_spire" | "the_vault" => {
                let northbound = matches!(target, "cadenza_city" | "quartet_spire" | "the_vault");
                if here == "cadenza_city" && !northbound {
                    self.go_y(11);
                    self.go_x(12);
                    self.go_y(17);
                } else if here == "route_6" {
                    self.go_x(6);
                    if northbound {
                        self.go_y(19); // → cadenza
                    } else {
                        self.go_y(0); // → frostine
                    }
                }
                return false;
            }
            _ => {}
        }
        let (Some(from), Some(to)) = (index_of(&here), index_of(target)) else {
            // target off-chain: walk toward frostine then route_6 north
            if target == "cadenza_city" || target == "route_6" {
                if here == "frostine" {
                    self.go_x(12); // east lane — the rest stop blocks x11
                    self.go_y(7);
                    self.go_x(9);
                    self.go_x(12);
                    self.go_y(13);
                    return false;
                }
                return self.chain_step(true);
            }
            return false;
        };
        let _ = (from, to);
        self.chain_step(index_of(&here) < index_of(target))
    }

    /// One hop along the chain. `north` = toward Frostine.
    fn chain_step(&mut self, north: bool) -> bool {
        match (self.world.current_map.as_str(), north) {
            ("pausa_village", true) => {
                self.go_y(6);
                self.go_x(0);
            }
            ("route_2", true) => {
                // Weave: gardener owns (9,6), courier owns (20,7).
                self.go_y(7);
                self.go_x(19);
                if self.world.current_map.as_str() == "route_2" {
                    self.go_y(6);
                    self.go_x(29);
                }
            }
            ("route_2", false) => {
                if self.world.player.0 > 10 {
                    self.go_y(6);
                    self.go_x(10);
                }
                if self.world.current_map.as_str() == "route_2" {
                    self.go_y(7);
                    self.go_x(0);
                }
            }
            ("arbor_vale", true) => {
                self.go_x(13);
                self.go_y(9);
                self.go_x(10);
                self.go_x(13);
                self.go_y(14);
                self.go_x(12);
                self.go_y(15);
            }
            ("arbor_vale", false) => {
                self.go_x(13);
                self.go_y(9);
                self.go_x(10);
                self.go_y(8);
                self.go_x(2);
                self.go_y(7);
                self.go_x(0);
            }
            ("route_3", true) => {
                self.go_y(10);
                self.go_x(6);
                self.go_y(25);
            }
            ("route_3", false) => {
                self.go_y(10);
                self.go_x(6);
                self.go_y(0);
            }
            ("route_4", true) => {
                self.go_x(6);
                self.go_y(21);
            }
            ("route_4", false) => {
                self.go_x(6);
                self.go_y(0);
            }
            ("port_calando", true) => {
                self.go_x(12); // the east lane clears both buildings
                self.go_y(16);
            }
            ("port_calando", false) => {
                self.go_x(12);
                self.go_y(2);
                self.go_x(11);
                self.go_y(0);
            }
            ("route_5", true) => {
                self.go_y(10);
                self.go_x(6);
                self.go_y(23);
            }
            ("route_5", false) => {
                self.go_y(10);
                self.go_x(6);
                self.go_y(0);
            }
            ("voltaccia", true) => {
                self.go_x(12);
                self.go_y(9);
                self.go_x(9);
                self.go_x(12);
                self.go_y(15);
            }
            ("voltaccia", false) => {
                self.go_x(12);
                self.go_y(9);
                self.go_x(9);
                self.go_x(11);
                self.go_y(0);
            }
            ("route_5b", true) => {
                self.go_y(7);
                self.go_x(16);
                if self.world.current_map.as_str() == "route_5b" {
                    self.go_y(8);
                    self.go_x(24);
                    self.go_y(7);
                    self.go_x(25);
                }
            }
            ("route_5b", false) => {
                self.go_y(8);
                self.go_x(2);
                self.go_y(7);
                self.go_x(0);
            }
            ("hollowfen", true) => {
                self.go_x(12);
                self.go_y(7);
                self.go_x(9);
                self.go_x(12);
                self.go_y(12);
                self.go_x(10);
                self.go_y(13);
            }
            ("hollowfen", false) => {
                self.go_x(12);
                self.go_y(7);
                self.go_x(9);
                self.go_y(7);
                self.go_x(0);
            }
            ("graven_pass", true) => {
                if self.world.player.1 < 20 {
                    self.go_y(19);
                    self.go_x(8);
                    self.go_y(24);
                }
                self.go_x(6);
                self.go_y(27);
            }
            ("graven_pass", false) => {
                if self.world.player.1 > 23 {
                    self.go_y(24);
                    self.go_x(8);
                    self.go_y(19);
                }
                self.go_x(6);
                self.go_y(0);
            }
            ("frostine", false) => {
                self.go_x(12);
                self.go_y(7);
                self.go_x(9);
                self.go_x(11);
                self.go_y(0);
            }
            _ => return self.world.current_map.as_str() == "frostine" && north,
        }
        false
    }

    /// Walks the chain until `target` (or gives up after 16 hops).
    pub fn tour_goto(&mut self, target: &str) {
        for _ in 0..16 {
            if self.tour_hop(target) {
                return;
            }
        }
    }

    pub fn caught_count(&self) -> usize {
        self.world
            .vars
            .flags
            .iter()
            .filter(|f| f.starts_with("dex.caught."))
            .count()
    }

    pub fn has_flag(&self, flag: &str) -> bool {
        self.world.vars.flags.contains(flag)
    }

    /// Grind on the CURRENT tile pair (vertical bounce) until the lead
    /// reaches `level`; recovers to (x, y) after whiteouts via the
    /// caller-provided retrek closure.
    pub fn grind_until(&mut self, level: u8, mut retrek: impl FnMut(&mut Driver)) {
        for iteration in 0..40000 {
            if iteration % 2000 == 1999 && std::env::var_os("DRIVER_DEBUG").is_some() {
                eprintln!(
                    "  grind[{iteration}] target {level}: lead L{} map {} at {:?} clock {}",
                    self.world.party.first().map(|p| p.level).unwrap_or(0),
                    self.world.current_map,
                    self.world.player,
                    self.world.clock_ticks,
                );
            }
            if self.world.party.first().map(|p| p.level).unwrap_or(0) >= level {
                return;
            }
            if self.world.battle.is_some()
                || self.world.dialogue.is_some()
                || !self.world.pending_evolutions.is_empty()
                || !self.world.pending_learn_queue.is_empty()
            {
                self.drain();
                continue;
            }
            retrek(self);
            let dir = if self.world.player.1.is_multiple_of(2) {
                Up
            } else {
                Down
            };
            if self.world.facing != dir {
                self.input(Input::Step(dir));
            }
            self.input(Input::Step(dir));
        }
        panic!("grind_until({level}) did not finish");
    }
}

pub const ACT_SEED: u64 = 0x00AC_71AC;

pub fn content_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content")
}

#[expect(clippy::too_many_lines, reason = "one continuous scripted run")]
pub fn run_act1() -> Driver {
    let world = game::world::load_game_world(&crate::common::content_root(), ACT_SEED)
        .expect("game world loads");
    let mut driver = Driver::new(world);

    // ---- Badge 1 (the P3 route, adaptive) --------------------------------
    driver.walk(&[(Left, 2)]); // (6,4)
    driver.go_y(9); // lab door → pausa_lab (4,1)
    assert_eq!(driver.world.current_map.as_str(), "pausa_lab");
    driver.go_y(3); // intro fires at (4,2)
    driver.interact();
    driver.answer_choice(0); // the starter choice → fanfyre
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
                    d.buy_potions(3);
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
    if driver
        .world
        .bag
        .get(&"fermata".into())
        .copied()
        .unwrap_or(0)
        == 0
    {
        // No bells left — detour through the arbor mart (the recovery
        // path buys them) before hunting the third voice.
        back_to_route4(&mut driver);
        driver.go_y(6);
        driver.go_x(10);
    }
    let catch_start_bells: u32 = driver
        .world
        .bag
        .get(&"fermata".into())
        .copied()
        .unwrap_or(0);
    // The hunt survives whiteouts: re-enter the field and keep ringing.
    'hunt: for _hunt_round in 0..4 {
        if driver.world.party.len() >= 3 {
            break;
        }
        if driver.world.current_map.as_str() != "route_4" {
            back_to_route4(&mut driver);
            if driver.world.current_map.as_str() != "route_4" {
                break;
            }
            driver.go_y(6);
            driver.go_x(10);
        }
        for _ in 0..400 {
            if driver.world.party.len() >= 3 {
                break 'hunt;
            }
            if driver.world.battle.is_some() {
                let bells = driver
                    .world
                    .bag
                    .get(&"fermata".into())
                    .copied()
                    .unwrap_or(0);
                let foe_weak = driver.world.battle.as_ref().is_some_and(|session| {
                    let foe = session.state.sides[1].active_mote();
                    u32::from(foe.hp) * 3 <= u32::from(foe.max_hp())
                });
                if bells > 0 && foe_weak {
                    driver.input(Input::Battle(game::session::BattleCmd::Bell));
                } else if bells > 0 {
                    driver.battle_policy(); // soften it up first
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
                continue 'hunt; // whiteout — recover and re-enter
            }
            let dir = if driver.world.player.1.is_multiple_of(2) {
                Up
            } else {
                Down
            };
            driver.face(dir);
            driver.input(Input::Step(dir));
        }
    }
    if std::env::var_os("DRIVER_DEBUG").is_some() {
        eprintln!(
            "catch exit: party {} bells {}→{} map {} at {:?} money {}",
            driver.world.party.len(),
            catch_start_bells,
            driver
                .world
                .bag
                .get(&"fermata".into())
                .copied()
                .unwrap_or(0),
            driver.world.current_map,
            driver.world.player,
            driver.world.money,
        );
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
                    d.buy_potions(4);
                    if d.world.bag.get(&"fermata".into()).copied().unwrap_or(0) < 2 {
                        d.buy("fermata", 3); // the catch detour needs bells
                    }
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

    // One more training pass on route_4's field before the doubles
    // hall — Stelt's maridian tanks underleveled pairs all day.
    driver.go_x(11);
    driver.go_y(0); // south doors → route_4 (6,20)
    if driver.world.current_map.as_str() == "route_4" {
        driver.go_y(7);
        driver.go_x(10);
        // The doubles hall judges the PAIR: bench the lead and train
        // the partner on this field first.
        driver.input(Input::BoxDeposit { party_index: 0 });
        driver.grind_until(25, |d| {
            if d.world.current_map.as_str() == "port_calando" {
                d.go_y(11);
                d.go_x(11);
                d.go_y(0);
                d.go_y(7);
                d.go_x(10);
            } else if d.world.current_map.as_str() == "arbor_vale" {
                d.go_x(13);
                d.go_y(14);
                d.go_x(12);
                d.go_y(15);
                d.go_x(6);
                d.go_y(25);
                d.go_y(7);
                d.go_x(10);
            } else if d.world.current_map.as_str() == "route_4"
                && !((9..=11).contains(&d.world.player.0) && (5..=9).contains(&d.world.player.1))
            {
                d.go_x(6);
                d.go_y(7);
                d.go_x(10);
            }
        });
        if driver.world.party.len() > 1 {
            driver.input(Input::BoxDeposit { party_index: 1 }); // bench the third
        }
        let boxes = u32::try_from(driver.world.boxes.len()).unwrap_or(1);
        driver.input(Input::BoxWithdraw {
            box_index: boxes.saturating_sub(2), // the old lead
        });
        let boxes = u32::try_from(driver.world.boxes.len()).unwrap_or(1);
        driver.input(Input::BoxWithdraw {
            box_index: boxes.saturating_sub(1), // the third voice
        });
        driver.grind_until(29, |d| {
            if d.world.current_map.as_str() == "port_calando" {
                d.go_y(11);
                d.go_x(11);
                d.go_y(0); // → route_4
                d.go_y(7);
                d.go_x(10);
            } else if d.world.current_map.as_str() == "arbor_vale" {
                d.go_x(13);
                d.go_y(14);
                d.go_x(12);
                d.go_y(15);
                d.go_x(6);
                d.go_y(25);
                d.go_y(7);
                d.go_x(10);
            } else if d.world.current_map.as_str() == "route_4"
                && !((9..=11).contains(&d.world.player.0) && (5..=9).contains(&d.world.player.1))
            {
                d.go_x(6);
                d.go_y(7);
                d.go_x(10);
            }
        });
        if driver.world.current_map.as_str() == "route_4" {
            driver.go_x(6);
            driver.go_y(21); // back → calando (11,1)
        }
    }
    for _ in 0..3 {
        match driver.world.current_map.as_str() {
            "arbor_vale" => {
                driver.go_x(13);
                driver.go_y(14);
                driver.go_x(12);
                driver.go_y(15);
                driver.go_x(6);
                driver.go_y(25);
                driver.go_y(21);
            }
            "route_4" => {
                driver.go_x(6);
                driver.go_y(21);
            }
            _ => break,
        }
    }
    assert_eq!(driver.world.current_map.as_str(), "port_calando");
    driver.go_y(11);
    driver.go_x(9); // rest + bank the grind survivor's wallet
    driver.open_shop_at(4, 11);
    if std::env::var_os("DRIVER_DEBUG").is_some() {
        eprintln!(
            "  calando bank: map {} at {:?} shop {} money {}",
            driver.world.current_map,
            driver.world.player,
            driver.world.shop.is_some(),
            driver.world.money,
        );
    }
    driver.buy_potions(4);
    driver.close_shop();
    driver.go_y(11);
    driver.go_x(19);
    driver.go_y(12); // hall_3 door → (6,1)
    assert_eq!(driver.world.current_map.as_str(), "hall_3");

    // Hall 3: the doubles gauntlet, retry-structured.
    /// From anywhere inside hall_3, return to the (6,5) staging tile
    /// (walls at y7 x2..8 and y11 x5..11 dictate the channels).
    fn hall3_stage(d: &mut Driver) {
        if d.world.current_map.as_str() != "hall_3" {
            return;
        }
        if d.world.player.1 > 10 {
            d.go_x(3);
            d.go_y(10);
        }
        if d.world.player.1 > 6 {
            d.go_x(9);
            d.go_y(5);
        }
        d.go_x(6);
        d.go_y(5);
    }

    fn back_to_hall3(d: &mut Driver) {
        for _ in 0..4 {
            match d.world.current_map.as_str() {
                "port_calando" => {
                    d.go_y(11);
                    d.go_x(9); // rest doorstep heals
                    d.open_shop_at(4, 11);
                    d.buy_potions(4);
                    d.close_shop();
                    d.go_y(11);
                    d.go_x(19);
                    d.go_y(12); // → hall_3 (6,1)
                }
                "hall_3" => return,
                _ => return,
            }
        }
    }

    driver.until_flag(
        "trainer.hall3_duo_a.defeated",
        4,
        |d| {
            if d.world.current_map.as_str() == "hall_3" {
                hall3_stage(d);
                d.go_x(5); // duo_a sight (4,5),(5,5)
                if !d.has_flag("trainer.hall3_duo_a.defeated") {
                    d.go_x(4);
                    d.face(Left);
                    d.interact();
                }
            }
        },
        back_to_hall3,
    );
    back_to_hall3(&mut driver);
    driver.until_flag(
        "trainer.hall3_duo_b.defeated",
        4,
        |d| {
            if d.world.current_map.as_str() == "hall_3" {
                hall3_stage(d);
                d.go_x(9);
                d.go_y(9); // duo_b sight (8,9),(9,9)
                if !d.has_flag("trainer.hall3_duo_b.defeated") {
                    d.face(Right);
                    d.interact();
                }
            }
        },
        back_to_hall3,
    );
    back_to_hall3(&mut driver);
    driver.until_flag(
        "badge.3",
        5,
        |d| {
            // Spend the duo payouts before the maestro: if the bag is
            // dry and the wallet isn't, duck out to the mart first.
            let dry = ["potion_x", "potion_l", "potion_m", "potion_s"]
                .iter()
                .all(|t| d.world.bag.get(&(*t).into()).copied().unwrap_or(0) == 0);
            if d.world.current_map.as_str() == "hall_3" && dry && d.world.money >= 200 {
                hall3_stage(d);
                if d.world.current_map.as_str() == "hall_3" {
                    d.go_y(0); // → calando (19,11)
                }
            }
            if d.world.current_map.as_str() == "port_calando" {
                d.go_y(11);
                d.go_x(9); // rest
                d.open_shop_at(4, 11);
                d.buy_potions(4);
                d.close_shop();
                d.go_y(11);
                d.go_x(19);
                d.go_y(12); // → hall_3
            }
            if d.world.current_map.as_str() == "hall_3" {
                hall3_stage(d);
                d.go_x(9);
                d.go_y(10);
                d.go_x(3);
                d.go_y(12);
                d.go_x(7);
                d.go_y(14);
                if std::env::var_os("DRIVER_DEBUG").is_some() {
                    eprintln!(
                        "  stelt attempt: party {:?} potions m{:?} s{:?} money {}",
                        d.world
                            .party
                            .iter()
                            .map(|p| format!(
                                "{} L{} hp{:?} pp{:?}",
                                p.species,
                                p.level,
                                p.hp,
                                p.moves.iter().map(|m| m.pp).collect::<Vec<_>>()
                            ))
                            .collect::<Vec<_>>(),
                        d.world.bag.get(&"potion_m".into()),
                        d.world.bag.get(&"potion_s".into()),
                        d.world.money,
                    );
                }
                d.interact(); // Maestro Stelt (doubles, T3)
            }
        },
        back_to_hall3,
    );

    // ---- Beat 5: Lull, on the way out (reverse the gauntlet maze) ---------
    hall3_stage(&mut driver);
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
