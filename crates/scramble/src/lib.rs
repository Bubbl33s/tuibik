//! `scramble` — WCA random-state scramble generation.
//!
//! Uses `m2p-core` (Kociemba two-phase, GPL-3.0) to generate a uniformly
//! random cube state and then compute the move sequence that produces it from
//! a solved cube (the inverse of the solver's solution). This yields
//! competition-quality random-state scrambles.

use std::sync::Arc;

use cube::{parse_sequence, Cube, Move};
use m2p_core::{tools, Solver, Tables};
use rand::rngs::StdRng;
use rand::{RngCore, SeedableRng};

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

/// Holds the precomputed Kociemba tables (built once) and generates scrambles.
///
/// `Tables::build` takes ~100ms, so construct a `Scrambler` once and reuse it.
pub struct Scrambler {
    tables: Arc<Tables>,
    rng: StdRng,
    /// Max solver search depth; WCA-quality scrambles are typically <= 21.
    max_depth: i32,
}

impl Scrambler {
    /// Build a scrambler with freshly computed tables and OS-seeded RNG.
    pub fn new() -> Self {
        Scrambler {
            tables: Arc::new(Tables::build(true)),
            rng: StdRng::from_entropy(),
            max_depth: 21,
        }
    }

    /// Build a scrambler with a fixed RNG seed (deterministic, for tests).
    pub fn with_seed(seed: u64) -> Self {
        Scrambler {
            tables: Arc::new(Tables::build(true)),
            rng: StdRng::seed_from_u64(seed),
            max_depth: 21,
        }
    }

    /// Generate one WCA random-state scramble.
    pub fn generate(&mut self) -> Result<Scramble, ScrambleError> {
        // 1. Uniformly random solvable cube state as 54-char facelets.
        let facelets = {
            let rng: &mut dyn RngCore = &mut self.rng;
            tools::random_cube(rng)
        };

        // 2. Ask the solver for the sequence that SCRAMBLES a solved cube into
        //    this state — i.e. the inverse of the solution — via the
        //    INVERSE_SOLUTION verbose flag.
        let mut solver = Solver::with_tables(self.tables.clone());
        let raw = solver
            .solve(
                &facelets,
                self.max_depth,
                1_000_000, // probe_max
                0,         // probe_min
                m2p_core::verbose::INVERSE_SOLUTION,
            )
            .map_err(|e| ScrambleError::Solver(e.to_string()))?;

        let text = normalize(&raw);
        let moves = parse_sequence(&text).map_err(|e| ScrambleError::Parse(text.clone(), e))?;

        Ok(Scramble { moves, text })
    }
}

impl Default for Scrambler {
    fn default() -> Self {
        Self::new()
    }
}

/// Normalize solver output spacing: the solver emits tokens like `"U "`, `"R2"`,
/// `"F'"` sometimes separated by extra spaces. Collapse to single spaces.
fn normalize(raw: &str) -> String {
    raw.split_whitespace().collect::<Vec<_>>().join(" ")
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
            // Random-state solutions are typically 16..=21 moves; allow a small
            // margin. They must never exceed max_depth.
            assert!(
                sc.moves.len() <= 21,
                "scramble too long: {} ({})",
                sc.moves.len(),
                sc.text
            );
            assert!(
                sc.moves.len() >= 15,
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
    fn cube_matches_m2p_from_scramble() {
        // Regression: our cubie->facelet conversion must be byte-identical to
        // m2p-core's own scramble application, across several scrambles.
        let mut s = Scrambler::with_seed(2024);
        for _ in 0..10 {
            let sc = s.generate().unwrap();
            let our = sc.cube().to_facelet_str();
            let m2p_state = tools::from_scramble(&sc.text, &s.tables);
            assert_eq!(our, m2p_state, "mismatch for scramble '{}'", sc.text);
        }
    }

    #[test]
    fn cube_facelets_match_solver_state() {
        // Cross-check: our cubie->facelet convention must match m2p-core's.
        // Generate a scramble, apply it with OUR cube, and confirm the solver
        // agrees the resulting state is solvable back to solved by re-solving.
        let mut s = Scrambler::with_seed(2024);
        let sc = s.generate().unwrap();
        let our_facelets = sc.cube().to_facelet_str();

        // The solver must accept our facelet string and solve it (length >= 0).
        let mut solver = Solver::with_tables(s.tables.clone());
        let solution = solver.solve(&our_facelets, 21, 1_000_000, 0, 0);
        assert!(
            solution.is_ok(),
            "solver rejected our facelet string '{}': {:?}",
            our_facelets,
            solution.err()
        );
    }
}
