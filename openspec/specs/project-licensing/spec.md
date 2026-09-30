# project-licensing Specification

## Purpose

States the licensing terms of the project and the dependency licence constraints that make those terms valid.

## Requirements

### Requirement: Dual licence
The project SHALL be licensed under `MIT OR Apache-2.0`, declared in the workspace manifest and provided as `LICENSE-MIT` and `LICENSE-APACHE` at the repository root, with the copyright holder's name and year in the MIT text.

#### Scenario: Licence files present
- **WHEN** the repository root is inspected
- **THEN** `LICENSE-MIT` and `LICENSE-APACHE` exist, the old GPL `LICENSE` does not, and `cargo metadata` reports `MIT OR Apache-2.0` for every workspace crate

#### Scenario: README states terms
- **WHEN** the README licence section is read
- **THEN** it names both licences, links both files, includes the standard Rust contribution clause, and does not mention `m2p-core` or GPL

### Requirement: No copyleft dependencies
The dependency tree SHALL NOT contain GPL, LGPL or AGPL-licensed packages, and SHALL NOT contain `m2p-core`.

#### Scenario: Dependency audit
- **WHEN** licences of all packages are listed via `cargo metadata`
- **THEN** no non-workspace package has a GPL, LGPL or AGPL licence and `Cargo.lock` has no `m2p-core` entry

### Requirement: Relicensing consent
The project SHALL be relicensed only if all copyright holders in the git history consent.

#### Scenario: Multiple authors
- **WHEN** `git shortlog -sne` shows an author other than the project owner
- **THEN** relicensing does not proceed until the owner confirms consent has been obtained
