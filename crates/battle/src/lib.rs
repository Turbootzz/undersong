//! The pure battle simulation — Undersong's crown jewel.
//!
//! Contract (docs/03-ARCHITECTURE.md §2): a deterministic pure state
//! machine. No Bevy, no I/O, no wall clock; `BattleRng` is the only entropy
//! source. `step()` consumes a state + both sides' actions and returns the
//! next state plus the event stream; every consumer (Bevy presenter, CLI
//! renderer, fuzzer, balance Monte Carlo) is a reader of that stream.
//!
//! `BattleState` is self-contained: move specs and species-derived numbers
//! are resolved into the state at battle start, so a replay
//! `(initial_state, action_log, seed)` reproduces its event stream even if
//! content packs change later.

#![forbid(unsafe_code)]

pub mod abilities;
pub mod actions;
pub mod ai;
pub mod catch;
pub mod damage;
pub mod events;
pub mod exp;
pub mod mote;
pub mod state;
pub mod stats;
pub mod turn;

pub use actions::{Action, TurnActions};
pub use events::{BattleEvent, Outcome, SideId};
pub use mote::{BattleMote, BattleMove, MoteBuilder};
pub use state::{BattleKind, BattleState, Side};
pub use turn::step;

/// Bump consciously when a rule change breaks golden replays
/// (docs/03-ARCHITECTURE.md §2); regenerate goldens in the same commit.
/// v2: doc 02 v1.2 — player-side-only exp awards, two-turn commitment,
/// faint-before-switch ordering.
pub const REPLAY_VERSION: u32 = 3;
