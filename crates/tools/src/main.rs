//! Undersong CLI tools (docs/03-ARCHITECTURE.md §1).
//!
//! P0 ships `validate`. The other subcommands — simulate, importmap,
//! cries, sigils, atlas — arrive with the phases that need them.
//!
//! No CLI dependency: the closed dependency list (doc 03 §7) has no
//! argument parser, and two flags don't justify one.

use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{Context, Result, bail};

const USAGE: &str = "usage: tools validate [--content <dir>]";

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

/// Returns `Ok(true)` when the command ran and found no errors.
fn run() -> Result<bool> {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("validate") => validate(parse_content_root(args)?),
        Some(other) => bail!("unknown command `{other}`\n{USAGE}"),
        None => bail!("{USAGE}"),
    }
}

fn parse_content_root(mut args: impl Iterator<Item = String>) -> Result<PathBuf> {
    let mut root = PathBuf::from("content");
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--content" => {
                root = args
                    .next()
                    .map(PathBuf::from)
                    .context("--content needs a directory argument")?;
            }
            other => bail!("unknown argument `{other}`\n{USAGE}"),
        }
    }
    Ok(root)
}

fn validate(content_root: PathBuf) -> Result<bool> {
    println!("validating {}", content_root.display());

    let content = data::load_core(&content_root)
        .with_context(|| format!("loading content pack at {}", content_root.display()))?;
    let findings = data::validate_core(&content);

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
