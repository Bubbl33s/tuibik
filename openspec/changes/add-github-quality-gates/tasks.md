# Tasks

## 1. Continuous integration

- [ ] 1.1 Add `.github/workflows/ci.yml` with `pull_request` (targeting `main`) and `push` (to `main`) triggers, top-level read-only token permissions, and separately named `fmt`, `clippy`, `test`, and `build` jobs; verify the workflow YAML parses and GitHub displays all four jobs on a pull request.
- [x] 1.2 Pin checkout, Rust setup, and Cargo cache actions in the CI workflow to reviewed full commit SHAs with release-version comments; verify no `uses:` reference in the workflow is a mutable tag.
- [ ] 1.3 Configure the CI jobs to run the stable-toolchain workspace commands `cargo fmt --check`, `cargo clippy --workspace -- -D warnings`, `cargo test --workspace --locked`, and `cargo build --workspace --release --locked`; verify each corresponding job passes on the CI pull-request run.

## 2. Dependency maintenance and maintainer guidance

- [ ] 2.1 Add `.github/dependabot.yml` with weekly update schedules for the root Cargo ecosystem and GitHub Actions; verify GitHub recognizes both update configurations in the Dependabot settings.
- [x] 2.2 Add `.github/branch-protection.md` that identifies the exact `fmt`, `clippy`, `test`, and `build` required checks and the required-pull-request, stale-review dismissal, resolved-conversation, force-push, and deletion settings; verify the documented check names exactly match the workflow job display names.
- [ ] 2.3 Configure the remote `main` GitHub ruleset according to `.github/branch-protection.md` after the CI checks have reported once; verify a pull request with a deliberately failing required check cannot merge, then remove the deliberate failure.

## 3. Documentation and end-to-end verification

- [ ] 3.1 Add a CI status badge near the README title using the `Bubbl33s/tuibik` repository and `ci.yml` workflow; verify the badge URL targets the workflow and renders from GitHub after merge.
- [ ] 3.2 Confirm a clean checkout passes `cargo fmt --check`, `cargo clippy --workspace -- -D warnings`, `cargo test --workspace --locked`, and `cargo build --workspace --release --locked`; verify all four are green locally and in one pull-request run.
