# 05 — UI & VISUAL STYLE

> Goal: "nice looking and pleasing to the eye" on a retro chassis. The identity is
> **sheet music made playable**: staff lines, clefs, waveforms, ink-on-parchment.
> Every widget should look like it was engraved, not skinned. When in doubt, ask:
> *would this read on a printed score?*

## 1. Canvas & scaling

- Virtual resolution **480×270** (16:9), integer-scaled (×2/×3/×4...), nearest-neighbor,
  letterboxed. All UI is authored in virtual pixels; never sub-pixel positions.
- Tiles 16 px; overworld character sprites 16×24 (overhang head); battle sigils
  rendered at 96×96 (front) / 80×80 (back, slightly cropped, classic over-shoulder).
- Safe margins: 8 px from every screen edge for any text.

## 2. Palette

Global "ink & parchment" base + one accent ramp per region. Colors live in
`content/core/palette.ron`; nothing is hardcoded.

| Token | Hex | Use |
|---|---|---|
| ink | `#1a1822` | text, outlines, staff lines |
| ink_soft | `#3c3a4a` | secondary text, disabled |
| parchment | `#f2e9d8` | panel fill |
| parchment_dim | `#d9cfba` | panel fill (inactive) |
| gilt | `#c9a227` | badges, highlights, "Keyshifted" sparkle |
| cantorel_accent | `#7d4f9e` | region ramp base (orchestral violet) |
| hp_high | `#46b06a` | HP > 50% |
| hp_mid | `#d8a534` | 20–50% |
| hp_low | `#c0392b` | < 20% (pulse) |
| type colors | 12 entries | one per type; defined in palette.ron; must pass 4.5:1 contrast on parchment |

Night palette: multiply overworld by `#9aa0c8` at 60% (data-driven curve per hour).

## 3. Typography

- Pixel font (P19, shipped): **monogram** by datagoblin, CC0 — one
  monospace 6×9 face for everything (`assets/fonts/monogram.ttf`,
  license vendored alongside). It replaces Bevy's default-font handle
  at boot, so all text is pixel type with no per-site plumbing.
  (m5x7/m6x11 were the original plan; monogram won on glyph coverage —
  577 glyphs incl. em-dash, cedi, ellipsis, arrows — and CC0.)
- Sizes ride the pixel grid: monogram is 16 design px per em, so with
  UiScale 8/3 the on-grid logical sizes are multiples of **6.0** on
  every display (6.0 fine print, 12.0 headings) or multiples of 3.0
  on 2x displays (**9.0 body**, 27.0 title — accepted: crisp on 2x,
  slightly uneven at 1x). Never scale fonts off this grid.
- Text speed: instant per-character reveal at 30/60/instant chars-per-tick (Settings);
  default fast. A/confirm dumps the full page (era convention).

## 4. Core widgets

### Dialogue box
- Bottom-anchored, 480×64, parchment 9-slice with a **staff-line motif**: four faint
  horizontal rules inside the panel, like an empty staff awaiting notes.
- Speaker name on a small tab (left), portrait optional (right, 48×48 sigil-style
  cameo for major cast).
- Choice list pops above as a narrow 9-slice with a treble-clef cursor (replaces the
  classic ▶).

### HP bar = waveform (signature widget)
- Not a rectangle. A **sine ribbon** drawn left→right inside a 1 px ink channel:
  - amplitude = HP% (full HP = tall confident wave; low HP = shallow tremble),
  - frequency fixed per species (derived from `cry_seed` — every Mote's "pulse"
    looks subtly its own),
  - color from hp_high/mid/low; at < 20% the ribbon **pulses** at 90 bpm,
  - at 0 HP the wave **flatlines** to a straight ink line, then fades — that's the
    faint animation.
- Damage: the wave amplitude eases down over 300 ms; heals ease up with a soft
  shimmer. Numbers tick alongside (current/max in m6x11).
- Implementation: one quad + small shader (or 48-segment polyline mesh updated per
  frame — fine at this resolution; choose simplest that works in Bevy 0.18).

### EXP bar
- Thin gilt "measure fill" under the HP channel; on level-up it flashes as a
  **bar line** stamps at the right edge.

### Stat display = staff chart (Summary screen)
- Replaces the radar/hexagon: a 5-line musical staff; each of the six stats is a
  **note head** placed at a height proportional to the stat (relative to species max
  at that level), in left→right order `HP atk def spa spd spe`, with ledger lines
  when off-staff. Nature-boosted stat note is gilt; hindered is hollow.
- Hovering/selecting a note shows exact value + EV/IV ("Practice / Timbre") detail.

### Type icons = instrument glyphs (16×16, 1-bit + type color)
| Type | Glyph | Type | Glyph |
|---|---|---|---|
| feral | tuning fork | stone | timpani |
| ember | horn | frost | glass chime |
| tide | wave-cello scroll | venom | bent reed |
| bloom | pan flute | phantom | theremin wisp |
| volt | electric pickup | alloy | cymbal |
| gale | flute | resonant | fermata symbol |

### Buttons / menu rows
- Parchment rows, ink text, 2 px ink outline; selected row gets a gilt left bar +
  the clef cursor. No drop shadows, no gradients — flat engraved look.

## 5. Screen inventory (what exists, roughly wireframed in words)

