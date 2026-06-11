//! The pause-hub screens (doc 06 P4): Party+Bag (with item use), Box
//! (Repertoire, 16 boxes, quick-move), Score (dex measure-fill),
//! Programme (badge case), Player Card, and Options. One Bevy state
//! (`AppState::Dialogue`, the menu hub) hosts them all behind a screen
//! selector; every mutation goes through pure world inputs.

use bevy::prelude::*;
use game::world::Input as WorldInput;

use crate::AppState;
use crate::app::{SettingsRes, Theme, WorldRes, despawn_tagged};

pub struct ScreensPlugin;

impl Plugin for ScreensPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ScreenState::default())
            .add_systems(OnEnter(AppState::Dialogue), screens_open)
            .add_systems(
                Update,
                (screens_input, screens_render, screens_visual)
                    .chain()
                    .run_if(in_state(AppState::Dialogue)),
            )
            .add_systems(OnExit(AppState::Dialogue), despawn_tagged::<ScreenUi>);
    }
}

#[derive(Component)]
pub struct ScreenUi;

#[derive(Component)]
struct ScreenText;

/// The right-hand visual pane (party rows, dex detail).
#[derive(Component)]
struct ScreenVisual;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Screen {
    #[default]
    Party,
    Boxes,
    Score,
    Programme,
    Card,
    Options,
}

const SCREENS: [Screen; 6] = [
    Screen::Party,
    Screen::Boxes,
    Screen::Score,
    Screen::Programme,
    Screen::Card,
    Screen::Options,
];

#[derive(Resource, Default)]
pub struct ScreenState {
    pub screen: Screen,
    /// Generic row cursor (party member / box slot / option row).
    pub cursor: usize,
    /// Party: bag-row cursor when in the bag half (Tab toggles).
    pub in_bag: bool,
    pub bag_cursor: usize,
    /// Box screen: current box page.
    pub box_page: usize,
    /// TM teach flow: picked bag item awaiting a move slot.
    pub pending_tm: Option<undersong_core::ids::ItemId>,
}

fn screens_open(mut commands: Commands, theme: Res<Theme>, mut state: ResMut<ScreenState>) {
    state.cursor = 0;
    state.in_bag = false;
    state.bag_cursor = 0;
    state.pending_tm = None;
    commands
        .spawn((
            ScreenUi,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(16.0),
                right: Val::Px(16.0),
                top: Val::Px(10.0),
                bottom: Val::Px(10.0),
                padding: UiRect::all(Val::Px(8.0)),
                border: UiRect::all(Val::Px(2.0)),
                column_gap: Val::Px(10.0),
                ..default()
            },
            BackgroundColor(theme.color(&theme.palette.parchment)),
            BorderColor::all(theme.color(&theme.palette.ink)),
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    width: Val::Percent(52.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                children![(
                    ScreenText,
                    Text::new(""),
                    TextFont::from_font_size(8.0),
                    TextColor(theme.color(&theme.palette.ink)),
                )],
            ));
            root.spawn((
                ScreenVisual,
                Node {
                    width: Val::Percent(46.0),
                    height: Val::Percent(100.0),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(4.0),
                    ..default()
                },
            ));
        });
}

