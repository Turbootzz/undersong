//! Region pack schemas (docs/04-CONTENT-PIPELINE.md §1–§2).
//!
//! `content/regions/<id>/` holds the album: region.ron, motifs/, moves/,
//! trainers/, maps/, dialogue/. A region may add, never modify, core
//! content.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};
use undersong_core::collections::UniqueMap;
use undersong_core::ids::{ItemId, MapId, MoveId, SpeciesId, TrainerId};
use undersong_core::moves::{Frac, MoveSpec};
use undersong_core::species::SpeciesSpec;

use crate::content::{LoadError, load_ron_file};

/// Evolution methods at launch (doc 02 §9).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EvolutionMethod {
    Level(u8),
    Item(ItemId),
    /// Friendship ≥ 220.
    Friendship,
    /// Trade-flagged → Duet Stone substitutes (doc 02 §9).
    DuetStone,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Evolution {
    pub method: EvolutionMethod,
    pub target: SpeciesId,
}

/// Dex cosmetics (doc 02 §2). Heights/weights in tenths (integer law:
/// 0.9 m → 9, 19.5 kg → 195).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DexInfo {
    pub height_dm: u16,
    pub weight_hg: u16,
    pub entry_key: String,
}

/// Full content species: the battle-facing fields plus content-only ones
/// (doc 04 §2's motif file shape). Fields mirror `SpeciesSpec` explicitly
/// — serde(flatten) is incompatible with RON's named-struct syntax.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Motif {
    pub id: SpeciesId,
    pub name_key: String,
    pub types: Vec<undersong_core::types::Type>,
    pub base_stats: undersong_core::species::StatSpread,
    pub catch_rate: u8,
    pub base_exp_yield: u16,
    pub ev_yield: undersong_core::collections::UniqueMap<undersong_core::stats::Stat, u8>,
    pub growth_curve: undersong_core::species::GrowthCurve,
    pub learnset: Vec<(u8, MoveId)>,
    #[serde(default)]
    pub abilities: Vec<undersong_core::ids::AbilityId>,
    #[serde(default)]
    pub hidden_ability: Option<undersong_core::ids::AbilityId>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub tm_set: Vec<MoveId>,
    #[serde(default)]
    pub evolution: Option<Evolution>,
    pub cry_seed: u64,
    pub sigil_seed: u64,
    pub dex: DexInfo,
}

impl Motif {
    /// The battle-facing subset (what the sim and tools consume).
    pub fn spec(&self) -> SpeciesSpec {
        SpeciesSpec {
            id: self.id.clone(),
            name_key: self.name_key.clone(),
            types: self.types.clone(),
            base_stats: self.base_stats,
            catch_rate: self.catch_rate,
            base_exp_yield: self.base_exp_yield,
            ev_yield: self.ev_yield.clone(),
            growth_curve: self.growth_curve,
            learnset: self.learnset.clone(),
            abilities: self.abilities.clone(),
            hidden_ability: self.hidden_ability.clone(),
            tags: self.tags.clone(),
        }
    }
}

/// A trainer's party member (doc 04 §2 trainer shape).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrainerMote {
    pub species: SpeciesId,
    pub level: u8,
    #[serde(default)]
    pub moves: Option<Vec<MoveId>>,
    #[serde(default)]
    pub ivs: Option<undersong_core::species::StatSpread>,
    #[serde(default)]
    pub held_item: Option<ItemId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Trainer {
    pub id: TrainerId,
    pub class: String,
    pub name_key: String,
    /// 0–3 (doc 02 §14).
    pub ai_tier: u8,
    pub payout_base: u32,
    #[serde(default)]
    pub double_battle: bool,
    pub party: Vec<TrainerMote>,
    pub defeat_flag: String,
    #[serde(default)]
    pub reward_items: Vec<(ItemId, u32)>,
    pub intro_key: String,
    pub defeat_key: String,
}

