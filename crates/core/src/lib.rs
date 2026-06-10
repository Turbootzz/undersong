//! Shared vocabulary for Undersong: ids, stats, types, and the battle RNG.
//!
//! Everything here is plain data with no engine, I/O, or clock dependencies.
//! Every other crate in the workspace builds on these definitions
//! (`core ← battle/data/save/script ← game/tools`, docs/03-ARCHITECTURE.md §1).

#![forbid(unsafe_code)]

pub mod chart;
pub mod collections;
pub mod ids;
pub mod moves;
pub mod rng;
pub mod species;
pub mod stats;
pub mod types;
