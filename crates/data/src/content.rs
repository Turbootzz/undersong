//! Schemas and RON loaders for `content/core`.
//!
//! Schema strictness conventions (all future content schemas follow them):
//! `deny_unknown_fields` on every struct, and [`UniqueMap`] instead of bare
//! `BTreeMap` so typos and duplicate keys are parse errors, not silent drift.

use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use undersong_core::collections::UniqueMap;
use undersong_core::moves::MoveSpec;
use undersong_core::types::{Eff, Type};

/// The 12×12 effectiveness chart, attacker → defender → multiplier.
///
/// The RON file is fully explicit — every attacker lists every defender —
/// so the validator proves totality (doc 04 §3 rule 6) instead of silently
/// defaulting missing pairs to neutral.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TypeChart {
    pub entries: UniqueMap<Type, UniqueMap<Type, Eff>>,
}

impl TypeChart {
    /// Effectiveness of `attacker` against `defender`, if the pair is present.
    pub fn eff(&self, attacker: Type, defender: Type) -> Option<Eff> {
        self.entries.get(&attacker)?.get(&defender).copied()
    }
}

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
