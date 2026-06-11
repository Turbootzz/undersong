#![allow(dead_code)] // shared across gate tests; not all use every helper
//! Shared adaptive driver for gate replays (badge run, act 1).

use game::session::BattleCmd;
use game::world::{Input, WorldEvent, WorldState};
use undersong_core::world::Facing::{self, Down, Left, Right, Up};

pub struct Driver {
    pub world: WorldState,
    pub log: Vec<Input>,
    pub dialogue_lines: u32,
    battle_turns: u32,
}

impl Driver {
    pub fn new(world: WorldState) -> Self {
        Driver {
            world,
            log: Vec::new(),
            dialogue_lines: 0,
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
        }
        events
    }

    /// Battle policy: potion under 40%, otherwise T1's pick; doubles
    /// declare per position with choose_doubles. Deterministic.
    fn battle_policy(&mut self) {
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
        let no_backup = self.world.party.iter().filter(|m| m.hp != Some(0)).count() <= 1;
        if critical
            && !has_potion
            && no_backup
            && matches!(session.context, game::session::BattleContext::Wild { .. })
        {
            self.input(Input::Battle(BattleCmd::Run));
            return;
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
                self.input(Input::Learn { replace: None });
                continue;
            }
            if self.world.dialogue.is_some() {
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

    pub fn has_flag(&self, flag: &str) -> bool {
        self.world.vars.flags.contains(flag)
    }

    /// Grind on the CURRENT tile pair (vertical bounce) until the lead
    /// reaches `level`; recovers to (x, y) after whiteouts via the
    /// caller-provided retrek closure.
    pub fn grind_until(&mut self, level: u8, mut retrek: impl FnMut(&mut Driver)) {
        for _ in 0..4000 {
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
