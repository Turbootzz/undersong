//! Undersong client (docs/03-ARCHITECTURE.md §3).
//!
//! `--replay <file>` runs the pure replay driver and exits — no window,
//! no Bevy app (the overworld core is engine-free; see `world.rs`).
//! Otherwise the windowed app renders the world state.

#[cfg(not(feature = "headless"))]
mod app;
#[cfg(not(feature = "headless"))]
mod battle_ui;
#[cfg(not(feature = "headless"))]
mod screens;

use bevy::prelude::AppExit;
#[cfg(not(feature = "headless"))]
use bevy::prelude::*;
#[cfg(not(feature = "headless"))]
use bevy::window::WindowResolution;

use game::replay;

/// Virtual resolution (doc 05 §1): all UI authored at 480×270,
/// integer-scaled.
#[cfg(not(feature = "headless"))]
const VIRTUAL_WIDTH: u32 = 480;
#[cfg(not(feature = "headless"))]
const VIRTUAL_HEIGHT: u32 = 270;
/// P2 still opens at a fixed ×2; the Settings scale picker applies it
/// for real in P3 polish. The UI scale and camera zoom derive from this
/// one constant.
pub const WINDOW_SCALE: u32 = 2;

/// Top-level app states (doc 03 §3). Dialogue is an overlay in P2 (the
/// pure world gates movement); the variant stays reserved for the P3
/// presenter flow.
#[cfg(not(feature = "headless"))]
#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum AppState {
    #[default]
    Boot,
    Title,
    Overworld,
    Battle,
    Menu,
    Dialogue,
    Transition,
}

fn main() -> AppExit {
    let args: Vec<String> = std::env::args().collect();
    if let Some(index) = args.iter().position(|a| a == "--replay") {
        let Some(path) = args.get(index + 1) else {
            eprintln!("--replay needs a file path");
            return AppExit::error();
        };
        return match replay::run_replay_file(
            std::path::Path::new("content"),
            std::path::Path::new(path),
        ) {
            Ok(outcome) => {
                println!(
                    "replay ok: {} dialogue lines, {} warps, {} saves",
                    outcome.dialogue_lines, outcome.warps, outcome.saves
                );
                AppExit::Success
            }
            Err(message) => {
                eprintln!("replay failed: {message}");
                AppExit::error()
            }
        };
    }

    #[cfg(feature = "headless")]
    {
        eprintln!("headless build: pass --replay <file> (no window will open)");
        AppExit::error()
    }
    #[cfg(not(feature = "headless"))]
    App::new()
        .add_plugins(
            DefaultPlugins
                .set(bevy::asset::AssetPlugin {
                    // Run from the workspace root in dev; the shipping
                    // layout is settled in P7 (doc 06).
                    file_path: "../../assets".into(),
                    ..default()
                })
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Undersong".into(),
                        resolution: WindowResolution::new(
                            VIRTUAL_WIDTH * WINDOW_SCALE,
                            VIRTUAL_HEIGHT * WINDOW_SCALE,
                        ),
                        resizable: false,
                        ..default()
                    }),
                    ..default()
                })
                .set(ImagePlugin::default_nearest()),
        )
        .init_state::<AppState>()
        .add_plugins(app::UndersongPlugin)
        .add_plugins(battle_ui::BattleUiPlugin)
        .add_plugins(screens::ScreensPlugin)
        .run()
}
