//! Player/AI intentions for one turn.

use serde::{Deserialize, Serialize};
use undersong_core::moves::Frac;

/// One side's chosen action for the turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Action {
    /// Use the move in slot `0..4` of the active Mote.
    Move { slot: u8 },
    /// Switch to party index `to` (doc 02 v1.1 #17 legality).
    Switch { to: u8 },
    /// Ring a bell (wild battles only; doc 02 §8). `bell_mod` is the
    /// bell's multiplier as a fraction (e.g. Grand Fermata = 3/2).
    UseBell { bell_mod: Frac },
    /// Attempt to flee (wild battles only; doc 02 §12 formula).
    Run,
    /// Explicit pass: the side takes no action this turn. (Replacement
    /// choice after a faint is engine auto-replace until the presenter
    /// adds a prompt flow in P2 — see turn.rs.)
    None,
}

/// Both sides' actions, indexed by side.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TurnActions {
    pub actions: [Action; 2],
}

impl TurnActions {
    pub fn new(side0: Action, side1: Action) -> Self {
        Self {
            actions: [side0, side1],
        }
    }

    pub fn get(&self, side: u8) -> Action {
        assert!(side < 2, "side must be 0 or 1");
        self.actions[usize::from(side)]
    }
}
