# 02 — GAME DESIGN (mechanics law)

> Rule zero: **nothing in this file is implemented from memory of other games.**
> These are *our* formulas. Where they match Gen 3–5 conventions, that's intentional;
> where they deviate, deviations are listed in §13. If a needed rule is missing,
> add it here first (PR-style note in the changelog at the bottom), then implement.
> Code uses mechanical names (`iv`, `burn`); UI uses flavor names (Timbre, Scorched)
> via string tables.

## 1. Types (12) and the chart

`feral, ember, tide, bloom, volt, gale, stone, frost, venom, phantom, alloy, resonant`

Stone = rock+ground merged. Resonant is the rare signature type (sound/Undersong);
story creatures and sound-flagged specialists.

**Attacking type → 2× / ½× / 0× (defending types):**

| Attacker | 2× vs | ½× vs | 0× vs |
|---|---|---|---|
| feral | — | stone, alloy | phantom |
| ember | bloom, frost, alloy | ember, tide, stone | — |
| tide | ember, stone, alloy | tide, bloom | — |
| bloom | tide, stone | bloom, ember, gale, venom | — |
| volt | tide, gale | volt, bloom | stone |
| gale | bloom, venom | volt, stone, alloy | — |
| stone | ember, volt, gale, frost, venom | bloom, alloy | — |
| frost | bloom, gale, stone | frost, tide, alloy, ember | — |
| venom | bloom, resonant | stone, venom, phantom | alloy |
| phantom | phantom, resonant | — | feral |
| alloy | frost, stone | ember, tide, volt, alloy | — |
| resonant | phantom, frost | resonant, venom | — |

Derived defensive weakness counts (sanity): feral 0 (imm. phantom) · ember 2 ·
tide 2 · bloom 4 · volt 1 (imm. n/a) · gale 3 · stone 4 (imm. volt) · frost 4 ·
venom 2 · phantom 2 (imm. feral) · alloy 2 (imm. venom) · resonant 2.
*(frost corrected 3→4 and alloy's venom immunity annotated, 2026-06-10 — see
Changelog; the table above is the source of truth.)*

The chart ships as data (`content/core/typechart.ron`). The balance simulator
(see 06, gate P1) must report: per-type offensive coverage, defensive profile, and
usage spread across generated species; tune via data, never via code.

## 2. Creature data model

```
Species (Motif): id, name_key, types[1..2], base_stats{hp,atk,def,spa,spd,spe},
  abilities[1..2] + hidden?, growth_curve, catch_rate(3..255), base_exp_yield,
  ev_yield{stat:1..3}, learnset[(level, move_id)], tm_set[move_id],
  evolution{method, target, param}, cry_seed, sigil_seed, dex{height,weight,entry_key},
  tags[performer.*, habitat.*]
Individual (Mote): species_id, level, exp, ivs{0..31 ×6}, evs{0..252 ×6, Σ≤510},
  nature(0..24), ability_slot, moves[≤4]{id, pp, pp_ups}, status, held_item?,
  friendship(0..255), keyshifted(bool, P[base]=1/4096), ot, nickname?
```

## 3. Stat math

- `HP    = floor((2·Base + IV + floor(EV/4)) · L / 100) + L + 10`
- `Other = floor((floor((2·Base + IV + floor(EV/4)) · L / 100) + 5) · NatureMod)`
- Nature: index `n ∈ 0..24`; boosted stat = `n / 5`, hindered = `n % 5`, over order
  `[atk, def, spa, spd, spe]`; ×1.10 / ×0.90; neutral when equal. UI names
  ("Temperaments") — 5×5 grid, diagonal neutral:

|        | −atk | −def | −spa | −spd | −spe |
|--------|------|------|------|------|------|
| **+atk** | *Marcato* | Forte | Bravura | Sforzando | Pesante |
| **+def** | Tenuto | *Fermo* | Solido | Grave | Largo |
| **+spa** | Brillante | Estro | *Lucido* | Acuto | Rubato |
| **+spd** | Placido | Sereno | Velato | *Calmo* | Adagio |
| **+spe** | Vivace | Presto | Agile | Scherzo | *Moderato* |

