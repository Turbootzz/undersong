//! Undersong CLI tools (docs/03-ARCHITECTURE.md §1).
//!
//! P1 ships `validate`, `simulate`, and `battle`. importmap/cries/sigils/
//! atlas arrive with the phases that need them.
//!
//! No CLI dependency: the closed dependency list (doc 03 §7) has no
//! argument parser, and a handful of flags doesn't justify one.

mod asset_tests;
mod cries;
mod effects;
mod heroes;
mod importmap;
mod melody;
mod music;
mod render;
mod sigils;
mod sim;
mod sprites;
mod stars;
mod wiki;

use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{Context, Result, bail};
use battle::ai::AiTier;

const USAGE: &str = "usage:
  tools validate  [--content <dir>]
  tools simulate  [--battles <n>] [--pool <file>] [--level <n>] [--seed <n>] [--content <dir>]
  tools battle    --seed <n> [--pool <file>] [--level <n>] [--content <dir>]
  tools importmap --in <project.ldtk> --out <maps dir>
  tools assets    --region <id> [--content <dir>] [--out <dir>]
  tools music     [--out <dir>]
  tools sprites   [--content <dir>] [--out <dir>]
  tools wiki      [--content <dir>] [--out <dir>]";

fn main() -> ExitCode {
    match run() {
        Ok(clean) => {
            if clean {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        }
        Err(err) => {
            eprintln!("error: {err:#}");
            ExitCode::FAILURE
        }
    }
}

#[derive(Debug)]
struct Options {
    content: PathBuf,
    pool: PathBuf,
    battles: u32,
    level: u8,
    seed: Option<u64>,
    region: Option<String>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            content: PathBuf::from("content"),
            pool: PathBuf::from("content/dev/testbed.ron"),
            battles: 1000,
            level: 30,
            seed: None,
            region: None,
        }
    }
}

fn parse_options(args: impl Iterator<Item = String>, allowed: &[&str]) -> Result<Options> {
    let mut options = Options::default();
    let mut args = args.peekable();
    while let Some(arg) = args.next() {
        if !allowed.contains(&arg.as_str()) {
            bail!("unknown argument `{arg}` for this command\n{USAGE}");
        }
        let mut value = |name: &str| {
            args.next()
                .with_context(|| format!("{name} needs a value\n{USAGE}"))
        };
        match arg.as_str() {
            "--content" => options.content = PathBuf::from(value("--content")?),
            "--pool" => options.pool = PathBuf::from(value("--pool")?),
            "--battles" => {
                let raw = value("--battles")?;
                options.battles = raw
                    .parse()
                    .with_context(|| format!("--battles expects a number, got `{raw}`"))?;
            }
            "--level" => {
                let raw = value("--level")?;
                options.level = raw
                    .parse()
                    .with_context(|| format!("--level expects a number, got `{raw}`"))?;
            }
            "--seed" => {
                let raw = value("--seed")?;
                options.seed = Some(
                    parse_seed(&raw)
                        .with_context(|| format!("--seed expects a number, got `{raw}`"))?,
                );
            }
            "--region" => options.region = Some(value("--region")?),
            other => bail!("unknown argument `{other}`\n{USAGE}"),
        }
    }
    Ok(options)
}

fn parse_seed(raw: &str) -> Result<u64> {
    if let Some(hex) = raw.strip_prefix("0x") {
        Ok(u64::from_str_radix(hex, 16)?)
    } else {
        Ok(raw.parse()?)
    }
}

