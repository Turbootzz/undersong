//! `tools sprites` — the P10 art engine (doc 05 v2). Everything is
//! deterministic pixel art: tiles, characters, and creature bodies,
//! seeded like the cries so evolution lines stay visibly related.
//! Output lands in `assets/sprites/`; a PNG at the same relative path
//! under `assets/custom/` always wins at load time.

use std::path::Path;

use anyhow::{Context, Result};
use image::{Rgba, RgbaImage, imageops};
use undersong_core::rng::BattleRng;
use undersong_core::types::Type;

pub const TILE: u32 = 32;

// ----- the doc-05 ink-and-parchment ramp --------------------------------

const INK: Rgba<u8> = Rgba([26, 24, 34, 255]);
const PARCHMENT: Rgba<u8> = Rgba([232, 220, 195, 255]);
const GILT: Rgba<u8> = Rgba([198, 161, 71, 255]);

fn hex(rgb: u32) -> Rgba<u8> {
    Rgba([
        u8::try_from((rgb >> 16) & 0xff).expect("byte"),
        u8::try_from((rgb >> 8) & 0xff).expect("byte"),
        u8::try_from(rgb & 0xff).expect("byte"),
        255,
    ])
}

fn shade(c: Rgba<u8>, f: f32) -> Rgba<u8> {
    let s = |v: u8| -> u8 {
        let scaled = f32::from(v) * f;
        if scaled >= 255.0 { 255 } else { scaled as u8 }
    };
    Rgba([s(c[0]), s(c[1]), s(c[2]), c[3]])
}

fn type_color(t: Type) -> Rgba<u8> {
    match t {
        Type::Feral => hex(0x9a8a6f),
        Type::Ember => hex(0xc4593a),
        Type::Tide => hex(0x4a7a9c),
        Type::Bloom => hex(0x6a9a4e),
        Type::Gale => hex(0x8fb3c7),
        Type::Stone => hex(0x8a7f6d),
        Type::Frost => hex(0xa9c7d4),
        Type::Volt => hex(0xd4b13e),
        Type::Venom => hex(0x7d5a8e),
        Type::Phantom => hex(0x5d5470),
        Type::Alloy => hex(0x9aa0a8),
        Type::Resonant => hex(0xb08ec4),
    }
}

// ----- tiles -------------------------------------------------------------

