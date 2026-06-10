//! The only entropy source in the simulation.
//!
//! `BattleRng` wraps ChaCha8 behind a minimal interface so call sites cannot
//! reach `thread_rng` or OS entropy — this crate does not even depend on the
//! `rand` crate, only `rand_chacha` + `rand_core`, neither of which exposes a
//! global RNG. Determinism is sacred (docs/03-ARCHITECTURE.md §2).

use rand_chacha::ChaCha8Rng;
use rand_core::{Rng, SeedableRng};

/// Seeded, cloneable, deterministic RNG for battle and content generation.
///
/// Cloning forks the stream: the clone continues exactly where the original
/// was, which is what replay verification needs.
#[derive(Debug, Clone)]
pub struct BattleRng {
    inner: ChaCha8Rng,
}

impl BattleRng {
    pub fn from_seed(seed: u64) -> Self {
        Self {
            inner: ChaCha8Rng::seed_from_u64(seed),
        }
    }

    /// Next raw 32-bit draw.
    pub fn next_u32(&mut self) -> u32 {
        self.inner.next_u32()
    }

    /// Next raw 16-bit draw (the catch formula in doc 02 §8 compares
    /// against `rand_u16`).
    pub fn next_u16(&mut self) -> u16 {
        (self.inner.next_u32() >> 16) as u16
    }

    /// Uniform draw in `0..n`. Panics if `n == 0`.
    ///
    /// Uses rejection sampling, so it is unbiased; the number of raw draws
    /// consumed is a pure function of the stream, preserving replayability.
    pub fn below(&mut self, n: u32) -> u32 {
        assert!(n > 0, "BattleRng::below(0) has no legal result");
        // Largest multiple of n that fits in u32; draws at or above it would
        // bias the modulo, so they are rejected and redrawn.
        let zone = (u32::MAX / n) * n;
        loop {
            let draw = self.next_u32();
            if draw < zone {
                return draw % n;
            }
        }
    }

    /// Uniform draw in `lo..=hi` (inclusive). Panics if `lo > hi`.
    pub fn range_inclusive(&mut self, lo: u32, hi: u32) -> u32 {
        assert!(lo <= hi, "BattleRng::range_inclusive: lo > hi");
        // The span is computed in u64: for the full domain (0, u32::MAX)
        // `hi - lo + 1` would overflow u32.
        let span = u64::from(hi) - u64::from(lo) + 1;
        match u32::try_from(span) {
            Ok(span) => lo + self.below(span),
            // Full domain: every raw draw is already uniform.
            Err(_) => self.next_u32(),
        }
    }

    /// True with probability `num/den`. Panics if `den == 0`.
    pub fn chance(&mut self, num: u32, den: u32) -> bool {
        self.below(den) < num
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_same_stream() {
        let mut a = BattleRng::from_seed(0x00DE_C0DE);
        let mut b = BattleRng::from_seed(0x00DE_C0DE);
        for _ in 0..64 {
            assert_eq!(a.next_u32(), b.next_u32());
        }
    }

    #[test]
    fn different_seeds_diverge() {
        let mut a = BattleRng::from_seed(1);
        let mut b = BattleRng::from_seed(2);
        let first_eight: (Vec<u32>, Vec<u32>) = (
            (0..8).map(|_| a.next_u32()).collect(),
            (0..8).map(|_| b.next_u32()).collect(),
        );
        assert_ne!(first_eight.0, first_eight.1);
    }

    #[test]
    fn clone_forks_the_stream() {
        let mut original = BattleRng::from_seed(99);
        original.next_u32();
        let mut fork = original.clone();
        for _ in 0..32 {
            assert_eq!(original.next_u32(), fork.next_u32());
        }
    }

    #[test]
    fn below_stays_in_bounds_and_hits_every_residue() {
        let mut rng = BattleRng::from_seed(7);
        let mut seen = [false; 6];
        for _ in 0..1_000 {
            let v = rng.below(6);
            assert!(v < 6);
            seen[v as usize] = true;
        }
        assert!(seen.iter().all(|&hit| hit), "all residues reachable");
    }

    #[test]
    fn range_inclusive_covers_damage_roll_band() {
        // The damage pipeline rolls uniform 85..=100 (doc 02 §4).
        let mut rng = BattleRng::from_seed(4242);
        for _ in 0..1_000 {
            let roll = rng.range_inclusive(85, 100);
            assert!((85..=100).contains(&roll));
        }
    }

    #[test]
    fn range_inclusive_full_domain_does_not_overflow() {
        let mut rng = BattleRng::from_seed(11);
        let mut reference = BattleRng::from_seed(11);
        // The full-domain draw is exactly the raw stream — and must not
        // panic computing the span.
        assert_eq!(rng.range_inclusive(0, u32::MAX), reference.next_u32());
    }

    #[test]
    fn chance_extremes_are_certainties() {
        let mut rng = BattleRng::from_seed(5);
        assert!(!rng.chance(0, 100));
        assert!(rng.chance(100, 100));
    }
}
