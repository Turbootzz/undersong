# Undersong — Session handoff (written 2026-06-11, for the P17+ session)

You are a fresh Claude session picking up a mature project. Read order:
`CLAUDE.md` → `docs/06-ROADMAP.md` §P17–P20 + §STATUS (the ledger of
everything done) → this file. Doc 02 is mechanics law; doc 05 v2 is the
art direction; doc 03 is architecture law (the lib stays Bevy-free).

## State at handoff

P0–P16 complete (see STATUS ledger). The game: 130 species across two
regions, three endings, post-game, generated pixel-art everything,
Gen-3 battle layout, a Bun/TS/Tailwind/Vue wiki under `wiki/`, and six
recorded replay gates in CI. The user's third playtest ordered the
P17–P20 "look & feel" arc: battle theater, overworld feel, beauty pass
2 (pixel font), sound & light story. **Hero variant B (teal wayfarer)
is chosen — switching it is P17's task zero.**

## Commands you will live in

```bash
cargo test --workspace                       # 193 tests incl. 6 replay gates
cargo run -p tools -- validate               # content gate — run before every content commit
cargo run -p tools -- sprites                # regenerates ALL art (tiles/chars/creatures/stars/heroes)
cargo run -p tools -- music                  # regenerates stems + cues
cargo run -p tools -- wiki                   # regenerates wiki JSON + sprites (CI checks drift)
python3 scripts/mapgen.py                    # regenerates every map.ron (idempotent)
cargo run -p game                            # play (1280×720)
```

Dev rigs (env vars on `cargo run -p game`):
- `UNDERSONG_SHOT=path.png` — auto-screenshot ~6s after boot; F12 manual.
- `UNDERSONG_BOOT_CONTINUE=1` — skip title via slot-1 save; `=fresh` for a new world.
- `UNDERSONG_BOOT_BATTLE=<trainer_id>` — drop straight into that fight.
- `UNDERSONG_VISUAL_REPLAY=tests/replays/<f>.ron` — replay a recorded run
  in the window, filming frames to `docs/playtests/<stem>/`. **This is
  your playtesting instrument — use it for the self-review gates.**

Screenshots larger than ~1500px get rejected when you Read them — downscale
first: `sips -Z 1200 in.png --out small.png`.

## Self-review protocol (the user's explicit mandate)

You can see the game now; judge it. Every phase in this arc ends with:
1. Harness film(s) + boot/battle screenshots of the new work.
2. Read the frames yourself and write an honest playfeel paragraph in
   STATUS: what reads well, what's still stiff/ugly, what you'd fix
   next. The user wants your taste engaged, not just your tests.

## Where things live

| Thing | Place |
|---|---|
| Art generator (tiles/chars/creatures) | `crates/tools/src/sprites.rs` — grammar v2: `body_from_seed`, `creature_front/back`, `blot_shaded`, plans enum |
| Hand-authored stars (char-grid pixel art) | `crates/tools/src/stars.rs` — 32×32 grids ×3 upscale; legend chars→colors; `derive_back` |
| Hero variants + player set | `crates/tools/src/heroes.rs` — `render_heroes` paints `player.*` with `legend_a()` today; **switch to `legend_b()` for hero B** |
| Music/sfx generator | `crates/tools/src/music.rs` — Mood × Mode (ionian/dorian/aeolian), track table in `main.rs` |
| Wiki exporter | `crates/tools/src/wiki.rs`; site in `wiki/` (Bun + TS + Tailwind v4 + Vue 3; `bun run build` must stay green — CI checks data drift) |
| Presenter | `crates/game/src/app.rs` (overworld, rigs, music director, cues via `play_cue`), `battle_ui.rs` (battle scene, FxState, plates), `screens.rs` (hub + Score-dex) |
| Pure core | `crates/game/src/{world,session}.rs` — `state × Input → events`; NEVER put presentation here |
| Art override | `assets/custom/<same rel path>` beats generated (`game::art::art()`, test-enforced) |
| Map generator | `scripts/mapgen.py` — maps are committed artifacts; regen is idempotent; `dress()` paints building decor |
| Replays | `tests/replays/*.ron`; re-record with `UPDATE_REPLAYS=1 cargo test -p game --test <runner>` |

## Rules learned the hard way (violate at your peril)

1. **Clippy before commit, always** (`cargo clippy --workspace
   --all-targets -- -D warnings`). Three pushes went out red this way.
2. **Every replay needs its playback test in the same commit** — a
   recording without one went stale invisibly for a whole phase.
3. **Content changes near recorded paths invalidate replays.** New map
   *triggers* on a recorded path = re-record. Pure *decor* changes are
   safe (P14 proved it). Budget re-records as routine.
4. **Glyphs**: the default font drops —, ₵, ♪, ·, ◀▶, …; the P19 pixel
   font may fix this, but until then ASCII only in UI/content strings.
5. **Bevy 0.18 quirks**: `Anchor` is a separate component (constant
   `Anchor::BOTTOM_CENTER`), not a Sprite field; one-shot audio =
   `AudioPlayer::new + PlaybackSettings::DESPAWN.with_volume(Volume::
   Linear(v))`; UI borders need both `Node.border` width and
   `BorderColor::all(...)`; when an API fights you, check
   https://docs.rs/bevy/0.18.1 before guessing.
6. **The lib (`game::`) stays Bevy-free** (doc 03). `art()` lives in
   `game::art` precisely for that.
7. **Characters are 32×44 bottom-anchored** (`char_sprite`/`char_pos`
   in app.rs); world actor positions use y = tile*TILE (feet), not
   center.
8. **Determinism**: nothing presenter-side may feed back into the core.
   Animations read events; they never produce inputs.
9. After `cargo fmt`, your in-flight `python3` string replacements may
   stop matching — re-grep before patching.
10. Commit style: conventional commits, end with the Co-Authored-By
    line for Claude; STATUS block + box-ticking closes every phase;
    `git tag pN-start` opens the next.

## Sequencing notes for P17–P20

- P17 task zero (hero B) is one legend swap + regenerate + screenshot.
- Battle effects: drive everything from the existing `BattleEvent`
  stream in `battle_ui.rs::queue_battle_events_fx` (it already maps
  events → FxState; extend, don't parallel).
- The evolution scene hooks `WorldEvent`s around `Input::Evolve` — the
  prompt flow already exists in the core; the scene is pure presenter.
- P19 font: m5x7 by Daniel Linssen (free) or monogram (CC0) — download,
  vendor under `assets/fonts/`, include the license text, note in doc
  03 §7 as an asset. Then sweep `TextFont::from_font_size` calls —
  pixel fonts want integer multiples of their native size.
- P20 music: extend the track table; the music director maps map.music
  ids — new ids in content maps need stems generated first (engine
  list + content id must agree; see docs/08-PACK-RETRO.md wrinkle #4).

## Open user preferences on file

- Hero B chosen; character designs overall "not a big fan yet" —
  iterate when judgment says so, screenshot variants for him to pick.
- Monster art: "more detail and personality" — P19's pass; stars are
  hand-made in stars.rs if you'd rather author than generate.
- He wants the game compared against other indie pixel games — do the
  side-by-side honestly in P19 and say where it falls short.