fn screens_input(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    assets: Res<AssetServer>,
    mut state: ResMut<ScreenState>,
    mut world: ResMut<WorldRes>,
    mut settings: ResMut<SettingsRes>,
    mut next: ResMut<NextState<AppState>>,
) {
    if keys.just_pressed(KeyCode::Escape)
        || (keys.just_pressed(KeyCode::KeyX) && state.pending_tm.is_none())
    {
        crate::app::play_cue(&mut commands, &assets, &settings.0, "cancel");
        next.set(AppState::Overworld);
        return;
    }
    // Cursor + tab cues (one per frame is plenty).
    if keys.any_just_pressed([
        KeyCode::ArrowUp,
        KeyCode::ArrowDown,
        KeyCode::ArrowLeft,
        KeyCode::ArrowRight,
        KeyCode::KeyQ,
        KeyCode::KeyE,
    ]) {
        crate::app::play_cue(&mut commands, &assets, &settings.0, "cursor");
    }
    if keys.just_pressed(KeyCode::KeyZ) || keys.just_pressed(KeyCode::Enter) {
        crate::app::play_cue(&mut commands, &assets, &settings.0, "confirm");
    }
    // Q/E cycle screens.
    if keys.just_pressed(KeyCode::KeyQ) || keys.just_pressed(KeyCode::KeyE) {
        let index = SCREENS.iter().position(|s| *s == state.screen).unwrap_or(0);
        let next_index = if keys.just_pressed(KeyCode::KeyE) {
            (index + 1) % SCREENS.len()
        } else {
            (index + SCREENS.len() - 1) % SCREENS.len()
        };
        state.screen = SCREENS[next_index];
        state.cursor = 0;
        state.bag_cursor = 0;
        state.pending_tm = None;
        return;
    }
    let up = keys.just_pressed(KeyCode::ArrowUp);
    let down = keys.just_pressed(KeyCode::ArrowDown);
    let confirm = keys.just_pressed(KeyCode::KeyZ) || keys.just_pressed(KeyCode::Enter);

    match state.screen {
        Screen::Party => {
            // TM slot pick mode: cursor selects the move to replace.
            if let Some(item) = state.pending_tm.clone() {
                let member = world.0.party.get(state.cursor);
                let move_count = member.map(|m| m.moves.len()).unwrap_or(0);
                if down && state.bag_cursor + 1 < move_count {
                    state.bag_cursor += 1;
                }
                if up && state.bag_cursor > 0 {
                    state.bag_cursor -= 1;
                }
                if confirm {
                    let slot = u8::try_from(state.bag_cursor).unwrap_or(0);
                    world.0.apply(WorldInput::UseItem {
                        item,
                        target: u8::try_from(state.cursor).unwrap_or(0),
                        slot: Some(slot),
                    });
                    state.pending_tm = None;
                }
                if keys.just_pressed(KeyCode::KeyX) {
                    state.pending_tm = None;
                }
                return;
            }
            if keys.just_pressed(KeyCode::Tab) {
                state.in_bag = !state.in_bag;
            }
            if state.in_bag {
                let bag_len = world.0.bag.len();
                // Clamp after consumption shrank the bag.
                if state.bag_cursor >= bag_len && bag_len > 0 {
                    state.bag_cursor = bag_len - 1;
                }
                if down && state.bag_cursor + 1 < bag_len {
                    state.bag_cursor += 1;
                }
                if up && state.bag_cursor > 0 {
                    state.bag_cursor -= 1;
                }
                if confirm
                    && let Some((item, _)) = world
                        .0
                        .bag
                        .iter()
                        .nth(state.bag_cursor)
                        .map(|(k, v)| (k.clone(), *v))
                {
                    // TMs with a full moveset need a slot pick first.
                    let is_tm = world.0.registry.as_ref().is_some_and(|r| {
                        matches!(
                            r.items.get(&item).map(|d| &d.kind),
                            Some(data::ItemKind::Tm { .. })
                        )
                    });
                    let target_full = world
                        .0
                        .party
                        .get(state.cursor)
                        .map(|m| m.moves.len() >= 4)
                        .unwrap_or(false);
                    if is_tm && target_full {
                        state.pending_tm = Some(item);
                        state.bag_cursor = 0;
                    } else {
                        world.0.apply(WorldInput::UseItem {
                            item,
                            target: u8::try_from(state.cursor).unwrap_or(0),
                            slot: None,
                        });
                    }
                }
            } else {
                let party_len = world.0.party.len();
                if down && state.cursor + 1 < party_len {
                    state.cursor += 1;
                }
                if up && state.cursor > 0 {
                    state.cursor -= 1;
                }
            }
        }
        Screen::Boxes => {
            if keys.just_pressed(KeyCode::ArrowRight) {
                state.box_page = (state.box_page + 1) % 16;
            }
            if keys.just_pressed(KeyCode::ArrowLeft) {
                state.box_page = (state.box_page + 15) % 16;
            }
            // Quick-move (doc 06 P4): top rows = party (deposit), below =
            // this box page's residents (withdraw). One flat cursor.
            let party_len = world.0.party.len();
            let page = state.box_page;
            let residents: Vec<usize> = world
                .0
                .boxes
                .iter()
                .enumerate()
                .filter(|(i, _)| i / 30 == page)
                .map(|(i, _)| i)
                .collect();
            let total = party_len + residents.len();
            if down && state.cursor + 1 < total {
                state.cursor += 1;
            }
            if up && state.cursor > 0 {
                state.cursor -= 1;
            }
            if confirm && total > 0 {
                // All mutations route through pure inputs (replayable).
                if state.cursor < party_len {
                    world.0.apply(WorldInput::BoxDeposit {
                        party_index: u8::try_from(state.cursor).unwrap_or(0),
                    });
                    state.cursor = 0;
                } else if let Some(&box_index) = residents.get(state.cursor - party_len) {
                    world.0.apply(WorldInput::BoxWithdraw {
                        box_index: u32::try_from(box_index).unwrap_or(u32::MAX),
                    });
                    state.cursor = 0;
                }
            }
        }
        Screen::Options => {
            let rows = 7;
            if down && state.cursor + 1 < rows {
                state.cursor += 1;
            }
            if up && state.cursor > 0 {
                state.cursor -= 1;
            }
            let delta: i32 = i32::from(keys.just_pressed(KeyCode::ArrowRight))
                - i32::from(keys.just_pressed(KeyCode::ArrowLeft));
            if delta != 0 || confirm {
                match state.cursor {
                    0 => settings.0.set_mode = !settings.0.set_mode,
                    1 => settings.0.battle_animations = !settings.0.battle_animations,
                    2 => settings.0.reduced_motion = !settings.0.reduced_motion,
                    3 => settings.0.high_contrast = !settings.0.high_contrast,
                    4 => {
                        settings.0.text_speed = match settings.0.text_speed {
                            0 => 30,
                            30 => 60,
                            _ => 0,
                        };
                    }
                    5 => {
                        settings.0.battle_pace = (settings.0.battle_pace + 1) % 3;
                    }
                    _ => {
                        let volume = i32::from(settings.0.volume_music) + delta * 10;
                        settings.0.volume_music = u8::try_from(volume.clamp(0, 100)).unwrap_or(80);
                    }
                }
            }
        }
        // Read-only screens.
        Screen::Score | Screen::Programme | Screen::Card => {
            if down {
                state.cursor = state.cursor.saturating_add(1);
            }
            if up {
                state.cursor = state.cursor.saturating_sub(1);
            }
            if state.screen == Screen::Score {
                let dex_len = if world.0.primary_dex.is_empty() {
                    world
                        .0
                        .registry
                        .as_ref()
                        .map(|r| r.species.len())
                        .unwrap_or(0)
                } else {
                    world.0.primary_dex.len()
                };
                state.cursor = state.cursor.min(dex_len.saturating_sub(1));
                // Z: the cry — the Score is a songbook, after all.
                if keys.just_pressed(KeyCode::KeyZ) || keys.just_pressed(KeyCode::Enter) {
                    let dex: Vec<undersong_core::ids::SpeciesId> = if world.0.primary_dex.is_empty()
                    {
                        world
                            .0
                            .registry
                            .as_ref()
                            .map(|r| r.species.keys().cloned().collect())
                            .unwrap_or_default()
                    } else {
                        world.0.primary_dex.clone()
                    };
                    if let Some(species) = dex.get(state.cursor)
                        && world.0.vars.flags.contains(&format!("dex.seen.{species}"))
                    {
                        let volume = f32::from(settings.0.volume_sfx) / 100.0 * 0.8;
                        commands.spawn((
                            bevy::audio::AudioPlayer::new(
                                assets.load(format!("cries/{}/{}.wav", world.0.region_id, species)),
                            ),
                            bevy::audio::PlaybackSettings::DESPAWN
                                .with_volume(bevy::audio::Volume::Linear(volume)),
                        ));
                    }
                }
            }
        }
    }
}

