# 00 — VISION

## Logline

**Undersong** is a classic monster-taming RPG — the Gen 3–5 loop you grew up with —
where the loop itself turns out to be the villain's machine. You collect, you badge,
you climb. Then you learn what the league is *for*, and the last third of the game
asks you to choose what to do about it. The anti-villain is right about the problem
and catastrophic about the solution; the institution fighting her is monstrous for
defensible reasons. There is a third way, and it's built out of the one thing you've
been doing the entire game: forming bonds.

## The world in five sentences

The world hums with a continuous, sustaining music called the **Undersong**.
Creatures called **Motes** are knots of that song given body — each species is a
recurring **Motif**, each individual a slightly different performance of it.
Centuries ago the song broke off mid-phrase — the **Caesura** — and the world began
dissolving into silence, until one trainer bound the broken passage into herself and
became the **Held Note**: a single sustained human voice bridging the gap.
Held Notes wear out, so the **Conservatory** (publicly: the beloved league that runs
the eight-Hall badge circuit) secretly uses the circuit as a sieve — the whole
journey exists to find the rare child whose bond-resonance is strong enough to be
the next one. **TACET**, the "villain" team, is led by the previous winner, who
found out at the top, refused, and is now trying to cut the Held Note loose entirely —
freeing the prisoner by gambling the world.

## Design pillars

1. **The formula, then the knife.** Honor the Gen 3–5 loop sincerely — starter, rival,
   grass, badges, evil team, league. No irony. The subversion lands *because* the
   first half is played straight.
2. **Everything is data.** The engine knows nothing about Cantorel. Regions, species,
   moves, trainers, maps, dialogue are content packs. "All regions" is an
   architecture property, not a heroic content grind.
3. **Determinism is sacred.** Battles are a pure, seeded simulation. Replays are
   first-class. Thousands of battles per second in CI. Balance is measured, not felt.
4. **Music is mechanics, not garnish.** Creature cries are procedurally synthesized
   melodic phrases (per-species seed), HP bars are living waveforms, badge cases are
   score sheets, the Dex is literally a musical Score you're transcribing — and
   completing it is how you unlock the true ending.
5. **Respect player time.** Modern QoL on a retro chassis: fast text, no HM move-slot
   tax (field skills are party-wide "Performances"), reusable TMs, autosave slots,
   set-mode option, instant PC access from menu post-Badge-3.

## What "all regions" means here

One world, multiple regions, shipped as **content packs**:

| Region | Musical identity | Role |
|---|---|---|
| **Cantorel** | Orchestral / classical | Main game: 8 Halls, TACET arc, three endings |
| **Skalden** | Folk / nordic / skaldic | Post-game pack 1 (G/S→Kanto style: leaner, harder) |
| **Tamburra** | Percussion / desert | Pack 2 |
| **Neonata** | Electronic / neon | Pack 3 — and the hook for the sequel-threat, *Static* |

Cantorel is scoped like one classic game (8 badges, ~120 species, ~25 maps).
Skalden onward only exist as packs once the pack format has proven itself — the
roadmap's P8 gate is literally "Region 2 boots with zero engine changes."
True-ending lore reason regions unlock: the Chorus stabilizes the song, borders open.

## Scope honesty

A full classic-gen game is enormous (FireRed: ~150 species, ~350 moves, ~100 trainers,
dozens of maps). The plan handles this three ways:

- **Phased roadmap with hard gates** (`06-ROADMAP.md`) — vertical slice first, always
  playable, never a big-bang.
- **12 types instead of 18** — denser, more meaningful coverage with a 120-species dex,
  and a tractable chart to balance. One signature original type (**Resonant**).
- **Generative content with measured balance** — species/moves are template-driven
  data; the `tools` balance simulator Monte-Carlos thousands of battles to validate
  every batch before it ships.

## Art & audio honesty

Claude Code cannot draw a charming pixel fox. So the *canonical* launch art style is
**sigils**: Motes are crystallized sound, rendered as procedurally generated
geometric/waveform glyph creatures (deterministic from species seed — see
`04-CONTENT-PIPELINE.md`). This is a coherent aesthetic, not a placeholder apology —
and every species has a defined upgrade path to hand-drawn pixel art (Aseprite →
atlas) that replaces the sigil without touching data. Cries are synthesized melodic
phrases (fundsp, offline-rendered to OGG), which is both achievable in pure code and
the most on-theme audio identity possible.

## Platform & stack (summary — full rationale in 03)

Rust workspace, **Bevy 0.18** for the client, pure-Rust `battle` crate for the sim,
RON content, local file saves. Native desktop first; WASM web build at P7 (Bevy
supports both from one codebase). **No database in the core game** — single-player
saves are files. Postgres enters only if/when the optional online companion service
("Chorus Network": cloud saves / trading / sim ladder) is built — see Roadmap §Icebox.

## Naming note

"Undersong", region names, and all creature names are working titles. Do a trademark /
collision search before any public release. Everything is grep-renameable by design
(names live in string tables, not code).
