//! The Bevy presentation layer: renders the pure `world` core and feeds
//! it inputs. No game rules live here (doc 03 §3) — tiles, actors, and
//! UI are colored quads and text until the sigil pipeline lands (P3+).

use bevy::prelude::*;
use bevy::ui::UiScale;
use data::Palette;
use game::world::{Input as WorldInput, WorldEvent, WorldState, load_game_world};
use undersong_core::types::Type;
use undersong_core::world::Facing;

use crate::{AppState, WINDOW_SCALE};

const TILE: f32 = 32.0;
const VIEW_W: f32 = 640.0;
const VIEW_H: f32 = 360.0;
/// Walk interpolation (doc 02 §12: 150 ms per tile).
const WALK_SECONDS: f32 = 0.15;

pub struct UndersongPlugin;

impl Plugin for UndersongPlugin {
    fn build(&self, app: &mut App) {
        // UI is authored on the old 270-line canvas; the 360-line world
        // (doc 05 v2) keeps those proportions via a 4/3 factor.
        app.insert_resource(UiScale(WINDOW_SCALE as f32 * 4.0 / 3.0))
            .insert_resource(RenderedMap(None))
            .insert_resource(PlayerAnim(None))
            .insert_resource(WanderTimer(Timer::from_seconds(1.2, TimerMode::Repeating)))
            .insert_resource(MenuCursor(0))
            .insert_resource(SettingsRes(save::Settings::default()))
            .insert_resource(SettingsOpen(false))
            .insert_resource(Wipe(None))
            .insert_resource(DialogueReveal::default())
            .insert_resource(StepParity(false))
            .insert_resource(Spotted(None))
            .insert_resource(FxQueue::default())
            .add_systems(OnEnter(AppState::Boot), boot_load)
            .add_systems(
                Update,
                (
                    player_input,
                    rebuild_map_if_needed,
                    animate_player,
                    animate_player_frame,
                    facing_marker,
                    npc_wander,
                    sync_npc_sprites,
                    camera_follow,
                    dialogue_ui,
                    shop_ui,
                    advance_wipe,
                )
                    .chain()
                    .run_if(in_state(AppState::Overworld)),
            )
            .add_systems(
                Update,
                (
                    spotted_tick,
                    overworld_fx,
                    water_shimmer,
                    window_glow,
                    weather_fx,
                    flurry_drift,
                )
                    .run_if(in_state(AppState::Overworld)),
            )
            .insert_resource(Toast::default())
            .insert_resource(CurrentMusic::default())
            .insert_resource(CreditsState::default())
            .insert_resource(AudioUnlocked::default())
            .add_systems(
                Update,
                (
                    audio_unlock,
                    pixel_font_swap,
                    music_director,
                    credits_watch,
                    screenshot_key,
                    boot_battle_rig,
                    boot_map_rig,
                    walk_demo_rig,
                    theater_demo_rig,
                    visual_replay,
                ),
            )
            .add_systems(
                Update,
                (toast_ui, night_tint).run_if(in_state(AppState::Overworld)),
            )
            .add_systems(
                OnExit(AppState::Overworld),
                (
                    cleanup_wipe,
                    clear_overworld_fx,
                    despawn_tagged::<DialogueUi>,
                    despawn_tagged::<ToastUi>,
                    despawn_tagged::<WeatherFx>,
                    despawn_tagged::<SpottedBubble>,
                ),
            )
            .add_systems(OnExit(AppState::Battle), resync_after_battle)
            .add_systems(OnEnter(AppState::Menu), menu_open)
            .add_systems(Update, menu_input.run_if(in_state(AppState::Menu)))
            .add_systems(OnExit(AppState::Menu), despawn_tagged::<MenuUi>)
            .add_systems(OnEnter(AppState::Title), title_open)
            .add_systems(Update, title_input.run_if(in_state(AppState::Title)))
            .add_systems(OnExit(AppState::Title), despawn_tagged::<TitleUi>);
    }
}

// ----- resources & markers ------------------------------------------------

#[derive(Resource)]
pub struct WorldRes(pub WorldState);

#[derive(Resource)]
pub struct Theme {
    pub palette: Palette,
}

impl Theme {
    pub fn color(&self, hex: &str) -> Color {
        let parse = |s: &str| u8::from_str_radix(s, 16).unwrap_or(255);
        Color::srgb_u8(parse(&hex[1..3]), parse(&hex[3..5]), parse(&hex[5..7]))
    }

    pub fn ty(&self, ty: Type) -> Color {
        self.palette
            .type_colors
            .get(&ty)
            .map(|hex| self.color(hex))
            .unwrap_or(Color::WHITE)
    }
}

pub fn shade(color: Color, factor: f32) -> Color {
    let c = color.to_srgba();
    Color::srgba(
        (c.red * factor).min(1.0),
        (c.green * factor).min(1.0),
        (c.blue * factor).min(1.0),
        c.alpha,
    )
}

#[derive(Resource)]
struct RenderedMap(Option<undersong_core::ids::MapId>);

#[derive(Resource)]
struct PlayerAnim(Option<(Vec2, Vec2, f32)>);

/// The soft dot on the tile the player faces (interact target).
#[derive(Component)]
struct FacingMarker;

#[derive(Resource)]
struct WanderTimer(Timer);

#[derive(Resource)]
struct MenuCursor(usize);

/// Live settings (doc 05 §5 subset); persisted into saves.
#[derive(Resource)]
pub struct SettingsRes(pub save::Settings);

#[derive(Resource)]
struct SettingsOpen(bool);

/// Measure-bar wipe on warps (doc 05 §6): a bar sweeps across, then
/// fades. `t` runs 0..1.
#[derive(Resource)]
struct Wipe(Option<f32>);

#[derive(Component)]
struct MapTile;

#[derive(Component)]
struct PlayerSprite;

#[derive(Component)]
struct NpcSprite(String);

#[derive(Component)]
struct DialogueUi;

#[derive(Component)]
struct DialogueText;

#[derive(Component)]
struct NameTagText;

#[derive(Component)]
struct MenuUi;

#[derive(Component)]
struct MenuRow(usize);

#[derive(Component)]
struct WipeBar;

// ----- P18 overworld feel ----------------------------------------------

/// Walk-cycle step parity: strides alternate per tile so the four-frame
/// gait reads as a two-step (presenter-only).
#[derive(Resource, Default)]
struct StepParity(bool);

/// Spotted! (P18): a trainer's line of sight engaged — the alert cue,
/// the "!" bubble, and a beat of held input before the scene moves.
/// (npc id, seconds remaining, cue+bubble fired).
#[derive(Resource, Default)]
struct Spotted(Option<(String, f32, bool)>);

#[derive(Component)]
struct SpottedBubble;

/// One-shot world effects queued by the event fan-out (it has no
/// Commands) and drained by `overworld_fx`.
#[derive(Resource, Default)]
struct FxQueue {
    rustles: Vec<(u32, u32)>,
    door: Option<(u32, u32)>,
}

/// Grass-rustle burst on a resonance patch (elapsed seconds).
#[derive(Component)]
struct RustleFx(f32);

/// The lit-doorway flash on the arrival door (elapsed seconds; runs
/// long enough to outlive the warp wipe).
#[derive(Component)]
struct DoorFx(f32);

/// Animated water tile (two-frame shimmer).
#[derive(Component)]
struct WaterTile;

/// A lit-window decor tile (brightens at night).
#[derive(Component)]
struct WindowTile;

/// Weather overlay pieces (vignette, drifting specks).
#[derive(Component)]
struct WeatherFx;

/// One drifting weather speck; the seed spreads them deterministically.
#[derive(Component)]
struct FlurrySpeck(u32);

// ----- boot -----------------------------------------------------------

