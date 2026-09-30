//! Public `Cube` type: a cubie-level state with a facelet view for rendering.

use crate::cubie::CubieCube;
use crate::facelet::{self, FACELET_COUNT};
use crate::moves::Move;

pub type Facelet = u8;
pub use crate::facelet::FACELET_COUNT as FACELETS;

/// A 3x3 cube. Internally a cubie permutation/orientation state; exposes a
/// 54-facelet color view (URFDLB order) for rendering and solver interop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cube {
    inner: CubieCube,
}

impl Default for Cube {
    fn default() -> Self {
        Cube::solved()
    }
}

impl Cube {
    pub fn solved() -> Self {
        Cube {
            inner: CubieCube::solved(),
        }
    }

    pub fn is_solved(&self) -> bool {
        self.inner.is_solved()
    }

    pub fn apply_move(&mut self, mv: Move) {
        self.inner.apply_move(mv);
    }

    pub fn apply_sequence(&mut self, moves: &[Move]) {
        self.inner.apply_sequence(moves);
    }

    pub fn with_sequence(&self, moves: &[Move]) -> Cube {
        let mut c = self.clone();
        c.apply_sequence(moves);
        c
    }

    /// The 54 facelet color indices (0..6, URFDLB order).
    pub fn facelets(&self) -> [Facelet; FACELET_COUNT] {
        facelet::to_facelets(&self.inner)
    }

    /// Render as a 54-char facelet string in URFDLB order (`U R F D L B`).
    pub fn to_facelet_str(&self) -> String {
        const CH: [char; 6] = ['U', 'R', 'F', 'D', 'L', 'B'];
        self.facelets().iter().map(|&f| CH[f as usize]).collect()
    }

    /// Access the underlying cubie state (for solver interop).
    pub fn cubie(&self) -> &CubieCube {
        &self.inner
    }
}
