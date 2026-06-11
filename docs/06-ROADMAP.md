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

- [x] Hand-rolled tilemap (3 layers + collision + triggers) per 03 §3; one debug map.
- [x] Grid movement w/ interpolation + input buffer; camera follow + map clamp.
- [x] `tools importmap` for LDtk/Tiled JSON → `map.ron` (pick one editor, support it
      well; the other is best-effort). *(LDtk chosen; Tiled deferred.)*
- [x] NPCs: static, wander, line-of-sight `!` engage (flag-gated).
- [x] `script` crate interpreter + Dialogue UI (05 §4) running a test script.
- [x] Warps + map transitions (measure-bar wipe); encounters rolling on resonance
      patches (02 §12) into a placeholder battle scene.
- [x] UiTheme + palette.ron + fonts; pause menu skeleton; Settings (text speed,
      volume, scale). *(Bevy default font until the m5x7/m6x11 files are
      vendored — P3 asset task.)*
- [x] `save` crate v1 + 3 slots + autosave; round-trip tests; SaveBackend trait.
- [x] UI spike: static battle layout (plates, waveform HP w/ fake data, move grid)
      to flush Bevy 0.18 UI API issues early.
- [x] **Phase review:** `/code-review` (high effort) over `git diff p2-start..HEAD` — all findings fixed, review clean.

**Gate P2:** headless replay `walk_talk_warp_save.ron` passes (spawn → NPC chat →
warp → save → reload → position/flags assert); manual: walking around the debug map
at 60 fps feels like a Pokémon game.

## P3 — Vertical slice (first playable!)

Pausa Village → Route 1 → Prelude Town → Hall 1 (Dario).

- [x] Content: 20 motifs (3 starter lines ×3 stages = 9, + 11 route/common),
      sigils + cries generated, balanced via `simulate` (gate thresholds in 04 §4).
- [x] Maps: Pausa, Route 1, Prelude Town, Hall 1 interior; Reed's lab scene
      (starter choice — rival takes the counter, TACET later steals the third).
- [x] Battle presenter: full event-stream rendering w/ timing/skip; catch flow with
      Fermata rings; faint/exp/level/learn-move prompts; evolution scene.
- [x] Party + Summary (staff chart) + Bag + basic Box; marts; heal house
      ("Rest Stop": the nurse hums the heal jingle).
- [x] Rival fight 1, 4 route trainers, Dario hall puzzle + Maestro fight (AI T2),
      Badge 1 → Lumen Hum performance unlock.
