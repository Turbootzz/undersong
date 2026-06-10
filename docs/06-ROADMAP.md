# 06 — ROADMAP

> This is the agent's todo list. Work top-down. A phase is **done** only when its
> acceptance gate passes AND its phase review is clean; paste evidence into the
> STATUS block, tick the boxes, commit. Never start phase N+1 with a red gate at N.
> New ideas mid-phase → §Icebox, not into the sprint.

**Phase protocol (applies to every phase, no exceptions):**

1. On starting phase PN: `git tag pN-start` (so the phase diff is recoverable later).
2. Work the checkboxes top-down in small, always-green commits.
3. Run the acceptance gate; paste the evidence into STATUS.
4. **Phase review:** run the built-in `/code-review` skill at high effort and point
   it at the full phase diff (`git diff pN-start..HEAD`) — not just the last commit.
   Fix every finding (or record a justified won't-fix in STATUS); re-run until clean.
5. Tick the boxes, update STATUS (gate evidence + review verdict), commit.
   Only then start PN+1.

Phase sizing assumes Claude Code sessions; each box ≈ one coherent commit-series.

---

## P0 — Bootstrap (repo that proves the pipeline)

- [x] `cargo new` workspace; crates `core, battle, data, save, script, game, tools`
      per `03-ARCHITECTURE.md` §1; shared lints (`clippy::all`, `-D warnings`);
      release profile settings.
- [x] Pin deps from 03 §7 exactly; `rust-toolchain.toml` (stable, edition 2024).
- [x] `core`: ids (newtypes), Stat enum, Type enum, `BattleRng` wrapper (ChaCha8),
      no `thread_rng` reachable.
- [x] `data`: load `content/core/typechart.ron` + `natures.ron`; first validation
      rule (chart is total over 12×12).
- [x] `tools validate` skeleton (loads pack, runs rules, exit code).
- [x] `game`: Bevy 0.18 window at 480×270 integer scale, "UNDERSONG P0" text,
      AppState enum stubbed.
- [x] CI (GitHub Actions): fmt, clippy, test, validate; cache cargo+target.
- [x] Write the typechart + natures RON from doc 02 §1/§3 (data, not code).
- [x] **Phase review:** `/code-review` (high effort) over `git diff p0-start..HEAD` — all findings fixed, review clean.

**Gate P0:** `cargo run -p game` opens the window; CI green on a fresh clone.

## P1 — Battle core (the crown jewel, headless)

- [x] `battle`: BattleState, TurnActions, `step()` signature per 03 §2.
- [x] Stat math (02 §3) + unit tests with hand-computed values.
- [x] Damage pipeline (02 §4) exactly; integer-only; pipeline-order test vectors.
- [x] Action order: priority → speed → tie = rng; switch resolves before moves.
- [x] Status (02 §5): majors + confusion/flinch; end-of-turn tick order test.
- [x] Move effect interpreter for the Effect enum (02 §6); the 18 canon moves in RON.
- [x] Catch formula (02 §8) incl. ring-count events; exp/level/learnset (02 §9).
- [x] AI tiers 0–2 (02 §14); tier 3 stub returns tier-2 (full T3 in P5).
- [x] `tools simulate --battles N`: random legal teams from current species pool,
      win-rate + turn-count report.
- [x] CLI battle runner (`tools battle --seed`): renders the event stream as text.
- [x] proptest properties + golden replay corpus (5 scripted battles) per 03 §2.
- [x] **Phase review:** `/code-review` (high effort) over `git diff p1-start..HEAD` — all findings fixed, review clean.

**Gate P1:** `cargo test -p battle` green; 10,000-battle fuzz: zero panics, all
terminate; same seed twice ⇒ byte-identical event streams (test asserts it);
T2 beats T0 ≥ 90% with equal teams in `simulate`.

## P2 — Overworld & UI shell

- [ ] Hand-rolled tilemap (3 layers + collision + triggers) per 03 §3; one debug map.
- [ ] Grid movement w/ interpolation + input buffer; camera follow + map clamp.
- [ ] `tools importmap` for LDtk/Tiled JSON → `map.ron` (pick one editor, support it
      well; the other is best-effort).
- [ ] NPCs: static, wander, line-of-sight `!` engage (flag-gated).
- [ ] `script` crate interpreter + Dialogue UI (05 §4) running a test script.
- [ ] Warps + map transitions (measure-bar wipe); encounters rolling on resonance
      patches (02 §12) into a placeholder battle scene.
- [ ] UiTheme + palette.ron + fonts; pause menu skeleton; Settings (text speed,
      volume, scale).
- [ ] `save` crate v1 + 3 slots + autosave; round-trip tests; SaveBackend trait.
- [ ] UI spike: static battle layout (plates, waveform HP w/ fake data, move grid)
      to flush Bevy 0.18 UI API issues early.
- [ ] **Phase review:** `/code-review` (high effort) over `git diff p2-start..HEAD` — all findings fixed, review clean.

**Gate P2:** headless replay `walk_talk_warp_save.ron` passes (spawn → NPC chat →
warp → save → reload → position/flags assert); manual: walking around the debug map
at 60 fps feels like a Pokémon game.

## P3 — Vertical slice (first playable!)

Pausa Village → Route 1 → Prelude Town → Hall 1 (Dario).

- [ ] Content: 20 motifs (3 starter lines ×3 stages = 9, + 11 route/common),
      sigils + cries generated, balanced via `simulate` (gate thresholds in 04 §4).
- [ ] Maps: Pausa, Route 1, Prelude Town, Hall 1 interior; Reed's lab scene
      (starter choice — rival takes the counter, TACET later steals the third).
- [ ] Battle presenter: full event-stream rendering w/ timing/skip; catch flow with
      Fermata rings; faint/exp/level/learn-move prompts; evolution scene.
- [ ] Party + Summary (staff chart) + Bag + basic Box; marts; heal house
      ("Rest Stop": the nurse hums the heal jingle).
- [ ] Rival fight 1, 4 route trainers, Dario hall puzzle + Maestro fight (AI T2),
      Badge 1 → Lumen Hum performance unlock.
- [ ] Title screen + save select; intro cutscene (Reed's "the world is humming" talk).
- [ ] **Phase review:** `/code-review` (high effort) over `git diff p3-start..HEAD` — all findings fixed, review clean.

**Gate P3:** a fresh player reaches Badge 1 in < 30 min with zero crashes;
headless replay `new_game_to_first_badge.ron` passes in CI; `validate` + `simulate`
(win-rate bands) green for the 20-motif pool.

## P4 — Systems complete (everything the era expects)

- [ ] Full Box UI (16 boxes, quick-move); day/night cycle + per-time encounter
      tables; weather moves/abilities wired to overworld weather zones.
- [ ] Evolutions: all four methods (02 §9) incl. Duet Stone; friendship counter.
- [ ] Items pass: held items framework, all bell tiers, TMs (reusable), vitamins,
      Mute Charm, key items; Bag pockets final.
- [ ] Performances framework: all 6 (02 §11) with overworld interactions
      (cut brush, smash rocks, surf tiles, boulders, fly map).
- [ ] Trainer classes + payouts + rematch flag support; doubles battles.
- [ ] Abilities: all 24 implemented + tested; AI tier 3 (2-ply expectimax) done.
- [ ] Score (dex) screen with measure-fill; Programme (badge case); Player Card.
- [ ] Options: Set/Shift; battle anim toggle; reduced-motion + high-contrast.
- [ ] **Phase review:** `/code-review` (high effort) over `git diff p4-start..HEAD` — all findings fixed, review clean.

**Gate P4:** `cargo test --workspace` includes ability/evolution/perf-skill suites,
green; fuzz now includes doubles; replay corpus extended; T3 beats T1 ≥ 85%.

## P5 — Act 1 (Badges 1–3 + TACET introduced)

- [ ] Maps through Port Calando (Hall 3) + The Quiet Coast optional area.
- [ ] Species pool → 60 (batches per 04 §4, each batch simulate-gated).
- [ ] Story beats 1–5 (01 §5) scripted + tested; TACET grunts + Lull encounter;
      third-starter theft scene; Keyshift tutorial moment.
- [ ] Music: region overworld theme + battle themes (wild/trainer/hall) +
      Pausa/Prelude/Arbor/Calando town stems (04 §7); UI sfx set.
- [ ] First Anchor Echo quests (1–3) implemented (post-badge unlocks).
- [ ] Polish pass 1: move FX bursts, hit-stop, screen shakes, intro baton taps.
- [ ] **Phase review:** `/code-review` (high effort) over `git diff p5-start..HEAD` — all findings fixed, review clean.

**Gate P5:** replay `act1_complete.ron` (start → Badge 3 → Lull scene) passes;
playtest: 3–4 h of content; `simulate` bands green at 60 species.

## P6 — Acts 2–3 (Badges 4–8, Quartet, endings)

- [ ] Remaining maps/towns (Voltaccia → Cadenza City) + Vault of the Bass Clef
      (the thinning-music descent is a scripted audio sequence).
- [ ] Species pool → ~120 incl. legendaries (Primavoce, the Triad) + TACET admin
      aces; full encounter tables everywhere.
- [ ] Story beats 6–17: maintenance door, the Roster scene, Maren/Ilva fights,
      Calder's equation speech, Cade's plea (choice flag), Quartet gauntlet,
      Vault finale with **all three endings** + the 25 recontext line pairs.
- [ ] Anchor Echoes 4–8; Chorus-gate logic (Score ≥ 60% + 8 echoes) enforced and
      *communicated* in-game (Reed tracks it).
- [ ] Credits sequences per ending (incl. Da Capo's dead-input final 30 s).
- [ ] **Phase review:** `/code-review` (high effort) over `git diff p6-start..HEAD` — all findings fixed, review clean.

**Gate P6:** three replays — `ending_tacet.ron`, `ending_dacapo.ron`,
`ending_chorus.ron` — each reach their credits flag in CI; validator's
"ending reachability" rule green; full playthrough 12–18 h.

## P7 — Post-game, polish, ship

- [ ] The Encore (battle tower on sim ladder, streak save), Coda bell reward,
      legendary epilogue hunts, Vesper rematch.
- [ ] Performance/memory pass against the perf smoke budget; load-time pass.
- [ ] **WASM build**: SaveBackend→LocalStorage, autoplay gate verified, itch.io/web
      deploy script; native bundles (Win/Linux/macOS) via CI release workflow.
- [ ] Name & trademark check (Undersong + creature names); LICENSE decisions;
      README with screenshots; trailer GIFs.
- [ ] Difficulty review: badge-curve level audit vs simulate data; QoL audit list.
- [ ] **Phase review:** `/code-review` (high effort) over `git diff p7-start..HEAD` — all findings fixed, review clean.

**Gate P7:** web build playable start→Badge 1 in browser; native release artifacts
build in CI; no P0/P1-severity bugs open.

## P8 — "All regions" proof: Skalden pack

- [ ] Author `content/regions/skalden/` (folk identity): 4-badge mini-arc, ~40 new
      motifs, new tileset palette, travel unlock from the Chorus ending.
- [ ] Region-switch flow (boat from Port Calando), per-region music key change.
- [ ] Pack-format retro: document every place region-adding *did* touch engine code;
      fix the format so the next pack doesn't.
- [ ] **Phase review:** `/code-review` (high effort) over `git diff p8-start..HEAD` — all findings fixed, review clean.

**Gate P8 (the thesis):** Skalden boots and is completable with **zero engine-crate
changes** — only `content/` + `assets/` diffs (CI job asserts the diff paths).
Tamburra/Neonata are then "just content."

---

## Icebox (parked, deliberate)

- Breeding/eggs; held-item move interactions beyond the launch framework
- Real trading / link battles → **Chorus Network** companion service
  (axum + sqlx + **Postgres**, cloud saves, async trades, Encore leaderboard)
- Save obfuscation/compression (zstd + checksum)
- Hand-drawn pixel art overrides (per 04 §5 path) — commission/draw per species
- Adaptive overworld music layers reacting to party/story state (kira evaluation)
- Speedrun timer + seeded-run mode (the sim makes this nearly free)
- Localization beyond `en` (string tables are ready)

---

## STATUS (append newest on top — this is the session memory)

> Template:
> `### YYYY-MM-DD — Phase Px`
> `Done: …` / `Gate evidence: …` / `Next: …` / `Open questions: …`

### 2026-06-10 — Phase P1 (complete)

**Done:** The battle crate, whole: `step()` per 03 §2 (pure, BattleRng
the only entropy, self-contained BattleState with embedded chart/specs,
event-stream contract). Stat math §3 + nature grid; damage pipeline §4
exact (hand vectors pin crit-before-rand floor order, burn-physical,
weather scaling, crit stage-cancellation, min-1); turn structure + EOT
order per v1.1; status majors/volatiles with immunities; full §6 Effect
interpreter incl. two-turn commitment and Last Resort Hum; catch §8 with
ring events; exp/levels/learnsets §9 with EV caps; AI T0–T2 per §14
(+T3 stub). Core spec vocab (MoveSpec/Effect/SpeciesSpec/TypeChart in
core — battle and data are siblings that meet only there). 18 canon
moves as content with full cross-transcription tests. tools simulate
(Monte Carlo + tier regression) and tools battle (text event renderer);
dev testbed pool (12 species, one per type). Doc 02 gained v1.1
(mechanics completions, written before implementing) and v1.2 (phase-
review rulings).

**Gate P1 evidence:**
- `cargo test -p battle` green — 70 battle-crate tests (102 workspace).
- 10,000-battle fuzz: zero panics, all terminate, ~4 s; fuzz strategy
  emits all 14 Effect variants, random charts, wild + trainer kinds.
- Same seed twice ⇒ byte-identical streams: asserted by an engine test
  and a 64-case property over random battles (wild + trainer).
- `simulate --battles 1000`: **T2 beats T0 96.5%** (gate ≥ 90%); all 12
  testbed species inside 35–65% win band; avg 8 turns, 0 draws.

**Phase review:** CodeRabbit CLI (4 findings) + 50-agent adversarial
workflow (5 lenses → per-finding refutation; ~20 confirmed, 4 refuted).
Real engine bugs found and fixed: ForceSwitch/SelfSwitch could bench a
KO'd Mote with no Fainted event or award; two-turn moves double-charged
PP and ignored the committed slot; flurry/dustchord §7 primary effects
were missing; wild/AI Motes gained exp/EVs/levels mid-battle. All fixes
legislated in doc 02 v1.2 first, REPLAY_VERSION bumped to 2, goldens
regenerated. Refuted findings (no action): learnset dup-move rule,
confusion-unreachable (deferred by v1.2 #12), combined type-rational
(legalized by v1.2 #6), UPDATE_GOLDENS CI guard.

**Next:** P2 — `git tag p2-start`; hand-rolled tilemap (3 layers +
collision + triggers) per 03 §3; grid movement + camera; debug map.

**Open questions:** none.

### 2026-06-10 — Phase P0 (complete)

**Done:** Workspace bootstrap — 7 crates per 03 §1, shared lints
(`clippy::all` + `warnings` denied), pinned profiles, `rust-toolchain.toml`
(stable, edition 2024). `core`: id newtypes, `Stat`, `Type`, `Eff`,
`BattleRng` (ChaCha8; depends on rand_core + rand_chacha only, so
`thread_rng` is unlinkable). `data`: TypeChart/Natures schemas, RON
loaders, validation (12×12 totality, natures count/uniqueness;
`UniqueMap` rejects duplicate keys at parse time; `deny_unknown_fields`
everywhere). `content/core/`: typechart + natures transcribed from doc 02
§1/§3, guarded by a cross-transcription test (doc lists re-derived
independently of the RON matrix). `tools validate` CLI (exit 1 on any
error finding). `game`: Bevy 0.18.1, 960×540 fixed window (480×270 ×2),
nearest sampling, "UNDERSONG P0" text, AppState stub. CI workflow.
Package-name note: `crates/core` is `undersong-core` (cargo reserves
`core`) — 03 §10; all other package names match the docs.

**Gate evidence:**
- `cargo run -p game` → winit `Creating new window Undersong`, Metal
  renderer init, process healthy, no panic (macOS 26.5, M3 Pro).
- Fresh-clone simulation (full clone to /tmp): `cargo fmt --check` ✓,
  `clippy --workspace --all-targets -D warnings` exit 0,
  `cargo test --workspace` exit 0 (28 tests), `tools validate`
  → `0 error(s), 0 warning(s)` exit 0. GitHub Actions itself hasn't run
  yet (nothing pushed); the workflow mirrors exactly these commands.

**Phase review:** `/code-review` resolved to the CodeRabbit plugin
(1 finding) + a 22-agent adversarial review workflow over
`p0-start..HEAD` (4 lenses → per-finding refutation agents; 16 confirmed,
2 refuted). All confirmed findings fixed: duplicate-RON-key last-wins
through the validator (major → `UniqueMap`), CI missing wayland build
deps for bevy 0.18 on ubuntu runners (critical), `range_inclusive`
full-domain overflow, missing `--locked`, dead `rustup show` step,
`expect(dead_code)`, full 25-key natures assertion, `rand_core` recorded
in 03 §7, doc 02 frost sanity-count corrected 3→4 via changelog.
Won't-fix (justified): CodeRabbit's `data`→`undersong-data` rename — docs
pin the crate names used in cargo commands; only `core` is cargo-reserved
(03 §10). Pinning a concrete stable toolchain version — roadmap mandates
channel "stable"; the standalone finding was refuted in verification.

**Next:** P1 — `git tag p1-start`; `battle` crate: `BattleState`,
`TurnActions`, `step()` per 03 §2; stat math (02 §3) with hand-computed
test vectors.

**Open questions:** none.
