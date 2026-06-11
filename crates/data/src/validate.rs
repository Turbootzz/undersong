//! Validation rules for loaded content (doc 04 §3).
//!
//! P0 implements the rules that have data to act on: type-chart totality
//! (rule 6 — value range is already enforced by the `Eff` enum at parse
//! time) and the natures shape. The rest of the rule list lands with the
//! content kinds it validates.

use std::collections::BTreeSet;

use undersong_core::types::Type;

use crate::content::{CoreContent, NATURE_COUNT, SpeciesPool};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    /// Hard failure: CI red, commit blocked (doc 04 §3).
    Error,
    /// Report, don't fail.
    Warning,
}

#[derive(Debug, Clone)]
pub struct Finding {
    pub severity: Severity,
    pub rule: &'static str,
    pub message: String,
}

impl Finding {
    fn error(rule: &'static str, message: String) -> Self {
        Self {
            severity: Severity::Error,
            rule,
            message,
        }
    }
}

impl std::fmt::Display for Finding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let tag = match self.severity {
            Severity::Error => "ERROR",
            Severity::Warning => "WARN ",
        };
        write!(f, "{tag} [{}] {}", self.rule, self.message)
    }
}

/// Runs all `content/core` rules. Empty result = clean.
pub fn validate_core(content: &CoreContent) -> Vec<Finding> {
    let mut findings = Vec::new();
    check_typechart_total(content, &mut findings);
    check_natures(content, &mut findings);
    check_moves(content, &mut findings);
    findings
}

/// Doc 04 §3 rule 6: the chart is total over 12×12. (No entry outside
/// {0, ½, 1, 2} is possible: `Eff` has exactly those four variants, so an
/// illegal value is a parse error before validation ever runs.)
fn check_typechart_total(content: &CoreContent, findings: &mut Vec<Finding>) {
    for attacker in Type::ALL {
        let Some(row) = content.typechart.entries.get(&attacker) else {
            findings.push(Finding::error(
                "typechart.totality",
                format!("missing attacker row {attacker:?}"),
            ));
            continue;
        };
        for defender in Type::ALL {
            if !row.contains_key(&defender) {
                findings.push(Finding::error(
                    "typechart.totality",
                    format!("missing entry {attacker:?} → {defender:?}"),
                ));
            }
        }
    }
}

/// Natures: exactly 25, unique, non-empty name keys (doc 02 §3 grid).
fn check_natures(content: &CoreContent, findings: &mut Vec<Finding>) {
    let keys = &content.natures.name_keys;
    if keys.len() != NATURE_COUNT {
        findings.push(Finding::error(
            "natures.count",
            format!("expected {NATURE_COUNT} natures, found {}", keys.len()),
        ));
    }
    let mut seen = BTreeSet::new();
    for (index, key) in keys.iter().enumerate() {
        if key.is_empty() {
            findings.push(Finding::error(
                "natures.empty",
                format!("nature {index} has an empty name key"),
            ));
        }
        if !seen.insert(key.as_str()) {
            findings.push(Finding::error(
                "natures.unique",
                format!("duplicate nature name key `{key}` at index {index}"),
            ));
        }
    }
}

