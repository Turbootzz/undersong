//! `tools sigils` — the launch art style (doc 04 §5): creatures rendered
//! as crystallized sound. Deterministic from `sigil_seed`; the polar
//! waveform comes from the SAME melody as the cry.
//!
//! Outputs per species: front 96×96, back 96×96 (cropped feel via zoom),
//! icon 32×32, party 48×48. Hand-drawn art in `assets/art-overrides/`
//! wins by filename convention later (doc 04 §5).

use std::path::Path;

use anyhow::{Context, Result};
use image::{Rgba, RgbaImage, imageops};
use undersong_core::rng::BattleRng;
use undersong_core::types::Type;

use crate::melody::Melody;

const SIZE: u32 = 96;

#[derive(Clone, Copy)]
enum Symmetry {
    Bilateral,
    Radial(u32),
    Broken,
}

fn symmetry_for(ty: Type) -> Symmetry {
    match ty {
        Type::Feral | Type::Alloy | Type::Stone => Symmetry::Bilateral,
        Type::Resonant | Type::Phantom => Symmetry::Radial(6),
        Type::Venom => Symmetry::Broken,
        _ => Symmetry::Bilateral,
    }
}

fn parse_hex(hex: &str) -> Rgba<u8> {
    if hex.len() < 7 || !hex.is_ascii() {
        return Rgba([125, 79, 158, 255]); // resonant violet fallback
    }
    let channel = |range| u8::from_str_radix(&hex[range], 16).unwrap_or(255);
    Rgba([channel(1..3), channel(3..5), channel(5..7), 255])
}

fn shade(color: Rgba<u8>, factor: f32) -> Rgba<u8> {
    let scale = |v: u8| -> u8 {
        let scaled = f32::from(v) * factor;
        if scaled >= 255.0 { 255 } else { scaled as u8 }
    };
    Rgba([scale(color[0]), scale(color[1]), scale(color[2]), 255])
}

/// Renders all four sprite sizes for one species.
pub fn render_sigil(
    id: &str,
    seed: u64,
    primary: Type,
    type_hex: &str,
    accent_hex: &str,
    melody: &Melody,
    out_dir: &Path,
) -> Result<()> {
    let mut rng = BattleRng::from_seed(seed);
    let base = parse_hex(type_hex);
    // 4-color ramp from the type color + one accent (doc 04 §5).
    let ramp = [
        shade(base, 0.45),
        shade(base, 0.75),
        base,
        shade(base, 1.35),
    ];
    // Accent varies with the seed (doc 04 §5): the gilt base rotated
    // through a small deterministic palette of warm offsets.
    let accent_base = parse_hex(accent_hex);
    let accent_shift = rng.below(3) as i16;
    let accent = Rgba([
        accent_base[0],
        accent_base[1].saturating_add_signed((accent_shift as i8 - 1) * 24),
        accent_base[2].saturating_add_signed((accent_shift as i8 - 1) * 18),
        255,
    ]);

    // Harmonic content from the melody: each note contributes one polar
    // harmonic — look and sound are the same data.
    let harmonics: Vec<(f32, f32, f32)> = melody
        .notes
        .iter()
        .map(|note| {
            let order = f32::from(note.midi % 7) + 1.0;
            let amplitude = 3.0 + (rng.below(70) as f32) / 10.0;
            let phase = (rng.below(628) as f32) / 100.0;
            (order, amplitude, phase)
        })
        .collect();

    let symmetry = symmetry_for(primary);
    let layers = 2 + rng.below(3);
    let mut front = RgbaImage::new(SIZE, SIZE);
    let center = SIZE as f32 / 2.0;

    for layer in (0..layers).rev() {
        let radius_base = 14.0 + 8.0 * layer as f32;
        let color = ramp[layer as usize % 4];
        for py in 0..SIZE {
            for px in 0..SIZE {
                let dx = px as f32 - center;
                let dy = py as f32 - center;
                let (dx, dy) = match symmetry {
                    Symmetry::Bilateral => (dx.abs(), dy),
                    Symmetry::Radial(n) => {
                        let angle = dy.atan2(dx);
                        let sector = std::f32::consts::TAU / n as f32;
                        let folded = angle.rem_euclid(sector) - sector / 2.0;
                        let radius = (dx * dx + dy * dy).sqrt();
                        (radius * folded.cos(), radius * folded.sin())
                    }
                    Symmetry::Broken => (dx, dy),
                };
                let radius = (dx * dx + dy * dy).sqrt();
                let theta = dy.atan2(dx);
                let mut boundary = radius_base;
                for (order, amplitude, phase) in &harmonics {
                    boundary +=
                        amplitude * (order * theta + phase).sin() / (1.0 + layer as f32 * 0.5);
                }
                if radius <= boundary {
                    front.put_pixel(px, py, color);
                }
            }
        }
    }

    // Accent ring fragment derived from the seed.
    let ring_radius = 30.0 + (rng.below(8)) as f32;
    let arc_start = (rng.below(628)) as f32 / 100.0;
    for step in 0..160 {
        let theta = arc_start + step as f32 * 0.02;
        let px = (center + ring_radius * theta.cos()) as i64;
        let py = (center + ring_radius * theta.sin()) as i64;
        if (0..i64::from(SIZE)).contains(&px) && (0..i64::from(SIZE)).contains(&py) {
            front.put_pixel(
                u32::try_from(px).expect("bounds"),
                u32::try_from(py).expect("bounds"),
                accent,
            );
        }
    }

    // The eyes rule (doc 04 §5): exactly one readable regard element —
    // walk the desired offset back toward the center until it sits on a
    // filled body pixel, so the eye never floats in space.
    let mut eye_x = (center + 8.0) as u32;
    let mut eye_y = (center - 10.0) as u32;
    while (eye_x > center as u32 || eye_y < center as u32) && front.get_pixel(eye_x, eye_y)[3] == 0
    {
        if eye_x > center as u32 {
            eye_x -= 1;
        }
        if eye_y < center as u32 {
            eye_y += 1;
        }
    }
    for (dx, dy) in [(0i32, 0i32), (1, 0), (0, 1), (1, 1)] {
        let x = eye_x.saturating_add_signed(dx);
        let y = eye_y.saturating_add_signed(dy);
        if x < SIZE && y < SIZE {
            front.put_pixel(x, y, Rgba([26, 24, 34, 255])); // ink
        }
    }
    front.put_pixel(eye_x, eye_y.saturating_sub(1), Rgba([242, 233, 216, 255])); // glint

    std::fs::create_dir_all(out_dir).with_context(|| format!("creating {}", out_dir.display()))?;
    let save = |img: &RgbaImage, suffix: &str| -> Result<()> {
        let path = out_dir.join(format!("{id}.{suffix}.png"));
        img.save(&path)
            .with_context(|| format!("writing {}", path.display()))
    };
    save(&front, "front")?;

    // Back: zoomed lower half (classic over-shoulder crop).
    let cropped = imageops::crop_imm(&front, 8, 24, 80, 64).to_image();
    let back = imageops::resize(
        &cropped,
        SIZE,
        SIZE * 64 / 80,
        imageops::FilterType::Nearest,
    );
    let mut back_canvas = RgbaImage::new(SIZE, SIZE);
    imageops::overlay(&mut back_canvas, &back, 0, 10);
    save(&back_canvas, "back")?;

    let icon = imageops::resize(&front, 32, 32, imageops::FilterType::Nearest);
    save(&icon, "icon")?;
    let party = imageops::resize(&front, 48, 48, imageops::FilterType::Nearest);
    save(&party, "party")?;
    Ok(())
}
