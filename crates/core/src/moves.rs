//! Move specifications and the effect vocabulary (doc 02 §6).
//!
//! These shapes are shared vocabulary: `data` parses content RON into them
//! and `battle` executes them, and those two crates are siblings that may
//! only meet here (docs/03-ARCHITECTURE.md §1).

use serde::{Deserialize, Serialize};

use crate::ids::MoveId;
use crate::stats::Stat;
use crate::types::Type;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MoveCategory {
    Physical,
    Special,
    Status,
}

/// Single-battle targets; the doubles target set arrives in P4.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MoveTarget {
    Foe,
    User,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WeatherKind {
    Heatwave,
    Downpour,
    Flurry,
    Dustchord,
}

/// Major status ailments (doc 02 §5). `Toxic` is the escalating poison
/// variant; it shares poison's immunities.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Ailment {
    Burn,
    Poison,
    Toxic,
    Paralysis,
    Sleep,
    Freeze,
}

/// An exact fraction `(numerator, denominator)` — battle math is
/// integer-only (doc 03 §2), so content carries rationals, never floats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Frac(pub u32, pub u32);

impl Frac {
    /// `floor(value · num / den)`, computed in u64 to avoid overflow.
    pub fn apply(self, value: u32) -> u32 {
        let Frac(num, den) = self;
        u32::try_from(u64::from(value) * u64::from(num) / u64::from(den.max(1))).unwrap_or(u32::MAX)
    }
}

/// Move flags (doc 02 §6, plus the two flag-like properties the canon
/// moves table uses: high crit stage and evasion-stage bypass).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MoveFlags {
    pub contact: bool,
    pub sound: bool,
    pub protectable: bool,
    pub reflectable: bool,
    pub punch: bool,
    pub bite: bool,
    /// +1 crit stage (doc 02 v1.1 #10), e.g. `leaf_pick`.
    pub high_crit: bool,
    /// Ignores the target's evasion stages in the accuracy formula
    /// (doc 02 v1.1 #18), e.g. `resonate`.
    pub ignore_evasion: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EffectTarget {
    User,
    Target,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FixedAmount {
    Amount(u16),
    UserLevel,
}

/// Secondary effects, executed in list order after damage (doc 02 §6).
/// Damage itself is implicit from `power > 0` — the doc's effect list is
/// "executed after damage", so a damage marker would be redundant.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Effect {
    StatStage {
        target: EffectTarget,
        stat: Stat,
        delta: i8,
        /// Percent chance 1–100.
        chance: u8,
    },
    Status {
        ailment: Ailment,
        /// Percent chance 1–100.
        chance: u8,
    },
    /// Heals the user by a fraction of its max HP.
    Heal {
        frac: Frac,
    },
    /// Heals the user by a fraction of damage dealt.
    Drain {
        frac: Frac,
    },
    /// Recoil to the user as a fraction of damage dealt.
    Recoil {
        frac: Frac,
    },
    /// 2–5 hits, distribution 2:3/8, 3:3/8, 4:1/8, 5:1/8 (doc 02 §6).
    MultiHit,
    TwoTurn {
        charge_text: String,
    },
    Protect,
    Weather {
        kind: WeatherKind,
    },
    Flinch {
        /// Percent chance 1–100.
        chance: u8,
    },
    ForceSwitch,
    SelfSwitch,
    Ohko,
    FixedDamage {
        amount: FixedAmount,
    },
}

/// A complete move definition (doc 02 §6 schema).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MoveSpec {
    pub id: MoveId,
    pub name_key: String,
    pub r#type: Type,
    pub category: MoveCategory,
    /// 0 = not a damaging move.
    pub power: u16,
    /// 1–100; 0 = never misses (doc 02 §4).
    pub accuracy: u8,
    pub pp: u8,
    /// −7..=+5.
    pub priority: i8,
    pub target: MoveTarget,
    #[serde(default)]
    pub flags: MoveFlags,
    #[serde(default)]
    pub effects: Vec<Effect>,
}

impl MoveSpec {
    pub fn is_damaging(&self) -> bool {
        self.power > 0
            || matches!(
                self.category,
                MoveCategory::Physical | MoveCategory::Special
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frac_applies_floor_semantics() {
        assert_eq!(Frac(1, 2).apply(75), 37);
        assert_eq!(Frac(1, 16).apply(48), 3);
        assert_eq!(Frac(3, 2).apply(100), 150);
        assert_eq!(Frac(0, 1).apply(999), 0);
    }

    #[test]
    fn move_spec_round_trips_through_ron() {
        let spec = MoveSpec {
            id: MoveId::new("ember_note"),
            name_key: "move.ember_note".into(),
            r#type: Type::Ember,
            category: MoveCategory::Special,
            power: 40,
            accuracy: 100,
            pp: 25,
            priority: 0,
            target: MoveTarget::Foe,
            flags: MoveFlags {
                sound: true,
                ..MoveFlags::default()
            },
            effects: vec![Effect::Status {
                ailment: Ailment::Burn,
                chance: 10,
            }],
        };
        let text = ron::to_string(&spec).expect("serialize");
        let back: MoveSpec = ron::from_str(&text).expect("deserialize");
        assert_eq!(back, spec);
    }
}