/// Moves: unique ids, schema ranges from doc 02 §6
/// (power 0 only for status / >0 for damaging, accuracy 0–100, pp 5–40,
/// priority −7..=+5, effect chances 1–100).
fn check_moves(content: &CoreContent, findings: &mut Vec<Finding>) {
    let mut seen = BTreeSet::new();
    for spec in &content.moves.moves {
        let id = spec.id.as_str();
        if !seen.insert(id.to_owned()) {
            findings.push(Finding::error(
                "moves.unique",
                format!("duplicate move id `{id}`"),
            ));
        }
        if matches!(
            spec.category,
            undersong_core::moves::MoveCategory::Physical
                | undersong_core::moves::MoveCategory::Special
        ) && spec.power == 0
        {
            findings.push(Finding::error(
                "moves.ranges",
                format!("`{id}` is {:?} but has power 0", spec.category),
            ));
        }
        if matches!(spec.category, undersong_core::moves::MoveCategory::Status) && spec.power != 0 {
            findings.push(Finding::error(
                "moves.ranges",
                format!("`{id}` is Status but has power {}", spec.power),
            ));
        }
        if spec.accuracy > 100 {
            findings.push(Finding::error(
                "moves.ranges",
                format!("`{id}` accuracy {} outside 0..=100", spec.accuracy),
            ));
        }
        if spec.power > 250 {
            findings.push(Finding::error(
                "moves.ranges",
                format!("`{id}` power {} above 250 (doc 02 v1.2 #13)", spec.power),
            ));
        }
        if !(5..=40).contains(&spec.pp) {
            findings.push(Finding::error(
                "moves.ranges",
                format!("`{id}` pp {} outside 5..=40", spec.pp),
            ));
        }
        if !(-7..=5).contains(&spec.priority) {
            findings.push(Finding::error(
                "moves.ranges",
                format!("`{id}` priority {} outside -7..=5", spec.priority),
            ));
        }
        for effect in &spec.effects {
            use undersong_core::moves::Effect;
            let chance = match effect {
                Effect::StatStage { chance, .. }
                | Effect::Status { chance, .. }
                | Effect::Flinch { chance } => Some(*chance),
                _ => None,
            };
            if let Some(chance) = chance
                && !(1..=100).contains(&chance)
            {
                findings.push(Finding::error(
                    "moves.ranges",
                    format!("`{id}` effect chance {chance} outside 1..=100"),
                ));
            }
            if let Effect::StatStage { delta, .. } = effect
                && (*delta == 0 || !(-6..=6).contains(delta))
            {
                findings.push(Finding::error(
                    "moves.ranges",
                    format!(
                        "`{id}` StatStage delta {delta} outside nonzero −6..=6 (doc 02 v1.2 #13)"
                    ),
                ));
            }
        }
    }
}

/// Species-pool rules (doc 04 §3 rule 4 subset that applies to a bare
/// pool file): unique ids, 1–2 types, learnset levels ascending, every
/// move reference resolves, a damaging move by level 5, ev_yield 1–3,
/// catch_rate ≥ 3.
pub fn validate_species_pool(
    pool: &crate::content::SpeciesPool,
    moves: &crate::content::MoveSet,
) -> Vec<Finding> {
    use undersong_core::moves::MoveCategory;

    let mut findings = Vec::new();
    let mut seen = BTreeSet::new();
    for spec in &pool.species {
        let id = spec.id.as_str();
        if !seen.insert(id.to_owned()) {
            findings.push(Finding::error(
                "species.unique",
                format!("duplicate species id `{id}`"),
            ));
        }
        if spec.types.is_empty() || spec.types.len() > 2 {
            findings.push(Finding::error(
                "species.types",
                format!("`{id}` has {} types (need 1–2)", spec.types.len()),
            ));
        }
        if spec.catch_rate < 3 {
            findings.push(Finding::error(
                "species.catch_rate",
                format!("`{id}` catch_rate {} below 3", spec.catch_rate),
            ));
        }
        for (stat, amount) in spec.ev_yield.iter() {
            if !(1..=3).contains(amount) {
                findings.push(Finding::error(
                    "species.ev_yield",
                    format!("`{id}` ev_yield {stat:?}={amount} outside 1..=3"),
                ));
            }
        }
        let mut last_level = 0u8;
        let mut damaging_by_5 = false;
        for (level, move_id) in &spec.learnset {
            if !(1..=100).contains(level) {
                findings.push(Finding::error(
                    "species.learnset_level",
                    format!("`{id}` learnset level {level} outside 1..=100 (doc 02 v1.2 #13)"),
                ));
            }
            if *level < last_level {
                findings.push(Finding::error(
                    "species.learnset_order",
                    format!("`{id}` learnset levels not ascending at {move_id}"),
                ));
            }
            last_level = *level;
            match moves.get(move_id) {
                None => findings.push(Finding::error(
                    "species.move_ref",
                    format!("`{id}` references unknown move `{move_id}`"),
                )),
                Some(m) => {
                    if *level <= 5 && m.power > 0 && !matches!(m.category, MoveCategory::Status) {
                        damaging_by_5 = true;
                    }
                }
            }
        }
        if !damaging_by_5 {
            findings.push(Finding::error(
                "species.damaging_by_5",
                format!("`{id}` has no damaging move by level 5 (doc 04 §3 rule 4)"),
            ));
        }
    }
    findings
}

