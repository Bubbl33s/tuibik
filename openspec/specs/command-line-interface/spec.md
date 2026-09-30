# command-line-interface Specification

## Purpose

Defines non-interactive discovery commands for the tuibik executable so users can identify its version and usage without starting the terminal interface.

## Requirements

### Requirement: Help output is available without terminal initialization
The `tuibik` executable SHALL accept `-h` and `--help`, print concise usage and flag information to standard output, exit successfully, and SHALL NOT enter raw mode or the alternate terminal screen.

#### Scenario: Long help flag
- **WHEN** a user runs `tuibik --help` in a non-interactive shell
- **THEN** it exits with status 0 after printing usage information and without initializing the TUI

#### Scenario: Short help flag
- **WHEN** a user runs `tuibik -h`
- **THEN** it produces the same successful non-interactive help behavior as `tuibik --help`

### Requirement: Version output identifies the packaged release
The `tuibik` executable SHALL accept `-V` and `--version`, print the executable name and its Cargo package version to standard output, exit successfully, and SHALL NOT enter raw mode or the alternate terminal screen.

#### Scenario: Long version flag
- **WHEN** a user runs `tuibik --version`
- **THEN** it exits with status 0 after printing a version string containing the package version

#### Scenario: Short version flag
- **WHEN** a user runs `tuibik -V`
- **THEN** it produces the same non-interactive version behavior as `tuibik --version`

### Requirement: Default invocation starts the timer application
The `tuibik` executable SHALL preserve its existing interactive timer behavior when invoked without command-line arguments.

#### Scenario: No arguments
- **WHEN** a user runs `tuibik` without command-line arguments in a supported interactive terminal
- **THEN** the terminal UI starts rather than printing help or version output

### Requirement: Unsupported command-line arguments fail safely
The `tuibik` executable SHALL reject unsupported command-line arguments with a non-zero exit status and a usage hint on standard error, and SHALL NOT initialize the TUI.

#### Scenario: Unknown argument
- **WHEN** a user runs `tuibik --unknown-option`
- **THEN** it exits non-zero after printing a usage hint to standard error and without entering raw mode or the alternate terminal screen
