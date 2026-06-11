# Pack-format retrospective (P8)

The thesis: adding a region must require **no engine changes**. This
document records every place the engine had to be touched to make that
true — i.e., the P8 engine-prep commit — so the next pack (Tamburra,
Neonata) genuinely ships as `content/` + `assets/` only.

## What region-adding used to touch (now fixed)

| # | Touch point | Fix (engine-prep) |
|---|---|---|
| 1 | `load_game_world` hardcoded `"cantorel"` | Loads the union of every pack under `content/regions/`; the primary (entry) region is cantorel when present, else the first alphabetically |
| 2 | `Registry` was built from exactly one pack | `Registry::extend_with_pack` folds further packs in (species, moves, trainers, evolutions, TM sets); first-pack-wins on id collision |
| 3 | Warp validation rejected cross-pack targets | `validate_maps`/`validate_region` accept targets that exist in *another* pack (`external_maps` union, collected in a first pass) |
| 4 | Region music ids lived in `tools music`'s track list | Skalden's five stems pre-generated in the prep commit; **remaining wrinkle:** a future pack with NEW track ids still edits `tools` — track lists should one day move into pack manifests (Icebox) |
| 5 | The completability proof would be a new test file | `skalden_run.rs` predates the pack and self-skips while it's absent — the pack's arrival activates it without an engine diff |

## What a new pack provides (no engine edits)

- `content/regions/<id>/region.ron` — id, name key, starters, dex order,
  entry map/spawn (entry only used if primary).
- `motifs/`, `moves/` (optional), `trainers/`, `maps/<id>/{map.ron,scripts/}`,
  `dialogue/strings.ron`.
- Encounter tables, weather/night/dark per map, music by existing track id.
- Cross-region doors: plain `Warp` triggers at both ends (validated
  against the union).
- Assets: `tools assets --region <id>` emits sigils + cries into
  `assets/` from the pack's own seeds.

## Enforcement

`scripts/check-pack-purity.sh <from> <to>` fails when a pack range
touches anything outside `content/` + `assets/`. The release CI runs it
between the `skalden-pack-start` and `skalden-pack-end` tags.
