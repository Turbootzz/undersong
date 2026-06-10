//! The 12×12 type-effectiveness chart (doc 02 §1).
//!
//! Shared vocabulary: `data` parses it from content, `battle` consults it
//! (a battle embeds its chart so replays stay reproducible even if content
//! changes), and those crates may only meet in core (doc 03 §1).

use serde::{Deserialize, Serialize};

use crate::collections::UniqueMap;
use crate::types::{Eff, Type};

/// Attacker → defender → multiplier. The RON file is fully explicit —
/// every attacker lists every defender — so the validator proves totality
/// (doc 04 §3 rule 6) instead of silently defaulting missing pairs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TypeChart {
    pub entries: UniqueMap<Type, UniqueMap<Type, Eff>>,
}

impl TypeChart {
    /// Effectiveness of `attacker` against `defender`, if the pair is present.
    pub fn eff(&self, attacker: Type, defender: Type) -> Option<Eff> {
        self.entries.get(&attacker)?.get(&defender).copied()
    }

    /// Combined multiplier of one attacking type against a (mono or dual)
    /// defender, as an exact rational. Missing pairs count as neutral —
    /// the validator guarantees totality for shipped charts.
    pub fn product(&self, attacker: Type, defender_types: &[Type]) -> (u32, u32) {
        defender_types
            .iter()
            .map(|&d| self.eff(attacker, d).unwrap_or(Eff::Neutral).ratio())
            .fold((1, 1), |(num, den), (n, d)| (num * n, den * d))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tiny_chart() -> TypeChart {
        ron::from_str(
            "TypeChart(entries: {
                Ember: { Bloom: Double, Tide: Half, Ember: Half, Stone: Half },
                Volt:  { Stone: Zero, Tide: Double },
            })",
        )
        .expect("parse")
    }

    #[test]
    fn product_multiplies_dual_types() {
        let chart = tiny_chart();
        // 2 × ½ = 1
        assert_eq!(
            chart.product(Type::Ember, &[Type::Bloom, Type::Tide]),
            (2, 2)
        );
        // 2 × (missing → neutral) = 2
        assert_eq!(
            chart.product(Type::Ember, &[Type::Bloom, Type::Gale]),
            (2, 1)
        );
        // 0 × 2 = 0
        assert_eq!(chart.product(Type::Volt, &[Type::Stone, Type::Tide]).0, 0);
    }
}
