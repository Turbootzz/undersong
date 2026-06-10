//! The battle presenter: renders the session's battle state and the
//! event stream as paced messages (doc 03 §2: the Bevy layer is a
//! renderer of events; doc 05 §5 battle screen).

use bevy::prelude::*;
use game::session::BattleCmd;
use game::world::{Input as WorldInput, WorldEvent};

use crate::AppState;
use crate::app::{Theme, WorldRes, despawn_tagged, shade};

pub struct BattleUiPlugin;

impl Plugin for BattleUiPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(MessageQueue::default())
            .insert_resource(BattleCursor::default())
            .add_systems(OnEnter(AppState::Battle), battle_enter)
            .add_systems(
                Update,
                (pump_messages, battle_input, refresh_sprites, refresh_panels)
                    .chain()
                    .run_if(in_state(AppState::Battle)),
            )
            .add_systems(OnExit(AppState::Battle), despawn_tagged::<BattleUi>);
    }
}

#[derive(Component)]
pub struct BattleUi;

#[derive(Component)]
struct FoePlateText;

#[derive(Component)]
struct FoeHpBar;

#[derive(Component)]
struct PlayerPlateText;

#[derive(Component)]
struct PlayerHpBar;

#[derive(Component)]
struct MessageText;

#[derive(Component)]
struct CommandRow(usize);

#[derive(Component)]
struct FoeSprite;

#[derive(Component)]
struct PlayerSpriteImg;

/// Which species a battle sprite currently shows (switch detection).
#[derive(Component)]
struct ShownSpecies(undersong_core::ids::SpeciesId);

/// Paced battle text (doc 05 §6: everything skippable with confirm).
#[derive(Resource, Default)]
pub struct MessageQueue {
    pub lines: std::collections::VecDeque<String>,
    pub timer: f32,
    /// Set when this frame's Z popped a message — the input system must
    /// not also consume that same press.
    pub popped_this_frame: bool,
}

#[derive(Resource, Default)]
struct BattleCursor {
    /// 0 = root command row, 1 = move grid.
    mode: u8,
    index: usize,
}

const COMMANDS: [&str; 4] = ["Fight", "Bell", "Tonic", "Slip Away"];

