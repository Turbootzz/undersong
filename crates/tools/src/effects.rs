//! `tools sprites` — battle impact strips (doc 06 P17). One 4-frame
//! 32×32 animation per creature type, stamped 2× scale over the target
//! when a move lands. Same laws as the rest of the art engine:
//! deterministic (fixed-seed `BattleRng` only), ink outlines where they
//! aid readability, and the palette's exact per-type inks
//! (`content/core/palette.ron` §type_colors).
//! Output: `assets/sprites/fx/<type>.<frame>.png`, frames `0..=3`
//! animating anticipation → peak → peak → decay.

use std::path::Path;

use anyhow::{Context, Result};
use image::{Rgba, RgbaImage};
use undersong_core::rng::BattleRng;
use undersong_core::types::Type;

const SIZE: i32 = 32;
const FRAMES: usize = 4;
const INK: Rgba<u8> = Rgba([26, 24, 34, 255]);

fn hex(rgb: u32) -> Rgba<u8> {
    Rgba([
        u8::try_from((rgb >> 16) & 0xff).expect("byte"),
        u8::try_from((rgb >> 8) & 0xff).expect("byte"),
        u8::try_from(rgb & 0xff).expect("byte"),
        255,
    ])
}

/// Multiplicative shade, clamped (the sprites.rs ramp trick).
fn shade(c: Rgba<u8>, f: f32) -> Rgba<u8> {
    let s = |v: u8| -> u8 {
        let scaled = f32::from(v) * f;
        if scaled >= 255.0 { 255 } else { scaled as u8 }
    };
    Rgba([s(c[0]), s(c[1]), s(c[2]), c[3]])
}

/// Mix toward white — highlight tones for the dark palette inks.
fn lighten(c: Rgba<u8>, f: f32) -> Rgba<u8> {
    let mix = |v: u8| -> u8 {
        let v = f32::from(v);
        (v + (255.0 - v) * f) as u8
    };
    Rgba([mix(c[0]), mix(c[1]), mix(c[2]), c[3]])
}

fn with_alpha(c: Rgba<u8>, a: u8) -> Rgba<u8> {
    Rgba([c[0], c[1], c[2], a])
}

/// The exact per-type inks of `content/core/palette.ron` §type_colors.
fn type_color(t: Type) -> Rgba<u8> {
    match t {
        Type::Feral => hex(0x6b5d45),
        Type::Ember => hex(0xa83c1e),
        Type::Tide => hex(0x1f5f8f),
        Type::Bloom => hex(0x3d7a33),
        Type::Volt => hex(0x8f7411),
        Type::Gale => hex(0x3f7d80),
        Type::Stone => hex(0x6e5849),
        Type::Frost => hex(0x3b6d96),
        Type::Venom => hex(0x6d3f7e),
        Type::Phantom => hex(0x54467c),
        Type::Alloy => hex(0x5c6066),
        Type::Resonant => hex(0x8a3d6b),
    }
}

// ----- drawing kit ---------------------------------------------------------

fn px(img: &mut RgbaImage, x: i32, y: i32, c: Rgba<u8>) {
    if (0..SIZE).contains(&x) && (0..SIZE).contains(&y) {
        img.put_pixel(x as u32, y as u32, c);
    }
}

fn disc(img: &mut RgbaImage, cx: i32, cy: i32, r: i32, c: Rgba<u8>) {
    for dy in -r..=r {
        for dx in -r..=r {
            if dx * dx + dy * dy <= r * r {
                px(img, cx + dx, cy + dy, c);
            }
        }
    }
}

/// A ~1px circle band; unequal `rx`/`ry` squash it into an ellipse, and
/// `dashed` breaks it into ghostly segments.
fn ring(img: &mut RgbaImage, cx: i32, cy: i32, rx: f32, ry: f32, c: Rgba<u8>, dashed: bool) {
    let band = 0.8 / rx.min(ry);
    for y in 0..SIZE {
        for x in 0..SIZE {
            let dx = (x - cx) as f32 / rx;
            let dy = (y - cy) as f32 / ry;
            let d = (dx * dx + dy * dy).sqrt();
            if (d - 1.0).abs() <= band {
                if dashed && (x * 3 + y * 7) % 5 < 2 {
                    continue;
                }
                px(img, x, y, c);
            }
        }
    }
}