- **Stat stages** −6..+6: multiplier `max(2, 2+s) / max(2, 2−s)`.
  Accuracy/evasion stages: `max(3, 3+s) / max(3, 3−s)`.

## 4. Damage pipeline (exact, in this order)

```
base   = floor(floor(floor(2·Level/5 + 2) · Power · A/D) / 50) + 2
× spread      (0.75 if a doubles move hit ≥2 targets, else 1)
× weather     (1.5 boosted / 0.5 hindered, see §7)
× crit        (2.0; crit ignores attacker's negative offensive stages and
               defender's positive defensive stages)
× rand        (uniform integer 85..100, /100)
× stab        (1.5 if move type ∈ user types)
× type1 × type2
× burn        (0.5 if user burned and move is physical)
× other       (ability/item/screen modifiers, each applied as listed in its data)
floor at each step; minimum 1 damage if type product > 0.
```

- `A/D`: physical → atk/def, special → spa/spd. **Phys/special is per-move**
  (Gen-4-style split), defined in move data.
- Crit stages: `1/16, 1/8, 1/4, 1/3, 1/2` (stage from move flags + items).
- Accuracy check: `P(hit) = move_acc × accStage(user) / evaStage(target)`,
  clamped to ≤ 100%. `acc = 0` in data means "never misses".

## 5. Status (UI flavor name in parentheses)

| Status | Effect | Immunity |
|---|---|---|
| burn (Scorched) | atk halved (in pipeline), 1/16 max HP at end of turn | ember |
| poison (Soured) | 1/8 max HP end of turn; **toxic** variant n/16, n+1 each turn, resets on switch | venom, alloy |
| paralysis (Detuned) | speed ×0.25; 25% full stop per action | volt |
| sleep (Lulled) | 1–3 turns rolled on apply; decrements on the sleeper's action; persists through switch | — |
| freeze (Frosted) | can't act; 20% thaw at action; thawed when hit by ember move | frost |

Volatile: confusion (1–4 turns, 33% chance to hit self: 40 BP typeless physical),
flinch (one action), seeded (1/8 drained to opposer end of turn), trapped (no
switch), charging / semi-invulnerable (two-turn moves). One major status at a time;
volatiles stack.

## 6. Moves

```
Move: id, name_key, type, category(physical|special|status), power(0=n/a),
  accuracy(1..100|0=sure), pp(5..40), priority(-7..+5), target(enum),
  flags{contact, sound, protectable, reflectable, punch, bite},
  effects: [Effect]    // ordered list, executed after damage
Effect: Damage | StatStage{target, stat, delta, chance} | Status{ailment, chance}
  | Heal{frac} | Drain{frac_of_dealt} | Recoil{frac_of_dealt} | MultiHit{2..5}
  | TwoTurn{charge_text} | Protect | Weather{kind} | Flinch{chance}
  | ForceSwitch | SelfSwitch | OHKO | FixedDamage{amount|level}
```

MultiHit count distribution: 2 (3/8), 3 (3/8), 4 (1/8), 5 (1/8).
Sound-flagged moves: ignore substitutes (later) and interact with Damper/Amplify
abilities; thematic backbone of the resonant type.

**Launch set: 60 moves.** 18 canon examples (rest generated to the same budget —
see 04 §generation):

