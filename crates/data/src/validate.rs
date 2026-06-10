//! Validation rules for loaded content (doc 04 §3).
//!
//! P0 implements the rules that have data to act on: type-chart totality
//! (rule 6 — value range is already enforced by the `Eff` enum at parse
//! time) and the natures shape. The rest of the rule list lands with the
//! content kinds it validates.

use std::collections::BTreeSet;

use undersong_core::types::Type;

use crate::content::{CoreContent, NATURE_COUNT};

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
        if let Some(encounters) = &map.encounters {
            if encounters.slots.len() != 12 {
                findings.push(Finding::error(
                    "map.encounters",
                    format!(
                        "`{mid}` has {} encounter slots, doc 02 §12 requires exactly 12",
                        encounters.slots.len()
                    ),
                ));
            }
            let weight_sum: u32 = encounters.slots.iter().map(|s| u32::from(s.3)).sum();
            if weight_sum != 100 {
                findings.push(Finding::error(
                    "map.encounters",
                    format!("`{mid}` encounter weights sum to {weight_sum}, expected 100"),
                ));
            }
            for (species, lo, hi, _) in &encounters.slots {
                if *lo < 1 || hi < lo {
                    findings.push(Finding::error(
                        "map.encounters",
                        format!("`{mid}` slot `{species}` level range {lo}..{hi} invalid"),
                    ));
                }
                if !pool.species.iter().any(|sp| &sp.id == species) {
                    findings.push(Finding::error(
                        "map.encounters",
                        format!("`{mid}` slot references unknown species `{species}`"),
                    ));
                }
            }
        }
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