/// Returns `Ok(true)` when the command ran and passed.
fn run() -> Result<bool> {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("validate") => {
            let options = parse_options(args, &["--content"])?;
            validate(&options)
        }
        Some("simulate") => {
            let options = parse_options(
                args,
                &[
                    "--battles",
                    "--pool",
                    "--level",
                    "--seed",
                    "--content",
                    "--region",
                ],
            )?;
            simulate(&options)
        }
        Some("battle") => {
            let options = parse_options(args, &["--seed", "--pool", "--level", "--content"])?;
            run_battle(&options)
        }
        Some("assets") => {
            let rest: Vec<String> = args.collect();
            let value = |flag: &str, default: &str| -> String {
                rest.iter()
                    .position(|a| a == flag)
                    .and_then(|i| rest.get(i + 1))
                    .cloned()
                    .unwrap_or_else(|| default.to_string())
            };
            let region = value("--region", "cantorel");
            let content = PathBuf::from(value("--content", "content"));
            let out = PathBuf::from(value("--out", "assets"));
            generate_assets(&content, &region, &out)
        }
        Some("wiki") => {
            let rest: Vec<String> = args.collect();
            let value = |flag: &str, default: &str| -> String {
                rest.iter()
                    .position(|a| a == flag)
                    .and_then(|i| rest.get(i + 1))
                    .cloned()
                    .unwrap_or_else(|| default.to_string())
            };
            let content = PathBuf::from(value("--content", "content"));
            let out = PathBuf::from(value("--out", "wiki/src/data"));
            let n = wiki::export(&content, &out)?;
            // sprites for the site (icons + fronts)
            let imgdir = PathBuf::from("wiki/public/sprites");
            std::fs::create_dir_all(&imgdir)?;
            let mut copied = 0u32;
            for region in ["cantorel", "skalden"] {
                let src = PathBuf::from("assets/sprites/monsters").join(region);
                let dst = imgdir.join(region);
                std::fs::create_dir_all(&dst)?;
                if let Ok(entries) = std::fs::read_dir(&src) {
                    for entry in entries.filter_map(Result::ok) {
                        let name = entry.file_name();
                        let n = name.to_string_lossy();
                        if n.ends_with(".front.png") || n.ends_with(".icon.png") {
                            std::fs::copy(entry.path(), dst.join(&name)).ok();
                            copied += 1;
                        }
                    }
                }
            }
            println!("wiki: {n} species exported, {copied} sprites copied");
            Ok(true)
        }
        Some("sprites") => {
            let rest: Vec<String> = args.collect();
            let value = |flag: &str, default: &str| -> String {
                rest.iter()
                    .position(|a| a == flag)
                    .and_then(|i| rest.get(i + 1))
                    .cloned()
                    .unwrap_or_else(|| default.to_string())
            };
            let content = PathBuf::from(value("--content", "content"));
            let out = PathBuf::from(value("--out", "assets"));
            let tiles = sprites::render_tiles(&out.join("sprites/tiles"))?;
            let chars = sprites::render_characters(&out.join("sprites/chars"))?;
            sprites::render_platform(&out.join("sprites/battle"))?;
            let fx = effects::render_effects(&out.join("sprites/fx"))?;
            heroes::render_heroes(&out.join("sprites/chars"))?;
            let mut creatures = 0;
            let regions_root = content.join("regions");
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
                creatures +=
                    sprites::render_creatures(&content, &region, &out.join("sprites/monsters"))?;
            }
            let stars = stars::render_stars(&out.join("sprites/monsters"))?;
            println!(
                "sprites: {tiles} tiles, {chars} character frames, {fx} fx frames, {creatures} creatures ({stars} hand-authored) → {}",
                out.display()
            );
            Ok(true)
        }
        Some("music") => {
            let rest: Vec<String> = args.collect();
            let out = rest
                .iter()
                .position(|a| a == "--out")
                .and_then(|i| rest.get(i + 1))
                .cloned()
                .unwrap_or_else(|| "assets".to_string());
            let out = PathBuf::from(out);
            let tracks = out.join("music");
            // (track, seed, mood, mode) — Quiet Coast intentionally
            // absent; Skalden takes the folk modes (doc 06 P11).
            use music::{Mode, Mood};
            for (id, seed, mood, mode) in [
                ("cantorel_bed", 0xCA_0001u64, Mood::Bed, Mode::Ionian),
                ("town_pausa", 0xCA_0002, Mood::Town, Mode::Ionian),
                ("town_prelude", 0xCA_0003, Mood::Town, Mode::Ionian),
                ("town_arbor", 0xCA_0004, Mood::Town, Mode::Ionian),
                ("town_calando", 0xCA_0005, Mood::Town, Mode::Ionian),
                ("battle_wild", 0xCA_0010, Mood::BattleWild, Mode::Aeolian),
                (
                    "battle_trainer",
                    0xCA_0011,
                    Mood::BattleTrainer,
                    Mode::Aeolian,
                ),
                ("battle_hall", 0xCA_0012, Mood::BattleHall, Mode::Ionian),
                ("skalden_bed", 0x5CA_0001, Mood::Bed, Mode::Dorian),
                ("town_skald", 0x5CA_0002, Mood::Town, Mode::Dorian),
                ("town_varde", 0x5CA_0003, Mood::Town, Mode::Aeolian),
                (
                    "battle_skalden",
                    0x5CA_0010,
                    Mood::BattleTrainer,
                    Mode::Dorian,
                ),
                (
                    "battle_skalden_hall",
                    0x5CA_0011,
                    Mood::BattleHall,
                    Mode::Aeolian,
                ),
            ] {
                music::render_track(id, seed, mood, mode, &tracks)?;
            }
            music::render_sfx(&out.join("sfx"))?;
            println!("music: 13 tracks + 13 cues → {}", out.display());
            Ok(true)
        }
        Some("importmap") => {
            let rest: Vec<String> = args.collect();
            let value = |flag: &str| -> Result<String> {
                rest.iter()
                    .position(|a| a == flag)
                    .and_then(|i| rest.get(i + 1))
                    .cloned()
                    .with_context(|| format!("importmap needs {flag} <value>\n{USAGE}"))
            };
            let input = PathBuf::from(value("--in")?);
            let out = PathBuf::from(value("--out")?);
            let written = importmap::import(&input, &out)?;
            println!(
                "importmap: wrote {} map(s): {}",
                written.len(),
                written.join(", ")
            );
            Ok(true)
        }
        Some(other) => bail!("unknown command `{other}`\n{USAGE}"),
        None => bail!("{USAGE}"),
    }
}