pub fn queue_battle_events(
    queue: &mut MessageQueue,
    world: &game::world::WorldState,
    events: &[WorldEvent],
) {
    let name = |species: &undersong_core::ids::SpeciesId| world.text(&format!("motif.{species}"));
    let move_name = |move_id: &undersong_core::ids::MoveId| world.text(&format!("move.{move_id}"));
    for event in events {
        match event {
            WorldEvent::Battle(stream) => {
                for battle_event in stream {
                    use battle::BattleEvent as E;
                    let line = match battle_event {
                        E::MoveUsed { side, move_id, .. } => Some(format!(
                            "{} uses {}",
                            if *side == 0 { "you" } else { "foe" },
                            move_name(move_id)
                        )),
                        E::LastResortUsed { side } => Some(format!(
                            "{} resorts to a desperate hum",
                            if *side == 0 { "you" } else { "foe" }
                        )),
                        E::DamageDealt {
                            target,
                            amount,
                            crit,
                            effectiveness,
                            ..
                        } => {
                            let mut line = format!(
                                "{} takes {amount}",
                                if *target == 0 { "your mote" } else { "the foe" }
                            );
                            if *crit {
                                line.push_str(" — crit!");
                            }
                            match effectiveness {
                                undersong_core::types::Eff::Double => line.push_str(" (resonant!)"),
                                undersong_core::types::Eff::Half => line.push_str(" (dampened)"),
                                undersong_core::types::Eff::Zero => line.push_str(" (no effect)"),
                                undersong_core::types::Eff::Neutral => {}
                            }
                            Some(line)
                        }
                        E::StatusApplied { target, status, .. } => Some(format!(
                            "{} is {}!",
                            if *target == 0 { "your mote" } else { "the foe" },
                            world.text(&format!(
                                "ui.status.{}",
                                match status {
                                    undersong_core::moves::Ailment::Burn => "burn",
                                    undersong_core::moves::Ailment::Poison => "poison",
                                    undersong_core::moves::Ailment::Toxic => "toxic",
                                    undersong_core::moves::Ailment::Paralysis => "paralysis",
                                    undersong_core::moves::Ailment::Sleep => "sleep",
                                    undersong_core::moves::Ailment::Freeze => "freeze",
                                }
                            ))
                        )),
                        E::Fainted { target, .. } => Some(format!(
                            "{} faints!",
                            if *target == 0 { "your mote" } else { "the foe" }
                        )),
                        E::ExpGained { amount, .. } => Some(format!("gained {amount} exp")),
                        E::LeveledUp { level, .. } => Some(format!("level {level}!")),
                        E::AttuneAttempt { rings, caught } => Some(if *caught {
                            "♪ ♪ ♪ ♪ — the fermata settles!".to_string()
                        } else {
                            format!("{} — it shatters out!", "♪ ".repeat(usize::from(*rings)))
                        }),
                        E::EscapeAttempt { fled, .. } => Some(if *fled {
                            "slipped away!".into()
                        } else {
                            "can't escape!".into()
                        }),
                        E::MoveMissed { side } => {
                            Some(format!("{} misses", if *side == 0 { "you" } else { "foe" }))
                        }
                        _ => None,
                    };
                    if let Some(line) = line {
                        queue.lines.push_back(line);
                    }
                }
            }
            WorldEvent::MoteCaught { species } => {
                queue
                    .lines
                    .push_back(format!("{} joins your score!", name(species)));
            }
            WorldEvent::LearnPrompt { species, move_id } => {
                queue.lines.push_back(format!(
                    "{} wants to learn {} (Z: replace first move · X: skip)",
                    name(species),
                    move_name(move_id)
                ));
            }
            WorldEvent::MoveLearned { species, move_id } => {
                queue
                    .lines
                    .push_back(format!("{} learned {}", name(species), move_name(move_id)));
            }
            WorldEvent::EvolutionPrompt { from, into } => {
                queue.lines.push_back(format!(
                    "{}'s phrase is shifting toward {}… (Z: let it · X: hold it back)",
                    name(from),
                    name(into)
                ));
            }
            WorldEvent::Evolved { from, into } => {
                queue
                    .lines
                    .push_back(format!("{} became {}!", name(from), name(into)));
            }
            WorldEvent::Whiteout => {
                queue
                    .lines
                    .push_back("your motes fall silent… (half your ₵ lost)".into());
            }
            WorldEvent::MoneyChanged { money } => {
                queue.lines.push_back(format!("₵{money}"));
            }
            WorldEvent::ActionRejected { reason_key } => {
                queue.lines.push_back(world.text(reason_key));
            }
            _ => {}
        }
    }
}

