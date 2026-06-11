//! Hand-authored anime-proportioned characters (P13). 32×44 grids,
//! bottom-anchored (heads overflow the 32px tile, era-style). Three
//! hero designs — the user picks from the variant sheet; variant A
//! ("the Conductor") ships as the default until then.

use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result};
use image::{Rgba, RgbaImage};

const INK: Rgba<u8> = Rgba([26, 24, 34, 255]);

fn hex(rgb: u32) -> Rgba<u8> {
    Rgba([
        u8::try_from((rgb >> 16) & 0xff).expect("byte"),
        u8::try_from((rgb >> 8) & 0xff).expect("byte"),
        u8::try_from(rgb & 0xff).expect("byte"),
        255,
    ])
}

const H: usize = 44;

fn paint(grid: &[&str], legend: &HashMap<char, Rgba<u8>>) -> RgbaImage {
    let mut img = RgbaImage::new(32, H as u32);
    for (y, row) in grid.iter().enumerate().take(H) {
        for (x, ch) in row.chars().enumerate().take(32) {
            if ch == '.' || ch == ' ' {
                continue;
            }
            img.put_pixel(x as u32, y as u32, legend.get(&ch).copied().unwrap_or(INK));
        }
    }
    img
}

/// The Conductor (variant A): gold coat, dark hair, gilt baton-pin.
/// Legend: o ink, h hair, H hair-light, s skin, e eye, w glint,
/// g coat, G coat-shade, c collar/cream, b boots.
fn legend_a() -> HashMap<char, Rgba<u8>> {
    [
        ('o', hex(0x1a1822)),
        ('h', hex(0x3b3047)),
        ('H', hex(0x5a4a6e)),
        ('s', hex(0xe8c39e)),
        ('e', hex(0x2b4a6e)),
        ('w', hex(0xf2e9d8)),
        ('g', hex(0xc6a147)),
        ('G', hex(0x9c7c2e)),
        ('c', hex(0xe8dcc3)),
        ('b', hex(0x42210f)),
    ]
    .into_iter()
    .collect()
}

const A_DOWN_0: &[&str] = &[
    "................................",
    "................................",
    "...........ooooooo..............",
    ".........oohhhhhhhoo............",
    "........ohhhhhhhhhhho...........",
    ".......ohhHhhhhhhHhhho..........",
    ".......ohhhhhhhhhhhhho..........",
    ".......ohhohhhhhhohhho..........",
    ".......ohsssssssssssho..........",
    ".......ohsssssssssssho..........",
    ".......osseewsseewsso...........",
    ".......osseeesseeesso...........",
    "........ssssssssssss............",
    "........ossssoossso.............",
    ".........ossssssso..............",
    "..........osssso................",
    ".......oogggggggoo..............",
    "......oggggggggggggo...........",
    ".....oggggggggggggggo...........",
    ".....ogGgggggggggGggo...........",
    "....ogggggcggggggggggo..........",
    "....osggggcggggggggsso..........",
    "....ossggggggggggosso...........",
    "....oss.ogggggggo.sso..........",
    ".....oo.oggggggGo..oo...........",
    "........oggggggGo...............",
    "........ogGgggggo...............",
    "........oggogggGo...............",
    "........oGgoogggo...............",
    "........oggo.oggo...............",
    "........oggo.oggo...............",
    "........oggo.oggo...............",
    "........obbo.obbo...............",
    "........obbo.obbo...............",
    ".......oobbo.obboo..............",
    ".......obbbo.obbbo..............",
    "........oo.....oo...............",
    "................................",
    "................................",
    "................................",
    "................................",
    "................................",
    "................................",
    "................................",
    "................................",
];

