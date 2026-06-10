//! Text renderer for the battle event stream (`tools battle`).
//!
//! The same stream the Bevy presenter will animate (doc 03 §2: one sim,
//! four consumers). Mechanical ids stand in for flavored names until the
//! string tables exist.

use battle::{BattleEvent, Outcome};

pub fn render_events(events: &[BattleEvent]) -> String {
    let mut out = String::new();
    for event in events {
        out.push_str(&render_event(event));
        out.push('\n');
    }
    out
}

fn side_tag(side: u8) -> &'static str {
    if side == 0 { "[you]" } else { "[foe]" }
}

fn render_event(event: &BattleEvent) -> String {
    match event {
        BattleEvent::TurnStarted { n } => format!("―― turn {n} ――"),
        BattleEvent::SwitchedIn { side, species, .. } => {
            format!("{} {species} takes the stage", side_tag(*side))
        }
        BattleEvent::MoveUsed { side, move_id } => {
            format!("{} uses {move_id}", side_tag(*side))
        }
        BattleEvent::LastResortUsed { side } => {
            format!("{} has nothing left — last resort hum!", side_tag(*side))
        }
        BattleEvent::MoveMissed { side } => format!("{} misses!", side_tag(*side)),
        BattleEvent::MoveFailed { side } => format!("{} but it failed", side_tag(*side)),
        BattleEvent::ChargeStarted { side } => {
            format!("{} draws a deep breath…", side_tag(*side))
        }
        BattleEvent::DamageDealt {
            target,
            amount,
            crit,
            effectiveness,
        } => {
            let mut line = format!("  → {} takes {amount}", side_tag(*target));
            if *crit {
                line.push_str("  CRIT!");
            }
            match effectiveness {
                undersong_core::types::Eff::Double => line.push_str("  (resonant strike!)"),
                undersong_core::types::Eff::Half => line.push_str("  (dampened…)"),
                undersong_core::types::Eff::Zero => line.push_str("  (no effect)"),
                undersong_core::types::Eff::Neutral => {}
            }
            line
        }
        BattleEvent::MultiHit { hits } => format!("  → hit {hits} times"),
        BattleEvent::StatStageChanged {
            target,
            stat,
            delta,
            new_stage,
        } => format!(
            "  {} {stat:?} {} (now {new_stage:+})",
            side_tag(*target),
            if *delta > 0 { "rises" } else { "falls" }
        ),
        BattleEvent::StatStageClamped { target, stat } => {
            format!("  {} {stat:?} can't go further", side_tag(*target))
        }
        BattleEvent::StatusApplied { target, status } => {
            format!("  {} is {status:?}!", side_tag(*target))
        }
        BattleEvent::StatusTicked {
            target,
            status,
            damage,
        } => format!("  {} hurt by {status:?} ({damage})", side_tag(*target)),
        BattleEvent::StatusCured { target, status } => {
            format!("  {} shakes off {status:?}", side_tag(*target))
        }
        BattleEvent::ActionLost { side, status } => {
            format!("{} can't act ({status:?})", side_tag(*side))
        }
        BattleEvent::Flinched { side } => format!("{} flinched!", side_tag(*side)),
        BattleEvent::ConfusionStarted { target } => {
            format!("  {} is confused!", side_tag(*target))
        }
        BattleEvent::ConfusionEnded { target } => {
            format!("  {} snaps out of confusion", side_tag(*target))
        }
        BattleEvent::HurtItselfInConfusion { side, damage } => {
            format!("{} hurt itself in confusion ({damage})", side_tag(*side))
        }
        BattleEvent::Healed { target, amount } => {
            format!("  {} restores {amount} HP", side_tag(*target))
        }
        BattleEvent::Drained { from, amount } => {
            format!("  drains {amount} from {}", side_tag(*from))
        }
        BattleEvent::Recoiled { side, amount } => {
            format!("  {} hit by recoil ({amount})", side_tag(*side))
        }
        BattleEvent::SeededDrain { from, amount } => {
            format!("  seed saps {amount} from {}", side_tag(*from))
        }
        BattleEvent::AbilityNote { side, ability } => {
            format!("{} ability: {ability:?}", side_tag(*side))
        }
        BattleEvent::ItemNote { side, item } => {
            format!("{} item: {item:?}", side_tag(*side))
        }
        BattleEvent::WeatherChanged { kind } => match kind {
            Some(kind) => format!("  the air shifts: {kind:?}"),
            None => "  the weather settles".to_string(),
        },
        BattleEvent::WeatherChip { target, amount } => {
            format!("  weather wears {} ({amount})", side_tag(*target))
        }
        BattleEvent::Fainted { target } => format!("{} faints!", side_tag(*target)),
        BattleEvent::ExpGained { side, amount, .. } => {
            format!("  {} gains {amount} exp", side_tag(*side))
        }
        BattleEvent::LeveledUp { side, level, .. } => {
            format!("  {} reaches level {level}!", side_tag(*side))
        }
        BattleEvent::MoveLearnable { side, move_id, .. } => {
            format!("  {} wants to learn {move_id}", side_tag(*side))
        }
        BattleEvent::AttuneAttempt { rings, caught } => {
            let pulses = "♪".repeat(usize::from(*rings));
            if *caught {
                format!("the fermata settles {pulses} — attuned!")
            } else {
                format!("the bell shatters out after {pulses} ({rings} rings)")
            }
        }
        BattleEvent::EscapeAttempt { side, fled } => {
            if *fled {
                format!("{} slips away!", side_tag(*side))
            } else {
                format!("{} can't escape!", side_tag(*side))
            }
        }
        BattleEvent::BattleEnded { outcome } => match outcome {
            Outcome::Won { winner } => format!("―― {} wins ――", side_tag(*winner)),
            Outcome::Fled { .. } => "―― escaped ――".to_string(),
            Outcome::Caught => "―― caught! ――".to_string(),
            Outcome::Drawn => "―― drawn out… ――".to_string(),
        },
    }
}
