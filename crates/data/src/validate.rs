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

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use undersong_core::types::Eff;

    use super::*;
    use crate::content::{Natures, TypeChart};

    fn full_neutral_chart() -> TypeChart {
        let mut entries = BTreeMap::new();
        for attacker in Type::ALL {
            let row: BTreeMap<Type, Eff> =
                Type::ALL.into_iter().map(|d| (d, Eff::Neutral)).collect();
            entries.insert(attacker, row);
        }
        TypeChart { entries }
    }

    fn canonical_natures() -> Natures {
        Natures {
            name_keys: (0..NATURE_COUNT).map(|n| format!("nature.{n}")).collect(),
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
        let content = CoreContent {
            typechart: full_neutral_chart(),
            natures: canonical_natures(),
        };
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
        let content = CoreContent {
            typechart: chart,
            natures: canonical_natures(),
        };
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
        let content = CoreContent {
            typechart: chart,
            natures: canonical_natures(),
        };
        let findings = validate_core(&content);
        let errs = errors(&findings);
        assert_eq!(errs.len(), 1);
        assert!(errs[0].message.contains("missing attacker row"));
    }

    #[test]
    fn wrong_nature_count_fails() {
        let mut natures = canonical_natures();
        natures.name_keys.pop();
        let content = CoreContent {
            typechart: full_neutral_chart(),
            natures,
        };
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
        let content = CoreContent {
            typechart: full_neutral_chart(),
            natures,
        };
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
}
