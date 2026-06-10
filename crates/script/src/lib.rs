//! Dialogue & cutscene command interpreter (docs/03-ARCHITECTURE.md §5).
//!
//! A tiny deterministic interpreter, not a scripting language:
//! `(state, cmd) -> (state, [SideEffectReq])`, pure and unit-testable.
//! Implementation arrives in P2 (docs/06-ROADMAP.md); P0 only reserves the
//! crate boundary.

#![forbid(unsafe_code)]
