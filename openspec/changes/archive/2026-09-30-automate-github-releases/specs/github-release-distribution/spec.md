# Spec Delta

## Purpose

Provides reproducible, verifiable downloads of versioned tuibik binaries for
supported desktop platforms through GitHub Releases.

## ADDED Requirements

### Requirement: Explicit versioned release invocation
The repository SHALL provide a release workflow that runs when a tag matching
`v*` is pushed and can also be started manually with a requested release tag.
Before publishing, it SHALL reject an invocation unless the selected tag is a
valid semantic version prefixed by `v`, its version equals the workspace package
version, and its target commit is reachable from `main`.

#### Scenario: Valid tag triggers a release
- **WHEN** a tag such as `v0.1.0` matching the workspace version is pushed from
  a commit reachable from `main`
- **THEN** the workflow proceeds to build and publish that release

#### Scenario: Manual release chooses a valid existing tag
- **WHEN** a maintainer manually runs the workflow and supplies an existing
  valid tag that matches the workspace version and `main`
- **THEN** the workflow uses that tag as the release source

#### Scenario: Tag fails release validation
- **WHEN** the selected tag has an invalid version, a mismatched workspace
  version, or does not target a commit reachable from `main`
- **THEN** the workflow fails without creating or modifying a GitHub Release

### Requirement: Cross-platform release archives
For each release, the workflow SHALL produce downloadable archives containing
the `tuibik` executable, `README.md`, `LICENSE-MIT`, and `LICENSE-APACHE` for
Linux x86_64, macOS x86_64, macOS ARM64, and Windows x86_64.

#### Scenario: Release artifacts are complete
- **WHEN** a release workflow completes successfully
- **THEN** its GitHub Release contains one correctly formatted archive per
  supported platform, each including the executable, README, and both licences

#### Scenario: Platform build failure
- **WHEN** any supported platform build fails
- **THEN** the workflow fails and does not publish a partial GitHub Release

### Requirement: Verifiable GitHub Release publication
The workflow SHALL publish its archives and a SHA-256 checksum manifest to the
GitHub Release for the validated tag, and SHALL not require write permissions in
jobs that only validate, build, or package the release.

#### Scenario: Consumer verifies a download
- **WHEN** a consumer opens a successfully published GitHub Release
- **THEN** they can download both a platform archive and its corresponding
  SHA-256 checksum from that release

#### Scenario: Least-privilege workflow execution
- **WHEN** the release workflow is inspected
- **THEN** only its publication job has permission to write repository contents
  and validation/build jobs retain read-only permissions
