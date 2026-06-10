//! The Bevy presentation layer: renders the pure `world` core and feeds
//! it inputs. No game rules live here (doc 03 §3) — tiles, actors, and
//! UI are colored quads and text until the sigil pipeline lands (P3+).

use bevy::prelude::*;
use bevy::ui::UiScale;
use data::Palette;
use game::world::{Input as WorldInput, WorldEvent, WorldState, load_dev_world};
use undersong_core::types::Type;
use undersong_core::world::Facing;

use crate::{AppState, WINDOW_SCALE};

const TILE: f32 = 16.0;
const VIEW_W: f32 = 480.0;
const VIEW_H: f32 = 270.0;
/// Walk interpolation (doc 02 §12: 150 ms per tile).
const WALK_SECONDS: f32 = 0.15;

pub struct UndersongPlugin;

impl Plugin for UndersongPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(UiScale(WINDOW_SCALE as f32))
            .insert_resource(RenderedMap(None))
            .insert_resource(PlayerAnim(None))
            .insert_resource(BufferedDir(None))
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
                    npc_wander,
                    sync_npc_sprites,
                    camera_follow,
                    dialogue_ui,
                    advance_wipe,
                )
                    .chain()
                    .run_if(in_state(AppState::Overworld)),
            )
            .add_systems(OnExit(AppState::Overworld), cleanup_wipe)
            .add_systems(OnEnter(AppState::Menu), menu_open)
            .add_systems(Update, menu_input.run_if(in_state(AppState::Menu)))
            .add_systems(OnExit(AppState::Menu), despawn_tagged::<MenuUi>)
            .add_systems(OnEnter(AppState::Battle), battle_open)
            .add_systems(Update, battle_input.run_if(in_state(AppState::Battle)))
            .add_systems(OnExit(AppState::Battle), despawn_tagged::<BattleUi>);
    }
}

// ----- resources & markers ------------------------------------------------

#[derive(Resource)]
struct WorldRes(WorldState);

#[derive(Resource)]
struct Theme {
    palette: Palette,
}

impl Theme {
    fn color(&self, hex: &str) -> Color {
        let parse = |s: &str| u8::from_str_radix(s, 16).unwrap_or(255);
        Color::srgb_u8(parse(&hex[1..3]), parse(&hex[3..5]), parse(&hex[5..7]))
    }

    fn ty(&self, ty: Type) -> Color {
        self.palette
            .type_colors
            .get(&ty)
            .map(|hex| self.color(hex))
            .unwrap_or(Color::WHITE)
    }
}