/// A 32×32 textured tile. `kind` matches the map ground/decor ids the
/// renderer translates.
fn tile(kind: &str, rng: &mut BattleRng) -> RgbaImage {
    let mut img = RgbaImage::new(TILE, TILE);
    let mut fill = |base: Rgba<u8>, speck: Rgba<u8>, density: u32| {
        for y in 0..TILE {
            for x in 0..TILE {
                img.put_pixel(x, y, base);
            }
        }
        for _ in 0..density {
            let x = rng.below(TILE);
            let y = rng.below(TILE);
            img.put_pixel(x, y, speck);
        }
    };
    match kind {
        "grass" => {
            let base = hex(0x6a9a4e);
            fill(base, shade(base, 0.85), 90);
            // blade tufts
            for _ in 0..10 {
                let x = 2 + rng.below(TILE - 4);
                let y = 2 + rng.below(TILE - 4);
                let dark = shade(base, 0.7);
                img.put_pixel(x, y, dark);
                img.put_pixel(x, y + 1, dark);
                img.put_pixel(x + 1, y + 1, shade(base, 1.15));
            }
        }
        "patch" => {
            let base = shade(hex(0x6a9a4e), 0.8);
            fill(base, shade(base, 0.65), 110);
            for _ in 0..14 {
                let x = 1 + rng.below(TILE - 3);
                let y = 1 + rng.below(TILE - 4);
                let dark = shade(base, 0.55);
                img.put_pixel(x, y + 2, dark);
                img.put_pixel(x + 1, y, dark);
                img.put_pixel(x + 1, y + 1, dark);
            }
        }
        "path" => {
            let base = hex(0xcfc4ab);
            fill(base, shade(base, 0.9), 70);
            for _ in 0..6 {
                let x = rng.below(TILE - 3);
                let y = rng.below(TILE - 2);
                let pebble = shade(base, 0.78);
                img.put_pixel(x + 1, y, pebble);
                img.put_pixel(x, y + 1, pebble);
                img.put_pixel(x + 1, y + 1, shade(base, 1.08));
            }
        }
        "water" | "deep" => {
            let base = if kind == "deep" {
                shade(hex(0x4a7a9c), 0.75)
            } else {
                hex(0x4a7a9c)
            };
            fill(base, shade(base, 1.1), 40);
            // wave strokes
            for row in 0..4u32 {
                let y = 4 + row * 8 + rng.below(3);
                let x0 = rng.below(TILE / 2);
                let light = shade(base, 1.25);
                for dx in 0..(6 + rng.below(6)) {
                    let x = (x0 + dx) % TILE;
                    if y < TILE {
                        img.put_pixel(x, y, light);
                    }
                }
            }
        }
        "floor" => {
            let base = PARCHMENT;
            fill(base, shade(base, 0.94), 50);
            // plank seams
            for y in (0..TILE).step_by(8) {
                for x in 0..TILE {
                    img.put_pixel(x, y, shade(base, 0.85));
                }
            }
        }
        "wall" => {
            let base = shade(PARCHMENT, 0.92);
            fill(base, shade(base, 0.96), 30);
            // masonry courses with offset joints
            for (row, y) in (0..TILE).step_by(8).enumerate() {
                for x in 0..TILE {
                    img.put_pixel(x, y, shade(base, 0.75));
                }
                let offset = if row % 2 == 0 { 0 } else { 8 };
                for x in ((offset + 4)..TILE).step_by(16) {
                    for dy in 1..8 {
                        if y + dy < TILE {
                            img.put_pixel(x, y + dy, shade(base, 0.8));
                        }
                    }
                }
            }
            // gilt eave along the top
            for x in 0..TILE {
                img.put_pixel(x, TILE - 1, shade(GILT, 0.9));
            }
        }
        "bush" => {
            // transparent backdrop, leafy ball
            let base = shade(hex(0x6a9a4e), 0.7);
            let c = (TILE / 2) as i32;
            for y in 0..TILE {
                for x in 0..TILE {
                    let dx = x as i32 - c;
                    let dy = y as i32 - c + 2;
                    if dx * dx + dy * dy < 165 {
                        let f = if (x * 7 + y * 13 + rng.below(3)).is_multiple_of(9) {
                            1.2
                        } else if (x + y).is_multiple_of(5) {
                            0.85
                        } else {
                            1.0
                        };
                        img.put_pixel(x, y, shade(base, f));
                    }
                }
            }
        }
        "sign" => {
            let post = shade(hex(0x9a8a6f), 0.8);
            for y in 14..30u32 {
                img.put_pixel(15, y, post);
                img.put_pixel(16, y, post);
            }
            for y in 6..16u32 {
                for x in 5..27u32 {
                    img.put_pixel(x, y, hex(0x9a8a6f));
                }
            }
            for x in 5..27u32 {
                img.put_pixel(x, 6, shade(hex(0x9a8a6f), 0.7));
                img.put_pixel(x, 15, shade(hex(0x9a8a6f), 0.7));
            }
            for x in (8..24u32).step_by(5) {
                img.put_pixel(x, 10, INK);
                img.put_pixel(x + 1, 10, INK);
            }
        }
        "canopy" => {
            let base = shade(hex(0x6a9a4e), 0.55);
            let c = (TILE / 2) as i32;
            for y in 0..TILE {
                for x in 0..TILE {
                    let dx = x as i32 - c;
                    let dy = y as i32 - c;
                    if dx * dx + dy * dy < 240 {
                        let f = if (x * 5 + y * 11).is_multiple_of(7) { 1.25 } else { 1.0 };
                        let mut p = shade(base, f);
                        p[3] = 235;
                        img.put_pixel(x, y, p);
                    }
                }
            }
        }
        _ => fill(hex(0x444444), hex(0x555555), 20),
    }
    img
}