const A_DOWN_1: &[&str] = &[
    "................................",
    "................................",
    "...........ooooooo..............",
    ".........oohhhhhhhoo............",
    "........ohhhhhhhhhhho...........",
    ".......ohhHhhhhhhHhhho..........",
    ".......ohhhhhhhhhhhhho..........",
    ".......ohhohhhhhhohhho..........",
    ".......ohsssssssssssho..........",
    ".......ohsssssssssssho..........",
    ".......osseewsseewsso...........",
    ".......osseeesseeesso...........",
    "........ssssssssssss............",
    "........ossssoossso.............",
    ".........ossssssso..............",
    "..........osssso................",
    ".......oogggggggoo..............",
    "......oggggggggggggo...........",
    ".....oggggggggggggggo...........",
    ".....ogGgggggggggGggo...........",
    "....ogggggcggggggggggo..........",
    "....osggggcggggggggsso..........",
    "....ossggggggggggosso...........",
    "....oss.ogggggggo.sso..........",
    ".....oo.oggggggGo..oo...........",
    "........oggggggGo...............",
    "........ogGgggggo...............",
    "........ogggggggo...............",
    "........ogggGggGo...............",
    ".........oggoggGo...............",
    ".........oggoggo................",
    ".........oggoggGo...............",
    ".........obbobbo................",
    ".........obbobbGo...............",
    ".........obbo.obbo..............",
    "........oobbo..obboo............",
    "........obbbo..obbbo............",
    ".........oo......oo.............",
    "................................",
    "................................",
    "................................",
    "................................",
    "................................",
    "................................",
];

const A_UP_0: &[&str] = &[
    "................................",
    "................................",
    "...........ooooooo..............",
    ".........oohhhhhhhoo............",
    "........ohhhhhhhhhhho...........",
    ".......ohhHhhhhhhHhhho..........",
    ".......ohhhhhhhhhhhhho..........",
    ".......ohhhhhhhhhhhhho..........",
    ".......ohhhhhhhhhhhhho..........",
    ".......ohhhhhhhhhhhhho..........",
    ".......ohhhhhhhhhhhhho..........",
    ".......ohhhhhhhhhhhhho..........",
    "........ohhhhhhhhhho............",
    "........oohhhhhhhhoo............",
    ".........oohhhhhho.............",
    "..........osssso................",
    ".......oogggggggoo..............",
    "......oggggggggggggo...........",
    ".....oggggggggggggggo...........",
    ".....ogGgggggggggGggo...........",
    "....oggggggggggggggggo..........",
    "....osgggggggggggggsso..........",
    "....ossggggggggggosso...........",
    "....oss.ogggggggo.sso..........",
    ".....oo.oggggggGo..oo...........",
    "........oggggggGo...............",
    "........ogGgggggo...............",
    "........oggogggGo...............",
    "........oGgoogggo...............",
    "........oggo.oggo...............",
    "........oggo.oggo...............",
    "........oggo.oggo...............",
    "........obbo.obbo...............",
    "........obbo.obbo...............",
    ".......oobbo.obboo..............",
    ".......obbbo.obbbo..............",
    "........oo.....oo...............",
    "................................",
    "................................",
    "................................",
    "................................",
    "................................",
    "................................",
    "................................",
];

const A_SIDE_0: &[&str] = &[
    "................................",
    "................................",
    "...........ooooooo..............",
    ".........oohhhhhhhoo............",
    "........ohhhhhhhhhhho...........",
    ".......ohhHhhhhhhhhhho..........",
    ".......ohhhhhhhhhhhhho..........",
    ".......ohhhhhhhhhohhho.........",
    ".......ohhsssssssshhho..........",
    ".......ohsssssssssshho..........",
    ".......ossseewsssssho...........",
    ".......ossseeessssso............",
    "........ssssssssssso............",
    "........osssssssso.............",
    ".........osssssso..............",
    "..........osssso................",
    ".......oogggggggoo..............",
    "......oggggggggggGo.............",
    ".....oggggggggggggGo............",
    ".....ogGggggggggggGo............",
    "....oggggggggggggggo...........",
    "....osgggggggggggGso............",
    "....ossgggggggggGso.............",
    ".....oo.ogggggggo..............",
    "........oggggggGo...............",
    "........oggggggGo...............",
    "........ogGgggggo...............",
    "........oggggggGo...............",
    "........ogggoggGo...............",
    "........oggo.ogo...............",
    "........oggo.oggo...............",
    "........oggo.oggo...............",
    "........obbo.obbo...............",
    "........obbo.obbo...............",
    ".......oobboooobboo.............",
    ".......obbbo..obbbo.............",
    "........oo......oo..............",
    "................................",
    "................................",
    "................................",
    "................................",
    "................................",
    "................................",
    "................................",
];

