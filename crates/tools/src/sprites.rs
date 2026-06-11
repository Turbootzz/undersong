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
            let base = hex(0x5fa653);
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
                shade(hex(0x3f7fb5), 0.7)
            } else {
                hex(0x3f7fb5)
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
                        let f = if (x * 5 + y * 11).is_multiple_of(7) {
                            1.25
                        } else {
                            1.0
                        };
                        let mut p = shade(base, f);
                        p[3] = 235;
                        img.put_pixel(x, y, p);
                    }
                }
            }
        }
        "wall_indoor" => {
            // dark wainscot paneling — interiors need readable walls
            let base = hex(0x4a4256);
            fill(base, shade(base, 0.92), 24);
            for x in (0..TILE).step_by(8) {
                for y in 0..TILE {
                    img.put_pixel(x, y, shade(base, 0.78));
                }
            }
            for x in 0..TILE {
                img.put_pixel(x, TILE - 2, shade(GILT, 0.85));
                img.put_pixel(x, 0, shade(base, 0.7));
            }
        }
        "roof_red" | "roof_blue" | "roof_green" => {
            let base = match kind {
                "roof_red" => hex(0xb5533c),
                "roof_blue" => hex(0x4a6f9c),
                _ => hex(0x5f8a4e),
            };
            fill(base, shade(base, 0.92), 30);
            // shingle courses
            for (row, y) in (0..TILE).step_by(6).enumerate() {
                for x in 0..TILE {
                    img.put_pixel(x, y, shade(base, 0.7));
                }
                let offset = if row % 2 == 0 { 3 } else { 9 };
                for x in (offset..TILE).step_by(12) {
                    for dy in 1..6u32 {
                        if y + dy < TILE {
                            img.put_pixel(x, y + dy, shade(base, 0.82));
                        }
                    }
                }
            }
        }
        "door" => {
            // frame on parchment wall, warm wooden door, gilt knob
            let wall = shade(PARCHMENT, 0.92);
            fill(wall, shade(wall, 0.96), 16);
            let wood = hex(0x7a4a2a);
            for y in 6..TILE {
                for x in 8..24u32 {
                    img.put_pixel(x, y, if x % 5 == 0 { shade(wood, 0.85) } else { wood });
                }
            }
            for y in 5..TILE {
                img.put_pixel(7, y, INK);
                img.put_pixel(24, y, INK);
            }
            for x in 7..25u32 {
                img.put_pixel(x, 5, INK);
            }
            img.put_pixel(21, 18, GILT);
            img.put_pixel(21, 19, GILT);
            img.put_pixel(20, 18, shade(GILT, 0.8));
        }
        "window" => {
            let wall = shade(PARCHMENT, 0.92);
            fill(wall, shade(wall, 0.96), 16);
            let glow = hex(0xf2d06b);
            for y in 9..21u32 {
                for x in 9..23u32 {
                    img.put_pixel(
                        x,
                        y,
                        if (x + y) % 7 == 0 {
                            shade(glow, 1.1)
                        } else {
                            glow
                        },
                    );
                }
            }
            for y in 8..22u32 {
                img.put_pixel(8, y, INK);
                img.put_pixel(23, y, INK);
            }
            for x in 8..24u32 {
                img.put_pixel(x, 8, INK);
                img.put_pixel(x, 21, INK);
            }
            for y in 9..21u32 {
                img.put_pixel(15, y, shade(INK, 1.4));
                img.put_pixel(16, y, shade(INK, 1.4));
            }
        }
        "flowers" => {
            // transparent over grass: scattered blooms
            let blooms = [hex(0xd45a6e), hex(0xf2d06b), hex(0xe8e8f0), hex(0xb08ec4)];
            for (i, &bloom) in blooms.iter().enumerate() {
                for _ in 0..3 {
                    let x = 2 + rng.below(TILE - 5);
                    let y = 2 + rng.below(TILE - 5);
                    img.put_pixel(x + 1, y, bloom);
                    img.put_pixel(x, y + 1, bloom);
                    img.put_pixel(x + 2, y + 1, bloom);
                    img.put_pixel(x + 1, y + 2, bloom);
                    img.put_pixel(x + 1, y + 1, shade(blooms[(i + 1) % 4], 1.1));
                    img.put_pixel(x + 1, y + 3, hex(0x3f6e35));
                }
            }
        }
        "fence" => {
            let wood = hex(0x8a6a42);
            for y in 12..26u32 {
                for x in 0..TILE {
                    if x % 8 < 3 {
                        img.put_pixel(x, y, if x % 8 == 1 { wood } else { shade(wood, 0.8) });
                    }
                }
            }
            for x in 0..TILE {
                img.put_pixel(x, 15, shade(wood, 0.9));
                img.put_pixel(x, 16, wood);
                img.put_pixel(x, 21, shade(wood, 0.9));
                img.put_pixel(x, 22, wood);
            }
        }
        "lamp" => {
            let post = hex(0x3a3a44);
            for y in 10..30u32 {
                img.put_pixel(15, y, post);
                img.put_pixel(16, y, shade(post, 1.3));
            }
            for y in 4..10u32 {
                for x in 12..20u32 {
                    img.put_pixel(x, y, hex(0xf2d06b));
                }
            }
            for x in 11..21u32 {
                img.put_pixel(x, 3, post);
                img.put_pixel(x, 10, post);
            }
            for y in 4..10u32 {
                img.put_pixel(11, y, post);
                img.put_pixel(20, y, post);
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

/// One 32×44 character frame (anime proportions, bottom-anchored).
/// `dir`: 0 down, 1 up, 2 left, 3 right. `step`: walk frame.
fn character(o: &Outfit, dir: u8, step: bool) -> RgbaImage {
    let mut img = RgbaImage::new(32, 44);
    let mut px = |x: i32, y: i32, c: Rgba<u8>| {
        if (0..32).contains(&x) && (0..44).contains(&y) {
            img.put_pixel(x as u32, y as u32, c);
        }
    };
    let cx = 15i32;
    // legs (alternate on step)
    let (l_off, r_off) = if step { (1, -1) } else { (0, 0) };
    for (lx, off) in [(cx - 5, l_off), (cx + 2, r_off)] {
        for y in 31..40i32 {
            for x in lx..lx + 4 {
                px(x, y + off, INK);
            }
        }
    }
    // coat / body (shorter than the head — anime ratio)
    for y in 18..32i32 {
        let w = 7 + i32::from(y > 21);
        for x in (cx - w)..=(cx + w) {
            px(x, y, o.coat);
        }
    }
    for x in (cx - 7)..=(cx + 7) {
        px(x, 28, o.trim);
    }
    // arms
    for y in 19..27i32 {
        px(cx - 9, y, o.coat);
        px(cx - 10, y, shade(o.coat, 0.8));
        px(cx + 9, y, o.coat);
        px(cx + 10, y, shade(o.coat, 0.8));
    }
    // big head
    for y in 3..18i32 {
        let w = match y {
            3 | 17 => 5,
            4 | 16 => 7,
            _ => 8,
        };
        for x in (cx - w)..=(cx + w) {
            px(x, y, o.skin);
        }
    }
    // hair / hat crown
    if o.hat {
        for y in 1..6i32 {
            let w = if y == 1 { 6 } else { 8 };
            for x in (cx - w)..=(cx + w) {
                px(x, y, o.trim);
            }
        }
        for x in (cx - 9)..=(cx + 9) {
            px(x, 6, o.trim);
        }
    } else {
        for y in 2..8i32 {
            let w = if y == 2 { 5 } else { 8 };
            for x in (cx - w)..=(cx + w) {
                px(x, y, o.hair);
            }
        }
        // side locks
        for y in 8..13i32 {
            px(cx - 8, y, o.hair);
            px(cx + 8, y, o.hair);
        }
    }
    match dir {
        0 => {
            // down: big anime eyes (2×3 with glint)
            for (ex, _) in [(cx - 5, 0), (cx + 3, 0)] {
                for dy in 0..3i32 {
                    for dx in 0..3i32 {
                        px(ex + dx, 10 + dy, INK);
                    }
                }
                px(ex, 10, Rgba([242, 233, 216, 255]));
            }
            // mouth
            px(cx, 15, shade(o.skin, 0.7));
            px(cx + 1, 15, shade(o.skin, 0.7));
        }
        1 => {
            for y in 8..18i32 {
                let w = 7;
                for x in (cx - w)..=(cx + w) {
                    px(x, y, o.hair);
                }
            }
        }
        2 => {
            for dy in 0..3i32 {
                for dx in 0..3i32 {
                    px(cx - 6 + dx, 10 + dy, INK);
                }
            }
            px(cx - 6, 10, Rgba([242, 233, 216, 255]));
        }
        _ => {
            for dy in 0..3i32 {
                for dx in 0..3i32 {
                    px(cx + 4 + dx, 10 + dy, INK);
                }
            }
            px(cx + 6, 10, Rgba([242, 233, 216, 255]));
        }
    }
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

/// Three-tone ramp around a base color (dark/base/light + outline ink).
struct Ramp {
    dark: Rgba<u8>,
    base: Rgba<u8>,
    light: Rgba<u8>,
}

fn ramp(c: Rgba<u8>) -> Ramp {
    Ramp {
        dark: shade(c, 0.72),
        base: c,
        light: shade(c, 1.22),
    }
}

/// Body context shared by front and back renderers.
struct Body {
    plan: Plan,
    cx: i32,
    body_w: i32,
    body_h: i32,
    body_cy: i32,
    head_r: i32,
    head_cy: i32,
    ramp: Ramp,
    accent: Ramp,
    belly: Rgba<u8>,
    ear_kind: u32,
    tail_kind: u32,
    crest: bool,
}

fn body_from_seed(seed: u64, primary: Type, secondary: Option<Type>, tags: &[String]) -> Body {
    let mut rng = BattleRng::from_seed(seed ^ 0x0059_217e);
    let plan = body_plan(primary, tags);
    let base = type_color(primary);
    let accent = secondary.map(type_color).unwrap_or(shade(base, 1.3));
    let body_w = 15 + rng.below(9) as i32;
    let body_h = 13 + rng.below(9) as i32;
    let body_cy = 58i32 - i32::from(plan == Plan::Bird) * 4;
    let head_r = 10 + rng.below(5) as i32;
    Body {
        plan,
        cx: 48,
        body_w,
        body_h,
        body_cy,
        head_r,
        head_cy: body_cy - body_h - head_r + 7,
        ramp: ramp(base),
        accent: ramp(accent),
        belly: shade(PARCHMENT, 0.97),
        ear_kind: rng.below(3),
        tail_kind: rng.below(3),
        crest: rng.chance(2, 5),
    }
}

/// Shaded blot: light from the upper-left, dithered transition bands.
fn blot_shaded(img: &mut RgbaImage, cx: i32, cy: i32, rx: i32, ry: i32, r: &Ramp) {
    let s = 96i32;
    for y in (cy - ry)..=(cy + ry) {
        for x in (cx - rx)..=(cx + rx) {
            if !(0..s).contains(&x) || !(0..s).contains(&y) {
                continue;
            }
            let dx = f64::from(x - cx) / f64::from(rx.max(1));
            let dy = f64::from(y - cy) / f64::from(ry.max(1));
            let d = dx * dx + dy * dy;
            if d > 1.0 {
                continue;
            }
            // light vector: up-left
            let lit = dx * -0.5 + dy * -0.7;
            let dither = (x + y) % 2 == 0;
            let c = if d > 0.74 && lit < 0.1 {
                if dither && d < 0.86 { r.base } else { r.dark }
            } else if lit > 0.34 {
                if dither || lit > 0.52 {
                    r.light
                } else {
                    r.base
                }
            } else {
                r.base
            };
            img.put_pixel(x as u32, y as u32, c);
        }
    }
}

fn px96(img: &mut RgbaImage, x: i32, y: i32, c: Rgba<u8>) {
    if (0..96).contains(&x) && (0..96).contains(&y) {
        img.put_pixel(x as u32, y as u32, c);
    }
}

/// Ears/horns/crest kit, used by quadruped-ish heads.
fn head_features(img: &mut RgbaImage, b: &Body, facing_front: bool) {
    let (cx, head_cy, head_r) = (b.cx, b.head_cy, b.head_r);
    match b.ear_kind {
        0 => {
            // pointed ears
            for side in [-1i32, 1] {
                for i in 0..7i32 {
                    let w = (7 - i) / 2;
                    for dx in -w..=w {
                        px96(
                            img,
                            cx + side * (head_r - 3) + dx,
                            head_cy - head_r + 2 - i,
                            if i > 4 { b.accent.base } else { b.ramp.base },
                        );
                    }
                }
            }
        }
        1 => {
            // round ears
            for side in [-1i32, 1] {
                blot_shaded(
                    img,
                    cx + side * (head_r - 2),
                    head_cy - head_r + 1,
                    4,
                    4,
                    &b.accent,
                );
            }
        }
        _ => {
            // single horn
            for i in 0..9i32 {
                let w = (9 - i) / 3;
                for dx in -w..=w {
                    px96(img, cx + dx, head_cy - head_r - i + 2, b.accent.light);
                }
            }
        }
    }
    if b.crest && facing_front {
        for i in 0..5i32 {
            px96(img, cx - 2 + i, head_cy - head_r - 1, b.accent.dark);
        }
    }
}

fn tail(img: &mut RgbaImage, b: &Body, dir: i32) {
    match b.tail_kind {
        0 => {
            for i in 0..11i32 {
                blot_shaded(
                    img,
                    b.cx + dir * (b.body_w + i),
                    b.body_cy - i / 2,
                    3,
                    3,
                    &b.accent,
                );
            }
        }
        1 => {
            // tuft tail
            for i in 0..7i32 {
                blot_shaded(
                    img,
                    b.cx + dir * (b.body_w + i),
                    b.body_cy + 2,
                    2,
                    2,
                    &b.ramp,
                );
            }
            blot_shaded(
                img,
                b.cx + dir * (b.body_w + 8),
                b.body_cy + 1,
                4,
                4,
                &b.accent,
            );
        }
        _ => {
            // curl
            for i in 0..10i32 {
                let t = f64::from(i) * 0.6;
                blot_shaded(
                    img,
                    b.cx + dir * (b.body_w + 2 + (t.cos() * 5.0) as i32),
                    b.body_cy - 4 - (t.sin() * 6.0) as i32,
                    2,
                    2,
                    &b.accent,
                );
            }
        }
    }
}

/// 96×96 battle-front creature, grammar v2: shaded, feature kits.
#[expect(clippy::too_many_lines, reason = "one body grammar, eight plans")]
fn creature_front(seed: u64, primary: Type, secondary: Option<Type>, tags: &[String]) -> RgbaImage {
    let mut img = RgbaImage::new(96, 96);
    let b = body_from_seed(seed, primary, secondary, tags);
    let (cx, body_cy, body_w, body_h, head_r, head_cy) =
        (b.cx, b.body_cy, b.body_w, b.body_h, b.head_r, b.head_cy);

    match b.plan {
        Plan::Quadruped => {
            for off in [-body_w + 4, body_w - 8] {
                for leg in 0..2i32 {
                    let lx = cx + off + leg * 5;
                    for y in body_cy + body_h - 6..body_cy + body_h + 11 {
                        px96(&mut img, lx, y, b.ramp.dark);
                        px96(&mut img, lx + 1, y, b.ramp.base);
                        px96(&mut img, lx + 2, y, b.ramp.dark);
                    }
                    // paw
                    for dx in -1..3i32 {
                        px96(&mut img, lx + dx, body_cy + body_h + 11, b.ramp.dark);
                    }
                }
            }
            tail(&mut img, &b, 1);
            blot_shaded(&mut img, cx, body_cy, body_w, body_h, &b.ramp);
            let belly_ramp = ramp(b.belly);
            blot_shaded(
                &mut img,
                cx,
                body_cy + 3,
                body_w - 7,
                body_h - 6,
                &belly_ramp,
            );
            head_features(&mut img, &b, true);
            blot_shaded(&mut img, cx, head_cy, head_r, head_r, &b.ramp);
            // muzzle
            blot_shaded(
                &mut img,
                cx,
                head_cy + head_r / 2,
                head_r / 2,
                3,
                &ramp(b.belly),
            );
        }
        Plan::Bird => {
            for side in [-1i32, 1] {
                for i in 0..body_w + 7 {
                    let wx = cx + side * (body_w / 2 + i);
                    let wy = body_cy - 7 - i / 2;
                    blot_shaded(&mut img, wx, wy, 3, (6 - i / 7).max(2), &b.accent);
                }
            }
            blot_shaded(&mut img, cx, body_cy, body_w - 3, body_h + 3, &b.ramp);
            blot_shaded(
                &mut img,
                cx,
                body_cy + 4,
                body_w - 8,
                body_h - 4,
                &ramp(b.belly),
            );
            blot_shaded(&mut img, cx, head_cy + 4, head_r - 1, head_r - 1, &b.ramp);
            if b.crest {
                for i in 0..6i32 {
                    px96(
                        &mut img,
                        cx - 1 + i / 2,
                        head_cy - head_r + 2 - i,
                        b.accent.base,
                    );
                }
            }
            for i in 0..5i32 {
                px96(&mut img, cx - 2 + i, head_cy + 7 + i / 2, GILT);
                px96(&mut img, cx - 1 + i, head_cy + 8 + i / 2, shade(GILT, 0.8));
            }
            for off in [-5i32, 5] {
                for y in body_cy + body_h + 2..body_cy + body_h + 9 {
                    px96(&mut img, cx + off, y, GILT);
                }
                for dx in -2..3i32 {
                    px96(&mut img, cx + off + dx, body_cy + body_h + 9, GILT);
                }
            }
        }
        Plan::Serpent => {
            let coils = 4;
            for i in 0..coils * 9 {
                let t = f64::from(i) / 8.0;
                let sx = cx + ((t * 2.0).sin() * f64::from(body_w)) as i32;
                let sy = body_cy + body_h - i * 2;
                let r = if i % 6 < 3 { &b.ramp } else { &b.accent };
                blot_shaded(&mut img, sx, sy, 8, 6, r);
            }
            blot_shaded(&mut img, cx, head_cy + 8, head_r, head_r - 2, &b.ramp);
            head_features(&mut img, &b, true);
            // tongue
            px96(&mut img, cx, head_cy + 8 + head_r, hex(0xc4593a));
            px96(&mut img, cx, head_cy + 9 + head_r, hex(0xc4593a));
        }
        Plan::Moth => {
            for side in [-1i32, 1] {
                blot_shaded(
                    &mut img,
                    cx + side * (body_w + 4),
                    body_cy - 11,
                    body_w - 1,
                    body_h + 1,
                    &b.accent,
                );
                blot_shaded(
                    &mut img,
                    cx + side * (body_w + 1),
                    body_cy + 7,
                    body_w - 6,
                    body_h - 4,
                    &b.accent,
                );
                blot_shaded(
                    &mut img,
                    cx + side * (body_w + 4),
                    body_cy - 11,
                    5,
                    5,
                    &b.ramp,
                );
            }
            blot_shaded(&mut img, cx, body_cy, 7, body_h + 5, &b.ramp);
            blot_shaded(&mut img, cx, head_cy + 8, head_r - 2, head_r - 2, &b.ramp);
            for side in [-1i32, 1] {
                for i in 0..9i32 {
                    px96(&mut img, cx + side * (2 + i / 2), head_cy + 2 - i, INK);
                }
                px96(&mut img, cx + side * 6, head_cy - 7, b.accent.light);
            }
        }
        Plan::Fish => {
            for i in 0..11i32 {
                blot_shaded(
                    &mut img,
                    cx - body_w - 5 - i / 2,
                    body_cy - 8 - i + 5,
                    2,
                    4,
                    &b.accent,
                );
                blot_shaded(
                    &mut img,
                    cx - body_w - 5 - i / 2,
                    body_cy - 8 + i - 5,
                    2,
                    4,
                    &b.accent,
                );
            }
            blot_shaded(&mut img, cx, body_cy - 8, body_w + 5, body_h + 1, &b.ramp);
            blot_shaded(
                &mut img,
                cx + 2,
                body_cy - 4,
                body_w - 3,
                body_h - 6,
                &ramp(b.belly),
            );
            for i in 0..body_w {
                px96(
                    &mut img,
                    cx - body_w / 2 + i,
                    body_cy - 9 - body_h - i % 4,
                    b.accent.base,
                );
                px96(
                    &mut img,
                    cx - body_w / 2 + i,
                    body_cy - 8 - body_h - i % 4,
                    b.accent.dark,
                );
            }
            // side fin
            for i in 0..6i32 {
                px96(&mut img, cx + 4 + i, body_cy - 2 + i / 2, b.accent.base);
                px96(&mut img, cx + 5 + i, body_cy - 1 + i / 2, b.accent.dark);
            }
        }
        Plan::Blob => {
            blot_shaded(&mut img, cx, body_cy - 4, body_w + 3, body_h + 7, &b.ramp);
            for i in 0..8i32 {
                px96(
                    &mut img,
                    cx,
                    body_cy - body_h - 11 - i,
                    shade(hex(0x6a9a4e), 0.8),
                );
                px96(
                    &mut img,
                    cx + 1,
                    body_cy - body_h - 11 - i,
                    shade(hex(0x6a9a4e), 0.65),
                );
            }
            blot_shaded(&mut img, cx - 5, body_cy - body_h - 17, 5, 3, &b.accent);
            blot_shaded(&mut img, cx + 5, body_cy - body_h - 17, 5, 3, &b.accent);
            let mut rng = BattleRng::from_seed(seed ^ 0xb10b);
            for _ in 0..7 {
                let fx = cx - body_w + 4 + rng.below((b.body_w as u32) * 2 - 8) as i32;
                let fy = body_cy - 4 + rng.below(b.body_h as u32) as i32 - body_h / 2;
                px96(&mut img, fx, fy, b.accent.dark);
                px96(&mut img, fx + 1, fy, b.accent.base);
            }
        }
        Plan::Golem => {
            blot_shaded(
                &mut img,
                cx,
                body_cy + 6,
                body_w + 5,
                body_h - 2,
                &Ramp {
                    dark: shade(b.ramp.dark, 0.9),
                    base: b.ramp.dark,
                    light: b.ramp.base,
                },
            );
            blot_shaded(&mut img, cx, body_cy - 8, body_w, body_h - 3, &b.ramp);
            for side in [-1i32, 1] {
                blot_shaded(
                    &mut img,
                    cx + side * (body_w + 8),
                    body_cy - 4,
                    5,
                    11,
                    &b.ramp,
                );
                // knuckles
                blot_shaded(
                    &mut img,
                    cx + side * (body_w + 8),
                    body_cy + 8,
                    4,
                    3,
                    &b.accent,
                );
            }
            head_features(&mut img, &b, true);
            blot_shaded(
                &mut img,
                cx,
                body_cy - body_h - 10,
                head_r,
                head_r - 1,
                &b.ramp,
            );
            let mut rng = BattleRng::from_seed(seed ^ 0x901e);
            for _ in 0..6 {
                let fx = cx - body_w + rng.below((b.body_w as u32) * 2) as i32;
                let fy = body_cy - 8 + rng.below(b.body_h as u32) as i32;
                px96(&mut img, fx, fy, INK);
                px96(&mut img, fx + 1, fy + 1, b.ramp.dark);
            }
            for dx in -(body_w - 6)..(body_w - 6) {
                px96(&mut img, cx + dx, body_cy - 8, GILT);
            }
        }
        Plan::Wisp => {
            for i in 0..body_h * 2 {
                let w = body_w - i / 3;
                if w <= 2 {
                    break;
                }
                blot_shaded(&mut img, cx, head_cy + 8 + i, w, 2, &b.ramp);
            }
            // ragged hem
            for k in 0..4i32 {
                let hx = cx - body_w / 2 + k * (body_w / 2);
                blot_shaded(
                    &mut img,
                    hx,
                    head_cy + 8 + body_h * 2 - 2 + (k % 2) * 3,
                    2,
                    3,
                    &b.ramp,
                );
            }
            blot_shaded(&mut img, cx, head_cy + 4, head_r + 2, head_r, &b.ramp);
            head_features(&mut img, &b, true);
            for side in [-1i32, 1] {
                for i in 0..9i32 {
                    px96(
                        &mut img,
                        cx + side * (body_w - 2) + side * i / 2,
                        head_cy + 20 + i * 3,
                        b.accent.light,
                    );
                }
            }
        }
    }

    // eyes: ink with parchment glint, slightly larger (v2)
    let eye_y = match b.plan {
        Plan::Fish => body_cy - 10,
        Plan::Golem => body_cy - body_h - 11,
        _ => head_cy + 5,
    };
    let eye_dx = if b.plan == Plan::Fish {
        11
    } else {
        head_r / 2 + 1
    };
    for side in [-1i32, 1] {
        let ex = cx + side * eye_dx;
        for dy in 0..4i32 {
            for dx in 0..3i32 {
                px96(&mut img, ex + dx, eye_y + dy, INK);
            }
        }
        px96(&mut img, ex, eye_y, PARCHMENT);
        px96(&mut img, ex + 1, eye_y + 1, shade(PARCHMENT, 0.8));
    }

    outline(&mut img);
    img
}

/// 96×96 REAL back view (P13: the cropped-front fake was the "my
/// monster looks weird" bug): same body, drawn from behind — no face,
/// back markings, head from the rear.
fn creature_back(seed: u64, primary: Type, secondary: Option<Type>, tags: &[String]) -> RgbaImage {
    let mut img = RgbaImage::new(96, 96);
    let b = body_from_seed(seed, primary, secondary, tags);
    let (cx, body_cy, body_w, body_h, head_r, head_cy) =
        (b.cx, b.body_cy, b.body_w, b.body_h, b.head_r, b.head_cy);

    match b.plan {
        Plan::Quadruped => {
            // hind legs dominate from behind
            for off in [-body_w + 2, body_w - 7] {
                for y in body_cy + body_h - 8..body_cy + body_h + 12 {
                    for dx in 0..5i32 {
                        px96(
                            &mut img,
                            cx + off + dx,
                            y,
                            if dx == 2 { b.ramp.base } else { b.ramp.dark },
                        );
                    }
                }
            }
            tail(&mut img, &b, 1);
            blot_shaded(&mut img, cx, body_cy, body_w + 2, body_h + 2, &b.ramp);
            // spine stripe
            for y in (body_cy - body_h)..(body_cy + body_h - 4) {
                px96(&mut img, cx, y, b.accent.dark);
                px96(&mut img, cx + 1, y, b.accent.base);
            }
            head_features(&mut img, &b, false);
            blot_shaded(&mut img, cx, head_cy, head_r, head_r, &b.ramp);
        }
        Plan::Bird => {
            // folded wings from behind: two shaded panels
            blot_shaded(&mut img, cx, body_cy, body_w - 2, body_h + 4, &b.ramp);
            for side in [-1i32, 1] {
                blot_shaded(
                    &mut img,
                    cx + side * (body_w / 2 + 2),
                    body_cy - 2,
                    body_w / 2 + 2,
                    body_h,
                    &b.accent,
                );
            }
            blot_shaded(&mut img, cx, head_cy + 4, head_r - 1, head_r - 1, &b.ramp);
            if b.crest {
                for i in 0..6i32 {
                    px96(
                        &mut img,
                        cx - 1 + i / 2,
                        head_cy - head_r + 2 - i,
                        b.accent.base,
                    );
                }
            }
            // tail feathers
            for k in -1..=1i32 {
                for i in 0..7i32 {
                    px96(
                        &mut img,
                        cx + k * 4,
                        body_cy + body_h + 3 + i,
                        b.accent.base,
                    );
                }
            }
        }
        Plan::Serpent => {
            for i in 0..36i32 {
                let t = f64::from(i) / 8.0;
                let sx = cx + ((t * 2.0).cos() * f64::from(body_w)) as i32;
                let sy = body_cy + body_h - i * 2;
                let r = if i % 6 < 3 { &b.ramp } else { &b.accent };
                blot_shaded(&mut img, sx, sy, 8, 6, r);
            }
            blot_shaded(&mut img, cx, head_cy + 8, head_r, head_r - 2, &b.ramp);
            head_features(&mut img, &b, false);
        }
        Plan::Moth => {
            // wings dominate from behind
            for side in [-1i32, 1] {
                blot_shaded(
                    &mut img,
                    cx + side * (body_w + 4),
                    body_cy - 11,
                    body_w,
                    body_h + 2,
                    &b.accent,
                );
                blot_shaded(
                    &mut img,
                    cx + side * (body_w + 1),
                    body_cy + 7,
                    body_w - 5,
                    body_h - 3,
                    &b.accent,
                );
                // back-of-wing veining
                for i in 0..body_h {
                    px96(
                        &mut img,
                        cx + side * (body_w + 4),
                        body_cy - 11 + i - body_h / 2,
                        b.accent.dark,
                    );
                }
            }
            blot_shaded(&mut img, cx, body_cy, 6, body_h + 5, &b.ramp);
            blot_shaded(&mut img, cx, head_cy + 8, head_r - 3, head_r - 3, &b.ramp);
        }
        Plan::Fish => {
            // tail toward the viewer
            for i in 0..13i32 {
                blot_shaded(
                    &mut img,
                    cx + body_w + 2 - i / 2,
                    body_cy - 8 - i + 6,
                    3,
                    4,
                    &b.accent,
                );
                blot_shaded(
                    &mut img,
                    cx + body_w + 2 - i / 2,
                    body_cy - 8 + i - 6,
                    3,
                    4,
                    &b.accent,
                );
            }
            blot_shaded(&mut img, cx, body_cy - 8, body_w + 3, body_h, &b.ramp);
            for i in 0..body_w {
                px96(
                    &mut img,
                    cx - body_w / 2 + i,
                    body_cy - 9 - body_h - i % 4,
                    b.accent.base,
                );
            }
        }
        Plan::Blob => {
            blot_shaded(&mut img, cx, body_cy - 4, body_w + 3, body_h + 7, &b.ramp);
            for i in 0..8i32 {
                px96(
                    &mut img,
                    cx,
                    body_cy - body_h - 11 - i,
                    shade(hex(0x6a9a4e), 0.8),
                );
            }
            // back freckle band
            let mut rng = BattleRng::from_seed(seed ^ 0xb10c);
            for _ in 0..9 {
                let fx = cx - body_w + 4 + rng.below((b.body_w as u32) * 2 - 8) as i32;
                let fy = body_cy - 8 + rng.below(b.body_h as u32) as i32 - body_h / 2;
                px96(&mut img, fx, fy, b.accent.dark);
            }
        }
        Plan::Golem => {
            blot_shaded(&mut img, cx, body_cy + 6, body_w + 5, body_h - 2, &b.ramp);
            blot_shaded(&mut img, cx, body_cy - 8, body_w, body_h - 3, &b.ramp);
            for side in [-1i32, 1] {
                blot_shaded(
                    &mut img,
                    cx + side * (body_w + 8),
                    body_cy - 4,
                    5,
                    11,
                    &b.ramp,
                );
            }
            blot_shaded(
                &mut img,
                cx,
                body_cy - body_h - 10,
                head_r,
                head_r - 1,
                &b.ramp,
            );
            // back seam
            for y in (body_cy - body_h - 4)..(body_cy + body_h) {
                px96(&mut img, cx, y, b.ramp.dark);
            }
        }
        Plan::Wisp => {
            for i in 0..body_h * 2 {
                let w = body_w - i / 3;
                if w <= 2 {
                    break;
                }
                blot_shaded(&mut img, cx, head_cy + 8 + i, w, 2, &b.ramp);
            }
            blot_shaded(&mut img, cx, head_cy + 4, head_r + 2, head_r, &b.ramp);
            head_features(&mut img, &b, false);
        }
    }

    outline(&mut img);
    img
}

// ----- entry points -------------------------------------------------------

/// The battle ground pad (a soft shadow ellipse, drawn once).
pub fn render_platform(out: &Path) -> Result<()> {
    std::fs::create_dir_all(out).with_context(|| format!("creating {}", out.display()))?;
    let (w, h) = (120u32, 32u32);
    let mut img = RgbaImage::new(w, h);
    let (cx, cy) = (w as i32 / 2, h as i32 / 2);
    for y in 0..h as i32 {
        for x in 0..w as i32 {
            let dx = f64::from(x - cx) / f64::from(cx - 2);
            let dy = f64::from(y - cy) / f64::from(cy - 2);
            let d = dx * dx + dy * dy;
            if d <= 1.0 {
                let mut c = shade(PARCHMENT, 0.82);
                if d > 0.78 {
                    c = shade(PARCHMENT, 0.7);
                }
                c[3] = 230;
                img.put_pixel(x as u32, y as u32, c);
            }
        }
    }
    img.save(out.join("platform.png"))
        .context("writing platform")
}

pub fn render_tiles(out: &Path) -> Result<usize> {
    std::fs::create_dir_all(out).with_context(|| format!("creating {}", out.display()))?;
    let kinds = [
        "grass",
        "patch",
        "path",
        "water",
        "deep",
        "floor",
        "wall",
        "bush",
        "sign",
        "canopy",
        "roof_red",
        "roof_blue",
        "roof_green",
        "door",
        "window",
        "flowers",
        "fence",
        "lamp",
        "wall_indoor",
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
        let back = creature_back(
            motif.sigil_seed,
            motif.types[0],
            motif.types.get(1).copied(),
            &motif.tags,
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