| id | name | type | cat | pow | acc | pp | prio | effects/flags |
|---|---|---|---|---|---|---|---|---|
| tackle | Tackle | feral | phys | 40 | 100 | 35 | 0 | contact |
| quick_step | Quick Step | feral | phys | 40 | 100 | 30 | +1 | contact |
| ember_note | Ember Note | ember | spec | 40 | 100 | 25 | 0 | Status{burn,10%}, sound |
| flare_brass | Flare Brass | ember | spec | 90 | 100 | 15 | 0 | Status{burn,10%}, sound |
| ripple | Ripple | tide | spec | 40 | 100 | 25 | 0 | — |
| undertow | Undertow | tide | spec | 80 | 100 | 15 | 0 | StatStage{tgt,spe,−1,20%} |
| leaf_pick | Leaf Pick | bloom | phys | 55 | 95 | 25 | 0 | high crit (stage+1), contact |
| root_chord | Root Chord | bloom | spec | 75 | 100 | 10 | 0 | Drain{1/2} |
| volt_pluck | Volt Pluck | volt | spec | 65 | 100 | 20 | 0 | Status{paralysis,10%} |
| gale_riff | Gale Riff | gale | spec | 60 | 100 | 20 | 0 | sound |
| stone_toll | Stone Toll | stone | phys | 75 | 90 | 15 | 0 | — |
| venom_trill | Venom Trill | venom | spec | 65 | 100 | 20 | 0 | Status{poison,30%}, sound |
| phantom_rest | Phantom Rest | phantom | spec | 80 | 100 | 15 | 0 | — |
| alloy_clang | Alloy Clang | alloy | phys | 80 | 100 | 15 | 0 | StatStage{self,def,+1,10%}, sound |
| frost_lull | Frost Lull | frost | status | — | 75 | 10 | 0 | Status{sleep,100%}, sound |
| resonate | Resonate | resonant | spec | 85 | 100 | 10 | 0 | sound; ignores eva stages |
| crescendo | Crescendo | resonant | status | — | 0 | 20 | 0 | StatStage{self,spa,+1}+{self,spe,+1} |
| dampen | Dampen | feral | status | — | 100 | 20 | 0 | StatStage{tgt,atk,−1}, sound |

Power budgets for generation: early STAB 40–55, mid 60–80, late 85–110 with a
drawback (recoil/acc/charge). Status moves: strong effects get ≤ 75 acc or ≤ 10 pp.

## 7. Weather (4)

| Weather | Set by | Effects (5 turns) |
|---|---|---|
| heatwave | move/ability | ember ×1.5, tide ×0.5 |
| downpour | " | tide ×1.5, ember ×0.5 |
| flurry | " | frost-types +50% spd? **no** — frost moves never miss; 1/16 chip to non-frost/alloy/stone |
| dustchord | " | stone-types spd ×1.0 but spa-def ×1.5; 1/16 chip to non-stone/alloy |

## 8. Catching (Attunement)

```
a = floor((3·MaxHP − 2·CurHP) · catch_rate · bell_mod · status_mod / (3·MaxHP))
if a ≥ 255 → caught immediately
b = floor(1048560 / floor(sqrt(floor(sqrt(16711680 / a)))))
four checks: caught iff rand_u16() < b for all four
```

UI: the Fermata rings once per passed check (3 audible pulses), then "settles" on the
4th (resolved chord) — or shatters out with a dissonant interval on the failed check.
status_mod: sleep/freeze ×2.0; burn/poison/paralysis ×1.5; none ×1.0.

| Bell | mod | Notes |
|---|---|---|
| Fermata | ×1.0 | base, 200₵ |
| Grand Fermata | ×1.5 | 600₵, Badge 3+ |
| Maestro Fermata | ×2.0 | 1200₵, Badge 6+ |
| Overture Bell | ×4.0 on turn 1, else ×1 | "openings matter" |
| Cradle Bell | ×3.5 if target lulled/frosted | — |
| Vesper Bell | ×3.5 at night | — |
| Coda | always succeeds | one per save, post-game |

## 9. Experience & growth

- Gain: `ΔExp = floor(b · L_defeated / 7) · 1.5(trainer battle) · 1.5(traded)`,
  split evenly among participants; Exp Share item (Badge 4 reward) gives
  non-participants 50% without splitting.
- Curves (total exp to level n): medium_fast `n³`, fast `0.8n³`, slow `1.25n³`,
  medium_slow `1.2n³ − 15n² + 100n − 140`.
- Evolution methods at launch: `level(n)`, `item(id)`, `friendship(≥220)`,
  `trade-flagged → level-up alt` (no real trading yet: "Duet Stone" item substitutes).

## 10. Abilities (launch 24)

