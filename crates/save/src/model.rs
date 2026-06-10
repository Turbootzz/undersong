//! SaveFile v1 (doc 03 §4), field for field, plus versioned migration.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use undersong_core::ids::{ItemId, MapId};
use undersong_core::individual::Individual;

/// Current save format version. Every breaking change bumps this and
/// adds a migration; old fixtures must load forever (doc 03 §4).
pub const SAVE_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SlotId {
    Slot1,
    Slot2,
    Slot3,
    Auto,
}

#[derive(Debug, thiserror::Error)]
pub enum SaveError {
    #[error("save io: {0}")]
    Io(String),
    #[error("save encode: {0}")]
    Encode(String),
    #[error("save decode: {0}")]
    Decode(String),
    #[error("save version {0} is newer than this build supports")]
    UnsupportedVersion(u32),
}

/// Re-exported from core: facing is shared overworld vocabulary.
pub use undersong_core::world::Facing;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Position {
    pub map: MapId,
    pub x: u32,
    pub y: u32,
    pub facing: Facing,
}

/// Player-facing options (doc 05 §5 Settings screen; persisted per save).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Characters per tick: 30 / 60 / 0 = instant (doc 05 §3).
    pub text_speed: u8,
    pub battle_animations: bool,
    /// Set = no switch prompt on foe faint (doc 02 era convention).
    pub set_mode: bool,
    pub volume_music: u8,
    pub volume_sfx: u8,
    pub screen_scale: u8,
    pub reduced_motion: bool,
    pub high_contrast: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            text_speed: 60,
            battle_animations: true,
            set_mode: false,
            volume_music: 80,
            volume_sfx: 80,
            screen_scale: 2,
            reduced_motion: false,
            high_contrast: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SaveHeader {
    pub version: u32,
    /// Unix seconds at creation (set by the game layer — the save crate
    /// itself never reads a clock).
    pub created_epoch_s: u64,
    pub playtime_s: u64,
    pub region: String,
    /// Bit n = badge n+1 earned.
    pub badge_bits: u8,
    /// Score (dex) completion percent 0–100.
    pub score_pct: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Player {
    pub name: String,
    pub money: u32,
    pub position: Position,
    pub settings: Settings,
}

/// SaveFile v1 (doc 03 §4). RON, human-readable during development.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SaveFile {
    pub header: SaveHeader,
    pub player: Player,
    /// ≤ 6.
    pub party: Vec<Individual>,
    /// Boxes of Motes ("Repertoire"); 16×30 at P4, growable until then.
    pub boxes: Vec<Vec<Individual>>,
    /// pocket → [(item, qty)].
    pub bag: BTreeMap<String, Vec<(ItemId, u32)>>,
    /// Namespaced flags (doc 01 §9).
    pub flags: BTreeSet<String>,
    pub vars: BTreeMap<String, i32>,
    pub counters: BTreeMap<String, u64>,
    pub world_seed: u64,
}

/// Parses any supported save text into the current version.
pub fn migrate(text: &str) -> Result<SaveFile, SaveError> {
    // Peek the version first so future formats fail with a real message
    // rather than a field-mismatch parse error.
    #[derive(Deserialize)]
    struct VersionPeek {
        header: HeaderPeek,
    }
    #[derive(Deserialize)]
    struct HeaderPeek {
        version: u32,
    }
    let peek: VersionPeek = ron::from_str(text).map_err(|e| SaveError::Decode(e.to_string()))?;
    match peek.header.version {
        SAVE_VERSION => ron::from_str(text).map_err(|e| SaveError::Decode(e.to_string())),
        // Future migrations chain here: 1 → 2 → … keeping every fixture
        // loading (doc 03 §4).
        v if v > SAVE_VERSION => Err(SaveError::UnsupportedVersion(v)),
        v => Err(SaveError::Decode(format!("unknown save version {v}"))),
    }
}
