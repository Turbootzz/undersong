//! `tools music` — generated stems (doc 04 §7 source #1): ambient beds
//! and battle loops built from the cry pipeline's harmonic language.
//! Deterministic per (track, seed); WAV like the cries (doc 03 §10).
//! The Quiet Coast deliberately has no track — dead air is the point.

use std::path::Path;

use anyhow::{Context, Result};
use fundsp::wave::Wave;
use undersong_core::rng::BattleRng;

use crate::melody::midi_hz;

#[derive(Debug, Clone, Copy)]
pub enum Mood {
    /// Slow pad + sparse pentatonic walk (routes, region bed).
    Bed,
    /// Warmer, slower, rounder (towns).
    Town,
    /// Driving pulse + melody (wild battles).
    BattleWild,
    /// Brassier, denser (trainer battles).
    BattleTrainer,
    /// Stately, weighty (hall battles).
    BattleHall,
}

/// One bar of I–vi–IV–V-ish movement in A minor pentatonic space.
const PROGRESSION: [i32; 4] = [0, -3, 5, 7];

pub fn render_track(id: &str, seed: u64, mood: Mood, out_dir: &Path) -> Result<()> {
    let sample_rate = 44_100.0f64;
    let mut rng = BattleRng::from_seed(seed);
    let (bpm, bars, root_midi) = match mood {
        Mood::Bed => (84.0, 8, 57),
        Mood::Town => (72.0, 8, 60),
        Mood::BattleWild => (140.0, 8, 52),
        Mood::BattleTrainer => (152.0, 8, 50),
        Mood::BattleHall => (120.0, 8, 48),
    };
    let beat = 60.0 / bpm;
    let bar = beat * 4.0;
    let total = bar * bars as f64;

    let mut wave = Wave::new(1, sample_rate);
    let length = (total * sample_rate) as usize;

    // Pre-plan the melody: one note per beat, pentatonic walk.
    let scale = [0i32, 3, 5, 7, 10];
    let mut degree: i32 = i32::try_from(rng.below(5)).expect("0..5");
    let mut melody_plan: Vec<u8> = Vec::new();
    for _ in 0..(bars * 4) {
        let stride = if rng.chance(1, 4) { 2 } else { 1 };
        degree = (degree + if rng.chance(1, 2) { stride } else { -stride }).rem_euclid(5);
        let octave = if rng.chance(1, 5) { 12 } else { 0 };
        let midi = root_midi + 12 + scale[degree.unsigned_abs() as usize % 5] + octave;
        melody_plan.push(u8::try_from(midi.clamp(36, 96)).expect("midi"));
    }

    for index in 0..length {
        let t = index as f64 / sample_rate;
        let bar_index = ((t / bar) as usize).min(bars - 1);
        let beat_index = ((t / beat) as usize).min(bars * 4 - 1);
        let chord_root = root_midi + PROGRESSION[bar_index % 4];

        // Pad: root + fifth + octave sines, slow attack per bar.
        let bar_t = ((t % bar) / bar) as f32;
        let pad_env = (bar_t * 8.0).min(1.0) * (1.0 - bar_t * 0.25);
        let f0 = midi_hz(u8::try_from(chord_root.clamp(24, 96)).expect("midi"));
        let phase = |hz: f32| (t as f32 * hz * std::f32::consts::TAU).sin();
        let mut sample =
            (phase(f0) + 0.6 * phase(f0 * 1.5) + 0.4 * phase(f0 * 2.0)) * 0.05 * pad_env;

        // Melody voice: one note per beat, short decay.
        let beat_t = ((t % beat) / beat) as f32;
        let note_hz = midi_hz(melody_plan[beat_index]);
        let note_env = (1.0 - beat_t).powf(1.5) * (beat_t * 30.0).min(1.0);
        let voice = match mood {
            Mood::Town | Mood::Bed => phase(note_hz) * 0.05,
            Mood::BattleWild => (phase(note_hz) + 0.4 * phase(note_hz * 2.0)) * 0.06,
            Mood::BattleTrainer | Mood::BattleHall => {
                // brassy: saw-ish via odd harmonics
                (phase(note_hz) + 0.5 * phase(note_hz * 3.0) + 0.25 * phase(note_hz * 5.0)) * 0.05
            }
        };
        sample += voice * note_env;

        // Pulse for battle moods: kick-ish sine thump on the beat.
        if matches!(
            mood,
            Mood::BattleWild | Mood::BattleTrainer | Mood::BattleHall
        ) && beat_t < 0.12
        {
            let thump_env = 1.0 - beat_t / 0.12;
            sample += (t as f32 * 55.0 * std::f32::consts::TAU).sin() * 0.08 * thump_env;
        }

        wave.push(sample);
    }

    std::fs::create_dir_all(out_dir).with_context(|| format!("creating {}", out_dir.display()))?;
    let path = out_dir.join(format!("{id}.wav"));
    wave.save_wav16(&path)
        .with_context(|| format!("writing {}", path.display()))?;
    Ok(())
}

/// Short UI cues, also deterministic.
pub fn render_sfx(out_dir: &Path) -> Result<()> {
    let sample_rate = 44_100.0f64;
    let cue = |notes: &[(u8, f64)]| -> Wave {
        let mut wave = Wave::new(1, sample_rate);
        for &(midi, seconds) in notes {
            let hz = midi_hz(midi);
            let length = (seconds * sample_rate) as usize;
            for index in 0..length {
                let t = index as f64 / sample_rate;
                let env = ((1.0 - t / seconds) as f32).powf(2.0);
                let sample =
                    ((t * f64::from(hz) * std::f64::consts::TAU).sin() as f32) * 0.18 * env;
                wave.push(sample);
            }
        }
        wave
    };
    std::fs::create_dir_all(out_dir).with_context(|| format!("creating {}", out_dir.display()))?;
    let cues: [(&str, &[(u8, f64)]); 6] = [
        ("cursor", &[(76, 0.05)]),
        ("confirm", &[(72, 0.06), (79, 0.10)]),
        ("cancel", &[(67, 0.06), (60, 0.10)]),
        ("buy", &[(72, 0.05), (76, 0.05), (79, 0.12)]),
        ("heal", &[(64, 0.12), (67, 0.12), (72, 0.25)]),
        ("badge", &[(60, 0.10), (64, 0.10), (67, 0.10), (72, 0.35)]),
    ];
    for (name, notes) in cues {
        let wave = cue(notes);
        wave.save_wav16(out_dir.join(format!("{name}.wav")))
            .with_context(|| format!("writing {name}"))?;
    }
    Ok(())
}