| id | Effect |
|---|---|
| crescendo_ember/tide/bloom | at HP ≤ 1/3, own-type moves ×1.5 (one per starter line) |
| dissonance | on entry: foes' atk −1 |
| amplify | sound moves ×1.3 |
| damper | immune to sound moves |
| floating | immune to stone moves |
| live_wire | 30% paralysis on being contacted |
| thorn_coat | 1/8 recoil to contacters |
| heat_haze / rain_caller / flurry_caller / dust_caller | sets weather on entry |
| perfect_pitch | accuracy can't be lowered; ignores target evasion stages |
| stage_fright | +1 spe on first entry each battle |
| thick_hide | cannot be struck critically |
| metronome_soul | speed can't be lowered (incl. paralysis penalty) |
| understudy | stats +1 atk/spa when an ally faints (doubles) |
| encore_heart | restores 1/16 HP each turn in any weather |
| keysmith | ×2 catch-assist: foe flinch chance +10% on sound moves |
| vigor | immune to sleep |
| iron_ear | immune to flinch |
| soloist | ×1.3 damage in singles, ×0.9 in doubles |
| chorister | ×1.2 damage in doubles |
| tuning_fork | normal-effect feral moves become resonant type, ×1.2 |

## 11. Field Performances (HM replacement)

Party-wide field skills: usable if **any** party Mote carries the tag — no move slot
cost. Unlocked by badge; performing plays that Mote's cry as the jingle seed.

| Performance | Unlocks after | Tag needed | Effect |
|---|---|---|---|
| Lumen Hum | Badge 1 | `performer.light` | light dark caves |
| Clearing Chord | Badge 2 | `performer.clear` | cut brush obstacles |
| Tunneling Bass | Badge 3 | `performer.smash` | break cracked rocks |
| Ferry Song | Badge 4 | `performer.ferry` | surf water tiles |
| Lift Motif | Badge 5 | `performer.lift` | push anchor-boulders |
| Skybridge Aria | Badge 6 | `performer.sky` | fly to visited towns |

## 12. Overworld & encounters

- Grid 16 px tiles; tile-step movement with 150 ms walk / 90 ms run interpolation;
  bike-equivalent ("Tempo Wheel", Badge 2 gift) 60 ms.
- Resonance patches (grass): per-step encounter roll, default `P = 0.12`
  (per-area in data). 12-slot table with fixed weights
  `20,20,10,10,10,10,5,5,4,4,1,1` (%); per-slot species+level range; separate
  tables for day/night and surf/fishing.
- Trainer line-of-sight: classic `!` engage, 1–5 tile range, facing-based, one-time
  flag per trainer.
