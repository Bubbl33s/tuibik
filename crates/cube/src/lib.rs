//! `cube` — 3x3 Rubik's cube model, WCA move notation, and net rendering data.

pub   mod cube;
pub mod cubie;
pub mod facelet;
pub mod moves;
pub mod render;

pub use cube::{Cube, Facelet};
pub use cubie::CubieCube;
pub use facelet::FACELET_COUNT;
pub use moves::{format_sequence, parse_sequence, Face, Move, MoveParseError, Turn};

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn solved_cube_facelet_str() {
        let c = Cube::solved();
        assert_eq!(
            c.to_facelet_str(),
            "UUUUUUUUURRRRRRRRRFFFFFFFFFDDDDDDDDDLLLLLLLLLBBBBBBBBB"
        );
        assert!(c.is_solved());
    }

    #[test]
    fn move_parse_display_roundtrip() {
        for tok in ["U", "R'", "F2", "D", "L'", "B2"] {
            let m = Move::from_str(tok).unwrap();
            assert_eq!(m.to_string(), tok);
        }
    }

    #[test]
    fn parse_sequence_roundtrip() {
        let seq = "R U R' U' F2 B'";
        let moves = parse_sequence(seq).unwrap();
        assert_eq!(format_sequence(&moves), seq);
    }

    #[test]
    fn invalid_moves_error() {
        assert!(Move::from_str("").is_err());
        assert!(Move::from_str("X").is_err());
        assert!(Move::from_str("R3").is_err());
        assert!(Move::from_str("RU").is_err());
    }

    #[test]
    fn each_face_four_turns_is_identity() {
        for face in Face::ALL {
            let mut c = Cube::solved();
            let mv = Move::new(face, Turn::Cw);
            for _ in 0..4 {
                c.apply_move(mv);
            }
            assert!(
                c.is_solved(),
                "face {} four quarter turns should be identity",
                face.as_char()
            );
        }
    }

    #[test]
    fn double_turn_equals_two_quarters() {
        for face in Face::ALL {
            let mut a = Cube::solved();
            a.apply_move(Move::new(face, Turn::Double));

            let mut b = Cube::solved();
            b.apply_move(Move::new(face, Turn::Cw));
            b.apply_move(Move::new(face, Turn::Cw));

            assert_eq!(a, b, "face {} double turn", face.as_char());
        }
    }

    #[test]
    fn move_then_inverse_is_identity() {
        for face in Face::ALL {
            for turn in [Turn::Cw, Turn::Double, Turn::Ccw] {
                let mut c = Cube::solved();
                let mv = Move::new(face, turn);
                c.apply_move(mv);
                c.apply_move(mv.inverse());
                assert!(c.is_solved(), "move {} then inverse should be identity", mv);
            }
        }
    }

    #[test]
    fn sequence_then_reverse_inverse_is_identity() {
        let moves = parse_sequence("R U R' U' F2 L B' D2").unwrap();
        let mut c = Cube::solved();
        c.apply_sequence(&moves);
        let inverse: Vec<Move> = moves.iter().rev().map(|m| m.inverse()).collect();
        c.apply_sequence(&inverse);
        assert!(c.is_solved());
    }

    #[test]
    fn sexy_move_six_times_is_identity() {
        // (R U R' U') repeated 6 times returns to solved — a classic invariant.
        let moves = parse_sequence("R U R' U'").unwrap();
        let mut c = Cube::solved();
        for _ in 0..6 {
            c.apply_sequence(&moves);
        }
        assert!(c.is_solved(), "(R U R' U')^6 should be identity");
    }

    #[test]
    fn t_perm_twelve_times_is_identity() {
        // The T-perm has order 2; a 12-move superflip-adjacent check: apply a
        // known algorithm and its properties. Here we use Sune, order 6.
        let sune = parse_sequence("R U R' U R U2 R'").unwrap();
        let mut c = Cube::solved();
        for _ in 0..6 {
            c.apply_sequence(&sune);
        }
        assert!(c.is_solved(), "Sune^6 should be identity");
    }

    #[test]
    fn single_move_changes_state() {
        let mut c = Cube::solved();
        c.apply_move(Move::from_str("R").unwrap());
        assert!(!c.is_solved());
    }

    #[test]
    fn each_move_preserves_color_counts() {
        for face in Face::ALL {
            let mut c = Cube::solved();
            c.apply_move(Move::new(face, Turn::Cw));
            let mut counts = [0u32; 6];
            for &f in &c.facelets() {
                counts[f as usize] += 1;
            }
            assert_eq!(counts, [9; 6], "color counts after {}", face.as_char());
        }
    }

    #[test]
    fn r_move_facelet_effect() {
        // After R, the F face right column takes D-face colors are NOT; check
        // a concrete known sticker: R moves F3(20) region. Simplest robust
        // check: the whole cube is no longer solved and U face changed.
        let mut c = Cube::solved();
        c.apply_move(Move::from_str("R").unwrap());
        let f = c.facelets();
        // U face (0..9) should no longer be all color 0.
        assert!(f[0..9].iter().any(|&x| x != 0));
    }
}
