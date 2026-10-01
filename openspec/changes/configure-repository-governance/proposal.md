# Proposal

## Why

tuibik needs enforceable GitHub controls before its first public release. As a single-maintainer repository, its GitHub controls must protect `main` without requiring an approval that the maintainer cannot legitimately supply for their own pull request.

## What Changes

- Add `CODEOWNERS` so repository changes have an explicit owner and future co-maintainers can be added deliberately.
- Add a versioned manual GitHub ruleset checklist for `main` and release tags: pull-request workflow, required CI checks, prevention of force pushes/deletion, and maintainer-only bypass.
- Define the single-maintainer review policy: self-requested reviews and self-approvals do not satisfy GitHub's required-approval policy; required approvals remain disabled until an independent co-maintainer exists.
- Reference the community-health documents delivered by `prepare-0-1-0-oss-documentation` rather than creating or owning them in this change.

## Capabilities

### New Capabilities
- `repository-governance`: Defines code ownership and single-maintainer GitHub branch/tag-protection policy.

### Modified Capabilities

- None.

## Impact

- Adds `.github/CODEOWNERS` and a versioned GitHub-ruleset checklist.
- Requires a one-time manual ruleset configuration in the GitHub repository settings; GitHub settings cannot be made effective solely by a committed workflow file.
- Depends on `prepare-0-1-0-oss-documentation` for `CONTRIBUTING.md`, `SECURITY.md`, `CODE_OF_CONDUCT.md`, and issue/pull-request templates.
