//! The 12 creature types and the effectiveness multiplier vocabulary.
//!
//! The chart itself is content (`content/core/typechart.ron`), never code
//! (docs/02-GAME-DESIGN.md §1).

/// The 12 types, in the canonical order of docs/02-GAME-DESIGN.md §1.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub enum Type {
    Feral,
    Ember,
    Tide,
    Bloom,
    Volt,
    Gale,
    Stone,
    Frost,
    Venom,
    Phantom,
    Alloy,
    Resonant,
}

impl Type {
    pub const COUNT: usize = 12;

    /// Canonical order (doc 02 §1).
    pub const ALL: [Type; Type::COUNT] = [
        Type::Feral,
        Type::Ember,
        Type::Tide,
        Type::Bloom,
        Type::Volt,
        Type::Gale,
        Type::Stone,
        Type::Frost,
        Type::Venom,
        Type::Phantom,
        Type::Alloy,
        Type::Resonant,
    ];
}

/// A type-effectiveness multiplier. Only these four values exist
/// (doc 02 §1; validator rule 6 in doc 04 §3) — battle math is integer-only,
/// so the multiplier is exposed as a rational.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum Eff {
    Zero,
    Half,
    Neutral,
    Double,
}

impl Eff {
    /// The multiplier as `(numerator, denominator)`.
    pub const fn ratio(self) -> (u32, u32) {
        match self {
            Eff::Zero => (0, 1),
            Eff::Half => (1, 2),
            Eff::Neutral => (1, 1),
            Eff::Double => (2, 1),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn twelve_types_in_canonical_order() {
        assert_eq!(Type::ALL.len(), Type::COUNT);
        assert_eq!(Type::ALL[0], Type::Feral);
        assert_eq!(Type::ALL[11], Type::Resonant);
    }

    #[test]
    fn type_variants_parse_from_ron_identifiers() {
        let parsed: Type = ron::from_str("Resonant").expect("deserialize");
        assert_eq!(parsed, Type::Resonant);
    }

    #[test]
    fn eff_ratios_are_the_four_legal_multipliers() {
        assert_eq!(Eff::Zero.ratio(), (0, 1));
        assert_eq!(Eff::Half.ratio(), (1, 2));
        assert_eq!(Eff::Neutral.ratio(), (1, 1));
        assert_eq!(Eff::Double.ratio(), (2, 1));
    }
}