/// Palette rules (doc 05 §2): every color is `#rrggbb`, all 12 type
/// colors present.
pub fn validate_palette(palette: &crate::content::Palette) -> Vec<Finding> {
    fn check(findings: &mut Vec<Finding>, name: &str, value: &str) {
        let ok = value.len() == 7
            && value.starts_with('#')
            && value[1..].chars().all(|c| c.is_ascii_hexdigit());
        if !ok {
            findings.push(Finding::error(
                "palette.color",
                format!("`{name}` is `{value}`, expected #rrggbb"),
            ));
        }
    }
    let mut findings = Vec::new();
    for (name, value) in [
        ("ink", &palette.ink),
        ("ink_soft", &palette.ink_soft),
        ("parchment", &palette.parchment),
        ("parchment_dim", &palette.parchment_dim),
        ("gilt", &palette.gilt),
        ("cantorel_accent", &palette.cantorel_accent),
        ("hp_high", &palette.hp_high),
        ("hp_mid", &palette.hp_mid),
        ("hp_low", &palette.hp_low),
        ("night", &palette.night),
    ] {
        check(&mut findings, name, value);
    }
    for ty in Type::ALL {
        match palette.type_colors.get(&ty) {
            Some(color) => check(&mut findings, &format!("type.{ty:?}"), color),
            None => findings.push(Finding::error(
                "palette.types",
                format!("missing type color for {ty:?}"),
            )),
        }
    }
    findings
}