/// Thick parametric line; `hw` is the half-width in pixels (0 = 1px).
fn stroke(img: &mut RgbaImage, a: (f32, f32), b: (f32, f32), hw: i32, c: Rgba<u8>) {
    let steps = ((b.0 - a.0).abs().max((b.1 - a.1).abs()) * 2.0) as i32 + 1;
    for s in 0..=steps {
        let t = s as f32 / steps as f32;
        let x = (a.0 + (b.0 - a.0) * t).round() as i32;
        let y = (a.1 + (b.1 - a.1) * t).round() as i32;
        if hw == 0 {
            px(img, x, y, c);
        } else {
            disc(img, x, y, hw, c);
        }
    }
}

/// Ink edge around fully opaque regions only — translucent glow is left
/// alone (sprites.rs outlines everything; effects need restraint).
fn outline_solid(img: &mut RgbaImage) {
    let mut edges = Vec::new();
    for y in 0..SIZE {
        for x in 0..SIZE {
            if img.get_pixel(x as u32, y as u32)[3] != 255 {
                continue;
            }
            let open = [(1i32, 0i32), (-1, 0), (0, 1), (0, -1)]
                .iter()
                .any(|(dx, dy)| {
                    let (nx, ny) = (x + dx, y + dy);
                    nx < 0
                        || ny < 0
                        || nx >= SIZE
                        || ny >= SIZE
                        || img.get_pixel(nx as u32, ny as u32)[3] < 128
                });
            if open {
                edges.push((x as u32, y as u32));
            }
        }
    }
    for (x, y) in edges {
        img.put_pixel(x, y, INK);
    }
}

// ----- the twelve impacts --------------------------------------------------

/// Feral — a plain diagonal slash, drawn like an inked brush stroke.
fn feral(img: &mut RgbaImage, c: Rgba<u8>, f: usize) {
    let reach = [0.45f32, 1.0, 1.0, 1.0][f];
    let a = (25.0f32, 5.0f32);
    let tip = (25.0 - 19.0 * reach, 5.0 + 23.0 * reach);
    if f < 3 {
        stroke(img, a, tip, 2, INK);
        stroke(img, a, tip, 1, c);
        stroke(img, a, tip, 0, lighten(c, 0.65));
        if f > 0 {
            // peak: a second, shorter cut trailing the first
            stroke(img, (28.0, 12.0), (14.0, 28.0), 0, with_alpha(c, 180));
        }
    } else {
        // decay: the cut hangs as a thin afterimage
        stroke(img, a, tip, 0, with_alpha(c, 110));
        stroke(img, (28.0, 12.0), (14.0, 28.0), 0, with_alpha(c, 70));
    }
}

/// Ember — a flame burst that collapses into drifting embers.
fn ember(img: &mut RgbaImage, c: Rgba<u8>, f: usize) {
    let light = lighten(c, 0.55);
    let mut rng = BattleRng::from_seed(0xE3B0 ^ (f as u64 * 0x9e37));
    if f < 3 {
        let h = [8i32, 16, 19][f];
        let base_r = [3.0f32, 5.0, 5.5][f];
        let foot = 27i32;
        for i in 0..h {
            let t = i as f32 / h as f32;
            let r = (base_r * (1.0 - t * t)).round() as i32;
            let flicker = if t > 0.45 { rng.below(3) as i32 - 1 } else { 0 };
            disc(img, 16 + flicker, foot - i, r.max(0), c);
        }
        // the hot core
        let core_h = h * 3 / 5;
        for i in 0..core_h {
            let t = i as f32 / core_h as f32;
            let r = (base_r * 0.55 * (1.0 - t)).round() as i32;
            disc(img, 16, foot - 1 - i, r.max(0), light);
        }
        outline_solid(img);
    } else {
        // burst: embers scatter upward and cool
        for _ in 0..8 {
            let x = 8 + rng.below(17) as i32;
            let y = 5 + rng.below(18) as i32;
            px(img, x, y, with_alpha(light, 180));
            px(img, x, y + 1, with_alpha(c, 140));
        }
    }
}

/// Tide — a splash arc of droplets: rise, crest, rain back down.
fn tide(img: &mut RgbaImage, c: Rgba<u8>, f: usize) {
    let light = lighten(c, 0.55);
    let r = [5.0f32, 10.0, 13.0, 13.0][f];
    let alpha = [255u8, 255, 255, 130][f];
    let fall = [0i32, 0, 1, 5][f];
    let dr = [1i32, 2, 2, 1][f];
    for k in 0..=10i32 {
        let th = std::f32::consts::PI * k as f32 / 10.0;
        let x = (16.0 + th.cos() * r).round() as i32;
        let jitter = (k % 3 - 1) * i32::from(f == 3);
        let y = (26.0 - th.sin() * r * 0.95).round() as i32 + fall + jitter;
        disc(img, x, y, dr, with_alpha(c, alpha));
        px(img, x - 1, y - 1, with_alpha(light, alpha));
    }
    if f < 3 {
        // the swell at the impact point
        let mr = [2i32, 3, 4][f];
        disc(img, 16, 28, mr, c);
        disc(img, 16, 27, mr - 1, light);
    }
}