const A_SIDE_1: &[&str] = &[
    "................................",
    "................................",
    "...........ooooooo..............",
    ".........oohhhhhhhoo............",
    "........ohhhhhhhhhhho...........",
    ".......ohhHhhhhhhhhhho..........",
    ".......ohhhhhhhhhhhhho..........",
    ".......ohhhhhhhhhohhho.........",
    ".......ohhsssssssshhho..........",
    ".......ohsssssssssshho..........",
    ".......ossseewsssssho...........",
    ".......ossseeessssso............",
    "........ssssssssssso............",
    "........osssssssso.............",
    ".........osssssso..............",
    "..........osssso................",
    ".......oogggggggoo..............",
    "......oggggggggggGo.............",
    ".....oggggggggggggGo............",
    ".....ogGggggggggggGo............",
    "....oggggggggggggggo...........",
    "....osgggggggggggGso............",
    "....ossgggggggggGso.............",
    ".....oo.ogggggggo..............",
    "........oggggggGo...............",
    "........oggggggGo...............",
    "........oggGggggo...............",
    "........ogggggGgo...............",
    ".........ogggggo...............",
    ".........oggGggo...............",
    "........oggooggGo..............",
    "........oggo.oggGo..............",
    "........obbo..obbo..............",
    "........obbo..obbo..............",
    ".......oobbo...obboo............",
    ".......obbbo...obbbo............",
    "........oo.......oo.............",
    "................................",
    "................................",
    "................................",
    "................................",
    "................................",
    "................................",
    "................................",
];

/// Variant B — "the Wayfarer": teal scarf, auburn hair, satchel.
fn legend_b() -> HashMap<char, Rgba<u8>> {
    [
        ('o', hex(0x1a1822)),
        ('h', hex(0x8a4a2a)),
        ('H', hex(0xb06a3e)),
        ('s', hex(0xe8c39e)),
        ('e', hex(0x3e6e3e)),
        ('w', hex(0xf2e9d8)),
        ('g', hex(0x4a7a9c)),
        ('G', hex(0x35586f)),
        ('c', hex(0x6fb3a8)),
        ('b', hex(0x5d2f1c)),
    ]
    .into_iter()
    .collect()
}

/// Variant C — "the Chorister": violet hooded cape, silver hair.
fn legend_c() -> HashMap<char, Rgba<u8>> {
    [
        ('o', hex(0x1a1822)),
        ('h', hex(0xc9c9d4)),
        ('H', hex(0xe8e8f0)),
        ('s', hex(0xe8c39e)),
        ('e', hex(0x6e2b4a)),
        ('w', hex(0xf2e9d8)),
        ('g', hex(0x7d5a8e)),
        ('G', hex(0x5d4070)),
        ('c', hex(0xc6a147)),
        ('b', hex(0x2e2638)),
    ]
    .into_iter()
    .collect()
}

/// The opposite side-stride (P18 walk v2): the leading leg swaps, so
/// the four-beat cycle reads stride / stand / other-stride / stand.
/// Upper body identical to A_SIDE_0; only the legs differ.
const A_SIDE_2: &[&str] = &[
    "................................",
    "................................",
    "...........ooooooo..............",
    ".........oohhhhhhhoo............",
    "........ohhhhhhhhhhho...........",
    ".......ohhHhhhhhhhhhho..........",
    ".......ohhhhhhhhhhhhho..........",
    ".......ohhhhhhhhhohhho.........",
    ".......ohhsssssssshhho..........",
    ".......ohsssssssssshho..........",
    ".......ossseewsssssho...........",
    ".......ossseeessssso............",
    "........ssssssssssso............",
    "........osssssssso.............",
    ".........osssssso..............",
    "..........osssso................",
    ".......oogggggggoo..............",
    "......oggggggggggGo.............",
    ".....oggggggggggggGo............",
    ".....ogGggggggggggGo............",
    "....oggggggggggggggo...........",
    "....osgggggggggggGso............",
    "....ossgggggggggGso.............",
    ".....oo.ogggggggo..............",
    "........oggggggGo...............",
    "........oggggggGo...............",
    "........ogGgggggo...............",
    "........ogggggGgo...............",
    ".........ogggggo................",
    ".........oggGggo................",
    "........oggooggo................",
    ".......oggGo.oggo...............",
    ".......obbo..obbo...............",
    ".......obbo..obbo...............",
    "......oobbo..obboo..............",
    "......obbbo..obbbo..............",
    ".......oo......oo...............",
    "................................",
    "................................",
    "................................",
    "................................",
    "................................",
    "................................",
    "................................",
];

