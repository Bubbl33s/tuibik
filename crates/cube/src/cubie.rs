//! Cubie-level 3x3 cube model: corner and edge permutation + orientation.
//!
//! This representation is the standard one used by Kociemba-style solvers and
//! is far less error-prone than hand-written 54-facelet cycles. Facelets for
//! rendering are derived from the cubie state via the canonical
//! `CORNER_FACELET` / `EDGE_FACELET` tables (see [`crate::facelet`]).
//!
//! Corner slots (URFDLB corner order):
//! `URF=0 UFL=1 ULB=2 UBR=3 DFR=4 DLF=5 DBL=6 DRB=7`
//! Edge slots:
//! `UR=0 UF=1 UL=2 UB=3 DR=4 DF=5 DL=6 DB=7 FR=8 FL=9 BL=10 BR=11`

use crate::moves::{Face, Move};

/// A cube as cubie permutation + orientation.
///
/// `cp[i]` = which corner cubie is in slot `i`; `co[i]` = its twist (0,1,2).
/// `ep[i]` = which edge cubie is in slot `i`; `eo[i]` = its flip (0,1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CubieCube {
    pub cp: [u8; 8],
    pub co: [u8; 8],
    pub ep: [u8; 12],
    pub eo: [u8; 12],
}

impl Default for CubieCube {
    fn default() -> Self {
        CubieCube::solved()
    }
}

impl CubieCube {
    pub fn solved() -> Self {
        CubieCube {
            cp: [0, 1, 2, 3, 4, 5, 6, 7],
            co: [0; 8],
            ep: [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11],
            eo: [0; 12],
        }
    }

    pub fn is_solved(&self) -> bool {
        *self == CubieCube::solved()
    }

    /// Multiply `self` by a move cube `m` (apply `m` after `self`): standard
    /// cubie composition with orientation addition.
    fn multiply(&self, m: &CubieCube) -> CubieCube {
        let mut r = CubieCube::solved();
        // Corners
        for i in 0..8 {
            let piece = m.cp[i] as usize;
            r.cp[i] = self.cp[piece];
            r.co[i] = (self.co[piece] + m.co[i]) % 3;
        }
        // Edges
        for i in 0..12 {
            let piece = m.ep[i] as usize;
            r.ep[i] = self.ep[piece];
            r.eo[i] = (self.eo[piece] + m.eo[i]) % 2;
        }
        r
    }

    pub fn apply_move(&mut self, mv: Move) {
        let base = base_move_cube(mv.face);
        for _ in 0..mv.turn.quarter_turns() {
            *self = self.multiply(&base);
        }
    }

    pub fn apply_sequence(&mut self, moves: &[Move]) {
        for &mv in moves {
            self.apply_move(mv);
        }
    }

    pub fn with_sequence(&self, moves: &[Move]) -> CubieCube {
        let mut c = self.clone();
        c.apply_sequence(moves);
        c
    }
}

/// The six basic clockwise quarter-turn move cubes, defined once and verified
/// by the crate tests (X^4 = identity, (R U R' U')^6 = identity, and
/// facelet cross-checks against the solver's own convention).
///
/// These are the canonical Kociemba move definitions.
fn base_move_cube(face: Face) -> CubieCube {
    // Data from Kociemba's reference CubieCube.moveCube[] (U, R, F, D, L, B).
    match face {
        Face::U => CubieCube {
            cp: [3, 0, 1, 2, 4, 5, 6, 7],
            co: [0, 0, 0, 0, 0, 0, 0, 0],
            ep: [3, 0, 1, 2, 4, 5, 6, 7, 8, 9, 10, 11],
            eo: [0; 12],
        },
        Face::R => CubieCube {
            cp: [4, 1, 2, 0, 7, 5, 6, 3],
            co: [2, 0, 0, 1, 1, 0, 0, 2],
            ep: [8, 1, 2, 3, 11, 5, 6, 7, 4, 9, 10, 0],
            eo: [0; 12],
        },
        Face::F => CubieCube {
            cp: [1, 5, 2, 3, 0, 4, 6, 7],
            co: [1, 2, 0, 0, 2, 1, 0, 0],
            ep: [0, 9, 2, 3, 4, 8, 6, 7, 1, 5, 10, 11],
            eo: [0, 1, 0, 0, 0, 1, 0, 0, 1, 1, 0, 0],
        },
        Face::D => CubieCube {
            cp: [0, 1, 2, 3, 5, 6, 7, 4],
            co: [0, 0, 0, 0, 0, 0, 0, 0],
            ep: [0, 1, 2, 3, 5, 6, 7, 4, 8, 9, 10, 11],
            eo: [0; 12],
        },
        Face::L => CubieCube {
            cp: [0, 2, 6, 3, 4, 1, 5, 7],
            co: [0, 1, 2, 0, 0, 2, 1, 0],
            ep: [0, 1, 10, 3, 4, 5, 9, 7, 8, 2, 6, 11],
            eo: [0; 12],
        },
        Face::B => CubieCube {
            cp: [0, 1, 3, 7, 4, 5, 2, 6],
            co: [0, 0, 1, 2, 0, 0, 2, 1],
            ep: [0, 1, 2, 11, 4, 5, 6, 10, 8, 9, 3, 7],
            eo: [0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 1, 1],
        },
    }
}
