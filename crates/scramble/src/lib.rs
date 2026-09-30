//! `scramble` — WCA random-state scramble generation.
//!
//! Draws a uniformly random cube state from a seeded RNG, solves it with
//! `min2phase` (Kociemba two-phase, MIT) and inverts the solution to get the
//! move sequence that produces that state from a solved cube. This yields
//! competition-quality random-state scrambles.

use cube::{facelet, parse_sequence, Cube, CubieCube, Move};
use rand::rngs::StdRng;
use rand::{RngExt, SeedableRng};

/// A generated scramble: the parsed moves plus their WCA-notation text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scramble {
    pub moves: Vec<Move>,
    pub text: String,
}

impl Scramble {
    /// The cube state produced by applying this scramble to a solved cube.
    pub fn cube(&self) -> Cube {
        Cube::solved().with_sequence(&self.moves)
    }
}

/// Errors that can occur while generating a scramble.
#[derive(Debug, thiserror::Error)]
pub enum ScrambleError {
    #[error("solver failed: {0}")]
    Solver(String),
    #[error("could not parse solver output '{0}': {1}")]
    Parse(String, cube::MoveParseError),
}

/// Generates scrambles from a seedable RNG.
///
/// `min2phase` builds its lookup tables once, lazily, on first use and shares
/// them process-wide; `new`/`with_seed` trigger that build up front so the
/// first `generate` call is fast. Construct a `Scrambler` once and reuse it.
pub struct Scrambler {
    rng: StdRng,
    /// Max solver search depth; WCA-quality scrambles are typically <= 21.
    max_depth: u8,
}

impl Scrambler {
    /// Build a scrambler with tables ready and an OS-seeded RNG.
    pub fn new() -> Self {
        let mut rng = rand::rng();
        Self::from_rng(StdRng::from_rng(&mut rng))
    }

    /// Build a scrambler with a fixed RNG seed (deterministic, for tests).
    pub fn with_seed(seed: u64) -> Self {
        Self::from_rng(StdRng::seed_from_u64(seed))
    }

    fn from_rng(rng: StdRng) -> Self {
        // Force min2phase's global table construction now (solving a solved
        // cube is trivial once the tables exist).
        let _ = min2phase::solve(&Cube::solved().to_facelet_str(), 21);
        Scrambler { rng, max_depth: 21 }
    }

    /// Generate one WCA random-state scramble.
    pub fn generate(&mut self) -> Result<Scramble, ScrambleError> {
        // 1. Uniformly random solvable cube state as 54-char facelets.
        let state = random_state(&mut self.rng);
        let facelets = facelet_str(&state);

        // 2. Solve it, then invert the solution: the reversed sequence of
        //    inverted moves scrambles a solved cube into this state.
        let raw = min2phase::solve(&facelets, self.max_depth);
        if raw.starts_with("Error") {
            return Err(ScrambleError::Solver(raw));
        }
        let solution = parse_sequence(&raw).map_err(|e| ScrambleError::Parse(raw.clone(), e))?;
        let moves: Vec<Move> = solution.iter().rev().map(|m| m.inverse()).collect();
        let text = cube::format_sequence(&moves);

        Ok(Scramble { moves, text })
    }
}

impl Default for Scrambler {
    fn default() -> Self {
        Self::new()
    }
}

/// 54-char URFDLB facelet string for a cubie state.
fn facelet_str(cc: &CubieCube) -> String {
    const CH: [char; 6] = ['U', 'R', 'F', 'D', 'L', 'B'];
    facelet::to_facelets(cc)
        .iter()
        .map(|&f| CH[f as usize])
        .collect()
}

