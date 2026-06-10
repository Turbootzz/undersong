# 03 — ARCHITECTURE

## 0. Stack decision (recorded)

- **Rust** (edition 2024), cargo workspace.
- **Bevy 0.18** (`0.18.1` at project start) for the client only.
- **RON** for all content and saves; **serde** everywhere.
- Native desktop is the primary target; **WASM** web build is a P7 gate (same code).
- **No database.** Saves are local files. Postgres appears only in the optional
  "Chorus Network" companion service (Icebox; would be axum + sqlx + Postgres).

Why this stack for *this* project: the battle sim is the heart, and Rust lets it be a
pure, fuzzable, compiler-verified library; Bevy is 100% code (no GUI editor), which
suits an autonomous agent; the type system converts a whole class of agent mistakes
into compile errors, which are cheap feedback.

## 1. Workspace layout

```
undersong/
├── CLAUDE.md
├── docs/                       # these documents
├── Cargo.toml                  # [workspace] members, shared lints, profiles
├── crates/
│   ├── core/                   # shared vocab: ids, stats, types, rng wrapper
│   ├── battle/                 # THE pure simulation. No bevy. No io. No clock.
│   ├── data/                   # schemas, RON loaders, content registry, validation
│   ├── save/                   # save format, versioning, migrations
│   ├── script/                 # dialogue/cutscene command interpreter (pure)
│   ├── game/                   # the bevy app (binary)
│   └── tools/                  # CLI: validate | simulate | importmap | cries | sigils | atlas
├── content/
│   ├── core/                   # typechart.ron, natures.ron, items.ron, strings/en.ron
│   └── regions/cantorel/       # region pack (see 04)
├── assets/                     # runtime-loadable: atlases, ogg, fonts (generated + authored)
└── tests/replays/              # scripted input replays for headless integration tests
```

Dependency direction (enforced by review; violations are bugs):

```
core ← battle ← (game, tools)
core ← data   ← (game, tools)
core ← save   ← (game, tools)
core ← script ← (game, tools)
game depends on bevy; nothing else does.
```

## 2. The battle crate (crown jewel)

**Contract:** battle is a deterministic pure state machine.

```rust
pub fn step(state: &BattleState, actions: &TurnActions, rng: &mut BattleRng)
    -> (BattleState, Vec<BattleEvent>);
```

- `BattleRng` = `rand_chacha::ChaCha8Rng` seeded per battle; the *only* entropy
  source. `core::rng` wraps it so call sites can't reach `thread_rng`.
- No floats in game-state math. All formula arithmetic is integer (the formulas in
  doc 02 are written floor-by-floor for this reason). Multipliers are rational
  `(num, den)` pairs applied in pipeline order.
- `BattleState` is plain data: serde-serializable, cloneable, diffable.
- **Replay invariant:** `(initial_state, action_log, seed)` must reproduce the
  identical event stream forever. A golden-replay test corpus guards this; breaking
  a replay requires bumping `REPLAY_VERSION` and regenerating goldens consciously.

### Events drive everything

```rust
pub enum BattleEvent {
    TurnStarted { n: u16 },
    SwitchedIn { side, slot, mote },
    MoveUsed { side, slot, move_id, targets },
    DamageDealt { target, amount, crit: bool, effectiveness: Eff },
    StatStageChanged { target, stat, delta, new_stage },
    StatusApplied { target, status }, StatusTicked { .. },
    Fainted { target }, ExpGained { mote, amount }, LeveledUp { .. },
    AttuneAttempt { rings: u8, caught: bool },
    WeatherChanged { kind }, Message { key, args },
    BattleEnded { outcome },
}
```

The Bevy layer is a *renderer of this event stream* (queued, timed, skippable).
The CLI battle runner in `tools` renders the same stream as text. AI evaluation,
balance Monte Carlo, and fuzzing consume it headlessly. One sim, four consumers.

### Testing requirements for `battle`
- Unit tests per mechanic, written against doc 02's numbers (e.g. the damage
  pipeline test cases include hand-computed expected values).
- `proptest` properties: damage ≥ 1 when effectiveness > 0; HP never < 0; stat
  stages clamp to ±6; EV sum ≤ 510 post-any-operation; a battle always terminates
  ≤ 1000 turns under random legal play; serialization round-trips.
- Fuzz gate: 10,000 random seeded battles, zero panics, in CI.

## 3. The game crate (Bevy)

### App states

```rust
enum AppState { Boot, Title, Overworld, Battle, Menu, Dialogue, Transition }
```

One plugin per domain, each owning its systems/resources/assets:
`BootPlugin, OverworldPlugin (map render, grid movement, npc, encounters, triggers),
BattlePlugin (event-stream presenter), UiPlugin (widgets from doc 05),
DialoguePlugin (script crate runner), AudioPlugin, SavePlugin, DebugPlugin`.

### Conventions
- Grid movement: discrete tile steps; a `Moving { from, to, t }` component
  interpolates render position; logic reads only tile coords. Input buffered one
  step (era-authentic feel).
- Camera: integer-scaled virtual resolution **480×270**, nearest-neighbor, letterbox.
- Tilemap: **hand-rolled** (one mesh per layer chunk, 3 layers: ground/decor/overhang
  + collision grid + trigger grid). A Pokémon-like doesn't need a tilemap dependency,
  and avoiding plugins avoids Bevy-version churn.
- Map authoring: maps may be drawn in **LDtk or Tiled**; `tools importmap` compiles
  their JSON/TMX export into our runtime RON (`map.ron`). The engine never parses
  editor formats at runtime. (Human edits visually; agent edits RON; both converge.)
