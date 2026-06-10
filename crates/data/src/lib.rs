//! Content schemas, RON loaders, and validation
//! (docs/03-ARCHITECTURE.md §1, docs/04-CONTENT-PIPELINE.md).
//!
//! P0 scope: `content/core/typechart.ron` + `natures.ron` and the first
//! validation rules. Region packs, species, moves, and the rest of the
//! doc 04 §3 rule list arrive with later phases.

#![forbid(unsafe_code)]

pub mod content;
pub mod map;
pub mod validate;

pub use content::{
    CoreContent, LoadError, Natures, Palette, SpeciesPool, TypeChart, load_core, load_palette,
    load_species_pool,
};
pub use map::{EncounterDef, MapDef, NpcBehavior, NpcDef, Trigger, TriggerKind, load_maps};
pub use undersong_core::collections::UniqueMap;
pub use validate::{
    Finding, Severity, validate_core, validate_maps, validate_palette, validate_species_pool,
};
