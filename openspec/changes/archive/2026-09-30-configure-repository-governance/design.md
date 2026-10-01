# Design

## Context

The repository is public at GitHub but currently has no code ownership configuration or recorded GitHub rulesets. GitHub branch and tag rulesets are hosted configuration, so a committed document must provide a repeatable setup and verification checklist. `prepare-0-1-0-oss-documentation` owns the community-health documents and templates.

## Goals / Non-Goals

**Goals:**

- Provide a maintainable, single-owner policy that protects `main` through pull requests and CI without blocking legitimate releases.
- Make the manual GitHub configuration exact enough to recreate and verify after an ownership or plan change.
- Reference the required community-health documents without duplicating their content or ownership.

**Non-Goals:**

- Automating GitHub settings with an external API, Terraform, or GitHub App.
- Requiring reviews that the sole maintainer cannot independently satisfy.
- Creating `CONTRIBUTING.md`, `SECURITY.md`, `CODE_OF_CONDUCT.md`, or GitHub issue/pull-request templates.
- Replacing the existing license terms or publishing a release workflow.

## Decisions

### Repository files express policy; GitHub rulesets enforce it

Add `.github/CODEOWNERS` to version control. Add a repository governance document that lists the exact manual ruleset settings for `main` and tags `v*`, plus a verification checklist and references to the community-health documents owned by `prepare-0-1-0-oss-documentation`. This separates reviewable policy from GitHub-hosted state and avoids duplicated community documents.

An API-driven configuration was considered, but it would add credential handling and provider-specific automation before the project has established release infrastructure. Screenshots alone were rejected because they become stale and are not searchable or reviewable as text.

### Whole-repository ownership begins with the sole maintainer

Use a catch-all `*` ownership pattern mapped to the current GitHub handle, with a short comment documenting how to add co-maintainers. This gives future repository changes an explicit owner without prematurely splitting ownership by directories.

Path-specific owners were considered but rejected for now: there is one maintainer and no established subsystem ownership.

### Protection favors required CI and controlled bypass over required approvals

The `main` ruleset requires pull requests, the CI checks defined by the quality-gates change, linear history if compatible with the chosen merge policy, and blocks deletion and force pushes. It allows only the maintainer to bypass when recovery or a release correction is necessary. Required approving reviews remain disabled while the repository has no independent reviewer.

Requiring one approval now was rejected because GitHub does not count self-approval as an independent required approval; it would deadlock normal maintenance. Allowing unrestricted direct pushes was rejected because it bypasses the review and CI record.

### Release-tag protection preserves the release boundary

Protect tags matching `v*`; authorize only the maintainer and, when introduced, the release automation identity. Tag rules deliberately do not trigger releases themselves: the release automation change owns that behavior.

## Risks / Trade-offs

- [Manual rulesets can drift from committed guidance] → Include explicit setup and post-configuration verification steps, and review the checklist whenever governance changes.
- [Maintainer bypass can circumvent protections] → Restrict bypass to the owner, preserve a pull-request/CI default, and use bypass only for recovery.
- [The documentation change may not land before governance configuration] → Treat its community-health files as a prerequisite and do not duplicate them here.
- [GitHub plan or organization features may constrain rulesets] → Document the closest supported configuration and record any unavailable setting in the verification checklist.

## Migration Plan

1. Merge `prepare-0-1-0-oss-documentation` so community-health documents are available.
2. Merge the ownership file and manual configuration checklist through a pull request.
3. Configure and verify the `main` and `v*` rulesets in GitHub repository settings.
4. Confirm the CI workflow's check names before selecting them as required checks.
5. When an independent co-maintainer joins, amend `CODEOWNERS`, grant the appropriate repository role, and enable required approvals through a follow-up change.

Rollback consists of temporarily disabling or editing the relevant GitHub ruleset if it blocks recovery, then correcting the documented checklist in a follow-up pull request.
