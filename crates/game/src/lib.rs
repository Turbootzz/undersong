//! Undersong client library: the pure overworld core (`world`) and the
//! replay driver. The Bevy app lives in `main.rs`; integration tests and
//! the `--replay` flag drive `world` directly (doc 03 §3 headless lane).

#![forbid(unsafe_code)]

pub mod art;
pub mod replay;
pub mod session;
pub mod world;
