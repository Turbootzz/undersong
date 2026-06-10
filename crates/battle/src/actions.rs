//! Player/AI intentions for one turn.

use serde::{Deserialize, Serialize};
use undersong_core::moves::Frac;

/// One position's chosen action for the turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Action {
    /// Use the move in slot `0..4` of the acting Mote.
    Move { slot: u8 },
    /// Switch to party index `to` (doc 02 v1.1 #17 legality).
    Switch { to: u8 },
    /// Ring a bell (wild battles only; doc 02 §8). `bell_mod` is the
    /// bell's multiplier as a fraction (e.g. Grand Fermata = 3/2).
    UseBell { bell_mod: Frac },
    /// Attempt to flee (wild battles only; doc 02 §12 formula).
    Run,
    /// Explicit pass: the position takes no action this turn.
    /// (Replacement choice after a faint is engine auto-replace until the
    /// presenter adds a prompt flow in P2 — see turn.rs.)
    None,
}

/// One acting position's declaration: who acts, what it does, and which
/// foe position it aims at (doc 02 v1.5 #2 — all launch moves are
/// single-target; the actor declares a target slot).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PositionAction {
    pub side: u8,
    pub position: u8,
    pub action: Action,
    /// Foe position targeted by `Move`; ignored for every other action.
    /// Always 0 in singles.
    #[serde(default)]
    pub target_position: u8,
}

/// All acting positions' declarations for one turn. Singles submits one
/// entry per side; doubles up to two.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TurnActions {
    pub actions: Vec<PositionAction>,
}

impl TurnActions {
    /// The singles shape: one action per side, position 0, target 0.
    pub fn new(side0: Action, side1: Action) -> Self {
        Self {
            actions: vec![
                PositionAction {
                    side: 0,
                    position: 0,
                    action: side0,
                    target_position: 0,
                },
                PositionAction {
                    side: 1,
                    position: 0,
                    action: side1,
                    target_position: 0,
                },
            ],
        }
    }

    /// The doubles shape: explicit per-position declarations. Duplicate
    /// `(side, position)` entries resolve first-wins; missing positions
    /// act as `Action::None` (engine policy, deterministic).
    pub fn doubles(actions: Vec<PositionAction>) -> Self {
        Self { actions }
    }

    /// Compatibility accessor: the side's position-0 action
    /// (`Action::None` when absent).
    pub fn get(&self, side: u8) -> Action {
        assert!(side < 2, "side must be 0 or 1");
        self.actions
            .iter()
            .find(|a| a.side == side && a.position == 0)
            .map_or(Action::None, |a| a.action)
    }
}
