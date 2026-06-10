//! `tools importmap` — compiles an LDtk project export into runtime
//! `map.ron` files (doc 03 §3: the engine never parses editor formats).
//!
//! Conventions the LDtk project must follow (documented for map authors):
//! - One level per map; the level identifier (lowercased) is the map id.
//! - IntGrid layers named `ground`, `decor`, `overhang`, `collision`,
//!   `patches`; cell values map 1:1 to runtime tile ids.
//! - An entity layer `markers` with entity types:
//!   `Warp` (fields: map String, to_x Int, to_y Int, facing String
//!   Up/Down/Left/Right), `Script` (fields: path String), and `Npc`
//!   (fields: sprite String, script optional String, facing String,
//!   wander Int radius with 0 = static, optional sight Int default 0).
//! - LDtk's y axis points down; the importer flips rows so runtime maps
//!   keep y growing upward.
//!
//! Tiled is best-effort later (doc 06 P2 box picks one editor).

use std::path::Path;

use anyhow::{Context, Result, bail};
use data::map::{MapDef, NpcBehavior, NpcDef, Trigger, TriggerKind};
use serde_json::Value;
use undersong_core::world::Facing;

pub fn import(ldtk_path: &Path, out_root: &Path) -> Result<Vec<String>> {
    let text = std::fs::read_to_string(ldtk_path)
        .with_context(|| format!("reading {}", ldtk_path.display()))?;
    let project: Value =
        serde_json::from_str(&text).with_context(|| format!("parsing {}", ldtk_path.display()))?;
    let levels = project["levels"]
        .as_array()
        .context("LDtk project has no levels array")?;

    let mut written = Vec::new();
    for level in levels {
        let map = import_level(level)?;
        let dir = out_root.join(map.id.as_str());
        std::fs::create_dir_all(dir.join("scripts"))
            .with_context(|| format!("creating {}", dir.display()))?;
        let ron_text = ron::ser::to_string_pretty(&map, ron::ser::PrettyConfig::default())
            .context("serializing map")?;
        let out = dir.join("map.ron");
        std::fs::write(
            &out,
            format!("// compiled by `tools importmap` — do not hand-edit\n{ron_text}\n"),
        )
        .with_context(|| format!("writing {}", out.display()))?;
        written.push(map.id.to_string());
    }
    Ok(written)
}

