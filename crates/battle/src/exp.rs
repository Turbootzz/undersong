//! Experience, leveling, and learnset events (doc 02 §9, v1.1 #11–14).

use undersong_core::ids::MoveId;

use crate::mote::BattleMote;

/// Base exp gain before participant split:
/// `ΔExp = floor(b · L_defeated / 7)`, then ×1.5 trainer / ×1.5 traded
/// (doc 02 §9). Returns the per-participant share for `participants`
/// splitting evenly (integer division).
pub fn exp_gain(
    defeated_base_yield: u16,
    defeated_level: u8,
    participants: u32,
    trainer_battle: bool,
    traded: bool,
) -> u32 {
    let base = u32::from(defeated_base_yield) * u32::from(defeated_level) / 7;
    let mut share = base / participants.max(1);
    if trainer_battle {
        share = share * 3 / 2;
    }
    if traded {
        share = share * 3 / 2;
    }
    share
}

pub struct LevelUp {
    pub new_level: u8,
    /// Moves the learnset offers at exactly this level.
    pub learnable: Vec<MoveId>,
}

/// Adds exp and processes any level-ups (stats recompute per v1.1 #11).
/// Returns one entry per level gained, in order.
pub fn apply_exp(mote: &mut BattleMote, amount: u32) -> Vec<LevelUp> {
    let mut ups = Vec::new();
    mote.exp = mote.exp.saturating_add(amount);
    while mote.level < 100 && mote.exp >= mote.growth.total_exp(mote.level + 1) {
        mote.level += 1;
        mote.recompute_stats();
        let learnable = mote
            .learnset
            .iter()
            .filter(|(level, _)| *level == mote.level)
            .map(|(_, id)| id.clone())
            .collect();
        ups.push(LevelUp {
            new_level: mote.level,
            learnable,
        });
    }
    ups
}

#[cfg(test)]
mod tests {
    use undersong_core::ids::SpeciesId;
    use undersong_core::species::{GrowthCurve, StatSpread};

    use super::*;
    use crate::mote::ZERO_SPREAD;

    fn leveler(level: u8) -> BattleMote {
        let base = StatSpread {
            hp: 50,
            atk: 50,
            def: 50,
            spa: 50,
            spd: 50,
            spe: 50,
        };
        let mut mote = BattleMote {
            species: SpeciesId::new("leveler"),
            name_key: "motif.leveler".into(),
            types: vec![undersong_core::types::Type::Feral],
            level,
            exp: GrowthCurve::MediumFast.total_exp(level),
            growth: GrowthCurve::MediumFast,
            nature: 0,
            base_stats: base,
            ivs: ZERO_SPREAD,
            evs: ZERO_SPREAD,
            stats: base,
            hp: 1,
            status: None,
            moves: vec![],
            catch_rate: 100,
            base_exp_yield: 100,
            ev_yield: vec![],
            learnset: vec![(6, "dampen".into()), (7, "gale_riff".into())],
        };
        mote.recompute_stats();
        mote.hp = mote.max_hp();
        mote
    }

    #[test]
    fn exp_gain_hand_vectors() {
        // b=142, L=13: floor(142·13/7) = floor(263.7) = 263
        assert_eq!(exp_gain(142, 13, 1, false, false), 263);
        // split between 2: floor(263/2)=131; trainer ×1.5 → 196
        assert_eq!(exp_gain(142, 13, 2, true, false), 196);
        // trainer + traded: 263 → ×1.5=394 → ×1.5=591
        assert_eq!(exp_gain(142, 13, 1, true, true), 591);
    }

    #[test]
    fn level_up_chain_collects_learnables() {
        let mut mote = leveler(5);
        // medium_fast: to reach 7 needs 343 total; at level 5 has 125.
        let ups = apply_exp(&mut mote, 343 - 125);
        assert_eq!(ups.len(), 2);
        assert_eq!(ups[0].new_level, 6);
        assert_eq!(ups[0].learnable, vec![MoveId::from("dampen")]);
        assert_eq!(ups[1].new_level, 7);
        assert_eq!(ups[1].learnable, vec![MoveId::from("gale_riff")]);
        assert_eq!(mote.level, 7);
    }

    #[test]
    fn level_up_raises_current_hp_by_max_delta() {
        let mut mote = leveler(5);
        let old_max = mote.max_hp();
        mote.hp = 3;
        apply_exp(&mut mote, 100);
        assert!(mote.level > 5);
        let delta = mote.max_hp() - old_max;
        assert_eq!(mote.hp, 3 + delta);
    }

    #[test]
    fn level_caps_at_100() {
        let mut mote = leveler(99);
        apply_exp(&mut mote, u32::MAX);
        assert_eq!(mote.level, 100);
    }
}
