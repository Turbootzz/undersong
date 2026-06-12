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
    // ---- P20 story themes (doc 06) ----
    /// Sparse waltz lullaby, no drums; the lead lands a semitone above
    /// the pad's chord tones — sweet shapes in the wrong light (Lull).
    Lull,
    /// Low register, long held tones, real rests between phrases —
    /// the held rests are her signature (Vesper).
    Vesper,
    /// BattleHall escalated: heavier kick, wider voicing, for the last
    /// two Maestros (Ilva/Calder).
    HallFinal,
    /// Credits: the main theme, full band, triumphant (Chorus ending).
    CreditsFull,
    /// Credits: the main theme again, slower and sparser (Da Capo).
    CreditsSlow,
    /// Credits: near-silence, one voice carrying the melody (Tacet).
    CreditsLone,
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
    /// P20: root-fifth bass line on/off (off = the sparser credits).
    bass: bool,
    /// P20: kick weight — 0.11 is the classic kit; hall-final hits harder.
    kick_gain: f32,
    /// P20: pad voicing reaches up to the octave and tenth.
    wide_pad: bool,
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
            bass: true,
            kick_gain: 0.11,
            wide_pad: false,
        },
        Mood::Town => Plan {
            bpm: 92.0,
            bars: 16,
            root_midi: 60,
            drums: true,
            arp: false,
            brassy: false,
            intro_bar: false,
            bass: true,
            kick_gain: 0.11,
            wide_pad: false,
        },
        Mood::BattleWild => Plan {
            bpm: 148.0,
            bars: 16,
            root_midi: 52,
            drums: true,
            arp: false,
            brassy: false,
            intro_bar: true,
            bass: true,
            kick_gain: 0.11,
            wide_pad: false,
        },
        Mood::BattleTrainer => Plan {
            bpm: 156.0,
            bars: 16,
            root_midi: 50,
            drums: true,
            arp: true,
            brassy: true,
            intro_bar: true,
            bass: true,
            kick_gain: 0.11,
            wide_pad: false,
        },
        Mood::BattleHall => Plan {
            bpm: 126.0,
            bars: 16,
            root_midi: 48,
            drums: true,
            arp: true,
            brassy: true,
            intro_bar: true,
            bass: true,
            kick_gain: 0.11,
            wide_pad: false,
        },
        // P20: the hall feel escalated for the last two Maestros.
        Mood::HallFinal => Plan {
            bpm: 132.0,
            bars: 16,
            root_midi: 46,
            drums: true,
            arp: true,
            brassy: true,
            intro_bar: true,
            bass: true,
            kick_gain: 0.17,
            wide_pad: true,
        },
        // P20 credits: the main theme, full band, triumphant.
        Mood::CreditsFull => Plan {
            bpm: 118.0,
            bars: 16,
            root_midi: 60,
            drums: true,
            arp: true,
            brassy: true,
            intro_bar: false,
            bass: true,
            kick_gain: 0.13,
            wide_pad: true,
        },
        // P20 credits: the main theme again, slower and sparser —
        // pad and lead only, no kit, no bass (Da Capo).
        Mood::CreditsSlow => Plan {
            bpm: 63.0,
            bars: 16,
            root_midi: 57,
            drums: false,
            arp: false,
            brassy: false,
            intro_bar: false,
            bass: false,
            kick_gain: 0.11,
            wide_pad: false,
        },
        Mood::Lull | Mood::Vesper | Mood::CreditsLone => {
            unreachable!("sparse story moods render via story_score")
        }
    }
}

fn degree_midi(root: i32, mode: Mode, degree: i32, octave: i32) -> u8 {
    let steps = mode.steps();
    let idx = degree.rem_euclid(7) as usize;
    let wrap = degree.div_euclid(7);
    let midi = root + steps[idx] + (octave + wrap) * 12;
    u8::try_from(midi.clamp(24, 102)).expect("midi range")
}

/// One AABA section phrase as a random walk over scale degrees. Pulled
/// out of the band renderer so the credits moods can replay the SAME
/// melody from the same seed (P20). The RNG consumption here must never
/// change, or every existing stem shifts.
fn lead_phrase(rng: &mut BattleRng, wide: bool) -> Vec<i32> {
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
}

