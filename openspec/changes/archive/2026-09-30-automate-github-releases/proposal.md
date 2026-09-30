# Proposal

## Why

tuibik has no reproducible way for users to download a verified release: the
repository currently contains no GitHub Actions workflows or published binary
artifacts. A tag-driven release pipeline is needed for the first public
`v0.1.0` release and for subsequent versions.

## What Changes

- Add a manually dispatchable and `v*` tag-triggered GitHub Actions release
  workflow.
- Verify that a release tag is a semantic version matching the workspace
  package version and points at a commit reachable from `main` before assets
  are published.
- Build the `tuibik` binary for supported Linux, macOS, and Windows targets;
  package each binary with the project README and both licence files.
- Publish SHA-256 checksums and the generated archives as a GitHub Release.
- Apply least-privilege workflow permissions, granting release publication
  access only to the job that needs it.

## Capabilities

### New Capabilities

- `github-release-distribution`: Produces verified, downloadable cross-platform
  tuibik releases from an explicitly versioned GitHub tag or manual dispatch.

### Modified Capabilities

- None.

## Impact

- Adds a release workflow under `.github/workflows/` and its documented
  packaging/release contract.
- Uses GitHub Actions hosted runners and GitHub Releases; no application
  runtime behaviour or Rust dependencies change.
- Release archives reuse the existing dual-licence files and README.