/// Keeps small, frequently-swapped sprites resident — without this the
/// first use of each walk pose / fx frame hits an async load and the
/// sprite blinks out for a beat.
#[derive(Resource)]
struct PreloadedArt(#[expect(dead_code, reason = "held to pin the assets")] Vec<Handle<Image>>);

/// The vendored monogram pixel font (P19, CC0 — see
/// assets/fonts/LICENSE-monogram.txt). It replaces Bevy's default
/// font asset, so every `TextFont::from_font_size` in the codebase
/// picks it up with no per-site plumbing. Native swaps synchronously
/// in boot_load; wasm (no fs) swaps via this handle as soon as the
/// asset lands — early enough, since wasm boots into the title.
#[derive(Resource)]
struct PixelFontHandle(Handle<Font>);

fn pixel_font_swap(
    handle: Option<Res<PixelFontHandle>>,
    mut fonts: ResMut<Assets<Font>>,
    mut done: Local<bool>,
) {
    if *done {
        return;
    }
    let Some(handle) = handle else { return };
    let Some(font) = fonts.get(&handle.0).cloned() else {
        return;
    };
    if fonts.insert(&TextFont::default().font, font).is_err() {
        bevy::log::warn!("pixel font: default-font swap failed");
    }
    *done = true;
}

fn boot_load(
    mut commands: Commands,
    assets: Res<AssetServer>,
    mut fonts: ResMut<Assets<Font>>,
    mut next: ResMut<NextState<AppState>>,
) {
    // The pixel font must own the default-font handle BEFORE any text
    // spawns — glyph atlases are cached per font id, so a later swap
    // never repaints text that already rendered once.
    #[cfg(not(target_arch = "wasm32"))]
    match std::fs::read("assets/fonts/monogram.ttf") {
        Ok(bytes) => match Font::try_from_bytes(bytes) {
            Ok(font) => {
                let _ = fonts.insert(&TextFont::default().font, font);
            }
            Err(error) => bevy::log::warn!("pixel font parse failed: {error:?}"),
        },
        Err(error) => bevy::log::warn!("pixel font read failed: {error}"),
    }
    #[cfg(target_arch = "wasm32")]
    commands.insert_resource(PixelFontHandle(assets.load("fonts/monogram.ttf")));

    let content = std::path::Path::new("content");
    let palette = data::load_palette(content).expect("palette.ron must load");
    let world = load_game_world(content, 0x00D0_5EED).expect("game world must load");

    let zoom = 1.0 / WINDOW_SCALE as f32;
    commands.spawn((Camera2d, Transform::from_scale(Vec3::new(zoom, zoom, 1.0))));
    commands.insert_resource(Theme { palette });
    commands.insert_resource(WorldRes(world));

    let mut held = Vec::new();
    for dir in ["down", "up", "left", "right"] {
        for frame in 0..4 {
            held.push(assets.load(art(&format!("sprites/chars/player.{dir}.{frame}.png"))));
        }
    }
    for frame in 0..3 {
        held.push(assets.load(art(&format!("sprites/fx/rustle.{frame}.png"))));
    }
    for ty in [
        "feral", "ember", "tide", "bloom", "volt", "gale", "stone", "frost", "venom",
        "phantom", "alloy", "resonant",
    ] {
        for frame in 0..4 {
            held.push(assets.load(art(&format!("sprites/fx/{ty}.{frame}.png"))));
        }
    }
    for rel in [
        "sprites/fx/alert.png",
        "sprites/fx/shadow.png",
        "sprites/fx/vignette.png",
        "sprites/tiles/water.1.png",
        "sprites/tiles/door_open.png",
    ] {
        held.push(assets.load(art(rel)));
    }
    commands.insert_resource(PreloadedArt(held));
    next.set(AppState::Title);
}

// ----- map rendering --------------------------------------------------

fn tile_pos(x: u32, y: u32, z: f32) -> Transform {
    Transform::from_xyz(
        x as f32 * TILE + TILE / 2.0,
        y as f32 * TILE + TILE / 2.0,
        z,
    )
}

use game::art::art;

fn art_sprite(assets: &AssetServer, rel: &str, size: f32) -> Sprite {
    Sprite {
        image: assets.load(art(rel)),
        custom_size: Some(Vec2::splat(size)),
        ..default()
    }
}

/// Character sprites are 32×44 (P13 anime proportions), anchored so
/// the feet stand on the tile and the head overflows upward.
fn char_sprite(assets: &AssetServer, rel: &str) -> (Sprite, bevy::sprite::Anchor) {
    (
        Sprite {
            image: assets.load(art(rel)),
            custom_size: Some(Vec2::new(TILE, TILE * 44.0 / 32.0)),
            ..default()
        },
        bevy::sprite::Anchor::BOTTOM_CENTER,
    )
}

/// Transform for a bottom-anchored character on tile (x, y).
fn char_pos(x: u32, y: u32, z: f32) -> Transform {
    Transform::from_xyz(x as f32 * TILE + TILE / 2.0, y as f32 * TILE, z)
}

/// The soft ellipse under every actor (P18): a child of the character
/// entity, riding just below it in z so it follows for free.
fn shadow_child(assets: &AssetServer) -> (Sprite, Transform) {
    (
        Sprite {
            image: assets.load(art("sprites/fx/shadow.png")),
            custom_size: Some(Vec2::new(24.0, 10.0)),
            ..default()
        },
        Transform::from_xyz(0.0, 3.0, -0.05),
    )
}

fn quad(color: Color, size: f32) -> Sprite {
    Sprite {
        color,
        custom_size: Some(Vec2::splat(size)),
        ..default()
    }
}

#[expect(clippy::too_many_arguments, reason = "bevy system parameters")]
fn rebuild_map_if_needed(
    mut commands: Commands,
    world: Res<WorldRes>,
    theme: Option<Res<Theme>>,
    assets: Res<AssetServer>,
    mut rendered: ResMut<RenderedMap>,
    tiles: Query<Entity, With<MapTile>>,
    npcs: Query<Entity, With<NpcSprite>>,
    player: Query<Entity, With<PlayerSprite>>,
) {
    let Some(theme) = theme else { return };
    if rendered.0.as_ref() == Some(&world.0.current_map) {
        return;
    }
    for entity in tiles.iter().chain(npcs.iter()) {
        commands.entity(entity).despawn();
    }

    let map = world.0.map();
    // Generated tile art (P10) — interiors share ground id 4 with town
    // building blocks; the indoor flag picks plank floor vs masonry.
    let ground_tile = |id: u16| match id {
        1 => "sprites/tiles/grass.png",
        2 => "sprites/tiles/path.png",
        3 => "sprites/tiles/water.png",
        4 => {
            if map.indoor {
                "sprites/tiles/floor.png" // solids resolved per-tile below
            } else {
                "sprites/tiles/wall.png"
            }
        }
        5 => "sprites/tiles/deep.png",
        _ => "sprites/tiles/path.png",
    };
    for y in 0..map.height {
        for x in 0..map.width {
            let index = map.index(x, y);
            let ground = map.ground[index];
            if ground != 0 {
                let rel = if map.indoor && ground == 4 && map.is_solid(x, y) {
                    "sprites/tiles/wall_indoor.png"
                } else {
                    ground_tile(ground)
                };
                let mut tile = commands.spawn((
                    MapTile,
                    art_sprite(&assets, rel, TILE),
                    tile_pos(x, y, 0.0),
                ));
                if ground == 3 {
                    tile.insert(WaterTile); // two-frame shimmer (P18)
                }
            }
            if map.is_patch(x, y) {
                commands.spawn((
                    MapTile,
                    art_sprite(&assets, "sprites/tiles/patch.png", TILE),
                    tile_pos(x, y, 0.5),
                ));
            }
            if let Some(&decor) = map.decor.get(index)
                && decor != 0
            {
                // Town roof color rides the map id (P14 identity).
                let roof = match world
                    .0
                    .current_map
                    .as_str()
                    .bytes()
                    .map(u32::from)
                    .sum::<u32>()
                    % 3
                {
                    0 => "sprites/tiles/roof_red.png",
                    1 => "sprites/tiles/roof_blue.png",
                    _ => "sprites/tiles/roof_green.png",
                };
                let rel = match decor {
                    5 => "sprites/tiles/bush.png",
                    6 => "sprites/tiles/sign.png",
                    10 => roof,
                    12 => "sprites/tiles/door.png",
                    13 => "sprites/tiles/window.png",
                    14 => "sprites/tiles/flowers.png",
                    15 => "sprites/tiles/fence.png",
                    16 => "sprites/tiles/lamp.png",
                    _ => "sprites/tiles/bush.png",
                };
                let mut tile =
                    commands.spawn((MapTile, art_sprite(&assets, rel, TILE), tile_pos(x, y, 1.0)));
                if decor == 13 {
                    tile.insert(WindowTile); // glows at night (P18)
                }
            }
            if let Some(&overhang) = map.overhang.get(index)
                && overhang != 0
            {
                commands.spawn((
                    MapTile,
                    art_sprite(&assets, "sprites/tiles/canopy.png", TILE),
                    tile_pos(x, y, 3.0),
                ));
            }
        }
    }

    // NPCs for this map: archetype sprite by key, facing baked in
    // (unknown keys fall back to the villager set).
    if let Some(states) = world.0.npcs.get(&world.0.current_map) {
        for npc in states {
            let known = matches!(
                npc.sprite.as_str(),
                "npc.villager"
                    | "npc.villager2"
                    | "npc.trainer"
                    | "npc.dario"
                    | "npc.mirelle"
                    | "npc.reed"
                    | "npc.greeter"
            );
            let key = if known {
                npc.sprite.as_str()
            } else {
                "npc.villager"
            };
            let dir = match npc.facing {
                Facing::Down => "down",
                Facing::Up => "up",
                Facing::Left => "left",
                Facing::Right => "right",
            };
            commands
                .spawn((
                    NpcSprite(npc.id.clone()),
                    char_sprite(&assets, &format!("sprites/chars/{key}.{dir}.0.png")),
                    char_pos(npc.at.0, npc.at.1, 2.0),
                ))
                .with_child(shadow_child(&assets));
        }
    }

    // The player persists across maps; spawn once (with the facing
    // marker — P9: "am I looking at the NPC?" must answer itself).
    if player.is_empty() {
        commands
            .spawn((
                PlayerSprite,
                char_sprite(&assets, "sprites/chars/player.down.0.png"),
                char_pos(world.0.player.0, world.0.player.1, 2.0),
            ))
            .with_child(shadow_child(&assets));
        let mut marker_color = theme.color(&theme.palette.gilt);
        marker_color.set_alpha(0.45);
        commands.spawn((
            FacingMarker,
            quad(marker_color, TILE / 3.0),
            tile_pos(world.0.player.0, world.0.player.1.saturating_sub(1), 1.5),
        ));
    }

    rendered.0 = Some(world.0.current_map.clone());
}

// ----- player input & motion -------------------------------------------

fn pressed_direction(keys: &ButtonInput<KeyCode>) -> Option<Facing> {
    if keys.pressed(KeyCode::ArrowUp) || keys.pressed(KeyCode::KeyW) {
        Some(Facing::Up)
    } else if keys.pressed(KeyCode::ArrowDown) || keys.pressed(KeyCode::KeyS) {
        Some(Facing::Down)
    } else if keys.pressed(KeyCode::ArrowLeft) || keys.pressed(KeyCode::KeyA) {
        Some(Facing::Left)
    } else if keys.pressed(KeyCode::ArrowRight) || keys.pressed(KeyCode::KeyD) {
        Some(Facing::Right)
    } else {
        None
    }
}

#[expect(clippy::too_many_arguments, reason = "bevy system parameters")]
fn player_input(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    assets: Res<AssetServer>,
    settings: Res<SettingsRes>,
    mut world: ResMut<WorldRes>,
    mut anim: ResMut<PlayerAnim>,
    mut rendered: ResMut<RenderedMap>,
    mut wipe: ResMut<Wipe>,
    mut next: ResMut<NextState<AppState>>,
    mut player: Query<&mut Transform, With<PlayerSprite>>,
    mut toast: ResMut<Toast>,
    mut reveal: ResMut<DialogueReveal>,
    mut spotted: ResMut<Spotted>,
    mut fxq: ResMut<FxQueue>,
    mut parity: ResMut<StepParity>,
) {
    // An open mart owns the keys (shop_ui routes them).
    if world.0.shop.is_some() {
        return;
    }
    // Spotted! — the beat of pause: the world holds while the bubble
    // hangs over the trainer's head (P18).
    if spotted.0.is_some() {
        return;
    }
    // Interact / advance dialogue / answer choice.
    if keys.just_pressed(KeyCode::KeyZ) || keys.just_pressed(KeyCode::Enter) {
        // Mid-reveal Z completes the typewriter instead of advancing.
        if world.0.dialogue.is_some() && !reveal.done() {
            reveal.complete();
            return;
        }
        let was_talking = world.0.dialogue.is_some();
        let events = world.0.apply(WorldInput::Interact);
        let line = events
            .iter()
            .any(|e| matches!(e, WorldEvent::DialogueLine { .. }));
        if line || was_talking {
            play_cue(&mut commands, &assets, &settings.0, "cursor");
        }
        handle_events(
            &events,
            &mut world,
            &mut anim,
            &mut rendered,
            &mut wipe,
            &mut next,
            &mut player,
            &mut toast,
            &mut spotted,
            &mut fxq,
        );
        return;
    }
    if keys.just_pressed(KeyCode::Escape) && world.0.dialogue.is_none() {
        next.set(AppState::Menu);
        return;
    }
    // Choice cursor movement routes Up/Down into the pure core.
    if world.0.dialogue.is_some() {
        if let Some(dir) = pressed_direction(&keys)
            && keys.any_just_pressed([
                KeyCode::ArrowUp,
                KeyCode::ArrowDown,
                KeyCode::KeyW,
                KeyCode::KeyS,
            ])
        {
            let _ = world.0.apply(WorldInput::Step(dir));
        }
        return;
    }
    // Movement: no buffering — a tap inside the slide window once
    // queued a second step (the playtest's "I skip one square").
    // A held key continues the walk the frame the tile lands; a tap
    // shorter than one slide moves exactly one tile.
    if anim.0.is_some() {
        return;
    }
    let Some(dir) = pressed_direction(&keys) else {
        return;
    };
    let from = world.0.player;
    let events = world.0.apply(WorldInput::Step(dir));
    let stepped = events
        .iter()
        .any(|e| matches!(e, WorldEvent::Stepped { .. }));
    let warped = events
        .iter()
        .any(|e| matches!(e, WorldEvent::Warped { .. }));
    if stepped && !warped {
        let to = world.0.player;
        anim.0 = Some((
            Vec2::new(from.0 as f32 * TILE + TILE / 2.0, from.1 as f32 * TILE),
            Vec2::new(to.0 as f32 * TILE + TILE / 2.0, to.1 as f32 * TILE),
            0.0,
        ));
        // Strides alternate per tile (the four-frame gait, P18).
        parity.0 = !parity.0;
    }
    handle_events(
        &events,
        &mut world,
        &mut anim,
        &mut rendered,
        &mut wipe,
        &mut next,
        &mut player,
        &mut toast,
        &mut spotted,
        &mut fxq,
    );
}

#[expect(clippy::too_many_arguments, reason = "event fan-out helper")]
fn handle_events(
    events: &[WorldEvent],
    world: &mut ResMut<WorldRes>,
    anim: &mut ResMut<PlayerAnim>,
    rendered: &mut ResMut<RenderedMap>,
    wipe: &mut ResMut<Wipe>,
    next: &mut ResMut<NextState<AppState>>,
    player: &mut Query<&mut Transform, With<PlayerSprite>>,
    toast: &mut ResMut<Toast>,
    spotted: &mut ResMut<Spotted>,
    fxq: &mut ResMut<FxQueue>,
) {
    // Any event batch that left a live battle session moves us to the
    // battle scene (wild rolls, LoS engagements, scripted fights).
    if world.0.battle.is_some() {
        anim.0 = None;
        wipe.0 = Some(0.0); // the ink sweep into combat (P11)
        next.set(AppState::Battle);
    }
    for event in events {
        match event {
            WorldEvent::ItemUsed { message_key, .. } => {
                toast.line = Some(world.0.text(message_key));
                toast.timer = 0.0;
            }
            WorldEvent::Performed { performance } => {
                toast.line = Some(world.0.text(performance));
                toast.timer = 0.0;
            }
            WorldEvent::ClockPhase { night } => {
                toast.line = Some(world.0.text(if *night {
                    "ui.clock.night"
                } else {
                    "ui.clock.day"
                }));
                toast.timer = 0.0;
            }
            WorldEvent::Engaged { npc } => {
                // Spotted! — the bubble + beat of pause (P18); the
                // spotted_tick system plays the cue and holds input.
                spotted.0 = Some((npc.clone(), 0.9, false));
            }
            // A step that warps never rustles: the Stepped coords
            // belong to the source map, but world.0.map() is already
            // the arrival map by the time events land here.
            WorldEvent::Stepped { to }
                if !events
                    .iter()
                    .any(|e| matches!(e, WorldEvent::Warped { .. }))
                    && world.0.map().is_patch(to.0, to.1) =>
            {
                fxq.rustles.push(*to);
            }
            _ => {}
        }
    }
    for event in events {
        match event {
            WorldEvent::Warped { to, .. } => {
                rendered.0 = None; // forces a rebuild
                anim.0 = None;
                wipe.0 = Some(0.0);
                if let Ok(mut transform) = player.single_mut() {
                    let (x, y) = world.0.player;
                    transform.translation =
                        Vec3::new(x as f32 * TILE + TILE / 2.0, y as f32 * TILE, 2.0);
                }
                // The arrival door swings lit for a beat (P18). Warps
                // land one tile past the doorway, so the door art sits
                // one step BEHIND the arrival facing (also checked on
                // the tile itself for gates that land on it).
                let map = world.0.map();
                let (dx, dy) = world.0.facing.delta();
                let behind = to
                    .0
                    .checked_add_signed(-dx)
                    .zip(to.1.checked_add_signed(-dy));
                for spot in [Some(*to), behind].into_iter().flatten() {
                    if spot.0 < map.width
                        && spot.1 < map.height
                        && map.decor.get(map.index(spot.0, spot.1)) == Some(&12)
                    {
                        fxq.door = Some(spot);
                        break;
                    }
                }
                // Autosave on map change (doc 03 §4).
                autosave(&world.0);
            }
            WorldEvent::EncounterStarted { .. } => {
                // The step that rolled the encounter never animates; keep
                // the sprite on the logical tile so Overworld resumes in
                // sync.
                anim.0 = None;
                if let Ok(mut transform) = player.single_mut() {
                    let (x, y) = world.0.player;
                    transform.translation =
                        Vec3::new(x as f32 * TILE + TILE / 2.0, y as f32 * TILE, 2.0);
                }
            }
            _ => {}
        }
    }
}

fn animate_player(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mut anim: ResMut<PlayerAnim>,
    mut player: Query<&mut Transform, With<PlayerSprite>>,
) {
    let Some((from, to, mut t)) = anim.0 else {
        return;
    };
    // Hold X to run (doc 02 v2.0 #2) — presenter-only: the logical
    // step cadence and the replays never see it.
    let speed = if keys.pressed(KeyCode::KeyX) {
        2.0
    } else {
        1.0
    };
    t = (t + time.delta_secs() * speed / WALK_SECONDS).min(1.0);
    if let Ok(mut transform) = player.single_mut() {
        let p = from.lerp(to, t);
        transform.translation = Vec3::new(p.x, p.y, 2.0);
    }
    anim.0 = if t >= 1.0 { None } else { Some((from, to, t)) };
}

fn facing_marker(
    world: Res<WorldRes>,
    player: Query<&Transform, (With<PlayerSprite>, Without<FacingMarker>)>,
    mut marker: Query<&mut Transform, With<FacingMarker>>,
) {
    let (Ok(player), Ok(mut marker)) = (player.single(), marker.single_mut()) else {
        return;
    };
    let (dx, dy) = match world.0.facing {
        Facing::Up => (0.0, TILE),
        Facing::Down => (0.0, -TILE),
        Facing::Left => (-TILE, 0.0),
        Facing::Right => (TILE, 0.0),
    };
    marker.translation = Vec3::new(
        player.translation.x + dx,
        player.translation.y + TILE / 2.0 + dy,
        1.5,
    );
}

// ----- NPCs ------------------------------------------------------------

fn npc_wander(time: Res<Time>, mut timer: ResMut<WanderTimer>, mut world: ResMut<WorldRes>) {
    if world.0.dialogue.is_some() {
        return;
    }
    if timer.0.tick(time.delta()).just_finished() {
        let _ = world.0.apply(WorldInput::Tick);
    }
}

fn sync_npc_sprites(
    world: Res<WorldRes>,
    assets: Res<AssetServer>,
    mut npcs: Query<(&NpcSprite, &mut Transform, &mut Sprite)>,
) {
    let Some(states) = world.0.npcs.get(&world.0.current_map) else {
        return;
    };
    for (marker, mut transform, mut sprite) in &mut npcs {
        if let Some(state) = states.iter().find(|n| n.id == marker.0) {
            transform.translation = Vec3::new(
                state.at.0 as f32 * TILE + TILE / 2.0,
                state.at.1 as f32 * TILE,
                2.0,
            );
            // Facing follows the world (wander, FaceNpc effects).
            let known = matches!(
                state.sprite.as_str(),
                "npc.villager"
                    | "npc.villager2"
                    | "npc.trainer"
                    | "npc.dario"
                    | "npc.mirelle"
                    | "npc.reed"
                    | "npc.greeter"
            );
            let key = if known {
                state.sprite.as_str()
            } else {
                "npc.villager"
            };
            let dir = match state.facing {
                Facing::Down => "down",
                Facing::Up => "up",
                Facing::Left => "left",
                Facing::Right => "right",
            };
            sprite.image = assets.load(art(&format!("sprites/chars/{key}.{dir}.0.png")));
        }
    }
}

/// The player's frame: facing × walk phase × step parity. Four frames
/// per direction (P18): the stride leg alternates per tile, with the
/// stand pose between strides — the classic two-step cycle. Running
/// (hold X) speeds the slide, so the cycle doubles with it for free.
fn animate_player_frame(
    world: Res<WorldRes>,
    assets: Res<AssetServer>,
    anim: Res<PlayerAnim>,
    parity: Res<StepParity>,
    mut player: Query<&mut Sprite, With<PlayerSprite>>,
) {
    let Ok(mut sprite) = player.single_mut() else {
        return;
    };
    let dir = match world.0.facing {
        Facing::Down => "down",
        Facing::Up => "up",
        Facing::Left => "left",
        Facing::Right => "right",
    };
    let frame = match (anim.0, parity.0) {
        (Some((_, _, t)), true) if t < 0.5 => 1,
        (Some((_, _, t)), false) if t < 0.5 => 3,
        (Some(_), true) => 2,
        _ => 0,
    };
    sprite.image = assets.load(art(&format!("sprites/chars/player.{dir}.{frame}.png")));
}

// ----- P18 overworld feel systems ---------------------------------------

/// Leaving the overworld clears the transient feel-state: a spotted
/// beat must not outlive its scene (it would gate input forever —
/// spotted_tick only runs in Overworld), and queued rustles/door
/// flashes must not replay at stale coordinates after a battle or
/// whiteout.
fn clear_overworld_fx(mut spotted: ResMut<Spotted>, mut fxq: ResMut<FxQueue>) {
    spotted.0 = None;
    fxq.rustles.clear();
    fxq.door = None;
}

/// Spotted! — plays the alert, pops the "!" bubble over the trainer's
/// head, and counts the beat of pause (player_input holds meanwhile).
fn spotted_tick(
    mut commands: Commands,
    time: Res<Time>,
    world: Res<WorldRes>,
    assets: Res<AssetServer>,
    settings: Res<SettingsRes>,
    mut spotted: ResMut<Spotted>,
    bubbles: Query<Entity, With<SpottedBubble>>,
) {
    let Some((npc_id, remaining, fired)) = spotted.0.take() else {
        return;
    };
    if !fired {
        play_cue(&mut commands, &assets, &settings.0, "alert");
        let at = world
            .0
            .npcs
            .get(&world.0.current_map)
            .and_then(|list| list.iter().find(|n| n.id == npc_id))
            .map(|n| n.at);
        if let Some((x, y)) = at {
            commands.spawn((
                SpottedBubble,
                Sprite {
                    image: assets.load(art("sprites/fx/alert.png")),
                    custom_size: Some(Vec2::splat(16.0)),
                    ..default()
                },
                Transform::from_xyz(
                    x as f32 * TILE + TILE / 2.0,
                    y as f32 * TILE + 54.0,
                    4.0,
                ),
            ));
        }
    }
    let remaining = remaining - time.delta_secs();
    if remaining <= 0.0 {
        for entity in &bubbles {
            commands.entity(entity).despawn();
        }
    } else {
        spotted.0 = Some((npc_id, remaining, true));
    }
}

/// Drains the FxQueue (rustles, the arrival-door flash) and ticks the
/// live one-shot effects.
fn overworld_fx(
    mut commands: Commands,
    time: Res<Time>,
    assets: Res<AssetServer>,
    mut fxq: ResMut<FxQueue>,
    mut rustles: Query<(Entity, &mut RustleFx, &mut Sprite), Without<DoorFx>>,
    mut doors: Query<(Entity, &mut DoorFx), Without<RustleFx>>,
) {
    for (x, y) in fxq.rustles.drain(..) {
        commands.spawn((
            RustleFx(0.0),
            art_sprite(&assets, "sprites/fx/rustle.0.png", TILE),
            tile_pos(x, y, 1.6),
        ));
    }
    if let Some((x, y)) = fxq.door.take() {
        // Runs long enough to outlive the warp wipe.
        commands.spawn((
            DoorFx(0.0),
            art_sprite(&assets, "sprites/tiles/door_open.png", TILE),
            tile_pos(x, y, 1.1),
        ));
    }
    let dt = time.delta_secs();
    for (entity, mut fx, mut sprite) in &mut rustles {
        fx.0 += dt;
        let frame = ((fx.0 / 0.12) as usize).min(2);
        sprite.image = assets.load(art(&format!("sprites/fx/rustle.{frame}.png")));
        if fx.0 >= 0.36 {
            commands.entity(entity).despawn();
        }
    }
    for (entity, mut fx) in &mut doors {
        fx.0 += dt;
        if fx.0 >= 0.6 {
            commands.entity(entity).despawn();
        }
    }
}

/// Two-frame water shimmer: every water tile flips between the two
/// generated frames on a slow clock.
fn water_shimmer(
    time: Res<Time>,
    assets: Res<AssetServer>,
    mut phase: Local<bool>,
    mut tiles: Query<&mut Sprite, With<WaterTile>>,
) {
    let now = (time.elapsed_secs() % 1.4) < 0.7;
    if now == *phase {
        return;
    }
    *phase = now;
    let rel = if now {
        "sprites/tiles/water.1.png"
    } else {
        "sprites/tiles/water.png"
    };
    for mut sprite in &mut tiles {
        sprite.image = assets.load(art(rel));
    }
}

/// Window tiles brighten at night (the P18 night read: blue veil +
/// warm windows).
fn window_glow(world: Res<WorldRes>, mut tiles: Query<&mut Sprite, With<WindowTile>>) {
    let glow = if world.0.is_night() {
        Color::srgb(1.7, 1.5, 1.05)
    } else {
        Color::WHITE
    };
    for mut sprite in &mut tiles {
        if sprite.color != glow {
            sprite.color = glow;
        }
    }
}

/// Weather reads (P18): heatwave/dustchord get a tinted vignette,
/// flurry/downpour get drifting specks. Presenter-only, per map zone.
fn weather_fx(
    mut commands: Commands,
    world: Res<WorldRes>,
    assets: Res<AssetServer>,
    mut shown: Local<Option<(undersong_core::ids::MapId, bool)>>,
    existing: Query<Entity, With<WeatherFx>>,
) {
    use undersong_core::moves::WeatherKind;
    let weather = world.0.map().weather;
    let key = Some((world.0.current_map.clone(), weather.is_some()));
    // The cache alone is not enough: OnExit(Overworld) despawns the
    // overlay on every battle/menu — respawn when it should exist but
    // doesn't.
    if *shown == key && (weather.is_none() || !existing.is_empty()) {
        return;
    }
    *shown = key;
    for entity in &existing {
        commands.entity(entity).despawn();
    }
    let Some(kind) = weather else { return };
    match kind {
        WeatherKind::Heatwave | WeatherKind::Dustchord => {
            let tint = if kind == WeatherKind::Heatwave {
                Color::srgba(1.0, 0.55, 0.25, 0.5) // warm vignette
            } else {
                Color::srgba(0.8, 0.7, 0.4, 0.45) // dust haze
            };
            let mut vignette = ImageNode::new(assets.load(art("sprites/fx/vignette.png")));
            vignette.color = tint;
            commands.spawn((
                WeatherFx,
                vignette,
                GlobalZIndex(-2), // weather sits under the dialogue box
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    top: Val::Px(0.0),
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
            ));
        }
        WeatherKind::Flurry | WeatherKind::Downpour => {
            let (color, size) = if kind == WeatherKind::Flurry {
                (Color::srgba(1.0, 1.0, 1.0, 0.85), 3.0)
            } else {
                (Color::srgba(0.6, 0.7, 0.95, 0.7), 2.0)
            };
            for seed in 0..36u32 {
                commands.spawn((
                    WeatherFx,
                    FlurrySpeck(seed),
                    GlobalZIndex(-2),
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(0.0),
                        top: Val::Px(0.0),
                        width: Val::Px(size),
                        height: Val::Px(size),
                        ..default()
                    },
                    BackgroundColor(color),
                ));
            }
        }
    }
}

