//! Tiny shared overworld vocabulary.

use serde::{Deserialize, Serialize};

/// Cardinal facing for actors, warps, and saves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Facing {
    Up,
    Down,
    Left,
    Right,
}

impl Facing {
    /// Tile delta this facing looks toward.
    pub const fn delta(self) -> (i32, i32) {
        match self {
            Facing::Up => (0, 1),
            Facing::Down => (0, -1),
            Facing::Left => (-1, 0),
            Facing::Right => (1, 0),
        }
    }

    pub const fn opposite(self) -> Facing {
        match self {
            Facing::Up => Facing::Down,
            Facing::Down => Facing::Up,
            Facing::Left => Facing::Right,
            Facing::Right => Facing::Left,
        }
    }
}
