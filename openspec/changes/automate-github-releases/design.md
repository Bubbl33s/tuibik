# Design

## Context

tuibik is a Rust workspace whose runnable package is `tuibik-tui` and whose
binary is `tuibik`. The workspace declares version `0.1.0`, has a dual
MIT/Apache-2.0 licence, and has neither workflow files nor packaged releases.
See proposal.md for motivation and the release-distribution spec for the
observable contract.

## Goals / Non-Goals

**Goals:**

- Make releases intentional, reproducible, and tied to a verified tag on
  `main`.
- Build native archives for Linux x86_64, macOS x86_64/ARM64, and Windows
  x86_64.
- Ensure a failed target cannot leave a partial public release.
- Keep `GITHUB_TOKEN` permissions narrowly scoped.

**Non-Goals:**

- Publishing crates to crates.io, Homebrew/Scoop packages, signing binaries,
  generating installers, or managing GitHub branch/ruleset configuration.
- Building Linux ARM64 or musl binaries in this first release workflow.

## Decisions

### Validate before fan-out, publish after fan-in

A read-only validation job resolves the tag from `push` or `workflow_dispatch`,
fetches `main` and tags, validates the `v<semver>` form, compares it with the
workspace version via Cargo metadata, and verifies ancestry with Git. Matrix
build jobs consume only the validated tag. A single publication job runs only
after every matrix artifact is available, generates checksums, and creates or
updates the GitHub Release.

This prevents an invalid manual invocation and avoids publishing a release
while another target has failed. Publishing independently from each matrix job
was rejected because it can expose incomplete releases.

### Native hosted runners for four initial targets

Use GitHub-hosted Ubuntu for Linux x86_64, macOS Intel and Apple Silicon runners
for the two macOS binaries, and Windows for Windows x86_64. Each job checks out
the validated tag, installs the matching stable Rust target where needed, runs
the locked workspace tests, builds `tuibik` in release mode, then creates a
platform-named `.tar.gz` or `.zip` archive containing the executable,
`README.md`, and both licence files.

Native runners are preferred to cross-compilation because the workspace has a
bundled SQLite dependency and native artifacts reduce linker and compatibility
risk. This leaves Linux ARM64 and static Linux portability for a later change.

### Treat the tag as the release authority

The automatic trigger is `push` on `v*`; manual dispatch takes a tag input for
recovering or deliberately rerunning a release. The workflow validates the
annotated or lightweight tag's resolved commit rather than trusting its name or
the dispatch branch. A tag must point at a commit reachable from `origin/main`.

This is preferred to releasing on PR approval or merge: approvals can become
stale and a merge is not necessarily a version release. Repository tag
protection/rulesets remain an administrative GitHub setting outside this
workflow.

### Scoped permissions and pinned actions

Set workflow default permissions to `contents: read`. The validation and build
jobs do not elevate them; only the final release job receives `contents: write`.
Use maintained GitHub actions pinned to immutable commit SHAs, pass artifacts
between jobs through the artifact service, and use the GitHub CLI or GitHub
release action in the final job to upload all files.

This avoids supplying a personal access token and limits repository mutation to
the one job that publishes the already-built payload.

### Artifact names and checksums

Archive names use `tuibik-<version>-<platform>` with recognizable platform
labels; the Windows archive includes `tuibik.exe`. The final job downloads all
archives into one directory and creates a conventional `SHA256SUMS` manifest
over those exact files before uploading them together.

## Risks / Trade-offs

- [macOS ARM64 runner availability or naming can differ by GitHub plan] -> Pin
  the supported runner labels during implementation and document any plan
  requirement; fail before publication if that matrix target cannot run.
- [A retagged version can make a release non-reproducible] -> Protect `v*` tags
  in repository settings and have the workflow reject a release that already
  exists for a different commit.
- [Git history may be too shallow to check `main` ancestry] -> Fetch the full
  tag and `main` history in the validation job.
- [An existing release upload can accidentally overwrite assets] -> Use
  collision-safe release creation/update behavior and fail on unexpected asset
  conflicts rather than silently replacing them.
- [Hosted-runner cost and duration] -> Matrix only the four stated targets and
  run release workflow only for tags or explicit dispatch.

## Migration Plan

1. Add the workflow and validate it first with `workflow_dispatch` against a
   non-production test version/tag or through a dry validation path.
2. In GitHub repository settings, protect `v*` tags before the first real tag.
3. Merge the workflow through the normal protected-`main` process.
4. Update the workspace version and changelog in their release PR, merge it,
   create `v0.1.0` at that merged commit, then verify release assets and their
   checksums.
5. If publication fails, fix the workflow in a new PR and rerun it manually for
   the same immutable tag; delete an incomplete draft release only after
   confirming no valid release assets are being removed.
