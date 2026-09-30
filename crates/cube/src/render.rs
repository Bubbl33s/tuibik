//! Facelet-to-color mapping and unfolded-net layout for rendering.
//!
//! Colors are the six standard WCA cube colors, keyed by face index
//! (URFDLB order). The renderer (in the TUI crate) turns these into
//! truecolor cells.

use crate::cube::Cube;
use crate::moves::Face;

/// An RGB color triple.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Rgb { r, g, b }
    }
}

/// Standard WCA color scheme, indexed by `Face::index()`.
/// U = white, R = red, F = green, D = yellow, L = orange, B = blue.
pub const WCA_COLORS: [Rgb; 6] = [
    Rgb::new(0xEE, 0xEE, 0xEE), // U white
    Rgb::new(0xD1, 0x1D, 0x1D), // R red
    Rgb::new(0x1F, 0xB8, 0x3A), // F green
    Rgb::new(0xF2, 0xD1, 0x1B), // D yellow
    Rgb::new(0xF2, 0x8C, 0x1B), // L orange
    Rgb::new(0x1B, 0x54, 0xF2), // B blue
];

/// A cell in the unfolded net: either a sticker with a color index, or a gap.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetCell {
    /// A sticker holding a facelet color index (0..6) plus its absolute
    /// facelet index (0..54) for highlighting.
    Sticker { color: u8, facelet: usize },
    /// Empty space in the cross-shaped layout.
    Gap,
}

/// The unfolded net is a 9-tall by 12-wide grid arranged as a cross:
/// ```text
///       U U U
///       U U U
///       U U U
/// L L L F F F R R R B B B
/// L L L F F F R R R B B B
/// L L L F F F R R R B B B
///       D D D
///       D D D
///       D D D
/// ```
pub const NET_ROWS: usize = 9;
pub const NET_COLS: usize = 12;

/// Build the unfolded net grid for a cube. Each `Sticker` carries the color
/// index (for theme lookup) and its absolute facelet index.
pub fn net(cube: &Cube) -> Vec<Vec<NetCell>> {
    let facelets = cube.facelets();
    let mut grid = vec![vec![NetCell::Gap; NET_COLS]; NET_ROWS];

    fn place(grid: &mut [Vec<NetCell>], facelets: &[u8], face: Face, row0: usize, col0: usize) {
        let base = face.index() * 9;
        for r in 0..3 {
            for c in 0..3 {
                let fl = base + r * 3 + c;
                grid[row0 + r][col0 + c] = NetCell::Sticker {
                    color: facelets[fl],
                    facelet: fl,
                };
            }
        }
    }

    place(&mut grid, &facelets, Face::U, 0, 3);
    place(&mut grid, &facelets, Face::L, 3, 0);
    place(&mut grid, &facelets, Face::F, 3, 3);
    place(&mut grid, &facelets, Face::R, 3, 6);
    place(&mut grid, &facelets, Face::B, 3, 9);
    place(&mut grid, &facelets, Face::D, 6, 3);

    grid
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cube::Cube;
    use crate::moves::Move;
    use std::str::FromStr;

    #[test]
    fn solved_net_each_face_single_color() {
        let cube = Cube::solved();
        let grid = net(&cube);
        // Collect stickers per face color; each of the 6 faces must be uniform.
        // U region rows 0..3 cols 3..6
        let regions: [(usize, usize, u8); 6] = [
            (0, 3, 0), // U
            (3, 0, 4), // L
            (3, 3, 2), // F
            (3, 6, 1), // R
            (3, 9, 5), // B
            (6, 3, 3), // D
        ];
        for (row0, col0, expected) in regions {
            for r in 0..3 {
                for c in 0..3 {
                    match grid[row0 + r][col0 + c] {
                        NetCell::Sticker { color, .. } => {
                            assert_eq!(color, expected, "face at ({row0},{col0})");
                        }
                        NetCell::Gap => panic!("expected sticker at ({},{})", row0 + r, col0 + c),
                    }
                }
            }
        }
    }

    #[test]
    fn net_dimensions() {
        let cube = Cube::solved();
        let grid = net(&cube);
        assert_eq!(grid.len(), NET_ROWS);
        assert!(grid.iter().all(|row| row.len() == NET_COLS));
    }

    #[test]
    #[allow(clippy::needless_range_loop)]
    fn corners_of_cross_are_gaps() {
        let cube = Cube::solved();
        let grid = net(&cube);
        // Top-left 3x3 corner (rows 0..3, cols 0..3) is empty in the cross.
        for r in 0..3 {
            for c in 0..3 {
                assert_eq!(grid[r][c], NetCell::Gap);
            }
        }
    }

    #[test]
    fn move_changes_some_net_stickers() {
        let solved = net(&Cube::solved());
        let mut cube = Cube::solved();
        cube.apply_move(Move::from_str("R").unwrap());
        let moved = net(&cube);
        assert_ne!(solved, moved, "R should change the rendered net");
    }

    #[test]
    #[allow(clippy::needless_range_loop)]
    fn wca_colors_are_distinct() {
        for i in 0..6 {
            for j in (i + 1)..6 {
                assert_ne!(WCA_COLORS[i], WCA_COLORS[j]);
            }
        }
    }
}
