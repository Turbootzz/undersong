//! Content schemas, RON loaders, and validation
//! (docs/03-ARCHITECTURE.md §1, docs/04-CONTENT-PIPELINE.md).
//!
//! P0 scope: `content/core/typechart.ron` + `natures.ron` and the first
//! validation rules. Region packs, species, moves, and the rest of the
//! doc 04 §3 rule list arrive with later phases.

#![forbid(unsafe_code)]

pub mod content;
pub mod map;
pub mod region;
pub mod validate;

pub use content::{
    CoreContent, LoadError, Natures, Palette, SpeciesPool, TypeChart, load_core, load_core_strings,
    load_palette, load_species_pool,
};
pub use map::{
    EncounterDef, MapDef, NpcBehavior, NpcDef, Obstacle, ObstacleKind, Trigger, TriggerKind,
    load_maps,
};
pub use region::{
    DexInfo, Evolution, EvolutionMethod, ItemDef, ItemKind, ItemSet, Motif, Pocket, RegionDef,
    RegionPack, Trainer, TrainerMote, load_items, load_region,
};
pub use undersong_core::collections::UniqueMap;
pub use validate::{
    Finding, Severity, validate_core, validate_item_moves, validate_items, validate_maps,
    validate_palette, validate_region, validate_species_pool, validate_strings,
};
