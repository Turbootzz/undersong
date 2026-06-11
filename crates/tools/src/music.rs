//! `tools music` — generated stems v2 (doc 06 P11): drums, bass, pads,
//! a lead with an AABA song shape, and per-region modes (Cantorel sings
//! ionian; Skalden goes dorian/aeolian — the folk shade). Deterministic
//! per (track, seed); WAV like the cries (doc 03 §10).
//! The Quiet Coast deliberately has no track — dead air is the point.

use std::path::Path;

use anyhow::{Context, Result};
use fundsp::wave::Wave;
use undersong_core::rng::BattleRng;

use crate::melody::midi_hz;

#[derive(Debug, Clone, Copy)]
pub enum Mood {
    /// Slow pad + sparse lead, no drums (routes, region bed).
    Bed,
    /// Warm: pad + bass + lead + a light hat (towns).
    Town,
    /// Full kit, driving (wild battles).
    BattleWild,
    /// Full kit + arpeggio voice (trainer battles).
    BattleTrainer,
    /// Heavy kick, stately tempo, brassy lead (hall battles).
    BattleHall,
}

#[derive(Debug, Clone, Copy)]
pub enum Mode {
    Ionian,
    Dorian,
    Aeolian,
}

impl Mode {
    fn steps(self) -> [i32; 7] {
        match self {
            Mode::Ionian => [0, 2, 4, 5, 7, 9, 11],
            Mode::Dorian => [0, 2, 3, 5, 7, 9, 10],
            Mode::Aeolian => [0, 2, 3, 5, 7, 8, 10],
        }
    }
}

/// Degree-built triads over an AABA progression (I–vi–IV–V flavored,
/// expressed in scale degrees so every mode harmonizes itself).
const PROGRESSION_A: [usize; 4] = [0, 5, 3, 4];
const PROGRESSION_B: [usize; 4] = [3, 4, 0, 4];

struct Plan {
    bpm: f64,
    bars: usize,
    root_midi: i32,
    drums: bool,
    arp: bool,
    brassy: bool,
    intro_bar: bool,
}

fn plan(mood: Mood) -> Plan {
    match mood {
        Mood::Bed => Plan {
            bpm: 82.0,
            bars: 16,
            root_midi: 57,
            drums: false,
            arp: false,
            brassy: false,
            intro_bar: false,
        },
        Mood::Town => Plan {
            bpm: 92.0,
            bars: 16,
            root_midi: 60,
            drums: true,
            arp: false,
            brassy: false,
            intro_bar: false,
        },
        Mood::BattleWild => Plan {
            bpm: 148.0,
            bars: 16,
            root_midi: 52,
            drums: true,
            arp: false,
            brassy: false,
            intro_bar: true,
        },
        Mood::BattleTrainer => Plan {
            bpm: 156.0,
            bars: 16,
            root_midi: 50,
            drums: true,
            arp: true,
            brassy: true,
            intro_bar: true,
        },
        Mood::BattleHall => Plan {
            bpm: 126.0,
            bars: 16,
            root_midi: 48,
            drums: true,
            arp: true,
            brassy: true,
            intro_bar: true,
        },
    }
}

fn degree_midi(root: i32, mode: Mode, degree: i32, octave: i32) -> u8 {
    let steps = mode.steps();
    let idx = degree.rem_euclid(7) as usize;
    let wrap = degree.div_euclid(7);
    let midi = root + steps[idx] + (octave + wrap) * 12;
    u8::try_from(midi.clamp(24, 102)).expect("midi range")
}

