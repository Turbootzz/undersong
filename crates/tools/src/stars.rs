//! Hand-authored pixel art for the stars (P13): the three starter
//! lines and the legendaries get deliberate, designed sprites — not
//! grammar output. Each is a 32×32 character grid upscaled ×3 to the
//! 96×96 battle canvas. Legend per sprite maps chars to colors;
//! `.` is transparent. The generator still draws everyone else, and
//! `assets/custom/` still beats all of this.

use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result};
use image::{Rgba, RgbaImage, imageops};

const INK: Rgba<u8> = Rgba([26, 24, 34, 255]);

fn hex(rgb: u32) -> Rgba<u8> {
    Rgba([
        u8::try_from((rgb >> 16) & 0xff).expect("byte"),
        u8::try_from((rgb >> 8) & 0xff).expect("byte"),
        u8::try_from(rgb & 0xff).expect("byte"),
        255,
    ])
}

struct StarSprite {
    id: &'static str,
    legend: &'static [(char, u32)],
    front: [&'static str; 32],
    /// Back view: drawn rows; when empty, the front is mirrored with
    /// the face rows replaced by the nape rows (good enough for pups).
    back: Option<[&'static str; 32]>,
}

fn paint(grid: &[&str; 32], legend: &HashMap<char, Rgba<u8>>) -> RgbaImage {
    let mut img = RgbaImage::new(32, 32);
    for (y, row) in grid.iter().enumerate() {
        for (x, ch) in row.chars().enumerate().take(32) {
            if ch == '.' || ch == ' ' {
                continue;
            }
            let color = legend.get(&ch).copied().unwrap_or(INK);
            img.put_pixel(x as u32, y as u32, color);
        }
    }
    imageops::resize(&img, 96, 96, imageops::FilterType::Nearest)
}

/// A derived back when none is authored: erase the face block (rows
/// carrying eye/mouth chars) by re-coloring those pixels to the body
/// char that dominates their row.
fn derive_back(front: &[&str; 32], legend: &HashMap<char, Rgba<u8>>) -> RgbaImage {
    let face_chars = ['e', 'w', 'm'];
    let mut rows: Vec<String> = Vec::with_capacity(32);
    for row in front.iter() {
        let dominant = row
            .chars()
            .filter(|c| *c != '.' && *c != ' ' && !face_chars.contains(c))
            .fold(HashMap::new(), |mut acc, c| {
                *acc.entry(c).or_insert(0u32) += 1;
                acc
            })
            .into_iter()
            .max_by_key(|(_, n)| *n)
            .map(|(c, _)| c)
            .unwrap_or('.');
        rows.push(
            row.chars()
                .map(|c| if face_chars.contains(&c) { dominant } else { c })
                .collect(),
        );
    }
    let grid_refs: Vec<&str> = rows.iter().map(String::as_str).collect();
    let arr: [&str; 32] = grid_refs[..32].try_into().expect("32 rows");
    paint(&arr, legend)
}

#[expect(clippy::too_many_lines, reason = "fourteen sprites of art data")]
fn stars() -> Vec<StarSprite> {
    vec![
        // ---- the ember line: fox kit → brass wolf → maned maestro ----
        StarSprite {
            id: "fanfyre",
            legend: &[
                ('o', 0x1a1822), // outline
                ('r', 0xc4593a), // coat
                ('d', 0x9c4129), // shade
                ('l', 0xe07a50), // light
                ('c', 0xe8dcc3), // cream belly/muzzle
                ('f', 0xe9a13b), // flame
                ('y', 0xf2d06b), // flame core
                ('e', 0x1a1822), // eye
                ('w', 0xf2e9d8), // eye glint
            ],
            front: [
                "................................",
                "................................",
                "................................",
                "......oo............oo.........",
                ".....ordo..........ordo........",
                ".....orrdo........orrdo........",
                "....orrrrdoooooooorrrrdo........",
                "....orlrrrrrrrrrrrrrlrdo........",
                "....orrrrrrrrrrrrrrrrrdo........",
                "....orrewwrrrrrrrewwrrdo........",
                "....orreeerrrrrrreeerrdo........",
                "....orrrrrrccccrrrrrrrdo........",
                ".....orrrrcccccccrrrrdo.........",
                ".....orrrrrcccccrrrrrdo.........",
                "......oorrrrrrrrrrrdoo..........",
                "........orrrrrrrrrdo............",
                ".......orrrrrrrrrrrdo...........",
                "......orrrcccccccrrrdo......of..",
                ".....orrrcccccccccrrrdo....ofyo.",
                ".....orrrcccccccccrrrdo...ofyfo.",
                ".....orrrcccccccccrrrddooofyfo..",
                ".....orrrrcccccccrrrrdddffyfo...",
                "......orrrrcccccrrrrrddofyfo....",
                ".......orrrrrrrrrrrrddoofo......",
                "........oorrrrrrrrddoo..........",
                ".........orro..orrdo............",
                ".........orro..orrdo............",
                ".........orro..orrdo............",
                ".........oddo..oddo.............",
                ".........oo......oo.............",
                "................................",
                "................................",
            ],
            back: None,
        },
        StarSprite {
            id: "embaritone",
            legend: &[
                ('o', 0x1a1822),
                ('r', 0xb24f33),
                ('d', 0x8a3a24),
                ('l', 0xd9744c),
                ('c', 0xe8dcc3),
                ('b', 0xc6a147), // brass ruff
                ('g', 0x9c7c2e), // brass shade
                ('f', 0xe9a13b),
                ('y', 0xf2d06b),
                ('e', 0x1a1822),
                ('w', 0xf2e9d8),
            ],
            front: [
                "................................",
                "................................",
                "......oo..............oo........",
                ".....orro............orro.......",
                ".....orrro..........orrro.......",
                "....orrrrdoooooooooorrrrdo......",
                "....orlrrrrrrrrrrrrrrrlrdo......",
                "....orrrrrrrrrrrrrrrrrrrdo......",
                "....orrewwrrrrrrrrrewwrrdo......",
                "....orreeerrrrrrrrreeerrdo......",
                "....orrrrrrrcccccrrrrrrrdo......",
                ".....orrrrrcccccccrrrrrdo.......",
                "......orrrrrcccccrrrrrdo........",
                ".....obbbbbbbbbbbbbbbbggo.......",
                "....obbgbbbbbbbbbbbbbgbggo......",
                "....obbbbbbbbbbbbbbbbbbggo......",
                ".....obbgggggggggggggggo........",
                "......orrrrrrrrrrrrrrdo.....of..",
                ".....orrrcccccccccrrrrdo...ofyo.",
                "....orrrcccccccccccrrrdo..ofyfo.",
                "....orrrcccccccccccrrrddoofyfo..",
                "....orrrrcccccccccrrrrddffyfo...",
                ".....orrrrcccccccrrrrdofyfoo....",
                "......orrrrrrrrrrrrrdoofo.......",
                ".......oorrrrrrrrrddoo..........",
                "........orro....orrdo...........",
                "........orro....orrdo...........",
                "........orro....orrdo...........",
                "........oddoo...oddoo...........",
                "........oo.......oo.............",
                "................................",
                "................................",
            ],
            back: None,
        },
        StarSprite {
            id: "maestroar",
            legend: &[
                ('o', 0x1a1822),
                ('r', 0xa84a30),
                ('d', 0x7e3520),
                ('l', 0xd06e46),
                ('c', 0xe8dcc3),
                ('m', 0x5d2f1c), // mane
                ('n', 0x42210f), // mane shade
                ('b', 0xc6a147),
                ('f', 0xe9a13b),
                ('y', 0xf2d06b),
                ('e', 0x1a1822),
                ('w', 0xf2e9d8),
            ],
            front: [
                "..............ofyo..............",
                ".............ofyfyo.............",
                "......oo....ofyfyfo....oo.......",
                ".....orro..oofffffoo..orro......",
                ".....orrromMmmmmmmmmorrrro......",
                "....ommmmmmmmmmmmmmmmmmmmo......",
                "....ommnmmmmmmmmmmmmmnmmno......",
                "...ommmrrrrrrrrrrrrrrrmmmno.....",
                "...ommrrlrrrrrrrrrrrlrrmmno.....",
                "...ommrrewwrrrrrrrewwrrmmno.....",
                "...ommrreeerrrrrrreeerrmmno.....",
                "...ommrrrrrrcccccrrrrrrmmno.....",
                "...ommmrrrrcccccccrrrrmmmno.....",
                "...ommmmrrrrcccccrrrrmmmmno.....",
                "....ommmmmmmmmmmmmmmmmmmno......",
                ".....obbbbbbbbbbbbbbbbbo........",
                "....obbbbbbbbbbbbbbbbbbbo.......",
                ".....orrrrrrrrrrrrrrrrdo........",
                "....orrrcccccccccccrrrrdo.......",
                "...orrrccccccccccccccrrdo..of...",
                "...orrrccccccccccccccrrddoofyo..",
                "...orrrrccccccccccccrrrdddfyfo..",
                "....orrrrccccccccccrrrrdffyfo...",
                ".....orrrrrrrrrrrrrrrddfyfoo....",
                "......oorrrrrrrrrrrddoofoo......",
                ".......orrro....orrrdo..........",
                ".......orrro....orrrdo..........",
                ".......orrro....orrrdo..........",
                ".......odddoo...odddoo..........",
                ".......oo........oo.............",
                "................................",
                "................................",
            ],
            back: None,
        },
        // ---- the tide line: ripple pup → otter → orca ----
        StarSprite {
            id: "rippeggio",
            legend: &[
                ('o', 0x1a1822),
                ('b', 0x4a7a9c),
                ('d', 0x35586f),
                ('l', 0x6f9fc0),
                ('c', 0xe8dcc3),
                ('a', 0x8fb3c7), // ripple collar
                ('e', 0x1a1822),
                ('w', 0xf2e9d8),
            ],
            front: [
                "................................",
                "................................",
                "................................",
                "................................",
                "..........oobbbboo..............",
                ".........obbbbbbbbo.............",
                "........obblbbbbbbdo............",
                ".......obbbbbbbbbbbdo...........",
                ".......obbewwbbbewwbo...........",
                ".......obbeeebbbeeebo...........",
                ".......obbbbbcccbbbdo...........",
                ".......obbbbcccccbbdo...........",
                "........obbbcccccbdo............",
                ".......oaaaaaaaaaaaao...........",
                "......oaalaaaaaaaaalao..........",
                ".......oaaaaaaaaaaaao...........",
                "........obbbbbbbbbdo............",
                ".......obbbcccccbbbdo...........",
                "......obbbcccccccbbbdo..........",
                "......obbbcccccccbbbdo..........",
                "......obbbcccccccbbbdoo.........",
                "......obbbbcccccbbbddobbo.......",
                ".......obbbbbbbbbbbddobbdo......",
                "........obbbbbbbbbdobbbbdo......",
                ".........oobbbbbdoobbbddo.......",
                "..........obbo.obbooddo.........",
                ".........obbo...obbo............",
                ".........obo.....obo............",
                ".........oo.......oo............",
                "................................",
                "................................",
                "................................",
            ],
            back: None,
        },
        StarSprite {
            id: "tidalegro",
            legend: &[
                ('o', 0x1a1822),
                ('b', 0x3f6d8e),
                ('d', 0x2c4e66),
                ('l', 0x649ab8),
                ('c', 0xe8dcc3),
                ('a', 0x8fb3c7),
                ('e', 0x1a1822),
                ('w', 0xf2e9d8),
            ],
            front: [
                "................................",
                "................................",
                "........oao..........oao........",
                ".......oaao..........oaao.......",
                "........obboooooooooobbo........",
                ".......obbbbbbbbbbbbbbbdo.......",
                "......obblbbbbbbbbbbbbbdo.......",
                "......obbbbbbbbbbbbbbbbdo.......",
                "......obbewwbbbbbbbewwbdo.......",
                "......obbeeebbbbbbbeeebdo.......",
                "......obbbbbbcccccbbbbbdo.......",
                ".......obbbbcccccccbbbdo........",
                "........obbbbcccccbbbdo.........",
                ".........obbbbbbbbbdo...........",
                "........obbbbbbbbbbbdo..........",
                ".......obbbcccccccbbbdo.........",
                "......obbbcccccccccbbbdo........",
                "......obbbcccccccccbbbdo........",
                "......obbbcccccccccbbbdo........",
                "......obbbbcccccccbbbddo........",
                ".......obbbbcccccbbbbdo.........",
                "........obbbbbbbbbbbdoo.........",
                ".........obbbbbbbbbdoabo........",
                "..........obbbbbbdooaabdo.......",
                "...........obbbbdoaaabddo.......",
                "...........obbodooaabdo.........",
                "..........obbo....oddo..........",
                ".........obbo...................",
                ".........obdo...................",
                "..........oo....................",
                "................................",
                "................................",
            ],
            back: None,
        },
        StarSprite {
            id: "cellorca",
            legend: &[
                ('o', 0x1a1822),
                ('k', 0x27384a), // orca back
                ('d', 0x1b2836),
                ('l', 0x41566b),
                ('c', 0xe8dcc3),
                ('b', 0x4a7a9c), // fin curves
                ('e', 0x1a1822),
                ('w', 0xf2e9d8),
                ('g', 0xc6a147), // cello f-hole gilt
            ],
            front: [
                "................................",
                "..............obo...............",
                ".............obbdo..............",
                "............obbddo..............",
                "...........obbdo................",
                "......ooookkbkdooooo............",
                "....ookkkkkkkkkkkkkkoo..........",
                "...okkklkkkkkkkkkkkkkdo.........",
                "..okkkkkkkkkkkkkkkkkkkdo........",
                "..okkewwkkkkkkkkkkewwkdo........",
                "..okkeeekkkkkkkkkkeeekdo........",
                "..okkkkkkkcccccckkkkkkdo........",
                "..okkkkkcccccccccckkkkdo........",
                "...okkkkcccccccccckkkdo.........",
                "....okkkkccccccccckkkkdo........",
                "...okkkkkkccccccckkkkkkdo.......",
                "..okkkgkkkkkkkkkkkkkgkkdo.......",
                "..okkkggkkkkkkkkkkkggkkdo.......",
                "..okkkkgkkkkkkkkkkkkgkkdo.......",
                "..okkkkkkkkkkkkkkkkkkkddo.......",
                "...okkkkkkkkkkkkkkkkkddo........",
                "....okkkkkkkkkkkkkkdddoobbo.....",
                ".....ookkkkkkkkkkddoo..obbdo....",
                ".......ookkkkkkddoo...obbbdo....",
                ".........okkkkdoooooobbbbdo.....",
                ".........okkkkbbbbbbbbbddo......",
                "..........okkkbbbbbbbddoo.......",
                "...........ookkbbddddoo.........",
                ".............oodoooo............",
                "................................",
                "................................",
                "................................",
            ],
            back: None,
        },
        // ---- the bloom line: fawn → vine stag → lyre stag ----
        StarSprite {
            id: "solfawn",
            legend: &[
                ('o', 0x1a1822),
                ('t', 0x9a8a6f), // fawn coat
                ('d', 0x73654d),
                ('l', 0xbcab8b),
                ('c', 0xe8dcc3),
                ('g', 0x6a9a4e), // leaf
                ('h', 0x4e7539), // leaf shade
                ('e', 0x1a1822),
                ('w', 0xf2e9d8),
            ],
            front: [
                "................................",
                "................................",
                "..........og.......go...........",
                ".........oggo.....oggo..........",
                ".........ohgo.....ogho.........",
                "..........ohoooooooho...........",
                ".........otttttttttttto.........",
                "........otltttttttttttdo........",
                "........otttttttttttttdo........",
                "........ottewwtttewwttdo........",
                "........otteeetteeeettdo........",
                "........otttttcccttttdo.........",
                ".........otttcccccttdo..........",
                "..........otttcccttdo...........",
                "...........ottttttdo............",
                "..........ottttttttdo...........",
                ".........otttccccttttdo.........",
                "........otttcccccctttdo.........",
                "........ottccccccccttdo.........",
                "........ottccclccccttdo.........",
                "........otttccccccttddo.........",
                ".........ottttttttttdo..........",
                "..........ottttttttdo...........",
                "..........otto..ottdo...........",
                "..........otto..ottdo...........",
                "..........otto..ottdo...........",
                "..........oddo..oddo............",
                "..........oo......oo............",
                "................................",
                "................................",
                "................................",
                "................................",
            ],
            back: None,
        },
        StarSprite {
            id: "vinebrato",
            legend: &[
                ('o', 0x1a1822),
                ('t', 0x8d7d62),
                ('d', 0x675a44),
                ('l', 0xb0a081),
                ('c', 0xe8dcc3),
                ('g', 0x6a9a4e),
                ('h', 0x4e7539),
                ('p', 0xb08ec4), // bloom
                ('e', 0x1a1822),
                ('w', 0xf2e9d8),
            ],
            front: [
                "................................",
                ".....og..go........og..go.......",
                "....oggogggo......oggogggo......",
                "....ohgggho........ohgggho......",
                ".....ohgho..........ohgho.......",
                "......ohgo..........ogho.......",
                "......ohhoooooooooooohho........",
                ".....otttttttttttttttttto.......",
                "....otlttttttttttttttttdo.......",
                "....ottttttttttttttttttdo......",
                "....ottewwttttttttewwttdo.......",
                "....otteeettttttteeeettdo.......",
                "....ottttttcccccttttttdo........",
                ".....otttttcccccctttttdo........",
                "......ottttcccccttttdo.........",
                ".......otttttttttttdo...........",
                "......otttttttttttttdo.........",
                ".....otttccccccccttttdo.........",
                "....otttcccccccccctttdo........",
                "....ottccccpcccccccttdo........",
                "....ottccccccccccccttdo.........",
                "....otttccccccccccttddo........",
                ".....ottttccccccttttdo.........",
                "......ottttttttttttdo..........",
                ".......ottttttttttdo............",
                ".......otto....ottdo............",
                ".......otto....ottdo............",
                ".......otto....ottdo............",
                ".......oddo....oddo.............",
                ".......oo........oo.............",
                "................................",
                "................................",
            ],
            back: None,
        },
        StarSprite {
            id: "sonatler",
            legend: &[
                ('o', 0x1a1822),
                ('t', 0x80714f),
                ('d', 0x5c503a),
                ('l', 0xa6946f),
                ('c', 0xe8dcc3),
                ('g', 0x5d8a42),
                ('h', 0x42662e),
                ('y', 0xc6a147), // lyre-gilt antler tips
                ('e', 0x1a1822),
                ('w', 0xf2e9d8),
            ],
            front: [
                "....oy..yo........oy..yo........",
                "...oyyoyyyo......oyyoyyyo.......",
                "...ogygygo........ogygygo.......",
                "....ohgho..........ohgho........",
                "....ohgo............ogho.......",
                ".....ohgo..........ogho.........",
                ".....ohho..........ohho.........",
                "......ohhoooooooooohho..........",
                "....ottttttttttttttttttoo.......",
                "...otlttttttttttttttttttdo......",
                "...ottttttttttttttttttttdo.....",
                "...otttewwttttttttewwtttdo.....",
                "...ottteeetttttttteeetttdo.....",
                "...otttttttccccccttttttdo......",
                "....ottttttcccccctttttdo.......",
                ".....ottttttccccttttdo.........",
                "......otttttttttttttdo..........",
                ".....otttttttttttttttdo........",
                "....otttcccccccccctttdo........",
                "...otttccccccccccccttdo........",
                "...ottcccccccccccccctdo........",
                "...otttccccccccccccttdo........",
                "....ottttcccccccctttddo........",
                ".....otttttcccctttttdo.........",
                "......ottttttttttttdo...........",
                ".......otto.....ottdo...........",
                ".......otto.....ottdo...........",
                ".......otto.....ottdo...........",
                ".......oddoo....oddoo...........",
                ".......oo.........oo............",
                "................................",
                "................................",
            ],
            back: None,
        },
        // ---- the legendaries ----
        StarSprite {
            id: "primavoce",
            legend: &[
                ('o', 0x1a1822),
                ('v', 0xb08ec4), // body violet
                ('d', 0x83689a),
                ('l', 0xd4bce4),
                ('y', 0xc6a147),
                ('s', 0xf2d06b), // halo
                ('c', 0xe8dcc3),
                ('e', 0x1a1822),
                ('w', 0xf2e9d8),
            ],
            front: [
                ".........osso....osso...........",
                "........os..s....s..so..........",
                ".............ssss...............",
                "...........osssssso............",
                "..........osvvvvvvso............",
                ".........osvvlvvvvvso...........",
                ".........ovvvvvvvvvvo..........",
                ".........ovvewwvewwvvo..........",
                ".........ovveeevveeevo..........",
                ".........ovvvvcccvvvvo..........",
                "..........ovvcccccvvo...........",
                "...........ovvcccvvo...........",
                "....oy......ovvvvvo......yo.....",
                "...oyyo....ovvvvvvvo....oyyo....",
                "...oyvyoooovvvvvvvvvooooyvyo....",
                "....oyvvvvvvvvvvvvvvvvvvvyo.....",
                ".....ovvvvvvlvvvvvvlvvvvvo......",
                "......ovvvvvvvvvvvvvvvvvo.......",
                ".....ovvvclvvvvvvvvclvvvvo......",
                "....ovvvvccvvvvvvvvccvvvvvo.....",
                "....ovvvvvvvvvvvvvvvvvvvvdo.....",
                "....ovvvvvvvvvvvvvvvvvvvddo.....",
                ".....ovvvvvvvvvvvvvvvvvddo......",
                "......ovvvvvvvvvvvvvvvddo.......",
                ".......ovvvvvvvvvvvvvdo........",
                "........ovvvvdovvvvddo..........",
                ".........ovvdo.ovvdo............",
                "..........ovo...ovo............",
                "...........oo....oo.............",
                "................................",
                "................................",
                "................................",
            ],
            back: None,
        },
        StarSprite {
            id: "cantavella",
            legend: &[
                ('o', 0x1a1822),
                ('b', 0x8fb3c7),
                ('d', 0x65899d),
                ('l', 0xbcd9e8),
                ('c', 0xe8dcc3),
                ('y', 0xc6a147),
                ('e', 0x1a1822),
                ('w', 0xf2e9d8),
            ],
            front: [
                "................................",
                "..ol..........................ol",
                "..olbo......................oblo",
                "...olbbo..................obblo.",
                "....olbbbo..............obbblo..",
                ".....olbbbboo........oobbbblo...",
                "......oolbbbbboooooobbbbbloo...",
                ".........obbbbbbbbbbbbbboo......",
                "..........obbbbbbbbbbbo........",
                "..........obblbbbbbbbbdo........",
                ".........obbbbbbbbbbbbbdo.......",
                ".........obbewwbbbbewwbdo.......",
                ".........obbeeebbbbeeebdo.......",
                "..........obbbycccybbbdo........",
                "...........obbcccccbbdo.........",
                "............obbcccbbdo.........",
                ".............obbbbbdo...........",
                "...........obbbbbbbbdo..........",
                "..........obbbcccccbbdo.........",
                ".........obbbcccccccbbdo........",
                ".........obbcccccccccbdo........",
                ".........obbbcccccccbbdo........",
                "..........obbbcccccbbdoo........",
                "...........obbbbbbbbdoyo........",
                "............obbbbbbdoyyo........",
                ".............obbbbdoyyo........",
                "..............obbdoyyo..........",
                "...............odooyo...........",
                "................oo..............",
                "................................",
                "................................",
                "................................",
            ],
            back: None,
        },
        StarSprite {
            id: "taciturn",
            legend: &[
                ('o', 0x1a1822),
                ('s', 0x8a7f6d), // stone
                ('d', 0x665d4e),
                ('l', 0xaaa08c),
                ('v', 0x5d5470), // hood shadow
                ('n', 0x42210f),
                ('g', 0xc6a147),
            ],
            front: [
                "................................",
                "................................",
                "...........ooooooo..............",
                ".........oossssssoo............",
                "........ossssssssssso...........",
                ".......osslsssssssssdo..........",
                ".......ossssssssssssdo..........",
                "......ossssvvvvvvssssdo.........",
                "......osssvvvvvvvvsssdo.........",
                "......ossvvvvvvvvvvssdo.........",
                "......ossvvvogovvvvssdo.........",
                "......ossvvvvvvvvvvssdo.........",
                "......osssvvvvvvvvsssdo.........",
                "......ossssvvvvvvssssdo.........",
                "......osssssssssssssddo.........",
                "......osssssssssssssddo.........",
                ".....osssslssssssssssdo.........",
                ".....ossssssssssssssssdo........",
                ".....osssssssssssssssdo.........",
                "....ossssssgggggssssssdo........",
                "....osssssgsssssgssssdo.........",
                "....ossssssssssssssssddo........",
                "....ossssssssssssssssddo........",
                "....ossssssssssssssssddo........",
                "....ossssssssssssssssddo........",
                "....osssssssssssssssdddo.......",
                "....ossssssssssssssssddo........",
                "...ossssssssssssssssssddo.......",
                "...oddddddddddddddddddddo......",
                "....oooooooooooooooooooo........",
                "................................",
                "................................",
            ],
            back: None,
        },
        StarSprite {
            id: "intervallia",
            legend: &[
                ('o', 0x1a1822),
                ('b', 0x4a7a9c),
                ('d', 0x35586f),
                ('l', 0x7fb0d0),
                ('c', 0xe8dcc3),
                ('y', 0xc6a147),
                ('e', 0x1a1822),
                ('w', 0xf2e9d8),
            ],
            front: [
                "................................",
                "...............oboo.............",
                "..............obbbdo............",
                ".............obbddo............",
                "............obbdo...............",
                "...........obbdo......oo........",
                "....oooobbbbbbdo.....obbo......",
                "..oobbbbbbbbbbbboo...obbbo......",
                ".obbblbbbbbbbbbbbbo..obbbdo.....",
                ".obbbbbbbbbbbbbbbbdoobbbbdo.....",
                ".obbewwbbbbbbbbbbbbdobbbdo......",
                ".obbeeebbbbbbbbbbbbbdbbdo......",
                ".obbbbbbycccybbbbbbbbbbo........",
                ".obbbbbcccccccbbbbbbbbdo........",
                "..obbbbcccccccbbbbbbbdo.........",
                "...obbbbcccccbbbbbbbbo.........",
                "....obbbbbbbbbbbbbbbbdo.........",
                "...obbbbbbbbbbbbbbbbbbdo........",
                "..obbbcccccccccccbbbbbbdo.......",
                ".obbbcccccccccccccbbbbbdo.......",
                ".obbcccccccccccccccbbbddo.......",
                ".obbbcccccccccccccbbbbdo........",
                "..obbbbccccccccccbbbddo........",
                "...oobbbbbbbbbbbbbddoo..........",
                ".....oobbbbbbbbbddoo............",
                ".......obbbbbbddoo..............",
                ".........obbbdo.................",
                "..........obbbdoooo.............",
                "...........obbbbbbdo............",
                "............oodddoo.............",
                "................................",
                "................................",
            ],
            back: None,
        },
        StarSprite {
            id: "vinterstem",
            legend: &[
                ('o', 0x1a1822),
                ('f', 0xa9c7d4), // frost body
                ('d', 0x7e98a6),
                ('l', 0xd6e8f0),
                ('v', 0xb08ec4), // aurora scarf
                ('p', 0x8e6aa6),
                ('c', 0xe8dcc3),
                ('y', 0xc6a147),
                ('e', 0x1a1822),
                ('w', 0xf2e9d8),
            ],
            front: [
                "..........ol..ol..ol............",
                ".........olfo.olfo.olfo.........",
                "..........off..off..off.........",
                "...........offoffooffo.........",
                "...........offfffffffo.........",
                "..........offfffffffffo.........",
                ".........offlfffffffffdo........",
                ".........offffffffffffdo........",
                ".........offewwffffewwdo........",
                ".........offeeeffffeeedo........",
                ".........offfffcccffffdo........",
                "..........offfcccccffdo.........",
                "...........offfcccffdo.........",
                "..........ovvvvvvvvvvvo.........",
                ".........ovpvvvvvvvvvpvo........",
                "........ovvvpvvvvvvvpvvvo.......",
                ".........ovvvvvvvvvvvvo........",
                "..........offfffffffffo........",
                ".........offflffffffffdo........",
                ".........offffffffffffdo........",
                ".........offffffffffffdo........",
                ".........offfffffffffddo........",
                ".........offfffffffffddo........",
                "..........offfffffffddo........",
                "..........offfffffffddo.........",
                "...........offfffffddo..........",
                "...........offffffddo..........",
                "............offffddo............",
                ".............offddo.............",
                "..............oddo..............",
                "...............oo...............",
                "................................",
            ],
            back: None,
        },
    ]
}

/// Renders every star over the generator's output (call after the
/// generator pass; the override directory still beats both).
pub fn render_stars(out_root: &Path) -> Result<usize> {
    let mut count = 0;
    for star in stars() {
        let legend: HashMap<char, Rgba<u8>> =
            star.legend.iter().map(|(c, rgb)| (*c, hex(*rgb))).collect();
        // Stars live in whichever region owns the id; both dirs are
        // checked by the caller — write into any that has the file.
        for region in ["cantorel", "skalden"] {
            let dir = out_root.join(region);
            let probe = dir.join(format!("{}.front.png", star.id));
            if !probe.exists() {
                continue;
            }
            let front = paint(&star.front, &legend);
            front
                .save(&probe)
                .with_context(|| format!("writing {} front", star.id))?;
            let back = match &star.back {
                Some(grid) => paint(grid, &legend),
                None => derive_back(&star.front, &legend),
            };
            back.save(dir.join(format!("{}.back.png", star.id)))
                .with_context(|| format!("writing {} back", star.id))?;
            let icon = imageops::resize(&front, 16, 16, imageops::FilterType::Nearest);
            icon.save(dir.join(format!("{}.icon.png", star.id)))
                .with_context(|| format!("writing {} icon", star.id))?;
            count += 1;
        }
    }
    Ok(count)
}