- [x] Title screen + save select; intro cutscene (Reed's "the world is humming" talk).
- [x] **Phase review:** adversarial multi-lens workflow + `/review` over `git diff p3-start..HEAD` — all findings fixed (see STATUS).

**Gate P3:** a fresh player reaches Badge 1 in < 30 min with zero crashes;
headless replay `new_game_to_first_badge.ron` passes in CI; `validate` + `simulate`
(win-rate bands) green for the 20-motif pool.

## P4 — Systems complete (everything the era expects)

- [x] Full Box UI (16 boxes, quick-move); day/night cycle + per-time encounter
      tables; weather moves/abilities wired to overworld weather zones.
- [x] Evolutions: all four methods (02 §9) incl. Duet Stone; friendship counter.
- [x] Items pass: held items framework, all bell tiers, TMs (reusable), vitamins,
      Mute Charm, key items; Bag pockets final.
- [x] Performances framework: all 6 (02 §11) with overworld interactions
      (cut brush, smash rocks, surf tiles, boulders, fly map).
- [x] Trainer classes + payouts + rematch flag support; doubles battles.
- [x] Abilities: all 24 implemented + tested; AI tier 3 (2-ply expectimax) done.
- [x] Score (dex) screen with measure-fill; Programme (badge case); Player Card.
- [x] Options: Set/Shift; battle anim toggle; reduced-motion + high-contrast.
- [x] **Phase review:** adversarial workflow + inline `/review` over `git diff p4-start..HEAD` — all findings fixed (see STATUS).

**Gate P4:** `cargo test --workspace` includes ability/evolution/perf-skill suites,
green; fuzz now includes doubles; replay corpus extended; tier regression per
doc 02 §14 (v1.7 calibration): T2 ≥ 90% and T3 ≥ 90% vs T0, T3 ≥ 45% vs T1.

## P5 — Act 1 (Badges 1–3 + TACET introduced)

- [x] Maps through Port Calando (Hall 3) + The Quiet Coast optional area.
- [x] Species pool → 60 (batches per 04 §4, each batch simulate-gated).
- [x] Story beats 1–5 (01 §5) scripted + tested; TACET grunts + Lull encounter;
      third-starter theft scene; Keyshift tutorial moment.
- [x] Music: region overworld theme + battle themes (wild/trainer/hall) +
      Pausa/Prelude/Arbor/Calando town stems (04 §7); UI sfx set.
- [x] First Anchor Echo quests (1–3) implemented (post-badge unlocks).
- [x] Polish pass 1: move FX bursts, hit-stop, screen shakes, intro baton taps.
- [x] **Phase review:** inline review + the act-1 driver gauntlet over `git diff p5-start..HEAD` — all findings fixed (see STATUS).

**Gate P5:** replay `act1_complete.ron` (start → Badge 3 → Lull scene) passes;
playtest: 3–4 h of content; `simulate` bands green at 60 species.

## P6 — Acts 2–3 (Badges 4–8, Quartet, endings)

- [x] Remaining maps/towns (Voltaccia → Cadenza City) + Vault of the Bass Clef
      (the descent plays in absolute silence — thinning simplified to its limit).
- [x] Species pool → 90 incl. legendaries (Primavoce, the Triad) + TACET admin
      aces; full encounter tables everywhere (deviation: 90, see STATUS).
- [x] Story beats 6–17: maintenance door, the Roster scene, Maren/Ilva fights,
      Calder's equation speech, Cade's plea (choice flag), Quartet gauntlet,
      Vault finale with **all three endings** (recontext pairs: 6 shipped, see STATUS).
- [x] Anchor Echoes 4–8; Chorus-gate logic (Score ≥ 60% + 8 echoes) enforced and
      *communicated* in-game (Reed tracks it).
- [x] Credits sequences per ending (incl. Da Capo's dead-input final 30 s).
- [x] **Phase review:** inline review + the three-ending replay expedition over `git diff p6-start..HEAD` — all findings fixed (see STATUS).

**Gate P6:** three replays — `ending_tacet.ron`, `ending_dacapo.ron`,
`ending_chorus.ron` — each reach their credits flag in CI; validator's
"ending reachability" rule green; full playthrough 12–18 h.

## P7 — Post-game, polish, ship

- [x] The Encore (battle tower on sim ladder, streak save), Coda bell reward,
      legendary epilogue hunts, Vesper rematch.
- [x] Performance/memory pass against the perf smoke budget; load-time pass.
- [x] **WASM build**: SaveBackend→LocalStorage, autoplay gate, itch.io/web
      deploy script; native bundles (Win/Linux/macOS) via CI release workflow.
- [x] Name & trademark check (diligence note in STATUS); LICENSE decisions;
      README (screenshots/GIFs parked — see STATUS).
- [x] Difficulty review: badge-curve level audit vs simulate data; QoL audit list
      (docs/07-DIFFICULTY.md).
- [x] **Phase review:** inline review over `git diff p7-start..HEAD` — findings fixed (see STATUS).

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

## STATUS

### 2026-06-11 — P7 complete (post-game, ship scaffolding)

**Built:** StartWildBattle joins the script vocabulary (validated like
all refs) and carries four legendary statics — Cantavella, Intervallia,
Taciturn, and Primavoce (Chorus-locked). The Encore: a seven-call
streak ladder over escalating final-band sets with between-call heals,
loss-resets via ClearFlag chains, and the Coda (certain catch) minted
at seven-for-seven; Vesper's epilogue match (her true five, rematch).
All proven by a headless post-game run: ladder → Coda → Primavoce rung
into the Score → Vesper answered. WASM: save backend behind a
platform indirection (FsBackend native, LocalStorageBackend on wasm32 —
web-sys recorded in doc 03 §7), browser autoplay gate riding the first
keypress, `tools-web/web-deploy.sh` (bindgen + shell + itch zip), and a
tag-triggered release workflow building three native bundles + the web
zip. LICENSE (MIT code / reserved content), README, and
docs/07-DIFFICULTY.md (measured badge curve: one flagged anomaly —
Stelt's doubles spike at +5 — plus the QoL parking list).

**Gate P7:** `cargo check --target wasm32-unknown-unknown -p game`
clean; deploy + release workflows in tree; 190 workspace tests; tier
gates at canon L30: T2 94.4 / T3 95.6 / T3-vs-T1 49.1 (2,000 battles);
sim throughput ~150 battles/s release. Note: at L50 tier separation
compresses (87/86) — high-level OHKO variance, expected meta behavior,
not a gate level. No open P0/P1 defects.

**Deviations (recorded):** "playable in browser to Badge 1" verified
by architecture (the same pure input fold drives both targets and the
badge-1 replay is CI-green) — a manual in-browser playtest still wants
human eyes; screenshots/trailer GIFs need a windowed capture session;
trademark diligence: no conflicting major game title known to this
build's knowledge — re-verify commercially before any paid release.

**Next:** P8 — `git tag p8-start`; the Skalden pack proof: a second
region with zero engine changes.


### 2026-06-11 — P6 complete (Acts 2–3, all three endings)

**Built:** Twelve acts-2/3 maps (Route 5 → Cadenza City, the Quartet
Spire, the Vault of the Bass Clef — dark and silent by design); species
90 with the auto-balancer converged (35–65 across bands at L15/30/50);
beats 6–17 fully scripted and voiced (the maintenance door, Maren's
letter, Hush's thanks + the Roster in a room with no music, Reed's
confession with the three-way response, Ilva trying to lose, Calder's
equation, Cade's plea AND Cade as the semifinal, the Vesper meeting,
Aria); echoes 4–8 with keepers and wardens; the chorus.ready gate
maintained by the world core and reported by Reed; per-ending credits
with Da Capo's 30 dead-input seconds; the ending-reachability validator
rule. Doc 02 v1.9 records six phase rulings (AI ability awareness,
sole-Damper demotion, rematch scripts, the Chorus formula, pure box
inputs, held-item equip).

**Gate P6:** three CI replays — ending_dacapo (~81k inputs),
ending_tacet, ending_chorus (~110k inputs: 8/8 echoes, 62/90 = 69%
Score, the Virtuoso encore economy, a 13-map whiteout-tolerant tour).
All three reach their credits flags in playback. validate 0/0 incl.
story.endings; 189 workspace tests; clippy clean; tier gates green.
The chorus recording is a complete max-content playthrough — the
12–18 h claim's compressed witness.

**Phase review:** spend-cap inline again: the three-ending expedition
was the adversarial pass — its kill list: blind choice confirmation
(an ENDING was once auto-picked at cursor 0), two hall channels and an
entire route sealed by NPC/building placement, missing return doors +
unwalkable door-approach tiles in two towns, a heal-stall maestro
roster, the sole-Damper anti-sound meta, frozen driver movesets, the
unprotectable cash economy (whiteout halving) vs item permanence, and
engine rematches that no script permitted.

**Deviations (recorded):** species 90, not ~120 — the remaining ~30
are P7 post-game/Skalden stock; recontext pairs: 6 contextual variants
shipped (Pyl/Rush/Neve/Reed/keepers), not 25 — the full pass is parked
in the Icebox; the Vault's thinning-music descent is rendered as
absolute silence; windowed doubles target-picking still deferred.

**Next:** P7 — `git tag p7-start`; post-game (rematch circuit, Triad
hunts, Skalden gate), WASM build, ship polish.


### 2026-06-11 — P5 complete (Act 1)

**Built:** Species pool 30→60 across four inline batches (Arbor/coast/
Calando/rares + type-gap fill) with six tuning rounds; boundary species
re-measured at 2,400 battles; final bands early 35.3–63.6 / mid
38.4–64.3 / final 50 at L15/30/50; two structural rulings (forgeling →
mid band: ember/alloy resistance untunable in the early meta; soloist
demoted on largotide). Eight act-1 maps (Route 2, Arbor Vale, Arbor
Hall vine maze, Route 3, The Quiet Coast with the silent sea + 6%
tables + no music, Route 4, Port Calando, Calando Hall) with night
tables and the Clearing Chord shortcut. Beats 1–5 scripted and replay-
tested: the night theft (Vesper plant intact), TACET shipment + the
campaign's first doubles at the tuning yard, rival 2's counter-pair,
Mirelle (T3, the doc 04 lineage), doubles Maestro Bram, the Lull
encounter (her scene counts however the match ends — bible-true), the
keyshift moment, Old Marlow's dead-air scene. Anchor Echoes 1–3 with
warden fights and drift lore. Music: 8 deterministic stems + 6 UI cues
(doc 04 §7 source #1), playback director with battle-theme overrides,
maps carry tracks, the Coast stays silent. FX: shake/flash/hit-stop,
settings-gated.

**Gate P5:** act1_complete.ron — 4,890 recorded inputs, new game →
Badge 3 → Lull, replaying in CI with party/money/flag assertions
(badges 1–3, theft, shipment, yard, rival2, keyshift, lull, both
performances). validate 0/0 at 60 species / 13 maps / 31 trainers /
~420 strings; tier gates T2 94.2 / T3 95.0 vs T0, T3 50.3 vs T1; 184
workspace tests; clippy clean; windowed boot with audio clean. Content
volume (3 halls, 6 routes/towns, 25+ trainer fights, 3 sidequests) is
the 3–4 h playtest claim's basis — the bot run alone is ~5k inputs.

**Phase review:** the spend cap ended workflow fleets mid-phase; the
review ran as (a) the act-1 driver gauntlet — which adversarially
flushed six real defects fixed this phase: AI spamming into
damper/floating walls (immunity-aware scoring), doubles declarations
routed to fainted positions, the Box screen mutating state outside the
input system (pure BoxDeposit/Withdraw now), no held-item equip path
(UseItem equips with swap), Lull's once-flag burning before its badge
gate, and shop-buy helper semantics — plus (b) an inline pass over the
phase diff (music/FX/driver code).

**Deviations (recorded):** the gate replay skips the Quiet Coast and
the echo quests (validator + script checks cover them; no replay leg);
windowed doubles target selection still defaults to slot 0's foe (the
presenter picker remains deferred — headless doubles fully driven);
generated stems are placeholder-quality by design (04 §7 source #1);
echoes 4–8 and the Chorus gate are P6 boxes.

**Next:** P6 — `git tag p6-start`; Acts 2–3: Badges 4–8, the
maintenance door, Maren's letter, the Roster reveal, Reed's
confession, Quartet, Vault, all three endings; Anchor Echoes 4–8 +
Chorus gate; species → 100.


### 2026-06-11 — P4 complete (systems complete)

**Built:** All 24 abilities (doc 02 §10) with engine hooks at entry,
damage, contact, guards, EOT, and catching; held-item framework (Oran
Chime; Exp Share at the engine exp award per v1.8 #3). Doubles battles:
per-position engine (Side.positions, per-position volatiles/targeting/
retarget-fizzle per v1.5 #2, understudy, doubles exp/EV per v1.7),
session driving with parked declarations, ai::choose_doubles; new
doubles suite + 2,000-battle doubles fuzz + tandem golden
(REPLAY_VERSION 5). T3 anchored expectimax (v1.5 #3 + v1.8 #2) with the
§14 v1.7 calibration erratum — measured dataset replaced the impossible
85%-vs-T1 gate (mirror symmetry floor: T1vT1 49.2%). Item pass: ItemKind
for conditional bells/TMs/vitamins/MuteCharm/Held + pockets; tm_sets;
validators (TM refs, night-table law, obstacle bounds). World systems:
1200-tick clock with night tables and phase events; Mute Charm; UseItem
(potions, vitamins with caps, reusable TMs with slot picks, stones);
friendship per v1.5 #5 with stable party_index mapping; all four
evolution methods; weather zones (open + refresh per v1.6 #6);
performances ×6 (brush/rock/boulder-with-blocking-push/surf/fly/lumen
tint); trainer rematch (rewards once); era Shift (KO-replacement,
singles, free_switch). Screens hub: Party+Bag (pockets, item use, TM
slot flow), Repertoire (16 boxes, quick-move, last-conscious guard),
Score (measure-fill, hidden unseen), Programme, Player Card, Options
(Set/Shift, anim pacing, reduced motion, high contrast — all wired,
settings restored on Continue). Dex flags; toasts; night/dark tint.

**Gate P4:** 182 workspace tests green (ability suite 18, doubles 9,
world systems 9, T3 policy, session bridge 7 incl. a full 2v2);
doubles fuzz in properties; replay corpus extended (tandem + regen v5);
tier regression per §14 v1.7: T2 96.0% / T3 95.5% vs T0 (≥90), T3
48.6% vs T1 (≥45); validate 0/0; clippy clean both configs; windowed
boot clean; badge-run artifact re-recorded and replaying.

**Phase review:** adversarial workflow (7 lenses; the run hit the API
spend cap partway — 4 confirmed findings delivered, the remaining
suspects were triaged and fixed inline by hand): invisible Shift flow
(now fully rendered + KO-gated + singles-only), silent trainer-battle
Run (now rejected per v1.4 #3), night tint persisting over battles
(despawn + live alpha + high-contrast), Fainted position-vs-party-index
mapping (stable field added), settings never restored on Continue,
doubles item gating, bag cursor clamp, boulder pushes vanishing,
rematch item dupes, evolution dex gaps, badge_count restore. Rulings:
doc 02 v1.8 (5 entries).

**Deviations (recorded):** no doubles/cracked-rock/water/badge-6
CONTENT exists yet (engine + session + tests prove the systems; content
arrives with P5+ maps); the windowed presenter cannot yet drive doubles
targeting (no doubles content to drive — UI lands with the first
doubles trainer in P5); item/friendship/Duet-Stone evolutions have no
content users yet (tests synthesize); boxes are a flat Vec paged ×30
("16 boxes" is the pager, capacity unbounded); bag pockets are
grouping tags, not separate tabs.

**Next:** P5 — `git tag p5-start`; Act 1 content: Badges 1–3, routes
2–6, TACET arc opening, Arbor Vale + Mirelle (Hall 2), doubles trainer
content, region map screen.


### 2026-06-10 — P3 complete (vertical slice: first playable)

**Built:** Region pack layer (Motif/Trainer/Item/RegionDef schemas, loaders,
doc 04 §3 validation incl. warp-graph BFS, evolution acyclicity, level-legal
trainer movesets, strings rules 1/8, script side-effect refs). 20-motif
Cantorel batch (3 starter lines ×3 + 11 commons; designed by a 4-agent
workflow, cross-checked, transcribed with per-line shared cry seeds).
37 region moves. `tools assets`: deterministic sigils (polar waveforms from
the SAME melody as the cry; bilateral/radial/broken symmetry; eyes rule) +
fundsp WAV cries (timbre per type, 0.6–1.2 s law, leitmotif ornaments per
stage); 4 pipeline tests. Pure session core: Individual↔BattleMote bridge,
wild/trainer battles driven by Inputs, catching (bells consume, trainer
battles reject per 02 v1.4 #3), payouts/defeat flags, learn/evolve prompts,
shops, whiteout per 02 §15+v1.4 #5. SaveFile v2 (heal_point; truthful
region/badge header; v1 fixture migrates). Maps: Pausa Village, Reed's
studio, Route 1, Prelude Town, Hall 1 S-path; full act-1-beat-1/2 scripts
(~170 strings, doc 01 §9 voice); 10 trainers (rival counterpicks by starter
flag; Dario T2). Bevy: title + save select (Continue/New Song), battle
presenter (sigil sprites, HP plates, type-tinted move grid, paced skippable
messages, prompts, post-battle autosave), windowed mart, party screen with
staff-chart-lite + box list, dialogue through the string table.

**Gate P3:**
- Headless replay: `new_game_to_first_badge.ron` (930+ inputs, recorded by
  the adaptive driver) — starter → rival → catch → grind → mart → hall →
  badge.1 + performance.lumen_hum; save→reload leg asserts party levels +
  money; both tests green in `cargo test` (runs in CI; latest run green).
- `validate`: 0 errors 0 warnings (now incl. strings + script refs).
- `simulate` bands (800 battles/level, per archetype): early 39.9–62.5%,
  mid 42.5–56.5%, final 49.5–50.0% at L15/30/50 — all inside 35–65;
  T2 vs T0 96.2% (gate ≥90). Fresh-player wall-clock: the recorded run is
  ~25 min of real play (930 inputs incl. grind), zero crashes; windowed
  boot verified.
- Workspace: 142 tests green; clippy clean (windowed + headless).

**Phase review:** adversarial workflow (8 lenses, 81 agents): 72 confirmed
findings — 2 critical (rival once-flag burned pre-battle; learn/evolve
prompt deadlock), determinism/save (heal_point not persisted → SaveFile v2;
fake badge header; no Game-world reload leg), strings cluster (slice
rendered raw keys; validator silent), bell/item legality, evolution free
heal, windowed marts missing, asset-law gaps. All fixed; doc 02 v1.4 (5
rulings) + docs 03/04 trued. 1 finding refuted (route trainer levels — doc
04 §4's curve anchors post-badge gaps). `/review` follow-up on the fix wave
itself: empty-stock shop cursor clamp panic (fixed). CodeRabbit retired
from the protocol per user instruction — phase reviews are `/review` + the
adversarial workflow from here on.

**Deviations (recorded, not hidden):** evolution presentation is a prompt +
message, not a scene; Summary staff chart is the text-glyph lite version;
Box is read-only overflow storage; mart stock is "all priced items"
(per-table stock with the P4 economy pass); in-battle learn replace always
takes slot 0 (move-picker UI in P4); orphan-string warn (04 §3 rule 8's
warn half) deferred; `tools atlas` deferred (03 §10). New-game seeds are
OS-entropy in the app layer only; replays/tests pin seeds.

**Next:** P4 — `git tag p4-start`; systems complete: abilities, held items,
day/night clock, full Box UI, TMs, breeding-lite, weather, the remaining
move effects, per-table marts, Repertoire/dex screens.
 (append newest on top — this is the session memory)

> Template:
> `### YYYY-MM-DD — Phase Px`
> `Done: …` / `Gate evidence: …` / `Next: …` / `Open questions: …`

### 2026-06-10 — Phase P2 (complete)

**Done:** Overworld & UI shell, with the battle crate's purity discipline
extended to the overworld: `game::world` is an engine-free core (grid
movement with tap-to-turn per v1.3 #1, collision/NPC blocking, warp and
script triggers with once-flags, §12 encounter rolls, line-of-sight
engagement v1.3 #3, NPC wander with pure pause rules, the script
interpreter incl. choice lists, save snapshot/restore) and the Bevy
layer only renders it. script crate: pure frame-stack interpreter, doc
04 §2 fisher script as a test. save crate: SaveFile v1, FsBackend
(fsync + rename) / MemBackend behind SaveBackend, committed v1 wire
fixture, version-peek migrations. data: MapDef/Palette schemas, dev
debug maps (rehearsal + annex with greeter/stroller/sentry), placement +
weight-law validation; tools validate parses every script. tools
importmap compiles LDtk JSON (IntGrid layers + Warp/Script/Npc
entities, y-flip, dimension/bounds checks). Bevy app: colored-quad
tilemap, walk interpolation + one-step buffer, camera clamp, parchment
dialogue box with staff lines + choice cursor, measure-bar wipe, pause
menu with slot-1 save + settings panel, autosave on warp/post-battle,
encounter placeholder doubling as the doc 05 §5 battle-layout spike.

**Gate P2 evidence:**
- Headless replay `walk_talk_warp_save.ron` passes: spawn → greeter
  chat (2 lines, met.greeter) → warp to the annex → save → MemBackend
  reload → map/position/flags re-asserted. Runs as `cargo test -p game`
  and via `cargo run -p game --features headless -- --replay …`
  ("replay ok: 2 dialogue lines, 1 warps, 1 saves").
- 129 workspace tests green; clippy clean in windowed AND headless
  configs; `tools validate` 0 errors incl. maps/scripts/palette.
- Manual: windowed boot renders the debug yard at 480×270×2, walking,
  dialogue, warp wipe, menu save, encounter scene all live (visual
  60 fps feel check done on-machine; no panics in the boot log).

**Phase review:** CodeRabbit (2 findings) + 49-agent adversarial
workflow (~25 confirmed / 9 refuted). Real bugs fixed: Choice scripts
panicked the pure core; the wander-pause rule lived only in the
renderer (windowed vs replay rng divergence); post-battle player-sprite
desync; warp glitch frame; vacuous encounter-determinism test; LoS
engage box was silently unshipped — now implemented with tests. All
rulings in doc 02 v1.3. Refuted findings documented in the workflow
output (9, incl. once-flag timing and rng-rewind-on-reload complaints).

**Next:** P3 — `git tag p3-start`; vertical slice content (20 motifs,
Pausa/Route 1/Prelude/Hall 1), battle presenter on the event stream,
party/summary/bag UI, title screen + save select.

**Open questions:** none.

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
