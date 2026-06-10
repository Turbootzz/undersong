//! Attunement — the catch formula (doc 02 §8), integer-only.

use undersong_core::moves::Frac;
use undersong_core::rng::BattleRng;

use crate::mote::{BattleMote, MajorStatus};

/// `floor(sqrt(n))` for u64, by Newton's method; exact for all inputs.
fn isqrt(n: u64) -> u64 {
    if n < 2 {
        return n;
    }
    let mut x = n;
    let mut y = x.div_ceil(2);
    while y < x {
        x = y;
        y = (x + n / x) / 2;
    }
    x
}

/// Status multiplier (doc 02 §8): sleep/freeze ×2.0, burn/poison/
/// paralysis (and toxic, which is poison) ×1.5, none ×1.0.
fn status_mod(status: Option<MajorStatus>) -> Frac {
    match status {
        Some(MajorStatus::Sleep { .. } | MajorStatus::Freeze) => Frac(2, 1),
        Some(_) => Frac(3, 2),
        None => Frac(1, 1),
    }
}

pub struct AttuneResult {
    /// Passed checks, 0–4; 4 = caught ("settles" on the resolved chord).
    pub rings: u8,
    pub caught: bool,
}

/// One bell ring (doc 02 §8):
/// `a = floor((3·MaxHP − 2·CurHP) · catch_rate · bell · status / (3·MaxHP))`
/// `a ≥ 255` → caught immediately; otherwise
/// `b = floor(1048560 / floor(sqrt(floor(sqrt(16711680 / a)))))` and four
/// checks `rand_u16 < b`.
pub fn attune(target: &BattleMote, bell_mod: Frac, rng: &mut BattleRng) -> AttuneResult {
    let max_hp = u64::from(target.max_hp());
    let cur_hp = u64::from(target.hp);
    let status = status_mod(target.status);

    let numerator = (3 * max_hp - 2 * cur_hp)
        * u64::from(target.catch_rate)
        * u64::from(bell_mod.0)
        * u64::from(status.0);
    let denominator = 3 * max_hp * u64::from(bell_mod.1) * u64::from(status.1);
    // Guard a ≥ 1: a = 0 only via degenerate inputs (catch_rate ≥ 3 per
    // schema), and the b-formula divides by a.
    let a = (numerator / denominator.max(1)).clamp(1, u64::from(u32::MAX));

    if a >= 255 {
        return AttuneResult {
            rings: 4,
            caught: true,
        };
    }

    let b = 1_048_560 / isqrt(isqrt(16_711_680 / a)).max(1);
    let mut rings: u8 = 0;
    for _ in 0..4 {
        if u64::from(rng.next_u16()) < b {
            rings += 1;
        } else {
            break;
        }
    }
    AttuneResult {
        rings,
        caught: rings == 4,
    }
}

#[cfg(test)]
mod tests {
    use undersong_core::ids::SpeciesId;
    use undersong_core::species::{GrowthCurve, StatSpread};

    use super::*;
    use crate::mote::ZERO_SPREAD;

    fn wild(max_hp: u16, cur_hp: u16, catch_rate: u8, status: Option<MajorStatus>) -> BattleMote {
        let stats = StatSpread {
            hp: max_hp,
            atk: 50,
            def: 50,
            spa: 50,
            spd: 50,
            spe: 50,
        };
        BattleMote {
            species: SpeciesId::new("wild"),
            name_key: "motif.wild".into(),
            types: vec![undersong_core::types::Type::Feral],
            level: 10,
            exp: 0,
            growth: GrowthCurve::MediumFast,
            nature: 0,
            base_stats: stats,
            ivs: ZERO_SPREAD,
            evs: ZERO_SPREAD,
            stats,
            hp: cur_hp,
            status,
            moves: vec![],
            catch_rate,
            base_exp_yield: 100,
            ev_yield: vec![],
            learnset: vec![],
            ability: crate::abilities::Ability::None,
            held: crate::abilities::HeldItem::None,
            entry_boosted: false,
        }
    }

    #[test]
    fn isqrt_exactness() {
        assert_eq!(isqrt(0), 0);
        assert_eq!(isqrt(1), 1);
        assert_eq!(isqrt(15), 3);
        assert_eq!(isqrt(16), 4);
        // 4087² = 16_703_569 ≤ 16_711_680 < 4088² = 16_711_744
        assert_eq!(isqrt(16_711_680), 4_087);
        assert_eq!(isqrt(u64::MAX), 4_294_967_295);
    }

    #[test]
    fn full_health_max_rate_with_strong_bell_is_instant() {
        // a = floor((3M − 2M)·255·2·1 / 3M) = floor(255·2/3) = 170 < 255 —
        // not instant. But asleep at 1 HP with a Maestro bell:
        // a = floor((300−2)·255·2·2 / 300) ≈ 1013 → instant.
        let target = wild(100, 1, 255, Some(MajorStatus::Sleep { turns: 2 }));
        let mut rng = BattleRng::from_seed(1);
        let result = attune(&target, Frac(2, 1), &mut rng);
        assert!(result.caught);
        assert_eq!(result.rings, 4);
    }

    #[test]
    fn hopeless_target_rarely_catches_but_never_panics() {
        // Full HP, minimum catch rate, base bell: catches must be rare.
        let target = wild(200, 200, 3, None);
        let mut caught = 0;
        for seed in 0..2_000u64 {
            let mut rng = BattleRng::from_seed(seed);
            let result = attune(&target, Frac(1, 1), &mut rng);
            assert!(result.rings <= 4);
            if result.caught {
                caught += 1;
            }
        }
        // a = floor((600−400)·3/600) = 1 → b = 1048560/isqrt(isqrt(16711680))
        //   isqrt(16711680)=4087, isqrt(4087)=63 → b = 16643
        // P(one check) = 16643/65536 ≈ 25.4% → P(catch) ≈ 0.42%.
        assert!(caught < 100, "caught {caught} of 2000 — formula drifted?");
    }

    #[test]
    fn status_and_hp_strictly_help() {
        // Same seeds; lower HP and sleep must never reduce ring counts on
        // identical rng streams (b is monotone in a).
        for seed in 0..200u64 {
            let healthy = wild(100, 100, 45, None);
            let weakened = wild(100, 10, 45, Some(MajorStatus::Sleep { turns: 1 }));
            let mut rng_a = BattleRng::from_seed(seed);
            let mut rng_b = BattleRng::from_seed(seed);
            let base = attune(&healthy, Frac(1, 1), &mut rng_a);
            let helped = attune(&weakened, Frac(1, 1), &mut rng_b);
            assert!(helped.rings >= base.rings, "seed {seed}");
        }
    }

    #[test]
    fn coda_equivalent_bell_always_succeeds() {
        // The Coda "always succeeds" is modeled as a huge bell mod by the
        // item layer; verify the formula saturates into the instant path.
        let target = wild(500, 500, 3, None);
        let mut rng = BattleRng::from_seed(7);
        let result = attune(&target, Frac(1000, 1), &mut rng);
        assert!(result.caught);
    }
}
