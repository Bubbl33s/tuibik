# repository-governance Specification

## Purpose

Defines code ownership and GitHub protections for the repository while it has one maintainer, including the external controls that preserve a safe `main` branch.

## Requirements

### Requirement: Repository records code ownership
The repository SHALL include a `CODEOWNERS` file that assigns all repository paths to the current maintainer and is structured so future co-maintainers can be added through a reviewed change.

#### Scenario: Ownership file is evaluated
- **WHEN** GitHub evaluates a pull request that changes a repository file
- **THEN** the file matches an owner entry covering all repository paths

### Requirement: Governance policy references community documentation
The versioned GitHub-ruleset checklist SHALL reference the contribution, security, and conduct documents owned by the OSS documentation release change without duplicating their content or creating them.

#### Scenario: Governance checklist is reviewed
- **WHEN** a maintainer reviews the GitHub-ruleset checklist
- **THEN** it identifies the community documents that are prerequisites for public repository governance and assigns their ownership to the OSS documentation release change

### Requirement: Main branch is protected for a sole maintainer
The repository SHALL document a manually configured GitHub ruleset for `main` that requires pull requests and the project's required status checks, blocks force pushes and branch deletion, and limits bypass to the maintainer.

#### Scenario: Direct unprivileged update targets main
- **WHEN** an actor without the configured bypass permission attempts to push directly to `main`
- **THEN** GitHub rejects the update unless it is merged through a pull request satisfying the required status checks

### Requirement: Single-maintainer review policy avoids false approval requirements
The repository SHALL state that a maintainer cannot provide the independent approval required by GitHub for their own pull request, and SHALL not configure a required approving review until at least one independent co-maintainer is available.

#### Scenario: Maintainer opens their own pull request
- **WHEN** the sole maintainer requests or submits a review on their own pull request
- **THEN** the governance guidance does not treat that action as satisfying an independent required approval

#### Scenario: Independent co-maintainer joins
- **WHEN** an independent co-maintainer is granted repository-maintainer responsibility
- **THEN** the documented ruleset checklist directs the maintainer to add them as a code owner and enable an appropriate required-approval rule

### Requirement: Release tags are protected
The repository SHALL document a manually configured GitHub ruleset for release tags matching `v*` that limits creation, update, and deletion to the maintainer or explicitly authorized release automation.

#### Scenario: Unauthorized actor changes a release tag
- **WHEN** an actor without release-tag permission attempts to create, update, or delete a tag matching `v*`
- **THEN** GitHub rejects the operation