/// Drifts the weather specks down-left, wrapping over the 480×270 UI
/// canvas; flurry floats, downpour falls.
fn flurry_drift(
    time: Res<Time>,
    world: Res<WorldRes>,
    mut specks: Query<(&FlurrySpeck, &mut Node)>,
) {
    use undersong_core::moves::WeatherKind;
    let falling = world.0.map().weather == Some(WeatherKind::Downpour);
    let t = time.elapsed_secs();
    let (vx, vy) = if falling { (28.0, 180.0) } else { (14.0, 26.0) };
    for (speck, mut node) in &mut specks {
        let seed = speck.0 as f32;
        let x0 = (seed * 73.7) % 480.0;
        let y0 = (seed * 131.3) % 270.0;
        let sway = if falling {
            0.0
        } else {
            ((t * 1.3 + seed) * 0.7).sin() * 9.0
        };
        let x = (x0 - t * vx + sway).rem_euclid(480.0);
        let y = (y0 + t * vy).rem_euclid(270.0);
        node.left = Val::Px(x);
        node.top = Val::Px(y);
    }
}

// ----- camera ----------------------------------------------------------

fn camera_follow(
    world: Res<WorldRes>,
    player: Query<&Transform, (With<PlayerSprite>, Without<Camera2d>)>,
    mut camera: Query<&mut Transform, With<Camera2d>>,
) {
    let (Ok(player), Ok(mut camera)) = (player.single(), camera.single_mut()) else {
        return;
    };
    let map = world.0.map();
    let map_w = map.width as f32 * TILE;
    let map_h = map.height as f32 * TILE;
    let clamp_axis = |target: f32, map_size: f32, view: f32| {
        if map_size <= view {
            map_size / 2.0
        } else {
            target.clamp(view / 2.0, map_size - view / 2.0)
        }
    };
    camera.translation.x = clamp_axis(player.translation.x, map_w, VIEW_W);
    camera.translation.y = clamp_axis(player.translation.y, map_h, VIEW_H);
}