fn shade(color: Color, factor: f32) -> Color {
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

/// One-step input buffer (doc 03 §3): a tap during interpolation is
/// remembered and fired the frame the lerp completes.
#[derive(Resource)]
struct BufferedDir(Option<Facing>);

#[derive(Resource)]
struct WanderTimer(Timer);

#[derive(Resource)]
struct MenuCursor(usize);

/// Live settings (doc 05 §5 subset); persisted into saves.
#[derive(Resource)]
struct SettingsRes(save::Settings);

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
struct MenuUi;

#[derive(Component)]
struct MenuRow(usize);

#[derive(Component)]
struct BattleUi;

#[derive(Component)]
struct WipeBar;

// ----- boot -----------------------------------------------------------

fn boot_load(mut commands: Commands, mut next: ResMut<NextState<AppState>>) {
    let content = std::path::Path::new("content");
    let palette = data::load_palette(content).expect("palette.ron must load");
    let world = load_dev_world(content, 0x00D0_5EED).expect("dev world must load");

    let zoom = 1.0 / WINDOW_SCALE as f32;
    commands.spawn((Camera2d, Transform::from_scale(Vec3::new(zoom, zoom, 1.0))));
    commands.insert_resource(Theme { palette });
    commands.insert_resource(WorldRes(world));
    next.set(AppState::Overworld);
}

// ----- map rendering --------------------------------------------------

fn tile_pos(x: u32, y: u32, z: f32) -> Transform {
    Transform::from_xyz(
        x as f32 * TILE + TILE / 2.0,
        y as f32 * TILE + TILE / 2.0,
        z,
    )
}

fn quad(color: Color, size: f32) -> Sprite {
    Sprite {
        color,
        custom_size: Some(Vec2::splat(size)),
        ..default()
    }
}

fn rebuild_map_if_needed(
    mut commands: Commands,
    world: Res<WorldRes>,
    theme: Option<Res<Theme>>,
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
    let ground_color = |id: u16| match id {
        1 => shade(theme.ty(Type::Bloom), 1.25),        // grass
        2 => theme.color(&theme.palette.parchment_dim), // path
        3 => theme.ty(Type::Tide),                      // water
        4 => theme.color(&theme.palette.parchment),     // floor
        _ => theme.color(&theme.palette.ink_soft),
    };
    for y in 0..map.height {
        for x in 0..map.width {
            let index = map.index(x, y);
            let ground = map.ground[index];
            if ground != 0 {
                commands.spawn((
                    MapTile,
                    quad(ground_color(ground), TILE),
                    tile_pos(x, y, 0.0),
                ));
            }
            if map.is_patch(x, y) {
                let mut color = shade(theme.ty(Type::Bloom), 0.9);
                color = color.with_alpha(0.55);
                commands.spawn((MapTile, quad(color, TILE - 4.0), tile_pos(x, y, 0.5)));
            }
            if let Some(&decor) = map.decor.get(index)
                && decor != 0
            {
                let color = match decor {
                    5 => shade(theme.ty(Type::Bloom), 0.7), // bush
                    6 => theme.color(&theme.palette.gilt),  // sign
                    _ => theme.color(&theme.palette.ink_soft),
                };
                commands.spawn((MapTile, quad(color, TILE - 6.0), tile_pos(x, y, 1.0)));
            }
            if let Some(&overhang) = map.overhang.get(index)
                && overhang != 0
            {
                let color = shade(theme.ty(Type::Bloom), 0.5).with_alpha(0.85);
                commands.spawn((MapTile, quad(color, TILE), tile_pos(x, y, 3.0)));
            }
        }
    }

    // NPCs for this map.
    if let Some(states) = world.0.npcs.get(&world.0.current_map) {
        for npc in states {
            let color = match npc.sprite.as_str() {
                "npc.greeter" => theme.color(&theme.palette.cantorel_accent),
                _ => theme.color(&theme.palette.hp_mid),
            };
            commands.spawn((
                NpcSprite(npc.id.clone()),
                quad(color, TILE - 4.0),
                tile_pos(npc.at.0, npc.at.1, 2.0),
            ));
        }
    }

    // The player persists across maps; spawn once.
    if player.is_empty() {
        commands.spawn((
            PlayerSprite,
            quad(theme.color(&theme.palette.gilt), TILE - 4.0),
            tile_pos(world.0.player.0, world.0.player.1, 2.0),
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
    keys: Res<ButtonInput<KeyCode>>,
    mut world: ResMut<WorldRes>,
    mut anim: ResMut<PlayerAnim>,
    mut buffered: ResMut<BufferedDir>,
    mut rendered: ResMut<RenderedMap>,
    mut wipe: ResMut<Wipe>,
    mut next: ResMut<NextState<AppState>>,
    mut player: Query<&mut Transform, With<PlayerSprite>>,
) {
    // Interact / advance dialogue / answer choice.
    if keys.just_pressed(KeyCode::KeyZ) || keys.just_pressed(KeyCode::Enter) {
        let events = world.0.apply(WorldInput::Interact);
        handle_events(
            &events,
            &mut world,
            &mut anim,
            &mut rendered,
            &mut wipe,
            &mut next,
            &mut player,
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
    // Movement: buffer one step while interpolating (doc 03 §3).
    if anim.0.is_some() {
        if let Some(dir) = pressed_direction(&keys) {
            buffered.0 = Some(dir);
        }
        return;
    }
    let Some(dir) = buffered.0.take().or_else(|| pressed_direction(&keys)) else {
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
            Vec2::new(from.0 as f32 * TILE + 8.0, from.1 as f32 * TILE + 8.0),
            Vec2::new(to.0 as f32 * TILE + 8.0, to.1 as f32 * TILE + 8.0),
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
    );
}

fn handle_events(
    events: &[WorldEvent],
    world: &mut ResMut<WorldRes>,
    anim: &mut ResMut<PlayerAnim>,
    rendered: &mut ResMut<RenderedMap>,
    wipe: &mut ResMut<Wipe>,
    next: &mut ResMut<NextState<AppState>>,
    player: &mut Query<&mut Transform, With<PlayerSprite>>,
) {
    for event in events {
        match event {
            WorldEvent::Warped { .. } => {
                rendered.0 = None; // forces a rebuild
                anim.0 = None;
                wipe.0 = Some(0.0);
                if let Ok(mut transform) = player.single_mut() {
                    let (x, y) = world.0.player;
                    transform.translation =
                        Vec3::new(x as f32 * TILE + 8.0, y as f32 * TILE + 8.0, 2.0);
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
                        Vec3::new(x as f32 * TILE + 8.0, y as f32 * TILE + 8.0, 2.0);
                }
                next.set(AppState::Battle);
            }
            _ => {}
        }
    }
}

fn animate_player(
    time: Res<Time>,
    mut anim: ResMut<PlayerAnim>,
    mut player: Query<&mut Transform, With<PlayerSprite>>,
) {
    let Some((from, to, mut t)) = anim.0 else {
        return;
    };
    t = (t + time.delta_secs() / WALK_SECONDS).min(1.0);
    if let Ok(mut transform) = player.single_mut() {
        let p = from.lerp(to, t);
        transform.translation = Vec3::new(p.x, p.y, 2.0);
    }
    anim.0 = if t >= 1.0 { None } else { Some((from, to, t)) };
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

fn sync_npc_sprites(world: Res<WorldRes>, mut npcs: Query<(&NpcSprite, &mut Transform)>) {
    let Some(states) = world.0.npcs.get(&world.0.current_map) else {
        return;
    };
    for (marker, mut transform) in &mut npcs {
        if let Some(state) = states.iter().find(|n| n.id == marker.0) {
            transform.translation = Vec3::new(
                state.at.0 as f32 * TILE + 8.0,
                state.at.1 as f32 * TILE + 8.0,
                2.0,
            );
        }
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

fn dialogue_ui(
    mut commands: Commands,
    world: Res<WorldRes>,
    theme: Option<Res<Theme>>,
    existing: Query<Entity, With<DialogueUi>>,
    mut text: Query<&mut Text, With<DialogueText>>,
) {
    let Some(theme) = theme else { return };
    match &world.0.dialogue {
        Some(dialogue) => {
            let (who, key) = dialogue
                .current
                .clone()
                .unwrap_or((String::new(), String::new()));
            let line = format!("{who}: {key}");
            // The choice list, when open, is appended to the text so the
            // pure cursor is visible; the dedicated popup widget arrives
            // with real fonts in P3 (doc 05 §4).
            let line = match &dialogue.choice {
                Some((_, options, cursor)) => {
                    let rendered: Vec<String> = options
                        .iter()
                        .enumerate()
                        .map(|(i, option)| {
                            if i == *cursor {
                                format!("> {option}")
                            } else {
                                format!("  {option}")
                            }
                        })
                        .collect();
                    format!("{line}\n{}", rendered.join("\n"))
                }
                None => line,
            };
            if existing.is_empty() {
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
            } else if let Ok(mut existing_text) = text.single_mut()
                && existing_text.0 != line
            {
                existing_text.0 = line;
            }
        }
        None => {
            for entity in &existing {
                commands.entity(entity).despawn();
            }
        }
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

const MENU_ROWS: [&str; 3] = ["Resume", "Save", "Settings"];

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
    keys: Res<ButtonInput<KeyCode>>,
    theme: Res<Theme>,
    world: Res<WorldRes>,
    time: Res<Time>,
    mut cursor: ResMut<MenuCursor>,
    mut settings: ResMut<SettingsRes>,
    mut settings_ui: ResMut<SettingsOpen>,
    mut next: ResMut<NextState<AppState>>,
    mut rows: Query<(&MenuRow, &mut BackgroundColor)>,
) {
    if keys.just_pressed(KeyCode::ArrowDown) {
        cursor.0 = (cursor.0 + 1) % MENU_ROWS.len();
    }
    if keys.just_pressed(KeyCode::ArrowUp) {
        cursor.0 = (cursor.0 + MENU_ROWS.len() - 1) % MENU_ROWS.len();
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
        next.set(AppState::Overworld);
        return;
    }
    if keys.just_pressed(KeyCode::KeyZ) || keys.just_pressed(KeyCode::Enter) {
        match cursor.0 {
            1 => {
                // Manual save → slot 1 (slot picker arrives with the P3
                // save-select screen). Playtime is the session clock;
                // created stays 0 until the title flow stamps it.
                match save::FsBackend::platform_default() {
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
            2 => {
                settings_ui.0 = !settings_ui.0;
            }
            _ => next.set(AppState::Overworld),
        }
    }
    // Settings adjustments while the panel is open: Left/Right tweak the
    // row matching the cursor (text speed / music volume / scale).
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

// ----- battle placeholder + layout spike -----------------------------------

fn battle_open(mut commands: Commands, theme: Res<Theme>, world: Res<WorldRes>) {
    let (species, level) = world
        .0
        .pending_encounter
        .clone()
        .unwrap_or(("???".into(), 0));

    commands
        .spawn((
            BattleUi,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            BackgroundColor(theme.color(&theme.palette.parchment)),
        ))
        .with_children(|root| {
            // Foe plate, top-left (doc 05 §5): name, level, waveform HP
            // placeholder (12 bars at full amplitude).
            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(8.0),
                    top: Val::Px(8.0),
                    width: Val::Px(180.0),
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(Val::Px(6.0)),
                    row_gap: Val::Px(4.0),
                    ..default()
                },
                BackgroundColor(theme.color(&theme.palette.parchment_dim)),
            ))
            .with_children(|plate| {
                plate.spawn((
                    Text::new(format!("{species}  L{level}")),
                    TextFont::from_font_size(8.0),
                    TextColor(theme.color(&theme.palette.ink)),
                ));
                plate
                    .spawn(Node {
                        flex_direction: FlexDirection::Row,
                        column_gap: Val::Px(2.0),
                        height: Val::Px(14.0),
                        align_items: AlignItems::Center,
                        ..default()
                    })
                    .with_children(|wave| {
                        for i in 0..12 {
                            let height = 6.0 + 6.0 * (1.0 + (i as f32 * 0.9).sin()) / 2.0;
                            wave.spawn((
                                Node {
                                    width: Val::Px(3.0),
                                    height: Val::Px(height),
                                    ..default()
                                },
                                BackgroundColor(theme.color(&theme.palette.hp_high)),
                            ));
                        }
                    });
            });

            // Player plate, bottom-right.
            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    right: Val::Px(8.0),
                    bottom: Val::Px(80.0),
                    width: Val::Px(180.0),
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(Val::Px(6.0)),
                    row_gap: Val::Px(4.0),
                    ..default()
                },
                BackgroundColor(theme.color(&theme.palette.parchment_dim)),
            ))
            .with_children(|plate| {
                plate.spawn((
                    Text::new("your side (P3)"),
                    TextFont::from_font_size(8.0),
                    TextColor(theme.color(&theme.palette.ink_soft)),
                ));
                plate
                    .spawn(Node {
                        flex_direction: FlexDirection::Row,
                        column_gap: Val::Px(2.0),
                        height: Val::Px(14.0),
                        align_items: AlignItems::Center,
                        ..default()
                    })
                    .with_children(|wave| {
                        for i in 0..12 {
                            let height = 6.0 + 6.0 * (1.0 + (i as f32 * 1.3).sin()) / 2.0;
                            wave.spawn((
                                Node {
                                    width: Val::Px(3.0),
                                    height: Val::Px(height),
                                    ..default()
                                },
                                BackgroundColor(theme.color(&theme.palette.hp_mid)),
                            ));
                        }
                    });
            });

            // Move grid spike: 2×2 type-tinted buttons (doc 05 §5).
            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(8.0),
                    bottom: Val::Px(8.0),
                    width: Val::Px(280.0),
                    height: Val::Px(64.0),
                    flex_direction: FlexDirection::Row,
                    flex_wrap: FlexWrap::Wrap,
                    column_gap: Val::Px(4.0),
                    row_gap: Val::Px(4.0),
                    ..default()
                },
                BackgroundColor(theme.color(&theme.palette.ink)),
            ))
            .with_children(|grid| {
                for (label, ty) in [
                    ("tackle", Type::Feral),
                    ("ember_note", Type::Ember),
                    ("ripple", Type::Tide),
                    ("dampen", Type::Feral),
                ] {
                    grid.spawn((
                        Node {
                            width: Val::Px(134.0),
                            height: Val::Px(28.0),
                            padding: UiRect::all(Val::Px(4.0)),
                            ..default()
                        },
                        BackgroundColor(shade(theme.ty(ty), 1.6)),
                    ))
                    .with_child((
                        Text::new(label),
                        TextFont::from_font_size(8.0),
                        TextColor(theme.color(&theme.palette.ink)),
                    ));
                }
            });

            // Message line.
            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    right: Val::Px(8.0),
                    bottom: Val::Px(8.0),
                    padding: UiRect::all(Val::Px(6.0)),
                    ..default()
                },
                BackgroundColor(theme.color(&theme.palette.parchment_dim)),
            ))
            .with_child((
                Text::new("a wild mote hums! (Z: slip away)"),
                TextFont::from_font_size(8.0),
                TextColor(theme.color(&theme.palette.ink)),
            ));
        });
}

fn battle_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut world: ResMut<WorldRes>,
    mut next: ResMut<NextState<AppState>>,
) {
    if keys.just_pressed(KeyCode::KeyZ)
        || keys.just_pressed(KeyCode::KeyX)
        || keys.just_pressed(KeyCode::Escape)
    {
        world.0.pending_encounter = None;
        // Autosave post-battle (doc 03 §4).
        autosave(&world.0);
        next.set(AppState::Overworld);
    }
}

/// Writes the rotating autosave (doc 03 §4: map change & post-battle).
fn autosave(world: &WorldState) {
    let Some(mut backend) = save::FsBackend::platform_default() else {
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

fn despawn_tagged<T: Component>(mut commands: Commands, tagged: Query<Entity, With<T>>) {
    for entity in &tagged {
        commands.entity(entity).despawn();
    }
}