fn validate(options: &Options) -> Result<bool> {
    println!("validating {}", options.content.display());

    let content = data::load_core(&options.content)
        .with_context(|| format!("loading content pack at {}", options.content.display()))?;
    let mut findings = data::validate_core(&content);

    // The dev testbed pool is validated whenever it exists (it is content
    // too — golden rule 5 applies to every commit that touches it).
    let dev_pool = options.content.join("dev/testbed.ron");
    if dev_pool.exists() {
        let pool = data::load_species_pool(&dev_pool)
            .with_context(|| format!("loading {}", dev_pool.display()))?;
        findings.extend(data::validate_species_pool(&pool, &content.moves));

        // Dev maps validate against the same pool.
        let maps_root = options.content.join("dev/maps");
        if maps_root.exists() {
            let maps = data::load_maps(&maps_root)
                .with_context(|| format!("loading maps under {}", maps_root.display()))?;
            let script_exists = |map: &undersong_core::ids::MapId, path: &str| {
                maps_root
                    .join(map.as_str())
                    .join("scripts")
                    .join(path)
                    .exists()
            };
            findings.extend(data::validate_maps(
                &maps,
                &pool,
                &script_exists,
                &Default::default(),
            ));

            // Scripts must parse as the Cmd vocabulary, and Choice
            // branches must not be empty (doc 03 §5).
            for map_id in maps.keys() {
                let scripts_dir = maps_root.join(map_id.as_str()).join("scripts");
                let Ok(entries) = std::fs::read_dir(&scripts_dir) else {
                    continue;
                };
                for entry in entries.filter_map(Result::ok) {
                    let path = entry.path();
                    if path.extension().is_none_or(|e| e != "ron") {
                        continue;
                    }
                    let text = std::fs::read_to_string(&path)
                        .with_context(|| format!("reading {}", path.display()))?;
                    match ron::from_str::<Vec<script::Cmd>>(&text) {
                        Err(error) => findings.push(data::Finding {
                            severity: data::Severity::Error,
                            rule: "script.parse",
                            message: format!("{}: {error}", path.display()),
                        }),
                        Ok(cmds) => check_script_cmds(&cmds, &path, &mut findings),
                    }
                }
            }
        }
    }

    // Palette (doc 05 §2) is core content.
    let palette = data::load_palette(&options.content)
        .with_context(|| format!("loading palette under {}", options.content.display()))?;
    findings.extend(data::validate_palette(&palette));

    // Items (doc 02 §8/§15).
    let items = data::load_items(&options.content)
        .with_context(|| format!("loading items under {}", options.content.display()))?;
    findings.extend(data::validate_items(&items));

    // Region packs (doc 04 §3): every directory under content/regions/.
    let regions_root = options.content.join("regions");
    if regions_root.exists() {
        // First pass: every pack's map ids (cross-region warp targets).
        let mut all_map_ids: std::collections::BTreeSet<undersong_core::ids::MapId> =
            Default::default();
        let mut all_pack_moves: Vec<undersong_core::moves::MoveSpec> = Vec::new();
        {
            let mut dirs: Vec<_> = std::fs::read_dir(&regions_root)
                .with_context(|| format!("reading {}", regions_root.display()))?
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
                let pack = data::load_region(&options.content, &region)
                    .with_context(|| format!("loading region `{region}`"))?;
                all_map_ids.extend(pack.maps.keys().cloned());
                all_pack_moves.extend(pack.moves.iter().cloned());
            }
        }
        let mut region_dirs: Vec<_> = std::fs::read_dir(&regions_root)
            .with_context(|| format!("reading {}", regions_root.display()))?
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .collect();
        region_dirs.sort();
        for dir in region_dirs {
            let region = dir
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or_default()
                .to_string();
            let pack = data::load_region(&options.content, &region)
                .with_context(|| format!("loading region `{region}`"))?;
            let mut script_keys: Vec<String> = Vec::new();
            let mut script_warps: Vec<(undersong_core::ids::MapId, undersong_core::ids::MapId)> =
                Vec::new();
            let mut script_flags: Vec<String> = Vec::new();

            // Region scripts parse, too.
            for map_id in pack.maps.keys() {
                let scripts_dir = dir.join("maps").join(map_id.as_str()).join("scripts");
                let Ok(entries) = std::fs::read_dir(&scripts_dir) else {
                    continue;
                };
                for entry in entries.filter_map(Result::ok) {
                    let path = entry.path();
                    if path.extension().is_none_or(|e| e != "ron") {
                        continue;
                    }
                    let text = std::fs::read_to_string(&path)
                        .with_context(|| format!("reading {}", path.display()))?;
                    match ron::from_str::<Vec<script::Cmd>>(&text) {
                        Err(error) => findings.push(data::Finding {
                            severity: data::Severity::Error,
                            rule: "script.parse",
                            message: format!("{}: {error}", path.display()),
                        }),
                        Ok(cmds) => {
                            check_script_cmds(&cmds, &path, &mut findings);
                            collect_script_refs(
                                &cmds,
                                &pack,
                                &items,
                                &path,
                                &mut script_keys,
                                &mut findings,
                                &all_map_ids,
                            );
                            collect_script_warps(&cmds, map_id, &mut script_warps);
                            collect_script_flags(&cmds, &mut script_flags);
                        }
                    }
                }
            }

            let external: std::collections::BTreeSet<undersong_core::ids::MapId> = all_map_ids
                .iter()
                .filter(|m| !pack.maps.contains_key(*m))
                .cloned()
                .collect();
            // Moves are shared across packs at load (the registry is a
            // union) — validate against the same union.
            let mut content_union = content.clone();
            content_union
                .moves
                .moves
                .extend(all_pack_moves.iter().cloned());
            findings.extend(data::validate_region(
                &pack,
                &content_union,
                &items,
                &script_warps,
                &external,
            ));

            // Gate P6: ending reachability — every credits flag must be
            // settable by some script in the pack.
            if region == "cantorel" {
                for ending in ["credits.dacapo", "credits.tacet", "credits.chorus"] {
                    if !script_flags.iter().any(|f| f == ending) {
                        findings.push(data::Finding {
                            severity: data::Severity::Error,
                            rule: "story.endings",
                            message: format!("no script sets `{ending}` — ending unreachable"),
                        });
                    }
                }
            }

            // Doc 04 §3 rules 1 & 8: all referenced strings resolve.
            let core_strings = data::load_core_strings(&options.content)
                .with_context(|| "loading core strings")?;
            findings.extend(data::validate_strings(&pack, &core_strings, &script_keys));

            // TM references resolve (doc 02 v1.6 #2).
            let move_exists =
                |id: &undersong_core::ids::MoveId| content_union.moves.get(id).is_some();
            findings.extend(data::validate_item_moves(&items, &move_exists));
        }
    }

    for finding in &findings {
        println!("{finding}");
    }
    let errors = findings
        .iter()
        .filter(|f| f.severity == data::Severity::Error)
        .count();
    let warnings = findings.len() - errors;
    println!("validate: {errors} error(s), {warnings} warning(s)");
    Ok(errors == 0)
}