// ----- dialogue UI ------------------------------------------------------

/// Typewriter state for the overworld dialogue box (P17): one line
/// reveals per-character; Z completes the reveal, then advances.
#[derive(Resource, Default)]
pub struct DialogueReveal {
    /// The (speaker, string-key) pair the reveal belongs to.
    key: Option<(String, String)>,
    shown: f32,
    chars: usize,
}

impl DialogueReveal {
    pub fn done(&self) -> bool {
        self.key.is_none() || self.shown as usize >= self.chars
    }

    pub fn complete(&mut self) {
        self.shown = self.chars as f32;
    }
}

#[expect(clippy::too_many_arguments, reason = "bevy system parameters")]
fn dialogue_ui(
    mut commands: Commands,
    time: Res<Time>,
    assets: Res<AssetServer>,
    settings: Res<SettingsRes>,
    world: Res<WorldRes>,
    theme: Option<Res<Theme>>,
    spotted: Res<Spotted>,
    mut reveal: ResMut<DialogueReveal>,
    existing: Query<Entity, With<DialogueUi>>,
    mut text: Query<&mut Text, (With<DialogueText>, Without<NameTagText>)>,
    mut tag: Query<&mut Text, (With<NameTagText>, Without<DialogueText>)>,
) {
    let Some(theme) = theme else { return };
    // The spotted beat precedes the trainer's opening line — the box
    // waits for the bubble.
    if spotted.0.is_some() {
        return;
    }
    match &world.0.dialogue {
        Some(dialogue) => {
            let (who, key) = dialogue
                .current
                .clone()
                .unwrap_or((String::new(), String::new()));
            let speaker = if who == "narrator" {
                String::new()
            } else {
                world.0.text(&format!("npc.{who}"))
            };
            let full_line = world.0.text(&key).to_string();
            // Per-character reveal, keyed by the line — choice cursor
            // re-renders must not restart it.
            let line_key = (who.clone(), key.clone());
            if reveal.key.as_ref() != Some(&line_key) {
                reveal.key = Some(line_key);
                reveal.shown = 0.0;
                reveal.chars = full_line.chars().count();
            }
            let speed = f32::from(settings.0.text_speed);
            if speed <= 0.0 || dialogue.choice.is_some() {
                reveal.complete();
            } else if !reveal.done() {
                let before = reveal.shown as usize;
                reveal.shown =
                    (reveal.shown + speed * time.delta_secs()).min(reveal.chars as f32);
                // A soft blip every third character.
                if reveal.shown as usize / 3 > before / 3 {
                    play_cue_volume(&mut commands, &assets, &settings.0, "blip", 0.5);
                }
            }
            let line: String = full_line.chars().take(reveal.shown as usize).collect();
            // The choice list, when open, is appended to the text so the
            // pure cursor is visible; the dedicated popup widget arrives
            // with real fonts in P3 (doc 05 §4).
            let line = match &dialogue.choice {
                Some((_, options, cursor)) => {
                    let rendered: Vec<String> = options
                        .iter()
                        .enumerate()
                        .map(|(i, option)| {
                            let label = world.0.text(option);
                            if i == *cursor {
                                format!("> {label}")
                            } else {
                                format!("  {label}")
                            }
                        })
                        .collect();
                    format!("{line}\n{}", rendered.join("\n"))
                }
                None => line,
            };
            if existing.is_empty() {
                // Name tag: a gilt tab riding the box's top edge (P11).
                commands.spawn((
                    DialogueUi,
                    NameTagText,
                    Text::new(speaker.clone()),
                    TextFont::from_font_size(9.0),
                    TextColor(theme.color(&theme.palette.parchment)),
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(14.0),
                        bottom: Val::Px(64.0),
                        padding: UiRect::axes(Val::Px(8.0), Val::Px(3.0)),
                        ..default()
                    },
                    BackgroundColor(theme.color(&theme.palette.ink)),
                ));
                // Bottom-anchored parchment box (doc 05 §4): ink border,
                // four faint staff lines behind the text.
                commands
                    .spawn((
                        DialogueUi,
                        Node {
                            position_type: PositionType::Absolute,
                            left: Val::Px(4.0),
                            right: Val::Px(4.0),
                            bottom: Val::Px(4.0),
                            height: Val::Px(64.0),
                            padding: UiRect::all(Val::Px(2.0)),
                            ..default()
                        },
                        BackgroundColor(theme.color(&theme.palette.ink)),
                    ))
                    .with_children(|border| {
                        border
                            .spawn((
                                Node {
                                    width: Val::Percent(100.0),
                                    height: Val::Percent(100.0),
                                    padding: UiRect::all(Val::Px(8.0)),
                                    ..default()
                                },
                                BackgroundColor(theme.color(&theme.palette.parchment)),
                            ))
                            .with_children(|panel| {
                                // Four staff lines (doc 05 §4 motif).
                                for i in 0..4 {
                                    panel.spawn((
                                        Node {
                                            position_type: PositionType::Absolute,
                                            left: Val::Px(8.0),
                                            right: Val::Px(8.0),
                                            top: Val::Px(14.0 + i as f32 * 11.0),
                                            height: Val::Px(1.0),
                                            ..default()
                                        },
                                        BackgroundColor(theme.color(&theme.palette.parchment_dim)),
                                    ));
                                }
                                panel.spawn((
                                    DialogueText,
                                    Text::new(line.clone()),
                                    TextFont::from_font_size(9.0),
                                    TextColor(theme.color(&theme.palette.ink)),
                                ));
                            });
                    });
            } else {
                if let Ok(mut existing_text) = text.single_mut()
                    && existing_text.0 != line
                {
                    existing_text.0 = line;
                }
                if let Ok(mut tag_text) = tag.single_mut()
                    && tag_text.0 != speaker
                {
                    tag_text.0 = speaker;
                }
            }
        }
        None => {
            reveal.key = None;
            for entity in &existing {
                commands.entity(entity).despawn();
            }
        }
    }
}