/// Map rules (doc 04 §3 #2–3 subset for the loaded set): layer lengths,
/// in-bounds coordinates, warp targets exist + in-bounds + non-solid,
/// encounter table shape (12 slots, weights sum 100, levels ≥ 1, species
/// resolve), referenced scripts exist.
pub fn validate_maps(
    maps: &std::collections::BTreeMap<undersong_core::ids::MapId, crate::map::MapDef>,
    pool: &crate::content::SpeciesPool,
    script_exists: &dyn Fn(&undersong_core::ids::MapId, &str) -> bool,
    external_maps: &std::collections::BTreeSet<undersong_core::ids::MapId>,
) -> Vec<Finding> {
    use crate::map::TriggerKind;

    let mut findings = Vec::new();
    for (id, map) in maps {
        let cells = usize::try_from(map.width * map.height).expect("fits");
        let mid = id.as_str();
        for (layer, len, required) in [
            ("ground", map.ground.len(), true),
            ("decor", map.decor.len(), false),
            ("overhang", map.overhang.len(), false),
            ("collision", map.collision.len(), true),
            ("patches", map.patches.len(), false),
        ] {
            let ok = len == cells || (!required && len == 0);
            if !ok {
                findings.push(Finding::error(
                    "map.layers",
                    format!("`{mid}` layer {layer} has {len} cells, expected {cells} (or empty)"),
                ));
            }
        }
        for trigger in &map.triggers {
            let (x, y) = trigger.at;
            if !map.in_bounds(x, y) {
                findings.push(Finding::error(
                    "map.bounds",
                    format!("`{mid}` trigger at ({x},{y}) out of bounds"),
                ));
            }
            match &trigger.kind {
                TriggerKind::Warp {
                    map: target,
                    to,
                    facing: _,
                } => match maps.get(target) {
                    // Cross-region doors (P8): the target lives in
                    // another pack — bounds checked at load, not here.
                    None if external_maps.contains(target) => {}
                    None => findings.push(Finding::error(
                        "map.warp_target",
                        format!("`{mid}` warps to unknown map `{target}`"),
                    )),
                    Some(dest) => {
                        if !dest.in_bounds(to.0, to.1) {
                            findings.push(Finding::error(
                                "map.warp_target",
                                format!("`{mid}` warp lands out of bounds in `{target}`"),
                            ));
                        } else if dest.is_solid(to.0, to.1) {
                            findings.push(Finding::error(
                                "map.warp_target",
                                format!("`{mid}` warp lands on a solid tile in `{target}`"),
                            ));
                        }
                    }
                },
                TriggerKind::Script { path } => {
                    if !script_exists(id, path) {
                        findings.push(Finding::error(
                            "map.script_ref",
                            format!("`{mid}` trigger references missing script `{path}`"),
                        ));
                    }
                }
            }
        }
        for npc in &map.npcs {
            let (x, y) = npc.at;
            if !map.in_bounds(x, y) {
                findings.push(Finding::error(
                    "map.bounds",
                    format!("`{mid}` npc `{}` at ({x},{y}) out of bounds", npc.id),
                ));
            }
            if let Some(path) = &npc.script
                && !script_exists(id, path)
            {
                findings.push(Finding::error(
                    "map.script_ref",
                    format!(
                        "`{mid}` npc `{}` references missing script `{path}`",
                        npc.id
                    ),
                ));
            }
        }
        // Placement interactions: triggers and NPCs must sit on coherent
        // tiles; ids and coordinates are unique per map.
        let mut trigger_coords = BTreeSet::new();
        for trigger in &map.triggers {
            if !trigger_coords.insert(trigger.at) {
                findings.push(Finding::error(
                    "map.placement",
                    format!("`{mid}` duplicate trigger at {:?}", trigger.at),
                ));
            }
            if map.in_bounds(trigger.at.0, trigger.at.1) && map.is_solid(trigger.at.0, trigger.at.1)
            {
                findings.push(Finding::error(
                    "map.placement",
                    format!("`{mid}` trigger at {:?} sits on a solid tile", trigger.at),
                ));
            }
        }
        let mut npc_ids = BTreeSet::new();
        let mut npc_coords = BTreeSet::new();
        for npc in &map.npcs {
            if !npc_ids.insert(npc.id.as_str()) {
                findings.push(Finding::error(
                    "map.placement",
                    format!("`{mid}` duplicate npc id `{}`", npc.id),
                ));
            }
            if !npc_coords.insert(npc.at) {
                findings.push(Finding::error(
                    "map.placement",
                    format!("`{mid}` two npcs share tile {:?}", npc.at),
                ));
            }
            if map.in_bounds(npc.at.0, npc.at.1) {
                if map.is_solid(npc.at.0, npc.at.1) {
                    findings.push(Finding::error(
                        "map.placement",
                        format!("`{mid}` npc `{}` spawns on a solid tile", npc.id),
                    ));
                }
                if trigger_coords.contains(&npc.at) {
                    findings.push(Finding::error(
                        "map.placement",
                        format!("`{mid}` npc `{}` spawns on a trigger tile", npc.id),
                    ));
                }
            }
        }

        if let Some(encounters) = &map.encounters {
            findings.extend(check_encounter_table(mid, encounters, pool, "day"));
        }
        // Night tables obey the same law (doc 02 v1.6 #5).
        if let Some(encounters) = &map.night_encounters {
            findings.extend(check_encounter_table(mid, encounters, pool, "night"));
        }
        for obstacle in &map.obstacles {
            let (x, y) = obstacle.at;
            if !map.in_bounds(x, y) {
                findings.push(Finding::error(
                    "map.obstacle",
                    format!("`{mid}` obstacle at ({x}, {y}) out of bounds"),
                ));
            }
        }
    }
    findings
}

