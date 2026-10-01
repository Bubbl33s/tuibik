# Repository governance

tuibik has one maintainer. This document records the GitHub-hosted controls
that protect `main` and release tags, and how they change when an independent
co-maintainer joins. GitHub rulesets live in the repository settings rather
than in version control, so this checklist is the reviewable source for them:
change it in a pull request whenever the configured rulesets change.

Code ownership is recorded in [`.github/CODEOWNERS`](../.github/CODEOWNERS).

## Community documents

These documents come before public repository governance. They are owned and
maintained by the OSS documentation release change, not by this checklist,
which only links to them:

- [CONTRIBUTING.md](../CONTRIBUTING.md): contribution workflow and the local
  checks that match CI.
- [SECURITY.md](../SECURITY.md): private vulnerability reporting.
- [CODE_OF_CONDUCT.md](../CODE_OF_CONDUCT.md): expected conduct.
- [Issue forms](../.github/ISSUE_TEMPLATE/) and the
  [pull request template](../.github/pull_request_template.md).

## Review policy for a sole maintainer

GitHub does not let a pull request author approve their own pull request, and
a review the maintainer requests from or submits for themselves is not an
independent approval. While there is one maintainer:

- Do not configure a required number of approvals or required code owner
  review. Either would block every maintainer pull request.
- Every change to `main` still goes through a pull request and must pass the
  required CI checks. The pull request is the review and CI record.
- Do not treat a self-review as satisfying any independent-approval
  requirement.

## `main` branch ruleset

Create it in **Settings → Rules → Rulesets → New ruleset → New branch
ruleset**. Configure it only after the **CI** workflow has reported on GitHub
at least once, so its checks can be selected by name.

| Setting | Value |
| --- | --- |
| Ruleset name | `Protect main` |
| Enforcement status | Active |
| Bypass list | Repository admin role, mode *Always allow*: the maintainer only. Use bypass only to recover from a broken `main` or correct a release. |
| Target branches | Include by pattern: `main` |
| Restrict deletions | Enabled |
| Block force pushes | Enabled |
| Require linear history | Disabled, because merge commits are an allowed merge method |
| Require a pull request before merging | Enabled |
| ↳ Required approvals | `0` while there is one maintainer |
| ↳ Dismiss stale pull request approvals when new commits are pushed | Enabled |
| ↳ Require review from Code Owners | Disabled while there is one maintainer |
| ↳ Require approval of the most recent reviewable push | Disabled while there is one maintainer |
| ↳ Require conversation resolution before merging | Enabled |
| Require status checks to pass | Enabled |
| ↳ Require branches to be up to date before merging | Enabled |
| ↳ Status checks | `fmt`, `clippy`, `test`, `build` |

The status check names must exactly match the job display names in
[`.github/workflows/ci.yml`](../.github/workflows/ci.yml). They are also listed
in [`.github/branch-protection.md`](../.github/branch-protection.md). If a job is
renamed, update the ruleset and both documents in the same pull request.

## Release tag ruleset

Create it in **Settings → Rules → Rulesets → New ruleset → New tag ruleset**.
See [releasing.md](releasing.md) for how tags drive releases.

| Setting | Value |
| --- | --- |
| Ruleset name | `Protect release tags` |
| Enforcement status | Active |
| Bypass list | Repository admin role, mode *Always allow*: the maintainer only |
| Target tags | Include by pattern: `v*` |
| Restrict creations | Enabled |
| Restrict updates | Enabled |
| Restrict deletions | Enabled |

The maintainer creates release tags by hand. The **Release** workflow publishes
a GitHub Release for an existing tag using the workflow's `GITHUB_TOKEN`. It
never creates, moves, or deletes tags, so no automation identity is on the
bypass list today. If release automation later needs to create tags, for
example through a GitHub App, add that specific identity to the bypass list,
record it in this table, and review the change in a pull request. Never add a
broad role for that purpose.

## Verification

Run this after creating or changing either ruleset, and whenever this document
changes:

- [ ] Open a pull request against `main`: the four required checks appear and
      the pull request cannot merge while any of them fails.
- [ ] A direct push to `main` from an account without bypass is rejected.
- [ ] Creating, moving, or deleting a `v*` tag from an account without bypass
      is rejected.
- [ ] **Settings → Rules → Rulesets** shows both rulesets as Active, and every
      setting matches the tables above. Record any setting your GitHub plan
      does not offer next to its table row.
- [ ] The links in [Community documents](#community-documents) resolve.
- [ ] GitHub shows no errors for `.github/CODEOWNERS` on the file's page, and a
      pull request that changes any file lists the maintainer as code owner.

## Adding an independent co-maintainer

1. Grant them the Maintain or Admin repository role.
2. In a pull request, add their handle to the catch-all rule in
   `.github/CODEOWNERS` and update this document.
3. After that merges, edit the `main` ruleset: set **Required approvals** to
   at least `1`, and enable **Require review from Code Owners** and **Require
   approval of the most recent reviewable push**.
4. Decide whether they belong on either bypass list. Record the decision here.
5. Run the verification checklist again.