/// Item kinds (doc 02 §8 bells, §15 economy, v1.6 item pass).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ItemKind {
    /// Restores `hp` HP.
    Potion {
        hp: u16,
    },
    /// Cures any major status.
    StatusHeal,
    /// Attunement bell with its catch multiplier.
    Bell {
        catch_mod: Frac,
    },
    /// Conditional bells (doc 02 v1.6 #4).
    OvertureBell,
    CradleBell,
    VesperBell,
    /// Always succeeds; one per save (post-game).
    Coda,
    /// Reusable TM teaching `move_id` (v1.6 #2).
    Tm {
        move_id: MoveId,
    },
    /// +10 EVs in `stat` (v1.6 #1).
    Vitamin {
        stat: undersong_core::stats::Stat,
    },
    /// 200 steps of wild silence (v1.6 #3).
    MuteCharm,
    /// Held in battle (oran_chime, exp_share — battle crate hooks).
    Held,
    /// Story/progression item; not usable from the bag.
    Key,
}

/// Bag pockets (doc 06 P4: "Bag pockets final").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Pocket {
    #[default]
    Items,
    Bells,
    Tms,
    Key,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ItemDef {
    pub id: ItemId,
    pub name_key: String,
    pub kind: ItemKind,
    /// Mart price in ₵; 0 = not sold.
    pub price: u32,
    #[serde(default)]
    pub pocket: Pocket,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ItemSet {
    pub items: Vec<ItemDef>,
}

/// region.ron (doc 04 §1): dex list, starters, entry map.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegionDef {
    pub id: String,
    pub name_key: String,
    pub dex: Vec<SpeciesId>,
    /// Exactly three (validated) — RON authors a plain list.
    pub starters: Vec<SpeciesId>,
    pub entry_map: MapId,
    pub entry_spawn: (u32, u32),
}

/// A loaded region pack.
#[derive(Debug, Clone)]
pub struct RegionPack {
    pub def: RegionDef,
    pub motifs: BTreeMap<SpeciesId, Motif>,
    /// Region-new moves (core moves live in content/core).
    pub moves: Vec<MoveSpec>,
    pub trainers: BTreeMap<TrainerId, Trainer>,
    pub maps: BTreeMap<MapId, crate::map::MapDef>,
    /// Keyed dialogue strings (en).
    pub strings: UniqueMap<String, String>,
}

/// Loads `content/regions/<region>/`.
pub fn load_region(content_root: &Path, region: &str) -> Result<RegionPack, LoadError> {
    let root = content_root.join("regions").join(region);
    let def: RegionDef = load_ron_file(&root.join("region.ron"))?;

    let mut motifs = BTreeMap::new();
    let motif_dir = root.join("motifs");
    if motif_dir.exists() {
        let mut paths: Vec<_> = std::fs::read_dir(&motif_dir)
            .map_err(|source| LoadError::Io {
                path: motif_dir.clone(),
                source,
            })?
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| e == "ron"))
            .collect();
        paths.sort();
        for path in paths {
            let motif: Motif = load_ron_file(&path)?;
            motifs.insert(motif.id.clone(), motif);
        }
    }

    let moves_path = root.join("moves/moves.ron");
    let moves = if moves_path.exists() {
        let set: crate::content::MoveSet = load_ron_file(&moves_path)?;
        set.moves
    } else {
        Vec::new()
    };

    let mut trainers = BTreeMap::new();
    let trainer_dir = root.join("trainers");
    if trainer_dir.exists() {
        let mut paths: Vec<_> = std::fs::read_dir(&trainer_dir)
            .map_err(|source| LoadError::Io {
                path: trainer_dir.clone(),
                source,
            })?
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| e == "ron"))
            .collect();
        paths.sort();
        for path in paths {
            let trainer: Trainer = load_ron_file(&path)?;
            trainers.insert(trainer.id.clone(), trainer);
        }
    }

    let maps_root = root.join("maps");
    let maps = if maps_root.exists() {
        crate::map::load_maps(&maps_root)?
    } else {
        BTreeMap::new()
    };

    let strings_path = root.join("dialogue/strings.ron");
    let strings: UniqueMap<String, String> = if strings_path.exists() {
        load_ron_file(&strings_path)?
    } else {
        BTreeMap::new().into()
    };

    Ok(RegionPack {
        def,
        motifs,
        moves,
        trainers,
        maps,
        strings,
    })
}

/// Loads `content/core/items.ron`.
pub fn load_items(content_root: &Path) -> Result<ItemSet, LoadError> {
    load_ron_file(&content_root.join("core/items.ron"))
}
