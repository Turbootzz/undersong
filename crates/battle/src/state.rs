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

/// Battle format (doc 02 v1.5 #2). Wild battles are always `Single` at
/// launch — `BattleState::new_double` asserts `Trainer`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Format {
    #[default]
    Single,
    Double,
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
    /// `Fainted` was already emitted for the Mote currently at this
    /// position (reset on switch-in). Replaces event-stream scanning so
    /// a fainted doubles position with an empty bench is not re-announced.
    #[serde(default)]
    pub fainted_emitted: bool,
}

/// One on-field battle position: which party member stands there plus its
/// volatile state. Singles fields one per side, doubles up to two.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Position {
    /// Index into the side's `party`.
    pub party_index: u8,
    pub state: ActiveState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Side {
    pub party: Vec<BattleMote>,
    /// On-field positions, in slot order. A fainted position with an
    /// empty bench keeps its fainted Mote and is skipped everywhere via
    /// `is_fainted` checks (doc 02 v1.5 #2 retarget/fizzle handles it).
    pub positions: Vec<Position>,
}

impl Side {
    /// Singles: field party index 0.
    pub fn new(party: Vec<BattleMote>) -> Self {
        Self {
            party,
            positions: vec![Position::default()],
        }
    }

    /// Doubles: field the first `min(2, conscious)` conscious party
    /// members. A fully fainted party still fields index 0 so position
    /// accessors stay total.
    pub fn new_double(party: Vec<BattleMote>) -> Self {
        let mut positions: Vec<Position> = party
            .iter()
            .enumerate()
            .filter(|(_, m)| !m.is_fainted())
            .take(2)
            .map(|(i, _)| Position {
                party_index: u8::try_from(i).expect("party ≤ 6"),
                state: ActiveState::default(),
            })
            .collect();
        if positions.is_empty() {
            positions.push(Position::default());
        }
        Self { party, positions }
    }

    /// Position 0's Mote (the only one in singles).
    pub fn active_mote(&self) -> &BattleMote {
        self.mote_at(0)
    }

    pub fn active_mote_mut(&mut self) -> &mut BattleMote {
        self.mote_at_mut(0)
    }

    /// The Mote standing at `position`.
    pub fn mote_at(&self, position: u8) -> &BattleMote {
        &self.party[usize::from(self.positions[usize::from(position)].party_index)]
    }

    pub fn mote_at_mut(&mut self, position: u8) -> &mut BattleMote {
        let index = usize::from(self.positions[usize::from(position)].party_index);
        &mut self.party[index]
    }

    pub fn position_count(&self) -> u8 {
        u8::try_from(self.positions.len()).expect("≤ 2 positions")
    }

    /// Is this party index currently standing in any position?
    pub fn is_fielded(&self, party_index: u8) -> bool {
        self.positions.iter().any(|p| p.party_index == party_index)
    }

    pub fn has_conscious(&self) -> bool {
        self.party.iter().any(|m| !m.is_fainted())
    }

    /// First non-fainted party index not currently fielded, if any.
    pub fn first_replacement(&self) -> Option<u8> {
        self.party
            .iter()
            .enumerate()
            .map(|(i, m)| (u8::try_from(i).expect("party <= 6"), m))
            .find(|(i, m)| !self.is_fielded(*i) && !m.is_fainted())
            .map(|(i, _)| i)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BattleState {
    pub kind: BattleKind,
    /// Single or double (doc 02 v1.5 #2); default keeps pre-doubles
    /// fixtures parseable.
    #[serde(default)]
    pub format: Format,
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
            format: Format::Single,
            sides: [Side::new(side0), Side::new(side1)],
            weather: None,
            turn: 0,
            outcome: None,
            escape_attempts: 0,
            chart,
        }
    }

    /// A doubles battle: each side fields `min(2, conscious)` Motes.
    /// Wild doubles don't exist at launch (doc 02 v1.5 #2), so `kind`
    /// must be `Trainer`.
    pub fn new_double(
        kind: BattleKind,
        side0: Vec<BattleMote>,
        side1: Vec<BattleMote>,
        chart: TypeChart,
    ) -> Self {
        assert!(
            matches!(kind, BattleKind::Trainer),
            "wild battles are always Single at launch (doc 02 v1.5 #2)"
        );
        Self {
            kind,
            format: Format::Double,
            sides: [Side::new_double(side0), Side::new_double(side1)],
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
