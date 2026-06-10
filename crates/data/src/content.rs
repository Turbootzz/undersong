//! Schemas and RON loaders for `content/core`.
//!
//! Schema strictness conventions (all future content schemas follow them):
//! `deny_unknown_fields` on every struct, and [`UniqueMap`] instead of bare
//! `BTreeMap` so typos and duplicate keys are parse errors, not silent drift.

use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use undersong_core::moves::MoveSpec;

/// Re-exported from core: the chart shape is shared vocabulary between
/// content loading (here) and the battle sim.
pub use undersong_core::chart::TypeChart;

/// The 25 temperaments (natures), in canonical index order `n ∈ 0..25`:
/// boosted stat = `n / 5`, hindered = `n % 5`, over `[atk, def, spa, spd, spe]`;
/// the diagonal (`n / 5 == n % 5`) is neutral (doc 02 §3). Data carries only
/// the UI name keys; the index math is law and lives with the stat formulas.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Natures {
    pub name_keys: Vec<String>,
}

/// Number of natures required by the 5×5 grid in doc 02 §3.
pub const NATURE_COUNT: usize = 25;

/// The core move set (`content/core/moves.ron`, doc 04 §1: core moves live
/// in content/core; region packs may add, never modify).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MoveSet {
    pub moves: Vec<MoveSpec>,
}

impl MoveSet {
    pub fn get(&self, id: &undersong_core::ids::MoveId) -> Option<&MoveSpec> {
        self.moves.iter().find(|m| &m.id == id)
    }
}

/// Everything loaded from `content/core` (cross-region law, doc 04 §1).
#[derive(Debug, Clone)]
pub struct CoreContent {
    pub typechart: TypeChart,
    pub natures: Natures,
    pub moves: MoveSet,
}

/// A standalone species pool file (the P1 dev testbed; region dexes in
/// P3+ use the full region.ron flow instead).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpeciesPool {
    pub species: Vec<undersong_core::species::SpeciesSpec>,
}

/// Loads a species pool RON file.
pub fn load_species_pool(path: &Path) -> Result<SpeciesPool, LoadError> {
    load_ron(path.to_path_buf())
}

#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    // Display omits {source}: callers print the chain (anyhow `{:#}`),
    // and including it here would double every cause.
    #[error("failed to read {path}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to parse {path}")]
    Parse {
        path: PathBuf,
        // Boxed: SpannedError is large and would bloat every Result
        // (clippy::result_large_err).
        #[source]
        source: Box<ron::error::SpannedError>,
    },
}

/// Loads `content/core` from a content root (the directory holding `core/`
/// and `regions/`).
pub fn load_core(content_root: &Path) -> Result<CoreContent, LoadError> {
    let typechart = load_ron(content_root.join("core/typechart.ron"))?;
    let natures = load_ron(content_root.join("core/natures.ron"))?;
    let moves = load_ron(content_root.join("core/moves.ron"))?;
    Ok(CoreContent {
        typechart,
        natures,
        moves,
    })
}

/// Crate-internal RON file loader shared by the schema modules.
pub(crate) fn load_ron_file<T: DeserializeOwned>(path: &Path) -> Result<T, LoadError> {
    load_ron(path.to_path_buf())
}

fn load_ron<T: DeserializeOwned>(path: PathBuf) -> Result<T, LoadError> {
    let text = std::fs::read_to_string(&path).map_err(|source| LoadError::Io {
        path: path.clone(),
        source,
    })?;
    ron::from_str(&text).map_err(|source| LoadError::Parse {
        path,
        source: Box::new(source),
    })
}

/// The ink-and-parchment palette (doc 05 §2); colors as `#rrggbb`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Palette {
    pub ink: String,
    pub ink_soft: String,
    pub parchment: String,
    pub parchment_dim: String,
    pub gilt: String,
    pub cantorel_accent: String,
    pub hp_high: String,
    pub hp_mid: String,
    pub hp_low: String,
    /// Night multiply color (doc 05 §2).
    pub night: String,
    /// One color per type; totality validated.
    pub type_colors: undersong_core::collections::UniqueMap<undersong_core::types::Type, String>,
}

/// Loads `content/core/palette.ron`.
/// Loads `content/core/strings.ron` (canon move/item/UI strings).
pub fn load_core_strings(
    content_root: &Path,
) -> Result<undersong_core::collections::UniqueMap<String, String>, LoadError> {
    load_ron_file(&content_root.join("core/strings.ron"))
}

pub fn load_palette(content_root: &Path) -> Result<Palette, LoadError> {
    load_ron(content_root.join("core/palette.ron"))
}
