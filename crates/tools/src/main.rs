//! Undersong CLI tools (docs/03-ARCHITECTURE.md §1).
//!
//! P1 ships `validate`, `simulate`, and `battle`. importmap/cries/sigils/
//! atlas arrive with the phases that need them.
//!
//! No CLI dependency: the closed dependency list (doc 03 §7) has no
//! argument parser, and a handful of flags doesn't justify one.

mod render;
mod sim;

use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{Context, Result, bail};
use battle::ai::AiTier;

const USAGE: &str = "usage:
  tools validate [--content <dir>]
  tools simulate [--battles <n>] [--pool <file>] [--level <n>] [--seed <n>] [--content <dir>]
  tools battle   --seed <n> [--pool <file>] [--level <n>] [--content <dir>]";

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
}

impl Default for Options {
    fn default() -> Self {
        Self {
            content: PathBuf::from("content"),
            pool: PathBuf::from("content/dev/testbed.ron"),
            battles: 1000,
            level: 30,
            seed: None,
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
                &["--battles", "--pool", "--level", "--seed", "--content"],
            )?;
            simulate(&options)
        }
        Some("battle") => {
            let options = parse_options(args, &["--seed", "--pool", "--level", "--content"])?;
            run_battle(&options)
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
            findings.extend(data::validate_maps(&maps, &pool, &script_exists));
        }
    }

    // Palette (doc 05 §2) is core content.
    let palette = data::load_palette(&options.content)
        .with_context(|| format!("loading palette under {}", options.content.display()))?;
    findings.extend(data::validate_palette(&palette));

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
    let (pool, content) = load_pool(options)?;
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
    Ok(report.t2_rate_percent() >= 90.0)
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