fn import_level(level: &Value) -> Result<MapDef> {
    let identifier = level["identifier"]
        .as_str()
        .context("level missing identifier")?
        .to_lowercase();
    let layers = level["layerInstances"]
        .as_array()
        .context("level missing layerInstances (enable 'save levels separately' OFF)")?;

    // Dimensions come from the required `ground` layer only; any other
    // consumed layer must agree.
    let mut width = 0u32;
    let mut height = 0u32;
    for layer in layers {
        if layer["__identifier"].as_str() == Some("ground") {
            width = u32::try_from(layer["__cWid"].as_u64().unwrap_or(0)).unwrap_or(0);
            height = u32::try_from(layer["__cHei"].as_u64().unwrap_or(0)).unwrap_or(0);
        }
    }
    if width == 0 || height == 0 {
        bail!("level {identifier}: required `ground` IntGrid layer missing or empty");
    }
    let mut ground = Vec::new();
    let mut decor = Vec::new();
    let mut overhang = Vec::new();
    let mut collision = Vec::new();
    let mut patches = Vec::new();
    let mut triggers = Vec::new();
    let mut npcs = Vec::new();

    for layer in layers {
        let name = layer["__identifier"].as_str().unwrap_or_default();
        let cw = u32::try_from(layer["__cWid"].as_u64().unwrap_or(0)).unwrap_or(0);
        let ch = u32::try_from(layer["__cHei"].as_u64().unwrap_or(0)).unwrap_or(0);
        match layer["__type"].as_str().unwrap_or_default() {
            "IntGrid" => {
                if (cw, ch) != (width, height) {
                    bail!(
                        "level {identifier}: layer `{name}` is {cw}x{ch}, ground is {width}x{height}"
                    );
                }
                let csv: Vec<u16> = layer["intGridCsv"]
                    .as_array()
                    .context("IntGrid layer missing intGridCsv")?
                    .iter()
                    .map(|v| u16::try_from(v.as_u64().unwrap_or(0)).unwrap_or(0))
                    .collect();
                if csv.len() != (cw as usize) * (ch as usize) {
                    bail!(
                        "level {identifier}: layer `{name}` has {} cells, expected {}",
                        csv.len(),
                        cw * ch
                    );
                }
                let flipped = flip_rows(&csv, cw as usize, ch as usize);
                match name {
                    "ground" => ground = flipped,
                    "decor" => decor = flipped,
                    "overhang" => overhang = flipped,
                    "collision" => {
                        collision = flipped
                            .into_iter()
                            .map(|v| u8::try_from(v.min(1)).expect("0/1"))
                            .collect();
                    }
                    "patches" => {
                        patches = flipped
                            .into_iter()
                            .map(|v| u8::try_from(v.min(1)).expect("0/1"))
                            .collect();
                    }
                    other => bail!("unknown IntGrid layer `{other}` in level {identifier}"),
                }
            }
            "Entities" => {
                for entity in layer["entityInstances"].as_array().into_iter().flatten() {
                    let kind = entity["__identifier"].as_str().unwrap_or_default();
                    let grid = entity["__grid"].as_array().context("entity grid")?;
                    let gx = u32::try_from(grid[0].as_u64().unwrap_or(0)).unwrap_or(0);
                    let gy_down = u32::try_from(grid[1].as_u64().unwrap_or(0)).unwrap_or(0);
                    if gx >= width || gy_down >= height {
                        bail!(
                            "level {identifier}: entity `{}` at grid ({gx},{gy_down}) is out of bounds",
                            entity["iid"].as_str().unwrap_or(kind)
                        );
                    }
                    let gy = height - 1 - gy_down;
                    let fields = FieldBag::from(entity);
                    match kind {
                        "Warp" => triggers.push(Trigger {
                            at: (gx, gy),
                            kind: TriggerKind::Warp {
                                map: fields.string("map")?.into(),
                                to: (
                                    u32::try_from(fields.int("to_x")?).context("to_x")?,
                                    u32::try_from(fields.int("to_y")?).context("to_y")?,
                                ),
                                facing: parse_facing(&fields.string("facing")?)?,
                            },
                            once_flag: fields.opt_string("once_flag"),
                        }),
                        "Script" => triggers.push(Trigger {
                            at: (gx, gy),
                            kind: TriggerKind::Script {
                                path: fields.string("path")?,
                            },
                            once_flag: fields.opt_string("once_flag"),
                        }),
                        "Npc" => {
                            let radius = fields.int("wander").unwrap_or(0);
                            npcs.push(NpcDef {
                                id: entity["iid"].as_str().unwrap_or("npc").to_string(),
                                at: (gx, gy),
                                facing: parse_facing(&fields.string("facing")?)?,
                                sprite: fields.string("sprite")?,
                                script: fields.opt_string("script"),
                                behavior: if radius > 0 {
                                    NpcBehavior::Wander {
                                        radius: u8::try_from(radius).context("wander radius")?,
                                    }
                                } else {
                                    NpcBehavior::Static
                                },
                                sight_range: u8::try_from(fields.int("sight").unwrap_or(0))
                                    .context("sight")?,
                            });
                        }
                        other => bail!("unknown entity `{other}` in level {identifier}"),
                    }
                }
            }
            _ => {} // Tiles/AutoLayer art layers are cosmetic in LDtk; ignored
        }
    }

    if ground.is_empty() || collision.is_empty() {
        bail!("level {identifier} needs at least `ground` and `collision` IntGrid layers");
    }

    Ok(MapDef {
        id: identifier.as_str().into(),
        name_key: format!("map.{identifier}"),
        width,
        height,
        ground,
        decor,
        overhang,
        collision,
        patches,
        triggers,
        npcs,
        encounters: None, // encounter tables stay hand-authored RON (doc 04)
        music: None,
    })
}

/// LDtk rows run top-down; runtime rows run bottom-up.
fn flip_rows(cells: &[u16], width: usize, height: usize) -> Vec<u16> {
    let mut out = Vec::with_capacity(cells.len());
    for y in (0..height).rev() {
        out.extend_from_slice(&cells[y * width..(y + 1) * width]);
    }
    out
}

fn parse_facing(raw: &str) -> Result<Facing> {
    Ok(match raw {
        "Up" => Facing::Up,
        "Down" => Facing::Down,
        "Left" => Facing::Left,
        "Right" => Facing::Right,
        other => bail!("bad facing `{other}` (Up/Down/Left/Right)"),
    })
}

