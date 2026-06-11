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

pub fn render_heroes(out: &Path) -> Result<usize> {
    std::fs::create_dir_all(out).with_context(|| format!("creating {}", out.display()))?;
    let a = legend_b();
    let save = |img: &RgbaImage, name: &str| -> Result<()> {
        img.save(out.join(format!("{name}.png")))
            .with_context(|| format!("writing {name}"))
    };
    // Variant B (teal wayfarer) is the live player set — the user's pick
    // from the P13 variant sheet.
    save(&paint(A_DOWN_0, &a), "player.down.0")?;
    save(&paint(A_DOWN_1, &a), "player.down.1")?;
    save(&paint(A_UP_0, &a), "player.up.0")?;
    save(&paint(A_UP_0, &a), "player.up.1")?;
    let left0 = paint(A_SIDE_0, &a);
    let left1 = paint(A_SIDE_1, &a);
    save(&left0, "player.left.0")?;
    save(&left1, "player.left.1")?;
    let m0: Vec<String> = mirror(A_SIDE_0);
    let m1: Vec<String> = mirror(A_SIDE_1);
    let m0refs: Vec<&str> = m0.iter().map(String::as_str).collect();
    let m1refs: Vec<&str> = m1.iter().map(String::as_str).collect();
    save(&paint(&m0refs, &a), "player.right.0")?;
    save(&paint(&m1refs, &a), "player.right.1")?;

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
    Ok(11)
}
