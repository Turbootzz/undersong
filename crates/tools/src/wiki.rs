//! `tools wiki` — exports the companion wiki's data straight from
//! `content/` (P16). The wiki never hand-copies game data: this export
//! is the only bridge, so the RON files stay the single source of
//! truth. Spoiler classification ships in-band; the site hides flagged
//! entries behind its toggle.

use std::path::Path;

use anyhow::{Context, Result};
use serde::Serialize;

#[derive(Serialize)]
struct WikiSpecies {
    id: String,
    name: String,
    region: String,
    types: Vec<String>,
    base_stats: [u16; 6],
    catch_rate: u8,
    growth: String,
    abilities: Vec<String>,
    hidden_ability: Option<String>,
    learnset: Vec<(u8, String)>,
    evolves_to: Option<(String, String)>,
    entry: String,
    /// Legendaries and story-locked species hide behind the toggle.
    spoiler: bool,
}

#[derive(Serialize)]
struct WikiMove {
    id: String,
    name: String,
    r#type: String,
    category: String,
    power: u16,
    accuracy: u8,
    pp: u8,
    sound: bool,
}

#[derive(Serialize)]
struct WikiLocation {
    map: String,
    name: String,
    region: String,
    slots: Vec<(String, u8, u8, u8)>,
    night_slots: Vec<(String, u8, u8, u8)>,
    /// Late-game maps (acts 2–3, the Spire, the Vault) are spoilers.
    spoiler: bool,
}

#[derive(Serialize)]
struct WikiChart {
    types: Vec<String>,
    /// multipliers[attacker][defender] in tenths (20 = 2.0×).
    multipliers: Vec<Vec<u8>>,
}

const SPOILER_MAPS: &[&str] = &[
    "route_5",
    "voltaccia",
    "route_5b",
    "hollowfen",
    "graven_pass",
    "frostine",
    "route_6",
    "cadenza_city",
    "quartet_spire",
    "the_vault",
];

pub fn export(content_root: &Path, out: &Path) -> Result<usize> {
    std::fs::create_dir_all(out).with_context(|| format!("creating {}", out.display()))?;
    let core = data::load_core(content_root)?;
    let core_strings = data::load_core_strings(content_root)?;

    let mut species = Vec::new();
    let mut locations = Vec::new();
    let mut moves: std::collections::BTreeMap<String, WikiMove> = Default::default();
    let mut regions = Vec::new();

    let regions_root = content_root.join("regions");
    let mut dirs: Vec<_> = std::fs::read_dir(&regions_root)?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    dirs.sort();

    for dir in dirs {
        let region = dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
            .to_string();
        let pack = data::load_region(content_root, &region)?;
        regions.push(region.clone());
        let text = |key: &str| -> String {
            pack.strings
                .get(key)
                .or_else(|| core_strings.get(key))
                .cloned()
                .unwrap_or_else(|| key.to_string())
        };

        for motif in pack.motifs.values() {
            let spoiler = motif.catch_rate <= 5;
            species.push(WikiSpecies {
                id: motif.id.to_string(),
                name: text(&format!("motif.{}", motif.id)),
                region: region.clone(),
                types: motif.types.iter().map(|t| format!("{t:?}")).collect(),
                base_stats: [
                    motif.base_stats.hp,
                    motif.base_stats.atk,
                    motif.base_stats.def,
                    motif.base_stats.spa,
                    motif.base_stats.spd,
                    motif.base_stats.spe,
                ],
                catch_rate: motif.catch_rate,
                growth: format!("{:?}", motif.growth_curve),
                abilities: motif.abilities.iter().map(ToString::to_string).collect(),
                hidden_ability: motif.hidden_ability.as_ref().map(ToString::to_string),
                learnset: motif
                    .learnset
                    .iter()
                    .map(|(level, id)| (*level, id.to_string()))
                    .collect(),
                evolves_to: motif
                    .evolution
                    .as_ref()
                    .map(|e| (e.target.to_string(), format!("{:?}", e.method))),
                entry: text(&format!("dex.{}", motif.id)),
                spoiler,
            });
        }

        for spec in core.moves.moves.iter().chain(pack.moves.iter()) {
            moves
                .entry(spec.id.to_string())
                .or_insert_with(|| WikiMove {
                    id: spec.id.to_string(),
                    name: text(&format!("move.{}", spec.id)),
                    r#type: format!("{:?}", spec.r#type),
                    category: format!("{:?}", spec.category),
                    power: spec.power,
                    accuracy: spec.accuracy,
                    pp: spec.pp,
                    sound: spec.flags.sound,
                });
        }

        for (map_id, map) in &pack.maps {
            let slots = |table: &Option<data::EncounterDef>| -> Vec<(String, u8, u8, u8)> {
                table
                    .as_ref()
                    .map(|t| {
                        t.slots
                            .iter()
                            .map(|(s, lo, hi, w)| (s.to_string(), *lo, *hi, *w))
                            .collect()
                    })
                    .unwrap_or_default()
            };
            let day = slots(&map.encounters);
            let night = slots(&map.night_encounters);
            if day.is_empty() && night.is_empty() {
                continue;
            }
            locations.push(WikiLocation {
                map: map_id.to_string(),
                name: text(&format!("map.{map_id}")),
                region: region.clone(),
                slots: day,
                night_slots: night,
                spoiler: SPOILER_MAPS.contains(&map_id.as_str()),
            });
        }
    }

    // The type chart as a full matrix.
    let all_types = [
        "Feral", "Ember", "Tide", "Bloom", "Gale", "Stone", "Frost", "Volt", "Venom", "Phantom",
        "Alloy", "Resonant",
    ];
    use undersong_core::types::Type;
    let parse = |name: &str| -> Type {
        match name {
            "Ember" => Type::Ember,
            "Tide" => Type::Tide,
            "Bloom" => Type::Bloom,
            "Gale" => Type::Gale,
            "Stone" => Type::Stone,
            "Frost" => Type::Frost,
            "Volt" => Type::Volt,
            "Venom" => Type::Venom,
            "Phantom" => Type::Phantom,
            "Alloy" => Type::Alloy,
            "Resonant" => Type::Resonant,
            _ => Type::Feral,
        }
    };
    let chart = WikiChart {
        types: all_types.iter().map(ToString::to_string).collect(),
        multipliers: all_types
            .iter()
            .map(|atk| {
                all_types
                    .iter()
                    .map(|def| {
                        let (num, den) = core.typechart.product(parse(atk), &[parse(def)]);
                        u8::try_from((num * 10 / den.max(1)).min(40)).unwrap_or(10)
                    })
                    .collect()
            })
            .collect(),
    };

    macro_rules! write_json {
        ($name:literal, $value:expr) => {{
            let file = std::fs::File::create(out.join($name))
                .with_context(|| format!("creating {}", $name))?;
            serde_json::to_writer_pretty(file, $value)
                .with_context(|| format!("writing {}", $name))?;
        }};
    }
    write_json!("species.json", &species);
    write_json!("moves.json", &moves.values().collect::<Vec<_>>());
    write_json!("locations.json", &locations);
    write_json!("typechart.json", &chart);
    write_json!("regions.json", &regions);
    Ok(species.len())
}