/// The A and B phrases, in the order the band renderer draws them.
fn lead_phrases(rng: &mut BattleRng) -> (Vec<i32>, Vec<i32>) {
    let a = lead_phrase(rng, false);
    let b = lead_phrase(rng, true);
    (a, b)
}

fn save_wave(wave: &Wave, id: &str, out_dir: &Path) -> Result<()> {
    std::fs::create_dir_all(out_dir).with_context(|| format!("creating {}", out_dir.display()))?;
    let path = out_dir.join(format!("{id}.wav"));
    wave.save_wav16(&path)
        .with_context(|| format!("writing {}", path.display()))
}

pub fn render_track(id: &str, seed: u64, mood: Mood, mode: Mode, out_dir: &Path) -> Result<()> {
    match mood {
        // P20 sparse story moods: planned as inspectable note lists.
        Mood::Lull | Mood::Vesper | Mood::CreditsLone => {
            save_wave(&story_wave(seed, mood, mode), id, out_dir)
        }
        _ => render_band_track(id, seed, mood, mode, out_dir),
    }
}

fn render_band_track(id: &str, seed: u64, mood: Mood, mode: Mode, out_dir: &Path) -> Result<()> {
    let sample_rate = 44_100.0f64;
    let mut rng = BattleRng::from_seed(seed);
    let p = plan(mood);
    let beat = 60.0 / p.bpm;
    let bar = beat * 4.0;
    let total = bar * p.bars as f64;

    // ---- pre-plan all voices (lead phrases per section: AABA) ----
    let (a_phrase, b_phrase) = lead_phrases(&mut rng);
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
        // (wide_pad adds the octave and tenth — hall-final / chorus credits)
        if !intro {
            let bar_t = ((t % bar) / bar) as f32;
            let env = (bar_t * 6.0).min(1.0) * (1.0 - bar_t * 0.2);
            let voicing: &[i32] = if p.wide_pad {
                &[0, 2, 4, 7, 9]
            } else {
                &[0, 2, 4]
            };
            for (i, off) in voicing.iter().enumerate() {
                let hz = midi_hz(degree_midi(p.root_midi, mode, chord_degree + off, 1));
                sample += phase(hz) * 0.030 * env / (i as f32 + 1.0).sqrt();
            }
        }

        // ---- bass: root-fifth eighths ----
        if p.bass && !intro {
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
                sample += (t as f32 * 52.0 * tau).sin() * p.kick_gain * env * env;
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

    save_wave(&wave, id, out_dir)
}

// ---------------------------------------------------------------------
// P20 sparse story moods: Lull, Vesper, the Tacet credits. These are
// planned as note lists first (so tests can READ the score — we cannot
// listen to it) and rendered second.
// ---------------------------------------------------------------------

/// One planned note, in beats from the top of the loop.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StoryNote {
    /// Onset in beats.
    pub start: f64,
    /// Length in beats.
    pub len: f64,
    pub midi: u8,
    pub gain: f32,
}

/// A planned sparse track: one lead voice plus accompaniment tones.
/// Silence is part of the score — anything not covered by a note is a
/// true rest.
pub struct StoryScore {
    pub bpm: f64,
    pub beats_per_bar: usize,
    pub bars: usize,
    pub lead: Vec<StoryNote>,
    pub pad: Vec<StoryNote>,
}

impl StoryScore {
    pub fn total_beats(&self) -> f64 {
        (self.bars * self.beats_per_bar) as f64
    }
}