/// Recursive Choice/If sanity for script content (doc 03 §5).
/// Collects every flag a script can set (ending reachability rule).
fn collect_script_flags(cmds: &[script::Cmd], flags: &mut Vec<String>) {
    for cmd in cmds {
        match cmd {
            script::Cmd::SetFlag { flag } => flags.push(flag.clone()),
            script::Cmd::Choice { branches, .. } => {
                for (_, branch) in branches {
                    collect_script_flags(branch, flags);
                }
            }
            script::Cmd::If { then, r#else, .. } => {
                collect_script_flags(then, flags);
                collect_script_flags(r#else, flags);
            }
            _ => {}
        }
    }
}

/// Collects script-driven warp edges for the reachability rule.
fn collect_script_warps(
    cmds: &[script::Cmd],
    from: &undersong_core::ids::MapId,
    warps: &mut Vec<(undersong_core::ids::MapId, undersong_core::ids::MapId)>,
) {
    for cmd in cmds {
        match cmd {
            script::Cmd::Warp { map, .. } => warps.push((from.clone(), map.clone())),
            script::Cmd::Choice { branches, .. } => {
                for (_, branch) in branches {
                    collect_script_warps(branch, from, warps);
                }
            }
            script::Cmd::If { then, r#else, .. } => {
                collect_script_warps(then, from, warps);
                collect_script_warps(r#else, from, warps);
            }
            _ => {}
        }
    }
}

/// Walks a script collecting string keys and validating side-effect
/// references (species/trainers/items/maps) against the pack.
fn collect_script_refs(
    cmds: &[script::Cmd],
    pack: &data::RegionPack,
    items: &data::ItemSet,
    path: &std::path::Path,
    keys: &mut Vec<String>,
    findings: &mut Vec<data::Finding>,
    all_maps: &std::collections::BTreeSet<undersong_core::ids::MapId>,
) {
    for cmd in cmds {
        match cmd {
            script::Cmd::Say { who, key } => {
                keys.push(format!("npc.{who}"));
                keys.push(key.clone());
            }
            script::Cmd::Choice { key, branches } => {
                keys.push(key.clone());
                for (label, branch) in branches {
                    keys.push(label.clone());
                    collect_script_refs(branch, pack, items, path, keys, findings, all_maps);
                }
            }
            script::Cmd::If { then, r#else, .. } => {
                collect_script_refs(then, pack, items, path, keys, findings, all_maps);
                collect_script_refs(r#else, pack, items, path, keys, findings, all_maps);
            }
            script::Cmd::GiveMote { species, .. } if !pack.motifs.contains_key(species) => {
                findings.push(data::Finding {
                    severity: data::Severity::Error,
                    rule: "script.ref",
                    message: format!("{}: unknown species `{species}`", path.display()),
                });
            }
            script::Cmd::StartWildBattle { species, .. } if !pack.motifs.contains_key(species) => {
                findings.push(data::Finding {
                    severity: data::Severity::Error,
                    rule: "script.ref",
                    message: format!("{}: unknown species `{species}`", path.display()),
                });
            }
            script::Cmd::StartBattle { trainer } if !pack.trainers.contains_key(trainer) => {
                findings.push(data::Finding {
                    severity: data::Severity::Error,
                    rule: "script.ref",
                    message: format!("{}: unknown trainer `{trainer}`", path.display()),
                });
            }
            script::Cmd::GiveItem { id, .. } if !items.items.iter().any(|i| &i.id == id) => {
                findings.push(data::Finding {
                    severity: data::Severity::Error,
                    rule: "script.ref",
                    message: format!("{}: unknown item `{id}`", path.display()),
                });
            }
            script::Cmd::Warp { map, .. }
                if !pack.maps.contains_key(map) && !all_maps.contains(map) =>
            {
                findings.push(data::Finding {
                    severity: data::Severity::Error,
                    rule: "script.ref",
                    message: format!("{}: unknown warp map `{map}`", path.display()),
                });
            }
            _ => {}
        }
    }
}

fn check_script_cmds(
    cmds: &[script::Cmd],
    path: &std::path::Path,
    findings: &mut Vec<data::Finding>,
) {
    for cmd in cmds {
        match cmd {
            script::Cmd::Choice { key, branches } => {
                if branches.is_empty() {
                    findings.push(data::Finding {
                        severity: data::Severity::Error,
                        rule: "script.choice",
                        message: format!("{}: Choice `{key}` has no branches", path.display()),
                    });
                }
                for (_, branch) in branches {
                    check_script_cmds(branch, path, findings);
                }
            }
            script::Cmd::If { then, r#else, .. } => {
                check_script_cmds(then, path, findings);
                check_script_cmds(r#else, path, findings);
            }
            _ => {}
        }
    }
}

/// `tools assets`: renders every motif's sigil sprites + cry from its
/// seeds (doc 04 §5–§6). Same melody feeds both — the signature trick.
fn generate_assets(
    content_root: &std::path::Path,
    region: &str,
    out: &std::path::Path,
) -> Result<bool> {
    let pack = data::load_region(content_root, region)
        .with_context(|| format!("loading region `{region}`"))?;
    let palette = data::load_palette(content_root).context("loading palette")?;
    let sigil_dir = out.join("sigils").join(region);
    let cry_dir = out.join("cries").join(region);
    // Evolution stage per species (root = 0): drives the leitmotif
    // ornament count (doc 04 §6).
    let mut stage: std::collections::BTreeMap<_, u8> = std::collections::BTreeMap::new();
    for motif in pack.motifs.values() {
        if let Some(evolution) = &motif.evolution {
            let parent_stage = stage.get(&motif.id).copied().unwrap_or(0);
            let entry = stage.entry(evolution.target.clone()).or_insert(0);
            *entry = (*entry).max(parent_stage + 1);
        }
    }
    // Two passes settle 3-stage lines regardless of BTree order.
    for motif in pack.motifs.values() {
        if let Some(evolution) = &motif.evolution {
            let parent_stage = stage.get(&motif.id).copied().unwrap_or(0);
            let entry = stage.entry(evolution.target.clone()).or_insert(0);
            *entry = (*entry).max(parent_stage + 1);
        }
    }

    let mut count = 0usize;
    for motif in pack.motifs.values() {
        let primary = motif.types[0];
        let tune = melody::melody(
            motif.cry_seed,
            primary,
            motif.base_stats.spe,
            motif.dex.weight_hg,
            stage.get(&motif.id).copied().unwrap_or(0),
        );
        let type_hex = palette
            .type_colors
            .get(&primary)
            .cloned()
            .unwrap_or_else(|| "#7d4f9e".to_string());
        sigils::render_sigil(
            motif.id.as_str(),
            motif.sigil_seed,
            primary,
            &type_hex,
            &palette.gilt,
            &tune,
            &sigil_dir,
        )?;
        cries::render_cry(motif.id.as_str(), &tune, &cry_dir)?;
        count += 1;
    }
    println!(
        "assets: {count} motifs → {} + {}",
        sigil_dir.display(),
        cry_dir.display()
    );
    Ok(true)
}

fn load_pool(options: &Options) -> Result<(data::SpeciesPool, data::CoreContent)> {
    let content = data::load_core(&options.content)
        .with_context(|| format!("loading content pack at {}", options.content.display()))?;
    let pool = data::load_species_pool(&options.pool)
        .with_context(|| format!("loading species pool {}", options.pool.display()))?;
    let pool_findings = data::validate_species_pool(&pool, &content.moves);
    if pool_findings
        .iter()
        .any(|f| f.severity == data::Severity::Error)
    {
        for finding in &pool_findings {
            eprintln!("{finding}");
        }
        bail!("species pool failed validation");
    }
    Ok((pool, content))
}

fn simulate(options: &Options) -> Result<bool> {
    let (pool, content) = if let Some(region) = &options.region {
        // Region pool: motifs' battle specs + core moves extended with
        // the region's own (doc 04 §4 balance loop). An explicit --pool
        // narrows the species set (band runs) while keeping the region's
        // move table.
        let pack = data::load_region(&options.content, region)
            .with_context(|| format!("loading region `{region}`"))?;
        let mut content = data::load_core(&options.content)?;
        content.moves.moves.extend(pack.moves.iter().cloned());
        let pool = if options.pool != Options::default().pool {
            data::load_species_pool(&options.pool)
                .with_context(|| format!("loading pool {}", options.pool.display()))?
        } else {
            data::SpeciesPool {
                species: pack.motifs.values().map(data::Motif::spec).collect(),
            }
        };
        let findings = data::validate_species_pool(&pool, &content.moves);
        if findings.iter().any(|f| f.severity == data::Severity::Error) {
            for finding in &findings {
                eprintln!("{finding}");
            }
            anyhow::bail!("species pool failed validation");
        }
        (pool, content)
    } else {
        load_pool(options)?
    };
    let config = sim::SimConfig {
        battles: options.battles,
        level: options.level,
        seed: options.seed.unwrap_or(0x00C0_FFEE),
        ..Default::default()
    };
    println!(
        "simulate: pool {} ({} species), {} battles, level {}, seed {:#x}",
        options.pool.display(),
        pool.species.len(),
        config.battles,
        config.level,
        config.seed
    );
    let report = sim::simulate(&pool, &content.moves, &content.typechart, &config);
    print!("{}", report.render());
    // Gate P1 (docs/06-ROADMAP.md): T2 must beat T0 ≥ 90% with equal teams.
    Ok(report.t2_rate_percent() >= 90.0
        && report.t3_vs_t0_rate_percent() >= 90.0
        && report.t3_rate_percent() >= 45.0)
}

fn run_battle(options: &Options) -> Result<bool> {
    let seed = options
        .seed
        .context("tools battle requires --seed (replays are seed-addressed)")?;
    let (pool, content) = load_pool(options)?;
    let (state, _, mut rng) = sim::one_battle(
        &pool,
        &content.moves,
        &content.typechart,
        options.level,
        3,
        seed,
    );

    println!("battle: seed {seed:#x}, level {}, T2 vs T2", options.level);
    for (side, label) in [(0usize, "you"), (1usize, "foe")] {
        let roster: Vec<String> = state.sides[side]
            .party
            .iter()
            .map(|m| format!("{} L{}", m.species, m.level))
            .collect();
        println!("  [{label}] {}", roster.join(", "));
    }
    println!();

    let (_, events) = sim::play_out_with_events(state, [AiTier::T2, AiTier::T2], &mut rng);
    print!("{}", render::render_events(&events));
    Ok(true)
}
