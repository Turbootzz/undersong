# UNDERSONG — CLAUDE.md (bootloader)

You are working on **Undersong**, an original monster-taming RPG in the structural
tradition of the Gen 3–5 era (FireRed / Emerald / Platinum / B2W2): grid overworld,
turn-based battles, 8 badges, an evil-team arc, a league, post-game regions.
Original IP — original creatures, world, names, story. No Nintendo/Game Freak assets,
names, or sprites, ever. Mechanics are inspired; expression is ours.

This file is a bootloader. It stays short. Durable knowledge lives in `docs/`.

## Read order (first session, and whenever lost)

1. `docs/00-VISION.md` — what we're building, pillars, scope phases
2. `docs/06-ROADMAP.md` — current phase, task checkboxes, acceptance gates ← **your todo list**
3. `docs/03-ARCHITECTURE.md` — workspace layout, crate boundaries, determinism rules
4. `docs/02-GAME-DESIGN.md` — exact mechanics & formulas. **This file is law.** Never invent or "remember" a formula; if it's not specified, propose it in the doc first, then implement.
5. `docs/04-CONTENT-PIPELINE.md` — data schemas, region packs, validation
6. `docs/01-STORY.md` — narrative bible (needed for dialogue/cutscene content)
7. `docs/05-UI-STYLE.md` — visual identity (needed for any UI work)

## Golden rules

1. **Determinism is sacred.** All battle logic lives in the `battle` crate: pure
   functions, seeded RNG, zero Bevy deps, zero I/O, zero wall-clock. If you can't
   unit-test it headlessly, it's in the wrong crate.
2. **Content is data, not code.** Species, moves, maps, trainers, dialogue, encounters
   live in `content/` as RON. Adding a region must require no engine changes.
3. **Pinned versions.** `bevy = "0.18"` (0.18.1 at project start). Your training data
   likely predates this — when an API doesn't compile, check
   https://docs.rs/bevy/0.18.1 and the official migration guides before guessing.
   Never bump bevy or add a dependency without recording it in
   `docs/03-ARCHITECTURE.md` §Dependencies, with one line of justification.
4. **Gates before progress.** A roadmap phase is done only when (a) its acceptance
   gate in `docs/06-ROADMAP.md` passes AND (b) the **phase review** is clean: run the
   built-in `/code-review` skill at high effort over the *entire* phase diff
   (everything since the `pN-start` git tag — see Roadmap §Phase protocol), fix every
   finding, re-run until clean. Paste gate + review results into the STATUS block,
   then tick the boxes.
5. **The validator is part of the build.** `cargo run -p tools -- validate` must pass
   before every commit that touches `content/`.
6. **Names in code are mechanical; names in UI are flavored.** Code says `iv`, `ev`,
   `burn`; the UI says Timbre, Practice, Scorched. String tables map between them.
   Never let flavor names leak into code identifiers.
7. **Don't gold-plate.** Implement what the current phase asks. Park ideas in
   `docs/06-ROADMAP.md` §Icebox instead of building them.

## Commands

```bash
cargo build --workspace                 # build everything
cargo test  --workspace                 # all tests (battle sim, save, validators)
cargo run -p tools -- validate          # validate all content packs
cargo run -p tools -- simulate --battles 1000   # balance Monte Carlo
cargo run -p game                       # run the game (dev profile)
cargo run -p game --features headless -- --replay tests/replays/<name>.ron
```

## Session protocol (durable memory)

At the end of every working session, update the `## STATUS` block at the bottom of
`docs/06-ROADMAP.md`: date, what was completed, gate results, next action, open
questions. The next session (you, with no memory) starts by reading it.
Commit style: conventional commits (`feat(battle): …`, `fix(save): …`,
`content(cantorel): …`, `docs: …`). Small commits, always green.

## Current phase pointer

→ See `docs/06-ROADMAP.md` §STATUS. If empty: you are at **P0 — Bootstrap**.
