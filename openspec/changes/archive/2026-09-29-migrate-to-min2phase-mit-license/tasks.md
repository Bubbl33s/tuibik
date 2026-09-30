# Tasks

## 1. Baseline

- [x] 1.1 Create branch `migrate-min2phase` and verify with `git branch --show-current`
- [x] 1.2 Run `cargo test --workspace` before any edit and record pass/fail counts as the baseline
- [x] 1.3 Record the current `Scrambler::new()` time and a few seeded scrambles for the before/after comparison

## 2. API investigation

- [x] 2.1 Fetch `min2phase` 0.2.x (`cargo add` in scratch or `cargo fetch`) and read its source and `examples/` in the cargo registry; write down signatures of `solve`, `apply_moves`, `from_moves`, `random_cube`, `random_moves`
- [x] 2.2 Record the decisions (manual inversion, own seeded generator, global warmed tables) for the final summary; verify no code is copied from `min2phase`

## 3. Dependency migration

- [x] 3.1 In `crates/scramble/Cargo.toml` replace the `m2p-core` git dependency with `min2phase` and update `description`; verify `cargo build -p scramble` compiles the manifest
- [x] 3.2 Add a random-cubie generator driven by `StdRng` (`cp`/`ep` permutations with equal parity, `co` sum 0 mod 3, `eo` sum 0 mod 2, uniformity comment) and verify with a test of the parity and orientation invariants over many seeds
- [x] 3.3 Add the solution-inversion step (reverse order + `Move::inverse`) and verify with a round-trip test: applying the result to a solved cube gives the starting state
- [x] 3.4 Rewrite `crates/scramble/src/lib.rs` (module docs, imports, `Scrambler`, `new`, `with_seed` with warm-up `solve`, `generate`) keeping the public API identical; verify `cargo build --workspace` succeeds with `tuibik-tui` untouched
- [x] 3.5 Rewrite `cube_matches_m2p_from_scramble` (rename, use `from_moves`/`apply_moves`) and `cube_facelets_match_solver_state`; verify both pass
- [x] 3.6 Add tests: generated states are always solvable, same seed gives same sequence, scrambles never solved, length within 16..=21 over many seeds (if outside, STOP and report; do not adjust the range); verify `cargo test --workspace` passes
- [x] 3.7 Run `cargo clippy --workspace` and verify no new warnings
- [x] 3.8 Measure `Scrambler::new()` time after the change and compare with 1.3; run the app and verify it generates scrambles (via the `run` skill / tmux capture)

## 4. GPL-free verification

- [x] 4.1 Verify `Cargo.lock` has no `m2p-core` and `cargo tree -i m2p-core` errors
- [x] 4.2 List licences with `cargo metadata` and verify no GPL/LGPL/AGPL outside workspace crates
- [x] 4.3 Commit the migration (message ending with the required Co-Authored-By line) and verify with `git log -1 --stat`

## 5. Relicense (only if section 4 passed)

- [x] 5.1 Run `git shortlog -sne`; if any author other than the owner appears, STOP and ask before continuing
- [x] 5.2 Confirm copyright name/year (default `Valeria Luciel`, 2026) with the user if uncertain
- [x] 5.3 Set workspace `license = "MIT OR Apache-2.0"`; verify `cargo metadata` shows it for all workspace crates
- [x] 5.4 `git rm LICENSE`; add `LICENSE-MIT` and `LICENSE-APACHE`; verify files exist and MIT has the right holder and year
- [x] 5.5 Rewrite the README licence section (both licences, links, contribution clause "Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.") and remove `m2p-core`/GPL mentions at ~lines 7, 38, 104, 111; verify with `grep -n -i "m2p\|gpl" README.md` returning nothing
- [x] 5.6 Run `cargo build --workspace && cargo test --workspace`, commit the licence change (no push), and verify with `git log --oneline -3`

## 6. Report

- [x] 6.1 Give the user a summary: files changed, before/after (test counts, `Scrambler::new()` time, sample scrambles), and any behaviour differences