fn battle_enter(
    mut commands: Commands,
    theme: Res<Theme>,
    world: Res<WorldRes>,
    assets: Res<AssetServer>,
    mut cursor: ResMut<BattleCursor>,
) {
    cursor.mode = 0;
    cursor.index = 0;
    let Some(session) = &world.0.battle else {
        return;
    };
    let foe = session.state.sides[1].active_mote();
    let us = session.state.sides[0].active_mote();

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
            // Foe sigil, upper right.
            root.spawn((
                FoeSprite,
                ShownSpecies(foe.species.clone()),
                ImageNode::new(assets.load(format!(
                    "sigils/{}/{}.front.png",
                    world.0.region_id, foe.species
                ))),
                Node {
                    position_type: PositionType::Absolute,
                    right: Val::Px(60.0),
                    top: Val::Px(24.0),
                    width: Val::Px(96.0),
                    height: Val::Px(96.0),
                    ..default()
                },
            ));
            // Player sigil (back), lower left.
            root.spawn((
                PlayerSpriteImg,
                ShownSpecies(us.species.clone()),
                ImageNode::new(assets.load(format!(
                    "sigils/{}/{}.back.png",
                    world.0.region_id, us.species
                ))),
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(48.0),
                    bottom: Val::Px(76.0),
                    width: Val::Px(96.0),
                    height: Val::Px(96.0),
                    ..default()
                },
            ));
            // Foe plate.
            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(8.0),
                    top: Val::Px(8.0),
                    width: Val::Px(170.0),
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(Val::Px(6.0)),
                    row_gap: Val::Px(3.0),
                    ..default()
                },
                BackgroundColor(theme.color(&theme.palette.parchment_dim)),
            ))
            .with_children(|plate| {
                plate.spawn((
                    FoePlateText,
                    Text::new(""),
                    TextFont::from_font_size(8.0),
                    TextColor(theme.color(&theme.palette.ink)),
                ));
                plate
                    .spawn((
                        Node {
                            width: Val::Percent(100.0),
                            height: Val::Px(5.0),
                            ..default()
                        },
                        BackgroundColor(theme.color(&theme.palette.ink)),
                    ))
                    .with_child((
                        FoeHpBar,
                        Node {
                            width: Val::Percent(100.0),
                            height: Val::Percent(100.0),
                            ..default()
                        },
                        BackgroundColor(theme.color(&theme.palette.hp_high)),
                    ));
            });
            // Player plate.
            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    right: Val::Px(8.0),
                    bottom: Val::Px(76.0),
                    width: Val::Px(170.0),
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(Val::Px(6.0)),
                    row_gap: Val::Px(3.0),
                    ..default()
                },
                BackgroundColor(theme.color(&theme.palette.parchment_dim)),
            ))
            .with_children(|plate| {
                plate.spawn((
                    PlayerPlateText,
                    Text::new(""),
                    TextFont::from_font_size(8.0),
                    TextColor(theme.color(&theme.palette.ink)),
                ));
                plate
                    .spawn((
                        Node {
                            width: Val::Percent(100.0),
                            height: Val::Px(5.0),
                            ..default()
                        },
                        BackgroundColor(theme.color(&theme.palette.ink)),
                    ))
                    .with_child((
                        PlayerHpBar,
                        Node {
                            width: Val::Percent(100.0),
                            height: Val::Percent(100.0),
                            ..default()
                        },
                        BackgroundColor(theme.color(&theme.palette.hp_high)),
                    ));
            });
            // Command / move row.
            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(4.0),
                    right: Val::Px(4.0),
                    bottom: Val::Px(34.0),
                    height: Val::Px(38.0),
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(4.0),
                    padding: UiRect::all(Val::Px(4.0)),
                    ..default()
                },
                BackgroundColor(theme.color(&theme.palette.ink)),
            ))
            .with_children(|row| {
                for (index, label) in COMMANDS.iter().enumerate() {
                    row.spawn((
                        CommandRow(index),
                        Node {
                            flex_grow: 1.0,
                            padding: UiRect::all(Val::Px(5.0)),
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
            // Message line.
            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(4.0),
                    right: Val::Px(4.0),
                    bottom: Val::Px(4.0),
                    height: Val::Px(26.0),
                    padding: UiRect::all(Val::Px(6.0)),
                    ..default()
                },
                BackgroundColor(theme.color(&theme.palette.parchment_dim)),
            ))
            .with_child((
                MessageText,
                Text::new(match session.context {
                    game::session::BattleContext::Wild { .. } => {
                        world.0.text("ui.battle.wild_intro")
                    }
                    game::session::BattleContext::Trainer { .. } => {
                        world.0.text("ui.battle.trainer_intro")
                    }
                }),
                TextFont::from_font_size(8.0),
                TextColor(theme.color(&theme.palette.ink)),
            ));
        });
}

/// Advances the message queue: each line shows ~0.5 s; Z dumps it.
fn pump_messages(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mut queue: ResMut<MessageQueue>,
    mut text: Query<&mut Text, With<MessageText>>,
) {
    queue.popped_this_frame = false;
    if queue.lines.is_empty() {
        return;
    }
    queue.timer += time.delta_secs();
    let skip = keys.just_pressed(KeyCode::KeyZ);
    if queue.timer >= 0.5 || skip {
        queue.timer = 0.0;
        if let Some(line) = queue.lines.pop_front()
            && let Ok(mut message) = text.single_mut()
        {
            message.0 = line;
        }
        if skip {
            queue.popped_this_frame = true;
        }
    }
}

