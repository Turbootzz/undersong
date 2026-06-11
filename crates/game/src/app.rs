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
            .insert_resource(Toast::default())
            .insert_resource(CurrentMusic::default())
            .insert_resource(CreditsState::default())
            .insert_resource(AudioUnlocked::default())
            .add_systems(
                Update,
                (
                    audio_unlock,
                    music_director,
                    credits_watch,
                    screenshot_key,
                    boot_battle_rig,
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
                    despawn_tagged::<DialogueUi>,
                    despawn_tagged::<ToastUi>,
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

// ----- boot -----------------------------------------------------------

fn boot_load(mut commands: Commands, mut next: ResMut<NextState<AppState>>) {
    let content = std::path::Path::new("content");
    let palette = data::load_palette(content).expect("palette.ron must load");
    let world = load_game_world(content, 0x00D0_5EED).expect("game world must load");

    let zoom = 1.0 / WINDOW_SCALE as f32;
    commands.spawn((Camera2d, Transform::from_scale(Vec3::new(zoom, zoom, 1.0))));
    commands.insert_resource(Theme { palette });
    commands.insert_resource(WorldRes(world));
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
                "sprites/tiles/floor.png"
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
                commands.spawn((
                    MapTile,
                    art_sprite(&assets, ground_tile(ground), TILE),
                    tile_pos(x, y, 0.0),
                ));
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
                let rel = match decor {
                    5 => "sprites/tiles/bush.png",
                    6 => "sprites/tiles/sign.png",
                    _ => "sprites/tiles/bush.png",
                };
                commands.spawn((MapTile, art_sprite(&assets, rel, TILE), tile_pos(x, y, 1.0)));
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
            commands.spawn((
                NpcSprite(npc.id.clone()),
                art_sprite(&assets, &format!("sprites/chars/{key}.{dir}.0.png"), TILE),
                tile_pos(npc.at.0, npc.at.1, 2.0),
            ));
        }
    }

    // The player persists across maps; spawn once (with the facing
    // marker — P9: "am I looking at the NPC?" must answer itself).
    if player.is_empty() {
        commands.spawn((
            PlayerSprite,
            art_sprite(&assets, "sprites/chars/player.down.0.png", TILE),
            tile_pos(world.0.player.0, world.0.player.1, 2.0),
        ));
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
) {
    // An open mart owns the keys (shop_ui routes them).
    if world.0.shop.is_some() {
        return;
    }
    // Interact / advance dialogue / answer choice.
    if keys.just_pressed(KeyCode::KeyZ) || keys.just_pressed(KeyCode::Enter) {
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
            Vec2::new(
                from.0 as f32 * TILE + TILE / 2.0,
                from.1 as f32 * TILE + TILE / 2.0,
            ),
            Vec2::new(
                to.0 as f32 * TILE + TILE / 2.0,
                to.1 as f32 * TILE + TILE / 2.0,
            ),
            0.0,
        ));
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
            _ => {}
        }
    }
    for event in events {
        match event {
            WorldEvent::Warped { .. } => {
                rendered.0 = None; // forces a rebuild
                anim.0 = None;
                wipe.0 = Some(0.0);
                if let Ok(mut transform) = player.single_mut() {
                    let (x, y) = world.0.player;
                    transform.translation = Vec3::new(
                        x as f32 * TILE + TILE / 2.0,
                        y as f32 * TILE + TILE / 2.0,
                        2.0,
                    );
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
                    transform.translation = Vec3::new(
                        x as f32 * TILE + TILE / 2.0,
                        y as f32 * TILE + TILE / 2.0,
                        2.0,
                    );
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
    marker.translation = Vec3::new(player.translation.x + dx, player.translation.y + dy, 1.5);
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
                state.at.1 as f32 * TILE + TILE / 2.0,
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

/// The player's frame: facing × walk phase (frame 1 during the first
/// half of each slide — a two-beat gait).
fn animate_player_frame(
    world: Res<WorldRes>,
    assets: Res<AssetServer>,
    anim: Res<PlayerAnim>,
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
    let frame = match anim.0 {
        Some((_, _, t)) if t < 0.5 => 1,
        _ => 0,
    };
    sprite.image = assets.load(art(&format!("sprites/chars/player.{dir}.{frame}.png")));
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

fn dialogue_ui(
    mut commands: Commands,
    world: Res<WorldRes>,
    theme: Option<Res<Theme>>,
    existing: Query<Entity, With<DialogueUi>>,
    mut text: Query<&mut Text, (With<DialogueText>, Without<NameTagText>)>,
    mut tag: Query<&mut Text, (With<NameTagText>, Without<DialogueText>)>,
) {
    let Some(theme) = theme else { return };
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
            let line = world.0.text(&key).to_string();
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
                    TextFont::from_font_size(8.0),
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
                                    TextFont::from_font_size(8.0),
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
                TextFont::from_font_size(8.0),
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
                        TextFont::from_font_size(8.0),
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
        transform.translation = Vec3::new(
            x as f32 * TILE + TILE / 2.0,
            y as f32 * TILE + TILE / 2.0,
            2.0,
        );
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
    if settings.volume_sfx == 0 {
        return;
    }
    let volume = f32::from(settings.volume_sfx) / 100.0 * 0.8;
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
                    TextFont::from_font_size(24.0),
                    TextColor(theme.color(&theme.palette.gilt)),
                ));
                root.spawn((
                    Text::new(lines),
                    TextFont::from_font_size(8.0),
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
                TextFont::from_font_size(8.0),
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
        let current = Color::srgba(0.05, 0.07, 0.2, alpha);
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
            BackgroundColor(Color::srgba(0.05, 0.07, 0.2, alpha)),
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
                TextFont::from_font_size(24.0),
                TextColor(theme.color(&theme.palette.gilt)),
            ));
            root.spawn((
                Text::new("the song holds, for now"),
                TextFont::from_font_size(8.0),
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
                    TextFont::from_font_size(8.0),
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
    let fresh = keys.just_pressed(KeyCode::KeyN);
    if !load && !fresh {
        return;
    }
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
