//! `BattleState` — plain, serde-serializable, cloneable data
//! (docs/03-ARCHITECTURE.md §2).

use serde::{Deserialize, Serialize};
use undersong_core::chart::TypeChart;
use undersong_core::moves::WeatherKind;

use crate::events::{Outcome, SideId};
use crate::mote::BattleMote;
use crate::stats::Stages;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BattleKind {
    /// Side 1 is a single wild Mote; bells and escapes are legal.
    Wild,
    /// Full trainer battle; no bells, no running (doc 02 §12).
    Trainer,
}

/// Per-active-position volatile state, cleared when the position's Mote
/// leaves the field (doc 02 §5: volatiles do not persist through switch).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActiveState {
    pub stages: Stages,
    /// Remaining confusion turns; 0 = not confused.
    pub confusion: u8,
    pub flinched: bool,
    pub seeded: bool,
    pub trapped: bool,
    /// Mid two-turn move: the committed move slot.
    pub charging: Option<u8>,
    pub protected: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Side {
    pub party: Vec<BattleMote>,
    /// Index into `party`.
    pub active: u8,
    pub active_state: ActiveState,
}

impl Side {
    pub fn new(party: Vec<BattleMote>) -> Self {
        Self {
            party,
            active: 0,
            active_state: ActiveState::default(),
        }
    }

    pub fn active_mote(&self) -> &BattleMote {
        &self.party[usize::from(self.active)]
    }

    pub fn active_mote_mut(&mut self) -> &mut BattleMote {
        &mut self.party[usize::from(self.active)]
    }

    pub fn has_conscious(&self) -> bool {
        self.party.iter().any(|m| !m.is_fainted())
    }

    /// First non-fainted, non-active party index, if any.
    pub fn first_replacement(&self) -> Option<u8> {
        self.party
            .iter()
            .enumerate()
            .find(|(i, m)| *i != usize::from(self.active) && !m.is_fainted())
            .map(|(i, _)| u8::try_from(i).expect("party <= 6"))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BattleState {
    pub kind: BattleKind,
    pub sides: [Side; 2],
    /// `(kind, remaining_turns)`; weather lasts 5 turns (doc 02 §7).
    pub weather: Option<(WeatherKind, u8)>,
    pub turn: u16,
    pub outcome: Option<Outcome>,
    /// Failed escape attempts so far (doc 02 §12: +30 per attempt).
    pub escape_attempts: u8,
    /// Embedded chart: replays must reproduce even if content changes.
    pub chart: TypeChart,
}

impl BattleState {
    pub fn new(
        kind: BattleKind,
        side0: Vec<BattleMote>,
        side1: Vec<BattleMote>,
        chart: TypeChart,
    ) -> Self {
        Self {
            kind,
            sides: [Side::new(side0), Side::new(side1)],
            weather: None,
            turn: 0,
            outcome: None,
            escape_attempts: 0,
            chart,
        }
    }

    pub fn side(&self, id: SideId) -> &Side {
        &self.sides[usize::from(id)]
    }

    pub fn side_mut(&mut self, id: SideId) -> &mut Side {
        &mut self.sides[usize::from(id)]
    }

    pub fn is_over(&self) -> bool {
        self.outcome.is_some()
    }
}