/// Plans the three sparse story moods (doc 06 P20).
pub fn story_score(seed: u64, mood: Mood, mode: Mode) -> StoryScore {
    let mut rng = BattleRng::from_seed(seed);
    match mood {
        // A lullaby in 3/4 with no drums: a low cradle root and a soft
        // rocked triad each bar; the music-box lead lands a semitone
        // ABOVE a chord tone every single time. Sweet shapes, wrong
        // light (doc 01: Lull is kindly, terrifyingly persuasive).
        Mood::Lull => {
            let root = 57;
            let bars = 16;
            let mut lead = Vec::new();
            let mut pad = Vec::new();
            for bar in 0..bars {
                let prog = if bar / 4 == 2 {
                    PROGRESSION_B
                } else {
                    PROGRESSION_A
                };
                let chord = prog[bar % 4] as i32;
                let start = (bar * 3) as f64;
                // cradle: the bar's root, low, on the downbeat
                pad.push(StoryNote {
                    start,
                    len: 3.0,
                    midi: degree_midi(root, mode, chord, 0),
                    gain: 0.050,
                });
                // the rocked triad, mid register
                for (i, off) in [0i32, 2, 4].into_iter().enumerate() {
                    pad.push(StoryNote {
                        start,
                        len: 3.0,
                        midi: degree_midi(root, mode, chord + off, 1),
                        gain: 0.026 / (i as f32 + 1.0).sqrt(),
                    });
                }
                // music-box lead on beats 1 and (sometimes) 3 of the
                // waltz, each one semitone above a chord tone.
                let tone = [0i32, 2, 4][rng.below(3) as usize];
                lead.push(StoryNote {
                    start,
                    len: 1.8,
                    midi: degree_midi(root, mode, chord + tone, 2) + 1,
                    gain: 0.058,
                });
                if rng.chance(3, 5) {
                    let tone = [0i32, 2, 4][rng.below(3) as usize];
                    lead.push(StoryNote {
                        start: start + 2.0,
                        // The final bar's note may not ring past the
                        // loop seam (the stem loops via the director).
                        len: if bar + 1 == bars { 1.0 } else { 1.4 },
                        midi: degree_midi(root, mode, chord + tone, 2) + 1,
                        gain: 0.050,
                    });
                }
            }
            StoryScore {
                bpm: 76.0,
                beats_per_bar: 3,
                bars,
                lead,
                pad,
            }
        }
        // Low register, long held tones, and the held rests that are
        // her signature: each 4-bar phrase speaks twice, then beats
        // 11..16 are silence. Bare root-fifth dyads — no third, grave.
        Mood::Vesper => {
            let root = 41;
            let bars = 16;
            let mut lead = Vec::new();
            let mut pad = Vec::new();
            let low = |m: u8| if m > 60 { m - 12 } else { m };
            for (i, chord) in [0i32, 5, 3, 4].into_iter().enumerate() {
                let base = (i * 16) as f64;
                let first = chord + [0i32, 2, 4][rng.below(3) as usize];
                lead.push(StoryNote {
                    start: base,
                    len: 6.0,
                    midi: low(degree_midi(root, mode, first, 1)),
                    gain: 0.080,
                });
                // the answer leans one step away, still held
                let step = if rng.chance(1, 2) { 1 } else { -1 };
                lead.push(StoryNote {
                    start: base + 7.0,
                    len: 4.0,
                    midi: low(degree_midi(root, mode, first + step, 1)),
                    gain: 0.072,
                });
                // bare fifth under the voice; it dies before the rest does
                pad.push(StoryNote {
                    start: base,
                    len: 10.0,
                    midi: degree_midi(root, mode, chord, 0),
                    gain: 0.046,
                });
                pad.push(StoryNote {
                    start: base,
                    len: 10.0,
                    midi: degree_midi(root, mode, chord + 4, 0),
                    gain: 0.030,
                });
            }
            StoryScore {
                bpm: 58.0,
                beats_per_bar: 4,
                bars,
                lead,
                pad,
            }
        }
        // The main theme (same seed → same phrase RNG as the band) hummed
        // by ONE voice: each AABA section opens with its first bar of
        // melody at half speed, then falls silent. Near-silence is the
        // point.
        Mood::CreditsLone => {
            let (a, b) = lead_phrases(&mut rng);
            let bars = 16;
            let mut lead = Vec::new();
            for section in 0..4usize {
                let (phr, prog) = if section == 2 {
                    (&b, PROGRESSION_B)
                } else {
                    (&a, PROGRESSION_A)
                };
                let chord = prog[0] as i32;
                let base = (section * 16) as f64;
                for (k, &deg) in phr.iter().take(8).enumerate() {
                    let held = k == 7;
                    lead.push(StoryNote {
                        start: base + k as f64,
                        len: if held { 3.0 } else { 0.9 },
                        midi: degree_midi(57, mode, deg + chord, 1),
                        gain: 0.055,
                    });
                }
            }
            StoryScore {
                bpm: 66.0,
                beats_per_bar: 4,
                bars,
                lead,
                pad: Vec::new(),
            }
        }
        _ => unreachable!("band moods render via render_band_track"),
    }
}