- Repel item: **Mute Charm** (no wild engagement below your lead's level, 200 steps).
- Run from wild: `F = floor(A·32 / max(1,B)) + 30·attempts`, escape iff
  `rand(0..255) < F` (A = your spe, B = wild spe). Trainer battles: no running.

## 13. Deliberate deviations from the Gen 3–5 baseline (documented so nobody "fixes" them)

1. 12 types, custom chart; one new type (resonant).
2. Phys/special split is per-move (Gen 4 behavior) from day one.
3. Volt-types immune to paralysis; confusion self-hit 33% (modern QoL).
4. No HM move slots — Performances (§11).
5. TMs reusable; infinite-use.
6. Sleep counter doesn't reset on switch.
7. No breeding/eggs at launch (Icebox); no held-item knockoff moves at launch.
8. Crit = ×2.0 (era-authentic, deliberately spicy).

## 14. Trainer AI tiers

| Tier | Used by | Policy |
|---|---|---|
| 0 | early wilds/youngsters | uniform random legal move |
| 1 | route trainers | greedy max expected damage; never uses status |
| 2 | aces/admins | 1-ply: scores damage + status value + setup if safe; switches out on hard counter (type product ≥ 4 against it) |
| 3 | Maestros/Quartet/Vesper | 2-ply expectimax over (move, switch) with hand-tuned weights; sees its own team plan (scripted opener allowed) |

AI tier is per-trainer in data. The `tools simulate` command pits tiers against
each other for regression. **v1.7 calibration erratum:** the original "T3 beats
T1 ≥ 85% with equal teams" is unattainable in this engine's mirror format and
is replaced. Measured at P4 (800–1600 battles each, cantorel pool, L30 3v3
identical rosters): T1 vs T1 = 49.2% (the symmetry baseline — identical teams
and near-equal policies converge on a coin flip; every same-species duel ties
speed, so turn order itself is a 50/50 each round), T2 vs T1 = 47.8%, T3 vs
T1 = 48.6%, T2 vs T0 = 96.0%, T3 vs T0 ≥ 92.8%. Greedy max-expected-damage
(T1) is near-optimal in a 3v3 race: a matchup switch concedes a free hit that
roughly cancels the type edge it buys. The regression gates are therefore:
T2 ≥ 90% vs T0, T3 ≥ 90% vs T0, T3 ≥ 45% vs T1 (parity-noise floor), plus the
unit test proving T3 switches out of a type wall when the bench holds the
counter (its qualitative edge — scripted Maestro fights, not mirror races,
are where T3's plan-reading shows).

## 15. Economy

- Loss on defeat: half money, floor 0 (era-authentic — and post-Roster, an NPC jokes
  the Conservatory's "insurance" always collects).
- Mart tiers by badge count; potions 200/700/1200/3000₵; status heals 250₵;
  vitamins (+10 EV, cap 100) 9800₵.
- Trainer payout: `class_base × ace_level`.

## Changelog

- v1.0 — initial law (this document). All future rule changes append here with
  date + reason, and must keep `tools validate && cargo test -p battle` green.
- 2026-06-10 (P0) — annotation fix, no rule change: §1's derived defensive
  weakness sanity line said "frost 3"; the chart table (which is the law) gives
  frost **4** weaknesses (ember, stone, alloy, resonant all hit frost 2×). The
  sanity line is corrected to 4. Content ships the table as written; the
  cross-transcription test in `crates/data/tests/core_content.rs` guards it.
- v1.1, 2026-06-10 (P1) — mechanics completions. The battle sim needs rules
  this doc left unstated; per rule zero they are specified here before being
  implemented. None alters an existing rule.
  1. **Turn structure:** a turn resolves in phases:
     (a) **escape attempts** (wild only, §12 formula; a successful escape ends
     the battle, a failed one consumes that side's action),
     (b) **switches** (both sides' switches resolve before any move; if both
     switch, faster side first),
     (c) **bell use** (Attunement, wild only; consumes the action; illegal in
     trainer battles),
     (d) **moves**, ordered by priority desc → effective speed desc → tie
     broken by one rng draw (50/50),
     (e) **end of turn**.
  2. **End-of-turn tick order:** 1) weather chip damage (flurry/dustchord, in
     side order: side 0 active first), 2) seeded drain, 3) burn/poison/toxic
     damage, 4) weather counter decrement + expiry message. Faint checks run
     after each sub-step; a fainted Mote takes no further ticks.
  3. **No-PP fallback:** a Mote whose moves all have 0 PP uses **Last Resort
     Hum**: 50 power, typeless (type product 1, no STAB, normal-effect),
     physical, never misses, cannot crit, user takes recoil = floor(damage/4),
     not a sound move, infinite use. It is engine-built-in, not content.
  4. **Confusion self-hit:** 40 power, typeless, physical, computed with the
     user's own atk vs the user's own def, no crit, no STAB, no random 85–100
     roll (deterministic base damage), cannot flinch or apply effects.
  5. **Sleep counter:** rolled uniform 1–3 on application. When the sleeper
     would act: decrement first; if the counter hits 0 the Mote wakes and acts
     this turn, otherwise the action is lost ("is lulled").
  6. **Freeze:** 20% thaw check when the frozen Mote would act (thaw → act
     this turn). Being hit by an ember-type move thaws immediately.
  7. **Paralysis:** effective speed = floor(spe / 4) (the ×0.25 of §5); the
     25% full-stop check happens when the Mote would act, after sleep/freeze
     checks and before confusion.
  8. **Volatile check order at action time:** flinch → sleep → freeze →
     paralysis stop → confusion (33% self-hit replaces the action).
  9. **Toxic:** counter n starts at 1, +1 each end of turn; switching out
     resets the Mote's toxic counter to 1 (stays badly poisoned).
  10. **Crit stages:** base stage 0 (1/16); move flag `high_crit` adds +1.
      Stage indexes the §4 table; stage caps at 4 (1/2).
  11. **Stat recompute on level-up:** stats recompute from the §3 formulas at
      the new level; current HP increases by (new max − old max).
  12. **medium_slow floor:** total-exp values below 0 (levels 1–2) clamp to 0;
      every curve's total exp at level 1 is 0.
  13. **Exp participation:** a Mote participates by being active when the
      opposing Mote faints; participants split evenly (integer division,
      remainder dropped), each then ×1.5 trainer / ×1.5 traded as applicable.
  14. **EV award:** the species' ev_yield goes to every participant, capped at
      252/stat and 510 total (excess dropped stat-by-stat in §3 stat order).
  15. **AI T1 "expected damage":** full §4 pipeline with rand fixed at 92, no
      crit assumed; picks the highest-damage usable damaging move (ties → the
      lowest move slot). If it has no damaging move with PP it uses slot 0's
      legal fallback (Last Resort Hum rule applies naturally).
  16. **AI T2 scoring (1-ply):** score = expected damage (as T1, as % of
      defender's current HP, capped 100) + 25 if the move can KO + 15 for a
      major-status move against a healthy (>50% HP) un-statused foe + 10 for a
      self-stat-stage move while at full HP ("setup if safe"). Switch rule: if
      the foe's type product vs the active Mote is ≥ 4 (using the foe's best
      STAB type), and the bench has a Mote whose defensive product vs that
      type is ≤ 1, switch to the first such Mote. T2 never uses bells/escape.
  17. **Switch legality:** a switch target must be a non-fainted, non-active
      party member; a side with no legal replacement after a faint loses (a
      battle ends when one side has no conscious Motes).
  18. **Accuracy stages** apply per §3 (acc/eva 3-based table); `resonate`'s
      "ignores eva stages" flag zeroes the target's evasion stage in the §4
      accuracy formula only.
