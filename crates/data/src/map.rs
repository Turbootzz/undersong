//! Runtime map schema (docs/03-ARCHITECTURE.md §3, doc 04 §2).
//!
//! Maps are compiled RON: hand-written for the debug map, produced by
//! `tools importmap` from LDtk for authored maps. The engine never parses
//! editor formats at runtime.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};
use undersong_core::ids::{MapId, SpeciesId};
use undersong_core::world::Facing;

use crate::content::LoadError;

/// One compiled map. Layers are row-major, `width × height`, index
/// `y * width + x`, y growing upward (world space); tile id 0 = empty.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MapDef {
    pub id: MapId,
    pub name_key: String,
    pub width: u32,
    pub height: u32,
    pub ground: Vec<u16>,
    #[serde(default)]
    pub decor: Vec<u16>,
    #[serde(default)]
    pub overhang: Vec<u16>,
    /// 0 = walkable, 1 = solid.
    pub collision: Vec<u8>,
    /// 1 = resonance patch (per-step encounter roll, doc 02 §12).
    #[serde(default)]
    pub patches: Vec<u8>,
    #[serde(default)]
    pub triggers: Vec<Trigger>,
    #[serde(default)]
    pub npcs: Vec<NpcDef>,
    #[serde(default)]
    pub encounters: Option<EncounterDef>,
    #[serde(default)]
    pub music: Option<String>,
    /// Ambient weather zone (doc 02 v1.6 #6).
    #[serde(default)]
    pub weather: Option<undersong_core::moves::WeatherKind>,
    /// Night encounter table (v1.6 #5); same 12-weight law.
    #[serde(default)]
    pub night_encounters: Option<EncounterDef>,
    /// Performance obstacles (doc 02 §11).
    #[serde(default)]
    pub obstacles: Vec<Obstacle>,
    /// Dark cave: unlit without Lumen Hum (doc 02 §11).
    #[serde(default)]
    pub dark: bool,
    /// Interior map (halls, labs, towers): exempt from the outdoor
    /// door-direction rule (you enter buildings at their bottom).
    #[serde(default)]
    pub indoor: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Trigger {
    pub at: (u32, u32),
    pub kind: TriggerKind,
    #[serde(default)]
    pub once_flag: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TriggerKind {
    Warp {
        map: MapId,
        to: (u32, u32),
        facing: Facing,
    },
    /// Path relative to the map's `scripts/` directory.
    Script { path: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NpcBehavior {
    Static,
    /// Random 1-tile strolls within `radius` of the spawn tile.
    Wander {
        radius: u8,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NpcDef {
    pub id: String,
    pub at: (u32, u32),
    pub facing: Facing,
    /// Sprite key; the P2 debug renderer maps it to a color.
    pub sprite: String,
    /// Interaction script path (relative `scripts/`), if any.
    #[serde(default)]
    pub script: Option<String>,
    #[serde(default = "default_behavior")]
    pub behavior: NpcBehavior,
    /// Trainer line-of-sight range in tiles; 0 = no engagement
    /// (doc 02 §12). Engagement itself is flag-gated by the game layer.
    #[serde(default)]
    pub sight_range: u8,
}

fn default_behavior() -> NpcBehavior {
    NpcBehavior::Static
}

/// Overworld obstacles cleared by Performances (doc 02 §11).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ObstacleKind {
    /// Clearing Chord (Badge 2).
    Brush,
    /// Tunneling Bass (Badge 3).
    CrackedRock,
    /// Lift Motif (Badge 5): pushable one tile along the facing.
    Boulder,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Obstacle {
    pub at: (u32, u32),
    pub kind: ObstacleKind,
}

/// Per-map wild encounter config (doc 02 §12: 12 slots, weights sum 100).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EncounterDef {
    /// Per-step roll chance in percent (doc 02 §12 default 12).
    pub patch_rate_pct: u8,
    /// `(species, level_min, level_max, weight_pct)` — exactly 12 slots.
    pub slots: Vec<(SpeciesId, u8, u8, u8)>,
}

impl MapDef {
    pub fn in_bounds(&self, x: u32, y: u32) -> bool {
        x < self.width && y < self.height
    }

    pub fn index(&self, x: u32, y: u32) -> usize {
        usize::try_from(y * self.width + x).expect("map fits usize")
    }

    pub fn is_solid(&self, x: u32, y: u32) -> bool {
        !self.in_bounds(x, y) || self.collision[self.index(x, y)] != 0
    }

    /// Ground tile id at a tile (id 5 = water by convention, doc 02 §11
    /// Ferry Song).
    pub fn ground_at(&self, x: u32, y: u32) -> Option<u16> {
        if !self.in_bounds(x, y) {
            return None;
        }
        self.ground.get(self.index(x, y)).copied()
    }

    pub fn is_patch(&self, x: u32, y: u32) -> bool {
        self.in_bounds(x, y) && self.patches.get(self.index(x, y)).is_some_and(|&p| p != 0)
    }

    pub fn trigger_at(&self, x: u32, y: u32) -> Option<&Trigger> {
        self.triggers.iter().find(|t| t.at == (x, y))
    }

    pub fn npc_at(&self, x: u32, y: u32) -> Option<&NpcDef> {
        self.npcs.iter().find(|n| n.at == (x, y))
    }
}

/// Loads every `*/map.ron` under a maps directory, keyed by map id.
pub fn load_maps(maps_root: &Path) -> Result<BTreeMap<MapId, MapDef>, LoadError> {
    let mut maps = BTreeMap::new();
    let entries = std::fs::read_dir(maps_root).map_err(|source| LoadError::Io {
        path: maps_root.to_path_buf(),
        source,
    })?;
    let mut dirs: Vec<_> = entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    dirs.sort(); // deterministic load order
    for dir in dirs {
        let path = dir.join("map.ron");
        if path.exists() {
            let map: MapDef = crate::content::load_ron_file(&path)?;
            maps.insert(map.id.clone(), map);
        }
    }
    Ok(maps)
}