// ----- shop (mart) panel -----------------------------------------------

#[derive(Component)]
struct ShopUi;

#[derive(Component)]
struct ShopText;

/// Renders the open mart and routes keys to the pure shop inputs.
/// Movement is already frozen by the core while a shop is open.
#[expect(clippy::too_many_arguments, reason = "bevy system parameters")]
fn shop_ui(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    theme: Option<Res<Theme>>,
    assets: Res<AssetServer>,
    settings: Res<SettingsRes>,
    mut world: ResMut<WorldRes>,
    existing: Query<Entity, With<ShopUi>>,
    mut text: Query<&mut Text, With<ShopText>>,
) {
    let Some(theme) = theme else { return };
    if world.0.shop.is_none() {
        for entity in &existing {
            commands.entity(entity).despawn();
        }
        return;
    }

    // Keys → pure inputs (with their cues).
    if keys.just_pressed(KeyCode::ArrowDown) {
        world.0.apply(WorldInput::ShopCursor(1));
        play_cue(&mut commands, &assets, &settings.0, "cursor");
    }
    if keys.just_pressed(KeyCode::ArrowUp) {
        world.0.apply(WorldInput::ShopCursor(-1));
        play_cue(&mut commands, &assets, &settings.0, "cursor");
    }
    if keys.just_pressed(KeyCode::KeyZ) || keys.just_pressed(KeyCode::Enter) {
        let events = world.0.apply(WorldInput::ShopBuy);
        let bought = events
            .iter()
            .any(|e| matches!(e, WorldEvent::ItemBought { .. }));
        play_cue(
            &mut commands,
            &assets,
            &settings.0,
            if bought { "buy" } else { "cancel" },
        );
    }
    if keys.just_pressed(KeyCode::KeyX) || keys.just_pressed(KeyCode::Escape) {
        world.0.apply(WorldInput::ShopClose);
        play_cue(&mut commands, &assets, &settings.0, "cancel");
        return;
    }

    let Some((stock, cursor)) = &world.0.shop else {
        return;
    };
    let mut lines = vec![format!(
        "COMMISSARY - {}c   (Z buy / X leave)",
        world.0.money
    )];
    for (index, item_id) in stock.iter().enumerate() {
        let price = world
            .0
            .registry
            .as_ref()
            .and_then(|r| r.items.get(item_id))
            .map(|d| d.price)
            .unwrap_or(0);
        let name = world.0.text(&format!("item.{item_id}"));
        let marker = if index == *cursor { ">" } else { " " };
        lines.push(format!("{marker} {name:<16} {price}c"));
    }
    let body = lines.join("\n");

    if existing.is_empty() {
        commands
            .spawn((
                ShopUi,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(40.0),
                    right: Val::Px(40.0),
                    top: Val::Px(16.0),
                    padding: UiRect::all(Val::Px(8.0)),
                    ..default()
                },
                BackgroundColor(theme.color(&theme.palette.parchment)),
            ))
            .with_child((
                ShopText,
                Text::new(body),
                TextFont::from_font_size(9.0),
                TextColor(theme.color(&theme.palette.ink)),
            ));
    } else if let Ok(mut existing_text) = text.single_mut()
        && existing_text.0 != body
    {
        existing_text.0 = body;
    }
}

// ----- warp wipe ---------------------------------------------------------

fn advance_wipe(
    mut commands: Commands,
    time: Res<Time>,
    mut wipe: ResMut<Wipe>,
    theme: Option<Res<Theme>>,
    bars: Query<Entity, With<WipeBar>>,
    mut nodes: Query<&mut Node, With<WipeBar>>,
) {
    let Some(theme) = theme else { return };
    let Some(mut t) = wipe.0 else {
        for entity in &bars {
            commands.entity(entity).despawn();
        }
        return;
    };
    t += time.delta_secs() / 0.3;
    if bars.is_empty() {
        // A vertical measure bar sweeping across (doc 05 §6, minimal P2
        // version: full-height bar whose width sweeps over the screen).
        commands.spawn((
            WipeBar,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                bottom: Val::Px(0.0),
                width: Val::Percent(0.0),
                ..default()
            },
            BackgroundColor(theme.color(&theme.palette.ink)),
        ));
    }
    if let Ok(mut node) = nodes.single_mut() {
        // 0..0.5 sweep in, 0.5..1 sweep out.
        let width = if t < 0.5 { t * 2.0 } else { (1.0 - t) * 2.0 };
        node.width = Val::Percent(width.clamp(0.0, 1.0) * 100.0);
    }
    wipe.0 = if t >= 1.0 { None } else { Some(t) };
}

// ----- pause menu ---------------------------------------------------------

const MENU_ROWS: [&str; 4] = ["Resume", "Party", "Save", "Settings"];

fn menu_open(mut commands: Commands, theme: Res<Theme>, mut cursor: ResMut<MenuCursor>) {
    cursor.0 = 0;
    commands
        .spawn((
            MenuUi,
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(8.0),
                top: Val::Px(8.0),
                width: Val::Px(140.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(2.0)),
                row_gap: Val::Px(2.0),
                ..default()
            },
            BackgroundColor(theme.color(&theme.palette.ink)),
        ))
        .with_children(|panel| {
            for (index, label) in MENU_ROWS.iter().enumerate() {
                panel
                    .spawn((
                        MenuRow(index),
                        Node {
                            padding: UiRect::all(Val::Px(6.0)),
                            ..default()
                        },
                        BackgroundColor(theme.color(&theme.palette.parchment)),
                    ))
                    .with_child((
                        Text::new(*label),
                        TextFont::from_font_size(9.0),
                        TextColor(theme.color(&theme.palette.ink)),
                    ));
            }
        });
}

#[expect(clippy::too_many_arguments, reason = "bevy system parameters")]
fn menu_input(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    theme: Res<Theme>,
    world: Res<WorldRes>,
    time: Res<Time>,
    assets: Res<AssetServer>,
    mut cursor: ResMut<MenuCursor>,
    mut settings: ResMut<SettingsRes>,
    mut settings_ui: ResMut<SettingsOpen>,
    mut next: ResMut<NextState<AppState>>,
    mut rows: Query<(&MenuRow, &mut BackgroundColor)>,
) {
    if keys.just_pressed(KeyCode::ArrowDown) {
        cursor.0 = (cursor.0 + 1) % MENU_ROWS.len();
        play_cue(&mut commands, &assets, &settings.0, "cursor");
    }
    if keys.just_pressed(KeyCode::ArrowUp) {
        cursor.0 = (cursor.0 + MENU_ROWS.len() - 1) % MENU_ROWS.len();
        play_cue(&mut commands, &assets, &settings.0, "cursor");
    }
    for (row, mut background) in &mut rows {
        let selected = row.0 == cursor.0;
        background.0 = if selected {
            theme.color(&theme.palette.gilt)
        } else {
            theme.color(&theme.palette.parchment)
        };
    }
    if keys.just_pressed(KeyCode::Escape) || keys.just_pressed(KeyCode::KeyX) {
        play_cue(&mut commands, &assets, &settings.0, "cancel");
        if settings_ui.0 {
            settings_ui.0 = false;
        } else {
            next.set(AppState::Overworld);
        }
        return;
    }
    if !settings_ui.0 && (keys.just_pressed(KeyCode::KeyZ) || keys.just_pressed(KeyCode::Enter)) {
        play_cue(&mut commands, &assets, &settings.0, "confirm");
        match cursor.0 {
            1 => {
                next.set(AppState::Dialogue); // the party screen state
            }
            2 => {
                // Manual save → slot 1 (slot picker arrives with the P3
                // save-select screen). Playtime is the session clock;
                // created stays 0 until the title flow stamps it.
                match platform_backend() {
                    Some(mut backend) => {
                        let mut snapshot = world.0.to_save("dev", time.elapsed_secs() as u64, 0);
                        snapshot.player.settings = settings.0.clone();
                        if let Err(error) = save::save(&mut backend, save::SlotId::Slot1, &snapshot)
                        {
                            bevy::log::error!("save failed: {error}");
                        }
                    }
                    None => bevy::log::warn!("no platform save directory; save skipped"),
                }
                next.set(AppState::Overworld);
            }
            3 => {
                settings_ui.0 = !settings_ui.0;
            }
            _ => next.set(AppState::Overworld),
        }
    }
    // Settings adjustments while the panel is open: Left/Right tweak the
    // row matching the cursor (cursor rows map 0/1/2+ regardless of the
    // menu's own row count).
    if settings_ui.0 {
        let delta: i32 = if keys.just_pressed(KeyCode::ArrowRight) {
            1
        } else if keys.just_pressed(KeyCode::ArrowLeft) {
            -1
        } else {
            0
        };
        if delta != 0 {
            match cursor.0 {
                0 => {
                    settings.0.text_speed = match (settings.0.text_speed, delta > 0) {
                        (30, true) => 60,
                        (60, true) => 0,
                        (0, true) => 30,
                        (30, false) => 0,
                        (60, false) => 30,
                        _ => 60,
                    };
                }
                1 => {
                    let volume = i32::from(settings.0.volume_music) + delta * 10;
                    settings.0.volume_music = u8::try_from(volume.clamp(0, 100)).expect("0..=100");
                }
                _ => {
                    let scale = i32::from(settings.0.screen_scale) + delta;
                    settings.0.screen_scale = u8::try_from(scale.clamp(1, 4)).expect("1..=4");
                }
            }
        }
    }
}

/// After a battle the world may have warped (whiteout to the rest
/// point): force a map rebuild and snap the player sprite to the
/// logical tile so the overworld resumes in sync.
fn resync_after_battle(
    world: Res<WorldRes>,
    mut rendered: ResMut<RenderedMap>,
    mut anim: ResMut<PlayerAnim>,
    mut player: Query<&mut Transform, With<PlayerSprite>>,
) {
    anim.0 = None;
    if rendered.0.as_ref() != Some(&world.0.current_map) {
        rendered.0 = None;
    }
    if let Ok(mut transform) = player.single_mut() {
        let (x, y) = world.0.player;
        transform.translation = Vec3::new(x as f32 * TILE + TILE / 2.0, y as f32 * TILE, 2.0);
    }
}

/// What should be playing right now; battle scenes override the map.
#[derive(Resource, Default)]
pub struct CurrentMusic {
    pub playing: Option<String>,
    /// Set by the battle scene; None = follow the map.
    pub override_track: Option<String>,
}

#[derive(Component)]
struct MusicPlayer;

/// One looping track at a time: map track in the overworld, the battle
/// scene's override elsewhere. Quiet Coast has None — dead air.
/// Browsers refuse autoplay until a user gesture: on wasm the director
/// stays silent until the first keypress (the title screen's "press
/// anything" doubles as the audio unlock).
#[derive(Resource, Default)]
pub struct AudioUnlocked(pub bool);