/// Bloom — petal puffs blowing outward from the hit.
fn bloom(img: &mut RgbaImage, c: Rgba<u8>, f: usize) {
    let light = lighten(c, 0.5);
    let dist = [3.0f32, 6.5, 8.5, 11.5][f];
    let pr = [2i32, 3, 3, 2][f];
    let alpha = [255u8, 255, 255, 110][f];
    for k in 0..6 {
        let th = std::f32::consts::TAU * (k as f32 + 0.25) / 6.0;
        let x = (16.0 + th.cos() * dist).round() as i32;
        let y = (15.0 + th.sin() * dist * 0.9).round() as i32;
        disc(img, x, y, pr, with_alpha(c, alpha));
        disc(img, x - 1, y - 1, (pr - 1).max(0), with_alpha(light, alpha));
    }
    if f < 3 {
        disc(img, 16, 15, 1, lighten(c, 0.8));
        outline_solid(img);
    }
}

/// Volt — the jagged flash, ink-edged with a hot core.
fn volt(img: &mut RgbaImage, c: Rgba<u8>, f: usize) {
    let path: [(f32, f32); 6] = [
        (14.0, 1.0),
        (19.0, 8.0),
        (12.0, 13.0),
        (19.0, 19.0),
        (13.0, 25.0),
        (17.0, 31.0),
    ];
    let segs = [2usize, 5, 5, 5][f];
    let white = lighten(c, 0.85);
    if f == 1 || f == 2 {
        // the flash behind the bolt
        disc(img, 16, 14, [0, 7, 9, 0][f], with_alpha(lighten(c, 0.35), 60));
    }
    if f < 3 {
        for pair in path.windows(2).take(segs) {
            stroke(img, pair[0], pair[1], 1, INK);
        }
        for pair in path.windows(2).take(segs) {
            stroke(img, pair[0], pair[1], 0, white);
        }
        if f == 0 {
            // anticipation: the leader spark
            disc(img, path[2].0 as i32, path[2].1 as i32, 1, white);
        }
    } else {
        // decay: a thin afterimage and two stray sparks
        for pair in path.windows(2) {
            stroke(img, pair[0], pair[1], 0, with_alpha(c, 120));
        }
        for (sx, sy) in [(22i32, 9i32), (10, 22)] {
            px(img, sx, sy, with_alpha(white, 120));
            px(img, sx + 1, sy, with_alpha(c, 100));
            px(img, sx - 1, sy, with_alpha(c, 100));
        }
    }
}

/// Gale — wind streaks raking across the target.
fn gale(img: &mut RgbaImage, c: Rgba<u8>, f: usize) {
    let light = lighten(c, 0.55);
    let head = [10i32, 19, 27, 31][f];
    let len = [7i32, 12, 14, 10][f];
    let alpha = [255u8, 255, 255, 120][f];
    for (k, y) in [6i32, 12, 19, 25].into_iter().enumerate() {
        let stagger = (k as i32 * 5) % 7 - 3;
        let hx = head + stagger;
        for i in 0..len {
            let col = if i < len / 3 { light } else { c };
            px(img, hx - i, y, with_alpha(col, alpha));
            if i < len / 2 {
                px(img, hx - i, y + 1, with_alpha(c, alpha.saturating_sub(80)));
            }
        }
        if f == 1 || f == 2 {
            disc(img, hx, y, 1, with_alpha(light, alpha));
        }
    }
}

