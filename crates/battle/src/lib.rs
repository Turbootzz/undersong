//! The pure battle simulation — Undersong's crown jewel.
//!
//! Contract (docs/03-ARCHITECTURE.md §2): a deterministic pure state machine.
//! No Bevy, no I/O, no wall clock; `undersong_core::rng::BattleRng` is the
//! only entropy source. Implementation arrives in P1
//! (docs/06-ROADMAP.md); P0 only reserves the crate and its boundary.

#![forbid(unsafe_code)]