struct FieldBag<'a> {
    fields: Vec<&'a Value>,
}

impl<'a> FieldBag<'a> {
    fn from(entity: &'a Value) -> Self {
        Self {
            fields: entity["fieldInstances"]
                .as_array()
                .map(|a| a.iter().collect())
                .unwrap_or_default(),
        }
    }

    fn find(&self, name: &str) -> Option<&'a Value> {
        self.fields
            .iter()
            .find(|f| f["__identifier"].as_str() == Some(name))
            .map(|f| &f["__value"])
    }

    fn string(&self, name: &str) -> Result<String> {
        self.find(name)
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .with_context(|| format!("entity missing string field `{name}`"))
    }

    fn opt_string(&self, name: &str) -> Option<String> {
        self.find(name)
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(str::to_string)
    }

    fn int(&self, name: &str) -> Result<i64> {
        self.find(name)
            .and_then(serde_json::Value::as_i64)
            .with_context(|| format!("entity missing int field `{name}`"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A miniature LDtk project (schema-faithful subset): 4×3 level with
    /// ground/collision IntGrids and one Warp + one Npc entity.
    const FIXTURE: &str = r#"{
        "levels": [{
            "identifier": "Fixture_Yard",
            "layerInstances": [
                { "__identifier": "ground", "__type": "IntGrid",
                  "__cWid": 4, "__cHei": 3,
                  "intGridCsv": [1,1,1,1, 2,2,2,2, 3,3,3,3] },
                { "__identifier": "collision", "__type": "IntGrid",
                  "__cWid": 4, "__cHei": 3,
                  "intGridCsv": [0,0,0,0, 0,0,0,0, 1,1,1,1] },
                { "__identifier": "markers", "__type": "Entities",
                  "__cWid": 4, "__cHei": 3,
                  "entityInstances": [
                    { "__identifier": "Warp", "__grid": [1, 0], "iid": "w1",
                      "fieldInstances": [
                        { "__identifier": "map", "__value": "debug_annex" },
                        { "__identifier": "to_x", "__value": 3 },
                        { "__identifier": "to_y", "__value": 4 },
                        { "__identifier": "facing", "__value": "Down" }
                      ] },
                    { "__identifier": "Npc", "__grid": [2, 1], "iid": "npc-aa",
                      "fieldInstances": [
                        { "__identifier": "sprite", "__value": "npc.greeter" },
                        { "__identifier": "script", "__value": "hello.script.ron" },
                        { "__identifier": "facing", "__value": "Left" },
                        { "__identifier": "wander", "__value": 2 },
                        { "__identifier": "sight", "__value": 0 }
                      ] }
                  ] }
            ]
        }]
    }"#;

    #[test]
    fn fixture_level_compiles_to_mapdef() {
        let project: Value = serde_json::from_str(FIXTURE).expect("fixture json");
        let map = import_level(&project["levels"][0]).expect("import");
        assert_eq!(map.id.as_str(), "fixture_yard");
        assert_eq!((map.width, map.height), (4, 3));
        // Row flip: LDtk top row (all 1) becomes runtime y=2.
        assert_eq!(map.ground[map.index(0, 2)], 1);
        assert_eq!(map.ground[map.index(0, 0)], 3);
        // Collision: LDtk bottom row (all 1) is runtime y=0.
        assert!(map.is_solid(0, 0));
        assert!(!map.is_solid(0, 2));
        // Warp at LDtk (1,0) = runtime (1,2).
        let warp = map.trigger_at(1, 2).expect("warp imported");
        assert!(matches!(
            &warp.kind,
            TriggerKind::Warp { map, to: (3, 4), facing: Facing::Down }
                if map.as_str() == "debug_annex"
        ));
        // Npc at LDtk (2,1) = runtime (2,1) (middle row).
        let npc = map.npc_at(2, 1).expect("npc imported");
        assert_eq!(npc.sprite, "npc.greeter");
        assert_eq!(npc.script.as_deref(), Some("hello.script.ron"));
        assert!(matches!(npc.behavior, NpcBehavior::Wander { radius: 2 }));
    }

    #[test]
    fn missing_required_layers_fail_loudly() {
        let project: Value =
            serde_json::from_str(r#"{"levels":[{"identifier":"Empty","layerInstances":[]}]}"#)
                .expect("json");
        assert!(import_level(&project["levels"][0]).is_err());
    }
}