// ----- characters --------------------------------------------------------

struct Outfit {
    skin: Rgba<u8>,
    hair: Rgba<u8>,
    coat: Rgba<u8>,
    trim: Rgba<u8>,
    hat: bool,
}

fn outfit(key: &str) -> Outfit {
    let skin = hex(0xe8c39e);
    match key {
        "player" => Outfit {
            skin,
            hair: hex(0x5a4632),
            coat: GILT,
            trim: INK,
            hat: true,
        },
        "npc.trainer" => Outfit {
            skin,
            hair: hex(0x3a3a3a),
            coat: hex(0xc4593a),
            trim: INK,
            hat: true,
        },
        "npc.dario" | "npc.mirelle" => Outfit {
            skin,
            hair: hex(0xd9d9d9),
            coat: hex(0x5d5470),
            trim: GILT,
            hat: false,
        },
        "npc.reed" => Outfit {
            skin,
            hair: hex(0x8a5a2a),
            coat: hex(0x4a7a9c),
            trim: PARCHMENT,
            hat: false,
        },
        "npc.villager2" => Outfit {
            skin,
            hair: hex(0x2e2620),
            coat: hex(0x6a9a4e),
            trim: PARCHMENT,
            hat: false,
        },
        "npc.greeter" => Outfit {
            skin,
            hair: hex(0x6e3b22),
            coat: hex(0xb08ec4),
            trim: INK,
            hat: false,
        },
        _ => Outfit {
            // villager and friends
            skin,
            hair: hex(0x4a3a28),
            coat: hex(0x9a8a6f),
            trim: PARCHMENT,
            hat: false,
        },
    }
}

/// One 32×32 character frame. `dir`: 0 down, 1 up, 2 left, 3 right.
/// `step`: walk frame (legs alternate).
fn character(o: &Outfit, dir: u8, step: bool) -> RgbaImage {
    let mut img = RgbaImage::new(TILE, TILE);
    let mut px = |x: i32, y: i32, c: Rgba<u8>| {
        if (0..TILE as i32).contains(&x) && (0..TILE as i32).contains(&y) {
            img.put_pixel(x as u32, y as u32, c);
        }
    };
    let cx = 16i32;
    // legs (alternate on step)
    let (l_off, r_off) = if step { (1, -1) } else { (-1, 1) };
    for (lx, off) in [(cx - 5, l_off), (cx + 2, r_off)] {
        for y in 22..28i32 {
            for x in lx..lx + 4 {
                px(x, y + off, INK);
            }
        }
    }
    // coat / body
    for y in 12..23i32 {
        let w = 7 + i32::from(y > 14);
        for x in (cx - w)..(cx + w) {
            px(x, y, o.coat);
        }
    }
    // trim sash
    for x in (cx - 7)..(cx + 7) {
        px(x, 20, o.trim);
    }
    // arms
    for y in 13..20i32 {
        px(cx - 9, y, o.coat);
        px(cx - 8, y, shade(o.coat, 0.8));
        px(cx + 7, y, shade(o.coat, 0.8));
        px(cx + 8, y, o.coat);
    }
    // head
    for y in 3..13i32 {
        let w = match y {
            3 | 12 => 4,
            4 | 11 => 5,
            _ => 6,
        };
        for x in (cx - w)..(cx + w) {
            px(x, y, o.skin);
        }
    }
    // hair / hat
    if o.hat {
        for y in 1..5i32 {
            let w = if y == 1 { 5 } else { 7 };
            for x in (cx - w)..(cx + w) {
                px(x, y, o.trim);
            }
        }
        for x in (cx - 8)..(cx + 8) {
            px(x, 5, o.trim);
        }
    } else {
        for y in 2..6i32 {
            let w = if y == 2 { 4 } else { 6 };
            for x in (cx - w)..(cx + w) {
                px(x, y, o.hair);
            }
        }
    }
    // face by direction
    match dir {
        0 => {
            // down: two eyes
            px(cx - 3, 8, INK);
            px(cx - 2, 8, INK);
            px(cx + 1, 8, INK);
            px(cx + 2, 8, INK);
        }
        1 => {
            // up: hair back, no face
            for y in 6..12i32 {
                let w = 5;
                for x in (cx - w)..(cx + w) {
                    px(x, y, o.hair);
                }
            }
        }
        2 => {
            // left: one eye, west side
            px(cx - 4, 8, INK);
            px(cx - 3, 8, INK);
        }
        _ => {
            px(cx + 2, 8, INK);
            px(cx + 3, 8, INK);
        }
    }
    // ink outline pass: any opaque pixel bordering transparency darkens
    outline(&mut img);
    img
}

