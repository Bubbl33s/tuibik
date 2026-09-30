//! Convert a [`CubieCube`] to the 54-facelet URFDLB representation used for
//! rendering and for interop with the solver.
//!
//! The `CORNER_FACELET` / `EDGE_FACELET` tables are the canonical Kociemba
//! facelet-position tables (indices into the 54-facelet URFDLB layout).

use crate::cubie::CubieCube;

pub const FACELET_COUNT: usize = 54;

/// Facelet index of each of the 3 stickers of each corner slot.
/// Order: URF UFL ULB UBR DFR DLF DBL DRB.
const CORNER_FACELET: [[usize; 3]; 8] = [
    [8, 9, 20],   // URF: U9 R1 F3
    [6, 18, 38],  // UFL: U7 F1 L3
    [0, 36, 47],  // ULB: U1 L1 B3
    [2, 45, 11],  // UBR: U3 B1 R3
    [29, 26, 15], // DFR: D3 F9 R7
    [27, 44, 24], // DLF: D1 L9 F7
    [33, 53, 42], // DBL: D7 B9 L7
    [35, 17, 51], // DRB: D9 R9 B7
];

/// Facelet index of each of the 2 stickers of each edge slot.
/// Order: UR UF UL UB DR DF DL DB FR FL BL BR.
const EDGE_FACELET: [[usize; 2]; 12] = [
    [5, 10],  // UR: U6 R2
    [7, 19],  // UF: U8 F2
    [3, 37],  // UL: U4 L2
    [1, 46],  // UB: U2 B2
    [32, 16], // DR: D6 R8
    [28, 25], // DF: D2 F8
    [30, 43], // DL: D4 L8
    [34, 52], // DB: D8 B8
    [23, 12], // FR: F6 R4
    [21, 41], // FL: F4 L6
    [50, 39], // BL: B6 L4
    [48, 14], // BR: B4 R6
];

/// The face color (0..6, URFDLB) that the solved cube shows at a given facelet.
/// This is just `facelet / 9`, i.e. the center color of the face it belongs to.
#[inline]
fn facelet_color_of(index: usize) -> u8 {
    (index / 9) as u8
}

/// Produce the 54 facelet color indices (0..6) from a cubie cube.
#[allow(clippy::needless_range_loop)]
pub fn to_facelets(cc: &CubieCube) -> [u8; FACELET_COUNT] {
    let mut f = [0u8; FACELET_COUNT];
    // Centers are fixed.
    for i in 0..FACELET_COUNT {
        f[i] = facelet_color_of(i);
    }
    // Corners
    for slot in 0..8 {
        let piece = cc.cp[slot] as usize;
        let ori = cc.co[slot] as usize;
        for n in 0..3 {
            let dst = CORNER_FACELET[slot][(n + ori) % 3];
            let src = CORNER_FACELET[piece][n];
            f[dst] = facelet_color_of(src);
        }
    }
    // Edges
    for slot in 0..12 {
        let piece = cc.ep[slot] as usize;
        let ori = cc.eo[slot] as usize;
        for n in 0..2 {
            let dst = EDGE_FACELET[slot][(n + ori) % 2];
            let src = EDGE_FACELET[piece][n];
            f[dst] = facelet_color_of(src);
        }
    }
    f
}