/// The 12-slot encounter-table law (doc 02 §12, v1.3 #2): exactly 12
/// slots, the fixed weight multiset, integer patch rate, sane levels,
/// known species.
fn check_encounter_table(
    mid: &str,
    encounters: &crate::map::EncounterDef,
    pool: &SpeciesPool,
    which: &str,
) -> Vec<Finding> {
    let mut findings = Vec::new();
    if encounters.slots.len() != 12 {
        findings.push(Finding::error(
            "map.encounters",
            format!(
                "`{mid}` {which} table has {} slots, doc 02 §12 requires exactly 12",
                encounters.slots.len()
            ),
        ));
    }
    let mut weights: Vec<u8> = encounters.slots.iter().map(|s| s.3).collect();
    weights.sort_unstable_by(|a, b| b.cmp(a));
    if weights != [20, 20, 10, 10, 10, 10, 5, 5, 4, 4, 1, 1] {
        findings.push(Finding::error(
            "map.encounters",
            format!(
                "`{mid}` {which} weight multiset {weights:?} differs from doc 02 §12's fixed schedule"
            ),
        ));
    }
    if !(1..=100).contains(&encounters.patch_rate_pct) {
        findings.push(Finding::error(
            "map.encounters",
            format!(
                "`{mid}` {which} patch_rate_pct {} outside 1..=100 (doc 02 v1.3 #2)",
                encounters.patch_rate_pct
            ),
        ));
    }
    for (species, lo, hi, _) in &encounters.slots {
        if *lo < 1 || hi < lo {
            findings.push(Finding::error(
                "map.encounters",
                format!("`{mid}` {which} slot `{species}` level range {lo}..{hi} invalid"),
            ));
        }
        if !pool.species.iter().any(|sp| &sp.id == species) {
            findings.push(Finding::error(
                "map.encounters",
                format!("`{mid}` {which} slot references unknown species `{species}`"),
            ));
        }
    }
    findings
}

/// Items: unique ids, sane prices, bell mods positive.
pub fn validate_items(items: &crate::region::ItemSet) -> Vec<Finding> {
    let mut findings = Vec::new();
    let mut seen = BTreeSet::new();
    for item in &items.items {
        let id = item.id.as_str();
        if !seen.insert(id.to_owned()) {
            findings.push(Finding::error(
                "items.unique",
                format!("duplicate item id `{id}`"),
            ));
        }
        if let crate::region::ItemKind::Bell { catch_mod } = &item.kind
            && (catch_mod.0 == 0 || catch_mod.1 == 0)
        {
            findings.push(Finding::error(
                "items.bell",
                format!("`{id}` bell mod {catch_mod:?} must be a positive fraction"),
            ));
        }
        if let crate::region::ItemKind::Potion { hp } = &item.kind
            && *hp == 0
        {
            findings.push(Finding::error("items.potion", format!("`{id}` heals 0 HP")));
        }
    }
    findings
}

/// TM move references resolve against a move table (core + region).
pub fn validate_item_moves(
    items: &crate::region::ItemSet,
    move_exists: &dyn Fn(&undersong_core::ids::MoveId) -> bool,
) -> Vec<Finding> {
    let mut findings = Vec::new();
    for item in &items.items {
        if let crate::region::ItemKind::Tm { move_id } = &item.kind
            && !move_exists(move_id)
        {
            findings.push(Finding::error(
                "items.tm",
                format!("`{}` teaches unknown move `{move_id}`", item.id),
            ));
        }
    }
    findings
}

