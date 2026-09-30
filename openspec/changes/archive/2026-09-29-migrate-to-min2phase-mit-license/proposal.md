# Proposal

## Why

`tuibik` links `m2p-core` (GPL-3.0-or-later), which forces the whole project to be GPL. It is the only GPL dependency in the tree. Replacing it with `min2phase` (MIT, crates.io, cs0x7f) removes that constraint and lets the project adopt the permissive, Rust-ecosystem-standard `MIT OR Apache-2.0` licence.

## What Changes

- Replace the git dependency on `RuiminYan/min2phase-rust` (`m2p-core`) with the `min2phase` crate from crates.io in `crates/scramble`, and update the crate `description`.
- Adapt `crates/scramble/src/lib.rs` to the `min2phase` API while keeping the public API (`Scrambler`, `Scramble`, `ScrambleError`, `generate`, `with_seed`) unchanged. No `tuibik-tui` changes expected. Because `min2phase` has no inverse solution and no seedable RNG, the scrambler draws a random cube state from its own seeded `StdRng`, solves it with `min2phase::solve`, and inverts the solution by hand (reverse order + `Move::inverse`). Its lazily-built global tables are forced at construction with a warm-up solve.
- Rewrite the tests that use `m2p-core` (`cube_matches_m2p_from_scramble`, `cube_facelets_match_solver_state`) against `min2phase`.
- Verify `m2p-core` is gone from `Cargo.lock` and no GPL/LGPL/AGPL licence remains outside our crates.
- Only after that passes and the contributor check is clean: relicense to `MIT OR Apache-2.0` (workspace `license`, `LICENSE` → `LICENSE-MIT` + `LICENSE-APACHE`, README licence section and `m2p-core` mentions, standard Rust contribution clause).
- Work on branch `migrate-min2phase`, two commits (dependency migration, licence change), no push.

Stop conditions: if `git shortlog -sne` shows authors other than the owner, stop and ask before relicensing; if scramble lengths fall outside 16..=21, report it and do not adjust the range. (The missing inverse solution was found during investigation and resolved by manual inversion; `kewb` is discarded.)

## Capabilities

### New Capabilities
- `scramble-generation`: contract for WCA-style random-state scramble generation (validity, length, determinism by seed, table reuse). Written to pin behaviour that must not change across the migration.
- `project-licensing`: the project is dual-licensed `MIT OR Apache-2.0`, with no copyleft dependencies.

### Modified Capabilities
<!-- none: openspec/specs is empty -->

## Impact

- Code: `crates/scramble/Cargo.toml`, `crates/scramble/src/lib.rs`, `Cargo.lock`.
- Licensing files: `Cargo.toml` (workspace), `LICENSE` (removed), `LICENSE-MIT`, `LICENSE-APACHE`, `README.md`.
- Public API: unchanged. Possible behavioural differences (different RNG→state mapping, so seeded scrambles will differ from before; build time of tables) to be reported at the end.
- Still to confirm during apply: `min2phase` facelet convention vs. `cube` (covered by a test).