| Screen | Layout notes |
|---|---|
| Title | Logo (wordmark over a slowly-scrolling grand staff), "press start" pulse; the title theme begins **only after first input** (doubles as the WASM autoplay gate) |
| Save select | 3 slot cards: name, badges as inked clefs, playtime, party sigil row |
| Overworld HUD | Clean by default. Contextual button hints fade in at interactables. Performance prompt shows the needed instrument glyph |
| Pause menu | Right-side vertical panel: Score(dex) / Party / Bag / Player Card / Save / Settings — classic order |
| Battle | Foe plate top-left (name, lvl, status chip, waveform HP), player plate bottom-right (+ EXP); message box bottom; 4-slot move grid color-tinted by type with PP `x/y` and phys/spec/status glyph; intro = **conductor's baton tap-tap** then theme |
| Party | 6 row-cards: mini-sigil, name, waveform HP (small), status chip; reorder/hold-to-swap |
| Summary | 3 tabs: Profile (sigil, dex blurb, Temperament), Staff chart (stats), Moves |
| Bag | 4 pockets (Items / Bells / TMs / Key); icons 16×16; description panel below |
| Box ("Repertoire") | 6×5 grid of mini-sigils per box, 16 boxes, quick-move mode |
| Score (dex) | The showpiece: entries are **measures on a grand score**, filled in as captured/seen; completion % shown as "movement progress"; selecting an entry plays the cry and shows the sigil + dex text |
| Badge case ("Programme") | A concert programme page; each badge inked as its historical clef when earned |
| Settings | Text speed, battle anims on/off, Set/Shift, volume sliders, palette scale, screen scale, key remap |

## 6. Motion language

- Standard ease: cubic-out, **150 ms**; nothing slower than 250 ms except scripted
  cutscenes. Everything skippable with confirm.
- Scene transitions: **measure-bar wipe** — a vertical bar line sweeps across like a
  page turn (battle entry: three quick bar lines on the baton taps, then white-out).
- Battle move FX at launch: type-colored geometric bursts + screen shake tiers
  (S/M/L) + hit-stop 2 frames on super-effective. No bespoke per-move animation
  before P6 polish; the event stream makes adding them later mechanical.
- Keyshifted (shiny) reveal: the sigil draws in **one palette rotation step off**,
  with a brief arpeggio sparkle and a gilt note above its plate.

## 7. Audio identity (UI side)

- Every UI tick is a short pitched pluck in the current region's key; confirm = up a
  third, cancel = down a third, error = flat second. Generated once by `tools cries`
  into `assets/sfx/ui/`.
- Low-HP situation layers a quiet metronome under the battle theme (classic beep,
  reharmonized).

## 8. Accessibility (cheap, do from day one)

- Screen scale ×1–×4 + fullscreen; remappable keys + controller.
- Text size is fixed-pixel but the **reduced-motion** toggle disables shakes/pulses;
  **high-contrast** toggle swaps parchment→white, ink→black.
- Color is never the only channel: type glyphs accompany type colors; status chips
  have letters; effectiveness messages are textual.
- All cries/jingles have visual counterparts (waveform flash on the plate).

## 9. Implementation notes for Bevy 0.18

- Build widgets as small composable bundles in `game::ui` (plate, panel9, wavebar,
  staffchart, movegrid…). One `UiTheme` resource loads palette.ron; **no literal hex
  in systems**.
- Prefer Bevy UI nodes for menus/panels; the waveform HP and staff chart are
  world-space meshes/sprites inside UI-anchored containers (simplest path at this
  resolution).
- Check current Bevy UI API on docs.rs before building — UI is the most
  version-drifted part of Bevy. Budget one spike task in P2 for "render the battle
  layout statically" before wiring data.


---

## v2 Art Direction (P10+, decided 2026-06-11 with the user)

Supersedes the flat-quad placeholder look everywhere it conflicts.

1. **Canvas**: 640×360 internal, 32px tile grid, integer scaling to window.
2. **Source of art**: everything generated deterministically by `tools sprites`
   (seeded like cries/sigils — one seed family per species keeps evolution lines
   visually related). **Override rule**: a PNG at `assets/custom/<same relative
   path>` always replaces its generated counterpart; the loader checks custom
   first. Hand-made art needs zero code changes.
3. **Tiles**: textured, with edge/corner variants (grass↔path↔water blending,
   tree clusters, building walls/roofs/doors). Region palette keys: Cantorel
   warm parchment-and-moss; Skalden cold fjord blues and bone.
4. **Characters**: 32px-tall sprites, 4 directions, 2-frame walk. Distinct
   archetype silhouettes (hat/robe/apron/pack) + class accent colors.
5. **Monsters**: body-plan grammar. Primary type + tags pick a silhouette family
   (quadruped, bird, serpent, moth, fish, blob, golem, sprite); the species seed
   drives proportions, markings, and palette within doc-05 ramps. Outputs:
   16×16 menu icon, 32×32 overworld, 96×96 battle front, 96×96 battle back.
   Readability rule: silhouette first — a galliard must read as *a creature*
   at battle size before any detail goes in.
6. **Battle scene**: classic Gen-3 layout — foe front-view upper-right on a
   ground platform, ally back-view lower-left, HP boxes with name/level/bar/
   status, message bar below. The sigil medallion moves to the Score screen
   and dex entries (it stays the species' "signature", not its battle body).
7. **UI chrome**: 9-slice parchment panels with ink borders and gilt accents,
   text blips during dialogue, cursor/confirm/cancel cues on every menu.


## v3 addenda (P17–P20 arc, decided 2026-06-11)

1. **Hero**: variant B — the teal Wayfarer (scarf, auburn hair, satchel).
2. **Font**: a vendored pixel font (m5x7 or monogram) replaces the
   default; all UI sizes move to integer multiples of its native size.
3. **Motion language**: battle = type-flavored effect families over a
   universal lunge/flash/shake base; overworld = 4-frame player gait,
   2-frame NPC bob, "!" spotted bubble with alert cue and a beat of
   pause. Animations are presenter-only — they read events, never make
   inputs (doc 03 determinism holds).
4. **Self-review**: every presentation phase closes with the agent
   filming harness runs and writing a playfeel verdict in STATUS.
