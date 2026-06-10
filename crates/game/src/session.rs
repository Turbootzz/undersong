//! The pure session layer: party, bag, money, shops, and the battle
//! bridge. Extends the overworld core so a COMPLETE run — title to badge
//! — is drivable by replay inputs with no engine attached.
//!
//! The bridge owns Individual ↔ BattleMote resolution: persistent Motes
//! are content-referencing (core::individual), battles run on fully
//! resolved state (battle crate), results fold back here.

use std::collections::BTreeMap;

use battle::ai::{AiTier, choose};
use battle::{
    Action, BattleKind, BattleMote, BattleState, MoteBuilder, Outcome, TurnActions, step,
};
use undersong_core::chart::TypeChart;
use undersong_core::ids::{ItemId, SpeciesId, TrainerId};
use undersong_core::individual::{Individual, LearnedMove};
use undersong_core::moves::{Frac, MoveSpec};
use undersong_core::rng::BattleRng;
use undersong_core::species::SpeciesSpec;

/// Everything battles need from content, resolved once at load.
pub struct Registry {
    pub species: BTreeMap<SpeciesId, SpeciesSpec>,
    pub moves: BTreeMap<undersong_core::ids::MoveId, MoveSpec>,
    pub chart: TypeChart,
    pub items: BTreeMap<ItemId, data::ItemDef>,
    pub trainers: BTreeMap<TrainerId, data::Trainer>,
    /// Evolution edges: species → (method level, target) for Level method.
    pub evolutions: BTreeMap<SpeciesId, (u8, SpeciesId)>,
}

impl Registry {
    pub fn from_content(
        core_content: &data::CoreContent,
        pack: &data::RegionPack,
        items: &data::ItemSet,
    ) -> Self {
        let mut moves = BTreeMap::new();
        for spec in core_content.moves.moves.iter().chain(pack.moves.iter()) {
            moves.insert(spec.id.clone(), spec.clone());
        }
        let mut evolutions = BTreeMap::new();
        for motif in pack.motifs.values() {
            if let Some(evolution) = &motif.evolution
                && let data::EvolutionMethod::Level(level) = &evolution.method
            {
                evolutions.insert(motif.id.clone(), (*level, evolution.target.clone()));
            }
        }
        Self {
            species: pack
                .motifs
                .values()
                .map(|m| (m.id.clone(), m.spec()))
                .collect(),
            moves: moves.clone().into_iter().collect(),
            chart: core_content.typechart.clone(),
            items: items
                .items
                .iter()
                .map(|i| (i.id.clone(), i.clone()))
                .collect(),
            trainers: pack.trainers.clone().into_iter().collect(),
            evolutions,
        }
    }

    /// Resolves a persistent Individual into a battle-ready Mote.
    pub fn resolve(&self, individual: &Individual) -> Option<BattleMote> {
        let spec = self.species.get(&individual.species)?;
        let moves: Vec<MoveSpec> = individual
            .moves
            .iter()
            .filter_map(|m| self.moves.get(&m.id).cloned())
            .collect();
        // Ability: slot 0/1 from the species list, or the hidden one.
        let ability_id = if individual.uses_hidden_ability {
            spec.hidden_ability.clone()
        } else {
            spec.abilities
                .get(usize::from(individual.ability_slot))
                .or_else(|| spec.abilities.first())
                .cloned()
        };
        let ability = ability_id
            .map(|id| battle::abilities::Ability::from_id(id.as_str()))
            .unwrap_or_default();
        let held = individual
            .held_item
            .as_ref()
            .map(|id| battle::abilities::HeldItem::from_id(id.as_str()))
            .unwrap_or_default();
        let mut mote = MoteBuilder::new(spec, individual.level)
            .ivs(individual.ivs)
            .evs(individual.evs)
            .nature(individual.nature)
            .moves(moves)
            .ability(ability)
            .held(held)
            .build();
        mote.exp = individual.exp.max(mote.exp);
        // Carry persistent HP/PP/status; PP matches by move id so a
        // missing move cannot shift its neighbours' PP.
        for learned in &individual.moves {
            if let Some(battle_move) = mote.moves.iter_mut().find(|m| m.spec.id == learned.id) {
                battle_move.pp = learned.pp.min(battle_move.spec.pp);
            }
        }
        if let Some(hp) = individual.hp {
            mote.hp = hp.min(mote.max_hp());
        }
        mote.status = individual.status.map(|ailment| match ailment {
            undersong_core::moves::Ailment::Burn => battle::mote::MajorStatus::Burn,
            undersong_core::moves::Ailment::Poison => battle::mote::MajorStatus::Poison,
            undersong_core::moves::Ailment::Toxic => battle::mote::MajorStatus::Toxic { n: 1 },
            undersong_core::moves::Ailment::Paralysis => battle::mote::MajorStatus::Paralysis,
            undersong_core::moves::Ailment::Sleep => battle::mote::MajorStatus::Sleep { turns: 2 },
            undersong_core::moves::Ailment::Freeze => battle::mote::MajorStatus::Freeze,
        });
        Some(mote)
    }

