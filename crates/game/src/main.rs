//! Undersong client (docs/03-ARCHITECTURE.md §3).
//!
//! P0 scope: a window at the virtual resolution with proof-of-life text and
//! the stubbed `AppState`. The real plugin set (Boot, Overworld, Battle, …)
//! arrives from P2.

use bevy::prelude::*;
use bevy::window::WindowResolution;

/// Virtual resolution (docs/05-UI-STYLE.md §1): all UI is authored at
/// 480×270 and integer-scaled to the window.
const VIRTUAL_WIDTH: u32 = 480;
const VIRTUAL_HEIGHT: u32 = 270;

/// P0 opens at a fixed ×2 (960×540). The scale picker and letterboxed
/// resizing arrive with Settings in P2.
const WINDOW_SCALE: u32 = 2;

/// Top-level app states (doc 03 §3). Stubbed in P0: only `Boot` is ever
/// active; transitions arrive with the state plugins from P2.
#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[allow(dead_code, reason = "variants are wired up by the P2 state plugins")]
enum AppState {
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
    App::new()
        .add_plugins(
            DefaultPlugins
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
                // Nearest-neighbor everywhere: crisp pixels are the law
                // (doc 05 §1).
                .set(ImagePlugin::default_nearest()),
        )
        .init_state::<AppState>()
        .add_systems(Startup, setup)
        .run()
}

fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);
    commands.spawn((
        Text2d::new("UNDERSONG P0"),
        TextFont::from_font_size(32.0),
        TextColor(Color::WHITE),
    ));
}