/// Stone — shards dropping onto the target, dust on the landing.
fn stone(img: &mut RgbaImage, c: Rgba<u8>, f: usize) {
    let light = lighten(c, 0.45);
    let dark = shade(c, 0.7);
    if f < 3 {
        let top = [2i32, 8, 15][f];
        for (k, &(cx, w, h)) in [(9i32, 5i32, 7i32), (16, 7, 9), (23, 5, 6)].iter().enumerate() {
            let y0 = top + (k as i32 * 3) % 5 - 2;
            for row in 0..h {
                let half = (w / 2) * (h - row) / h;
                for dx in -half..=half {
                    let col = if dx < 0 {
                        light
                    } else if dx > half / 2 {
                        dark
                    } else {
                        c
                    };
                    px(img, cx + dx, y0 + row, col);
                }
            }
        }
        outline_solid(img);
    } else {
        // impact: dust puffs and settled rubble
        let mut rng = BattleRng::from_seed(0x5709);
        for gx in [7i32, 15, 23] {
            disc(img, gx, 26, 2, with_alpha(c, 120));
            disc(img, gx - 2, 24, 1, with_alpha(light, 100));
        }
        for _ in 0..6 {
            let x = 5 + rng.below(22) as i32;
            let y = 22 + rng.below(7) as i32;
            px(img, x, y, with_alpha(dark, 160));
        }
    }
}

/// Frost — a six-armed crystal with glints winking around it.
fn frost(img: &mut RgbaImage, c: Rgba<u8>, f: usize) {
    let light = lighten(c, 0.6);
    let white = lighten(c, 0.9);
    let len = [4.0f32, 8.0, 10.0, 10.0][f];
    let alpha = [255u8, 255, 255, 110][f];
    for k in 0..6 {
        let th = std::f32::consts::TAU * k as f32 / 6.0 + 0.26;
        let (dx, dy) = (th.cos(), th.sin());
        let tip = (16.0 + dx * len, 15.0 + dy * len);
        stroke(img, (16.0, 15.0), tip, 0, with_alpha(light, alpha));
        if f > 0 {
            // side ticks at 60% reach
            let (tx, ty) = (16.0 + dx * len * 0.6, 15.0 + dy * len * 0.6);
            px(img, (tx - dy).round() as i32, (ty + dx).round() as i32, with_alpha(c, alpha));
            px(img, (tx + dy).round() as i32, (ty - dx).round() as i32, with_alpha(c, alpha));
        }
    }
    disc(img, 16, 15, 1, with_alpha(white, alpha));
    // glints
    let spots = [(26i32, 6i32), (6, 10), (25, 23), (9, 26)];
    let shown = [0usize, 2, 4, 4][f];
    let ga = [0u8, 255, 255, 150][f];
    for &(gx, gy) in spots.iter().take(shown) {
        px(img, gx, gy, with_alpha(white, ga));
        for (ox, oy) in [(1i32, 0i32), (-1, 0), (0, 1), (0, -1)] {
            px(img, gx + ox, gy + oy, with_alpha(light, ga));
        }
    }
}

/// Venom — bubbles swelling up, then bursting into drips.
fn venom(img: &mut RgbaImage, c: Rgba<u8>, f: usize) {
    let light = lighten(c, 0.55);
    if f < 3 {
        let grow = [0.55f32, 0.8, 1.0][f];
        let rise = [3i32, 1, 0][f];
        for &(bx, by, br) in &[(12i32, 17i32, 4.0f32), (21, 12, 3.0), (15, 7, 2.0)] {
            let r = (br * grow).round();
            if r < 1.0 {
                continue;
            }
            ring(img, bx, by + rise, r, r, c, false);
            // the glint inside the film
            px(
                img,
                bx - (r * 0.4) as i32,
                by + rise - (r * 0.5) as i32,
                light,
            );
        }
    } else {
        // pop: each bubble leaves a running drip
        for &(bx, top, bot) in &[(12i32, 17i32, 27i32), (21, 13, 23), (15, 9, 16)] {
            for y in top..=bot {
                px(img, bx, y, with_alpha(c, 150));
            }
            disc(img, bx, bot + 1, 1, with_alpha(c, 190));
            px(img, bx, top, with_alpha(light, 150));
        }
    }
}

/// Phantom — broken ripples spreading and thinning to nothing.
fn phantom(img: &mut RgbaImage, c: Rgba<u8>, f: usize) {
    let sets: [&[(f32, u8)]; FRAMES] = [
        &[(4.0, 230)],
        &[(7.0, 200), (3.5, 230)],
        &[(10.5, 150), (6.5, 200)],
        &[(13.5, 90), (9.5, 130)],
    ];
    for &(r, a) in sets[f] {
        ring(img, 16, 16, r * 1.25, r * 0.8, with_alpha(c, a), true);
    }
    // the wisp at the heart, drifting upward as it fades
    let wa = [220u8, 170, 110, 60][f];
    disc(img, 16, 15 - f as i32, 2, with_alpha(lighten(c, 0.45), wa));
}

