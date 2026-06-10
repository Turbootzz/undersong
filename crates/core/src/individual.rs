//! The persistent individual Mote (doc 02 §2 "Individual").
//!
//! This is the *saved* shape: content-referencing (move ids, not embedded
//! specs). The game layer resolves it against the loaded content pack to
//! build a `battle::BattleMote` at encounter time, and folds results back
//! afterwards. Shared vocabulary: `save` stores it, `game`/`tools`
//! convert it.

use serde::{Deserialize, Serialize};

use crate::ids::{ItemId, MoveId, SpeciesId};
use crate::moves::Ailment;
use crate::species::StatSpread;

/// A learned move slot as persisted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LearnedMove {
    pub id: MoveId,
    pub pp: u8,
    #[serde(default)]
    pub pp_ups: u8,
}

/// Doc 02 §2 Individual, field for field. UI flavor (Timbre/Practice/
/// Keyshifted) maps onto `ivs`/`evs`/`keyshifted` via string tables.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Individual {
    pub species: SpeciesId,
    pub level: u8,
    pub exp: u32,
    /// 0–31 per stat.
    pub ivs: StatSpread,
    /// 0–252 per stat, Σ ≤ 510.
    pub evs: StatSpread,
    /// 0..25 (doc 02 §3 grid index).
    pub nature: u8,
    #[serde(default)]
    pub ability_slot: u8,
    /// ≤ 4 moves.
    pub moves: Vec<LearnedMove>,
    #[serde(default)]
    pub status: Option<Ailment>,
    #[serde(default)]
    pub held_item: Option<ItemId>,
    /// 0–255.
    #[serde(default)]
    pub friendship: u8,
    /// Shiny analog; base odds 1/4096 (doc 02 §2).
    #[serde(default)]
    pub keyshifted: bool,
    /// Original trainer name.
    pub ot: String,
    #[serde(default)]
    pub nickname: Option<String>,
    /// Current HP; `None` = full (computed at resolve time).
    #[serde(default)]
    pub hp: Option<u16>,
    /// Hidden ability flag is per doc 02 §2's `abilities[1..2] + hidden?`;
    /// resolution happens against species data.
    #[serde(default)]
    pub uses_hidden_ability: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn individual_round_trips_through_ron() {
        let individual = Individual {
            species: "fanfyre".into(),
            level: 23,
            exp: 12_167,
            ivs: StatSpread {
                hp: 31,
                atk: 20,
                def: 25,
                spa: 31,
                spd: 25,
                spe: 31,
            },
            evs: StatSpread {
                hp: 0,
                atk: 0,
                def: 0,
                spa: 44,
                spd: 0,
                spe: 12,
            },
            nature: 10,
            ability_slot: 0,
            moves: vec![
                LearnedMove {
                    id: "ember_note".into(),
                    pp: 17,
                    pp_ups: 0,
                },
                LearnedMove {
                    id: "dampen".into(),
                    pp: 20,
                    pp_ups: 0,
                },
            ],
            status: Some(Ailment::Paralysis),
            held_item: None,
            friendship: 112,
            keyshifted: false,
            ot: "Junie".into(),
            nickname: Some("Brassbud".into()),
            hp: Some(51),
            uses_hidden_ability: false,
        };
        let text = ron::to_string(&individual).expect("serialize");
        let back: Individual = ron::from_str(&text).expect("deserialize");
        assert_eq!(back, individual);
    }
}