/// The up-facing stride: A_UP_0's back view over A_DOWN_1's stride
/// legs (the lower body is shared between the two facings).
fn up_stride() -> Vec<String> {
    A_UP_0
        .iter()
        .take(27)
        .chain(A_DOWN_1.iter().skip(27))
        .map(|row| (*row).to_string())
        .collect()
}

fn mirror(grid: &[&str]) -> Vec<String> {
    grid.iter()
        .map(|row| {
            let mut chars: Vec<char> = row.chars().collect();
            while chars.len() < 32 {
                chars.push('.');
            }
            chars.truncate(32);
            chars.reverse();
            chars.into_iter().collect()
        })
        .collect()
}

/// Mirrors about the FIGURE's axis, not the canvas's. The hero grids
/// sit off-center (cols 4..=21), so a plain canvas mirror lurches the
/// body ~6px sideways — fine when a whole direction set is mirrored
/// (left → right), wrong when one mirrored frame joins three
/// unmirrored ones (the down/up alternate strides).
fn mirror_in_place(grid: &[&str]) -> Vec<String> {
    let (mut min, mut max) = (31usize, 0usize);
    for row in grid {
        for (x, ch) in row.chars().enumerate().take(32) {
            if ch != '.' && ch != ' ' {
                min = min.min(x);
                max = max.max(x);
            }
        }
    }
    let axis = min + max; // x → axis - x keeps the bounding box fixed
    grid.iter()
        .map(|row| {
            let chars: Vec<char> = row.chars().collect();
            (0..32)
                .map(|x| {
                    axis.checked_sub(x)
                        .and_then(|src| chars.get(src).copied())
                        .filter(|ch| *ch != ' ')
                        .unwrap_or('.')
                })
                .collect()
        })
        .collect()
}

fn refs(rows: &[String]) -> Vec<&str> {
    rows.iter().map(String::as_str).collect()
}

pub fn render_heroes(out: &Path) -> Result<usize> {
    std::fs::create_dir_all(out).with_context(|| format!("creating {}", out.display()))?;
    let a = legend_b();
    let save = |img: &RgbaImage, name: &str| -> Result<()> {
        img.save(out.join(format!("{name}.png")))
            .with_context(|| format!("writing {name}"))
    };
    // Variant B (teal wayfarer) is the live player set — the user's pick
    // from the P13 variant sheet. Four frames per direction (P18 walk
    // v2): 0 stand, 1 stride, 2 stand, 3 the opposite stride — the
    // renderer alternates strides per tile so the gait reads two-step.
    // The alternate strides mirror in place (the figure axis) so the
    // body holds still while the legs swap.
    let down3 = mirror_in_place(A_DOWN_1);
    let up1 = up_stride();
    let up1_refs = refs(&up1);
    let up3 = {
        let owned: Vec<&str> = refs(&up1);
        mirror_in_place(&owned)
    };
    save(&paint(A_DOWN_0, &a), "player.down.0")?;
    save(&paint(A_DOWN_1, &a), "player.down.1")?;
    save(&paint(A_DOWN_0, &a), "player.down.2")?;
    save(&paint(&refs(&down3), &a), "player.down.3")?;
    save(&paint(A_UP_0, &a), "player.up.0")?;
    save(&paint(&up1_refs, &a), "player.up.1")?;
    save(&paint(A_UP_0, &a), "player.up.2")?;
    save(&paint(&refs(&up3), &a), "player.up.3")?;
    save(&paint(A_SIDE_0, &a), "player.left.0")?;
    save(&paint(A_SIDE_1, &a), "player.left.1")?;
    save(&paint(A_SIDE_0, &a), "player.left.2")?;
    save(&paint(A_SIDE_2, &a), "player.left.3")?;
    for (grid, name) in [
        (A_SIDE_0, "player.right.0"),
        (A_SIDE_1, "player.right.1"),
        (A_SIDE_0, "player.right.2"),
        (A_SIDE_2, "player.right.3"),
    ] {
        let m = mirror(grid);
        save(&paint(&refs(&m), &a), name)?;
    }

    // The variant sheet: A/B/C down-idles, same silhouette, three
    // palettes+details — the user picks next playtest.
    std::fs::create_dir_all(out.join("variants")).with_context(|| "creating variants dir")?;
    for (legend, name) in [
        (legend_a(), "hero_a"),
        (legend_b(), "hero_b"),
        (legend_c(), "hero_c"),
    ] {
        save(&paint(A_DOWN_0, &legend), &format!("variants/{name}"))?;
    }
    Ok(19)
}
