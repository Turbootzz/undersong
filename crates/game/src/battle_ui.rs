//! The battle presenter: renders the session's battle state and the
//! event stream as a sequenced theater (doc 03 §2: the Bevy layer is a
//! renderer of events; doc 05 §5 battle screen; doc 06 P17).
//!
//! Events translate into a queue of `TheaterItem`s — typewriter lines
//! and blocking animations (entry slides, lunges, type-flavored
//! impacts, faints, switch-ins, the capture sequence, the evolution
//! scene). Nothing here feeds back into the core: animations read
//! events; they never produce inputs.

use std::collections::VecDeque;

use bevy::prelude::*;
use game::session::BattleCmd;
use game::world::{Input as WorldInput, WorldEvent};
use undersong_core::ids::SpeciesId;
use undersong_core::types::Type;

use crate::AppState;
use crate::app::{Theme, WorldRes, despawn_tagged, shade};

pub struct BattleUiPlugin;

impl Plugin for BattleUiPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Theater::default())
            .insert_resource(BattleCursor::default())
            .insert_resource(FxState::default())
            .insert_resource(DisplayedHp::default())
            .add_systems(Update, apply_fx.run_if(in_state(AppState::Battle)))
            .add_systems(OnEnter(AppState::Battle), battle_enter)
            .add_systems(
                Update,
                (
                    theater_tick,
                    battle_input,
                    command_pulse,
                    refresh_sprites,
                    hp_drain,
                    refresh_panels,
                )
                    .chain()
                    .run_if(in_state(AppState::Battle)),
            )
            .add_systems(
                OnExit(AppState::Battle),
                (despawn_tagged::<BattleUi>, clear_battle_music, clear_theater),
            );
    }
}

#[derive(Component)]
pub struct BattleUi;

/// The one full-screen container (the shake target). The FxSprite and
/// the evolution overlay are also BattleUi-tagged top-level entities,
/// so apply_fx must not identify the root by BattleUi alone.
#[derive(Component)]
struct BattleRoot;

fn clear_battle_music(mut music: ResMut<crate::app::CurrentMusic>) {
    music.override_track = None;
}

fn clear_theater(mut theater: ResMut<Theater>, mut disp: ResMut<DisplayedHp>) {
    *theater = Theater::default();
    *disp = DisplayedHp::default();
}

#[derive(Component)]
struct FoePlateText;

#[derive(Component)]
struct FoeHpBar;

#[derive(Component)]
struct PlayerPlateText;

#[derive(Component)]
struct PlayerExpBar;

#[derive(Component)]
struct PlayerHpBar;

#[derive(Component)]
struct MessageText;

#[derive(Component)]
struct CommandRow(usize);

/// The little glyph leading a command label (hidden in move mode —
/// the cells hold move names there).
#[derive(Component)]
struct CommandIcon;

#[derive(Component)]
struct FoeSprite;

#[derive(Component)]
struct PlayerSpriteImg;

/// The foe-side HP plate root (entry slide target).
#[derive(Component)]
struct FoePlate;

/// The player-side HP plate root (entry slide target).
#[derive(Component)]
struct PlayerPlate;

/// A transient move-impact effect sprite (frames swap, then despawn).
#[derive(Component)]
struct FxSprite;

/// The evolution-scene overlay sprite (flash alternation target).
#[derive(Component)]
struct EvoSprite;

/// Which species a battle sprite currently shows (switch detection).
#[derive(Component)]
struct ShownSpecies(undersong_core::ids::SpeciesId);

// Stage geometry (the Gen-3 layout, P11).
const FOE_RIGHT: f32 = 60.0;
const FOE_TOP: f32 = 24.0;
const ALLY_LEFT: f32 = 48.0;
const ALLY_BOTTOM: f32 = 76.0;
const SPRITE_SIZE: f32 = 96.0;
const FOE_PLATE_LEFT: f32 = 8.0;
const PLAYER_PLATE_RIGHT: f32 = 8.0;
const OFFSCREEN: f32 = -200.0;

/// One step of the battle theater: a typewriter line or a blocking
/// animation. Items play strictly in order; input waits for idle.
pub enum TheaterItem {
    Line(String),
    Anim(Anim),
}

/// The animation vocabulary (doc 06 P17). All presenter-side.
pub enum Anim {
    /// Foe slides in from the right with its cry; ally back lobs in
    /// from the left; plates slide in after (the "fade" of the roadmap,
    /// rendered as the era's plate slide).
    Entry { foe: SpeciesId },
    /// Attacker nudges toward the target.
    Lunge { side: u8 },
    /// Type-flavored impact on the target: effect frames + flash +
    /// shake (+ crit freeze), and the HP drain to `hp_to`.
    Impact {
        target: u8,
        ty: Type,
        crit: bool,
        hp_to: f32,
    },
    /// An unnarrated HP move (status tick, recoil, heal): drain only.
    HpSet { side: u8, to: f32 },
    /// Sprite drops and fades with a low cue.
    Faint { side: u8 },
    /// New mote slides in with its cry; displayed HP snaps to it.
    SwitchIn {
        side: u8,
        species: SpeciesId,
        hp: f32,
        max: f32,
        level: u8,
    },
    /// Bell ring → shrink to a gilt point → wobble pulses → settle
    /// chime or break-out.
    Capture { rings: u8, caught: bool },
    /// The dedicated overlay scene; X fast-forwards to the resolve.
    Evolve { from: SpeciesId, into: SpeciesId },
    /// Swaps the looping music override at its narrative moment (the
    /// victory jingle after the final faint resolves).
    Jingle { track: String },
}

struct ActiveAnim {
    kind: Anim,
    t: f32,
    spawned: Option<Entity>,
    /// One-shot cue/cry/frame bookkeeping (bitmask).
    fired: u32,
}

/// Paced battle theater (doc 05 §6: everything skippable with confirm).
#[derive(Resource, Default)]
pub struct Theater {
    items: VecDeque<TheaterItem>,
    active: Option<ActiveAnim>,
    /// The current message line and its typewriter reveal.
    line: String,
    chars: usize,
    shown: usize,
    reveal_acc: f32,
    dwell: f32,
    /// Set when this frame's Z/X was consumed by the theater — the
    /// input system must not also consume that same press.
    pub popped_this_frame: bool,
    /// Translation-time HP simulation per side (pts, max) so sequenced
    /// drains land with their impacts, not at stream arrival.
    sim: [(f32, f32); 2],
}

impl Theater {
    /// Input is allowed only when nothing is queued, playing, or
    /// still revealing.
    pub fn idle(&self) -> bool {
        self.items.is_empty() && self.active.is_none() && self.shown >= self.chars
    }

    pub fn push_line(&mut self, line: String) {
        self.items.push_back(TheaterItem::Line(line));
    }

    fn push_anim(&mut self, anim: Anim) {
        self.items.push_back(TheaterItem::Anim(anim));
    }

    fn set_line(&mut self, line: String) {
        self.chars = line.chars().count();
        self.line = line;
        self.shown = 0;
        self.reveal_acc = 0.0;
        self.dwell = 0.0;
    }
}

/// What the HP bars currently show, per side: (current, target, max)
/// in points. Drains move current toward target; the catch-all sync
/// (theater idle) retargets to live state so drift self-heals.
#[derive(Resource, Default)]
pub struct DisplayedHp {
    sides: [Option<(f32, f32, f32)>; 2],
    /// "species  Llevel" per side — the plates paint from this once
    /// the session is gone (the killing blow's drain must still show).
    labels: [Option<String>; 2],
}

impl DisplayedHp {
    fn drained(&self, side: u8) -> bool {
        self.sides[usize::from(side)]
            .map(|(cur, target, _)| (cur - target).abs() < 0.5)
            .unwrap_or(true)
    }

    fn set_target(&mut self, side: u8, to: f32) {
        if let Some((_, target, _)) = &mut self.sides[usize::from(side)] {
            *target = to;
        }
    }

    fn snap(&mut self, side: u8, hp: f32, max: f32) {
        self.sides[usize::from(side)] = Some((hp, hp, max));
    }

    fn set_max(&mut self, side: u8, to: f32) {
        if let Some((_, _, max)) = &mut self.sides[usize::from(side)] {
            *max = to;
        }
    }

    fn set_label(&mut self, side: u8, species: &SpeciesId, level: u8) {
        let label = format!("{species}  L{level}");
        if self.labels[usize::from(side)].as_ref() != Some(&label) {
            self.labels[usize::from(side)] = Some(label);
        }
    }
}

/// Move-impact presentation: shake + hit-stop + flash (doc 06 P5
/// polish pass; reduced-motion and anim-toggle aware).
#[derive(Resource, Default)]
pub struct FxState {
    /// Remaining shake seconds (decays; amplitude follows it).
    pub shake: f32,
    /// Remaining flash seconds over the struck side (side, t).
    pub flash: Option<(u8, f32)>,
    /// Hit-stop: theater pause remaining (longer on crits — the
    /// freeze-frame).
    pub hit_stop: f32,
}

#[derive(Resource, Default)]
struct BattleCursor {
    /// 0 = root command row, 1 = move grid.
    mode: u8,
    index: usize,
}