/// Darkens the silhouette boundary — the doc-05 ink line.
fn outline(img: &mut RgbaImage) {
    let w = img.width() as i32;
    let h = img.height() as i32;
    let mut edges = Vec::new();
    for y in 0..h {
        for x in 0..w {
            if img.get_pixel(x as u32, y as u32)[3] == 0 {
                continue;
            }
            let boundary = [(1i32, 0i32), (-1, 0), (0, 1), (0, -1)]
                .iter()
                .any(|(dx, dy)| {
                    let (nx, ny) = (x + dx, y + dy);
                    nx < 0
                        || ny < 0
                        || nx >= w
                        || ny >= h
                        || img.get_pixel(nx as u32, ny as u32)[3] == 0
                });
            if boundary {
                edges.push((x as u32, y as u32));
            }
        }
    }
    for (x, y) in edges {
        img.put_pixel(x, y, INK);
    }
}

// ----- creatures ----------------------------------------------------------

#[derive(Clone, Copy, PartialEq)]
enum Plan {
    Quadruped,
    Bird,
    Serpent,
    Moth,
    Fish,
    Blob,
    Golem,
    Wisp,
}

fn body_plan(primary: Type, tags: &[String]) -> Plan {
    if tags.iter().any(|t| t == "performer.sky") {
        return Plan::Bird;
    }
    if tags.iter().any(|t| t == "performer.ferry") {
        return Plan::Fish;
    }
    match primary {
        Type::Feral | Type::Ember | Type::Frost => Plan::Quadruped,
        Type::Gale => Plan::Bird,
        Type::Venom => Plan::Serpent,
        Type::Tide => Plan::Fish,
        Type::Bloom => Plan::Blob,
        Type::Stone | Type::Alloy => Plan::Golem,
        Type::Phantom => Plan::Wisp,
        Type::Volt | Type::Resonant => Plan::Moth,
    }
}

