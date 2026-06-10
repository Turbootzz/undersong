//! Stat math (doc 02 §3) — integer-only, floor at every step.

use serde::{Deserialize, Serialize};
use undersong_core::species::StatSpread;
use undersong_core::stats::Stat;

/// Stage-modifiable quantities: the five non-HP stats plus accuracy and
/// evasion (doc 02 §3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StageStat {
    Atk,
    Def,
    Spa,
    Spd,
    Spe,
    Acc,
    Eva,
}

impl StageStat {
    pub const COUNT: usize = 7;

    pub const fn index(self) -> usize {
        self as usize
    }

    pub const fn from_stat(stat: Stat) -> Option<StageStat> {
        match stat {
            Stat::Hp => None,
            Stat::Atk => Some(StageStat::Atk),
            Stat::Def => Some(StageStat::Def),
            Stat::Spa => Some(StageStat::Spa),
            Stat::Spd => Some(StageStat::Spd),
            Stat::Spe => Some(StageStat::Spe),
        }
    }
}

/// The −6..=+6 stage block for one active position; reset on switch.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Stages(pub [i8; StageStat::COUNT]);

impl Stages {
    pub fn get(&self, stat: StageStat) -> i8 {
        self.0[stat.index()]
    }

    /// Applies `delta`, clamping to ±6. Returns `(new_stage, clamped)`;
    /// `clamped` is true when the stage was already at the limit and did
    /// not move.
    pub fn bump(&mut self, stat: StageStat, delta: i8) -> (i8, bool) {
        let old = self.0[stat.index()];
        // i16 intermediate: i8 addition could overflow on extreme deltas
        // before the clamp ever ran.
        let new =
            i8::try_from((i16::from(old) + i16::from(delta)).clamp(-6, 6)).expect("clamped to ±6");
        self.0[stat.index()] = new;
        (new, new == old)
    }
}

/// `floor(value · max(2, 2+s) / max(2, 2−s))` (doc 02 §3) for stat stages.
pub fn stage_multiplied(value: u32, stage: i8) -> u32 {
    let num = u32::try_from(2i16.max(2 + i16::from(stage))).expect("positive");
    let den = u32::try_from(2i16.max(2 - i16::from(stage))).expect("positive");
    value * num / den
}

/// Accuracy/evasion stage factor `max(3, 3+s) / max(3, 3−s)` (doc 02 §3).
pub fn acc_stage_factor(stage: i8) -> (u32, u32) {
    let num = u32::try_from(3i16.max(3 + i16::from(stage))).expect("positive");
    let den = u32::try_from(3i16.max(3 - i16::from(stage))).expect("positive");
    (num, den)
}

/// Nature multiplier: index `n ∈ 0..25`, boosted stat = `n / 5`,
/// hindered = `n % 5`, over `[atk, def, spa, spd, spe]`; ×1.10 / ×0.90 as
/// 110/100 and 90/100; neutral when boosted == hindered (doc 02 §3).
fn nature_factor(nature: u8, stat: Stat) -> (u32, u32) {
    let Some(stage_index) = StageStat::from_stat(stat) else {
        return (1, 1); // HP is never nature-modified
    };
    let position = stage_index.index(); // atk=0 … spe=4
    let boosted = usize::from(nature / 5);
    let hindered = usize::from(nature % 5);
    if boosted == hindered || position >= 5 {
        (1, 1)
    } else if position == boosted {
        (110, 100)
    } else if position == hindered {
        (90, 100)
    } else {
        (1, 1)
    }
}

/// Computes one stat from the doc 02 §3 formulas.
///
/// `HP    = floor((2·Base + IV + floor(EV/4)) · L / 100) + L + 10`
/// `Other = floor((floor((2·Base + IV + floor(EV/4)) · L / 100) + 5) · Nature)`
pub fn compute_stat(stat: Stat, base: u16, iv: u8, ev: u16, level: u8, nature: u8) -> u16 {
    let core = (2 * u32::from(base) + u32::from(iv) + u32::from(ev) / 4) * u32::from(level) / 100;
    let value = if stat == Stat::Hp {
        core + u32::from(level) + 10
    } else {
        let (num, den) = nature_factor(nature, stat);
        (core + 5) * num / den
    };
    u16::try_from(value).expect("stats fit u16")
}

