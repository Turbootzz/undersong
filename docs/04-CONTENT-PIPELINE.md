# 04 — CONTENT PIPELINE

> The engine is a music player; regions are albums. This doc defines the album format.

## 1. Region pack layout

```
content/regions/cantorel/
├── region.ron            # id, name_key, dex (motif id list), starter trio, map graph entry
├── motifs/*.ron          # one file per species
├── moves/ moves.ron      # region-new moves (core moves live in content/core)
├── trainers/*.ron        # trainer parties + AI tier + payout class
├── maps/<map_id>/
│   ├── map.ron           # compiled runtime map (from importmap or hand-written)
│   ├── encounters.ron    # slot tables (day/night/surf/fish)
│   └── scripts/*.script.ron
├── dialogue/strings.ron  # all region text, keyed (en first; translations later)
└── audio/                # authored stems; generated cries land in assets/
```

`content/core/` holds cross-region law: typechart, natures, items, core moves,
UI strings. A region pack may add, never modify, core content.

## 2. Schema examples (canonical RON shapes)

### Species (`motifs/fanfyre.ron`)

```ron
Species(
    id: "fanfyre", name_key: "motif.fanfyre", types: [Ember],
    base_stats: (hp: 58, atk: 64, def: 50, spa: 80, spd: 58, spe: 80),
    abilities: ["crescendo_ember"], hidden_ability: Some("amplify"),
    growth_curve: MediumSlow, catch_rate: 45, base_exp_yield: 142,
    ev_yield: {spa: 2},
    learnset: [(1,"tackle"),(1,"ember_note"),(7,"dampen"),(13,"gale_riff"),
               (19,"flare_brass"),(28,"crescendo"),(36,"resonate")],
    tm_set: ["tm01","tm05","tm11"],
    evolution: Some((method: Level(34), target: "maestroar")),
    cry_seed: 0xFA9F_1E22, sigil_seed: 0xFA9F_1E22,
    dex: (height_m: 0.9, weight_kg: 19.5, entry_key: "dex.fanfyre"),
    tags: ["performer.light", "habitat.urban"],
)
```

### Trainer (`trainers/maestro_mirelle.ron`)

```ron
Trainer(
    id: "maestro_mirelle", class: Maestro, name_key: "npc.mirelle",
    ai_tier: 3, payout_class: Maestro, double_battle: false,
    party: [
        (species: "solfawn",  level: 14, moves: Some(["leaf_pick","dampen","quick_step"])),
        (species: "vinebrato",level: 17, ivs: Some((hp:31,atk:20,def:25,spa:31,spd:25,spe:31)),
         moves: Some(["root_chord","leaf_pick","frost_lull","crescendo"]),
         held_item: Some("oran_chime")),
    ],
    defeat_flag: "hall.2.cleared", reward: (money_mult: 1.0, items: [("tm05", 1)]),
    intro_key: "battle.mirelle.intro", defeat_key: "battle.mirelle.defeat",
)
```

### Encounters (`maps/route_02/encounters.ron`)

```ron
Encounters(
    patch_rate: 0.12,
    land_day:  [("pipling",2,4,20),("solfawn",3,4,20),("burrbass",2,5,10), /* …12 slots, weights 20,20,10,10,10,10,5,5,4,4,1,1 */],
    land_night: [ /* … */ ], surf: None, fishing: None,
)
```

### Map triggers (in `map.ron`)

```ron
triggers: [
    (at: (12, 7), kind: Warp(map: "prelude_town", to: (4, 18), facing: Down)),
    (at: (20, 3), kind: Script("scripts/anchor_door.script.ron"), once_flag: Some("story.act2.found_anchor")),
],
npcs: [
    (id: "fisher_old", at: (8, 14), facing: Left, sprite: "npc.fisher",
     interact: Script("scripts/fisher.script.ron"),
     trainer: None, sight_range: 0),
]
```

### Dialogue script

```ron
[
    Say(who: "fisher_old", key: "quietcoast.fisher.1"),
    Say(who: "fisher_old", key: "quietcoast.fisher.2"),
    If(flag: "story.act2.read_roster",
       then: [Say(who: "fisher_old", key: "quietcoast.fisher.recontext")],
       else: []),
    SetFlag("met.fisher_old"), End,
]
```

## 3. The validator (`tools validate`) — rule list

Hard failures (CI red):
1. Every referenced id resolves (moves, species, items, maps, scripts, flags read
   are flags written *somewhere*, string keys exist in strings.ron).
2. Every warp targets an existing map + in-bounds, non-solid tile; warp graph is
   fully connected from the region entry map.
3. Encounter slot lists: exactly 12 slots, weights sum to 100, level ranges ≥ 1.
4. Learnsets: levels ascending; every species has a damaging move by level 5;
   evolution targets exist and are acyclic.
