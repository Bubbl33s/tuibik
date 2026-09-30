# Proposal

## Why

tuibik's current README only describes building from source, and the executable immediately enters terminal raw mode without standard command-line discovery. A public `0.1.0` release needs trustworthy installation guidance, visible product evidence, recovery guidance for locally stored data, and the baseline repository materials users and contributors expect.

## What Changes

- Expand the README with release-download installation, source installation, platform and terminal compatibility, screenshots, data location, and backup/restore guidance.
- Add a Keep a Changelog-style `CHANGELOG.md` with an unreleased section and an accurate `0.1.0` release entry.
- Complete Cargo workspace package metadata with the canonical GitHub repository URL and a declared minimum Rust version appropriate to the supported toolchain.
- Add standard OSS repository materials: contribution guidance, a code of conduct, a security reporting policy, and issue/pull-request templates.
- Add non-interactive `--help` and `--version` command-line behavior that exits before terminal initialization; keep normal no-argument TUI startup unchanged.
- Preserve the existing `MIT OR Apache-2.0` licensing model and its contribution terms.

## Capabilities

### New Capabilities

- `command-line-interface`: Standard help and version information for the `tuibik` executable without starting the TUI.

### Modified Capabilities

None.

## Impact

- Updates `README.md` and the workspace section of `Cargo.toml`; adds `CHANGELOG.md`, documentation image assets, and OSS community-health files under repository-standard paths.
- Updates `crates/tuibik-tui/src/main.rs` (and tests as needed) to parse the two informational flags before raw-mode terminal setup.
- Documents distribution artifacts produced by the companion GitHub release automation change, but does not create or publish a release itself.
- Retains `LICENSE-MIT`, `LICENSE-APACHE`, Cargo's `MIT OR Apache-2.0` declaration, and the standard dual-license contribution clause.
