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

// P19 palette audit: the world had washed into pastel midtones
// (value p5≈141, median saturation 0.50 vs a reference p5≈61 /
// sat 0.84). World painters now carry deep shadow accents (value
// ~70-100), bright highlights (~235+), and richer base saturation.
// The parchment UI ramp (INK/PARCHMENT/GILT) is identity — untouched.

/// The grass family ramp — shared by grass/grass.1/grass.2, the
/// fringe pieces, and anything that wants to read as "lawn".
const GRASS_BASE: u32 = 0x49a832; // value 168, sat .70
const GRASS_DARK: u32 = 0x2c641e; // value 100 — tuft shadow
const GRASS_DEEP: u32 = 0x1d4a14; // value 74 — deepest accent
const GRASS_LIGHT: u32 = 0x7fd24c; // value 210 — lit blade
const GRASS_GLINT: u32 = 0xb8ef84; // value 239 — sun glint

/// Water base shared by `tile()` and the phase-shifted `water.1.png`
/// frame in `render_tiles` — one constant so they can never drift.
const WATER_BASE: u32 = 0x2766ae; // value 174, sat .78

/// Grass surface shared by the three variant tiles. `tufts` varies
/// per variant so the siblings read related but not identical.
fn grass_surface(img: &mut RgbaImage, rng: &mut BattleRng, tufts: u32) {
    let base = hex(GRASS_BASE);
    for y in 0..TILE {
        for x in 0..TILE {
            img.put_pixel(x, y, base);
        }
    }
    for _ in 0..90 {
        let x = rng.below(TILE);
        let y = rng.below(TILE);
        img.put_pixel(x, y, shade(base, 0.86));
    }
    // blade tufts: dark crook with a deep root and a lit tip
    for _ in 0..tufts {
        let x = 2 + rng.below(TILE - 4);
        let y = 2 + rng.below(TILE - 4);
        img.put_pixel(x, y, hex(GRASS_DARK));
        img.put_pixel(x, y + 1, hex(GRASS_DEEP));
        img.put_pixel(x + 1, y + 1, hex(GRASS_LIGHT));
    }
    // a few sun glints push the highlight end of the ramp
    for _ in 0..4 {
        let x = rng.below(TILE);
        let y = rng.below(TILE);
        img.put_pixel(x, y, hex(GRASS_GLINT));
    }
}

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
        // three siblings from one painter; tuft count + seed differ
        "grass" => grass_surface(&mut img, rng, 10),
        "grass.1" => grass_surface(&mut img, rng, 13),
        "grass.2" => grass_surface(&mut img, rng, 8),
        "patch" => {
            // tall grass: clearly darker than every lawn variant
            let base = hex(0x2e7224); // value 114 vs lawn's 168
            fill(base, shade(base, 0.78), 110);
            for _ in 0..14 {
                let x = 1 + rng.below(TILE - 3);
                let y = 1 + rng.below(TILE - 4);
                let dark = hex(0x1a4a12); // value 74
                img.put_pixel(x, y + 2, dark);
                img.put_pixel(x + 1, y, dark);
                img.put_pixel(x + 1, y + 1, dark);
                img.put_pixel(x + 2, y + 2, shade(base, 1.3));
            }
        }
        "path" | "path.1" => {
            let base = hex(0xd8bc8a); // warmer, more saturated dirt
            fill(base, shade(base, 0.88), 70);
            let pebbles = if kind == "path" { 6 } else { 8 };
            for _ in 0..pebbles {
                let x = rng.below(TILE - 3);
                let y = rng.below(TILE - 2);
                let pebble = hex(0x6b5436); // value 107 — real shadow
                img.put_pixel(x + 1, y, pebble);
                img.put_pixel(x, y + 1, pebble);
                img.put_pixel(x + 1, y + 1, shade(base, 1.12));
            }
        }
        "water" | "deep" => {
            let base = if kind == "deep" {
                shade(hex(WATER_BASE), 0.62)
            } else {
                hex(WATER_BASE)
            };
            water_surface(&mut img, base, rng, 0);
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
            let base = hex(0x35862a); // value 134, sat .69
            let c = (TILE / 2) as i32;
            for y in 0..TILE {
                for x in 0..TILE {
                    let dx = x as i32 - c;
                    let dy = y as i32 - c + 2;
                    if dx * dx + dy * dy < 165 {
                        let f = if (x * 7 + y * 13 + rng.below(3)).is_multiple_of(9) {
                            1.55 // lit leaf clusters
                        } else if (x + y).is_multiple_of(5) {
                            0.58 // shadow pockets, value ~78
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
            let base = hex(0x26621e); // value 98 — overhead shade
            let c = (TILE / 2) as i32;
            for y in 0..TILE {
                for x in 0..TILE {
                    let dx = x as i32 - c;
                    let dy = y as i32 - c;
                    if dx * dx + dy * dy < 240 {
                        let f = if (x * 5 + y * 11).is_multiple_of(7) {
                            1.5 // sun breaking through
                        } else if (x * 3 + y * 7).is_multiple_of(11) {
                            0.72 // deep leaf shadow
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
                "roof_red" => hex(0xc24a2e),   // sat .76
                "roof_blue" => hex(0x3a6cb4),  // sat .68
                _ => hex(0x4a8c34),            // sat .63
            };
            fill(base, shade(base, 0.92), 30);
            // shingle courses: deep shadow line, lit edge beneath it
            for (row, y) in (0..TILE).step_by(6).enumerate() {
                for x in 0..TILE {
                    img.put_pixel(x, y, shade(base, 0.5));
                    if y + 1 < TILE {
                        img.put_pixel(x, y + 1, shade(base, 1.22));
                    }
                }
                let offset = if row % 2 == 0 { 3 } else { 9 };
                for x in (offset..TILE).step_by(12) {
                    for dy in 2..6u32 {
                        if y + dy < TILE {
                            img.put_pixel(x, y + dy, shade(base, 0.76));
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
        "door_open" => {
            // the `door` tile with the leaf swung away: same parchment
            // wall, ink frame and posts, but a warm lit interior
            let wall = shade(PARCHMENT, 0.92);
            fill(wall, shade(wall, 0.96), 16);
            let glow = hex(0xf2d06b);
            for y in 6..TILE {
                for x in 8..24u32 {
                    let edge = !(10..=21).contains(&x) || y < 9;
                    let c = if edge {
                        shade(glow, 0.74)
                    } else if (x + y) % 7 == 0 {
                        shade(glow, 1.1)
                    } else {
                        glow
                    };
                    img.put_pixel(x, y, c);
                }
            }
            for y in 5..TILE {
                img.put_pixel(7, y, INK);
                img.put_pixel(24, y, INK);
            }
            for x in 7..25u32 {
                img.put_pixel(x, 5, INK);
            }
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
            let blooms = [hex(0xe04458), hex(0xffd23e), hex(0xfaf3e2), hex(0xa86ed4)];
            for (i, &bloom) in blooms.iter().enumerate() {
                for _ in 0..3 {
                    let x = 2 + rng.below(TILE - 5);
                    let y = 2 + rng.below(TILE - 5);
                    img.put_pixel(x + 1, y, bloom);
                    img.put_pixel(x, y + 1, bloom);
                    img.put_pixel(x + 2, y + 1, bloom);
                    img.put_pixel(x + 1, y + 2, bloom);
                    img.put_pixel(x + 1, y + 1, shade(blooms[(i + 1) % 4], 1.1));
                    img.put_pixel(x + 1, y + 3, hex(0x2a5a1c));
                }
            }
        }
        "fence" => {
            let wood = hex(0x91612f); // sat .68 warm timber
            for y in 12..26u32 {
                for x in 0..TILE {
                    if x % 8 < 3 {
                        img.put_pixel(x, y, if x % 8 == 1 { wood } else { shade(wood, 0.55) });
                    }
                }
            }
            for x in 0..TILE {
                img.put_pixel(x, 15, shade(wood, 1.28));
                img.put_pixel(x, 16, wood);
                img.put_pixel(x, 21, shade(wood, 1.28));
                img.put_pixel(x, 22, shade(wood, 0.7));
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
                    // hot core, warm rim — the glass should glow
                    let core = (5..=7).contains(&y) && (14..=17).contains(&x);
                    img.put_pixel(x, y, if core { hex(0xfff1a0) } else { hex(0xffd84a) });
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

/// Water painter shared by the tile and its shimmer frame: base fill,
/// sparkle specks, then wave highlight strokes. `phase` slides the
/// strokes sideways (alternating direction per row) — with the same
/// seed, phase 0 is the classic tile and phase 3 is frame 2, so the
/// two differ only in where the highlights sit.
fn water_surface(img: &mut RgbaImage, base: Rgba<u8>, rng: &mut BattleRng, phase: u32) {
    for y in 0..TILE {
        for x in 0..TILE {
            img.put_pixel(x, y, base);
        }
    }
    let speck = shade(base, 1.18);
    for _ in 0..40 {
        let x = rng.below(TILE);
        let y = rng.below(TILE);
        img.put_pixel(x, y, speck);
    }
    // dark wave troughs (P19 audit): static between the two shimmer
    // frames — only the highlights slide — value lands near 85.
    for row in 0..4u32 {
        let y = 8 + row * 8 + rng.below(2);
        let x0 = rng.below(TILE);
        let trough = shade(base, 0.48);
        for dx in 0..(5 + rng.below(5)) {
            let x = (x0 + dx) % TILE;
            if y < TILE {
                img.put_pixel(x, y, trough);
            }
        }
    }
    // wave strokes
    for row in 0..4u32 {
        let y = 4 + row * 8 + rng.below(3);
        let x0 = rng.below(TILE / 2);
        let light = shade(base, 1.45);
        let slide = if row % 2 == 0 { phase } else { TILE - phase };
        for dx in 0..(6 + rng.below(6)) {
            let x = (x0 + dx + slide) % TILE;
            if y < TILE {
                img.put_pixel(x, y, light);
            }
        }
    }
    // a few bright sparkle glints riding the highlight strokes
    for _ in 0..5 {
        let x = rng.below(TILE);
        let y = rng.below(TILE);
        img.put_pixel(x, y, shade(base, 1.85));
    }
}

// ----- grass fringes (P19) ------------------------------------------------

/// One fringe pixel from the grass ramp — jagged-edge color mixing
/// shared by the edge lips and the corner nibs.
fn fringe_pixel(x: u32, y: u32, tip: bool) -> Rgba<u8> {
    if tip {
        hex(GRASS_DEEP) // silhouette edge reads dark against path/water
    } else if (x * 3 + y * 7).is_multiple_of(9) {
        hex(GRASS_LIGHT)
    } else if (x + y * 5).is_multiple_of(6) {
        hex(GRASS_DARK)
    } else {
        hex(GRASS_BASE)
    }
}

/// A lip of grass tufts spilling over the TOP edge of the image (the
/// `_n` piece — the grass neighbor sits above on screen, which is row
/// 0 in PNG coordinates). Jagged 2-4px teeth over a 2px solid lip,
/// so the spill reaches rows 0..≈6. `waterline` lays a 1px darker
/// water shade under the teeth for the fringe_water pieces. The other
/// edges are rigid rotations/flips of this painting (each with its
/// own seed, so no two read as mirrors).
fn fringe_edge(rng: &mut BattleRng, waterline: bool) -> RgbaImage {
    let mut img = RgbaImage::new(TILE, TILE);
    // tooth depth per 2-3px run — irregular, never a straight strip
    let mut depths = [0u32; TILE as usize];
    let mut x = 0usize;
    while x < TILE as usize {
        let run = 2 + rng.below(2) as usize;
        let depth = 2 + rng.below(3); // 2-4px teeth
        for i in 0..run {
            if x + i < TILE as usize {
                depths[x + i] = depth;
            }
        }
        x += run;
    }
    for (ix, &d) in depths.iter().enumerate() {
        let x = u32::try_from(ix).expect("tile column");
        for y in 0..(2 + d) {
            img.put_pixel(x, y, fringe_pixel(x, y, y == 1 + d));
        }
        if waterline {
            img.put_pixel(x, 2 + d, shade(hex(WATER_BASE), 0.5));
        }
    }
    img
}

/// A small tuft nib in the TOP-LEFT image corner (the `_nw` piece —
/// grass only on the diagonal above-left). The other corners are
/// flips of this painting.
fn fringe_corner(rng: &mut BattleRng, waterline: bool) -> RgbaImage {
    let mut img = RgbaImage::new(TILE, TILE);
    for y in 0..7u32 {
        let w = 7u32.saturating_sub(y) + rng.below(2);
        for x in 0..w.min(TILE) {
            img.put_pixel(x, y, fringe_pixel(x, y, x + 1 == w || y == 6));
        }
        if waterline && w < TILE {
            img.put_pixel(w, y, shade(hex(WATER_BASE), 0.5));
        }
    }
    if waterline {
        for x in 0..3u32 {
            img.put_pixel(x, 7, shade(hex(WATER_BASE), 0.5));
        }
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

/// Temperament proxy (P19): catch rate maps to the mouth line — easy
/// to befriend species smile, hard ones scowl, the middle stays flat.
#[derive(Clone, Copy)]
enum Mood {
    Docile,
    Neutral,
    Ornery,
}

fn mood(catch_rate: u8) -> Mood {
    if catch_rate >= 130 {
        Mood::Docile
    } else if catch_rate <= 60 {
        Mood::Ornery
    } else {
        Mood::Neutral
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
    seed: u64,
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
    /// P19 pose: head lean in px (±2). Mirrored between front and back
    /// so both views lean the same way in creature space.
    tilt: i32,
    /// P19 pose: the side the creature favors (±1) — tail sweep,
    /// pricked ear, raised wing/arm all agree.
    flip: i32,
    /// P19 pose: how strongly the favored side rides up (1–3 px).
    perk: i32,
    /// P19 species kit: which of the plan's three surface details.
    kit: u32,
    mood: Mood,
}

fn body_from_seed(
    seed: u64,
    primary: Type,
    secondary: Option<Type>,
    tags: &[String],
    catch_rate: u8,
) -> Body {
    let mut rng = BattleRng::from_seed(seed ^ 0x0059_217e);
    let plan = body_plan(primary, tags);
    let base = type_color(primary);
    let accent = secondary.map(type_color).unwrap_or(shade(base, 1.3));
    let body_w = 15 + rng.below(9) as i32;
    let body_h = 13 + rng.below(9) as i32;
    let body_cy = 58i32 - i32::from(plan == Plan::Bird) * 4;
    let head_r = 10 + rng.below(5) as i32;
    let ear_kind = rng.below(3);
    let tail_kind = rng.below(3);
    let crest = rng.chance(2, 5);
    // P19 pose draws come after the v2 ones so bodies keep their builds.
    let tilt = rng.below(5) as i32 - 2;
    let flip = if rng.chance(1, 2) { 1 } else { -1 };
    let perk = 1 + rng.below(3) as i32;
    let kit = rng.below(3);
    Body {
        plan,
        seed,
        cx: 48,
        body_w,
        body_h,
        body_cy,
        head_r,
        head_cy: body_cy - body_h - head_r + 7,
        ramp: ramp(base),
        accent: ramp(accent),
        belly: shade(PARCHMENT, 0.97),
        ear_kind,
        tail_kind,
        crest,
        tilt,
        flip,
        perk,
        kit,
        mood: mood(catch_rate),
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

/// Ears/horns/crest kit, used by quadruped-ish heads. P19: the head
/// carries the species tilt, the favored ear pricks up (taller) while
/// the other settles, and the horn leans toward the favored side.
/// Features are planted on the actual head surface of each plan, so
/// nothing floats when the pose lifts it.
fn head_features(img: &mut RgbaImage, b: &Body, facing_front: bool) {
    let f = if facing_front { 1 } else { -1 };
    let hx = b.cx + b.tilt * f;
    // where this plan really draws its head blot
    let (hcy, rx_h, ry_h) = match b.plan {
        Plan::Serpent => (b.head_cy + 8, b.head_r, b.head_r - 2),
        Plan::Wisp => (b.head_cy + 4, b.head_r + 2, b.head_r),
        Plan::Golem => (b.body_cy - b.body_h - 10, b.head_r, b.head_r - 1),
        _ => (b.head_cy, b.head_r, b.head_r),
    };
    // head-surface height at offset x (ellipse), for planting ears
    let surf = |x_off: i32| -> i32 {
        let t = f64::from(x_off) / f64::from(rx_h);
        hcy - (f64::from(ry_h) * (1.0 - t * t).sqrt()) as i32
    };
    match b.ear_kind {
        0 => {
            // pointed ears, the favored one pricked taller
            for side in [-1i32, 1] {
                let lift = if side == b.flip * f { b.perk } else { -1 };
                let ex = hx + side * (rx_h - 3);
                let base = surf(rx_h - 3) + 2;
                let rows = 7 + lift;
                for i in 0..rows {
                    let w = ((rows - i) / 2).min(3);
                    for dx in -w..=w {
                        px96(
                            img,
                            ex + dx,
                            base - i,
                            if i > rows - 3 {
                                b.accent.base
                            } else {
                                b.ramp.base
                            },
                        );
                    }
                }
            }
        }
        1 => {
            // round ears, one riding a touch higher
            for side in [-1i32, 1] {
                let lift = if side == b.flip * f { b.perk } else { -1 };
                let ex = hx + side * (rx_h - 2);
                blot_shaded(img, ex, surf(rx_h - 2) - lift, 4, 4, &b.accent);
            }
        }
        _ => {
            // single horn, leaning toward the favored side
            for i in 0..9i32 {
                let w = (9 - i) / 3;
                for dx in -w..=w {
                    px96(
                        img,
                        hx + dx + b.flip * f * (i / 4),
                        hcy - ry_h - i + 4,
                        b.accent.light,
                    );
                }
            }
        }
    }
    if b.crest && facing_front {
        for i in 0..5i32 {
            px96(img, hx - 2 + i, hcy - ry_h, b.accent.dark);
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

/// Temperament mouth (P19): a 1-px ink line. Docile corners lift into
/// a small smile, ornery corners drop over a parted-open second row,
/// neutral stays flat. `w` is the half-width.
fn mouth_line(img: &mut RgbaImage, cx: i32, y: i32, w: i32, mood: Mood) {
    match mood {
        Mood::Docile => {
            for dx in -w..=w {
                px96(img, cx + dx, y - i32::from(dx.abs() == w), INK);
            }
        }
        Mood::Neutral => {
            for dx in -(w - 1)..=(w - 1) {
                px96(img, cx + dx, y, INK);
            }
        }
        Mood::Ornery => {
            for dx in -w..=w {
                px96(img, cx + dx, y + i32::from(dx.abs() == w), INK);
            }
            for dx in -(w - 1)..=(w - 1) {
                px96(img, cx + dx, y + 1, INK);
            }
        }
    }
}

/// P19 species kit: each plan owns three small surface details — the
/// seed picks one and places it. Drawn over the body, under the eyes
/// and outline, in both views wherever the detail would actually show.
/// Geometry mirrors the plan painters (legs, wings, coils), so the
/// formulas here must stay in sync with `creature_front`/`_back`.
#[expect(clippy::too_many_lines, reason = "one kit pool, eight plans")]
fn species_kit(img: &mut RgbaImage, b: &Body, front: bool) {
    let f = if front { 1 } else { -1 };
    let hx = b.cx + b.tilt * f;
    let (cx, body_cy, body_w, body_h, head_r, head_cy) =
        (b.cx, b.body_cy, b.body_w, b.body_h, b.head_r, b.head_cy);
    let mut rng = BattleRng::from_seed(b.seed ^ 0x6b17);
    match (b.plan, b.kit) {
        (Plan::Quadruped, 0) if front => {
            // whisker dots beside the muzzle (a face detail: front only)
            for side in [-1i32, 1] {
                for i in 0..3i32 {
                    px96(
                        img,
                        hx + side * (head_r / 2 + 2 + i * 2),
                        head_cy + head_r / 2 + (i % 2),
                        INK,
                    );
                }
            }
        }
        (Plan::Quadruped, 1) => {
            // back ridge: accent teeth along the spine curve, parting
            // around the head
            let (rx, ry) = if front {
                (body_w, body_h)
            } else {
                (body_w + 2, body_h + 2)
            };
            let mut dx = -rx + 4;
            while dx <= rx - 4 {
                if (cx + dx - hx).abs() >= head_r {
                    let t = f64::from(dx) / f64::from(rx);
                    let top = body_cy - (f64::from(ry) * (1.0 - t * t).sqrt()) as i32;
                    px96(img, cx + dx, top, b.accent.dark);
                    px96(img, cx + dx + 1, top, b.accent.dark);
                    px96(img, cx + dx, top - 1, b.accent.base);
                }
                dx += 5;
            }
        }
        (Plan::Quadruped, 2) => {
            // leg banding above the paws
            let (offs, legs, w) = if front {
                ([-body_w + 4, body_w - 8], 2, 3)
            } else {
                ([-body_w + 2, body_w - 7], 1, 5)
            };
            for off in offs {
                for leg in 0..legs {
                    let lx = cx + off + leg * 5;
                    for y in [body_cy + body_h + 4, body_cy + body_h + 7] {
                        for dx in 0..w {
                            px96(img, lx + dx, y, b.accent.dark);
                        }
                    }
                }
            }
        }
        (Plan::Bird, 0) => {
            // wing bars across each wing
            for side in [-1i32, 1] {
                let lift = if side == b.flip * f { b.perk } else { 0 };
                if front {
                    for j in [body_w / 2, body_w + 2] {
                        let wx = cx + side * (body_w / 2 + j);
                        let wy = body_cy - 7 - j / 2 - lift;
                        for dy in -2..=2i32 {
                            px96(img, wx, wy + dy, b.accent.dark);
                            px96(img, wx + side, wy + dy, b.accent.dark);
                        }
                    }
                } else {
                    let pcx = cx + side * (body_w / 2 + 2);
                    for yy in [body_cy - 5 - lift, body_cy + 1 - lift] {
                        for dx in -3..=3i32 {
                            px96(img, pcx + dx, yy, b.accent.dark);
                        }
                    }
                }
            }
        }
        (Plan::Bird, 1) if front => {
            // chest speckles (front only — the breast faces the viewer)
            let rx = (body_w - 9).max(2);
            let ry = (body_h - 5).max(2);
            for _ in 0..6 {
                let dx = rng.below(rx as u32 * 2 + 1) as i32 - rx;
                let dy = rng.below(ry as u32 * 2 + 1) as i32 - ry;
                px96(img, cx + dx, body_cy + 4 + dy, b.accent.dark);
            }
        }
        (Plan::Bird, 2) => {
            // tips dipped dark: wing ends (front) / tail feathers (back)
            if front {
                for side in [-1i32, 1] {
                    let lift = if side == b.flip * f { b.perk } else { 0 };
                    for j in body_w + 4..body_w + 7 {
                        let wx = cx + side * (body_w / 2 + j);
                        let wy = body_cy - 7 - j / 2 - lift;
                        for dy in -1..=1i32 {
                            px96(img, wx, wy + dy, b.ramp.dark);
                            px96(img, wx - side, wy + dy, b.ramp.dark);
                        }
                    }
                }
            } else {
                for k in -1..=1i32 {
                    for i in 5..7i32 {
                        px96(
                            img,
                            cx + k * 4 - b.flip * (i / 3),
                            body_cy + body_h + 3 + i,
                            b.ramp.dark,
                        );
                    }
                }
            }
        }
        (Plan::Serpent, 0) => {
            // light diamonds along the body-colored coils
            for i in 0..36i32 {
                if i % 6 != 1 {
                    continue;
                }
                let t = f64::from(i) / 8.0;
                let wave = if front { (t * 2.0).sin() } else { (t * 2.0).cos() };
                let sx = cx + b.flip * f * ((wave * f64::from(body_w)) as i32);
                let sy = body_cy + body_h - i * 2;
                for ddy in -2..=2i32 {
                    let w = 2 - ddy.abs();
                    for ddx in -w..=w {
                        px96(img, sx + ddx, sy + ddy, b.ramp.light);
                    }
                }
            }
        }
        (Plan::Serpent, 1) => {
            // tail rings on the lowest coils
            for i in [0i32, 2] {
                let t = f64::from(i) / 8.0;
                let wave = if front { (t * 2.0).sin() } else { (t * 2.0).cos() };
                let sx = cx + b.flip * f * ((wave * f64::from(body_w)) as i32);
                let sy = body_cy + body_h - i * 2;
                for ddx in -5..=5i32 {
                    px96(img, sx + ddx, sy, b.ramp.light);
                    px96(img, sx + ddx, sy + 2, b.accent.dark);
                }
            }
        }
        (Plan::Serpent, 2) => {
            // hood slashes flanking the head; a nape slash from behind
            if front {
                for side in [-1i32, 1] {
                    for j in 0..5i32 {
                        px96(img, hx + side * (head_r - 2), head_cy + 4 + j, b.accent.dark);
                        px96(img, hx + side * (head_r - 1), head_cy + 5 + j, b.accent.dark);
                    }
                }
            } else {
                for j in 0..7i32 {
                    px96(img, hx, head_cy + 4 + j, b.accent.dark);
                    px96(img, hx + 1, head_cy + 4 + j, b.accent.dark);
                }
            }
        }
        (Plan::Moth, 0) => {
            // forewing eyespots: dark diamond, light pupil
            for side in [-1i32, 1] {
                let lift = if side == b.flip * f { b.perk } else { 0 };
                let ex = cx + side * (body_w + 8);
                let ey = body_cy - 11 - lift;
                for ddy in -3..=3i32 {
                    let w = 3 - ddy.abs();
                    for ddx in -w..=w {
                        px96(img, ex + ddx, ey + ddy, b.ramp.dark);
                    }
                }
                px96(img, ex, ey, b.ramp.light);
                px96(img, ex + 1, ey, b.ramp.light);
                px96(img, ex, ey + 1, b.ramp.light);
            }
        }
        (Plan::Moth, 1) => {
            // dark rim along each forewing's outer edge
            for side in [-1i32, 1] {
                let lift = if side == b.flip * f { b.perk } else { 0 };
                let (wcx, wcy) = (cx + side * (body_w + 4), body_cy - 11 - lift);
                let (rx, ry) = if front {
                    (body_w - 1, body_h + 1)
                } else {
                    (body_w, body_h + 2)
                };
                for ddy in -(ry - 2)..=(ry - 2) {
                    let t = f64::from(ddy) / f64::from(ry);
                    let edge = (f64::from(rx) * (1.0 - t * t).sqrt()) as i32;
                    px96(img, wcx + side * (edge - 1), wcy + ddy, b.ramp.dark);
                    px96(img, wcx + side * (edge - 2), wcy + ddy, b.ramp.dark);
                }
            }
        }
        (Plan::Moth, 2) => {
            // light fuzz bands across the thorax
            let bw = if front { 5 } else { 4 };
            for yy in [body_cy - 5, body_cy, body_cy + 5] {
                for dx in -bw..=bw {
                    px96(img, cx + dx, yy, b.ramp.light);
                }
            }
        }
        (Plan::Fish, 0) => {
            // vertical flank bars; the front pair sits outboard of the
            // face so it never cages the eyes
            let (rx, ry) = if front {
                (body_w + 5, body_h + 1)
            } else {
                (body_w + 3, body_h)
            };
            let xs: &[i32] = if front {
                &[-1, 1]
            } else {
                &[-1, 0, 1]
            };
            for &k in xs {
                let x = if front { cx + k * body_w } else { cx + k * (rx / 2) };
                let t = f64::from(x - cx) / f64::from(rx);
                let half = ((f64::from(ry) * (1.0 - t * t).sqrt()) as i32 - 1).max(1);
                for dy in -half..=half {
                    px96(img, x, body_cy - 8 + dy, b.accent.dark);
                    px96(img, x + 1, body_cy - 8 + dy, b.accent.dark);
                }
            }
        }
        (Plan::Fish, 1) => {
            // scale speckles across the body (ramp-dark: reads on both
            // the body and the pale belly, even for monotypes)
            let rx = (body_w - 2).max(2);
            let ry = (body_h - 3).max(2);
            for _ in 0..8 {
                let dx = rng.below(rx as u32 * 2 + 1) as i32 - rx;
                let dy = rng.below(ry as u32 * 2 + 1) as i32 - ry;
                if dx * dx * ry * ry + dy * dy * rx * rx <= rx * rx * ry * ry {
                    px96(img, cx + dx, body_cy - 8 + dy, b.ramp.dark);
                }
            }
        }
        (Plan::Fish, 2) if front => {
            // gill slashes behind the face (front only)
            for side in [-1i32, 1] {
                for j in 0..6i32 {
                    px96(img, cx + side * (body_w - 2), body_cy - 11 + j, b.ramp.dark);
                    px96(img, cx + side * (body_w - 3), body_cy - 10 + j, b.ramp.dark);
                }
            }
        }
        (Plan::Blob, 0) if front => {
            // belly patch low on the front
            let belly = ramp(b.belly);
            blot_shaded(
                img,
                cx,
                body_cy + 4,
                (body_w - 5).max(3),
                (body_h - 1).max(3),
                &belly,
            );
        }
        (Plan::Blob, 1) => {
            // two large accent spots at the flanks
            for side in [-1i32, 1] {
                blot_shaded(img, cx + side * (body_w - 3), body_cy - 2, 3, 4, &b.accent);
            }
        }
        (Plan::Blob, 2) => {
            // sproutlets flanking the main sprout
            let sx = cx + b.tilt * f;
            for side in [-1i32, 1] {
                for i in 0..4i32 {
                    px96(
                        img,
                        sx + side * 4,
                        body_cy - body_h - 8 - i,
                        shade(hex(0x6a9a4e), 0.8),
                    );
                }
                px96(img, sx + side * 3, body_cy - body_h - 12, b.accent.base);
                px96(img, sx + side * 4, body_cy - body_h - 13, b.accent.base);
                px96(img, sx + side * 5, body_cy - body_h - 12, b.accent.base);
            }
        }
        (Plan::Golem, 0) => {
            // a gilt rune on the chest; a stud between the shoulders
            if front {
                for ddy in -3..=3i32 {
                    px96(img, cx, body_cy - 10 + ddy, GILT);
                }
                px96(img, cx - 1, body_cy - 12, GILT);
                px96(img, cx - 2, body_cy - 11, GILT);
                px96(img, cx + 1, body_cy - 9, GILT);
                px96(img, cx + 2, body_cy - 8, GILT);
            } else {
                for (ddx, ddy) in [(0i32, 0i32), (1, 0), (0, 1), (1, 1)] {
                    px96(img, cx + ddx, body_cy - body_h - 3 + ddy, GILT);
                }
            }
        }
        (Plan::Golem, 1) => {
            // moss patches on the shoulders
            for side in [-1i32, 1] {
                blot_shaded(
                    img,
                    cx + side * (body_w - 4),
                    body_cy - body_h + 2,
                    3,
                    2,
                    &b.accent,
                );
                px96(img, cx + side * (body_w - 4), body_cy - body_h + 5, b.accent.dark);
            }
        }
        (Plan::Golem, 2) => {
            // light studs down the arms (following the raised arm)
            for side in [-1i32, 1] {
                let dy = if side == b.flip * f { -2 * b.perk } else { 0 };
                for j in [-6i32, -2, 2] {
                    px96(img, cx + side * (body_w + 8), body_cy - 4 + dy + j, b.ramp.light);
                    px96(
                        img,
                        cx + side * (body_w + 8) - 1,
                        body_cy - 4 + dy + j,
                        b.ramp.light,
                    );
                }
            }
        }
        (Plan::Wisp, 0) => {
            // collar band where the head meets the taper
            let y0 = head_cy + 4 + head_r;
            for dx in -(body_w - 4)..=(body_w - 4) {
                px96(img, cx + dx, y0, b.accent.base);
                px96(img, cx + dx, y0 + 1, b.accent.dark);
            }
        }
        (Plan::Wisp, 1) => {
            // drifting motes below the hem (outlined into ink diamonds)
            let hem = head_cy + 8 + body_h * 2;
            for _ in 0..4 {
                let dx = rng.below(body_w as u32 * 2 + 1) as i32 - body_w;
                let dy = rng.below(6) as i32;
                let (mx, my) = (cx + dx, hem + 3 + dy);
                px96(img, mx, my, b.accent.light);
                px96(img, mx - 1, my, b.accent.light);
                px96(img, mx + 1, my, b.accent.light);
                px96(img, mx, my - 1, b.accent.light);
                px96(img, mx, my + 1, b.accent.light);
            }
        }
        (Plan::Wisp, 2) => {
            // inner glow streak down the center of the shroud
            for i in 2..body_h * 2 - 3 {
                if body_w - i / 3 <= 2 {
                    break;
                }
                px96(img, cx - 1, head_cy + 8 + i, b.ramp.light);
                px96(img, cx, head_cy + 8 + i, b.ramp.light);
            }
        }
        _ => {}
    }
}

/// 96×96 battle-front creature, grammar v3: shaded, feature kits, plus
/// the P19 personality pass — seed-driven pose (head tilt, favored
/// side), one species-kit detail, and a temperament mouth.
#[expect(clippy::too_many_lines, reason = "one body grammar, eight plans")]
fn creature_front(
    seed: u64,
    primary: Type,
    secondary: Option<Type>,
    tags: &[String],
    catch_rate: u8,
) -> RgbaImage {
    let mut img = RgbaImage::new(96, 96);
    let b = body_from_seed(seed, primary, secondary, tags, catch_rate);
    let (cx, body_cy, body_w, body_h, head_r, head_cy) =
        (b.cx, b.body_cy, b.body_w, b.body_h, b.head_r, b.head_cy);
    let hx = cx + b.tilt; // head center: the species' carriage

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
            tail(&mut img, &b, b.flip);
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
            blot_shaded(&mut img, hx, head_cy, head_r, head_r, &b.ramp);
            // muzzle
            blot_shaded(
                &mut img,
                hx,
                head_cy + head_r / 2,
                head_r / 2,
                3,
                &ramp(b.belly),
            );
        }
        Plan::Bird => {
            for side in [-1i32, 1] {
                let lift = if side == b.flip { b.perk } else { 0 };
                for i in 0..body_w + 7 {
                    let wx = cx + side * (body_w / 2 + i);
                    let wy = body_cy - 7 - i / 2 - lift;
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
            blot_shaded(&mut img, hx, head_cy + 4, head_r - 1, head_r - 1, &b.ramp);
            if b.crest {
                for i in 0..6i32 {
                    px96(
                        &mut img,
                        hx - 1 + b.flip * (i / 2),
                        head_cy + 6 - head_r - i,
                        b.accent.base,
                    );
                }
            }
            // beak by temperament: agape when ornery, perked when docile
            match b.mood {
                Mood::Ornery => {
                    for i in 0..5i32 {
                        px96(&mut img, hx - 2 + i, head_cy + 5 + i / 2, GILT);
                        px96(&mut img, hx - 2 + i, head_cy + 6 + i / 2, GILT);
                    }
                    for i in 0..4i32 {
                        px96(&mut img, hx - 2 + i, head_cy + 8, INK);
                    }
                    // the hanging lower mandible
                    for i in 0..3i32 {
                        px96(&mut img, hx - 2 + i, head_cy + 9, shade(GILT, 0.8));
                    }
                    px96(&mut img, hx - 1, head_cy + 10, shade(GILT, 0.8));
                }
                _ => {
                    for i in 0..5i32 {
                        px96(&mut img, hx - 2 + i, head_cy + 7 + i / 2, GILT);
                        px96(&mut img, hx - 1 + i, head_cy + 8 + i / 2, shade(GILT, 0.8));
                    }
                    if matches!(b.mood, Mood::Docile) {
                        px96(&mut img, hx + 3, head_cy + 8, GILT);
                    }
                }
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
                let sx = cx + b.flip * (((t * 2.0).sin() * f64::from(body_w)) as i32);
                let sy = body_cy + body_h - i * 2;
                let r = if i % 6 < 3 { &b.ramp } else { &b.accent };
                blot_shaded(&mut img, sx, sy, 8, 6, r);
            }
            blot_shaded(&mut img, hx, head_cy + 8, head_r, head_r - 2, &b.ramp);
            head_features(&mut img, &b, true);
            // tongue: ornery serpents taste the air; docile keep it in
            match b.mood {
                Mood::Docile => {}
                Mood::Neutral => {
                    px96(&mut img, hx, head_cy + 8 + head_r, hex(0xc4593a));
                    px96(&mut img, hx, head_cy + 9 + head_r, hex(0xc4593a));
                }
                Mood::Ornery => {
                    for j in 0..3i32 {
                        px96(&mut img, hx, head_cy + 8 + head_r + j, hex(0xc4593a));
                    }
                    px96(&mut img, hx - 1, head_cy + 11 + head_r, hex(0xc4593a));
                    px96(&mut img, hx + 1, head_cy + 11 + head_r, hex(0xc4593a));
                }
            }
        }
        Plan::Moth => {
            for side in [-1i32, 1] {
                let lift = if side == b.flip { b.perk } else { 0 };
                blot_shaded(
                    &mut img,
                    cx + side * (body_w + 4),
                    body_cy - 11 - lift,
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
                    body_cy - 11 - lift,
                    5,
                    5,
                    &b.ramp,
                );
            }
            blot_shaded(&mut img, cx, body_cy, 7, body_h + 5, &b.ramp);
            blot_shaded(&mut img, hx, head_cy + 8, head_r - 2, head_r - 2, &b.ramp);
            for side in [-1i32, 1] {
                let bend = if side == b.flip { b.perk } else { 0 };
                for i in 0..9i32 {
                    px96(
                        &mut img,
                        hx + side * (2 + i / 2) + side * ((i * bend) / 9),
                        head_cy + 2 - i,
                        INK,
                    );
                }
                px96(&mut img, hx + side * (6 + bend), head_cy - 7, b.accent.light);
            }
        }
        Plan::Fish => {
            for i in 0..11i32 {
                blot_shaded(
                    &mut img,
                    cx - body_w - 5 - i / 2,
                    body_cy - 8 - i + 5 + b.tilt,
                    2,
                    4,
                    &b.accent,
                );
                blot_shaded(
                    &mut img,
                    cx - body_w - 5 - i / 2,
                    body_cy - 8 + i - 5 + b.tilt,
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
            // side fin on the favored flank
            for i in 0..6i32 {
                px96(
                    &mut img,
                    cx + b.flip * (4 + i),
                    body_cy - 2 + i / 2,
                    b.accent.base,
                );
                px96(
                    &mut img,
                    cx + b.flip * (5 + i),
                    body_cy - 1 + i / 2,
                    b.accent.dark,
                );
            }
        }
        Plan::Blob => {
            blot_shaded(&mut img, cx, body_cy - 4, body_w + 3, body_h + 7, &b.ramp);
            for i in 0..8i32 {
                px96(
                    &mut img,
                    hx,
                    body_cy - body_h - 11 - i,
                    shade(hex(0x6a9a4e), 0.8),
                );
                px96(
                    &mut img,
                    hx + 1,
                    body_cy - body_h - 11 - i,
                    shade(hex(0x6a9a4e), 0.65),
                );
            }
            blot_shaded(&mut img, hx - 5, body_cy - body_h - 17, 5, 3, &b.accent);
            blot_shaded(&mut img, hx + 5, body_cy - body_h - 17, 5, 3, &b.accent);
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
                let dy = if side == b.flip { -2 * b.perk } else { 0 };
                blot_shaded(
                    &mut img,
                    cx + side * (body_w + 8),
                    body_cy - 4 + dy,
                    5,
                    11,
                    &b.ramp,
                );
                // knuckles
                blot_shaded(
                    &mut img,
                    cx + side * (body_w + 8),
                    body_cy + 8 + dy,
                    4,
                    3,
                    &b.accent,
                );
            }
            head_features(&mut img, &b, true);
            blot_shaded(
                &mut img,
                hx,
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
                let kx = cx - body_w / 2 + k * (body_w / 2);
                blot_shaded(
                    &mut img,
                    kx,
                    head_cy + 8 + body_h * 2 - 2 + (k % 2) * 3,
                    2,
                    3,
                    &b.ramp,
                );
            }
            blot_shaded(&mut img, hx, head_cy + 4, head_r + 2, head_r, &b.ramp);
            head_features(&mut img, &b, true);
            for side in [-1i32, 1] {
                let len = if side == b.flip {
                    9 + b.perk * 2
                } else {
                    (8 - b.perk).max(4)
                };
                for i in 0..len {
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

    species_kit(&mut img, &b, true);

    // eyes: ink with parchment glint, slightly larger (v2); they ride
    // the head tilt with everything else on the face
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
        let ex = hx + side * eye_dx;
        for dy in 0..4i32 {
            for dx in 0..3i32 {
                px96(&mut img, ex + dx, eye_y + dy, INK);
            }
        }
        px96(&mut img, ex, eye_y, PARCHMENT);
        px96(&mut img, ex + 1, eye_y + 1, shade(PARCHMENT, 0.8));
    }

    // mouth: the temperament line (P19) — catch rate is the proxy.
    // Birds speak through the beak, drawn in their plan arm above.
    if !matches!(b.plan, Plan::Bird) {
        let (my, mw) = match b.plan {
            Plan::Quadruped => (head_cy + head_r / 2 + 2, 2),
            Plan::Serpent => (head_cy + 8 + (head_r - 2) / 2, 2),
            Plan::Moth => (eye_y + 6, 1),
            Plan::Fish => (body_cy - 3, 2),
            Plan::Blob => (eye_y + 7, 3),
            _ => (eye_y + 7, 2),
        };
        mouth_line(&mut img, hx, my, mw, b.mood);
    }

    outline(&mut img);
    img
}

/// 96×96 REAL back view (P13: the cropped-front fake was the "my
/// monster looks weird" bug): same body, drawn from behind — no face,
/// back markings, head from the rear. P19: the pose and species kit
/// mirror the front (a creature leaning to its left leans screen-left
/// from behind), so both views read as the same animal.
#[expect(clippy::too_many_lines, reason = "one body grammar, eight plans")]
fn creature_back(
    seed: u64,
    primary: Type,
    secondary: Option<Type>,
    tags: &[String],
    catch_rate: u8,
) -> RgbaImage {
    let mut img = RgbaImage::new(96, 96);
    let b = body_from_seed(seed, primary, secondary, tags, catch_rate);
    let (cx, body_cy, body_w, body_h, head_r, head_cy) =
        (b.cx, b.body_cy, b.body_w, b.body_h, b.head_r, b.head_cy);
    let hx = cx - b.tilt; // head tilt, mirrored from behind

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
            tail(&mut img, &b, -b.flip);
            blot_shaded(&mut img, cx, body_cy, body_w + 2, body_h + 2, &b.ramp);
            // spine stripe
            for y in (body_cy - body_h)..(body_cy + body_h - 4) {
                px96(&mut img, cx, y, b.accent.dark);
                px96(&mut img, cx + 1, y, b.accent.base);
            }
            head_features(&mut img, &b, false);
            blot_shaded(&mut img, hx, head_cy, head_r, head_r, &b.ramp);
        }
        Plan::Bird => {
            // folded wings from behind: two shaded panels, the favored
            // one held higher (mirrored)
            blot_shaded(&mut img, cx, body_cy, body_w - 2, body_h + 4, &b.ramp);
            for side in [-1i32, 1] {
                let lift = if side == -b.flip { b.perk } else { 0 };
                blot_shaded(
                    &mut img,
                    cx + side * (body_w / 2 + 2),
                    body_cy - 2 - lift,
                    body_w / 2 + 2,
                    body_h,
                    &b.accent,
                );
            }
            blot_shaded(&mut img, hx, head_cy + 4, head_r - 1, head_r - 1, &b.ramp);
            if b.crest {
                for i in 0..6i32 {
                    px96(
                        &mut img,
                        hx - 1 - b.flip * (i / 2),
                        head_cy + 6 - head_r - i,
                        b.accent.base,
                    );
                }
            }
            // tail feathers, swept toward the favored side (mirrored)
            for k in -1..=1i32 {
                for i in 0..7i32 {
                    px96(
                        &mut img,
                        cx + k * 4 - b.flip * (i / 3),
                        body_cy + body_h + 3 + i,
                        b.accent.base,
                    );
                }
            }
        }
        Plan::Serpent => {
            for i in 0..36i32 {
                let t = f64::from(i) / 8.0;
                let sx = cx - b.flip * (((t * 2.0).cos() * f64::from(body_w)) as i32);
                let sy = body_cy + body_h - i * 2;
                let r = if i % 6 < 3 { &b.ramp } else { &b.accent };
                blot_shaded(&mut img, sx, sy, 8, 6, r);
            }
            blot_shaded(&mut img, hx, head_cy + 8, head_r, head_r - 2, &b.ramp);
            head_features(&mut img, &b, false);
        }
        Plan::Moth => {
            // wings dominate from behind
            for side in [-1i32, 1] {
                let lift = if side == -b.flip { b.perk } else { 0 };
                blot_shaded(
                    &mut img,
                    cx + side * (body_w + 4),
                    body_cy - 11 - lift,
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
                        body_cy - 11 - lift + i - body_h / 2,
                        b.accent.dark,
                    );
                }
            }
            blot_shaded(&mut img, cx, body_cy, 6, body_h + 5, &b.ramp);
            blot_shaded(&mut img, hx, head_cy + 8, head_r - 3, head_r - 3, &b.ramp);
        }
        Plan::Fish => {
            // tail toward the viewer, carrying the same vertical sway
            for i in 0..13i32 {
                blot_shaded(
                    &mut img,
                    cx + body_w + 2 - i / 2,
                    body_cy - 8 - i + 6 + b.tilt,
                    3,
                    4,
                    &b.accent,
                );
                blot_shaded(
                    &mut img,
                    cx + body_w + 2 - i / 2,
                    body_cy - 8 + i - 6 + b.tilt,
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
                    hx,
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
                let dy = if side == -b.flip { -2 * b.perk } else { 0 };
                blot_shaded(
                    &mut img,
                    cx + side * (body_w + 8),
                    body_cy - 4 + dy,
                    5,
                    11,
                    &b.ramp,
                );
            }
            blot_shaded(
                &mut img,
                hx,
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
            blot_shaded(&mut img, hx, head_cy + 4, head_r + 2, head_r, &b.ramp);
            head_features(&mut img, &b, false);
        }
    }

    species_kit(&mut img, &b, false);

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

// ----- overworld fx (P18) -------------------------------------------------

/// 16×16 "spotted!" bubble: a parchment speech bubble with a bold ink
/// `!` and a tail toward the speaker, bottom-left. Sized to read at 1×
/// over a 32px world.
fn alert_bubble() -> RgbaImage {
    let mut img = RgbaImage::new(16, 16);
    let paper = shade(PARCHMENT, 1.08);
    // bubble body, corners pulled in a pixel
    for y in 1..=11u32 {
        let (x0, x1) = if y == 1 || y == 11 { (3, 12) } else { (2, 13) };
        for x in x0..=x1 {
            img.put_pixel(x, y, paper);
        }
    }
    // tail
    for (x0, x1, y) in [(3u32, 6u32, 12u32), (2, 5, 13), (2, 3, 14)] {
        for x in x0..=x1 {
            img.put_pixel(x, y, paper);
        }
    }
    // bold `!`: 2×4 bar, gap, 2×2 dot
    for y in [3u32, 4, 5, 6, 8, 9] {
        img.put_pixel(7, y, INK);
        img.put_pixel(8, y, INK);
    }
    outline(&mut img);
    img
}

/// 24×10 drop shadow: an ink ellipse, alpha ~90 at the center stepping
/// down to nothing at the rim (concentric bands, no blur).
fn drop_shadow() -> RgbaImage {
    let (w, h) = (24u32, 10u32);
    let mut img = RgbaImage::new(w, h);
    let (cx, cy) = (f64::from(w - 1) / 2.0, f64::from(h - 1) / 2.0);
    for y in 0..h {
        for x in 0..w {
            let dx = (f64::from(x) - cx) / (cx + 0.5);
            let dy = (f64::from(y) - cy) / (cy + 0.5);
            let d = dx * dx + dy * dy;
            let alpha = if d > 1.0 {
                0
            } else if d < 0.2 {
                90
            } else if d < 0.45 {
                64
            } else if d < 0.72 {
                38
            } else {
                16
            };
            if alpha > 0 {
                img.put_pixel(x, y, Rgba([INK[0], INK[1], INK[2], alpha]));
            }
        }
    }
    img
}

/// 32×32 grass-rustle burst, frame 0..=2: leaf flecks scattering from
/// foot level and fading. The same seed drives every frame, so each
/// fleck flies a straight line; frame 2 drops half of them (sparse).
fn rustle_frame(frame: u32) -> RgbaImage {
    let mut img = RgbaImage::new(TILE, TILE);
    let base = hex(0x5fa653);
    let greens = [
        shade(base, 0.7),
        base,
        shade(base, 1.15),
        shade(hex(0x6a9a4e), 0.8),
    ];
    let alpha = [255u8, 200, 110][frame as usize];
    let mut put = |x: i32, y: i32, mut c: Rgba<u8>| {
        c[3] = alpha;
        let t = TILE as i32;
        if (0..t).contains(&x) && (0..t).contains(&y) {
            img.put_pixel(x as u32, y as u32, c);
        }
    };
    let mut rng = BattleRng::from_seed(0x0018_5071);
    for i in 0..10u32 {
        // draw every fleck's path on every frame — rng order must match
        let angle = f64::from(rng.below(360)).to_radians();
        let speed = 2.0 + f64::from(rng.below(20)) / 10.0;
        let green = greens[rng.below(4) as usize];
        if frame == 2 && i % 2 == 1 {
            continue;
        }
        let r = 1.5 + speed * 1.4 * f64::from(frame);
        let (s, c) = angle.sin_cos();
        // squash vertically, drift upward as the burst opens
        let x = (15.5 + c * r) as i32;
        let y = (22.0 + s * r * 0.55 - f64::from(frame) * 1.5) as i32;
        put(x, y, green);
        if frame < 2 {
            put(x + 1, y - 1, greens[(i as usize + 1) % 4]);
        }
    }
    img
}

/// 320×180 radial vignette: transparent center, white alpha rising
/// smoothly toward the edges (~110 in the corners). The presenter
/// tints it, so the pixels stay white.
fn vignette() -> RgbaImage {
    let (w, h) = (320u32, 180u32);
    let mut img = RgbaImage::new(w, h);
    let (cx, cy) = (f64::from(w) / 2.0, f64::from(h) / 2.0);
    for y in 0..h {
        for x in 0..w {
            let dx = (f64::from(x) + 0.5 - cx) / cx;
            let dy = (f64::from(y) + 0.5 - cy) / cy;
            let d = (dx * dx + dy * dy).sqrt(); // 0 center, √2 corner
            let t = ((d - 0.35) / (std::f64::consts::SQRT_2 - 0.35)).clamp(0.0, 1.0);
            let smooth = t * t * (3.0 - 2.0 * t);
            let alpha = (smooth * 110.0) as u8;
            if alpha > 0 {
                img.put_pixel(x, y, Rgba([255, 255, 255, alpha]));
            }
        }
    }
    img
}

/// P18 overworld-feel kit: alert bubble, drop shadow, rustle burst,
/// vignette. Returns the frame count for the `sprites` summary line.
pub fn render_overworld_fx(out: &Path) -> Result<usize> {
    std::fs::create_dir_all(out).with_context(|| format!("creating {}", out.display()))?;
    alert_bubble()
        .save(out.join("alert.png"))
        .context("writing alert")?;
    drop_shadow()
        .save(out.join("shadow.png"))
        .context("writing shadow")?;
    for frame in 0..3u32 {
        rustle_frame(frame)
            .save(out.join(format!("rustle.{frame}.png")))
            .with_context(|| format!("writing rustle.{frame}"))?;
    }
    vignette()
        .save(out.join("vignette.png"))
        .context("writing vignette")?;
    Ok(6)
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
        "door_open",
        "window",
        "flowers",
        "fence",
        "lamp",
        "wall_indoor",
    ];
    for kind in kinds {
        // door_open shares door's seed: the wall speckle around the
        // doorway must not re-roll when the arrival flash swaps tiles.
        let seed_kind = if kind == "door_open" { "door" } else { kind };
        let mut rng = BattleRng::from_seed(0x711e ^ (seed_kind.len() as u64 * 7919));
        tile(kind, &mut rng)
            .save(out.join(format!("{kind}.png")))
            .with_context(|| format!("writing {kind}"))?;
    }
    // P19 variants: same painter family, explicit seeds — the
    // renderer hashes tile position to pick one, killing the
    // repetition shimmer of a single repeated texture.
    let variants = [
        ("grass.1", 0x9a55_0001u64),
        ("grass.2", 0x9a55_0002),
        ("path.1", 0x9a47_0001),
    ];
    for (kind, seed) in variants {
        let mut rng = BattleRng::from_seed(seed);
        tile(kind, &mut rng)
            .save(out.join(format!("{kind}.png")))
            .with_context(|| format!("writing {kind}"))?;
    }
    // water frame 2: same seed and painter as `water`, highlights
    // phase-shifted — alternating the two frames reads as shimmer.
    let mut rng = BattleRng::from_seed(0x711e ^ ("water".len() as u64 * 7919));
    let mut second = RgbaImage::new(TILE, TILE);
    water_surface(&mut second, hex(WATER_BASE), &mut rng, 3);
    second
        .save(out.join("water.1.png"))
        .context("writing water.1")?;
    // P19 grass fringes: per-piece seeds so rotations don't read as
    // mirrors of one another. `n` = grass above on screen = image top.
    let mut fringes = 0usize;
    for (kind, waterline) in [("path", false), ("water", true)] {
        for (i, edge) in ["n", "s", "e", "w"].iter().enumerate() {
            let mut rng =
                BattleRng::from_seed(0xf21d_9e00 ^ (i as u64) ^ ((waterline as u64) << 4));
            let lip = fringe_edge(&mut rng, waterline);
            let img = match *edge {
                "n" => lip,
                "s" => imageops::flip_vertical(&lip),
                "e" => imageops::rotate90(&lip),
                _ => imageops::rotate270(&lip),
            };
            img.save(out.join(format!("fringe_{kind}_{edge}.png")))
                .with_context(|| format!("writing fringe_{kind}_{edge}"))?;
            fringes += 1;
        }
        for (i, corner) in ["nw", "ne", "sw", "se"].iter().enumerate() {
            let mut rng =
                BattleRng::from_seed(0xf21d_c000 ^ (i as u64) ^ ((waterline as u64) << 4));
            let nib = fringe_corner(&mut rng, waterline);
            let img = match *corner {
                "nw" => nib,
                "ne" => imageops::flip_horizontal(&nib),
                "sw" => imageops::flip_vertical(&nib),
                _ => imageops::rotate180(&nib),
            };
            img.save(out.join(format!("fringe_{kind}_{corner}.png")))
                .with_context(|| format!("writing fringe_{kind}_{corner}"))?;
            fringes += 1;
        }
    }
    Ok(kinds.len() + 1 + variants.len() + fringes)
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
            motif.catch_rate,
        );
        front
            .save(out.join(format!("{}.front.png", motif.id)))
            .with_context(|| format!("writing {}", motif.id))?;
        let back = creature_back(
            motif.sigil_seed,
            motif.types[0],
            motif.types.get(1).copied(),
            &motif.tags,
            motif.catch_rate,
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

// ----- UI skin (P19) ------------------------------------------------------

/// 64×64 paper-grain overlay: transparent base, faint ink-toned fibre
/// flecks (alpha 10-22) plus a few light flecks (alpha 8-14). Tiled
/// over colored panels — the BackgroundColor still does the work, the
/// grain just kills the flatness.
fn paper_grain() -> RgbaImage {
    let (w, h) = (64u32, 64u32);
    let mut img = RgbaImage::new(w, h);
    let mut rng = BattleRng::from_seed(0x9a9e_0012);
    // dark fibres: short 2-3px strokes in random directions
    for _ in 0..70 {
        let x = i64::from(rng.below(w - 2));
        let y = i64::from(rng.below(h - 2));
        let alpha = u8::try_from(10 + rng.below(13)).expect("alpha 10-22");
        let len = i64::from(2 + rng.below(2));
        let (dx, dy) = match rng.below(4) {
            0 => (1, 0),
            1 => (0, 1),
            2 => (1, 1),
            _ => (1, -1),
        };
        for i in 0..len {
            let (fx, fy) = (x + dx * i, y + dy * i);
            if (0..i64::from(w)).contains(&fx) && (0..i64::from(h)).contains(&fy) {
                img.put_pixel(
                    u32::try_from(fx).expect("in range"),
                    u32::try_from(fy).expect("in range"),
                    Rgba([INK[0], INK[1], INK[2], alpha]),
                );
            }
        }
    }
    // light flecks: single parchment-light points
    let light = shade(PARCHMENT, 1.08);
    for _ in 0..28 {
        let x = rng.below(w);
        let y = rng.below(h);
        let alpha = u8::try_from(8 + rng.below(7)).expect("alpha 8-14");
        img.put_pixel(x, y, Rgba([light[0], light[1], light[2], alpha]));
    }
    img
}

/// 8×8 gilt corner cap, drawn for the TOP-LEFT corner — an L of gilt
/// 2px thick, tips tapered dark, with an ink seam pixel at the outer
/// corner. The presenter flips it for the other three corners.
fn corner_cap() -> RgbaImage {
    let mut img = RgbaImage::new(8, 8);
    let bright = shade(GILT, 1.15);
    let dark = shade(GILT, 0.72);
    for i in 0..8u32 {
        let inner = if i >= 6 { dark } else { GILT };
        img.put_pixel(i, 0, bright); // horizontal arm, outer line
        img.put_pixel(i, 1, inner); // horizontal arm, inner line
        img.put_pixel(0, i, bright); // vertical arm, outer line
        img.put_pixel(1, i, inner); // vertical arm, inner line
    }
    img.put_pixel(0, 0, INK); // the seam stitch
    img
}

/// A 12×12 glyph from a row stencil: `#` ink, `+` gilt, `o` soft ink,
/// `.` transparent. Hand-placed pixels — they have to read at 12px
/// over parchment, so no painter noise here.
fn glyph12(rows: [&str; 12]) -> RgbaImage {
    let mut img = RgbaImage::new(12, 12);
    for (y, row) in rows.iter().enumerate() {
        for (x, ch) in row.chars().enumerate() {
            let c = match ch {
                '#' => INK,
                '+' => GILT,
                'o' => Rgba([INK[0], INK[1], INK[2], 140]),
                _ => continue,
            };
            img.put_pixel(
                u32::try_from(x).expect("12 wide"),
                u32::try_from(y).expect("12 tall"),
                c,
            );
        }
    }
    img
}

/// The four battle-command glyphs: sword slash, bell, tonic flask,
/// open door with motion ticks.
fn command_glyph(name: &str) -> RgbaImage {
    match name {
        "fight" => glyph12([
            "............",
            ".........##.",
            "........##..",
            ".......##...",
            "......##....",
            "...#.##.....",
            "....###.....",
            "...##.#.....",
            "..##........",
            ".+#.........",
            "............",
            "............",
        ]),
        "bell" => glyph12([
            ".....##.....",
            "....#..#....",
            "...#....#...",
            "...#....#...",
            "..#......#..",
            "..#......#..",
            ".#........#.",
            ".##########.",
            ".....++.....",
            "............",
            "............",
            "............",
        ]),
        "tonic" => glyph12([
            "....####....",
            "....#..#....",
            "....#..#....",
            "...#....#...",
            "..#......#..",
            ".#........#.",
            ".#.++++++.#.",
            ".#.++++++.#.",
            ".#.++++++.#.",
            "..########..",
            "............",
            "............",
        ]),
        _ => glyph12([
            "...########.",
            "...#...##.#.",
            ".#.#...##.#.",
            "...#...##.#.",
            "##.#...#+.#.",
            "...#...##.#.",
            ".#.#...##.#.",
            "...#...##.#.",
            "...#...##.#.",
            "...########.",
            "............",
            "............",
        ]),
    }
}

/// P19 UI skin kit: paper grain, gilt corner cap, command icons.
/// Returns the asset count for the `sprites` summary line.
pub fn render_ui(out: &Path) -> Result<usize> {
    std::fs::create_dir_all(out).with_context(|| format!("creating {}", out.display()))?;
    paper_grain()
        .save(out.join("paper.png"))
        .context("writing paper")?;
    corner_cap()
        .save(out.join("corner.png"))
        .context("writing corner")?;
    for name in ["fight", "bell", "tonic", "run"] {
        command_glyph(name)
            .save(out.join(format!("icon_{name}.png")))
            .with_context(|| format!("writing icon_{name}"))?;
    }
    Ok(6)
}
