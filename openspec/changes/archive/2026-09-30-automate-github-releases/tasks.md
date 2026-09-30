# Tasks

## 1. Release source validation

- [x] 1.1 Add the tag-push (`v*`) and `workflow_dispatch` release entry points, with a required manual tag input; verify the workflow parses successfully and exposes both invocation paths in GitHub Actions.
- [x] 1.2 Implement a read-only validation job that resolves the selected tag, fetches tag and `main` history, validates the prefixed SemVer form, compares it to the workspace package version, and rejects commits outside `origin/main`; verify invalid tag, version mismatch, and non-main ancestry each fail before any publication job runs.
- [x] 1.3 Configure workflow- and job-level token permissions so validation and build jobs are read-only and only publication has `contents: write`; verify the workflow YAML and GitHub Actions permission summary show that separation.

## 2. Cross-platform package builds

- [x] 2.1 Add a native runner matrix for Linux x86_64, macOS x86_64, macOS ARM64, and Windows x86_64 that checks out the validated tag, runs locked workspace tests, and builds the release `tuibik` binary; verify every matrix target produces the expected executable.
- [x] 2.2 Package each target with its executable, `README.md`, `LICENSE-MIT`, and `LICENSE-APACHE` into consistently named `.tar.gz` or `.zip` archives; verify the contents of every archive and the Windows `.exe` name.
- [x] 2.3 Transfer package archives from matrix jobs to the final job without granting repository-write permission to build jobs; verify a failed matrix target prevents the publication job from running.

## 3. GitHub Release publication

- [x] 3.1 Implement a single final publication job that gathers all successful archives, produces a `SHA256SUMS` manifest, and creates or safely updates the GitHub Release for the validated tag; verify the release includes four archives and the checksum manifest.
- [x] 3.2 Make publication collision-safe by rejecting an existing release/tag that points to a different commit or has unexpected conflicting assets; verify the failure leaves previously published valid assets unchanged.
- [x] 3.3 Pin third-party workflow actions to immutable revisions and document any required GitHub repository setting for protecting `v*` tags; verify action references are immutable and the release instructions name the tag-protection prerequisite.

## 4. End-to-end release verification

- [x] 4.1 Exercise the workflow through `workflow_dispatch` for a valid test tag or the first `v0.1.0` tag after merge, then download an archive and verify it against `SHA256SUMS`; verify the executable, README, and both licence files are present.
