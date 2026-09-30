//! WCA move notation and facelet permutations for a 3x3 cube.
//!
//! Facelet indices follow the Kociemba URFDLB layout, 0..54:
//! U = 0..9, R = 9..18, F = 18..27, D = 27..36, L = 36..45, B = 45..54.
//! Within each face the 9 stickers are laid out row-major:
//! ```text
//! 0 1 2
//! 3 4 5
//! 6 7 8
//! ```

use std::fmt;
use std::str::FromStr;

/// One of the six cube faces.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Face {
    U,
    R,
    F,
    D,
    L,
    B,
}

impl Face {
    pub const ALL: [Face; 6] = [Face::U, Face::R, Face::F, Face::D, Face::L, Face::B];

    pub fn as_char(self) -> char {
        match self {
            Face::U => 'U',
            Face::R => 'R',
            Face::F => 'F',
            Face::D => 'D',
            Face::L => 'L',
            Face::B => 'B',
        }
    }

    /// Index of this face in the URFDLB order (0..6).
    pub fn index(self) -> usize {
        match self {
            Face::U => 0,
            Face::R => 1,
            Face::F => 2,
            Face::D => 3,
            Face::L => 4,
            Face::B => 5,
        }
    }
}

/// How far a face is turned, clockwise when looking directly at that face.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Turn {
    /// Quarter turn clockwise (e.g. `R`).
    Cw,
    /// Half turn (e.g. `R2`).
    Double,
    /// Quarter turn counter-clockwise (e.g. `R'`).
    Ccw,
}

impl Turn {
    /// Number of clockwise quarter turns this represents.
    pub fn quarter_turns(self) -> u8 {
        match self {
            Turn::Cw => 1,
            Turn::Double => 2,
            Turn::Ccw => 3,
        }
    }

    pub fn suffix(self) -> &'static str {
        match self {
            Turn::Cw => "",
            Turn::Double => "2",
            Turn::Ccw => "'",
        }
    }
}

/// A single WCA move: a face plus a turn amount.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Move {
    pub face: Face,
    pub turn: Turn,
}

impl Move {
    pub fn new(face: Face, turn: Turn) -> Self {
        Move { face, turn }
    }

    /// The move that undoes this one.
    pub fn inverse(self) -> Move {
        let turn = match self.turn {
            Turn::Cw => Turn::Ccw,
            Turn::Double => Turn::Double,
            Turn::Ccw => Turn::Cw,
        };
        Move {
            face: self.face,
            turn,
        }
    }
}

impl fmt::Display for Move {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", self.face.as_char(), self.turn.suffix())
    }
}

/// Error returned when a move token cannot be parsed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MoveParseError {
    #[error("empty move token")]
    Empty,
    #[error("invalid face '{0}'")]
    InvalidFace(char),
    #[error("invalid modifier '{0}'")]
    InvalidModifier(char),
    #[error("trailing characters after move: '{0}'")]
    Trailing(String),
}

impl FromStr for Move {
    type Err = MoveParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut chars = s.chars();
        let face_ch = chars.next().ok_or(MoveParseError::Empty)?;
        let face = match face_ch {
            'U' => Face::U,
            'R' => Face::R,
            'F' => Face::F,
            'D' => Face::D,
            'L' => Face::L,
            'B' => Face::B,
            other => return Err(MoveParseError::InvalidFace(other)),
        };
        let turn = match chars.next() {
            None => Turn::Cw,
            Some('\'') => Turn::Ccw,
            Some('2') => Turn::Double,
            Some(other) => return Err(MoveParseError::InvalidModifier(other)),
        };
        let rest: String = chars.collect();
        if !rest.is_empty() {
            return Err(MoveParseError::Trailing(rest));
        }
        Ok(Move { face, turn })
    }
}

/// Parse a whitespace-separated sequence of moves, e.g. `"R U R' U'"`.
pub fn parse_sequence(s: &str) -> Result<Vec<Move>, MoveParseError> {
    s.split_whitespace().map(Move::from_str).collect()
}

/// Format a sequence of moves back to WCA notation.
pub fn format_sequence(moves: &[Move]) -> String {
    moves
        .iter()
        .map(|m| m.to_string())
        .collect::<Vec<_>>()
        .join(" ")
}

// Facelet permutations are derived from the cubie-level model (see
// `crate::cubie` and `crate::facelet`), so no hand-written facelet cycle table
// is needed here.