- Asset loading: custom `AssetLoader`s for our RON types; a `ContentRegistry`
  resource holds the loaded, validated pack (built by `data` crate).
- `--features headless`: swaps `DefaultPlugins` for `MinimalPlugins` + asset/scene
  logic without windowing; used by replay integration tests (§6).

### Bevy version discipline
Training-data Bevy is probably ≤ 0.16/0.17. **0.18 has API drift.** When something
doesn't compile: check https://docs.rs/bevy/0.18.1 and the 0.17→0.18 migration guide
first; do not guess from memory; do not pin to an older bevy to "fix" it.

## 4. Save system (`save` crate)

```
SaveFile v1 (RON, optionally zstd later):
  header { version, created, playtime_s, region, badge_bits, score_pct }
  player { name, money, position{map,x,y,facing}, settings }
  party  [Mote ≤ 6]
  boxes  [[Mote; 30]; 16]
  bag    { pocket → [(item_id, qty)] }
  flags  BTreeSet<String>          # namespaced per doc 01 §9
  vars   BTreeMap<String, i32>
  counters { steps, battles, attunes, … }
  rng    { world_seed }
```

- Path: `directories::ProjectDirs("com", "turboot", "undersong")/saves/slot{N}.ron`.
- Three manual slots + rotating autosave (`auto.ron`, on map change & post-battle).
- **Migrations:** every format change adds `migrate_vN_to_vN+1`; `save` crate tests
  load fixture saves from every prior version. Never break old saves.
- Human-readable on purpose during development (debugging an agent's output matters
  more than save-scumming prevention; obfuscation is Icebox).

## 5. Script crate (dialogue & cutscenes)

A tiny deterministic command interpreter — not a scripting language:

```
Cmd: Say{who,key} | Choice{key,[branch]} | Move{npc,path} | Face{npc,dir}
   | Wait{ms} | SetFlag{f} | ClearFlag{f} | If{flag,then,else} | GiveItem{id,n}
   | GiveMote{spec} | StartBattle{trainer_id} | Warp{map,x,y} | PlayCry{species}
   | Music{track,fade} | ShakeScreen | OpenShop{table} | HealParty | End
```

Scripts live in content (`*.script.ron`), referenced by map triggers/NPCs.
The interpreter is pure (`script` crate): `(state, cmd) -> (state, [SideEffectReq])`;
the game crate executes side-effect requests. This keeps cutscenes unit-testable:
the Act-2 Roster scene is literally a test asserting its flag outcomes.

## 6. Testing strategy (the stress-test backbone)

| Layer | Tool | Gate |
|---|---|---|
| battle math | unit + proptest + fuzz corpus | every CI run |
| content | `tools validate` (see 04 §validation) | every CI run |
| balance | `tools simulate --battles N` report | per content batch (P-gates) |
| scripts | `script` crate unit tests on story scenes | every CI run |
| integration | headless replays: scripted inputs from title → assert flags/position/party (e.g. `new_game_to_first_badge.ron`) | per phase gate |
| save | round-trip + cross-version fixtures | every CI run |
| perf | frame budget smoke (headless tick rate), release build | per phase gate |

CI: GitHub Actions — `fmt --check`, `clippy -D warnings`, `test --workspace`,
`tools validate`, cache `~/.cargo` + `target`. A red main is an emergency.

## 7. Dependencies (closed list — additions require a line here + reason)

| Crate | Version | Why |
|---|---|---|
| bevy | 0.18 | client engine |
| serde / ron | 1 / 0.12 | content & saves |
| rand_core / rand_chacha | 0.10 | seeded determinism; `rand` itself is deliberately NOT a dependency, so no global RNG (`thread_rng`) is even linkable from sim code |
| directories | 6 | save paths |
| proptest | 1.11 | property tests (dev-dep) |
| fundsp | 0.23 | offline cry/jingle synthesis in `tools` only |
| anyhow / thiserror | 1 / 2 | tools ergonomics / library errors |
| image | latest 0.25.x | `tools sigils` + atlas packing (tools only) |

Explicitly avoided: bevy_ecs_tilemap, bevy_ecs_ldtk, kira (for now), any scripting
language. Each would couple us to Bevy's release cadence or add an interpreter we'd
rather own. Revisit only with a Roadmap note.

## 8. Performance posture

A 2D tile game in Rust will not have performance problems; don't optimize early.
Standing rules: `[profile.release] lto = "thin", codegen-units = 1`; dev profile
`opt-level = 1` for deps (compile-time sanity); atlas everything (one draw per layer);
preload region pack at region entry; target 60 fps on integrated graphics, budget
asserted by the perf smoke test only.

## 9. WASM (P7)

`wasm32-unknown-unknown` via trunk or `bevy_cli` equivalent at that time; storage:
saves go through a `SaveBackend` trait from day one (`FsBackend` now,
`LocalStorageBackend` for web later) so P7 is a backend, not a refactor. Audio
autoplay needs a first-interaction gate on web — title screen press handles it.

## 10. Decision log (append-only)

- 2026-06: Stack locked (this doc v1). Hand-rolled tilemap over plugins. RON over
  JSON (comments + enums). Integer-only battle math. Event-stream battle rendering.
- 2026-06-10 (P0): the `crates/core` package is named **`undersong-core`** — cargo
  reserves `core` (collides with Rust's built-in crate). All other packages keep
  their doc names, so `cargo run -p game` / `cargo test -p battle` etc. match the
  docs verbatim.
