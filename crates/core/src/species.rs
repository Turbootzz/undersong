//! Species (Motif) battle-facing specification (doc 02 §2).
//!
//! This is the subset the simulation and tools need: stats, typing,
//! progression. Content-only concerns (dex text, cry/sigil seeds, TM sets,
//! evolution targets) belong to the `data` crate's full content schema and
//! arrive with the content phases.

use serde::{Deserialize, Serialize};

use crate::collections::UniqueMap;
use crate::ids::{AbilityId, MoveId, SpeciesId};
use crate::stats::Stat;
use crate::types::Type;

/// The six base stats in content shape `(hp: …, atk: …, …)` (doc 04 §2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StatSpread {
    pub hp: u16,
    pub atk: u16,
    pub def: u16,
    pub spa: u16,
    pub spd: u16,
    pub spe: u16,
}

impl StatSpread {
    pub const fn get(&self, stat: Stat) -> u16 {
        match stat {
            Stat::Hp => self.hp,
            Stat::Atk => self.atk,
            Stat::Def => self.def,
            Stat::Spa => self.spa,
            Stat::Spd => self.spd,
            Stat::Spe => self.spe,
        }
    }

    pub fn total(&self) -> u32 {
        Stat::ALL.into_iter().map(|s| u32::from(self.get(s))).sum()
    }
}

/// Experience growth curves (doc 02 §9). Total exp to reach level n:
/// medium_fast n³ · fast 0.8n³ · slow 1.25n³ ·
/// medium_slow 1.2n³ − 15n² + 100n − 140 (clamped ≥ 0, doc 02 v1.1 #12).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GrowthCurve {
    MediumFast,
    Fast,
    Slow,
    MediumSlow,
}

impl GrowthCurve {
    /// Total experience required to be at `level`. Level 1 is 0 for every
    /// curve. Integer math only; fractions floor.
    pub fn total_exp(self, level: u8) -> u32 {
        if level <= 1 {
            return 0;
        }
        let n = i64::from(level);
        let cubed = n * n * n;
        let total = match self {
            GrowthCurve::MediumFast => cubed,
            GrowthCurve::Fast => 4 * cubed / 5,
            GrowthCurve::Slow => 5 * cubed / 4,
            GrowthCurve::MediumSlow => 6 * cubed / 5 - 15 * n * n + 100 * n - 140,
        };
        u32::try_from(total.max(0)).expect("exp fits u32 for level <= 100")
    }
}

/// Battle-facing species definition (doc 02 §2 subset).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpeciesSpec {
    pub id: SpeciesId,
    pub name_key: String,
    /// 1–2 types.
    pub types: Vec<Type>,
    pub base_stats: StatSpread,
    /// 3–255 (doc 02 §2).
    pub catch_rate: u8,
    pub base_exp_yield: u16,
    /// Per-stat EV award, each 1–3 (doc 02 §2).
    pub ev_yield: UniqueMap<Stat, u8>,
    pub growth_curve: GrowthCurve,
    /// `(level, move)` pairs, levels ascending (validated, doc 04 §3 rule 4).
    pub learnset: Vec<(u8, MoveId)>,
    /// Parsed now, wired up in P4 (abilities phase).
    #[serde(default)]
    pub abilities: Vec<AbilityId>,
    #[serde(default)]
    pub hidden_ability: Option<AbilityId>,
    /// `performer.*` / `habitat.*` tags (doc 02 §2).
    #[serde(default)]
    pub tags: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn growth_curves_match_doc_formulas_at_known_levels() {
        // medium_fast: n³
        assert_eq!(GrowthCurve::MediumFast.total_exp(10), 1_000);
        assert_eq!(GrowthCurve::MediumFast.total_exp(100), 1_000_000);
        // fast: 0.8 n³
        assert_eq!(GrowthCurve::Fast.total_exp(10), 800);
        assert_eq!(GrowthCurve::Fast.total_exp(100), 800_000);
        // slow: 1.25 n³
        assert_eq!(GrowthCurve::Slow.total_exp(10), 1_250);
        assert_eq!(GrowthCurve::Slow.total_exp(100), 1_250_000);
        // medium_slow: 1.2 n³ − 15 n² + 100 n − 140  →  at 10: 1200−1500+1000−140
        assert_eq!(GrowthCurve::MediumSlow.total_exp(10), 560);
        assert_eq!(
            GrowthCurve::MediumSlow.total_exp(100),
            1_200_000 - 150_000 + 10_000 - 140
        );
    }

    #[test]
    fn growth_curves_clamp_and_zero_at_level_one() {
        for curve in [
            GrowthCurve::MediumFast,
            GrowthCurve::Fast,
            GrowthCurve::Slow,
            GrowthCurve::MediumSlow,
        ] {
            assert_eq!(curve.total_exp(1), 0, "{curve:?} level 1");
        }
        // medium_slow at level 2: 9.6−60+200−140 = 9.6 → floor 9
        assert_eq!(GrowthCurve::MediumSlow.total_exp(2), 9);
    }

    #[test]
    fn growth_is_monotonic_from_level_two() {
        for curve in [
            GrowthCurve::MediumFast,
            GrowthCurve::Fast,
            GrowthCurve::Slow,
            GrowthCurve::MediumSlow,
        ] {
            for level in 2..100u8 {
                assert!(
                    curve.total_exp(level + 1) > curve.total_exp(level),
                    "{curve:?} not increasing at {level}"
                );
            }
        }
    }

    #[test]
    fn stat_spread_lookup_matches_fields() {
        let spread = StatSpread {
            hp: 58,
            atk: 64,
            def: 50,
            spa: 80,
            spd: 58,
            spe: 80,
        };
        assert_eq!(spread.get(Stat::Hp), 58);
        assert_eq!(spread.get(Stat::Spa), 80);
        assert_eq!(spread.total(), 390);
    }
}
