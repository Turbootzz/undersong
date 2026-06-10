//! Save format, versioning, and migrations (docs/03-ARCHITECTURE.md §4).
//!
//! Implementation arrives in P2 (docs/06-ROADMAP.md); P0 only reserves the
//! crate boundary. The `SaveBackend` trait (file system now, LocalStorage on
//! web at P7) starts here from day one.

#![forbid(unsafe_code)]