pub fn render_track(id: &str, seed: u64, mood: Mood, mode: Mode, out_dir: &Path) -> Result<()> {
    let sample_rate = 44_100.0f64;
    let mut rng = BattleRng::from_seed(seed);
    let p = plan(mood);
    let beat = 60.0 / p.bpm;
    let bar = beat * 4.0;
    let total = bar * p.bars as f64;

    // ---- pre-plan all voices (lead phrases per section: AABA) ----
    let phrase = |rng: &mut BattleRng, wide: bool| -> Vec<i32> {
        let mut degree: i32 = i32::try_from(rng.below(7)).expect("0..7");
        (0..16)
            .map(|_| {
                let stride = if rng.chance(1, if wide { 3 } else { 5 }) {
                    2
                } else {
                    1
                };
                degree += if rng.chance(1, 2) { stride } else { -stride };
                degree = degree.clamp(-2, 9);
                degree
            })
            .collect()
    };
    let a_phrase = phrase(&mut rng, false);
    let b_phrase = phrase(&mut rng, true);
    let section = |bar_index: usize| -> (&Vec<i32>, usize) {
        // AABA over 16 bars: 4-bar sections.
        match bar_index / 4 {
            2 => (&b_phrase, bar_index % 4),
            _ => (&a_phrase, bar_index % 4),
        }
    };

    let mut wave = Wave::new(1, sample_rate);
    let length = (total * sample_rate) as usize;
    let tau = std::f32::consts::TAU;

    // noise state for drums (deterministic LCG)
    let mut noise_state: u32 = (seed as u32) | 1;
    let mut noise = move || -> f32 {
        noise_state = noise_state
            .wrapping_mul(1_664_525)
            .wrapping_add(1_013_904_223);
        (noise_state >> 16) as f32 / 32_768.0 - 1.0
    };

    for index in 0..length {
        let t = index as f64 / sample_rate;
        let bar_index = ((t / bar) as usize).min(p.bars - 1);
        let beat_in_bar = (t % bar) / beat; // 0.0..4.0
        let beat_t = (beat_in_bar.fract()) as f32;
        let eighth_t = ((beat_in_bar * 2.0).fract()) as f32;
        let intro = p.intro_bar && bar_index == 0;

        let (phr, bar_in_section) = section(bar_index);
        let prog = if bar_index / 4 == 2 {
            PROGRESSION_B
        } else {
            PROGRESSION_A
        };
        let chord_degree = prog[bar_in_section] as i32;

        let phase = |hz: f32| (t as f32 * hz * tau).sin();
        let mut sample = 0.0f32;

        // ---- pad: triad over the chord degree, slow swell ----
        if !intro {
            let bar_t = ((t % bar) / bar) as f32;
            let env = (bar_t * 6.0).min(1.0) * (1.0 - bar_t * 0.2);
            for (i, off) in [0i32, 2, 4].iter().enumerate() {
                let hz = midi_hz(degree_midi(p.root_midi, mode, chord_degree + off, 1));
                sample += phase(hz) * 0.030 * env / (i as f32 + 1.0).sqrt();
            }
        }

        // ---- bass: root-fifth eighths ----
        if !intro {
            let eighth = (beat_in_bar * 2.0) as usize % 8;
            let deg = if eighth % 4 == 2 {
                chord_degree + 4
            } else {
                chord_degree
            };
            let hz = midi_hz(degree_midi(p.root_midi, mode, deg, 0));
            let env = (1.0 - eighth_t).powf(0.8);
            // square-ish: odd harmonics
            sample += (phase(hz) + 0.33 * phase(hz * 3.0)) * 0.055 * env;
        }

        // ---- lead: one note per half-beat from the section phrase ----
        if !intro {
            let slot = ((beat_in_bar * 2.0) as usize + bar_in_section * 8) % 16;
            let deg = phr[slot] + chord_degree;
            let hz = midi_hz(degree_midi(p.root_midi, mode, deg, 2));
            let env = (1.0 - eighth_t).powf(1.4) * (eighth_t * 24.0).min(1.0);
            let voice = if p.brassy {
                (phase(hz) + 0.5 * phase(hz * 3.0) + 0.25 * phase(hz * 5.0)) * 0.045
            } else {
                (phase(hz) + 0.3 * phase(hz * 2.0)) * 0.05
            };
            sample += voice * env;
        }

        // ---- arpeggio sparkle (trainer/hall) ----
        if p.arp && !intro {
            let slot = (beat_in_bar * 4.0) as i32 % 4;
            let hz = midi_hz(degree_midi(p.root_midi, mode, chord_degree + slot * 2, 3));
            let st = ((beat_in_bar * 4.0).fract()) as f32;
            sample += phase(hz) * 0.018 * (1.0 - st).powf(2.0);
        }

        // ---- drums ----
        if p.drums {
            let beat_idx = beat_in_bar as usize % 4;
            // kick on 1 and 3 (every beat in the intro roll)
            let kick_on = beat_idx.is_multiple_of(2) || intro;
            if kick_on && beat_t < 0.10 {
                let env = 1.0 - beat_t / 0.10;
                sample += (t as f32 * 52.0 * tau).sin() * 0.11 * env * env;
            }
            // snare on 2 and 4
            if !beat_idx.is_multiple_of(2) && beat_t < 0.08 && !intro {
                let env = 1.0 - beat_t / 0.08;
                sample += noise() * 0.06 * env;
            }
            // hats on the eighths (town keeps just these)
            if eighth_t < 0.03 {
                let env = 1.0 - eighth_t / 0.03;
                sample += noise() * 0.022 * env;
            }
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
    let cues: [(&str, &[(u8, f64)]); 13] = [
        ("cursor", &[(76, 0.05)]),
        ("confirm", &[(72, 0.06), (79, 0.10)]),
        ("cancel", &[(67, 0.06), (60, 0.10)]),
        ("buy", &[(72, 0.05), (76, 0.05), (79, 0.12)]),
        ("heal", &[(64, 0.12), (67, 0.12), (72, 0.25)]),
        ("badge", &[(60, 0.10), (64, 0.10), (67, 0.10), (72, 0.35)]),
        // P17 battle theater (doc 06): capture, faint, typewriter.
        ("bell", &[(88, 0.20), (95, 0.35)]),
        ("wobble", &[(55, 0.14)]),
        ("settle", &[(84, 0.07), (88, 0.07), (91, 0.22)]),
        ("breakout", &[(70, 0.06), (58, 0.16)]),
        ("faint", &[(48, 0.10), (41, 0.28)]),
        ("blip", &[(81, 0.025)]),
        // P18 "spotted!" — generated now so the asset exists.
        ("alert", &[(76, 0.07), (83, 0.12)]),
    ];
    for (name, notes) in cues {
        let wave = cue(notes);
        wave.save_wav16(out_dir.join(format!("{name}.wav")))
            .with_context(|| format!("writing {name}"))?;
    }
    Ok(())
}