5. Trainer parties: 1–6 members, legal moves for species+level, EV sums ≤ 510.
6. Type chart: 12×12 complete, no entry outside {0, ½, 1, 2}.
7. Story reachability: every `hall.N.cleared` flag reachable in flag-graph order;
   ending gates (§ doc 01) reachable.
8. Strings: no orphan keys (warn), no missing keys (fail), no key used with wrong
   arg count.

Warnings (report, don't fail): species unused by any encounter/trainer; moves no
species learns; BST outside its declared band; dead flags.

## 4. Batch content generation (how 120 species actually happen)

Species are produced in **batches of 10–15** through this loop:

1. **Slot plan** — `region.ron` declares dex slots by archetype:
   `route_critter(280–380 BST), early_bird, regional_rodent, mid(400–460),
   ace(480–540), pseudo(580–600), starter(310/420/530), legend(600)`, with type
   distribution targets (each type ≥ 7 motifs across the dex; resonant ≤ 5).
2. **Design pass** — for each slot: name (naming guide, doc 01 §9), 1–2 lines of
   creature concept, stat spread from the archetype template (±10% skew toward a
   personality stat), learnset assembled from move-pool rules (STAB at 1/early/mid/
   late, 2 coverage, 2 status), tags, dex entry text (≤ 2 sentences, world-flavored).
3. **Mechanical check** — `tools validate`.
4. **Balance check** — `tools simulate --battles 2000 --pool batch+existing`:
   Monte-Carlo round-robin at levels 15/30/50 with AI tier 2. Reject/adjust if:
   any motif win-rate outside 35–65% in its band; any type's aggregate win-rate
   outside 40–60%; any move chosen > 3× expected frequency.
5. **Commit** the batch + the simulator report in the commit message.

Trainers, routes, and dialogue follow the same pattern with their own templates
(route trainer count/level curves are declared per badge gap in `region.ron`:
levels ≈ badge_n_ace − 3 … + 1).

## 5. Sigil generator (`tools sigils`) — the launch art style

Deterministic from `sigil_seed` (so art is reproducible and diffable):

- Body: 2–4 superimposed closed curves built from the species' **cry melody**
  rendered as a polar waveform (the creature literally *is* its sound).
- Symmetry: bilateral for feral/alloy/stone, radial for resonant/phantom, broken
  symmetry for venom; silhouette must read at 48×48.
- Palette: 4 colors from the type's base ramp (doc 05) + 1 accent from the seed;
  keyshifted = hue-rotated ramp + sparkle frame.
- Output: front 96×96, back 96×96, icon 32×32, party-sprite 48×48 → PNG →
  `tools atlas` packs into per-region atlases with a RON index.
- Eyes rule: every sigil gets exactly one readable "regard" element (eye/lens/
  fermata-dot). Creatures need a face. Non-negotiable.

**Upgrade path:** hand-drawn pixel art replaces the atlas entry by filename
convention (`fanfyre.front.png` in `assets/art-overrides/` wins over generated);
zero data changes.

## 6. Cry synthesis (`tools cries`)

Each species gets a 0.6–1.2 s melodic phrase, offline-rendered with fundsp → OGG:

- Seeded derivation: key + mode from type (ember = brassy square/saw, staccato,
  major-with-bite; tide = sine/FM, legato, lydian; phantom = detuned, minor,
  reverb-tail; stone = low percussive hits; resonant = pure harmonics + fifth),
  contour from 3–5 notes of a pentatonic walk, tempo from base speed stat,
  weight → pitch register (heavier = lower).
- Evolution lines share a leitmotif: the evolved cry is the base cry transposed
  down + one extra ornament note. (Players should *hear* the family.)
- Same melody seeds the sigil's polar waveform (§5) — the creature's look and sound
  are the same data. This is the signature trick of the project; protect it.
- Battle/UI jingles (catch settle, level-up, badge get) are composed the same way
  from fixed seeds, so the whole game shares one harmonic language.

## 7. Music plan (pragmatic)

- Structure first: `Music{track, fade}` script command, per-map track id, battle
  themes per trainer class, layered-stem support (intro + loop, optional intensity
  layer that fades in under 25% HP).
- Sources, in order: (1) generated stems via the cry pipeline's harmonic language
  for ambient/route beds, (2) CC0/CC-BY packs catalogued in `assets/CREDITS.md`,
  (3) commissioned/authored later. The Vault sequence (one held tone) is, fittingly,
  the easiest track in the game to produce.

## 8. Strings & localization

All text through keys (`strings.ron`), `{0}`-style args. English is source of truth.
The doc-01 "recontext" pairs are plain key variants (`quietcoast.fisher.2` /
`…2.recontext`) selected by flag in scripts — no special engine feature needed.

## 9. Definition of done for any content PR

`tools validate` clean → `tools simulate` report attached (if species/moves/trainers
changed) → new strings have no TODO markers → headless replay for any new
story-critical script → roadmap checkbox ticked.