fn screens_render(
    state: Res<ScreenState>,
    world: Res<WorldRes>,
    settings: Res<SettingsRes>,
    mut text: Query<&mut Text, With<ScreenText>>,
) {
    let Ok(mut target) = text.single_mut() else {
        return;
    };
    let world = &world.0;
    let mut lines: Vec<String> = vec![format!(
        "< Q   {}   E >      (X: close)",
        match state.screen {
            Screen::Party => "PARTY & BAG",
            Screen::Boxes => "REPERTOIRE",
            Screen::Score => "SCORE",
            Screen::Programme => "PROGRAMME",
            Screen::Card => "PLAYER CARD",
            Screen::Options => "OPTIONS",
        }
    )];

    match state.screen {
        Screen::Party => {
            for (index, member) in world.party.iter().enumerate() {
                let marker = if !state.in_bag && index == state.cursor {
                    ">"
                } else {
                    " "
                };
                let hp = member
                    .hp
                    .map(|hp| hp.to_string())
                    .unwrap_or_else(|| "full".into());
                lines.push(format!(
                    "{marker} {}  L{}  hp {}  ♥{}",
                    world.text(&format!("motif.{}", member.species)),
                    member.level,
                    hp,
                    member.friendship,
                ));
                if let Some(tm) = &state.pending_tm
                    && index == state.cursor
                {
                    lines.push(format!(
                        "   replace which move with {}?",
                        world.text(&format!("item.{tm}"))
                    ));
                    for (slot, learned) in member.moves.iter().enumerate() {
                        let pick = if slot == state.bag_cursor { "›" } else { " " };
                        lines.push(format!(
                            "   {pick} {}",
                            world.text(&format!("move.{}", learned.id))
                        ));
                    }
                }
            }
            lines.push(format!(
                "- BAG (Tab {}) - {}c",
                if state.in_bag { "→ party" } else { "→ bag" },
                world.money
            ));
            for (index, (item, count)) in world.bag.iter().enumerate() {
                let marker = if state.in_bag && index == state.bag_cursor {
                    ">"
                } else {
                    " "
                };
                let pocket = world
                    .registry
                    .as_ref()
                    .and_then(|r| r.items.get(item))
                    .map(|d| format!("{:?}", d.pocket))
                    .unwrap_or_default();
                lines.push(format!(
                    "{marker} {} ×{count}  [{pocket}]",
                    world.text(&format!("item.{item}"))
                ));
            }
        }
        Screen::Boxes => {
            lines.push(format!(
                "Box {} / 16   (arrow pages, Z: move)",
                state.box_page + 1
            ));
            lines.push("- party -".into());
            for (index, member) in world.party.iter().enumerate() {
                let marker = if index == state.cursor { ">" } else { " " };
                lines.push(format!(
                    "{marker} {}  L{}",
                    world.text(&format!("motif.{}", member.species)),
                    member.level
                ));
            }
            lines.push("- resting -".into());
            let party_len = world.party.len();
            for (offset, (box_index, member)) in world
                .boxes
                .iter()
                .enumerate()
                .filter(|(i, _)| i / 30 == state.box_page)
                .enumerate()
            {
                let _ = box_index;
                let marker = if party_len + offset == state.cursor {
                    ">"
                } else {
                    " "
                };
                lines.push(format!(
                    "{marker} {}  L{}",
                    world.text(&format!("motif.{}", member.species)),
                    member.level
                ));
            }
        }
        Screen::Score => {
            let registry = world.registry.as_ref();
            let dex: Vec<undersong_core::ids::SpeciesId> = if world.primary_dex.is_empty() {
                registry
                    .map(|r| r.species.keys().cloned().collect())
                    .unwrap_or_default()
            } else {
                world.primary_dex.clone()
            };
            let caught = dex
                .iter()
                .filter(|s| world.vars.flags.contains(&format!("dex.caught.{s}")))
                .count();
            let seen = dex
                .iter()
                .filter(|s| world.vars.flags.contains(&format!("dex.seen.{s}")))
                .count();
            lines.push(format!(
                "transcribed {caught} / heard {seen} / {}   ({}%)",
                dex.len(),
                caught * 100 / dex.len().max(1)
            ));
            lines.push(String::new());
            // A windowed list around the cursor (the panel shows detail).
            let window = 14usize;
            let lo = state.cursor.saturating_sub(window / 2);
            for (i, species) in dex.iter().enumerate().skip(lo).take(window) {
                let mark = if world.vars.flags.contains(&format!("dex.caught.{species}")) {
                    "*"
                } else if world.vars.flags.contains(&format!("dex.seen.{species}")) {
                    "o"
                } else {
                    "."
                };
                let name = if mark == "." {
                    "-----".to_string()
                } else {
                    world.text(&format!("motif.{species}")).to_string()
                };
                let cursor_mark = if i == state.cursor { ">" } else { " " };
                lines.push(format!("{cursor_mark} {:>3} {mark} {name}", i + 1));
            }
            lines.push(String::new());
            lines.push("(Z: hear its cry)".into());
        }
        Screen::Programme => {
            lines.push("the eight clefs of Cantorel".into());
            let names = [
                "French Violin",
                "Treble",
                "Soprano",
                "Mezzo",
                "Alto",
                "Tenor",
                "Baritone",
                "Bass",
            ];
            for (index, name) in names.iter().enumerate() {
                let earned = world.vars.flags.contains(&format!("badge.{}", index + 1));
                lines.push(format!("{} {name} clef", if earned { "*" } else { "." }));
            }
        }
        Screen::Card => {
            let badges = (1..=8u8)
                .filter(|n| world.vars.flags.contains(&format!("badge.{n}")))
                .count();
            let caught = world
                .vars
                .flags
                .iter()
                .filter(|f| f.starts_with("dex.caught."))
                .count();
            lines.push("conductor-in-training".into());
            lines.push(format!("{}c", world.money));
            lines.push(format!("badges: {badges}/8"));
            lines.push(format!("score: {caught} caught"));
            lines.push(format!("steps: {}", world.steps));
            lines.push(
                if world.is_night() {
                    "the night side of the song"
                } else {
                    "daylight tempo"
                }
                .to_string(),
            );
        }
        Screen::Options => {
            let rows = [
                format!(
                    "battle style: {}",
                    if settings.0.set_mode { "SET" } else { "SHIFT" }
                ),
                format!(
                    "battle animations: {}",
                    if settings.0.battle_animations {
                        "on"
                    } else {
                        "off"
                    }
                ),
                format!(
                    "reduced motion: {}",
                    if settings.0.reduced_motion {
                        "on"
                    } else {
                        "off"
                    }
                ),
                format!(
                    "high contrast: {}",
                    if settings.0.high_contrast {
                        "on"
                    } else {
                        "off"
                    }
                ),
                format!("text speed: {}", settings.0.text_speed),
                format!(
                    "battle pace: {}",
                    match settings.0.battle_pace {
                        0 => "relaxed",
                        1 => "standard",
                        _ => "brisk",
                    }
                ),
                format!("music volume: {}", settings.0.volume_music),
            ];
            for (index, row) in rows.iter().enumerate() {
                let marker = if index == state.cursor { ">" } else { " " };
                lines.push(format!("{marker} {row}"));
            }
        }
    }
    let body = lines.join("\n");
    if target.0 != body {
        target.0 = body;
    }
}