/// Uniformly random solvable cubie state.
fn random_state(rng: &mut StdRng) -> CubieCube {
    let mut cc = CubieCube::solved();
    let parity_c = shuffle(rng, &mut cc.cp);
    let parity_e = shuffle(rng, &mut cc.ep);
    // Corner permutation parity must equal edge permutation parity; swapping
    // two edges flips it (a bijection, so the result stays uniform).
    if parity_c != parity_e {
        cc.ep.swap(10, 11);
    }
    // Orientations: last piece is fixed by the twist/flip sum constraints.
    for i in 0..7 {
        cc.co[i] = rng.random_range(0..3);
    }
    cc.co[7] = (3 - cc.co[..7].iter().sum::<u8>() % 3) % 3;
    for i in 0..11 {
        cc.eo[i] = rng.random_range(0..2);
    }
    cc.eo[11] = cc.eo[..11].iter().sum::<u8>() % 2;
    cc
}

/// Fisher-Yates shuffle; returns the parity (0/1) of the applied permutation.
fn shuffle<const N: usize>(rng: &mut StdRng, a: &mut [u8; N]) -> u8 {
    let mut parity = 0;
    for i in 0..N - 1 {
        let j = rng.random_range(i..N);
        if i != j {
            a.swap(i, j);
            parity ^= 1;
        }
    }
    parity
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_valid_scramble() {
        let mut s = Scrambler::with_seed(42);
        let scramble = s.generate().expect("scramble");
        assert!(!scramble.moves.is_empty(), "scramble should have moves");
        // Applying the scramble to a solved cube should leave it unsolved.
        assert!(!scramble.cube().is_solved());
    }

    #[test]
    fn scramble_length_in_wca_range() {
        let mut s = Scrambler::with_seed(7);
        for _ in 0..20 {
            let sc = s.generate().unwrap();
            // Random-state solutions are 16..=21 moves.
            assert!(
                sc.moves.len() <= 21,
                "scramble too long: {} ({})",
                sc.moves.len(),
                sc.text
            );
            assert!(
                sc.moves.len() >= 16,
                "scramble suspiciously short: {}",
                sc.text
            );
        }
    }

    #[test]
    fn text_reparses_to_same_moves() {
        let mut s = Scrambler::with_seed(123);
        let sc = s.generate().unwrap();
        let reparsed = parse_sequence(&sc.text).unwrap();
        assert_eq!(reparsed, sc.moves);
    }

    #[test]
    fn scrambles_differ() {
        let mut s = Scrambler::with_seed(999);
        let a = s.generate().unwrap();
        let b = s.generate().unwrap();
        assert_ne!(a.text, b.text, "consecutive scrambles should differ");
    }

    #[test]
    fn cube_matches_min2phase_from_moves() {
        // Regression: our cubie->facelet conversion must be byte-identical to
        // min2phase's own scramble application, across several scrambles.
        let mut s = Scrambler::with_seed(2024);
        for _ in 0..10 {
            let sc = s.generate().unwrap();
            let our = sc.cube().to_facelet_str();
            let theirs = min2phase::from_moves(&sc.text).expect("min2phase parses scramble");
            assert_eq!(our, theirs, "mismatch for scramble '{}'", sc.text);
        }
    }

    #[test]
    fn cube_facelets_match_solver_state() {
        // Our facelet string must be accepted by min2phase and solve.
        let mut s = Scrambler::with_seed(2024);
        let sc = s.generate().unwrap();
        let our_facelets = sc.cube().to_facelet_str();
        let solution = min2phase::solve(&our_facelets, 21);
        assert!(
            !solution.starts_with("Error"),
            "solver rejected our facelet string '{}': {}",
            our_facelets,
            solution
        );
    }

    #[test]
    fn same_seed_same_scrambles() {
        let mut a = Scrambler::with_seed(5);
        let mut b = Scrambler::with_seed(5);
        for _ in 0..5 {
            assert_eq!(a.generate().unwrap(), b.generate().unwrap());
        }
    }

    #[test]
    fn length_and_unsolved_over_many_seeds() {
        for seed in 0..100 {
            let sc = Scrambler::with_seed(seed).generate().unwrap();
            assert!(
                (16..=21).contains(&sc.moves.len()),
                "seed {seed}: {} moves ({})",
                sc.moves.len(),
                sc.text
            );
            assert!(!sc.cube().is_solved(), "seed {seed} produced solved cube");
        }
    }
}