fn audio_unlock(keys: Res<ButtonInput<KeyCode>>, mut unlocked: ResMut<AudioUnlocked>) {
    if !unlocked.0 && keys.get_just_pressed().next().is_some() {
        unlocked.0 = true;
    }
}

/// One-shot UI cue (P9: the cue WAVs existed since P5 — nothing ever
/// played them). Despawns itself when done.
pub fn play_cue(
    commands: &mut Commands,
    assets: &AssetServer,
    settings: &save::Settings,
    name: &str,
) {
    play_cue_volume(commands, assets, settings, name, 1.0);
}

/// A cue with a per-call gain on top of the SFX setting (typewriter
/// blips ride softer than confirms).
pub fn play_cue_volume(
    commands: &mut Commands,
    assets: &AssetServer,
    settings: &save::Settings,
    name: &str,
    gain: f32,
) {
    if settings.volume_sfx == 0 {
        return;
    }
    let volume = f32::from(settings.volume_sfx) / 100.0 * 0.8 * gain;
    commands.spawn((
        AudioPlayer::new(assets.load(format!("sfx/{name}.wav"))),
        PlaybackSettings::DESPAWN.with_volume(bevy::audio::Volume::Linear(volume)),
    ));
}

/// F12 saves a screenshot (and `UNDERSONG_SHOT=path` auto-captures one
/// a few seconds after boot — README/STATUS evidence without a hand).
fn screenshot_key(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    mut auto_done: Local<bool>,
) {
    use bevy::render::view::screenshot::{Screenshot, save_to_disk};
    if keys.just_pressed(KeyCode::F12) {
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk("screenshot.png"));
    }
    if !*auto_done
        && time.elapsed_secs() > 6.0
        && let Some(path) = std::env::var_os("UNDERSONG_SHOT")
    {
        *auto_done = true;
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(std::path::PathBuf::from(path)));
    }
}

/// Dev rig: UNDERSONG_BOOT_BATTLE=<trainer_id> jumps straight from
/// the title into that fight (README battle shots without a hand on
/// the keys). Dev-only; does nothing unless the env var is set.
fn boot_battle_rig(
    time: Res<Time>,
    mut world: ResMut<WorldRes>,
    mut next: ResMut<NextState<AppState>>,
    mut done: Local<bool>,
) {
    if *done || time.elapsed_secs() < 2.0 {
        return;
    }
    let Some(trainer) = std::env::var_os("UNDERSONG_BOOT_BATTLE") else {
        return;
    };
    *done = true;
    let trainer = trainer.to_string_lossy().to_string();
    let mut rng = undersong_core::rng::BattleRng::from_seed(0xB007);
    let mut party = Vec::new();
    if let Some(registry) = &world.0.registry {
        for (species, level) in [("embaritone", 24), ("galliard", 22)] {
            if let Some(mut mote) = registry.wild_individual(&species.into(), level, &mut rng) {
                mote.ot = "player".into();
                party.push(mote);
            }
        }
    }
    world.0.party = party;
    world.0.start_trainer_battle(&trainer.as_str().into());
    if world.0.battle.is_some() {
        next.set(AppState::Battle);
    }
}

/// Dev rig: UNDERSONG_BOOT_MAP=<map_id>[,night] teleports a fresh
/// world onto that map (first walkable tile) — weather/night reads get
/// screenshot evidence without a save there. Dev-only.
fn boot_map_rig(
    time: Res<Time>,
    mut world: ResMut<WorldRes>,
    mut rendered: ResMut<RenderedMap>,
    mut next: ResMut<NextState<AppState>>,
    mut done: Local<bool>,
) {
    if *done || time.elapsed_secs() < 1.5 {
        return;
    }
    let Ok(spec) = std::env::var("UNDERSONG_BOOT_MAP") else {
        return;
    };
    *done = true;
    let (map_id, night) = match spec.split_once(',') {
        Some((id, "night")) => (id.to_string(), true),
        _ => (spec.clone(), false),
    };
    let id: undersong_core::ids::MapId = map_id.as_str().into();
    if !world.0.maps.contains_key(&id) {
        bevy::log::error!("boot map rig: unknown map {map_id}");
        return;
    }
    world.0.current_map = id;
    let map = world.0.map();
    let mut spot = (1, 1);
    'scan: for y in 1..map.height.saturating_sub(1) {
        for x in 1..map.width.saturating_sub(1) {
            // Walkable AND dry — water is non-solid (surf) but no
            // place to stand a screenshot rig.
            let ground = map.ground[map.index(x, y)];
            if !map.is_solid(x, y) && ground != 3 && ground != 5 {
                spot = (x, y);
                break 'scan;
            }
        }
    }
    world.0.player = spot;
    if night {
        world.0.clock_ticks = 900; // inside the night third (doc 02 v1.6 #5)
    }
    rendered.0 = None;
    next.set(AppState::Overworld);
}

/// Dev rig: UNDERSONG_BOOT_WALK="<map>:<x>,<y>:<udlr...>" teleports a
/// staged party onto a map and walks the pattern, filming frames to
/// docs/playtests/walk-<map>/ — the P18 spotted!/rustle/door evidence
/// instrument. Holds during the spotted beat exactly like a player,
/// advances dialogue, and exits a few beats after a battle opens.
#[derive(Default)]
struct WalkRigState {
    started: bool,
    step: usize,
    since_act: f32,
    since_shot: f32,
    shots: u32,
    battle_at: Option<f32>,
}

#[expect(clippy::too_many_arguments, reason = "bevy system parameters")]
fn walk_demo_rig(
    mut commands: Commands,
    time: Res<Time>,
    state: Res<State<AppState>>,
    mut world: ResMut<WorldRes>,
    mut next: ResMut<NextState<AppState>>,
    mut rendered: ResMut<RenderedMap>,
    mut anim: ResMut<PlayerAnim>,
    mut wipe: ResMut<Wipe>,
    mut toast: ResMut<Toast>,
    mut spotted: ResMut<Spotted>,
    mut fxq: ResMut<FxQueue>,
    mut parity: ResMut<StepParity>,
    mut player: Query<&mut Transform, With<PlayerSprite>>,
    mut exit: MessageWriter<AppExit>,
    mut rig: Local<WalkRigState>,
) {
    use bevy::render::view::screenshot::{Screenshot, save_to_disk};
    let Ok(spec) = std::env::var("UNDERSONG_BOOT_WALK") else {
        return;
    };
    if time.elapsed_secs() < 1.5 {
        return;
    }
    let mut parts = spec.splitn(3, ':');
    let (Some(map_id), Some(at), Some(pattern)) = (parts.next(), parts.next(), parts.next())
    else {
        return;
    };
    if !rig.started {
        rig.started = true;
        let dir = std::path::PathBuf::from("docs/playtests").join(format!("walk-{map_id}"));
        std::fs::remove_dir_all(&dir).ok();
        let mut rng = undersong_core::rng::BattleRng::from_seed(0x5A1C);
        if let Some(registry) = &world.0.registry
            && let Some(mut mote) = registry.wild_individual(&"embaritone".into(), 24, &mut rng)
        {
            mote.ot = "player".into();
            world.0.party = vec![mote];
        }
        let id: undersong_core::ids::MapId = map_id.into();
        if !world.0.maps.contains_key(&id) {
            bevy::log::error!("walk rig: unknown map {map_id}");
            exit.write(AppExit::error());
            return;
        }
        world.0.current_map = id;
        if let Some((x, y)) = at.split_once(',')
            && let (Ok(x), Ok(y)) = (x.parse(), y.parse())
        {
            world.0.player = (x, y);
        }
        rendered.0 = None;
        next.set(AppState::Overworld);
        return;
    }
    // Film continuously.
    rig.since_shot += time.delta_secs();
    if rig.since_shot > 0.3 && rig.shots < 80 {
        rig.since_shot = 0.0;
        rig.shots += 1;
        let dir = std::path::PathBuf::from("docs/playtests").join(format!("walk-{map_id}"));
        std::fs::create_dir_all(&dir).ok();
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(
                dir.join(format!("frame_{:03}.png", rig.shots)),
            ));
    }
    // A battle opened: film its entry for a few beats, then exit.
    if *state.get() == AppState::Battle || world.0.battle.is_some() {
        let opened = *rig.battle_at.get_or_insert(time.elapsed_secs());
        if time.elapsed_secs() - opened > 6.0 {
            exit.write(AppExit::Success);
        }
        return;
    }
    // The spotted beat holds everything, exactly like player_input.
    if spotted.0.is_some() {
        return;
    }
    rig.since_act += time.delta_secs();
    // Advance dialogue at a readable pace.
    if world.0.dialogue.is_some() {
        if rig.since_act > 0.8 {
            rig.since_act = 0.0;
            let events = world.0.apply(WorldInput::Interact);
            handle_events(
                &events,
                &mut world,
                &mut anim,
                &mut rendered,
                &mut wipe,
                &mut next,
                &mut player,
                &mut toast,
                &mut spotted,
                &mut fxq,
            );
        }
        return;
    }
    // Walk the pattern, one step at a stroll.
    if rig.since_act > 0.35 && anim.0.is_none() {
        rig.since_act = 0.0;
        let Some(ch) = pattern.chars().nth(rig.step) else {
            // Pattern done: linger so a pattern-final warp's door
            // flash still lands on film, then exit.
            let done = *rig.battle_at.get_or_insert(time.elapsed_secs());
            if time.elapsed_secs() - done > 1.5 {
                exit.write(AppExit::Success);
            }
            return;
        };
        rig.step += 1;
        let dir = match ch {
            'u' => Facing::Up,
            'l' => Facing::Left,
            'r' => Facing::Right,
            _ => Facing::Down,
        };
        let from = world.0.player;
        let events = world.0.apply(WorldInput::Step(dir));
        let stepped = events
            .iter()
            .any(|e| matches!(e, WorldEvent::Stepped { .. }));
        let warped = events
            .iter()
            .any(|e| matches!(e, WorldEvent::Warped { .. }));
        if stepped && !warped {
            let to = world.0.player;
            anim.0 = Some((
                Vec2::new(from.0 as f32 * TILE + TILE / 2.0, from.1 as f32 * TILE),
                Vec2::new(to.0 as f32 * TILE + TILE / 2.0, to.1 as f32 * TILE),
                0.0,
            ));
            parity.0 = !parity.0;
        }
        handle_events(
            &events,
            &mut world,
            &mut anim,
            &mut rendered,
            &mut wipe,
            &mut next,
            &mut player,
            &mut toast,
            &mut spotted,
            &mut fxq,
        );
    }
}

/// Dev rig: UNDERSONG_BOOT_THEATER={fight|catch|evolve} boots straight
/// into a staged wild battle and autoplays it while filming frames to
/// docs/playtests/theater-<mode>/ — the P17 battle-theater self-review
/// instrument. Exits the app when the show ends.
#[derive(Default)]
struct TheaterRigState {
    started: bool,
    bells: u32,
    linger: f32,
    shots: u32,
    since_shot: f32,
}