/// Region rules (doc 04 §3 subset for the slice): dex/starters resolve,
/// motif species rules (via the pool checks), evolution targets exist and
/// are acyclic, trainer parties legal (1–6 members, species + moves
/// resolve, level-legal movesets), warp graph connected from the entry
/// map, encounter species in the region dex.
pub fn validate_region(
    pack: &crate::region::RegionPack,
    core: &CoreContent,
    items: &crate::region::ItemSet,
    script_warps: &[(undersong_core::ids::MapId, undersong_core::ids::MapId)],
    external_maps: &std::collections::BTreeSet<undersong_core::ids::MapId>,
) -> Vec<Finding> {
    use std::collections::VecDeque;

    let mut findings = Vec::new();
    let rid = &pack.def.id;

    let item_exists = |id: &undersong_core::ids::ItemId| items.items.iter().any(|i| &i.id == id);

    // Dex + starters resolve.
    for species in &pack.def.dex {
        if !pack.motifs.contains_key(species) {
            findings.push(Finding::error(
                "region.dex",
                format!("`{rid}` dex lists unknown motif `{species}`"),
            ));
        }
    }
    if pack.def.starters.len() != 3 {
        findings.push(Finding::error(
            "region.starters",
            format!(
                "{} starters, the trio must be exactly 3",
                pack.def.starters.len()
            ),
        ));
    }
    for starter in &pack.def.starters {
        if !pack.def.dex.contains(starter) {
            findings.push(Finding::error(
                "region.starters",
                format!("starter `{starter}` not in the dex"),
            ));
        }
    }

    // Motif rules: reuse the species-pool subset, plus evolution checks.
    let pool = crate::content::SpeciesPool {
        species: pack
            .motifs
            .values()
            .map(crate::region::Motif::spec)
            .collect(),
    };
    let mut combined_moves = core.moves.clone();
    combined_moves.moves.extend(pack.moves.iter().cloned());
    findings.extend(validate_species_pool(&pool, &combined_moves));

    for motif in pack.motifs.values() {
        let sid = motif.id.as_str();
        if let Some(evolution) = &motif.evolution {
            if !pack.motifs.contains_key(&evolution.target) {
                findings.push(Finding::error(
                    "region.evolution",
                    format!("`{sid}` evolves into unknown `{}`", evolution.target),
                ));
            }
            if let crate::region::EvolutionMethod::Item(item) = &evolution.method
                && !item_exists(item)
            {
                findings.push(Finding::error(
                    "region.evolution",
                    format!("`{sid}` evolution item `{item}` unknown"),
                ));
            }
        }
        for tm in &motif.tm_set {
            // tm_set lists TM ITEM ids (doc 04 §2); the item carries the
            // move (doc 02 v1.6 #2).
            let resolves = items.items.iter().any(|item| {
                item.id.as_str() == tm.as_str()
                    && matches!(item.kind, crate::region::ItemKind::Tm { .. })
            });
            if !resolves {
                findings.push(Finding::error(
                    "region.tm_set",
                    format!("`{sid}` tm_set references unknown TM item `{tm}`"),
                ));
            }
        }
    }
    // Evolution acyclicity (doc 04 §3 rule 4).
    for start in pack.motifs.keys() {
        let mut seen = BTreeSet::new();
        let mut current = start.clone();
        while let Some(next) = pack
            .motifs
            .get(&current)
            .and_then(|m| m.evolution.as_ref())
            .map(|e| e.target.clone())
        {
            if !seen.insert(next.clone()) {
                findings.push(Finding::error(
                    "region.evolution",
                    format!("evolution cycle reachable from `{start}`"),
                ));
                break;
            }
            current = next;
        }
    }

    // Trainers (doc 04 §3 rule 5 subset).
    for trainer in pack.trainers.values() {
        let tid = trainer.id.as_str();
        if trainer.party.is_empty() || trainer.party.len() > 6 {
            findings.push(Finding::error(
                "region.trainer",
                format!("`{tid}` party size {} outside 1..=6", trainer.party.len()),
            ));
        }
        if trainer.ai_tier > 3 {
            findings.push(Finding::error(
                "region.trainer",
                format!("`{tid}` ai_tier {} outside 0..=3", trainer.ai_tier),
            ));
        }
        for member in &trainer.party {
            let Some(motif) = pack.motifs.get(&member.species) else {
                findings.push(Finding::error(
                    "region.trainer",
                    format!("`{tid}` uses unknown species `{}`", member.species),
                ));
                continue;
            };
            if let Some(moves) = &member.moves {
                if moves.is_empty() || moves.len() > 4 {
                    findings.push(Finding::error(
                        "region.trainer",
                        format!("`{tid}` {} has {} moves", member.species, moves.len()),
                    ));
                }
                for move_id in moves {
                    let tm_taught = motif.tm_set.iter().any(|tm| {
                        items.items.iter().any(|item| {
                            item.id.as_str() == tm.as_str()
                                && matches!(
                                    &item.kind,
                                    crate::region::ItemKind::Tm { move_id: taught }
                                        if taught == move_id
                                )
                        })
                    });
                    let legal = motif
                        .learnset
                        .iter()
                        .any(|(level, id)| id == move_id && *level <= member.level)
                        || tm_taught;
                    if !legal {
                        findings.push(Finding::error(
                            "region.trainer",
                            format!(
                                "`{tid}` {} can't know `{move_id}` at level {}",
                                member.species, member.level
                            ),
                        ));
                    }
                }
            }
            if let Some(item) = &member.held_item
                && !item_exists(item)
            {
                findings.push(Finding::error(
                    "region.trainer",
                    format!("`{tid}` held item `{item}` unknown"),
                ));
            }
        }
    }

    // Maps: structural rules + dex-membership of encounters + warp graph
    // connectivity from the entry map (doc 04 §3 rule 2).
    let script_exists = |_: &undersong_core::ids::MapId, _: &str| true; // checked by tools on disk
    findings.extend(validate_maps(&pack.maps, &pool, &script_exists, external_maps));
    for (mid, map) in &pack.maps {
        if let Some(encounters) = &map.encounters {
            for (species, ..) in &encounters.slots {
                if !pack.def.dex.contains(species) {
                    findings.push(Finding::error(
                        "region.encounters",
                        format!("`{mid}` encounter species `{species}` not in the dex"),
                    ));
                }
            }
        }
    }
    if !pack.maps.is_empty() {
        if !pack.maps.contains_key(&pack.def.entry_map) {
            findings.push(Finding::error(
                "region.entry",
                format!("entry map `{}` not in the pack", pack.def.entry_map),
            ));
        } else {
            let mut reached = BTreeSet::new();
            let mut queue = VecDeque::from([pack.def.entry_map.clone()]);
            while let Some(map_id) = queue.pop_front() {
                if !reached.insert(map_id.clone()) {
                    continue;
                }
                if let Some(map) = pack.maps.get(&map_id) {
                    for trigger in &map.triggers {
                        if let crate::map::TriggerKind::Warp { map: target, .. } = &trigger.kind
                            && !reached.contains(target)
                        {
                            queue.push_back(target.clone());
                        }
                    }
                }
                // Script-driven warps count as edges too (the Vault is
                // reached through the Soloist stage script).
                for (from, to) in script_warps {
                    if *from == map_id && !reached.contains(to) {
                        queue.push_back(to.clone());
                    }
                }
            }
            for map_id in pack.maps.keys() {
                if !reached.contains(map_id) {
                    findings.push(Finding::error(
                        "region.warp_graph",
                        format!("map `{map_id}` unreachable from the entry map"),
                    ));
                }
            }
        }
    }

    findings
}