/// Alloy — the clang: a metallic spark star inside an expanding ring.
fn alloy(img: &mut RgbaImage, c: Rgba<u8>, f: usize) {
    let white = lighten(c, 0.8);
    let light = lighten(c, 0.5);
    if f < 3 {
        let spike = [3.0f32, 7.0, 9.0][f];
        for k in 0..4 {
            let th = std::f32::consts::TAU * k as f32 / 4.0;
            let tip = (16.0 + th.cos() * spike, 15.0 + th.sin() * spike);
            stroke(img, (16.0, 15.0), tip, 0, white);
            let th2 = th + std::f32::consts::FRAC_PI_4;
            let d = spike * 0.6;
            let tip2 = (16.0 + th2.cos() * d, 15.0 + th2.sin() * d);
            stroke(img, (16.0, 15.0), tip2, 0, light);
        }
        px(img, 16, 15, lighten(c, 0.95));
        if f > 0 {
            let r = [0.0f32, 6.0, 10.0][f];
            ring(img, 16, 15, r, r, with_alpha(c, 230), false);
        }
    } else {
        // decay: the ring rolls outward; the spark is spent
        ring(img, 16, 15, 13.0, 13.0, with_alpha(c, 110), false);
        for k in 0..4 {
            let th = std::f32::consts::TAU * k as f32 / 4.0;
            px(
                img,
                (16.0 + th.cos() * 9.0).round() as i32,
                (15.0 + th.sin() * 9.0).round() as i32,
                with_alpha(light, 140),
            );
        }
    }
}

/// Resonant — clean concentric rings, the note rippling outward.
fn resonant(img: &mut RgbaImage, c: Rgba<u8>, f: usize) {
    let light = lighten(c, 0.55);
    let sets: [&[f32]; FRAMES] = [&[3.0], &[4.0, 8.0], &[5.0, 9.0, 12.5], &[9.0, 13.5]];
    let alphas: [&[u8]; FRAMES] = [&[255], &[230, 180], &[230, 180, 140], &[120, 80]];
    for (i, &r) in sets[f].iter().enumerate() {
        let col = if i == 0 { light } else { c };
        ring(img, 16, 16, r, r, with_alpha(col, alphas[f][i]), false);
    }
    if f < 3 {
        disc(img, 16, 16, 1, lighten(c, 0.8));
    }
}

// ----- entry point ---------------------------------------------------------

fn effect(t: Type, frame: usize) -> RgbaImage {
    let mut img = RgbaImage::new(SIZE as u32, SIZE as u32);
    let c = type_color(t);
    match t {
        Type::Feral => feral(&mut img, c, frame),
        Type::Ember => ember(&mut img, c, frame),
        Type::Tide => tide(&mut img, c, frame),
        Type::Bloom => bloom(&mut img, c, frame),
        Type::Volt => volt(&mut img, c, frame),
        Type::Gale => gale(&mut img, c, frame),
        Type::Stone => stone(&mut img, c, frame),
        Type::Frost => frost(&mut img, c, frame),
        Type::Venom => venom(&mut img, c, frame),
        Type::Phantom => phantom(&mut img, c, frame),
        Type::Alloy => alloy(&mut img, c, frame),
        Type::Resonant => resonant(&mut img, c, frame),
    }
    img
}

/// Renders all 12 type strips (48 PNGs) into `out`, returning the count
/// of files written.
pub fn render_effects(out: &Path) -> Result<usize> {
    std::fs::create_dir_all(out).with_context(|| format!("creating {}", out.display()))?;
    let mut count = 0;
    for t in Type::ALL {
        let name = format!("{t:?}").to_lowercase();
        for frame in 0..FRAMES {
            effect(t, frame)
                .save(out.join(format!("{name}.{frame}.png")))
                .with_context(|| format!("writing fx {name}.{frame}"))?;
            count += 1;
        }
    }
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_are_deterministic() {
        for t in Type::ALL {
            for f in 0..FRAMES {
                assert_eq!(
                    effect(t, f).into_raw(),
                    effect(t, f).into_raw(),
                    "{t:?} frame {f} drifts between renders"
                );
            }
        }
    }

    #[test]
    fn every_frame_draws_something() {
        for t in Type::ALL {
            for f in 0..FRAMES {
                let img = effect(t, f);
                assert!(
                    img.pixels().any(|p| p[3] > 0),
                    "{t:?} frame {f} is empty"
                );
            }
        }
    }
}
