//! The battle event stream (docs/03-ARCHITECTURE.md §2).
//!
//! Every consumer — the Bevy presenter, the CLI text renderer, fuzzing,
//! AI evaluation — reads this stream. Events carry mechanical data plus
//! string keys; flavor text lives in string tables, never here.

use serde::{Deserialize, Serialize};
use undersong_core::ids::{MoveId, SpeciesId};
use undersong_core::moves::{Ailment, WeatherKind};
use undersong_core::stats::Stat;
use undersong_core::types::Eff;

/// Which side of the battle: 0 = player/challenger, 1 = opponent/wild.
pub type SideId = u8;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Outcome {
    /// `winner` has conscious Motes, the other side does not.
    Won { winner: SideId },
    /// Wild battle ended by a successful escape.
    Fled { side: SideId },
    /// Wild battle ended by attunement.
    Caught,
    /// Hit the turn-limit safety valve (doc 03 §2: battles terminate
    /// ≤ 1000 turns); treated as a draw.
    Drawn,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BattleEvent {
    TurnStarted {
        n: u16,
    },
    SwitchedIn {
        side: SideId,
        slot: u8,
        species: SpeciesId,
    },
    MoveUsed {
        side: SideId,
        /// Acting position (0 in singles).
        #[serde(default)]
        slot: u8,
        move_id: MoveId,
    },
    /// The built-in no-PP fallback (doc 02 v1.1 #3).
    LastResortUsed {
        side: SideId,
    },
    MoveMissed {
        side: SideId,
    },
    /// First turn of a two-turn move (doc 02 §5 charging volatile).
    ChargeStarted {
        side: SideId,
    },
    MoveFailed {
        side: SideId,
    },
    DamageDealt {
        target: SideId,
        /// Struck position on `target`'s side (0 in singles).
        #[serde(default)]
        target_slot: u8,
        amount: u16,
        crit: bool,
        effectiveness: Eff,
    },
    /// Multi-hit summary after all hits resolve.
    MultiHit {
        hits: u8,
    },
    StatStageChanged {
        target: SideId,
        /// Affected position (0 in singles).
        #[serde(default)]
        slot: u8,
        stat: Stat,
        delta: i8,
        new_stage: i8,
    },
    StatStageClamped {
        target: SideId,
        stat: Stat,
    },
    StatusApplied {
        target: SideId,
        /// Affected position (0 in singles).
        #[serde(default)]
        slot: u8,
        status: Ailment,
    },
    /// An ability visibly acted (presenter shows its name).
    AbilityNote {
        side: SideId,
        ability: crate::abilities::Ability,
    },
    /// A held item visibly acted (Oran Chime chimes…).
    ItemNote {
        side: SideId,
        item: crate::abilities::HeldItem,
    },
    StatusTicked {
        target: SideId,
        status: Ailment,
        damage: u16,
    },
    StatusCured {
        target: SideId,
        status: Ailment,
    },
    /// Sleep skip, freeze skip, paralysis full stop.
    ActionLost {
        side: SideId,
        /// Acting position (0 in singles).
        #[serde(default)]
        slot: u8,
        status: Ailment,
    },
    Flinched {
        side: SideId,
        /// Acting position (0 in singles).
        #[serde(default)]
        slot: u8,
    },
    ConfusionStarted {
        target: SideId,
    },
    ConfusionEnded {
        target: SideId,
    },
    HurtItselfInConfusion {
        side: SideId,
        damage: u16,
    },
    Healed {
        target: SideId,
        /// Affected position (0 in singles).
        #[serde(default)]
        slot: u8,
        amount: u16,
    },
    Drained {
        from: SideId,
        amount: u16,
    },
    Recoiled {
        side: SideId,
        /// Acting position (0 in singles).
        #[serde(default)]
        slot: u8,
        amount: u16,
    },
    SeededDrain {
        from: SideId,
        amount: u16,
    },
    WeatherChanged {
        kind: Option<WeatherKind>,
    },
    WeatherChip {
        target: SideId,
        amount: u16,
    },
    Fainted {
        target: SideId,
        /// Fainted position (0 in singles).
        #[serde(default)]
        slot: u8,
    },
    ExpGained {
        side: SideId,
        slot: u8,
        amount: u32,
    },
    LeveledUp {
        side: SideId,
        slot: u8,
        level: u8,
    },
    /// The Mote's learnset offers a move at the new level; the game layer
    /// runs the learn/replace prompt.
    MoveLearnable {
        side: SideId,
        slot: u8,
        move_id: MoveId,
    },
    /// Attunement attempt (doc 02 §8): `rings` passed checks (0–3 on
    /// failure short of the 4th, 4 = settled).
    AttuneAttempt {
        rings: u8,
        caught: bool,
    },
    EscapeAttempt {
        side: SideId,
        fled: bool,
    },
    BattleEnded {
        outcome: Outcome,
    },
}