/// Renders a [`StoryScore`]: additive sine voices per planned note with
/// per-mood envelope characters. Loop-clean by construction — every
/// envelope reaches zero at its note's end and no note crosses the
/// final bar line.
fn story_wave(seed: u64, mood: Mood, mode: Mode) -> Wave {
    let sample_rate = 44_100.0f64;
    let score = story_score(seed, mood, mode);
    let beat = 60.0 / score.bpm;
    let total = beat * score.total_beats();
    let length = (total * sample_rate) as usize;
    let tau = std::f32::consts::TAU;
    // (attack seconds, decay shape, 2nd/3rd partial weights)
    let (attack, shape, h2, h3) = match mood {
        Mood::Lull => (0.012f32, 1.6f32, 0.20f32, 0.0f32), // music box
        Mood::Vesper => (0.45, 0.7, 0.40, 0.22),           // low strings
        _ => (0.05, 1.1, 0.15, 0.0),                       // one hummed voice
    };
    let mut wave = Wave::new(1, sample_rate);
    for index in 0..length {
        let t = index as f64 / sample_rate;
        let tb = t / beat;
        let mut sample = 0.0f32;
        for n in &score.lead {
            if tb < n.start || tb >= n.start + n.len {
                continue;
            }
            let u = ((tb - n.start) / n.len) as f32;
            let el = ((tb - n.start) * beat) as f32;
            let env = (el / attack).min(1.0) * (1.0 - u).powf(shape);
            let hz = midi_hz(n.midi);
            let ph = |m: f32| (t as f32 * hz * m * tau).sin();
            sample += (ph(1.0) + h2 * ph(2.0) + h3 * ph(3.0)) * n.gain * env;
        }
        for n in &score.pad {
            if tb < n.start || tb >= n.start + n.len {
                continue;
            }
            let u = ((tb - n.start) / n.len) as f32;
            // slow swell in, fully out by the end (clean seams)
            let env = (u * 6.0).min(1.0) * (1.0 - u).powf(0.8);
            let hz = midi_hz(n.midi);
            sample += (t as f32 * hz * tau).sin() * n.gain * env;
        }
        wave.push(sample);
    }
    wave
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
    // P20: "heal" is no longer a bare cue — the nurse hums it now; see
    // render_jingles. "badge" stays as the in-world chime; the get
    // flourish is jingle_badge.
    let cues: [(&str, &[(u8, f64)]); 12] = [
        ("cursor", &[(76, 0.05)]),
        ("confirm", &[(72, 0.06), (79, 0.10)]),
        ("cancel", &[(67, 0.06), (60, 0.10)]),
        ("buy", &[(72, 0.05), (76, 0.05), (79, 0.12)]),
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

// ---------------------------------------------------------------------
// P20 jingles: one-shot phrases scored for the track voices (brass,
// soft lead, hum, bass, pad, kick) rather than the bare sine `cue`.
// ---------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
enum JingleVoice {
    /// The hall lead: odd harmonics, fast attack.
    Brass,
    /// The bed lead: sine + a little octave.
    Soft,
    /// A hummed voice: triangle-ish partials, gentle attack (the nurse).
    Hum,
    /// Square-ish low end.
    Bass,
    /// Slow-swelling sine.
    Pad,
    /// The 52 Hz kit kick (midi is ignored).
    Kick,
}

#[derive(Debug, Clone, Copy)]
struct JingleNote {
    /// Onset in beats.
    start: f64,
    /// Length in beats.
    len: f64,
    voice: JingleVoice,
    midi: u8,
    gain: f32,
}

struct JingleSpec {
    name: &'static str,
    bpm: f64,
    /// Total length in beats; nothing may ring past it.
    beats: f64,
    notes: Vec<JingleNote>,
}

fn jn(start: f64, len: f64, voice: JingleVoice, midi: u8, gain: f32) -> JingleNote {
    JingleNote {
        start,
        len,
        voice,
        midi,
        gain,
    }
}

fn pad_chord(notes: &mut Vec<JingleNote>, start: f64, len: f64, midis: &[u8], gain: f32) {
    for (i, &m) in midis.iter().enumerate() {
        notes.push(jn(
            start,
            len,
            JingleVoice::Pad,
            m,
            gain / (i as f32 + 1.0).sqrt(),
        ));
    }
}

/// All five jingle scores. `jingle_victory` loops as a post-battle
/// track via the music director, so its last figure is a turnaround
/// (G→B pickup) that resolves onto the downbeat C of the next pass,
/// and every envelope is silent by the seam.
fn jingle_specs() -> Vec<JingleSpec> {
    use JingleVoice::{Bass, Brass, Hum, Kick, Soft};
    let mut specs = Vec::new();

    // ---- jingle_victory: 4 bars, upbeat, resolves, loop-clean ----
    {
        let mut v = Vec::new();
        // melody (brass): I — IV — V — I with a turnaround pickup
        for &(start, len, midi) in &[
            (0.0, 0.45, 72u8),
            (0.5, 0.45, 76),
            (1.0, 0.95, 79),
            (2.0, 0.45, 76),
            (2.5, 0.45, 77),
            (3.0, 1.0, 79),
            (4.0, 0.7, 81),
            (5.0, 0.45, 77),
            (5.5, 0.45, 81),
            (6.0, 1.7, 84),
            (8.0, 0.45, 83),
            (8.5, 0.45, 79),
            (9.0, 0.95, 74),
            (10.0, 0.7, 79),
            (11.0, 0.7, 81),
            (12.0, 1.8, 84), // the resolve
            (14.5, 0.4, 67), // turnaround pickup…
            (15.0, 0.95, 71), // …leading tone pulls back to C at the loop
        ] {
            v.push(jn(start, len, Brass, midi, 0.10));
        }
        for &(start, midi) in &[
            (0.0, 48u8),
            (2.0, 48),
            (4.0, 53),
            (6.0, 53),
            (8.0, 55),
            (10.0, 55),
            (12.0, 48),
            (14.0, 43),
        ] {
            v.push(jn(start, 0.9, Bass, midi, 0.07));
        }
        pad_chord(&mut v, 0.0, 4.0, &[60, 64, 67], 0.040);
        pad_chord(&mut v, 4.0, 4.0, &[65, 69, 72], 0.040);
        pad_chord(&mut v, 8.0, 4.0, &[67, 71, 74], 0.040);
        pad_chord(&mut v, 12.0, 4.0, &[60, 64, 67], 0.040);
        for beat in [0.0, 2.0, 4.0, 6.0, 8.0, 10.0, 12.0, 14.0] {
            v.push(jn(beat, 0.3, Kick, 0, 0.13));
        }
        specs.push(JingleSpec {
            name: "jingle_victory",
            bpm: 148.0,
            beats: 16.0,
            notes: v,
        });
    }

    // ---- jingle_capture: ~2 bars, a settling triumphant figure ----
    {
        let mut v = vec![
            jn(0.0, 0.4, Soft, 76, 0.09),
            jn(0.5, 0.4, Soft, 79, 0.09),
            jn(1.0, 1.2, Brass, 84, 0.09),
            jn(2.5, 0.5, Soft, 81, 0.09),
            jn(3.0, 2.0, Soft, 79, 0.085),
            jn(5.0, 2.6, Hum, 72, 0.085), // the settle
            jn(0.0, 1.5, Bass, 48, 0.07),
            jn(4.0, 2.5, Bass, 48, 0.07),
            jn(0.0, 0.3, Kick, 0, 0.10),
            jn(4.0, 0.3, Kick, 0, 0.10),
        ];
        pad_chord(&mut v, 4.0, 3.6, &[60, 64, 67], 0.040);
        specs.push(JingleSpec {
            name: "jingle_capture",
            bpm: 116.0,
            beats: 8.0,
            notes: v,
        });
    }

    // ---- jingle_evolution: a rising fanfare, ~2.7 s ----
    {
        let mut v = vec![
            jn(0.0, 0.32, Brass, 60, 0.09),
            jn(0.33, 0.32, Brass, 64, 0.09),
            jn(0.66, 0.32, Brass, 67, 0.09),
            jn(1.0, 0.32, Brass, 72, 0.09),
            jn(1.33, 0.32, Brass, 76, 0.09),
            jn(1.66, 0.34, Brass, 79, 0.09),
            // the bloom
            jn(2.0, 3.6, Brass, 72, 0.07),
            jn(2.0, 3.6, Brass, 76, 0.06),
            jn(2.0, 3.6, Brass, 79, 0.055),
            jn(2.0, 3.6, Brass, 84, 0.045),
            jn(2.0, 3.5, Bass, 48, 0.075),
            jn(2.0, 0.3, Kick, 0, 0.13),
        ];
        pad_chord(&mut v, 0.0, 2.0, &[60, 64, 67], 0.035);
        specs.push(JingleSpec {
            name: "jingle_evolution",
            bpm: 132.0,
            beats: 6.0,
            notes: v,
        });
    }

    // ---- jingle_badge: the get flourish (the bare cue's four rising
    // notes, now scored: kit under, a turn on top, a full chord out) ----
    {
        let mut v = vec![
            jn(0.0, 0.7, Brass, 60, 0.10),
            jn(0.75, 0.7, Brass, 64, 0.10),
            jn(1.5, 0.7, Brass, 67, 0.10),
            jn(2.25, 1.2, Brass, 72, 0.10),
            jn(3.5, 0.3, Soft, 74, 0.08),
            jn(3.75, 0.3, Soft, 76, 0.08),
            jn(4.0, 0.5, Soft, 79, 0.08),
            jn(4.5, 0.4, Soft, 76, 0.08),
            jn(5.0, 2.8, Brass, 72, 0.07),
            jn(5.0, 2.8, Brass, 76, 0.06),
            jn(5.0, 2.8, Brass, 79, 0.055),
            jn(5.0, 2.8, Brass, 84, 0.045),
            jn(0.0, 2.0, Bass, 48, 0.06),
            jn(4.0, 1.0, Bass, 55, 0.06),
            jn(5.0, 2.8, Bass, 48, 0.075),
            jn(0.0, 0.3, Kick, 0, 0.11),
            jn(2.25, 0.3, Kick, 0, 0.11),
            jn(5.0, 0.3, Kick, 0, 0.13),
        ];
        pad_chord(&mut v, 5.0, 2.9, &[60, 64, 67], 0.035);
        specs.push(JingleSpec {
            name: "jingle_badge",
            bpm: 120.0,
            beats: 8.0,
            notes: v,
        });
    }

    // ---- heal: the nurse hums it — four warm notes, ~1.5 s ----
    {
        let v = vec![
            jn(0.0, 0.5, Hum, 64, 0.16),
            jn(0.5, 0.5, Hum, 67, 0.16),
            jn(1.0, 0.42, Hum, 69, 0.15),
            jn(1.45, 1.05, Hum, 72, 0.16),
        ];
        specs.push(JingleSpec {
            name: "heal",
            bpm: 100.0,
            beats: 2.5,
            notes: v,
        });
    }

    specs
}

fn jingle_wave(spec: &JingleSpec) -> Wave {
    let sample_rate = 44_100.0f64;
    let beat = 60.0 / spec.bpm;
    let length = (spec.beats * beat * sample_rate) as usize;
    let tau = std::f32::consts::TAU;
    let mut wave = Wave::new(1, sample_rate);
    for index in 0..length {
        let t = index as f64 / sample_rate;
        let tb = t / beat;
        let mut sample = 0.0f32;
        for n in &spec.notes {
            if tb < n.start || tb >= n.start + n.len {
                continue;
            }
            let u = ((tb - n.start) / n.len) as f32;
            let el = ((tb - n.start) * beat) as f32;
            let hz = midi_hz(n.midi);
            let ph = |m: f32| (t as f32 * hz * m * tau).sin();
            let voiced = match n.voice {
                JingleVoice::Brass => {
                    (ph(1.0) + 0.5 * ph(3.0) + 0.25 * ph(5.0))
                        * (el / 0.012).min(1.0)
                        * (1.0 - u).powf(0.6)
                }
                JingleVoice::Soft => {
                    (ph(1.0) + 0.3 * ph(2.0)) * (el / 0.02).min(1.0) * (1.0 - u).powf(1.2)
                }
                JingleVoice::Hum => {
                    (ph(1.0) - 0.11 * ph(3.0) + 0.04 * ph(5.0))
                        * (el / 0.09).min(1.0)
                        * (1.0 - u).powf(1.1)
                }
                JingleVoice::Bass => (ph(1.0) + 0.33 * ph(3.0)) * (1.0 - u).powf(0.8),
                JingleVoice::Pad => ph(1.0) * (u * 6.0).min(1.0) * (1.0 - u).powf(0.8),
                JingleVoice::Kick => {
                    let env = (1.0 - u).powf(2.0);
                    (t as f32 * 52.0 * tau).sin() * env
                }
            };
            sample += voiced * n.gain;
        }
        wave.push(sample);
    }
    wave
}

/// Renders the five P20 jingles into the sfx directory (they are
/// one-shots, not looping stems — except victory, which the director
/// loops and which is seam-clean by construction).
pub fn render_jingles(out_dir: &Path, music_dir: &Path) -> Result<()> {
    std::fs::create_dir_all(out_dir).with_context(|| format!("creating {}", out_dir.display()))?;
    std::fs::create_dir_all(music_dir)
        .with_context(|| format!("creating {}", music_dir.display()))?;
    for spec in jingle_specs() {
        let wave = jingle_wave(&spec);
        // The victory jingle LOOPS as a post-battle track through the
        // music director, so it lives with the stems; the rest are
        // one-shot cues in sfx/.
        let dir = if spec.name == "jingle_victory" {
            music_dir
        } else {
            out_dir
        };
        wave.save_wav16(dir.join(format!("{}.wav", spec.name)))
            .with_context(|| format!("writing {}", spec.name))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // The table seeds (main.rs) — repeated here so a table change that
    // would silently re-pitch a story theme fails a test instead.
    const LULL_SEED: u64 = 0xCA_0020;
    const VESPER_SEED: u64 = 0xCA_0021;
    /// cantorel_bed's seed: the region's main theme.
    const THEME_SEED: u64 = 0xCA_0001;

    #[test]
    fn lull_lead_clashes_a_semitone_against_its_own_pad() {
        let score = story_score(LULL_SEED, Mood::Lull, Mode::Ionian);
        println!("lull lead: {:?}", score.lead);
        println!("lull pad:  {:?}", score.pad);
        assert!(!score.lead.is_empty());
        for note in &score.lead {
            // Some pad tone sounding under this note sits exactly one
            // semitone (mod octaves) below it — the wrongness is real,
            // in every single lead note.
            let clash = score.pad.iter().any(|p| {
                p.start <= note.start
                    && note.start < p.start + p.len
                    && (i32::from(note.midi) - i32::from(p.midi)).rem_euclid(12) == 1
            });
            assert!(clash, "lull note {note:?} has no semitone partner");
        }
    }

    #[test]
    fn vesper_holds_long_low_tones_with_real_rests() {
        let score = story_score(VESPER_SEED, Mood::Vesper, Mode::Aeolian);
        println!("vesper lead: {:?}", score.lead);
        println!("vesper pad:  {:?}", score.pad);
        for n in &score.lead {
            assert!(n.len >= 4.0, "held tones only: {n:?}");
            assert!(n.midi <= 60, "low register only: {n:?}");
        }
        // Every 4-bar phrase ends in at least five beats of total
        // silence — the held rest.
        let all: Vec<(f64, f64)> = score
            .lead
            .iter()
            .chain(score.pad.iter())
            .map(|n| (n.start, n.start + n.len))
            .collect();
        for phrase in 0..4 {
            let lo = f64::from(phrase) * 16.0;
            let last_sound = all
                .iter()
                .filter(|(s, _)| *s >= lo && *s < lo + 16.0)
                .map(|(_, e)| *e)
                .fold(lo, f64::max);
            assert!(
                last_sound <= lo + 11.0 + 1e-9,
                "phrase {phrase} should fall silent at beat 11, sounds until {last_sound}"
            );
        }
    }

    #[test]
    fn credits_tacet_hums_the_main_theme_alone_with_long_gaps() {
        let score = story_score(THEME_SEED, Mood::CreditsLone, Mode::Ionian);
        println!("credits_tacet lead: {:?}", score.lead);
        assert!(score.pad.is_empty(), "one voice only");
        // The opening eight notes ARE the cantorel theme: the same
        // phrase walk from the same seed.
        let mut rng = BattleRng::from_seed(THEME_SEED);
        let (a, _) = lead_phrases(&mut rng);
        let expected: Vec<u8> = a
            .iter()
            .take(8)
            .map(|&d| degree_midi(57, Mode::Ionian, d, 1))
            .collect();
        let opening: Vec<u8> = score.lead.iter().take(8).map(|n| n.midi).collect();
        assert_eq!(opening, expected, "the lone voice must carry the theme");
        // Long gaps: silence between hummed fragments.
        let end_of_first = score
            .lead
            .iter()
            .filter(|n| n.start < 16.0)
            .map(|n| n.start + n.len)
            .fold(0.0, f64::max);
        let start_of_second = score
            .lead
            .iter()
            .map(|n| n.start)
            .filter(|s| *s >= 16.0)
            .fold(f64::INFINITY, f64::min);
        assert!(
            start_of_second - end_of_first >= 4.0,
            "expected a long gap, got {:.1} beats",
            start_of_second - end_of_first
        );
    }

    #[test]
    fn story_scores_are_deterministic() {
        for (seed, mood, mode) in [
            (LULL_SEED, Mood::Lull, Mode::Ionian),
            (VESPER_SEED, Mood::Vesper, Mode::Aeolian),
            (THEME_SEED, Mood::CreditsLone, Mode::Ionian),
        ] {
            let a = story_score(seed, mood, mode);
            let b = story_score(seed, mood, mode);
            assert_eq!(a.lead, b.lead);
            assert_eq!(a.pad, b.pad);
        }
    }

    #[test]
    fn jingles_have_sane_lengths_and_peaks() {
        for spec in jingle_specs() {
            let wave = jingle_wave(&spec);
            let seconds = wave.len() as f64 / wave.sample_rate();
            let amp = wave.amplitude();
            assert!(amp <= 0.9, "{} peaks at {amp}", spec.name);
            assert!(amp >= 0.05, "{} is near-silent ({amp})", spec.name);
            let ok = match spec.name {
                "jingle_victory" => (6.0..7.5).contains(&seconds),
                "jingle_capture" => (3.5..4.8).contains(&seconds),
                "jingle_evolution" => (2.0..3.0).contains(&seconds),
                "jingle_badge" => (3.0..4.5).contains(&seconds),
                "heal" => (1.2..1.8).contains(&seconds),
                other => panic!("unexpected jingle {other}"),
            };
            assert!(ok, "{} is {seconds:.2}s", spec.name);
        }
    }

    #[test]
    fn victory_jingle_is_seam_clean_for_looping() {
        let specs = jingle_specs();
        let victory = specs
            .iter()
            .find(|s| s.name == "jingle_victory")
            .expect("victory exists");
        for n in &victory.notes {
            assert!(
                n.start + n.len <= victory.beats + 1e-9,
                "note rings past the loop seam: {n:?}"
            );
        }
    }
}
