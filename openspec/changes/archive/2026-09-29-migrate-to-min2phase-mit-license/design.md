# Design

## Context

`m2p-core` is used only in `crates/scramble/src/lib.rs`: `Tables::build(true)` (shared via `Arc`), `Solver::with_tables`, `tools::random_cube(&mut dyn RngCore)`, `Solver::solve(facelets, 21, 1_000_000, 0, INVERSE_SOLUTION)` and `tools::from_scramble` in a test. `tuibik-tui` only imports `Scrambler` and `Scramble`. The workspace has one git author (Valeria Luciel). `openspec/specs/` is empty, so both specs are new. Investigation of `min2phase` 0.2.x found: `solve` returns the solution only (no inverse/scramble output); `random_cube()`/`random_moves()` call `rand::thread_rng()` internally (no RNG parameter, not seedable); tables are global, built lazily in a `lazy_static` (no `Tables`/`Arc` handle). `cube::CubieCube` exposes public `cp`, `co`, `ep`, `eo`, and `cube::Move::inverse` exists.

## Goals / Non-Goals

**Goals:**
- Swap the solver dependency with zero public API change to `scramble`.
- Preserve the behaviours in the `scramble-generation` spec.
- Reach a GPL-free tree, then relicense.

**Non-Goals:**
- Copying code from `min2phase-rust` (RuiminYan); only the public `min2phase` API is used.
- Changing `tuibik-tui`, `cube`, or scramble length policy.
- Reimplementing missing solver features.

## Decisions

1. **Investigate API first (done).** Findings are in Context. Alternative of coding from docs alone was rejected.
2. **Inverse solution: invert by hand.** `min2phase` has no inverse output. Solve the state with `min2phase::solve`, parse the solution, reverse the order and apply `Move::inverse` to each move. This is a small pure function. `kewb` is discarded (unverified licence/features, would change the plan).
3. **Random state and RNG: our own seeded generator.** Do not use `min2phase::random_cube`/`random_moves`. Draw the state from the scrambler's `StdRng` as cubies: random permutations of `cp` and `ep` with equal parity (if parities differ, swap two edges); `co` with the last corner fixed so the sum is 0 mod 3; `eo` with the last edge fixed so the sum is 0 mod 2. A code comment explains why the result is uniform over reachable states (each constraint maps evenly onto the free choices). Convert with `Cube::to_facelet_str`. No code is copied from `min2phase`. Seeded output differs from the old crate; only same-seed reproducibility is required.
4. **Tables: global, warmed at construction.** No table handle is stored in `Scrambler`. `Scrambler::new()` and `with_seed()` run a warm-up `solve` to force the lazy global init so the first `generate()` has no latency. Measure `new()` time before and after. "Build once" holds because the state is global. `max_depth` stays 21.
5. **Tests.** Replace `tools::from_scramble` with `from_moves`/`apply_moves` and compare to `Cube::to_facelet_str`. Add: (a) every generated state is accepted by `min2phase::solve`; (b) round trip: applying the scramble to a solved cube reproduces the starting state; (c) same seed gives the same sequence; (d) parity and orientation invariants of the generator over many seeds; (e) a 16..=21 length bound (current lower bound is 15). If lengths fall outside 16..=21, report and do not adjust the range.
6. **Licence check.** Use `cargo metadata --format-version 1` and a `jq` filter on `license` fields; confirm `cargo tree -i m2p-core` fails and `Cargo.lock` has no entry.
7. **Relicense gate.** `git shortlog -sne` runs first; with one author, proceed. Copyright line: `Copyright (c) 2026 Valeria Luciel` (name from git, year from first commit); confirm with the user if unsure. Apache text from the official unmodified licence.
8. **Commits/branch.** Branch `migrate-min2phase`; commit 1 = dependency migration, commit 2 = licence change. Untracked `.claude/` and `openspec/` are not staged unless asked.

## Risks / Trade-offs

- [Own state generator mis-samples or produces unsolvable states] → Invariant tests (parity, orientation sums), acceptance by `solve`, round-trip test, uniformity comment reviewed.
- [Global lazy tables make `new()` block for a while] → Warm-up in the constructor; measure and report before/after.
- [Lengths outside 16..=21] → Report; do not adjust the range silently.
- [Facelet convention mismatch (URFDLB order, orientation)] → Convention test against `cube`; adapt the conversion in `scramble`, not `cube`.
- [Scramble length outside 16..=21 or slower table build/solve] → Length test over many seeds; measure `Scrambler::new()` time and report.
- [Seeded scrambles differ from before] → Acceptable; report in final summary.
- [Transitive licence surprises (e.g. MPL-2.0 `option-ext`)] → MPL-2.0 is file-level copyleft, compatible; document in the audit output.
- [Relicensing needs consent from other authors] → Only one author found; re-check at apply time.