#[expect(clippy::too_many_arguments, reason = "bevy system parameters")]
fn battle_input(
    keys: Res<ButtonInput<KeyCode>>,
    theme: Res<Theme>,
    mut world: ResMut<WorldRes>,
    mut queue: ResMut<MessageQueue>,
    mut cursor: ResMut<BattleCursor>,
    mut next: ResMut<NextState<AppState>>,
    mut rows: Query<(&CommandRow, &mut BackgroundColor)>,
    mut command_texts: Query<(&ChildOf, &mut Text)>,
) {
    // While messages are pending, only pumping happens (handled above);
    // and a Z that just popped a message must not double-fire here.
    if !queue.lines.is_empty() || queue.popped_this_frame {
        return;
    }
    // Prompts take priority: learn / evolution answered with Z/X.
    if !world.0.pending_learn_queue.is_empty() {
        if keys.just_pressed(KeyCode::KeyZ) {
            let events = world.0.apply(WorldInput::Learn { replace: Some(0) });
            queue_battle_events(&mut queue, &world.0, &events);
        } else if keys.just_pressed(KeyCode::KeyX) {
            let events = world.0.apply(WorldInput::Learn { replace: None });
            queue_battle_events(&mut queue, &world.0, &events);
        }
        return;
    }
    if !world.0.pending_evolutions.is_empty() {
        if keys.just_pressed(KeyCode::KeyZ) {
            let events = world.0.apply(WorldInput::Evolve { accept: true });
            queue_battle_events(&mut queue, &world.0, &events);
        } else if keys.just_pressed(KeyCode::KeyX) {
            let events = world.0.apply(WorldInput::Evolve { accept: false });
            queue_battle_events(&mut queue, &world.0, &events);
        }
        return;
    }
    // Battle over and nothing queued → wait for an explicit confirm so
    // the last line stays readable, then autosave (doc 03 §4
    // post-battle) and back to the overworld.
    if world.0.battle.is_none() {
        if keys.just_pressed(KeyCode::KeyZ) || keys.just_pressed(KeyCode::Enter) {
            crate::app::autosave(&world.0);
            next.set(AppState::Overworld);
        }
        return;
    }

    // Cursor movement.
    let limit = if cursor.mode == 0 {
        4
    } else {
        world
            .0
            .battle
            .as_ref()
            .map_or(1, |s| s.state.sides[0].active_mote().moves.len().max(1))
    };
    if keys.just_pressed(KeyCode::ArrowRight) {
        cursor.index = (cursor.index + 1) % limit;
    }
    if keys.just_pressed(KeyCode::ArrowLeft) {
        cursor.index = (cursor.index + limit - 1) % limit;
    }

    // Paint the row: labels follow the mode.
    if let Some(session) = &world.0.battle {
        for (row, mut background) in &mut rows {
            let selected = row.0 == cursor.index;
            background.0 = if selected {
                theme.color(&theme.palette.gilt)
            } else if cursor.mode == 1 {
                let mote = session.state.sides[0].active_mote();
                mote.moves
                    .get(row.0)
                    .map(|m| shade(theme.ty(m.spec.r#type), 1.5))
                    .unwrap_or(theme.color(&theme.palette.parchment_dim))
            } else {
                theme.color(&theme.palette.parchment)
            };
        }
        // Update labels (Fight row shows moves in mode 1).
        for (parent, mut text) in &mut command_texts {
            if let Ok((row, _)) = rows.get(parent.parent()) {
                text.0 = if cursor.mode == 1 {
                    session.state.sides[0]
                        .active_mote()
                        .moves
                        .get(row.0)
                        .map(|m| format!("{} {}/{}", m.spec.id, m.pp, m.spec.pp))
                        .unwrap_or_default()
                } else {
                    COMMANDS[row.0].to_string()
                };
            }
        }
    }

    // Confirm / cancel.
    if keys.just_pressed(KeyCode::KeyX) && cursor.mode == 1 {
        cursor.mode = 0;
        cursor.index = 0;
        return;
    }
    if keys.just_pressed(KeyCode::KeyZ) || keys.just_pressed(KeyCode::Enter) {
        let command = if cursor.mode == 0 {
            match cursor.index {
                0 => {
                    cursor.mode = 1;
                    cursor.index = 0;
                    return;
                }
                1 => Some(BattleCmd::Bell),
                2 => Some(BattleCmd::Item),
                _ => Some(BattleCmd::Run),
            }
        } else {
            Some(BattleCmd::Move {
                slot: u8::try_from(cursor.index).unwrap_or(0),
            })
        };
        if let Some(command) = command {
            let events = world.0.apply(WorldInput::Battle(command));
            queue_battle_events(&mut queue, &world.0, &events);
            cursor.mode = 0;
            cursor.index = 0;
        }
    }
}

/// Live plates: names, levels, HP bars (color by fraction per doc 05 §2).
type HpBarQuery<'w, 's, T, U> =
    Query<'w, 's, (&'static mut Node, &'static mut BackgroundColor), (With<T>, Without<U>)>;

#[expect(clippy::type_complexity, reason = "bevy query filters")]
fn refresh_sprites(
    world: Res<WorldRes>,
    assets: Res<AssetServer>,
    mut foe: Query<
        (&mut ShownSpecies, &mut ImageNode),
        (With<FoeSprite>, Without<PlayerSpriteImg>),
    >,
    mut player: Query<
        (&mut ShownSpecies, &mut ImageNode),
        (With<PlayerSpriteImg>, Without<FoeSprite>),
    >,
) {
    let Some(session) = &world.0.battle else {
        return;
    };
    if let Ok((mut shown, mut image)) = foe.single_mut() {
        let current = &session.state.sides[1].active_mote().species;
        if &shown.0 != current {
            shown.0 = current.clone();
            image.image = assets.load(format!(
                "sigils/{}/{}.front.png",
                world.0.region_id, current
            ));
        }
    }
    if let Ok((mut shown, mut image)) = player.single_mut() {
        let current = &session.state.sides[0].active_mote().species;
        if &shown.0 != current {
            shown.0 = current.clone();
            image.image = assets.load(format!("sigils/{}/{}.back.png", world.0.region_id, current));
        }
    }
}

fn refresh_panels(
    theme: Res<Theme>,
    world: Res<WorldRes>,
    mut foe_text: Query<&mut Text, (With<FoePlateText>, Without<PlayerPlateText>)>,
    mut player_text: Query<&mut Text, (With<PlayerPlateText>, Without<FoePlateText>)>,
    mut foe_bar: HpBarQuery<FoeHpBar, PlayerHpBar>,
    mut player_bar: HpBarQuery<PlayerHpBar, FoeHpBar>,
) {
    let Some(session) = &world.0.battle else {
        return;
    };
    let paint = |mote: &battle::BattleMote,
                 text: &mut Text,
                 bar: (&mut Node, &mut BackgroundColor),
                 theme: &Theme| {
        text.0 = format!(
            "{}  L{}  {}/{}",
            mote.species,
            mote.level,
            mote.hp,
            mote.max_hp()
        );
        let fraction = f32::from(mote.hp) / f32::from(mote.max_hp().max(1));
        bar.0.width = Val::Percent(fraction * 100.0);
        bar.1.0 = if fraction > 0.5 {
            theme.color(&theme.palette.hp_high)
        } else if fraction > 0.2 {
            theme.color(&theme.palette.hp_mid)
        } else {
            theme.color(&theme.palette.hp_low)
        };
    };
    if let (Ok(mut text), Ok((mut node, mut color))) = (foe_text.single_mut(), foe_bar.single_mut())
    {
        paint(
            session.state.sides[1].active_mote(),
            &mut text,
            (&mut node, &mut color),
            &theme,
        );
    }
    if let (Ok(mut text), Ok((mut node, mut color))) =
        (player_text.single_mut(), player_bar.single_mut())
    {
        paint(
            session.state.sides[0].active_mote(),
            &mut text,
            (&mut node, &mut color),
            &theme,
        );
    }
}