/// Doc 04 §3 rules 1 & 8: every referenced string key resolves; the
/// caller passes the merged table (core strings + region strings) and
/// any extra keys referenced by scripts (collected by `tools`).
pub fn validate_strings(
    pack: &crate::region::RegionPack,
    core_strings: &undersong_core::collections::UniqueMap<String, String>,
    script_keys: &[String],
) -> Vec<Finding> {
    let mut findings = Vec::new();
    let exists = |key: &str| pack.strings.get(key).is_some() || core_strings.get(key).is_some();
    let mut require = |key: &str, source: String| {
        if !exists(key) {
            findings.push(Finding::error(
                "strings.missing",
                format!("`{key}` referenced by {source} is not in strings.ron"),
            ));
        }
    };
    require(&pack.def.name_key, "region.ron".into());
    for motif in pack.motifs.values() {
        require(&motif.name_key, format!("motif `{}`", motif.id));
        require(&motif.dex.entry_key, format!("motif `{}`", motif.id));
        for (_, move_id) in &motif.learnset {
            require(
                &format!("move.{move_id}"),
                format!("motif `{}` learnset", motif.id),
            );
        }
    }
    for trainer in pack.trainers.values() {
        require(&trainer.name_key, format!("trainer `{}`", trainer.id));
        require(&trainer.intro_key, format!("trainer `{}`", trainer.id));
        require(&trainer.defeat_key, format!("trainer `{}`", trainer.id));
    }
    for map in pack.maps.values() {
        require(&map.name_key, format!("map `{}`", map.id));
    }
    for key in script_keys {
        require(key, "scripts".into());
    }
    findings
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use undersong_core::types::Eff;

    use super::*;
    use crate::content::{MoveSet, Natures, TypeChart};

    fn full_neutral_chart() -> TypeChart {
        let mut entries = BTreeMap::new();
        for attacker in Type::ALL {
            let row: BTreeMap<Type, Eff> =
                Type::ALL.into_iter().map(|d| (d, Eff::Neutral)).collect();
            entries.insert(attacker, row.into());
        }
        TypeChart {
            entries: entries.into(),
        }
    }

    fn canonical_natures() -> Natures {
        Natures {
            name_keys: (0..NATURE_COUNT).map(|n| format!("nature.{n}")).collect(),
        }
    }

    fn content_of(typechart: TypeChart, natures: Natures) -> CoreContent {
        CoreContent {
            typechart,
            natures,
            moves: MoveSet { moves: vec![] },
        }
    }

    fn errors(findings: &[Finding]) -> Vec<&Finding> {
        findings
            .iter()
            .filter(|f| f.severity == Severity::Error)
            .collect()
    }

    #[test]
    fn complete_core_content_is_clean() {
        let content = content_of(full_neutral_chart(), canonical_natures());
        assert!(validate_core(&content).is_empty());
    }

    #[test]
    fn missing_chart_entry_fails_totality() {
        let mut chart = full_neutral_chart();
        chart
            .entries
            .get_mut(&Type::Frost)
            .expect("row exists")
            .remove(&Type::Alloy);
        let content = content_of(chart, canonical_natures());
        let findings = validate_core(&content);
        let errs = errors(&findings);
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].rule, "typechart.totality");
        assert!(errs[0].message.contains("Frost"));
        assert!(errs[0].message.contains("Alloy"));
    }

    #[test]
    fn missing_attacker_row_fails_totality() {
        let mut chart = full_neutral_chart();
        chart.entries.remove(&Type::Venom);
        let content = content_of(chart, canonical_natures());
        let findings = validate_core(&content);
        let errs = errors(&findings);
        assert_eq!(errs.len(), 1);
        assert!(errs[0].message.contains("missing attacker row"));
    }

    #[test]
    fn wrong_nature_count_fails() {
        let mut natures = canonical_natures();
        natures.name_keys.pop();
        let content = content_of(full_neutral_chart(), natures);
        let findings = validate_core(&content);
        assert!(
            findings
                .iter()
                .any(|f| f.rule == "natures.count" && f.severity == Severity::Error)
        );
    }

    #[test]
    fn duplicate_and_empty_nature_keys_fail() {
        let mut natures = canonical_natures();
        natures.name_keys[3] = natures.name_keys[2].clone();
        natures.name_keys[7] = String::new();
        let content = content_of(full_neutral_chart(), natures);
        let findings = validate_core(&content);
        assert!(findings.iter().any(|f| f.rule == "natures.unique"));
        assert!(findings.iter().any(|f| f.rule == "natures.empty"));
    }

    #[test]
    fn parse_rejects_effectiveness_outside_the_four_values() {
        // Doc 04 §3 rule 6's "no entry outside {0, ½, 1, 2}" is enforced at
        // the type level: an unknown Eff variant cannot parse.
        let bad = "TypeChart(entries: { Feral: { Feral: Triple } })";
        assert!(ron::from_str::<TypeChart>(bad).is_err());
    }

    #[test]
    fn parse_rejects_duplicate_defender_keys() {
        // serde's default map handling is last-wins; UniqueMap makes a
        // conflicting content entry a loud parse error instead.
        let bad = "TypeChart(entries: { Feral: { Stone: Half, Stone: Double } })";
        let err = ron::from_str::<TypeChart>(bad).expect_err("duplicate defender");
        assert!(err.to_string().contains("duplicate map key"));
    }

    #[test]
    fn parse_rejects_duplicate_attacker_rows() {
        let bad = "TypeChart(entries: { Feral: {}, Feral: {} })";
        let err = ron::from_str::<TypeChart>(bad).expect_err("duplicate attacker");
        assert!(err.to_string().contains("duplicate map key"));
    }

    #[test]
    fn parse_rejects_unknown_schema_fields() {
        // deny_unknown_fields: typo'd or leftover fields must not rot
        // silently in content packs.
        let bad_natures = "Natures(name_keys: [], name_kyes: [])";
        assert!(ron::from_str::<crate::Natures>(bad_natures).is_err());
        let bad_chart = "TypeChart(entries: {}, extra: 1)";
        assert!(ron::from_str::<TypeChart>(bad_chart).is_err());
    }
}
