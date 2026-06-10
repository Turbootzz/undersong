//! The six battle stats.
//!
//! Code uses mechanical names; UI flavor names ("Timbre", "Practice", …)
//! live in string tables and must never leak into identifiers
//! (CLAUDE.md golden rule 6).

/// One of the six battle stats, in canonical order.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum Stat {
    Hp,
    Atk,
    Def,
    Spa,
    Spd,
    Spe,
}

impl Stat {
    /// Canonical order, matching the `(hp, atk, def, spa, spd, spe)` shape
    /// used by content files (docs/04-CONTENT-PIPELINE.md §2).
    pub const ALL: [Stat; 6] = [
        Stat::Hp,
        Stat::Atk,
        Stat::Def,
        Stat::Spa,
        Stat::Spd,
        Stat::Spe,
    ];

    /// Index into stat arrays, following [`Stat::ALL`] order.
    pub const fn index(self) -> usize {
        self as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_order_and_indices_agree() {
        for (i, stat) in Stat::ALL.into_iter().enumerate() {
            assert_eq!(stat.index(), i);
        }
    }

    #[test]
    fn serializes_snake_case_for_content_files() {
        assert_eq!(ron::to_string(&Stat::Spa).expect("serialize"), "spa");
        let parsed: Stat = ron::from_str("atk").expect("deserialize");
        assert_eq!(parsed, Stat::Atk);
    }
}