    /// Folds battle results back into the persistent Individual.
    /// PP is matched by move id, not slot — a move that failed registry
    /// lookup at resolve time must not shift its neighbours' PP.
    pub fn fold_back(individual: &mut Individual, mote: &BattleMote) {
        individual.level = mote.level;
        individual.exp = mote.exp;
        individual.evs = mote.evs;
        individual.hp = Some(mote.hp);
        individual.status = mote.status.map(battle::mote::MajorStatus::ailment);
        for battle_move in &mote.moves {
            if let Some(learned) = individual
                .moves
                .iter_mut()
                .find(|m| m.id == battle_move.spec.id)
            {
                learned.pp = battle_move.pp;
            }
        }
    }

    /// A fresh wild Individual rolled from a species + level.
    pub fn wild_individual(
        &self,
        species: &SpeciesId,
        level: u8,
        rng: &mut BattleRng,
    ) -> Option<Individual> {
        let spec = self.species.get(species)?;
        let roll = |rng: &mut BattleRng| u16::try_from(rng.below(32)).expect("iv");
        let ivs = undersong_core::species::StatSpread {
            hp: roll(rng),
            atk: roll(rng),
            def: roll(rng),
            spa: roll(rng),
            spd: roll(rng),
            spe: roll(rng),
        };
        let known: Vec<LearnedMove> = spec
            .learnset
            .iter()
            .filter(|(l, _)| *l <= level)
            .map(|(_, id)| id.clone())
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .take(4)
            .rev()
            .filter_map(|id| {
                self.moves.get(&id).map(|m| LearnedMove {
                    id,
                    pp: m.pp,
                    pp_ups: 0,
                })
            })
            .collect();
        Some(Individual {
            species: species.clone(),
            level,
            exp: spec.growth_curve.total_exp(level),
            ivs,
            evs: crate::session::zero_spread(),
            nature: u8::try_from(rng.below(25)).expect("0..25"),
            ability_slot: 0,
            moves: known,
            status: None,
            held_item: None,
            friendship: 70,
            // Keyshifted: 1/4096 base odds (doc 02 §2).
            keyshifted: rng.chance(1, 4096),
            ot: "wild".into(),
            nickname: None,
            hp: None,
            uses_hidden_ability: false,
        })
    }
}

pub fn zero_spread() -> undersong_core::species::StatSpread {
    undersong_core::species::StatSpread {
        hp: 0,
        atk: 0,
        def: 0,
        spa: 0,
        spd: 0,
        spe: 0,
    }
}

/// Why this battle is happening; decides rewards and exits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BattleContext {
    Wild { species: SpeciesId, level: u8 },
    Trainer { id: TrainerId },
}

/// A running battle inside the session.
pub struct BattleSession {
    pub state: BattleState,
    pub context: BattleContext,
    /// Party index of the player's active battle slot 0 mote.
    pub party_map: Vec<usize>,
    /// The wild Individual being fought (for capture).
    pub wild: Option<Individual>,
    pub foe_tier: AiTier,
    /// Pending learn prompts: (party index, move id).
    pub pending_learn: Vec<(usize, undersong_core::ids::MoveId)>,
    /// Every event across the whole battle (level-up tracking).
    pub last_events_all: Vec<battle::BattleEvent>,
}

/// Player battle intentions (replay vocabulary).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum BattleCmd {
    Move {
        slot: u8,
    },
    Switch {
        to: u8,
    },
    /// Use a bell item from the bag.
    Bell,
    /// Use a healing item on the active Mote (consumes the turn).
    Item,
    Run,
}

