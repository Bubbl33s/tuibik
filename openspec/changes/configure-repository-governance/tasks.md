# Tasks

## 1. Record ownership and governance policy

- [ ] 1.1 Add `.github/CODEOWNERS` with a catch-all ownership entry for the current maintainer and future-maintainer guidance; verify GitHub recognizes the file and its `*` rule covers a changed repository path.
- [x] 1.2 Confirm `prepare-0-1-0-oss-documentation` has delivered `CONTRIBUTING.md`, `SECURITY.md`, `CODE_OF_CONDUCT.md`, and templates before merging this change; verify this change neither creates nor duplicates those files.

## 2. Document GitHub-hosted enforcement

- [x] 2.1 Add a versioned governance checklist that references the OSS documentation community-health files and specifies the `main` ruleset: pull requests, required CI checks once their exact names exist, no force pushes/deletion, maintainer-only bypass, and no required approving review while there is one maintainer; verify every setting can be located in GitHub repository ruleset settings.
- [x] 2.2 Add the `v*` tag-ruleset checklist with authorized actors, protection against update/deletion, and compatibility with future release automation; verify it identifies the release automation identity as a future addition rather than assuming one exists.
- [x] 2.3 State in the governance checklist that self-review/self-approval cannot satisfy independent required approval, and document the co-maintainer transition steps; verify the policy is consistent with the contribution guidance owned by the OSS documentation change.

## 3. Configure and validate GitHub settings

- [ ] 3.1 After the quality-gates workflow is merged, configure the documented `main` ruleset in GitHub using its exact CI check names; verify a non-bypass direct update is rejected and a pull request displays the required checks.
- [ ] 3.2 Configure the documented `v*` tag ruleset in GitHub; verify an unauthorized actor cannot create, update, or delete a matching release tag.
- [ ] 3.3 Review the repository ruleset pages after merge; verify the configured controls match the committed checklist and its links to the OSS documentation community-health files.
