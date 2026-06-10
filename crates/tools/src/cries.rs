//! `tools cries` — offline cry synthesis (doc 04 §6) with fundsp.
//!
//! Each species gets a 0.6–1.2 s phrase rendered to WAV (decision log:
//! WAV until an OGG encoder is approved). Evolution lines share a
//! leitmotif: the evolved cry is the base cry transposed down with one
//! ornament note — achieved by deriving from the PRE-evolution's seed
//! when the motif declares a `cry_parent` relationship via shared seeds
//! (content sets the same seed family; see the content generator).

use std::path::Path;

use anyhow::{Context, Result};
use fundsp::prelude32::*;

use crate::melody::{Melody, Timbre, midi_hz};

/// Renders one melody to `<out>/<id>.wav`.
pub fn render_cry(id: &str, melody: &Melody, out_dir: &Path) -> Result<()> {
    let sample_rate = 44_100.0;
    // Doc 04 §6: cries are 0.6–1.2 s. Scale the phrase uniformly into
    // the band instead of trusting per-note arithmetic.
    let mut total_ms: u32 = melody.notes.iter().map(|n| n.millis).sum();
    if total_ms == 0 {
        total_ms = 1;
    }
    let scale = (f64::from(total_ms) / 1000.0).clamp(0.6, 1.2) / (f64::from(total_ms) / 1000.0);

    let mut wave = Wave::new(1, sample_rate);
    for note in &melody.notes {
        let hz = midi_hz(note.midi);
        let seconds = f64::from(note.millis) / 1000.0 * scale;
        let mut unit: Box<dyn AudioUnit> = match melody.timbre {
            Timbre::Brass => Box::new((saw_hz(hz) * 0.30) >> lowpass_hz(hz * 3.0, 0.8)),
            Timbre::Glass => {
                Box::new((sine_hz(hz) * 0.28 + sine_hz(hz * 2.0) * 0.10) >> shape(Tanh(0.9)))
            }
            Timbre::Haunt => Box::new(
                (sine_hz(hz * 0.99) * 0.18 + sine_hz(hz * 1.01) * 0.18)
                    >> lowpass_hz(hz * 2.0, 1.0),
            ),
            Timbre::Percussive => Box::new(
                (sine_hz(hz * 0.5) * 0.4 + noise() * 0.06) >> lowpass_hz(hz.max(200.0), 0.6),
            ),
            Timbre::Pure => Box::new(sine_hz(hz) * 0.22 + sine_hz(hz * 1.5) * 0.16),
            Timbre::Pluck => Box::new((triangle_hz(hz) * 0.26) >> lowpass_hz(hz * 4.0, 0.7)),
        };
        let segment = Wave::render(sample_rate, seconds, unit.as_mut());
        // Simple attack/decay envelope applied per note, then appended.
        let len = segment.len();
        for index in 0..len {
            let t = index as f32 / len as f32;
            let attack = (t * 18.0).min(1.0);
            let decay = (1.0 - t).powf(0.6);
            let sample = segment.at(0, index) * attack * decay;
            wave.push(sample);
        }
    }
    std::fs::create_dir_all(out_dir).with_context(|| format!("creating {}", out_dir.display()))?;
    let path = out_dir.join(format!("{id}.wav"));
    wave.save_wav16(&path)
        .with_context(|| format!("writing {}", path.display()))?;
    Ok(())
}