/// 96×96 battle-front creature. Symmetric body, plan-driven anatomy,
/// seeded proportions and markings.
#[expect(clippy::too_many_lines, reason = "one body grammar, eight plans")]
fn creature_front(seed: u64, primary: Type, secondary: Option<Type>, tags: &[String]) -> RgbaImage {
    const S: u32 = 96;
    let mut rng = BattleRng::from_seed(seed ^ 0x0059_217e);
    let mut img = RgbaImage::new(S, S);
    let plan = body_plan(primary, tags);
    let base = type_color(primary);
    let accent = secondary.map(type_color).unwrap_or(shade(base, 1.25));
    let belly = shade(PARCHMENT, 0.97);

    let cx = 48i32;
    // seeded proportions
    let body_w = 16 + rng.below(8) as i32; // half-width
    let body_h = 14 + rng.below(8) as i32;
    let body_cy = 56i32 - i32::from(plan == Plan::Bird) * 4;
    let head_r = 9 + rng.below(5) as i32;
    let head_cy = body_cy - body_h - head_r + 6;

    let px = |img: &mut RgbaImage, x: i32, y: i32, c: Rgba<u8>| {
        if (0..S as i32).contains(&x) && (0..S as i32).contains(&y) {
            img.put_pixel(x as u32, y as u32, c);
        }
    };
    let blot = |img: &mut RgbaImage, cx: i32, cy: i32, rx: i32, ry: i32, c: Rgba<u8>| {
        for y in (cy - ry)..=(cy + ry) {
            for x in (cx - rx)..=(cx + rx) {
                let dx = f64::from(x - cx) / f64::from(rx.max(1));
                let dy = f64::from(y - cy) / f64::from(ry.max(1));
                if dx * dx + dy * dy <= 1.0 {
                    px(img, x, y, c);
                }
            }
        }
    };

    match plan {
        Plan::Quadruped => {
            // legs first (behind body)
            for off in [-body_w + 4, body_w - 8] {
                for leg in 0..2i32 {
                    let lx = cx + off + leg * 4;
                    for y in body_cy + body_h - 6..body_cy + body_h + 10 {
                        px(&mut img, lx, y, shade(base, 0.8));
                        px(&mut img, lx + 1, y, shade(base, 0.8));
                        px(&mut img, lx + 2, y, shade(base, 0.7));
                    }
                }
            }
            blot(&mut img, cx, body_cy, body_w, body_h, base);
            blot(&mut img, cx, body_cy + 3, body_w - 6, body_h - 6, belly);
            // ears
            blot(
                &mut img,
                cx - head_r + 2,
                head_cy - head_r + 2,
                3,
                5,
                accent,
            );
            blot(
                &mut img,
                cx + head_r - 2,
                head_cy - head_r + 2,
                3,
                5,
                accent,
            );
            blot(&mut img, cx, head_cy, head_r, head_r, base);
            // tail
            let t = 1 + rng.below(3) as i32;
            for i in 0..10i32 {
                blot(&mut img, cx + body_w + i, body_cy - i * t / 2, 3, 3, accent);
            }
        }
        Plan::Bird => {
            // wings out
            for side in [-1i32, 1] {
                for i in 0..body_w + 6 {
                    let wx = cx + side * (body_w / 2 + i);
                    let wy = body_cy - 6 - i / 2;
                    blot(&mut img, wx, wy, 3, 5 - i / 8, accent);
                }
            }
            blot(&mut img, cx, body_cy, body_w - 4, body_h + 2, base);
            blot(&mut img, cx, body_cy + 4, body_w - 9, body_h - 5, belly);
            blot(&mut img, cx, head_cy + 4, head_r - 1, head_r - 1, base);
            // beak
            for i in 0..5i32 {
                px(&mut img, cx - 2 + i, head_cy + 6 + i / 2, GILT);
                px(&mut img, cx - 1 + i, head_cy + 7 + i / 2, GILT);
            }
            // feet
            for off in [-4i32, 4] {
                for y in body_cy + body_h..body_cy + body_h + 8 {
                    px(&mut img, cx + off, y, GILT);
                }
            }
        }
        Plan::Serpent => {
            // coiled S of blobs
            let coils = 4 + rng.below(2) as i32;
            for i in 0..coils * 8 {
                let t = f64::from(i) / 8.0;
                let sx = cx + ((t * 2.2).sin() * f64::from(body_w)) as i32;
                let sy = body_cy + body_h - i * 2;
                blot(
                    &mut img,
                    sx,
                    sy,
                    7,
                    6,
                    if i % 6 < 3 { base } else { shade(base, 0.85) },
                );
            }
            blot(&mut img, cx, head_cy + 8, head_r, head_r - 2, base);
            blot(&mut img, cx, head_cy + 12, head_r - 4, 3, accent);
        }
        Plan::Moth => {
            // two wing pairs
            for side in [-1i32, 1] {
                blot(
                    &mut img,
                    cx + side * (body_w + 4),
                    body_cy - 10,
                    body_w - 2,
                    body_h,
                    accent,
                );
                blot(
                    &mut img,
                    cx + side * (body_w + 1),
                    body_cy + 6,
                    body_w - 7,
                    body_h - 5,
                    shade(accent, 0.8),
                );
                // wing eye-spot
                blot(
                    &mut img,
                    cx + side * (body_w + 4),
                    body_cy - 10,
                    4,
                    4,
                    shade(base, 0.7),
                );
            }
            blot(&mut img, cx, body_cy, 7, body_h + 4, base);
            blot(&mut img, cx, head_cy + 8, head_r - 2, head_r - 2, base);
            // antennae
            for side in [-1i32, 1] {
                for i in 0..8i32 {
                    px(&mut img, cx + side * (2 + i / 2), head_cy + 2 - i, INK);
                }
            }
        }
        Plan::Fish => {
            blot(&mut img, cx, body_cy - 8, body_w + 4, body_h, base);
            blot(&mut img, cx, body_cy - 4, body_w - 3, body_h - 6, belly);
            // tail fin
            for i in 0..10i32 {
                blot(
                    &mut img,
                    cx - body_w - 4 - i / 2,
                    body_cy - 8 - i + 5,
                    2,
                    4,
                    accent,
                );
                blot(
                    &mut img,
                    cx - body_w - 4 - i / 2,
                    body_cy - 8 + i - 5,
                    2,
                    4,
                    accent,
                );
            }
            // dorsal
            for i in 0..body_w {
                px(
                    &mut img,
                    cx - body_w / 2 + i,
                    body_cy - 8 - body_h - i % 4,
                    accent,
                );
            }
            // no separate head: big eye on the body
        }
        Plan::Blob => {
            blot(&mut img, cx, body_cy - 4, body_w + 2, body_h + 6, base);
            // sprout
            for i in 0..7i32 {
                px(
                    &mut img,
                    cx,
                    body_cy - body_h - 10 - i,
                    shade(hex(0x6a9a4e), 0.8),
                );
            }
            blot(&mut img, cx - 4, body_cy - body_h - 16, 5, 3, accent);
            blot(&mut img, cx + 4, body_cy - body_h - 16, 5, 3, accent);
            // freckles
            for _ in 0..6 {
                let fx = cx - body_w + 4 + rng.below((body_w as u32) * 2 - 8) as i32;
                let fy = body_cy - 4 + rng.below(body_h as u32) as i32 - body_h / 2;
                blot(&mut img, fx, fy, 1, 1, accent);
            }
        }
        Plan::Golem => {
            // stacked slabs, head clear of the shoulders
            blot(
                &mut img,
                cx,
                body_cy + 6,
                body_w + 4,
                body_h - 2,
                shade(base, 0.85),
            );
            blot(&mut img, cx, body_cy - 8, body_w, body_h - 4, base);
            blot(
                &mut img,
                cx,
                body_cy - body_h - 10,
                head_r,
                head_r - 1,
                shade(base, 1.1),
            );
            // arms: chunky side slabs
            for side in [-1i32, 1] {
                blot(
                    &mut img,
                    cx + side * (body_w + 7),
                    body_cy - 4,
                    5,
                    10,
                    shade(base, 0.75),
                );
            }
            // cracks
            for _ in 0..5 {
                let fx = cx - body_w + rng.below((body_w as u32) * 2) as i32;
                let fy = body_cy - 8 + rng.below(body_h as u32) as i32;
                px(&mut img, fx, fy, INK);
                px(&mut img, fx + 1, fy + 1, INK);
            }
            // gilt seam
            blot(&mut img, cx, body_cy - 8, body_w - 6, 1, GILT);
        }
        Plan::Wisp => {
            // tapering ghost-body with ragged hem
            for i in 0..body_h * 2 {
                let w = body_w - i / 3;
                if w <= 2 {
                    break;
                }
                let mut c = base;
                c[3] = 235;
                blot(&mut img, cx, head_cy + 8 + i, w, 2, c);
            }
            blot(&mut img, cx, head_cy + 4, head_r + 2, head_r, base);
            // trailing wisps
            for side in [-1i32, 1] {
                for i in 0..8i32 {
                    px(
                        &mut img,
                        cx + side * (body_w - 2) + side * i / 2,
                        head_cy + 20 + i * 3,
                        accent,
                    );
                }
            }
        }
    }

    // eyes (every plan): parchment glint over ink
    let eye_y = match plan {
        Plan::Fish => body_cy - 10,
        Plan::Golem => body_cy - body_h - 11,
        _ => head_cy + 6,
    };
    let eye_dx = if plan == Plan::Fish {
        10
    } else {
        head_r / 2 + 1
    };
    for side in [-1i32, 1] {
        let ex = cx + side * eye_dx;
        for dy in 0..3i32 {
            for dx in 0..2i32 {
                px(&mut img, ex + dx, eye_y + dy, INK);
            }
        }
        px(&mut img, ex, eye_y, PARCHMENT);
    }

    outline(&mut img);
    img
}