/// All six stats for a Mote.
pub fn compute_all(
    base: &StatSpread,
    ivs: &StatSpread,
    evs: &StatSpread,
    level: u8,
    nature: u8,
) -> StatSpread {
    let one = |stat: Stat| {
        compute_stat(
            stat,
            base.get(stat),
            u8::try_from(ivs.get(stat).min(31)).expect("iv <= 31"),
            evs.get(stat),
            level,
            nature,
        )
    };
    StatSpread {
        hp: one(Stat::Hp),
        atk: one(Stat::Atk),
        def: one(Stat::Def),
        spa: one(Stat::Spa),
        spd: one(Stat::Spd),
        spe: one(Stat::Spe),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Hand-computed vectors (doc 02 §3, worked by hand in comments).

    #[test]
    fn hp_formula_hand_vectors() {
        // base 58, IV 31, EV 0, L50: floor((116+31+0)·50/100)=73; +50+10=133
        assert_eq!(compute_stat(Stat::Hp, 58, 31, 0, 50, 0), 133);
        // base 100, IV 31, EV 252, L100: (200+31+63)·100/100=294; +110=404
        assert_eq!(compute_stat(Stat::Hp, 100, 31, 252, 100, 0), 404);
        // level 5 starter: base 45, IV 20, EV 0: floor((90+20)·5/100)=5; +15=20
        assert_eq!(compute_stat(Stat::Hp, 45, 20, 0, 5, 0), 20);
    }

    #[test]
    fn other_stat_formula_hand_vectors() {
        // base 80 spa, IV 31, EV 252, L50, neutral (n=0 is +atk/−atk):
        // floor((160+31+63)·50/100)=127; +5=132
        assert_eq!(compute_stat(Stat::Spa, 80, 31, 252, 50, 0), 132);
        // boosted spa: nature 10 (Brillante, +spa −atk): floor(132·110/100)=145
        assert_eq!(compute_stat(Stat::Spa, 80, 31, 252, 50, 10), 145);
        // hindered spa: nature 2 (Bravura, +atk −spa): floor(132·90/100)=118
        assert_eq!(compute_stat(Stat::Spa, 80, 31, 252, 50, 2), 118);
        // level 5: base 64 atk, IV 20, EV 0: floor((128+20)·5/100)=7; +5=12
        assert_eq!(compute_stat(Stat::Atk, 64, 20, 0, 5, 0), 12);
    }

    #[test]
    fn nature_diagonal_is_neutral() {
        for n in [0u8, 6, 12, 18, 24] {
            for stat in [Stat::Atk, Stat::Def, Stat::Spa, Stat::Spd, Stat::Spe] {
                assert_eq!(nature_factor(n, stat), (1, 1), "nature {n} {stat:?}");
            }
        }
    }

    #[test]
    fn nature_grid_examples() {
        // n=1 Forte: +atk −def
        assert_eq!(nature_factor(1, Stat::Atk), (110, 100));
        assert_eq!(nature_factor(1, Stat::Def), (90, 100));
        assert_eq!(nature_factor(1, Stat::Spe), (1, 1));
        // n=23 Scherzo: +spe −spd
        assert_eq!(nature_factor(23, Stat::Spe), (110, 100));
        assert_eq!(nature_factor(23, Stat::Spd), (90, 100));
        // HP never modified
        assert_eq!(nature_factor(1, Stat::Hp), (1, 1));
    }

    #[test]
    fn stage_multiplier_table() {
        // doc 02 §3: max(2,2+s)/max(2,2−s)
        assert_eq!(stage_multiplied(100, 0), 100);
        assert_eq!(stage_multiplied(100, 1), 150); // 3/2
        assert_eq!(stage_multiplied(100, 2), 200); // 4/2
        assert_eq!(stage_multiplied(100, 6), 400); // 8/2
        assert_eq!(stage_multiplied(100, -1), 66); // 2/3 floored
        assert_eq!(stage_multiplied(100, -6), 25); // 2/8
    }

    #[test]
    fn acc_stage_table() {
        assert_eq!(acc_stage_factor(0), (3, 3));
        assert_eq!(acc_stage_factor(1), (4, 3));
        assert_eq!(acc_stage_factor(6), (9, 3));
        assert_eq!(acc_stage_factor(-2), (3, 5));
    }

    #[test]
    fn stages_clamp_at_six() {
        let mut stages = Stages::default();
        let (new, clamped) = stages.bump(StageStat::Atk, 4);
        assert_eq!((new, clamped), (4, false));
        let (new, clamped) = stages.bump(StageStat::Atk, 4);
        assert_eq!((new, clamped), (6, false));
        let (new, clamped) = stages.bump(StageStat::Atk, 1);
        assert_eq!((new, clamped), (6, true));
        let (new, clamped) = stages.bump(StageStat::Eva, -7);
        assert_eq!((new, clamped), (-6, false));
    }
}