#[expect(clippy::too_many_arguments, reason = "bevy system parameters")]
fn theater_demo_rig(
    mut commands: Commands,
    time: Res<Time>,
    state: Res<State<AppState>>,
    mut world: ResMut<WorldRes>,
    mut next: ResMut<NextState<AppState>>,
    mut theater: ResMut<crate::battle_ui::Theater>,
    mut exit: MessageWriter<AppExit>,
    mut rig: Local<TheaterRigState>,
) {
    use bevy::render::view::screenshot::{Screenshot, save_to_disk};
    let Ok(mode) = std::env::var("UNDERSONG_BOOT_THEATER") else {
        return;
    };
    if time.elapsed_secs() < 2.0 {
        return;
    }
    if !rig.started {
        rig.started = true;
        // A fresh film: stale frames from a previous run must not
        // interleave with this one's.
        let dir = std::path::PathBuf::from("docs/playtests").join(format!("theater-{mode}"));
        std::fs::remove_dir_all(&dir).ok();
        let mut rng = undersong_core::rng::BattleRng::from_seed(0x7EA7E2);
        let Some(registry) = &world.0.registry else {
            bevy::log::error!("theater rig: no registry loaded");
            exit.write(AppExit::error());
            return;
        };
        let (ally, ally_level, foe, foe_level) = match mode.as_str() {
            "catch" => ("embaritone", 30, "galliard", 6),
            "evolve" => ("ampurr", 23, "bloomara", 4),
            _ => ("embaritone", 24, "galliard", 16),
        };
        let mut party = Vec::new();
        if let Some(mut mote) = registry.wild_individual(&ally.into(), ally_level, &mut rng) {
            mote.ot = "player".into();
            if mode == "evolve" {
                // Park exp a hair under the evolution level (ampurr →
                // voltacelle at 24) so one win triggers the scene.
                if let Some(spec) = registry.species.get(&mote.species) {
                    mote.exp = spec.growth_curve.total_exp(24).saturating_sub(10);
                }
            }
            party.push(mote);
        }
        world.0.party = party;
        if mode == "catch" {
            world.0.bag.insert("fermata".into(), 10);
        }
        world.0.start_wild_battle(foe.into(), foe_level);
        if world.0.battle.is_some() {
            next.set(AppState::Battle);
        } else {
            bevy::log::error!("theater rig: battle failed to start");
            exit.write(AppExit::error());
        }
        return;
    }
    // Film: a frame every ~0.6s while the show runs (the impact strips
    // run 0.48s — a slower cadence never catches them).
    rig.since_shot += time.delta_secs();
    if rig.since_shot > 0.6 && rig.shots < 240 {
        rig.since_shot = 0.0;
        rig.shots += 1;
        let dir = std::path::PathBuf::from("docs/playtests").join(format!("theater-{mode}"));
        std::fs::create_dir_all(&dir).ok();
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(
                dir.join(format!("frame_{:03}.png", rig.shots)),
            ));
    }
    // Autopilot: act only between theater beats, exactly like a player.
    if *state.get() != AppState::Battle || !theater.idle() {
        return;
    }
    if world.0.pending_shift {
        world.0.apply(WorldInput::Shift(None));
        return;
    }
    if !world.0.pending_learn_queue.is_empty() {
        let events = world.0.apply(WorldInput::Learn { replace: None });
        crate::battle_ui::stage_battle_events(&mut theater, &world.0, &events);
        return;
    }
    if !world.0.pending_evolutions.is_empty() {
        let events = world.0.apply(WorldInput::Evolve { accept: true });
        crate::battle_ui::stage_battle_events(&mut theater, &world.0, &events);
        return;
    }
    if world.0.battle.is_some() {
        let cmd = if mode == "catch" {
            rig.bells += 1;
            if rig.bells > 12 {
                game::session::BattleCmd::Run
            } else {
                game::session::BattleCmd::Bell
            }
        } else {
            game::session::BattleCmd::Move { slot: 0 }
        };
        let events = world.0.apply(WorldInput::Battle(cmd));
        crate::battle_ui::stage_battle_events(&mut theater, &world.0, &events);
        return;
    }
    // The show is over: linger so the film catches the last line.
    rig.linger += time.delta_secs();
    if rig.linger > 4.0 {
        exit.write(AppExit::Success);
    }
}

/// The visual playtest harness (P16): UNDERSONG_VISUAL_REPLAY=<file>
/// replays a recorded run inside the windowed app, pacing the input
/// stream and capturing a frame series to docs/playtests/<stem>/ —
/// the agent's eyes on motion it can't otherwise see.
#[derive(Default)]
struct VisualReplay {
    inputs: Option<Vec<game::world::Input>>,
    index: usize,
    shots: u32,
    since_shot: f32,
    /// Frame gate for UNDERSONG_REPLAY_SLOW (walk-speed overworld).
    slow_gate: u32,
}

#[expect(clippy::too_many_arguments, reason = "bevy system parameters")]
fn visual_replay(
    mut commands: Commands,
    time: Res<Time>,
    state: Res<State<AppState>>,
    mut world: ResMut<WorldRes>,
    mut next: ResMut<NextState<AppState>>,
    mut rendered: ResMut<RenderedMap>,
    mut anim: ResMut<PlayerAnim>,
    mut player: Query<&mut Transform, With<PlayerSprite>>,
    mut theater: ResMut<crate::battle_ui::Theater>,
    mut spotted: ResMut<Spotted>,
    mut parity: ResMut<StepParity>,
    mut fxq: ResMut<FxQueue>,
    mut rig: Local<VisualReplay>,
) {
    use bevy::render::view::screenshot::{Screenshot, save_to_disk};
    let Some(path) = std::env::var_os("UNDERSONG_VISUAL_REPLAY") else {
        return;
    };
    if time.elapsed_secs() < 2.0 {
        return;
    }
    let path = std::path::PathBuf::from(path);
    if rig.inputs.is_none() {
        let Ok(text) = std::fs::read_to_string(&path) else {
            return;
        };
        let stripped: String = text
            .lines()
            .filter(|l| !l.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join(
                "
",
            );
        let Ok(file) = ron::from_str::<game::replay::ReplayFile>(&stripped) else {
            bevy::log::warn!("visual replay: cannot parse {}", path.display());
            return;
        };
        if let Ok(reseeded) = load_game_world(std::path::Path::new("content"), file.seed) {
            world.0 = reseeded;
            rendered.0 = None;
        }
        rig.inputs = Some(file.inputs);
        if *state.get() == AppState::Title {
            next.set(AppState::Overworld);
        }
    }
    let total = rig.inputs.as_ref().map(Vec::len).unwrap_or(0);
    let finished = rig.index >= total;
    let in_battle_scene = *state.get() == AppState::Battle;
    // Battle scenes run on the theater's clock (P17): feed nothing
    // while it plays, one input per idle frame, and only drop the
    // curtain when the last animation has landed.
    if in_battle_scene && !theater.idle() {
        // hold — the film watches the theater
    } else if in_battle_scene
        && world.0.battle.is_none()
        && world.0.pending_learn_queue.is_empty()
        && world.0.pending_evolutions.is_empty()
    {
        next.set(AppState::Overworld); // the player's post-battle Z
    } else {
        // Pace: a small slice per frame keeps motion watchable.
        // UNDERSONG_REPLAY_SLOW=1 drops the overworld to ~walking speed
        // (one input every 8 frames) so steps, rustles, doors, and the
        // spotted beat land on film. The spotted beat holds the FEED
        // only — the camera keeps shooting so the bubble lands on film.
        let slow = std::env::var_os("UNDERSONG_REPLAY_SLOW").is_some();
        rig.slow_gate = (rig.slow_gate + 1) % 8;
        let hold =
            spotted.0.is_some() || (slow && !in_battle_scene && rig.slow_gate != 0);
        let per_frame = if hold {
            0
        } else if in_battle_scene || slow {
            1
        } else {
            4
        };
        let end = (rig.index + per_frame).min(total);
        let battle_now = world.0.battle.is_some();
        for i in rig.index..end {
            let _ = finished;
            let from = world.0.player;
            let input = rig.inputs.as_ref().expect("loaded")[i].clone();
            let events = world.0.apply(input);
            let battle_after = world.0.battle.is_some();
            // Battle-scene events drive the theater — including the
            // post-battle prompt answers (learn lines, the evolution
            // scene). Only the starting batch is skipped (battle_enter
            // resets the theater and seeds the entry choreography,
            // mirroring real play).
            if battle_now || in_battle_scene {
                crate::battle_ui::stage_battle_events(&mut theater, &world.0, &events);
            }
            let warped = events
                .iter()
                .any(|e| matches!(e, WorldEvent::Warped { .. }));
            for event in &events {
                match event {
                    WorldEvent::Warped { to, .. } => {
                        rendered.0 = None;
                        anim.0 = None;
                        if let Ok(mut transform) = player.single_mut() {
                            let (x, y) = world.0.player;
                            transform.translation =
                                Vec3::new(x as f32 * TILE + TILE / 2.0, y as f32 * TILE, 2.0);
                        }
                        if slow {
                            // The arrival-door flash, as in real play.
                            let map = world.0.map();
                            let (dx, dy) = world.0.facing.delta();
                            let behind = to
                                .0
                                .checked_add_signed(-dx)
                                .zip(to.1.checked_add_signed(-dy));
                            for spot in [Some(*to), behind].into_iter().flatten() {
                                if spot.0 < map.width
                                    && spot.1 < map.height
                                    && map.decor.get(map.index(spot.0, spot.1)) == Some(&12)
                                {
                                    fxq.door = Some(spot);
                                    break;
                                }
                            }
                        }
                    }
                    WorldEvent::Engaged { npc } => {
                        // Spotted! plays in films exactly like real play.
                        spotted.0 = Some((npc.clone(), 0.9, false));
                    }
                    WorldEvent::Stepped { to } if slow && !warped => {
                        // Slow films walk for real: slide + gait + rustle.
                        let to_world = *to;
                        anim.0 = Some((
                            Vec2::new(
                                from.0 as f32 * TILE + TILE / 2.0,
                                from.1 as f32 * TILE,
                            ),
                            Vec2::new(
                                to_world.0 as f32 * TILE + TILE / 2.0,
                                to_world.1 as f32 * TILE,
                            ),
                            0.0,
                        ));
                        parity.0 = !parity.0;
                        if world.0.map().is_patch(to_world.0, to_world.1) {
                            fxq.rustles.push(to_world);
                        }
                    }
                    _ => {}
                }
            }
            if battle_after && !battle_now {
                next.set(AppState::Battle);
                rig.index = i + 1;
                break; // let the scene switch render before continuing
            }
            rig.index = i + 1;
        }
    }
    // Frame series: one shot every ~2.5s (0.5s in slow mode — the
    // overworld touches are sub-second), capped; keeps shooting a few
    // beats after the run ends so the final scene lands on film.
    let cadence = if std::env::var_os("UNDERSONG_REPLAY_SLOW").is_some() {
        0.5
    } else {
        2.5
    };
    rig.since_shot += time.delta_secs();
    if rig.since_shot > cadence && rig.shots < 400 && (!finished || rig.shots < 3) {
        rig.since_shot = 0.0;
        rig.shots += 1;
        let stem = path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "run".into());
        let dir = std::path::PathBuf::from("docs/playtests").join(stem);
        std::fs::create_dir_all(&dir).ok();
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(
                dir.join(format!("frame_{:03}.png", rig.shots)),
            ));
    }
}

