# Design

## Context

The repository is a Rust 2021 workspace with five crates and no tracked `.github/` configuration. Its existing local quality commands are `cargo fmt --check`, `cargo clippy --workspace -- -D warnings`, and `cargo test --workspace`; the release-ready build must additionally use the lockfile. See `proposal.md` for motivation.

## Goals / Non-Goals

**Goals:**

- Make every proposed change to `main` reproducibly pass formatting, lint, test, and release-build checks.
- Give GitHub branch rules stable, independently visible check names to require.
- Keep workflow credentials and third-party action supply-chain exposure to the minimum necessary.
- Keep Cargo and GitHub Action dependency updates visible as reviewable pull requests.

**Non-Goals:**

- Creating release artifacts or publishing GitHub Releases.
- Configuring GitHub's remote branch ruleset through a workflow or API.
- Adding code coverage thresholds, platform matrix testing, or an external security scanner in this first quality gate.

## Decisions

### One CI workflow runs four independent required jobs

Create `.github/workflows/ci.yml` with triggers `pull_request` targeting `main` and `push` to `main`. It will expose the exact job display names `fmt`, `clippy`, `test`, and `build` so GitHub rules can require each one. Each job checks out the same revision, installs the stable Rust toolchain with the `rustfmt` and `clippy` components where needed, caches Cargo artifacts, then runs one command:

- `fmt`: `cargo fmt --check`
- `clippy`: `cargo clippy --workspace -- -D warnings`
- `test`: `cargo test --workspace --locked`
- `build`: `cargo build --workspace --release --locked`

Independent jobs make a failing gate obvious and avoid making a successful aggregate job mask a skipped check. A single sequential job was considered, but it produces only one required status and offers less useful feedback. A full OS matrix is deferred to the release automation change because this gate validates source quality, not packaged binaries.

### Workflows use explicit least-privilege permissions and immutable actions

Set top-level `permissions: contents: read`; no write token is necessary for CI. Pin each third-party action (checkout, Rust toolchain setup, and cache) to a full commit SHA, with a comment naming the reviewed release. Pinning protects the execution identity from a mutable tag; floating major tags were rejected because they weaken that supply-chain boundary. The implementation should retain Dependabot coverage for `github-actions` so pin updates arrive as pull requests.

### Dependabot covers Cargo and GitHub Actions weekly

Create `.github/dependabot.yml` with separate weekly update entries for the repository root Cargo ecosystem and GitHub Actions. Limit scope to the workspace root, where `Cargo.lock` and workflows reside. No auto-merge is configured: updates must pass the same CI and the maintainer's merge policy.

### Required checks are documented, not remotely configured

Add a concise maintainer document that names the four exact CI checks and explains selecting them in the `main` branch ruleset after the first successful CI run. Repository configuration belongs to GitHub's remote settings and cannot be reliably represented by a committed file. The document must also note that rules should require a pull request, dismiss stale approvals on new commits, require conversation resolution, and block force pushes/deletion; the governance change owns any broader review-policy documentation.

### README badge targets the canonical GitHub workflow

Add a `CI` badge near the README title using the existing `Bubbl33s/tuibik` origin and the workflow filename `ci.yml`. The badge is informational only and must not be treated as an enforcement mechanism.

## Risks / Trade-offs

- [A SHA-pinned action becomes outdated] → Dependabot proposes updates for review; release-name comments make audits practical.
- [Required check names change during workflow edits] → Treat the four display names as a compatibility contract and update the remote ruleset/document in the same PR if a rename is unavoidable.
- [Caching introduces an opaque CI failure] → Use only the standard Cargo cache inputs and make each command runnable locally without the cache.
- [GitHub rules cannot require a check until it has reported once] → Merge the CI workflow only after observing a completed run, then configure the named required checks in repository settings.

## Migration Plan

1. Add the workflow, Dependabot configuration, maintainer instructions, and README badge in one pull request.
2. Verify the workflow runs on that pull request and record the four exact displayed check names.
3. In GitHub repository settings, create or update the `main` ruleset to require those checks and the documented pull-request protections.
4. Confirm a deliberately failing pull request cannot merge until every check passes; remove or correct the temporary failure before merging.

Rollback consists of removing a newly required check from the remote ruleset before reverting its workflow job, so `main` is never blocked by a status that can no longer report.
