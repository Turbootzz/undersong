//! The in-battle Mote: a fully resolved individual (doc 02 §2).
//!
//! Everything the sim needs is embedded at build time — species numbers,
//! move specs, learnset — so `BattleState` is self-contained and replays
//! survive content edits (doc 03 §2 replay invariant).

use serde::{Deserialize, Serialize};
use undersong_core::ids::SpeciesId;
use undersong_core::moves::{Ailment, MoveSpec};
use undersong_core::species::{GrowthCurve, SpeciesSpec, StatSpread};
use undersong_core::stats::Stat;

use crate::stats::compute_all;

/// A move slot: the full spec plus remaining PP.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BattleMove {
    pub spec: MoveSpec,
    pub pp: u8,
}

/// Major status with its per-Mote counters (doc 02 §5). One at a time;
/// sleep's counter persists through switches, toxic's resets (v1.1 #9).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MajorStatus {
    Burn,
    Poison,
    /// `n` = end-of-turn damage numerator (n/16 of max HP), +1 per tick.
    Toxic {
        n: u8,
    },
    Paralysis,
    /// Remaining sleep turns (rolled 1–3 on apply).
    Sleep {
        turns: u8,
    },
    Freeze,
}

impl MajorStatus {
    pub fn ailment(self) -> Ailment {
        match self {
            MajorStatus::Burn => Ailment::Burn,
            MajorStatus::Poison => Ailment::Poison,
            MajorStatus::Toxic { .. } => Ailment::Toxic,
            MajorStatus::Paralysis => Ailment::Paralysis,
            MajorStatus::Sleep { .. } => Ailment::Sleep,
            MajorStatus::Freeze => Ailment::Freeze,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BattleMote {
    pub species: SpeciesId,
    pub name_key: String,
    pub types: Vec<undersong_core::types::Type>,
    pub level: u8,
    pub exp: u32,
    pub growth: GrowthCurve,
    pub nature: u8,
    pub base_stats: StatSpread,
    /// IVs 0–31 per stat (stored in spread shape for reuse).
    pub ivs: StatSpread,
    /// EVs 0–252 per stat, Σ ≤ 510.
    pub evs: StatSpread,
    /// Cached computed stats for the current level/EVs.
    pub stats: StatSpread,
    pub hp: u16,
    pub status: Option<MajorStatus>,
    pub moves: Vec<BattleMove>,
    pub catch_rate: u8,
    pub base_exp_yield: u16,
    /// `(stat, amount)` EV award to victors (from species).
    pub ev_yield: Vec<(Stat, u8)>,
    /// `(level, move)` for level-up learn events.
    pub learnset: Vec<(u8, undersong_core::ids::MoveId)>,
    /// Resolved ability (doc 02 §10); default None.
    #[serde(default)]
    pub ability: crate::abilities::Ability,
    /// Held battle item (doc 02 v1.5 #1); default None.
    #[serde(default)]
    pub held: crate::abilities::HeldItem,
    /// stage_fright fired already this battle (doc 02 §10: first entry).
    #[serde(default)]
    pub entry_boosted: bool,
}

impl BattleMote {
    pub fn is_fainted(&self) -> bool {
        self.hp == 0
    }

    pub fn max_hp(&self) -> u16 {
        self.stats.hp
    }

    /// Applies damage, flooring at 0. Returns the amount actually dealt.
    pub fn take_damage(&mut self, amount: u32) -> u16 {
        let dealt = u16::try_from(amount.min(u32::from(self.hp))).expect("clamped");
        self.hp -= dealt;
        dealt
    }

    /// Heals up to max HP. Returns the amount actually healed.
    pub fn heal(&mut self, amount: u32) -> u16 {
        let room = u32::from(self.max_hp().saturating_sub(self.hp));
        let healed = u16::try_from(amount.min(room)).expect("clamped");
        self.hp += healed;
        healed
    }

    /// Recomputes stats after a level or EV change (doc 02 v1.1 #11):
    /// current HP rises by the max-HP delta.
    pub fn recompute_stats(&mut self) {
        let old_max = self.stats.hp;
        self.stats = compute_all(
            &self.base_stats,
            &self.ivs,
            &self.evs,
            self.level,
            self.nature,
        );
        let gained = self.stats.hp.saturating_sub(old_max);
        self.hp = (self.hp + gained).min(self.stats.hp);
    }

    /// Total EVs across all stats.
    pub fn ev_sum(&self) -> u32 {
        Stat::ALL
            .into_iter()
            .map(|s| u32::from(self.evs.get(s)))
            .sum()
    }
}

/// Builds a `BattleMote` from a species spec plus individual values.
pub struct MoteBuilder<'a> {
    spec: &'a SpeciesSpec,
    level: u8,
    nature: u8,
    ivs: StatSpread,
    evs: StatSpread,
    moves: Vec<MoveSpec>,
    ability: crate::abilities::Ability,
    held: crate::abilities::HeldItem,
}

impl<'a> MoteBuilder<'a> {
    pub fn new(spec: &'a SpeciesSpec, level: u8) -> Self {
        Self {
            spec,
            level: level.clamp(1, 100),
            nature: 0,
            ivs: ZERO_SPREAD,
            evs: ZERO_SPREAD,
            moves: Vec::new(),
            ability: crate::abilities::Ability::None,
            held: crate::abilities::HeldItem::None,
        }
    }

    pub fn ability(mut self, ability: crate::abilities::Ability) -> Self {
        self.ability = ability;
        self
    }

    pub fn held(mut self, held: crate::abilities::HeldItem) -> Self {
        self.held = held;
        self
    }

    pub fn nature(mut self, nature: u8) -> Self {
        self.nature = nature % 25;
        self
    }

    pub fn ivs(mut self, ivs: StatSpread) -> Self {
        self.ivs = ivs;
        self
    }

    pub fn evs(mut self, evs: StatSpread) -> Self {
        self.evs = evs;
        self
    }

    /// Up to four moves; extras are dropped.
    pub fn moves(mut self, moves: Vec<MoveSpec>) -> Self {
        self.moves = moves;
        self.moves.truncate(4);
        self
    }

    pub fn build(self) -> BattleMote {
        let stats = compute_all(
            &self.spec.base_stats,
            &self.ivs,
            &self.evs,
            self.level,
            self.nature,
        );
        BattleMote {
            species: self.spec.id.clone(),
            name_key: self.spec.name_key.clone(),
            types: self.spec.types.clone(),
            level: self.level,
            exp: self.spec.growth_curve.total_exp(self.level),
            growth: self.spec.growth_curve,
            nature: self.nature,
            base_stats: self.spec.base_stats,
            ivs: self.ivs,
            evs: self.evs,
            hp: stats.hp,
            stats,
            status: None,
            moves: self
                .moves
                .into_iter()
                .map(|spec| BattleMove { pp: spec.pp, spec })
                .collect(),
            catch_rate: self.spec.catch_rate,
            base_exp_yield: self.spec.base_exp_yield,
            ev_yield: self
                .spec
                .ev_yield
                .iter()
                .map(|(stat, amount)| (*stat, *amount))
                .collect(),
            learnset: self.spec.learnset.clone(),
            ability: self.ability,
            held: self.held,
            entry_boosted: false,
        }
    }
}

pub const ZERO_SPREAD: StatSpread = StatSpread {
    hp: 0,
    atk: 0,
    def: 0,
    spa: 0,
    spd: 0,
    spe: 0,
};

/// Uniform 31-IV spread, handy for tests and the simulator.
pub const PERFECT_IVS: StatSpread = StatSpread {
    hp: 31,
    atk: 31,
    def: 31,
    spa: 31,
    spd: 31,
    spe: 31,
};