fn music_director(
    mut commands: Commands,
    world: Res<WorldRes>,
    settings: Res<SettingsRes>,
    assets: Res<AssetServer>,
    mut current: ResMut<CurrentMusic>,
    players: Query<Entity, With<MusicPlayer>>,
    unlocked: Res<AudioUnlocked>,
) {
    if cfg!(target_arch = "wasm32") && !unlocked.0 {
        return; // autoplay gate: wait for the first gesture
    }

    let desired = current
        .override_track
        .clone()
        .or_else(|| world.0.map().music.clone());
    if current.playing == desired {
        return;
    }
    for entity in &players {
        commands.entity(entity).despawn();
    }
    if let Some(track) = &desired {
        commands.spawn((
            MusicPlayer,
            AudioPlayer::new(assets.load(format!("music/{track}.wav"))),
            PlaybackSettings::LOOP.with_volume(bevy::audio::Volume::Linear(
                f32::from(settings.0.volume_music) / 100.0 * 0.8,
            )),
        ));
    }
    current.playing = desired;
}

/// The platform's save backend: filesystem natively, localStorage on
/// the web (doc 03 §4).
#[cfg(not(target_arch = "wasm32"))]
fn platform_backend() -> Option<save::FsBackend> {
    save::FsBackend::platform_default()
}

#[cfg(target_arch = "wasm32")]
fn platform_backend() -> Option<save::LocalStorageBackend> {
    Some(save::LocalStorageBackend)
}

/// Ending credits (doc 06 P6): full-screen roll per ending; the Da
/// Capo variant ignores input for its final 30 seconds — the player
/// sits with it, exactly as long as it sounds.
#[derive(Resource, Default)]
pub struct CreditsState {
    pub shown: bool,
    pub dead_input: f32,
}

#[derive(Component)]
struct CreditsUi;

fn credits_watch(
    mut commands: Commands,
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    theme: Option<Res<Theme>>,
    world: Res<WorldRes>,
    mut state: ResMut<CreditsState>,
    existing: Query<Entity, With<CreditsUi>>,
) {
    let Some(theme) = theme else { return };
    let ending = ["chorus", "dacapo", "tacet"]
        .into_iter()
        .find(|e| world.0.vars.flags.contains(&format!("credits.{e}")));
    let Some(ending) = ending else { return };

    if !state.shown {
        state.shown = true;
        state.dead_input = if ending == "dacapo" { 30.0 } else { 0.0 };
        let (title, lines) = match ending {
            "chorus" => (
                "THE CHORUS",
                "The song was never written for one voice.

UNDERSONG

every name on the Roster, read aloud
every Mote you ever attuned
you",
            ),
            "dacapo" => (
                "DA CAPO",
                "From the beginning.

UNDERSONG

the Roster gains a row
the music steadies
(the input does not respond - that is the point)",
            ),
            _ => (
                "TACET",
                "The rest is part of the music too.

UNDERSONG

the world, one voice quieter
Aria, free
the cost, posted later",
            ),
        };
        commands
            .spawn((
                CreditsUi,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    top: Val::Px(0.0),
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    row_gap: Val::Px(12.0),
                    ..default()
                },
                GlobalZIndex(50),
                BackgroundColor(theme.color(&theme.palette.ink)),
            ))
            .with_children(|root| {
                root.spawn((
                    Text::new(title),
                    TextFont::from_font_size(27.0),
                    TextColor(theme.color(&theme.palette.gilt)),
                ));
                root.spawn((
                    Text::new(lines),
                    TextFont::from_font_size(9.0),
                    TextColor(theme.color(&theme.palette.parchment)),
                ));
            });
        return;
    }
    if state.dead_input > 0.0 {
        state.dead_input -= time.delta_secs();
        return; // Da Capo: the held note doesn't care what you press.
    }
    if keys.just_pressed(KeyCode::KeyZ) && !existing.is_empty() {
        for entity in &existing {
            commands.entity(entity).despawn();
        }
    }
}

#[derive(Resource, Default)]
pub struct Toast {
    pub line: Option<String>,
    pub timer: f32,
}

#[derive(Component)]
struct ToastUi;

#[derive(Component)]
struct NightTint;

/// One-line transient messages (item used, performance, clock phase).
fn toast_ui(
    mut commands: Commands,
    time: Res<Time>,
    theme: Option<Res<Theme>>,
    mut toast: ResMut<Toast>,
    existing: Query<Entity, With<ToastUi>>,
    mut text: Query<&mut Text, With<ToastUi>>,
) {
    let Some(theme) = theme else { return };
    if let Some(line) = toast.line.clone() {
        toast.timer += time.delta_secs();
        if toast.timer > 2.2 {
            toast.line = None;
            toast.timer = 0.0;
            for entity in &existing {
                commands.entity(entity).despawn();
            }
            return;
        }
        if existing.is_empty() {
            commands.spawn((
                ToastUi,
                Text::new(line),
                TextFont::from_font_size(9.0),
                TextColor(theme.color(&theme.palette.parchment)),
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(8.0),
                    top: Val::Px(8.0),
                    padding: UiRect::all(Val::Px(4.0)),
                    ..default()
                },
                BackgroundColor(theme.color(&theme.palette.ink)),
            ));
        } else if let Ok(mut existing_text) = text.single_mut()
            && existing_text.0 != line
        {
            existing_text.0 = line;
        }
    } else {
        for entity in &existing {
            commands.entity(entity).despawn();
        }
    }
}

/// Night and dark-cave tint (doc 02 v1.6 #5, §11 Lumen Hum).
fn night_tint(
    mut commands: Commands,
    world: Res<WorldRes>,
    settings: Res<SettingsRes>,
    existing: Query<Entity, With<NightTint>>,
    mut tints: Query<&mut BackgroundColor, With<NightTint>>,
) {
    let dark_map = world.0.map().dark
        && !(world.0.vars.flags.contains("performance.lumen_hum")
            && world.0.party_has_tag("performer.light"));
    let wants = world.0.is_night() || dark_map;
    // High contrast lightens the veil so sprites stay readable.
    let mut alpha = if dark_map { 0.6 } else { 0.35 };
    if settings.0.high_contrast {
        alpha *= 0.6;
    }
    // Live-refresh: walking from night into a dark cave (and back)
    // retunes the alpha instead of keeping the spawn-time value.
    if wants && let Ok(mut color) = tints.single_mut() {
        let current = Color::srgba(0.04, 0.08, 0.30, alpha);
        if color.0 != current {
            color.0 = current;
        }
    }
    if wants && existing.is_empty() {
        commands.spawn((
            NightTint,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.04, 0.08, 0.30, alpha)),
            GlobalZIndex(5),
        ));
    } else if !wants {
        for entity in &existing {
            commands.entity(entity).despawn();
        }
    }
}

/// Writes the rotating autosave (doc 03 §4: map change & post-battle).
pub fn autosave(world: &WorldState) {
    let Some(mut backend) = platform_backend() else {
        bevy::log::warn!("no platform save directory; autosave skipped");
        return;
    };
    let snapshot = world.to_save("dev", 0, 0);
    if let Err(error) = save::save(&mut backend, save::SlotId::Auto, &snapshot) {
        bevy::log::error!("autosave failed: {error}");
    }
}

// ----- helpers --------------------------------------------------------------

fn cleanup_wipe(
    mut commands: Commands,
    mut wipe: ResMut<Wipe>,
    bars: Query<Entity, With<WipeBar>>,
) {
    wipe.0 = None;
    for entity in &bars {
        commands.entity(entity).despawn();
    }
}

pub fn despawn_tagged<T: Component>(mut commands: Commands, tagged: Query<Entity, With<T>>) {
    for entity in &tagged {
        commands.entity(entity).despawn();
    }
}

// ----- title & save select --------------------------------------------------

#[derive(Component)]
pub struct TitleUi;

#[derive(Component)]
struct TitleSlotRow;

fn title_open(mut commands: Commands, theme: Res<Theme>) {
    commands
        .spawn((
            TitleUi,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: Val::Px(10.0),
                ..default()
            },
            BackgroundColor(theme.color(&theme.palette.ink)),
        ))
        .with_children(|root| {
            // Five staff lines behind the wordmark (doc 05 §4 motif).
            for i in 0..5u8 {
                root.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Percent(12.0),
                        right: Val::Percent(12.0),
                        top: Val::Px(56.0 + f32::from(i) * 8.0),
                        height: Val::Px(1.0),
                        ..default()
                    },
                    BackgroundColor(theme.color(&theme.palette.parchment_dim).with_alpha(0.35)),
                ));
            }
            // A rising five-note phrase sitting on the staff.
            for (i, lift) in [0.0_f32, 8.0, 4.0, 16.0, 24.0].iter().enumerate() {
                root.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Percent(30.0 + i as f32 * 9.0),
                        top: Val::Px(82.0 - lift),
                        width: Val::Px(7.0),
                        height: Val::Px(6.0),
                        ..default()
                    },
                    BackgroundColor(theme.color(&theme.palette.gilt)),
                ));
            }
            root.spawn((
                Text::new("U N D E R S O N G"),
                TextFont::from_font_size(27.0),
                TextColor(theme.color(&theme.palette.gilt)),
            ));
            root.spawn((
                Text::new("the song holds, for now"),
                TextFont::from_font_size(9.0),
                TextColor(theme.color(&theme.palette.parchment_dim)),
            ));
            // Save select: continue (slot 1) when a save exists, else new.
            let has_save = platform_backend()
                .and_then(|backend| save::peek_header(&backend, save::SlotId::Slot1).ok())
                .flatten();
            let rows: Vec<String> = match &has_save {
                Some(header) => vec![
                    format!(
                        "Continue - {} badge(s), {}s played  (Z)",
                        header.badge_bits.count_ones(),
                        header.playtime_s
                    ),
                    "New Song  (N)".to_string(),
                ],
                None => vec!["New Song  (Z)".to_string()],
            };
            for label in &rows {
                root.spawn((
                    TitleSlotRow,
                    Text::new(label.clone()),
                    TextFont::from_font_size(9.0),
                    TextColor(theme.color(&theme.palette.parchment)),
                ));
            }
        });
}

fn title_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut world: ResMut<WorldRes>,
    mut settings: ResMut<SettingsRes>,
    mut next: ResMut<NextState<AppState>>,
) {
    let load = keys.just_pressed(KeyCode::KeyZ) || keys.just_pressed(KeyCode::Enter);
    // Dev rig: UNDERSONG_BOOT_CONTINUE walks past the title unattended
    // (screenshot runs).
    let auto = std::env::var_os("UNDERSONG_BOOT_CONTINUE").is_some()
        && std::env::var("UNDERSONG_BOOT_CONTINUE").as_deref() != Ok("fresh");
    let auto_fresh = std::env::var("UNDERSONG_BOOT_CONTINUE").as_deref() == Ok("fresh");
    let fresh = keys.just_pressed(KeyCode::KeyN) || auto_fresh;
    if !load && !fresh && !auto {
        return;
    }
    let load = (load || auto) && !auto_fresh;
    if load
        && let Some(backend) = platform_backend()
        && let Ok(Some(file)) = save::load(&backend, save::SlotId::Slot1)
    {
        settings.0 = file.player.settings.clone();
        world.0.restore(&file);
    } else {
        // A NEW song gets a fresh seed (app layer entropy — replays and
        // tests always pass explicit seeds, so determinism is intact).
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0x00D0_5EED);
        let reseeded = load_game_world(std::path::Path::new("content"), seed);
        if let Ok(reseeded) = reseeded {
            world.0 = reseeded;
        }
    }
    next.set(AppState::Overworld);
}