impl BattleSession {
    /// Starts a wild battle from the player's party.
    pub fn wild(
        registry: &Registry,
        party: &[Individual],
        wild: Individual,
        rng: &mut BattleRng,
    ) -> Option<BattleSession> {
        let (motes, party_map) = resolve_party(registry, party);
        if motes.is_empty() {
            return None;
        }
        let foe = registry.resolve(&wild)?;
        let _ = rng;
        Some(BattleSession {
            state: BattleState::new(BattleKind::Wild, motes, vec![foe], registry.chart.clone()),
            context: BattleContext::Wild {
                species: wild.species.clone(),
                level: wild.level,
            },
            party_map,
            wild: Some(wild),
            foe_tier: AiTier::T0,
            pending_learn: Vec::new(),
            last_events_all: Vec::new(),
        })
    }

    /// Starts a trainer battle.
    pub fn trainer(
        registry: &Registry,
        party: &[Individual],
        trainer: &data::Trainer,
        rng: &mut BattleRng,
    ) -> Option<BattleSession> {
        let (motes, party_map) = resolve_party(registry, party);
        if motes.is_empty() {
            return None;
        }
        let mut foes = Vec::new();
        for member in &trainer.party {
            let mut individual = registry.wild_individual(&member.species, member.level, rng)?;
            if let Some(ivs) = &member.ivs {
                individual.ivs = *ivs;
            }
            if let Some(moves) = &member.moves {
                individual.moves = moves
                    .iter()
                    .filter_map(|id| {
                        registry.moves.get(id).map(|m| LearnedMove {
                            id: id.clone(),
                            pp: m.pp,
                            pp_ups: 0,
                        })
                    })
                    .collect();
            }
            individual.ot = trainer.id.to_string();
            foes.push(registry.resolve(&individual)?);
        }
        let tier = match trainer.ai_tier {
            0 => AiTier::T0,
            1 => AiTier::T1,
            2 => AiTier::T2,
            _ => AiTier::T3,
        };
        Some(BattleSession {
            state: BattleState::new(BattleKind::Trainer, motes, foes, registry.chart.clone()),
            context: BattleContext::Trainer {
                id: trainer.id.clone(),
            },
            party_map,
            wild: None,
            foe_tier: tier,
            pending_learn: Vec::new(),
            last_events_all: Vec::new(),
        })
    }

    /// One battle turn from a player command. Returns the event stream.
    pub fn turn(
        &mut self,
        command: BattleCmd,
        bell_mod: Option<Frac>,
        heal: Option<u16>,
        rng: &mut BattleRng,
    ) -> Vec<battle::BattleEvent> {
        let player_action = match command {
            BattleCmd::Move { slot } => Action::Move { slot },
            BattleCmd::Switch { to } => Action::Switch { to },
            BattleCmd::Bell => match bell_mod {
                Some(bell_mod) => Action::UseBell { bell_mod },
                None => Action::Move { slot: 0 },
            },
            BattleCmd::Item => {
                // Item use spends the turn: heal lands first, then the
                // foe acts against a passing player.
                if let Some(amount) = heal {
                    let active = usize::from(self.state.sides[0].positions[0].party_index);
                    self.state.sides[0].party[active].heal(u32::from(amount));
                }
                Action::None
            }
            BattleCmd::Run => Action::Run,
        };
        let foe_action = choose(self.foe_tier, &self.state, 1, rng);
        let (next, events) = step(
            &self.state,
            &TurnActions::new(player_action, foe_action),
            rng,
        );
        self.state = next;
        self.last_events_all.extend(events.iter().cloned());
        events
    }

    pub fn outcome(&self) -> Option<Outcome> {
        self.state.outcome
    }
}

fn resolve_party(registry: &Registry, party: &[Individual]) -> (Vec<BattleMote>, Vec<usize>) {
    let mut motes = Vec::new();
    let mut map = Vec::new();
    // Conscious members first (the battle engine treats index 0 as the
    // sent-out Mote); fainted members are still resolved so exp/fold-back
    // indexes stay aligned, but ordered after conscious ones.
    for (index, individual) in party.iter().enumerate() {
        if individual.hp != Some(0)
            && let Some(mote) = registry.resolve(individual)
        {
            motes.push(mote);
            map.push(index);
        }
    }
    for (index, individual) in party.iter().enumerate() {
        if individual.hp == Some(0)
            && let Some(mote) = registry.resolve(individual)
        {
            motes.push(mote);
            map.push(index);
        }
    }
    (motes, map)
}
