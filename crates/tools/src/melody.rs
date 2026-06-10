//! Shared melody derivation (doc 04 §5–§6): a species' cry phrase and its
//! sigil are the same data — both derive from the seed through this one
//! function. Protect this property; it is the signature trick.

use undersong_core::rng::BattleRng;
use undersong_core::types::Type;

/// One cry note: semitone offset within the phrase + duration.
#[derive(Debug, Clone, Copy)]
pub struct Note {
    /// MIDI note number.
    pub midi: u8,
    pub millis: u32,
}

#[derive(Debug, Clone)]
pub struct Melody {
    pub notes: Vec<Note>,
    /// Mode/timbre family from the species' primary type.
    pub timbre: Timbre,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Timbre {
    /// Brassy square/saw, staccato (ember).
    Brass,
    /// Sine/FM, legato (tide).
    Glass,
    /// Detuned, reverb-tail (phantom, venom).
    Haunt,
    /// Low percussive (stone, alloy).
    Percussive,
    /// Pure harmonics + fifth (resonant).
    Pure,
    /// Default pluck (feral, bloom, volt, gale, frost).
    Pluck,
}

impl Timbre {
    fn for_type(ty: Type) -> Timbre {
        match ty {
            Type::Ember => Timbre::Brass,
            Type::Tide => Timbre::Glass,
            Type::Phantom | Type::Venom => Timbre::Haunt,
            Type::Stone | Type::Alloy => Timbre::Percussive,
            Type::Resonant => Timbre::Pure,
            _ => Timbre::Pluck,
        }
    }
}

/// Pentatonic walk (doc 04 §6): 3–5 notes, contour from the seed, tempo
/// from base speed (faster species chirp faster), register from weight
/// (heavier = lower). `ornaments` implements the leitmotif rule: an
/// evolution line shares one seed (same contour); each later stage adds
/// that many grace notes and sits lower via its heavier weight.
pub fn melody(seed: u64, primary: Type, base_spe: u16, weight_hg: u16, ornaments: u8) -> Melody {
    let mut rng = BattleRng::from_seed(seed);
    // Pentatonic degrees over a root; minor-pentatonic for the haunted
    // timbres, major otherwise.
    let timbre = Timbre::for_type(primary);
    let scale: [i8; 5] = match timbre {
        Timbre::Haunt => [0, 3, 5, 7, 10],
        _ => [0, 2, 4, 7, 9],
    };
    // Register: midi 70 for featherweights down to ~46 for the heaviest.
    let root =
        70u8.saturating_sub(u8::try_from((u32::from(weight_hg) / 100).min(24)).expect("≤24"));
    // Tempo: 90–260 ms per note, faster with speed.
    let per_note = 260u32.saturating_sub(u32::from(base_spe).min(170)).max(90);

    let count = 3 + rng.below(3) as usize + usize::from(ornaments);
    let mut degree: i32 = i32::try_from(rng.below(5)).expect("0..5");
    let mut notes = Vec::with_capacity(count);
    for index in 0..count {
        let octave_lift = i32::try_from(rng.below(2)).expect("0..2") * 12;
        let midi =
            i32::from(root) + i32::from(scale[degree.unsigned_abs() as usize % 5]) + octave_lift;
        let lengthen = if index + 1 == count { 2 } else { 1 };
        // Grace notes (the ornament tail) play at half length.
        let shorten = if index >= count - usize::from(ornaments).min(count) && lengthen == 1 {
            2
        } else {
            1
        };
        notes.push(Note {
            midi: u8::try_from(midi.clamp(36, 96)).expect("midi range"),
            millis: per_note * lengthen / shorten,
        });
        // Walk: mostly steps, occasional leap.
        let stride = if rng.chance(1, 4) { 2 } else { 1 };
        degree += if rng.chance(1, 2) { stride } else { -stride };
        degree = degree.rem_euclid(5);
    }
    Melody { notes, timbre }
}

pub fn midi_hz(midi: u8) -> f32 {
    440.0 * 2f32.powf((f32::from(midi) - 69.0) / 12.0)
}