/// The right-hand pane: party rows with icons + HP bars, or the
/// Score-dex detail card for the cursored species (P15).
#[expect(clippy::too_many_lines, reason = "two visual layouts")]
fn screens_visual(
    mut commands: Commands,
    state: Res<ScreenState>,
    world: Res<WorldRes>,
    theme: Res<Theme>,
    assets: Res<AssetServer>,
    panel: Query<Entity, With<ScreenVisual>>,
    mut key: Local<String>,
) {
    let Ok(panel) = panel.single() else { return };
    let world = &world.0;
    // Cheap change key: rebuild only when content shifts.
    let new_key = match state.screen {
        Screen::Party => format!(
            "party:{}",
            world
                .party
                .iter()
                .map(|p| format!("{}:{}:{:?}", p.species, p.level, p.hp))
                .collect::<Vec<_>>()
                .join(",")
        ),
        Screen::Score => format!("score:{}", state.cursor),
        _ => "none".into(),
    };
    if *key == new_key {
        return;
    }
    *key = new_key;
    commands.entity(panel).despawn_related::<Children>();

    match state.screen {
        Screen::Party => {
            for member in &world.party {
                let hp = member.hp.unwrap_or(0);
                let max = world
                    .registry
                    .as_ref()
                    .and_then(|r| r.species.get(&member.species))
                    .map(|spec| {
                        battle::stats::compute_all(
                            &spec.base_stats,
                            &member.ivs,
                            &member.evs,
                            member.level,
                            member.nature,
                        )
                        .hp
                    })
                    .unwrap_or(hp.max(1));
                let frac = if max > 0 {
                    f32::from(hp.min(max)) / f32::from(max)
                } else {
                    0.0
                };
                let bar_color = if frac > 0.5 {
                    theme.color(&theme.palette.hp_high)
                } else if frac > 0.2 {
                    theme.color(&theme.palette.hp_mid)
                } else {
                    theme.color(&theme.palette.hp_low)
                };
                let icon = game::art::art(&format!(
                    "sprites/monsters/{}/{}.icon.png",
                    world.region_id, member.species
                ));
                let row = commands
                    .spawn((
                        Node {
                            width: Val::Percent(100.0),
                            height: Val::Px(26.0),
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(6.0),
                            padding: UiRect::all(Val::Px(3.0)),
                            border: UiRect::all(Val::Px(1.0)),
                            ..default()
                        },
                        BackgroundColor(theme.color(&theme.palette.parchment_dim)),
                        BorderColor::all(theme.color(&theme.palette.ink)),
                    ))
                    .id();
                let icon_node = commands
                    .spawn((
                        ImageNode::new(assets.load(icon)),
                        Node {
                            width: Val::Px(18.0),
                            height: Val::Px(18.0),
                            ..default()
                        },
                    ))
                    .id();
                let label = commands
                    .spawn((
                        Text::new(format!("{}  L{}", member.species, member.level)),
                        TextFont::from_font_size(8.0),
                        TextColor(theme.color(&theme.palette.ink)),
                        Node {
                            width: Val::Px(110.0),
                            ..default()
                        },
                    ))
                    .id();
                let bar_back = commands
                    .spawn((
                        Node {
                            width: Val::Px(70.0),
                            height: Val::Px(6.0),
                            ..default()
                        },
                        BackgroundColor(theme.color(&theme.palette.ink)),
                    ))
                    .id();
                let bar = commands
                    .spawn((
                        Node {
                            width: Val::Percent(frac * 100.0),
                            height: Val::Percent(100.0),
                            ..default()
                        },
                        BackgroundColor(bar_color),
                    ))
                    .id();
                let hp_text = commands
                    .spawn((
                        Text::new(format!("{hp}/{max}")),
                        TextFont::from_font_size(7.0),
                        TextColor(theme.color(&theme.palette.ink_soft)),
                    ))
                    .id();
                commands.entity(bar_back).add_child(bar);
                commands
                    .entity(row)
                    .add_children(&[icon_node, label, bar_back, hp_text]);
                commands.entity(panel).add_child(row);
            }
        }
        Screen::Score => {
            let Some(registry) = world.registry.as_ref() else {
                return;
            };
            let dex: Vec<_> = if world.primary_dex.is_empty() {
                registry.species.keys().cloned().collect()
            } else {
                world.primary_dex.clone()
            };
            let Some(species) = dex.get(state.cursor) else {
                return;
            };
            let caught = world.vars.flags.contains(&format!("dex.caught.{species}"));
            let seen = world.vars.flags.contains(&format!("dex.seen.{species}"));
            if !caught && !seen {
                let unknown = commands
                    .spawn((
                        Text::new("- unheard -"),
                        TextFont::from_font_size(10.0),
                        TextColor(theme.color(&theme.palette.ink_soft)),
                    ))
                    .id();
                commands.entity(panel).add_child(unknown);
                return;
            }
            let sprite = commands
                .spawn((
                    ImageNode::new(assets.load(game::art::art(&format!(
                        "sprites/monsters/{}/{}.front.png",
                        world.region_id, species
                    )))),
                    Node {
                        width: Val::Px(72.0),
                        height: Val::Px(72.0),
                        ..default()
                    },
                ))
                .id();
            let name = commands
                .spawn((
                    Text::new(format!(
                        "{}  {}",
                        world.text(&format!("motif.{species}")),
                        if caught {
                            "(in the Score)"
                        } else {
                            "(heard only)"
                        }
                    )),
                    TextFont::from_font_size(9.0),
                    TextColor(theme.color(&theme.palette.ink)),
                ))
                .id();
            commands.entity(panel).add_children(&[sprite, name]);
            if let Some(spec) = registry.species.get(species) {
                let types = spec
                    .types
                    .iter()
                    .map(|t| format!("{t:?}"))
                    .collect::<Vec<_>>()
                    .join(" / ");
                let type_row = commands
                    .spawn((
                        Text::new(types),
                        TextFont::from_font_size(8.0),
                        TextColor(theme.color(&theme.palette.gilt)),
                    ))
                    .id();
                commands.entity(panel).add_child(type_row);
                if caught {
                    for (label, value) in [
                        ("HP", spec.base_stats.hp),
                        ("ATK", spec.base_stats.atk),
                        ("DEF", spec.base_stats.def),
                        ("SPA", spec.base_stats.spa),
                        ("SPD", spec.base_stats.spd),
                        ("SPE", spec.base_stats.spe),
                    ] {
                        let row = commands
                            .spawn((Node {
                                width: Val::Percent(100.0),
                                height: Val::Px(9.0),
                                align_items: AlignItems::Center,
                                column_gap: Val::Px(4.0),
                                ..default()
                            },))
                            .id();
                        let tag = commands
                            .spawn((
                                Text::new(label),
                                TextFont::from_font_size(7.0),
                                TextColor(theme.color(&theme.palette.ink_soft)),
                                Node {
                                    width: Val::Px(26.0),
                                    ..default()
                                },
                            ))
                            .id();
                        let back = commands
                            .spawn((
                                Node {
                                    width: Val::Px(110.0),
                                    height: Val::Px(5.0),
                                    ..default()
                                },
                                BackgroundColor(theme.color(&theme.palette.ink)),
                            ))
                            .id();
                        let fill = commands
                            .spawn((
                                Node {
                                    width: Val::Percent(f32::from(value).min(120.0) / 1.2),
                                    height: Val::Percent(100.0),
                                    ..default()
                                },
                                BackgroundColor(theme.color(&theme.palette.gilt)),
                            ))
                            .id();
                        commands.entity(back).add_child(fill);
                        commands.entity(row).add_children(&[tag, back]);
                        commands.entity(panel).add_child(row);
                    }
                }
            }
            if caught {
                let entry = commands
                    .spawn((
                        Text::new(world.text(&format!("dex.{species}")).to_string()),
                        TextFont::from_font_size(7.0),
                        TextColor(theme.color(&theme.palette.ink)),
                    ))
                    .id();
                commands.entity(panel).add_child(entry);
            }
        }
        _ => {}
    }
}
