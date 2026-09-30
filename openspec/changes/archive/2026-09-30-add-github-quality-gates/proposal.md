# Proposal

## Why

tuibik has no automated verification on pull requests or on updates to `main`, so regressions can reach the release branch without a reproducible quality gate. Before the first public release, maintainers need a small, auditable CI baseline and a documented mapping from its jobs to the GitHub branch rules that enforce them.

## What Changes

- Add a GitHub Actions continuous-integration workflow for pull requests targeting `main` and pushes to `main` that checks Rust formatting, Clippy warnings, workspace tests, and release-mode buildability.
- Add Dependabot configuration for GitHub Actions and Cargo dependency update pull requests.
- Apply least-privilege workflow permissions and pin third-party GitHub Actions to immutable commit SHAs, with human-readable version comments.
- Document the stable CI job names that repository branch protection/rulesets must require for `main`.
- Add a CI status badge to the README once the workflow name and public repository URL are defined by the companion release-readiness work.

## Capabilities

### New Capabilities

None. This is repository tooling and documentation; it does not change tuibik runtime behavior.

### Modified Capabilities

None.

## Impact

- Adds `.github/workflows/ci.yml`, `.github/dependabot.yml`, and a repository-maintenance document for required checks.
- Updates `README.md` with the CI badge when its canonical GitHub repository URL is available.
- Uses the stable Rust toolchain and existing workspace commands: `cargo fmt --check`, `cargo clippy --workspace -- -D warnings`, `cargo test --workspace`, and `cargo build --workspace --release --locked`.
- Requires a maintainer to configure the documented CI jobs as required status checks in the remote GitHub ruleset; workflow files alone cannot protect `main`.