// ----- entry points -------------------------------------------------------

pub fn render_tiles(out: &Path) -> Result<usize> {
    std::fs::create_dir_all(out).with_context(|| format!("creating {}", out.display()))?;
    let kinds = [
        "grass", "patch", "path", "water", "deep", "floor", "wall", "bush", "sign", "canopy",
    ];
    for kind in kinds {
        let mut rng = BattleRng::from_seed(0x711e ^ (kind.len() as u64 * 7919));
        tile(kind, &mut rng)
            .save(out.join(format!("{kind}.png")))
            .with_context(|| format!("writing {kind}"))?;
    }
    Ok(kinds.len())
}

pub fn render_characters(out: &Path) -> Result<usize> {
    std::fs::create_dir_all(out).with_context(|| format!("creating {}", out.display()))?;
    let keys = [
        "player",
        "npc.villager",
        "npc.villager2",
        "npc.trainer",
        "npc.dario",
        "npc.mirelle",
        "npc.reed",
        "npc.greeter",
    ];
    let mut count = 0;
    for key in keys {
        let o = outfit(key);
        for (dir, name) in [(0u8, "down"), (1, "up"), (2, "left"), (3, "right")] {
            for (step, frame) in [(false, 0u8), (true, 1)] {
                let img = character(&o, dir, step);
                img.save(out.join(format!("{key}.{name}.{frame}.png")))
                    .with_context(|| format!("writing {key}"))?;
                count += 1;
            }
        }
    }
    Ok(count)
}