- v1.2, 2026-06-10 (P1 phase review) — rulings forced by review findings;
  each either legislates an implementation decision or pins an ambiguity.
  1. **Exp/EV awards are player-side only.** Only side 0 (the player side)
     receives exp, EVs, and level-ups from faints; `Fainted` events are
     emitted for every faint regardless. v1.1 #13's side-neutral wording is
     superseded. Opposing trainers' and wild Motes' progression never
     mutates mid-battle.
  2. **Exp order: split, then multiply.** The participant split (integer
     division) happens first; the ×1.5 trainer and ×1.5 traded multipliers
     apply to each share afterwards. Where §9's prose reads otherwise, this
     clause wins.
  3. **A fainted participant receives nothing.** If the would-be victor's
     active Mote is itself fainted at award time (recoil, etc.), the award
     is skipped entirely.
  4. **EVs take effect at the next stat recompute** (level-up per #11), not
     the instant they are awarded; no mid-battle stat bump or heal from an
     EV award.
  5. **Two-turn moves:** the charge turn spends the action and 1 PP, emits
     `MoveUsed` + `ChargeStarted`, and commits the slot. The release turn is
     forced to the committed slot regardless of the submitted action, spends
     no PP, emits no second `MoveUsed`, and strikes. Leaving the field for
     any reason cancels the charge (volatiles clear on exit, §5).
  6. **The dual-type product is one pipeline step**: `× type1 × type2` in §4
     is computed as a single exact rational (order-independent), then
     floored once — consistent with doc 03 §2's "multipliers are rational
     pairs applied in pipeline order".
  7. **End-of-turn fraction damage has a 1 HP minimum** (burn, poison,
     toxic, weather chip, seeded drain).
  8. **Confusion self-hit uses unstaged atk/def** (raw computed stats); #4's
     "own atk vs own def" is pinned to mean stage-free.
  9. **Weather secondary effects (§7) pipeline positions:** flurry's "frost
     moves never miss" skips the §4 accuracy roll entirely for frost-type
     moves; dustchord's "spa-def ×1.5" multiplies the stone-type defender's
     effective (stage-modified) spd by 3/2 inside D for special moves.
  10. **ForceSwitch consumes the displaced side's pending action**: a Mote
      dragged in mid-turn does not act with its predecessor's queued move.
  11. **AI clarifications:** #15's candidate set excludes moves whose
      expected damage is 0 (an all-immune mover falls back to slot 0).
      #16's "major-status move" means a Status-category move carrying a
      100%-chance `Status` effect.
  12. **Confusion infliction is deferred**: the §6 Effect enum (the law)
      has no confuse variant, so no content can inflict confusion yet; the
      engine mechanics (§5 volatile, #4 self-hit) exist and are tested
      directly. An inflict effect arrives with the content phase that needs
      it, doc-first.
  13. **Content bounds (validator):** damaging-move power ≤ 250; StatStage
      deltas in −6..=+6 and nonzero; learnset levels in 1..=100.
- v1.3, 2026-06-10 (P2 phase review) — overworld rulings.
  1. **Tap-to-turn:** a Step input whose direction differs from the current
     facing only turns (no tile change); a Step in the faced direction
     moves. Held keys in the windowed game map to repeated Steps, so a hold
     turns on its first frame and walks from the second — era-authentic.
  2. **Encounter table weights are law, not convention:** the §12 weight
     sequence 20,20,10,10,10,10,5,5,4,4,1,1 is validated as a multiset;
     `patch_rate_pct` must be 1..=100.
  3. **Trainer line-of-sight engage (§12):** after every player step, each
     NPC with `sight_range > 0` whose facing points at the player along a
     straight, solid-free line within range engages once: the
     `engaged.<map>.<npc>` flag is set, the NPC's script runs. Wild-area
     NPCs with range 0 never engage.
  4. **NPC wander pauses during dialogue and pending encounters** — a
     property of the world core, not the renderer.
  5. **Wandering NPCs never end a stroll on a trigger tile.**

### v1.4 — 2026-06-10 (P3 phase review rulings)

P3 review rulings; each was unspecified or contradicted by code. Doc is law.

1. **Evolution preserves condition.** Evolving never heals: current HP
   and status carry over unchanged; the higher max HP from the new
   species' bases applies immediately (current HP is clamped only if it
   somehow exceeds the new max). No free heal on evolution.
2. **Evolution prompts trigger on level-up only.** The eligibility check
   runs when a Mote levels (or when a stone/trade-equivalent is used),
   not on every battle end and not for newly caught Motes at or above
   the threshold level. Refusing keeps the Mote unevolved until the
   next level-up re-prompts.
3. **Bells in trainer battles are blocked at selection.** The bell is
   not consumed, no turn passes; the UI explains ("the Maestro's score
   forbids it"). Likewise, ringing with no bells in the bag, or using a
   healing item with an empty bag or at full HP, rejects the action
   without consuming the turn.
4. **Trainer payout ace = highest level in the party** (doc §15's
   "ace's level", made precise).
5. **Whiteout clears modal state.** Loss wipes any suspended script,
   dialogue, or open shop along with the warp to the rest point; defeat
   re-walks the script from its trigger, so one-time fights must guard
   with their defeat flag rather than a fire-once trigger.

### v1.5 — 2026-06-10 (P4 system specs)

Specifications P4 needs that earlier sections left open. Doc is law.

1. **Held items (launch set).** Framework hooks: on-entry, pre-damage,
   post-damage, end-of-turn, on-exp. Launch items: **Oran Chime** (held:
   once per battle, when the holder first drops to ≤ 1/2 max HP, restore
   20 HP, consumed for the battle but not from inventory), **Exp Share**
   (held: §9 — non-participants holding it gain 50% unsplit). Further
   held items arrive with content phases, each legislated here first.
2. **Doubles targeting.** All launch moves are single-target in doubles;
   the actor declares a target slot. If the declared target has fainted
   by execution, the move retargets to the surviving foe slot, else
   fizzles. Allies cannot be targeted at launch. `understudy` triggers
   per ally faint; `soloist`/`chorister` read the format flag.
3. **AI T3 (2-ply expectimax).** For each of the actor's legal actions,
   assume each opposing response in turn (uniform weights), simulate one
   full turn on a cloned state with a fixed-seed probe rng (deterministic
   across runs), and score `Δ(own team HP%) − Δ(foe team HP%)` plus 10
   per foe KO and −10 per own KO. Pick the action with the highest MEAN
   score across responses (that's the "expecti" in expectimax — v1.7
   erratum: an earlier draft said "max-min", which mis-tunes into
   pathological pessimism); ties break toward the lower move slot, then
   Move over Switch.
4. **Weather setters.** Move effect `SetWeather(w)` (5 turns, replaces
   current weather, fails — turn consumed — if that weather is already
   up); on-entry abilities set the same way but never fail. The four
   caller abilities map heat_haze→heatwave, rain_caller→downpour,
   flurry_caller→flurry, dust_caller→dustchord.
5. **Friendship counter.** Starts at the species' base (70 launch-wide);
   +2 per level-up, +1 per badge won while in party, +5 per vitamin,
   −5 per faint; clamps 0–255. Friendship evolutions check ≥ 220 at
   level-up (v1.4 #2 timing).

### v1.6 — 2026-06-10 (P4 item & overworld specs)

1. **Vitamins.** Six, one per stat (Crescendo Tonic = atk, Forte Filter =
   def, Aria Drop = spa, Chorale Shell = spd, Presto Reed = spe, Vital
   Verse = hp). +10 EVs in the stat; fails (not consumed) if that stat
   already has ≥ 100 EVs or the 510 total would be exceeded. +5
   friendship (v1.5 #5).
2. **TMs are reusable** (roadmap P4 box, made law). A TM teaches its move
   to any species whose `tm_set` lists the TM's id; teaching replaces a
   chosen slot or fails politely with 4 moves and no choice. TM ids are
   `tm01`–`tm50`; the launch slice ships tm01–tm05.
3. **Mute Charm.** Bag item, 200 steps of no wild engagement when the
   lead's level exceeds the wild's (doc 02 §12); steps tick on Step
   inputs only; reapplying resets to 200.
4. **Special bells.** Overture Bell ×4 on the battle's first turn else
   ×1; Cradle Bell ×3.5 if the target is lulled (sleep) or frosted
   (freeze); Vesper Bell ×3.5 at night (overworld clock — battles seeded
   from a night overworld are night battles); Coda always succeeds, one
   per save (post-game). Conditional mods resolve at ring time.
5. **Day/night.** The overworld clock runs on steps: 1 step = 1 tick,
   1200 ticks = a full day; night is the last third. Maps may declare a
   `night_slots` encounter table (same 12-weight law) used during night.
6. **Weather zones.** A map may declare ambient weather; battles started
   there open with it active (5 turns refreshing each round the zone's
   weather — i.e. effectively permanent in-zone).

### v1.7 — 2026-06-10 (doubles implementation rulings)

1. **Replacement timing is end-of-turn** in both formats — a position
   whose Mote fainted refills from the bench when the turn ends, never
   mid-turn (keeps v1.5 #2's retarget/fizzle reachable and one policy
   across formats).
2. **EV yield in doubles**: every conscious fielded player Mote receives
   the full `ev_yield` of a fainted foe; §9's split applies to exp only.
3. **Multi-way speed ties** resolve with one rng draw per tied adjacent
   pair after a stable descending sort (pairwise, not a uniform
   shuffle); degenerates to the single coin flip in singles (v1.1 #1d).
4. **Dissonance in doubles** drops atk on both foe positions on entry.
5. **Benchless fainted positions** stay on the field as silent (fainted)
   slots; single-target moves declared at them retarget per v1.5 #2.

### v1.8 — 2026-06-11 (P4 phase-review rulings)

1. **tuning_fork converts every feral-type move** (damaging or status)
   to resonant with the ×1.2 boost — §10's "normal-effect" wording
   meant "the normal-type analog" (feral), not "moves without secondary
   effects".
2. **T3 is anchored expectimax**: the v1.5 #3 probe scoring may only
   override tier 2's analytic choice past a clear noise margin (the
   unanchored form measurably loses to T1's exact arithmetic — §14
   calibration table).
3. **Exp Share** (§9) is implemented at the engine's exp award: each
   non-fielded, conscious party Mote holding it gains 50% of the
   unsplit base share, no dilution of participants.
4. **The era Shift offer** fires only after a foe's KO replacement in
   SINGLE-format trainer battles; doubles never offer it (positions
   re-field at end of turn instead).
5. **Rematch rewards**: payouts repeat per win; `reward_items` grant
   once, on the first victory.