const COMMANDS: [&str; 4] = ["Fight", "Bell", "Tonic", "Slip Away"];

/// Translates a world event batch into theater items: lines in the
/// existing voice, animations interleaved at their narrative moment.
pub fn stage_battle_events(
    theater: &mut Theater,
    world: &game::world::WorldState,
    events: &[WorldEvent],
) {
    let name = |species: &undersong_core::ids::SpeciesId| world.text(&format!("motif.{species}"));
    let move_name = |move_id: &undersong_core::ids::MoveId| world.text(&format!("move.{move_id}"));

    // Pass 1 — reconcile unmodeled HP changes (bag heals are eventless:
    // the foe's same-turn hit must not stage against pre-heal HP). Walk
    // the batch's narrated deltas back from live end-state to what the
    // batch starts from; if the sim disagrees, drain there first.
    // Switches/faints make the ledger opaque — skip those sides (the
    // idle catch-all sync still self-heals any drift).
    if let Some(session) = &world.battle {
        let live_end = [
            f32::from(session.state.sides[0].active_mote().hp),
            f32::from(session.state.sides[1].active_mote().hp),
        ];
        let mut delta = [0.0f32; 2];
        let mut opaque = [false; 2];
        for event in events {
            let WorldEvent::Battle(stream) = event else {
                continue;
            };
            for battle_event in stream {
                use battle::BattleEvent as E;
                match battle_event {
                    E::DamageDealt {
                        target,
                        target_slot: 0,
                        amount,
                        ..
                    } => delta[usize::from(*target)] -= f32::from(*amount),
                    E::StatusTicked { target, damage, .. } => {
                        delta[usize::from(*target)] -= f32::from(*damage);
                    }
                    E::HurtItselfInConfusion { side, damage } => {
                        delta[usize::from(*side)] -= f32::from(*damage);
                    }
                    E::Recoiled {
                        side,
                        slot: 0,
                        amount,
                    } => delta[usize::from(*side)] -= f32::from(*amount),
                    E::WeatherChip { target, amount } => {
                        delta[usize::from(*target)] -= f32::from(*amount);
                    }
                    E::Healed {
                        target,
                        slot: 0,
                        amount,
                    } => delta[usize::from(*target)] += f32::from(*amount),
                    E::Drained { from, amount } => {
                        delta[usize::from(1 - *from)] += f32::from(*amount);
                    }
                    E::SeededDrain { from, amount } => {
                        delta[usize::from(*from)] -= f32::from(*amount);
                        delta[usize::from(1 - *from)] += f32::from(*amount);
                    }
                    E::SwitchedIn { side, .. } => opaque[usize::from(*side)] = true,
                    E::Fainted { target, .. } => opaque[usize::from(*target)] = true,
                    _ => {}
                }
            }
        }
        for side in 0..2u8 {
            let i = usize::from(side);
            if !opaque[i] {
                let start = (live_end[i] - delta[i]).clamp(0.0, theater.sim[i].1);
                if (start - theater.sim[i].0).abs() > 0.5 {
                    theater.sim[i].0 = start;
                    theater.push_anim(Anim::HpSet { side, to: start });
                }
            }
        }
    }

    // Pass 2 — translate, in event order. Only position 0 is staged
    // (the windowed presenter renders one sprite/plate per side; the
    // doubles stage remains the deferred P5 deviation) — other slots
    // keep their narration lines but drive no animation. The Shift
    // offer is held to the tail so it reads after the KO it answers.
    let mut current_ty: Option<Type> = None;
    let mut shift_offered = false;
    for event in events {
        match event {
            WorldEvent::Battle(stream) => {
                for battle_event in stream {
                    use battle::BattleEvent as E;
                    match battle_event {
                        E::MoveUsed {
                            side,
                            slot,
                            move_id,
                        } => {
                            current_ty = world
                                .registry
                                .as_ref()
                                .and_then(|r| r.moves.get(move_id))
                                .map(|spec| spec.r#type);
                            theater.push_line(format!(
                                "{} uses {}",
                                if *side == 0 { "you" } else { "foe" },
                                move_name(move_id)
                            ));
                            if *slot == 0 {
                                theater.push_anim(Anim::Lunge { side: *side });
                            }
                        }
                        E::LastResortUsed { side } => {
                            current_ty = Some(Type::Resonant);
                            theater.push_line(format!(
                                "{} resorts to a desperate hum",
                                if *side == 0 { "you" } else { "foe" }
                            ));
                            theater.push_anim(Anim::Lunge { side: *side });
                        }
                        E::DamageDealt {
                            target,
                            target_slot,
                            amount,
                            crit,
                            effectiveness,
                        } => {
                            // Blanked hits (Eff::Zero, amount 0) get the
                            // line only — no flash for a move that did
                            // nothing (P5 contract).
                            if *target_slot == 0 && *amount > 0 {
                                let sim = &mut theater.sim[usize::from(*target)];
                                sim.0 = (sim.0 - f32::from(*amount)).max(0.0);
                                let hp_to = sim.0;
                                theater.push_anim(Anim::Impact {
                                    target: *target,
                                    ty: current_ty.unwrap_or(Type::Feral),
                                    crit: *crit,
                                    hp_to,
                                });
                            }
                            let mut line = format!(
                                "{} takes {amount}",
                                if *target == 0 { "your mote" } else { "the foe" }
                            );
                            if *crit {
                                line.push_str(" - crit!");
                            }
                            match effectiveness {
                                undersong_core::types::Eff::Double => line.push_str(" (resonant!)"),
                                undersong_core::types::Eff::Half => line.push_str(" (dampened)"),
                                undersong_core::types::Eff::Zero => line.push_str(" (no effect)"),
                                undersong_core::types::Eff::Neutral => {}
                            }
                            theater.push_line(line);
                        }
                        E::StatusApplied { target, status, .. } => {
                            theater.push_line(format!(
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
                            ));
                        }
                        E::StatusTicked { target, damage, .. } => {
                            let sim = &mut theater.sim[usize::from(*target)];
                            sim.0 = (sim.0 - f32::from(*damage)).max(0.0);
                            let to = sim.0;
                            theater.push_anim(Anim::HpSet { side: *target, to });
                        }
                        E::HurtItselfInConfusion { side, damage } => {
                            let sim = &mut theater.sim[usize::from(*side)];
                            sim.0 = (sim.0 - f32::from(*damage)).max(0.0);
                            let to = sim.0;
                            theater.push_anim(Anim::HpSet { side: *side, to });
                        }
                        E::Recoiled {
                            side,
                            slot: 0,
                            amount,
                        } => {
                            let sim = &mut theater.sim[usize::from(*side)];
                            sim.0 = (sim.0 - f32::from(*amount)).max(0.0);
                            let to = sim.0;
                            theater.push_anim(Anim::HpSet { side: *side, to });
                        }
                        E::WeatherChip { target, amount } => {
                            let sim = &mut theater.sim[usize::from(*target)];
                            sim.0 = (sim.0 - f32::from(*amount)).max(0.0);
                            let to = sim.0;
                            theater.push_anim(Anim::HpSet { side: *target, to });
                        }
                        E::Healed {
                            target,
                            slot: 0,
                            amount,
                        } => {
                            let sim = &mut theater.sim[usize::from(*target)];
                            sim.0 = (sim.0 + f32::from(*amount)).min(sim.1);
                            let to = sim.0;
                            theater.push_anim(Anim::HpSet { side: *target, to });
                        }
                        E::Drained { from, amount } => {
                            let sim = &mut theater.sim[usize::from(1 - *from)];
                            sim.0 = (sim.0 + f32::from(*amount)).min(sim.1);
                            let to = sim.0;
                            theater.push_anim(Anim::HpSet {
                                side: 1 - *from,
                                to,
                            });
                        }
                        E::SeededDrain { from, amount } => {
                            let sim = &mut theater.sim[usize::from(*from)];
                            sim.0 = (sim.0 - f32::from(*amount)).max(0.0);
                            let drained_to = sim.0;
                            theater.push_anim(Anim::HpSet {
                                side: *from,
                                to: drained_to,
                            });
                            let sim = &mut theater.sim[usize::from(1 - *from)];
                            sim.0 = (sim.0 + f32::from(*amount)).min(sim.1);
                            let healed_to = sim.0;
                            theater.push_anim(Anim::HpSet {
                                side: 1 - *from,
                                to: healed_to,
                            });
                        }
                        E::Fainted { target, slot, .. } => {
                            if *slot == 0 {
                                theater.sim[usize::from(*target)].0 = 0.0;
                                theater.push_anim(Anim::Faint { side: *target });
                            }
                            theater.push_line(format!(
                                "{} faints!",
                                if *target == 0 { "your mote" } else { "the foe" }
                            ));
                        }
                        E::ExpGained { amount, .. } => {
                            theater.push_line(format!("gained {amount} exp"));
                        }
                        E::LeveledUp { level, .. } => {
                            theater.push_line(format!("level {level}!"));
                        }
                        E::AttuneAttempt { rings, caught } => {
                            theater.push_anim(Anim::Capture {
                                rings: *rings,
                                caught: *caught,
                            });
                            theater.push_line(if *caught {
                                "* * * * - the fermata settles!".to_string()
                            } else {
                                format!("{} - it shatters out!", "* ".repeat(usize::from(*rings)))
                            });
                        }
                        E::EscapeAttempt { fled, .. } => {
                            theater.push_line(if *fled {
                                "slipped away!".into()
                            } else {
                                "can't escape!".into()
                            });
                        }
                        E::MoveMissed { side } => {
                            theater.push_line(format!(
                                "{} misses",
                                if *side == 0 { "you" } else { "foe" }
                            ));
                        }
                        E::MoveFailed { side, .. } => {
                            theater.push_line(format!(
                                "{}'s move fails!",
                                if *side == 0 { "your" } else { "the foe's" }
                            ));
                        }
                        E::BattleEnded { outcome } => {
                            if matches!(outcome, battle::Outcome::Won { winner: 0 }) {
                                theater.push_anim(Anim::Jingle {
                                    track: "jingle_victory".into(),
                                });
                            }
                        }
                        E::SwitchedIn {
                            side,
                            slot,
                            species,
                        } => {
                            if *slot == 0 {
                                // Resolve the incoming mote through the
                                // engine's stable slot → party mapping
                                // (a species search can hit a benched
                                // duplicate). Session gone (the batch
                                // ended the battle): keep the sim's
                                // view and the old label.
                                let (hp, max, level) = world
                                    .battle
                                    .as_ref()
                                    .and_then(|s| {
                                        let battle_side = &s.state.sides[usize::from(*side)];
                                        let index = battle_side
                                            .positions
                                            .get(usize::from(*slot))
                                            .map(|p| usize::from(p.party_index))?;
                                        battle_side.party.get(index).map(|m| {
                                            (f32::from(m.hp), f32::from(m.max_hp()), m.level)
                                        })
                                    })
                                    .unwrap_or((
                                        theater.sim[usize::from(*side)].0,
                                        theater.sim[usize::from(*side)].1,
                                        0,
                                    ));
                                theater.sim[usize::from(*side)] = (hp, max);
                                theater.push_anim(Anim::SwitchIn {
                                    side: *side,
                                    species: species.clone(),
                                    hp,
                                    max,
                                    level,
                                });
                            }
                            theater.push_line(format!(
                                "{} {} takes the stage",
                                if *side == 0 { "your" } else { "foe" },
                                world.text(&format!("motif.{species}"))
                            ));
                        }
                        _ => {}
                    }
                }
            }
            WorldEvent::MoteCaught { species } => {
                theater.push_line(format!("{} joins your score!", name(species)));
            }
            WorldEvent::LearnPrompt { species, move_id } => {
                theater.push_line(format!(
                    "{} wants to learn {} (Z: replace first move / X: skip)",
                    name(species),
                    move_name(move_id)
                ));
            }
            WorldEvent::MoveLearned { species, move_id } => {
                theater.push_line(format!(
                    "{} learned {}",
                    name(species),
                    move_name(move_id)
                ));
            }
            WorldEvent::EvolutionPrompt { from, into } => {
                theater.push_line(
                    world
                        .text("ui.evolve.prompt")
                        .replace("{0}", &name(from))
                        .replace("{1}", &name(into)),
                );
            }
            WorldEvent::Evolved { from, into } => {
                theater.push_line(world.text("ui.evolve.shifting").replace("{0}", &name(from)));
                theater.push_anim(Anim::Evolve {
                    from: from.clone(),
                    into: into.clone(),
                });
                theater.push_line(
                    world
                        .text("ui.evolve.became")
                        .replace("{0}", &name(from))
                        .replace("{1}", &name(into)),
                );
            }
            WorldEvent::Whiteout => {
                theater.push_line("your motes fall silent... (half your coin lost)".into());
            }
            WorldEvent::MoneyChanged { money } => {
                theater.push_line(format!("{money}c"));
            }
            WorldEvent::ActionRejected { reason_key } => {
                theater.push_line(world.text(reason_key));
            }
            WorldEvent::ShiftOffered => {
                shift_offered = true;
            }
            _ => {}
        }
    }
    if shift_offered {
        theater.push_line("send in another mote? (arrows pick / Z send / X keep)".into());
    }
}

fn play_cry(
    commands: &mut Commands,
    assets: &AssetServer,
    settings: &save::Settings,
    region: &str,
    species: &SpeciesId,
) {
    let volume = f32::from(settings.volume_sfx) / 100.0;
    commands.spawn((
        AudioPlayer::new(assets.load(format!("cries/{region}/{species}.wav"))),
        PlaybackSettings::DESPAWN.with_volume(bevy::audio::Volume::Linear(volume)),
    ));
}

fn fx_frame_path(ty: Type, frame: usize) -> String {
    format!("sprites/fx/{}.{}.png", format!("{ty:?}").to_lowercase(), frame)
}

#[expect(clippy::too_many_arguments, reason = "bevy system parameters")]
fn battle_enter(
    mut commands: Commands,
    theme: Res<Theme>,
    world: Res<WorldRes>,
    assets: Res<AssetServer>,
    settings: Res<crate::app::SettingsRes>,
    mut cursor: ResMut<BattleCursor>,
    mut music: ResMut<crate::app::CurrentMusic>,
    mut theater: ResMut<Theater>,
    mut disp: ResMut<DisplayedHp>,
) {
    cursor.mode = 0;
    cursor.index = 0;
    *theater = Theater::default();
    if let Some(session) = &world.0.battle {
        // Battle theme by context (doc 04 §7): maestros + admins get
        // the hall theme.
        music.override_track = Some(match &session.context {
            game::session::BattleContext::Wild { .. } => "battle_wild".to_string(),
            game::session::BattleContext::Trainer { id } => match id.as_str() {
                // Story themes (P20): the named beats carry their own
                // stems.
                "admin_lull" => "lull_theme".to_string(),
                "vesper_epilogue" => "vesper_theme".to_string(),
                "maestro_ilva" | "maestro_calder" => "hall_final".to_string(),
                _ => {
                    let weighty = world.0.registry.as_ref().is_some_and(|r| {
                        r.trainers
                            .get(id)
                            .is_some_and(|t| t.class == "Maestro" || t.class == "TacetAdmin")
                    });
                    if weighty {
                        "battle_hall".to_string()
                    } else {
                        "battle_trainer".to_string()
                    }
                }
            },
        });
    }
    let Some(session) = &world.0.battle else {
        return;
    };
    let foe = session.state.sides[1].active_mote();
    let us = session.state.sides[0].active_mote();

    // Displayed HP mirrors live state at the curtain.
    disp.snap(0, f32::from(us.hp), f32::from(us.max_hp()));
    disp.snap(1, f32::from(foe.hp), f32::from(foe.max_hp()));
    disp.set_label(0, &us.species, us.level);
    disp.set_label(1, &foe.species, foe.level);
    theater.sim = [
        (f32::from(us.hp), f32::from(us.max_hp())),
        (f32::from(foe.hp), f32::from(foe.max_hp())),
    ];

    // The entry choreography, then the intro line (typewritten).
    let animate = settings.0.battle_animations;
    theater.push_anim(Anim::Entry {
        foe: foe.species.clone(),
    });
    theater.push_line(match session.context {
        game::session::BattleContext::Wild { .. } => world.0.text("ui.battle.wild_intro"),
        game::session::BattleContext::Trainer { .. } => world.0.text("ui.battle.trainer_intro"),
    });

    // Sprites and plates start off-stage when the entry animates.
    let (foe_right, ally_left) = if animate {
        (OFFSCREEN, OFFSCREEN)
    } else {
        (FOE_RIGHT, ALLY_LEFT)
    };
    let (foe_plate_left, player_plate_right) = if animate {
        (OFFSCREEN, OFFSCREEN)
    } else {
        (FOE_PLATE_LEFT, PLAYER_PLATE_RIGHT)
    };

    commands
        .spawn((
            BattleUi,
            BattleRoot,
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
            // Ground pads (P11: the Gen-3 stage).
            for (right, left, bottom, top, w) in [
                (Some(44.0_f32), None, None, Some(108.0_f32), 132.0_f32),
                (None, Some(30.0), Some(64.0), None, 132.0),
            ] {
                let mut node = Node {
                    position_type: PositionType::Absolute,
                    width: Val::Px(w),
                    height: Val::Px(34.0),
                    ..default()
                };
                if let Some(v) = right {
                    node.right = Val::Px(v);
                }
                if let Some(v) = left {
                    node.left = Val::Px(v);
                }
                if let Some(v) = bottom {
                    node.bottom = Val::Px(v);
                }
                if let Some(v) = top {
                    node.top = Val::Px(v);
                }
                root.spawn((
                    ImageNode::new(assets.load(game::art::art("sprites/battle/platform.png"))),
                    node,
                ));
            }
            // Foe sigil, upper right (off-stage until the entry).
            root.spawn((
                FoeSprite,
                ShownSpecies(foe.species.clone()),
                ImageNode::new(assets.load(game::art::art(&format!(
                    "sprites/monsters/{}/{}.front.png",
                    world.0.region_id, foe.species
                )))),
                Node {
                    position_type: PositionType::Absolute,
                    right: Val::Px(foe_right),
                    top: Val::Px(FOE_TOP),
                    width: Val::Px(SPRITE_SIZE),
                    height: Val::Px(SPRITE_SIZE),
                    ..default()
                },
            ));
            // Player sigil (back), lower left.
            root.spawn((
                PlayerSpriteImg,
                ShownSpecies(us.species.clone()),
                ImageNode::new(assets.load(game::art::art(&format!(
                    "sprites/monsters/{}/{}.back.png",
                    world.0.region_id, us.species
                )))),
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(ally_left),
                    bottom: Val::Px(ALLY_BOTTOM),
                    width: Val::Px(SPRITE_SIZE),
                    height: Val::Px(SPRITE_SIZE),
                    ..default()
                },
            ));
            // Foe plate.
            root.spawn((
                FoePlate,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(foe_plate_left),
                    top: Val::Px(8.0),
                    width: Val::Px(170.0),
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(Val::Px(6.0)),
                    border: UiRect::all(Val::Px(2.0)),
                    row_gap: Val::Px(3.0),
                    ..default()
                },
                BackgroundColor(theme.color(&theme.palette.parchment_dim)),
                BorderColor::all(theme.color(&theme.palette.ink)),
            ))
            .with_children(|plate| {
                crate::app::paper_overlay(plate, &assets);
                crate::app::corner_caps(plate, &assets);
                plate.spawn((
                    FoePlateText,
                    Text::new(""),
                    TextFont::from_font_size(9.0),
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
                PlayerPlate,
                Node {
                    position_type: PositionType::Absolute,
                    right: Val::Px(player_plate_right),
                    bottom: Val::Px(76.0),
                    width: Val::Px(170.0),
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(Val::Px(6.0)),
                    border: UiRect::all(Val::Px(2.0)),
                    row_gap: Val::Px(3.0),
                    ..default()
                },
                BackgroundColor(theme.color(&theme.palette.parchment_dim)),
                BorderColor::all(theme.color(&theme.palette.ink)),
            ))
            .with_children(|plate| {
                crate::app::paper_overlay(plate, &assets);
                crate::app::corner_caps(plate, &assets);
                plate.spawn((
                    PlayerPlateText,
                    Text::new(""),
                    TextFont::from_font_size(9.0),
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
                // EXP toward the next level (P15) — the thin gilt line.
                plate
                    .spawn((
                        Node {
                            width: Val::Percent(100.0),
                            height: Val::Px(3.0),
                            ..default()
                        },
                        BackgroundColor(theme.color(&theme.palette.ink_soft)),
                    ))
                    .with_child((
                        PlayerExpBar,
                        Node {
                            width: Val::Percent(0.0),
                            height: Val::Percent(100.0),
                            ..default()
                        },
                        BackgroundColor(theme.color(&theme.palette.gilt)),
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
                // Command icons (P19): tiny glyphs leading each label.
                const ICONS: [&str; 4] = ["fight", "bell", "tonic", "run"];
                for (index, label) in COMMANDS.iter().enumerate() {
                    row.spawn((
                        CommandRow(index),
                        Node {
                            flex_grow: 1.0,
                            padding: UiRect::all(Val::Px(5.0)),
                            column_gap: Val::Px(4.0),
                            align_items: AlignItems::Center,
                            ..default()
                        },
                        BackgroundColor(theme.color(&theme.palette.parchment)),
                    ))
                    .with_child((
                        CommandIcon,
                        ImageNode::new(assets.load(game::art::art(&format!(
                            "sprites/ui/icon_{}.png",
                            ICONS[index]
                        )))),
                        Node {
                            width: Val::Px(12.0),
                            height: Val::Px(12.0),
                            ..default()
                        },
                    ))
                    .with_child((
                        Text::new(*label),
                        TextFont::from_font_size(9.0),
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
                Text::new(""),
                TextFont::from_font_size(9.0),
                TextColor(theme.color(&theme.palette.ink)),
            ));
        });
}

/// The sprite/plate Node sets the theater animates. ParamSet because
/// they all reach for `Node`/`ImageNode` mutably. The inner queries
/// carry their own lifetimes — tying them to the ParamSet's would
/// break the for-all-lifetimes bound function systems need.
type StageQueries<'w, 's, 'wq, 'sq> = ParamSet<
    'w,
    's,
    (
        Query<'wq, 'sq, (&'static mut Node, &'static mut ImageNode, &'static mut ShownSpecies), With<FoeSprite>>,
        Query<'wq, 'sq, (&'static mut Node, &'static mut ImageNode, &'static mut ShownSpecies), With<PlayerSpriteImg>>,
        Query<'wq, 'sq, &'static mut Node, With<FoePlate>>,
        Query<'wq, 'sq, &'static mut Node, With<PlayerPlate>>,
        Query<'wq, 'sq, &'static mut ImageNode, With<FxSprite>>,
        Query<'wq, 'sq, (&'static mut ImageNode, Entity), With<EvoSprite>>,
    ),
>;

/// Smooth in-out bump: 0 → 1 → 0 over p ∈ [0, 1].
fn bump(p: f32) -> f32 {
    (p.clamp(0.0, 1.0) * std::f32::consts::PI).sin()
}

/// Eased slide: 0 → 1 over p ∈ [0, 1].
fn ease(p: f32) -> f32 {
    let p = p.clamp(0.0, 1.0);
    p * p * (3.0 - 2.0 * p)
}

/// The theater heart: reveals lines per-character, plays animations in
/// order, holds input until idle. Z reveals/advances (hold = 4×);
/// X fast-forwards the evolution scene.
#[expect(clippy::too_many_arguments, reason = "bevy system parameters")]
fn theater_tick(
    mut commands: Commands,
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    assets: Res<AssetServer>,
    settings: Res<crate::app::SettingsRes>,
    world: Res<WorldRes>,
    mut theater: ResMut<Theater>,
    mut fx: ResMut<FxState>,
    mut disp: ResMut<DisplayedHp>,
    mut music: ResMut<crate::app::CurrentMusic>,
    mut message: Query<&mut Text, With<MessageText>>,
    mut stage: StageQueries,
) {
    theater.popped_this_frame = false;
    // Hit-stop: the whole theater freezes (the crit freeze-frame).
    if fx.hit_stop > 0.0 {
        return;
    }
    let mut dt = time.delta_secs();
    if keys.pressed(KeyCode::KeyZ) {
        dt *= 4.0; // hold-Z fast-forward, animations included
    }
    let animate = settings.0.battle_animations;
    let region = world.0.region_id.clone();

    // 1) An active animation owns the stage.
    if theater.active.is_some() {
        let done = {
            let Theater { active, .. } = &mut *theater;
            let anim = active.as_mut().expect("checked above");
            anim.t += dt;
            if keys.just_pressed(KeyCode::KeyX)
                && matches!(anim.kind, Anim::Evolve { .. })
                && anim.t < 3.1
            {
                anim.t = 3.1; // skip to the resolve
                theater.popped_this_frame = true;
                let Theater { active, .. } = &mut *theater;
                let anim = active.as_mut().expect("still active");
                tick_anim(
                    anim, dt, animate, &region, &mut commands, &assets, &settings.0, &mut fx,
                    &mut disp, &mut music, &mut stage,
                )
            } else {
                tick_anim(
                    anim, dt, animate, &region, &mut commands, &assets, &settings.0, &mut fx,
                    &mut disp, &mut music, &mut stage,
                )
            }
        };
        if done
            && let Some(anim) = theater.active.take()
        {
            finish_anim(
                &anim, &region, &mut commands, &assets, &settings.0, &mut disp, &mut music,
                &mut stage,
            );
        }
        // A Z/X landing on an animation frame (including its last) must
        // not leak into battle_input as a menu confirm or prompt answer.
        if keys.just_pressed(KeyCode::KeyZ) || keys.just_pressed(KeyCode::KeyX) {
            theater.popped_this_frame = true;
        }
        return;
    }

    // 2) Typewriter reveal of the current line.
    if theater.shown < theater.chars {
        let speed = f32::from(settings.0.text_speed);
        if keys.just_pressed(KeyCode::KeyZ) {
            theater.shown = theater.chars;
            theater.popped_this_frame = true;
        } else if speed <= 0.0 || !animate {
            // Instant text: speed 0, or battle animations toggled off
            // (the P9 speedrun contract).
            theater.shown = theater.chars;
        } else {
            theater.reveal_acc += speed * dt;
            let step = theater.reveal_acc.floor() as usize;
            if step > 0 {
                theater.reveal_acc -= step as f32;
                let before = theater.shown;
                theater.shown = (theater.shown + step).min(theater.chars);
                // A soft blip every third character.
                if theater.shown / 3 > before / 3 {
                    crate::app::play_cue_volume(&mut commands, &assets, &settings.0, "blip", 0.5);
                }
            }
        }
        if let Ok(mut text) = message.single_mut() {
            text.0 = theater.line.chars().take(theater.shown).collect();
        }
        return;
    }

    // 3) Fully revealed: dwell, then advance to the next item.
    if theater.items.is_empty() {
        // Idle: the theater's view and the session agree again —
        // retarget displayed HP to live state so drift self-heals, and
        // refresh max/label (mid-battle level-ups raise both).
        if let Some(session) = &world.0.battle {
            for side in 0..2u8 {
                let mote = session.state.sides[usize::from(side)].active_mote();
                disp.set_target(side, f32::from(mote.hp));
                disp.set_max(side, f32::from(mote.max_hp()));
                disp.set_label(side, &mote.species, mote.level);
                theater.sim[usize::from(side)] =
                    (f32::from(mote.hp), f32::from(mote.max_hp()));
            }
        }
        return;
    }
    theater.dwell += dt;
    let pace = if !animate {
        0.05 // animations off: near-instant pacing (P9)
    } else {
        match settings.0.battle_pace {
            0 => 1.1,
            1 => 0.7,
            _ => 0.4,
        }
    };
    let advance = theater.dwell >= pace
        || keys.just_pressed(KeyCode::KeyZ)
        || theater.chars == 0;
    if !advance {
        return;
    }
    if keys.just_pressed(KeyCode::KeyZ) {
        theater.popped_this_frame = true;
    }
    match theater.items.pop_front() {
        Some(TheaterItem::Line(line)) => {
            theater.set_line(line);
        }
        Some(TheaterItem::Anim(kind)) => {
            theater.dwell = 0.0;
            let mut anim = ActiveAnim {
                kind,
                t: 0.0,
                spawned: None,
                fired: 0,
            };
            if animate {
                // First tick at t=0 fires entry cues/spawns.
                let done = tick_anim(
                    &mut anim, 0.0, animate, &region, &mut commands, &assets, &settings.0,
                    &mut fx, &mut disp, &mut music, &mut stage,
                );
                if done {
                    finish_anim(
                        &anim, &region, &mut commands, &assets, &settings.0, &mut disp,
                        &mut music, &mut stage,
                    );
                } else {
                    theater.active = Some(anim);
                }
            } else {
                finish_anim(
                    &anim, &region, &mut commands, &assets, &settings.0, &mut disp, &mut music,
                    &mut stage,
                );
            }
        }
        None => {}
    }
}

/// Advances one animation; returns true when it has finished. All
/// motion derives from `anim.t`; cues fire once via the `fired` mask.
#[expect(clippy::too_many_arguments, reason = "animation plumbing")]
fn tick_anim(
    anim: &mut ActiveAnim,
    _dt: f32,
    animate: bool,
    region: &str,
    commands: &mut Commands,
    assets: &AssetServer,
    settings: &save::Settings,
    fx: &mut FxState,
    disp: &mut DisplayedHp,
    music: &mut crate::app::CurrentMusic,
    stage: &mut StageQueries,
) -> bool {
    let reduced = settings.reduced_motion;
    let t = anim.t;
    let fire = |mask: u32, fired: &mut u32| -> bool {
        if *fired & mask == 0 {
            *fired |= mask;
            true
        } else {
            false
        }
    };
    match &anim.kind {
        Anim::Entry { foe } => {
            if reduced {
                return true; // finish_anim snaps everything into place
            }
            let slide = ease(t / 0.5);
            if let Ok((mut node, _, _)) = stage.p0().single_mut() {
                node.right = Val::Px(OFFSCREEN + (FOE_RIGHT - OFFSCREEN) * slide);
            }
            if let Ok((mut node, _, _)) = stage.p1().single_mut() {
                node.left = Val::Px(OFFSCREEN + (ALLY_LEFT - OFFSCREEN) * slide);
                // The lob: a small arc on the way in.
                node.bottom = Val::Px(ALLY_BOTTOM + 36.0 * bump(t / 0.5));
            }
            if t >= 0.5 && fire(1, &mut anim.fired) {
                play_cry(commands, assets, settings, region, foe);
            }
            let plates = ease((t - 0.5) / 0.3);
            if t >= 0.5 {
                if let Ok(mut node) = stage.p2().single_mut() {
                    node.left = Val::Px(OFFSCREEN + (FOE_PLATE_LEFT - OFFSCREEN) * plates);
                }
                if let Ok(mut node) = stage.p3().single_mut() {
                    node.right = Val::Px(OFFSCREEN + (PLAYER_PLATE_RIGHT - OFFSCREEN) * plates);
                }
            }
            t >= 0.9
        }
        Anim::Lunge { side } => {
            if reduced {
                return true;
            }
            let nudge = 18.0 * bump(t / 0.22);
            if *side == 0 {
                if let Ok((mut node, _, _)) = stage.p1().single_mut() {
                    node.left = Val::Px(ALLY_LEFT + nudge);
                }
            } else if let Ok((mut node, _, _)) = stage.p0().single_mut() {
                node.right = Val::Px(FOE_RIGHT + nudge);
            }
            t >= 0.22
        }
        Anim::Impact {
            target,
            ty,
            crit,
            hp_to,
        } => {
            if fire(1, &mut anim.fired) {
                // The strike lands: flash + shake (+ the crit freeze),
                // the effect sprite, and the HP drain all start here.
                fx.shake = fx.shake.max(if *crit { 0.30 } else { 0.15 });
                fx.flash = Some((*target, 0.18));
                if animate {
                    fx.hit_stop = if *crit { 0.25 } else { 0.10 };
                }
                disp.set_target(*target, *hp_to);
                if animate {
                    let mut node = Node {
                        position_type: PositionType::Absolute,
                        width: Val::Px(64.0),
                        height: Val::Px(64.0),
                        ..default()
                    };
                    if *target == 1 {
                        node.right = Val::Px(FOE_RIGHT + 16.0);
                        node.top = Val::Px(FOE_TOP + 16.0);
                    } else {
                        node.left = Val::Px(ALLY_LEFT + 16.0);
                        node.bottom = Val::Px(ALLY_BOTTOM + 16.0);
                    }
                    let entity = commands
                        .spawn((
                            BattleUi,
                            FxSprite,
                            ImageNode::new(assets.load(game::art::art(&fx_frame_path(*ty, 0)))),
                            node,
                        ))
                        .id();
                    anim.spawned = Some(entity);
                }
            }
            // Frame strip: 4 frames across 0.48s.
            let frame = ((t / 0.12) as usize).min(3);
            let bit = 2u32 << frame; // bits 1..=4 mark shown frames
            if fire(bit, &mut anim.fired)
                && let Ok(mut image) = stage.p4().single_mut()
            {
                image.image = assets.load(game::art::art(&fx_frame_path(*ty, frame)));
            }
            t >= 0.48 && disp.drained(*target)
        }
        Anim::HpSet { side, to } => {
            if fire(1, &mut anim.fired) {
                disp.set_target(*side, *to);
            }
            disp.drained(*side)
        }
        Anim::Faint { side } => {
            if fire(1, &mut anim.fired) {
                crate::app::play_cue(commands, assets, settings, "faint");
                disp.set_target(*side, 0.0);
            }
            let p = (t / 0.6).clamp(0.0, 1.0);
            let drop = if reduced { 0.0 } else { 40.0 * ease(p) };
            let alpha = 1.0 - p;
            if *side == 1 {
                if let Ok((mut node, mut image, _)) = stage.p0().single_mut() {
                    node.top = Val::Px(FOE_TOP + drop);
                    image.color = Color::srgba(1.0, 1.0, 1.0, alpha);
                }
            } else if let Ok((mut node, mut image, _)) = stage.p1().single_mut() {
                node.bottom = Val::Px(ALLY_BOTTOM - drop);
                image.color = Color::srgba(1.0, 1.0, 1.0, alpha);
            }
            t >= 0.6 && disp.drained(*side)
        }
        Anim::SwitchIn {
            side,
            species,
            hp,
            max,
            level,
        } => {
            if fire(1, &mut anim.fired) {
                disp.snap(*side, *hp, *max);
                if *level > 0 {
                    disp.set_label(*side, species, *level);
                }
                let path = if *side == 1 {
                    format!("sprites/monsters/{region}/{species}.front.png")
                } else {
                    format!("sprites/monsters/{region}/{species}.back.png")
                };
                let handle = assets.load(game::art::art(&path));
                if *side == 1 {
                    if let Ok((mut node, mut image, mut shown)) = stage.p0().single_mut() {
                        shown.0 = species.clone();
                        image.image = handle;
                        image.color = Color::WHITE;
                        node.top = Val::Px(FOE_TOP);
                        node.right = Val::Px(if reduced { FOE_RIGHT } else { OFFSCREEN });
                        node.width = Val::Px(SPRITE_SIZE);
                        node.height = Val::Px(SPRITE_SIZE);
                    }
                } else if let Ok((mut node, mut image, mut shown)) = stage.p1().single_mut() {
                    shown.0 = species.clone();
                    image.image = handle;
                    image.color = Color::WHITE;
                    node.bottom = Val::Px(ALLY_BOTTOM);
                    node.left = Val::Px(if reduced { ALLY_LEFT } else { OFFSCREEN });
                    node.width = Val::Px(SPRITE_SIZE);
                    node.height = Val::Px(SPRITE_SIZE);
                }
            }
            if !reduced {
                let slide = ease(t / 0.45);
                if *side == 1 {
                    if let Ok((mut node, _, _)) = stage.p0().single_mut() {
                        node.right = Val::Px(OFFSCREEN + (FOE_RIGHT - OFFSCREEN) * slide);
                    }
                } else if let Ok((mut node, _, _)) = stage.p1().single_mut() {
                    node.left = Val::Px(OFFSCREEN + (ALLY_LEFT - OFFSCREEN) * slide);
                }
            }
            if t >= 0.45 && fire(2, &mut anim.fired) {
                play_cry(commands, assets, settings, region, species);
            }
            t >= 0.55
        }
        Anim::Capture { rings, caught } => {
            // Phases: shrink (0..0.5), wobbles (0.55s each), resolve.
            let wobbles = if *caught { 3 } else { u32::from((*rings).min(3)) };
            let wobble_end = 0.5 + 0.55 * wobbles as f32;
            if fire(1, &mut anim.fired) {
                crate::app::play_cue(commands, assets, settings, "bell");
            }
            let gilt = Color::srgb(2.0, 1.7, 0.7);
            if let Ok((mut node, mut image, _)) = stage.p0().single_mut() {
                if t < 0.5 {
                    // reduced_motion: snap to the point, no shrink ease.
                    let p = if reduced { 1.0 } else { ease(t / 0.5) };
                    let size = SPRITE_SIZE - (SPRITE_SIZE - 10.0) * p;
                    node.width = Val::Px(size);
                    node.height = Val::Px(size);
                    node.right = Val::Px(FOE_RIGHT + (SPRITE_SIZE - size) / 2.0);
                    node.top = Val::Px(FOE_TOP + (SPRITE_SIZE - size) / 2.0);
                    image.color = Color::WHITE.mix(&gilt, p);
                } else if t < wobble_end {
                    let wp = ((t - 0.5) / 0.55).fract();
                    let wobble_index = ((t - 0.5) / 0.55) as u32;
                    if fire(2 << wobble_index, &mut anim.fired) {
                        crate::app::play_cue(commands, assets, settings, "wobble");
                    }
                    // reduced_motion: the point holds still; the wobble
                    // cues alone carry the count.
                    let size = if reduced { 10.0 } else { 10.0 + 4.0 * bump(wp) };
                    let jiggle = if reduced { 0.0 } else { 3.0 * bump(wp * 2.0) };
                    node.width = Val::Px(size);
                    node.height = Val::Px(size);
                    node.right = Val::Px(FOE_RIGHT + (SPRITE_SIZE - size) / 2.0 + jiggle);
                    node.top = Val::Px(FOE_TOP + (SPRITE_SIZE - size) / 2.0);
                    image.color = gilt;
                } else if *caught {
                    if fire(1 << 16, &mut anim.fired) {
                        // The capture jingle replaces the bare settle
                        // chime (P20); the battle theme bows out under
                        // the closing lines.
                        crate::app::play_cue(commands, assets, settings, "jingle_capture");
                        music.override_track = None;
                    }
                    // The point rests, then dims out.
                    let p = ((t - wobble_end) / 0.4).clamp(0.0, 1.0);
                    image.color = gilt.with_alpha(1.0 - p);
                } else {
                    if fire(1 << 16, &mut anim.fired) {
                        crate::app::play_cue(commands, assets, settings, "breakout");
                    }
                    let p = if reduced { 1.0 } else { ease((t - wobble_end) / 0.3) };
                    let size = 10.0 + (SPRITE_SIZE - 10.0) * p;
                    node.width = Val::Px(size);
                    node.height = Val::Px(size);
                    node.right = Val::Px(FOE_RIGHT + (SPRITE_SIZE - size) / 2.0);
                    node.top = Val::Px(FOE_TOP + (SPRITE_SIZE - size) / 2.0);
                    image.color = gilt.mix(&Color::WHITE, p);
                }
            }
            t >= wobble_end + if *caught { 0.45 } else { 0.35 }
        }
        Anim::Evolve { from, into } => {
            // 0..0.7 whiten; 0.7..3.1 alternating flashes
            // (accelerating); 3.1 resolve + cry; 3.7 done.
            if fire(1, &mut anim.fired) {
                let overlay = commands
                    .spawn((
                        BattleUi,
                        Node {
                            position_type: PositionType::Absolute,
                            left: Val::Px(0.0),
                            top: Val::Px(0.0),
                            width: Val::Percent(100.0),
                            height: Val::Percent(100.0),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.08, 0.07, 0.06, 0.92)),
                        GlobalZIndex(50),
                    ))
                    .with_child((
                        EvoSprite,
                        ImageNode::new(assets.load(game::art::art(&format!(
                            "sprites/monsters/{region}/{from}.front.png"
                        )))),
                        Node {
                            width: Val::Px(128.0),
                            height: Val::Px(128.0),
                            ..default()
                        },
                    ))
                    .id();
                anim.spawned = Some(overlay);
            }
            if let Ok((mut image, _)) = stage.p5().single_mut() {
                if t < 0.7 {
                    // reduced_motion: no whiten ramp — the scene plays
                    // as a calm dissolve, not a strobe.
                    let level = if reduced {
                        1.0
                    } else {
                        1.0 + 6.0 * ease(t / 0.7)
                    };
                    image.color = Color::srgb(level, level, level);
                } else if t < 3.1 {
                    if reduced {
                        // One quiet swap to the new form; no flashing.
                        if fire(1 << 8, &mut anim.fired) {
                            image.image = assets.load(game::art::art(&format!(
                                "sprites/monsters/{region}/{into}.front.png"
                            )));
                        }
                        image.color = Color::WHITE;
                    } else {
                        // Accelerating alternation: count flips by
                        // walking the shrinking-period schedule.
                        let mut flip_t = 0.7_f32;
                        let mut period = 0.55_f32;
                        let mut flips = 0u32;
                        while flip_t + period < t {
                            flip_t += period;
                            period = (period * 0.82).max(0.10);
                            flips += 1;
                        }
                        let show_into = flips % 2 == 1;
                        let bit = if show_into { 1 << 8 } else { 1 << 9 };
                        let other = if show_into { 1 << 9 } else { 1 << 8 };
                        if anim.fired & bit == 0 {
                            anim.fired = (anim.fired | bit) & !other;
                            let which = if show_into { into } else { from };
                            image.image = assets.load(game::art::art(&format!(
                                "sprites/monsters/{region}/{which}.front.png"
                            )));
                        }
                        image.color = Color::srgb(7.0, 7.0, 7.0);
                    }
                } else {
                    if fire(1 << 16, &mut anim.fired) {
                        image.image = assets.load(game::art::art(&format!(
                            "sprites/monsters/{region}/{into}.front.png"
                        )));
                        play_cry(commands, assets, settings, region, into);
                        crate::app::play_cue(commands, assets, settings, "jingle_evolution");
                    }
                    let level = if reduced {
                        1.0
                    } else {
                        7.0 - 6.0 * ease((t - 3.1) / 0.3)
                    };
                    image.color = Color::srgb(level, level, level);
                }
            }
            t >= 3.7
        }
        Anim::Jingle { track } => {
            if fire(1, &mut anim.fired) {
                music.override_track = Some(track.clone());
            }
            true
        }
    }
}

/// Applies an animation's end state — the skip path, the
/// animations-off path, and the natural-finish cleanup all land here.
/// Cues/cries the ticking never fired (instant and reduced-motion
/// paths) still sound: the audio is part of the scene, not the motion.
#[expect(clippy::too_many_arguments, reason = "animation plumbing")]
fn finish_anim(
    anim: &ActiveAnim,
    region: &str,
    commands: &mut Commands,
    assets: &AssetServer,
    settings: &save::Settings,
    disp: &mut DisplayedHp,
    music: &mut crate::app::CurrentMusic,
    stage: &mut StageQueries,
) {
    match &anim.kind {
        Anim::Entry { foe } => {
            if anim.fired & 1 == 0 {
                play_cry(commands, assets, settings, region, foe);
            }
            if let Ok((mut node, _, _)) = stage.p0().single_mut() {
                node.right = Val::Px(FOE_RIGHT);
                node.top = Val::Px(FOE_TOP);
            }
            if let Ok((mut node, _, _)) = stage.p1().single_mut() {
                node.left = Val::Px(ALLY_LEFT);
                node.bottom = Val::Px(ALLY_BOTTOM);
            }
            if let Ok(mut node) = stage.p2().single_mut() {
                node.left = Val::Px(FOE_PLATE_LEFT);
            }
            if let Ok(mut node) = stage.p3().single_mut() {
                node.right = Val::Px(PLAYER_PLATE_RIGHT);
            }
        }
        Anim::Lunge { side } => {
            if *side == 0 {
                if let Ok((mut node, _, _)) = stage.p1().single_mut() {
                    node.left = Val::Px(ALLY_LEFT);
                }
            } else if let Ok((mut node, _, _)) = stage.p0().single_mut() {
                node.right = Val::Px(FOE_RIGHT);
            }
        }
        Anim::Impact { target, hp_to, .. } => {
            disp.set_target(*target, *hp_to);
            if let Some(entity) = anim.spawned {
                commands.entity(entity).despawn();
            }
        }
        Anim::HpSet { side, to } => {
            disp.set_target(*side, *to);
        }
        Anim::Faint { side } => {
            if anim.fired & 1 == 0 {
                crate::app::play_cue(commands, assets, settings, "faint");
            }
            disp.set_target(*side, 0.0);
            // The sprite stays at alpha 0 until a SwitchIn restores it.
            if *side == 1 {
                if let Ok((mut node, mut image, _)) = stage.p0().single_mut() {
                    node.top = Val::Px(FOE_TOP);
                    image.color = Color::srgba(1.0, 1.0, 1.0, 0.0);
                }
            } else if let Ok((mut node, mut image, _)) = stage.p1().single_mut() {
                node.bottom = Val::Px(ALLY_BOTTOM);
                image.color = Color::srgba(1.0, 1.0, 1.0, 0.0);
            }
        }
        Anim::SwitchIn {
            side,
            species,
            hp,
            max,
            level,
        } => {
            if anim.fired & 2 == 0 {
                play_cry(commands, assets, settings, region, species);
            }
            disp.snap(*side, *hp, *max);
            if *level > 0 {
                disp.set_label(*side, species, *level);
            }
            let path = if *side == 1 {
                format!("sprites/monsters/{region}/{species}.front.png")
            } else {
                format!("sprites/monsters/{region}/{species}.back.png")
            };
            let handle = assets.load(game::art::art(&path));
            if *side == 1 {
                if let Ok((mut node, mut image, mut shown)) = stage.p0().single_mut() {
                    shown.0 = species.clone();
                    image.image = handle;
                    image.color = Color::WHITE;
                    node.right = Val::Px(FOE_RIGHT);
                    node.top = Val::Px(FOE_TOP);
                    node.width = Val::Px(SPRITE_SIZE);
                    node.height = Val::Px(SPRITE_SIZE);
                }
            } else if let Ok((mut node, mut image, mut shown)) = stage.p1().single_mut() {
                shown.0 = species.clone();
                image.image = handle;
                image.color = Color::WHITE;
                node.left = Val::Px(ALLY_LEFT);
                node.bottom = Val::Px(ALLY_BOTTOM);
                node.width = Val::Px(SPRITE_SIZE);
                node.height = Val::Px(SPRITE_SIZE);
            }
        }
        Anim::Capture { caught, .. } => {
            if anim.fired & (1 << 16) == 0 {
                crate::app::play_cue(
                    commands,
                    assets,
                    settings,
                    if *caught { "jingle_capture" } else { "breakout" },
                );
            }
            if *caught {
                music.override_track = None;
            }
            if let Ok((mut node, mut image, _)) = stage.p0().single_mut() {
                node.width = Val::Px(SPRITE_SIZE);
                node.height = Val::Px(SPRITE_SIZE);
                node.right = Val::Px(FOE_RIGHT);
                node.top = Val::Px(FOE_TOP);
                image.color = if *caught {
                    // Stays hidden — it lives in a bell now.
                    Color::srgba(1.0, 1.0, 1.0, 0.0)
                } else {
                    Color::WHITE
                };
            }
        }
        Anim::Evolve { into, .. } => {
            if anim.fired & (1 << 16) == 0 {
                play_cry(commands, assets, settings, region, into);
                crate::app::play_cue(commands, assets, settings, "jingle_evolution");
            }
            if let Some(entity) = anim.spawned {
                commands.entity(entity).despawn();
            }
        }
        Anim::Jingle { track } => {
            music.override_track = Some(track.clone());
        }
    }
}

#[expect(clippy::too_many_arguments, reason = "bevy system parameters")]
fn battle_input(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    assets: Res<AssetServer>,
    theme: Res<Theme>,
    settings: Res<crate::app::SettingsRes>,
    mut world: ResMut<WorldRes>,
    mut theater: ResMut<Theater>,
    mut cursor: ResMut<BattleCursor>,
    mut next: ResMut<NextState<AppState>>,
    mut rows: Query<(&CommandRow, &mut BackgroundColor)>,
    mut command_texts: Query<(&ChildOf, &mut Text), Without<MessageText>>,
    mut message_text: Query<&mut Text, With<MessageText>>,
) {
    // While the theater plays, only it owns the keys; and a Z/X it
    // consumed this frame must not double-fire here.
    if !theater.idle() || theater.popped_this_frame {
        return;
    }
    // Era Shift offer: free switch after a foe replacement (Set mode
    // auto-declines without surfacing it).
    if world.0.pending_shift {
        if settings.0.set_mode {
            world.0.apply(WorldInput::Shift(None));
            return;
        }
        let bench: Vec<u8> = world
            .0
            .battle
            .as_ref()
            .map(|session| {
                (0..session.state.sides[0].party.len() as u8)
                    .filter(|i| {
                        *i != session.state.sides[0].positions[0].party_index
                            && !session.state.sides[0].party[usize::from(*i)].is_fainted()
                    })
                    .collect()
            })
            .unwrap_or_default();
        if bench.is_empty() {
            world.0.apply(WorldInput::Shift(None));
            return;
        }
        let pick = bench[cursor.index % bench.len()];
        if keys.just_pressed(KeyCode::ArrowRight) {
            crate::app::play_cue(&mut commands, &assets, &settings.0, "cursor");
            cursor.index = (cursor.index + 1) % bench.len();
        }
        if keys.just_pressed(KeyCode::ArrowLeft) {
            crate::app::play_cue(&mut commands, &assets, &settings.0, "cursor");
            cursor.index = (cursor.index + bench.len() - 1) % bench.len();
        }
        if let Some(session) = &world.0.battle {
            let species = &session.state.sides[0].party[usize::from(pick)].species;
            let line = format!(
                "send {}? (arrows pick / Z send / X keep)",
                world.0.text(&format!("motif.{species}"))
            );
            if let Ok(mut message) = message_text.single_mut()
                && message.0 != line
            {
                message.0 = line;
            }
        }
        if keys.just_pressed(KeyCode::KeyZ) {
            let events = world.0.apply(WorldInput::Shift(Some(pick)));
            stage_battle_events(&mut theater, &world.0, &events);
            cursor.index = 0;
        } else if keys.just_pressed(KeyCode::KeyX) {
            world.0.apply(WorldInput::Shift(None));
            cursor.index = 0;
        }
        return;
    }

    // Prompts take priority: learn / evolution answered with Z/X. The
    // visible message is forced to the prompt actually being answered —
    // when several prompts stage in one batch, the last-typed line may
    // belong to a later one.
    if let Some((party_index, move_id)) = world.0.pending_learn_queue.first().cloned() {
        if let Some(mote) = world.0.party.get(party_index)
            && let Ok(mut message) = message_text.single_mut()
        {
            let line = format!(
                "{} wants to learn {} (Z: replace first move / X: skip)",
                world.0.text(&format!("motif.{}", mote.species)),
                world.0.text(&format!("move.{move_id}"))
            );
            if message.0 != line {
                message.0 = line;
            }
        }
        if keys.just_pressed(KeyCode::KeyZ) {
            let events = world.0.apply(WorldInput::Learn { replace: Some(0) });
            stage_battle_events(&mut theater, &world.0, &events);
        } else if keys.just_pressed(KeyCode::KeyX) {
            let events = world.0.apply(WorldInput::Learn { replace: None });
            stage_battle_events(&mut theater, &world.0, &events);
        }
        return;
    }
    if let Some((party_index, into)) = world.0.pending_evolutions.first().cloned() {
        let holder = world
            .0
            .party
            .get(party_index)
            .map(|mote| world.0.text(&format!("motif.{}", mote.species)));
        if let Some(species_name) = &holder
            && let Ok(mut message) = message_text.single_mut()
        {
            let line = world
                .0
                .text("ui.evolve.prompt")
                .replace("{0}", species_name)
                .replace("{1}", &world.0.text(&format!("motif.{into}")));
            if message.0 != line {
                message.0 = line;
            }
        }
        if keys.just_pressed(KeyCode::KeyZ) {
            let events = world.0.apply(WorldInput::Evolve { accept: true });
            stage_battle_events(&mut theater, &world.0, &events);
        } else if keys.just_pressed(KeyCode::KeyX) {
            let events = world.0.apply(WorldInput::Evolve { accept: false });
            stage_battle_events(&mut theater, &world.0, &events);
            // The decline gets its own beat (P20).
            if let Some(species_name) = holder {
                theater.push_line(
                    world.0.text("ui.evolve.held").replace("{0}", &species_name),
                );
            }
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
        crate::app::play_cue(&mut commands, &assets, &settings.0, "cursor");
        cursor.index = (cursor.index + 1) % limit;
    }
    if keys.just_pressed(KeyCode::ArrowLeft) {
        crate::app::play_cue(&mut commands, &assets, &settings.0, "cursor");
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
            stage_battle_events(&mut theater, &world.0, &events);
            cursor.mode = 0;
            cursor.index = 0;
        }
    }
}

/// Live plates: names, levels, HP bars (color by fraction per doc 05 §2).
type HpBarQuery<'w, 's, T, U> =
    Query<'w, 's, (&'static mut Node, &'static mut BackgroundColor), (With<T>, Without<U>)>;

fn apply_fx(
    time: Res<Time>,
    settings: Res<crate::app::SettingsRes>,
    mut fx: ResMut<FxState>,
    mut root: Query<&mut Node, With<BattleRoot>>,
    mut foe_img: Query<&mut ImageNode, (With<FoeSprite>, Without<PlayerSpriteImg>)>,
    mut player_img: Query<&mut ImageNode, (With<PlayerSpriteImg>, Without<FoeSprite>)>,
) {
    let dt = time.delta_secs();
    // Hit-stop: the freeze-frame (theater_tick pauses while it runs).
    if fx.hit_stop > 0.0 {
        fx.hit_stop -= dt;
    }
    let allow_motion = settings.0.battle_animations && !settings.0.reduced_motion;
    if fx.shake > 0.0 {
        fx.shake -= dt;
        if let Ok(mut node) = root.single_mut() {
            let offset = if allow_motion && fx.shake > 0.0 {
                // Deterministic-feeling jitter from the timer itself.
                let phase = (fx.shake * 90.0).sin();
                phase * fx.shake * 14.0
            } else {
                0.0
            };
            node.left = Val::Px(offset);
        }
    } else if let Ok(mut node) = root.single_mut()
        && node.left != Val::Px(0.0)
    {
        node.left = Val::Px(0.0);
    }
    // Flash: tint the struck sprite toward white, then restore. The
    // restore multiplies onto the current alpha so a faint fade or the
    // capture tint underneath survives the flash ending.
    if let Some((side, remaining)) = fx.flash {
        let remaining = remaining - dt;
        let alive = remaining > 0.0;
        let paint = |image: &mut ImageNode| {
            if alive {
                let level = 1.0 + (remaining.max(0.0) * 6.0);
                let alpha = image.color.alpha();
                image.color = Color::srgba(level, level, level, alpha);
            } else {
                let alpha = image.color.alpha();
                image.color = Color::srgba(1.0, 1.0, 1.0, alpha);
            }
        };
        if side == 1 {
            if let Ok(mut image) = foe_img.single_mut() {
                paint(&mut image);
            }
        } else if let Ok(mut image) = player_img.single_mut() {
            paint(&mut image);
        }
        fx.flash = if alive { Some((side, remaining)) } else { None };
    }
}

/// The gilt sweep (P19): the selected command breathes — a slow
/// brightness pulse over the gilt highlight battle_input painted
/// earlier this frame. Icons hide in move mode (cells hold move
/// names there).
fn command_pulse(
    time: Res<Time>,
    theme: Res<Theme>,
    cursor: Res<BattleCursor>,
    theater: Res<Theater>,
    world: Res<WorldRes>,
    mut rows: Query<(&CommandRow, &mut BackgroundColor)>,
    mut icons: Query<&mut Node, With<CommandIcon>>,
) {
    // Icons leave LAYOUT in move mode (Visibility::Hidden would keep
    // their 12px + gap as a dead indent in the move-name cells).
    let wanted = if cursor.mode == 1 {
        Display::None
    } else {
        Display::Flex
    };
    for mut node in &mut icons {
        if node.display != wanted {
            node.display = wanted;
        }
    }
    // The pulse rides only while battle_input actually paints the rows
    // — during the Shift offer the cursor indexes the BENCH, and an
    // unguarded pulse would freeze stray cells gilt.
    if !theater.idle()
        || world.0.pending_shift
        || !world.0.pending_learn_queue.is_empty()
        || !world.0.pending_evolutions.is_empty()
        || world.0.battle.is_none()
    {
        return;
    }
    let pulse = 1.0 + 0.10 * (time.elapsed_secs() * 4.0).sin();
    for (row, mut background) in &mut rows {
        if row.0 == cursor.index {
            background.0 = shade(theme.color(&theme.palette.gilt), pulse);
        }
    }
}

/// Moves displayed HP toward its target — the ~0.4s drain instead of
/// the snap. Holds during hit-stop (the freeze-frame freezes the bar).
fn hp_drain(time: Res<Time>, fx: Res<FxState>, mut disp: ResMut<DisplayedHp>) {
    if fx.hit_stop > 0.0 {
        return;
    }
    let dt = time.delta_secs();
    for (cur, target, max) in disp.sides.iter_mut().flatten() {
        if (*cur - *target).abs() < 0.01 {
            *cur = *target;
            continue;
        }
        // Proportional rate: a 35%-of-max hit drains in ~0.4s.
        let rate = (*max * 0.9).max(8.0) * dt;
        if *cur > *target {
            *cur = (*cur - rate).max(*target);
        } else {
            *cur = (*cur + rate).min(*target);
        }
    }
}

#[expect(clippy::type_complexity, reason = "bevy query filters")]
fn refresh_sprites(
    world: Res<WorldRes>,
    assets: Res<AssetServer>,
    theater: Res<Theater>,
    mut foe: Query<
        (&mut ShownSpecies, &mut ImageNode),
        (With<FoeSprite>, Without<PlayerSpriteImg>),
    >,
    mut player: Query<
        (&mut ShownSpecies, &mut ImageNode),
        (With<PlayerSpriteImg>, Without<FoeSprite>),
    >,
) {
    // The theater's SwitchIn animation owns sprite swaps; this system
    // is the idle-state safety net (e.g. animations toggled off).
    if !theater.idle() {
        return;
    }
    let Some(session) = &world.0.battle else {
        return;
    };
    if let Ok((mut shown, mut image)) = foe.single_mut() {
        let current = &session.state.sides[1].active_mote().species;
        if &shown.0 != current {
            shown.0 = current.clone();
            image.image = assets.load(game::art::art(&format!(
                "sprites/monsters/{}/{}.front.png",
                world.0.region_id, current
            )));
        }
    }
    if let Ok((mut shown, mut image)) = player.single_mut() {
        let current = &session.state.sides[0].active_mote().species;
        if &shown.0 != current {
            shown.0 = current.clone();
            image.image = assets.load(game::art::art(&format!(
                "sprites/monsters/{}/{}.back.png",
                world.0.region_id, current
            )));
        }
    }
}

type ExpBarQuery<'w, 's> =
    Query<'w, 's, &'static mut Node, (With<PlayerExpBar>, Without<FoeHpBar>, Without<PlayerHpBar>)>;

#[expect(clippy::too_many_arguments, reason = "bevy system parameters")]
fn refresh_panels(
    theme: Res<Theme>,
    world: Res<WorldRes>,
    disp: Res<DisplayedHp>,
    theater: Res<Theater>,
    mut foe_text: Query<&mut Text, (With<FoePlateText>, Without<PlayerPlateText>)>,
    mut player_text: Query<&mut Text, (With<PlayerPlateText>, Without<FoePlateText>)>,
    mut foe_bar: HpBarQuery<FoeHpBar, PlayerHpBar>,
    mut player_bar: HpBarQuery<PlayerHpBar, FoeHpBar>,
    mut exp_bar: ExpBarQuery,
) {
    // The plates paint from DisplayedHp + its labels — the THEATER's
    // view of the stage. Live session state augments it (status chip,
    // exp) only when the theater is idle: mid-replay the session is
    // already a turn ahead (the next foe's name/level/status must not
    // flash onto the dying mote's plate), and after the battle ends the
    // session is gone but the killing blow still has to land on the bar.
    let idle = theater.idle();
    let session = world.0.battle.as_ref();
    let status_chip = |side: usize| -> String {
        if !idle {
            return String::new();
        }
        session
            .and_then(|s| s.state.sides[side].active_mote().status)
            .map(|status| format!("  [{status:?}]"))
            .unwrap_or_default()
    };
    let paint = |side: usize, text: &mut Text, bar: (&mut Node, &mut BackgroundColor)| {
        let (Some((cur, _, max)), Some(label)) = (disp.sides[side], &disp.labels[side]) else {
            return;
        };
        let fraction = (cur / max.max(1.0)).clamp(0.0, 1.0);
        text.0 = format!(
            "{label}  {}/{}{}",
            cur.round() as u16,
            max.round() as u16,
            status_chip(side)
        );
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
        paint(1, &mut text, (&mut node, &mut color));
    }
    if let (Ok(mut text), Ok((mut node, mut color))) =
        (player_text.single_mut(), player_bar.single_mut())
    {
        paint(0, &mut text, (&mut node, &mut color));
    }
    if let (Some(session), Ok(mut exp_node)) = (session, exp_bar.single_mut()) {
        let mote = session.state.sides[0].active_mote();
        let floor = mote.growth.total_exp(mote.level);
        let ceil = mote
            .growth
            .total_exp(mote.level.saturating_add(1))
            .max(floor + 1);
        let fraction = (mote.exp.saturating_sub(floor)) as f32 / (ceil - floor) as f32;
        exp_node.width = Val::Percent(fraction.clamp(0.0, 1.0) * 100.0);
    }
}