pub fn render_creatures(content_root: &Path, region: &str, out_root: &Path) -> Result<usize> {
    let pack = data::load_region(content_root, region)
        .with_context(|| format!("loading region `{region}`"))?;
    let out = out_root.join(region);
    std::fs::create_dir_all(&out).with_context(|| format!("creating {}", out.display()))?;
    let mut count = 0;
    for motif in pack.motifs.values() {
        let front = creature_front(
            motif.sigil_seed,
            motif.types[0],
            motif.types.get(1).copied(),
            &motif.tags,
        );
        front
            .save(out.join(format!("{}.front.png", motif.id)))
            .with_context(|| format!("writing {}", motif.id))?;
        // Back view: the lower 2/3, zoomed (classic over-shoulder crop).
        let back = imageops::resize(
            &imageops::crop_imm(&front, 12, 32, 72, 56).to_image(),
            96,
            72,
            imageops::FilterType::Nearest,
        );
        back.save(out.join(format!("{}.back.png", motif.id)))
            .with_context(|| format!("writing {} back", motif.id))?;
        let icon = imageops::resize(&front, 16, 16, imageops::FilterType::Nearest);
        icon.save(out.join(format!("{}.icon.png", motif.id)))
            .with_context(|| format!("writing {} icon", motif.id))?;
        count += 1;
    }
    Ok(count)
}
