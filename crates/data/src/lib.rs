//! Content schemas, RON loaders, and validation
//! (docs/03-ARCHITECTURE.md §1, docs/04-CONTENT-PIPELINE.md).
//!
//! P0 scope: `content/core/typechart.ron` + `natures.ron` and the first
//! validation rules. Region packs, species, moves, and the rest of the
//! doc 04 §3 rule list arrive with later phases.

#![forbid(unsafe_code)]

pub mod content;
pub mod unique_map;
pub mod validate;

pub use content::{CoreContent, LoadError, Natures, TypeChart, load_core};
pub use unique_map::UniqueMap;
pub use validate::{Finding, Severity, validate_core};
