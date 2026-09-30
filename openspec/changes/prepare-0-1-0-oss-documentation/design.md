# Design

## Context

The workspace root declares version `0.1.0`, edition 2021, and dual `MIT OR Apache-2.0` licensing but leaves `repository` blank. The binary currently initializes crossterm raw mode before interpreting any arguments. README documents source execution only; no changelog, community-health files, or documentation image directory exists. The SQLite database is obtained through `directories::ProjectDirs` and is named `tuibik.sqlite` in the platform data directory. See `proposal.md` for motivation and the command-line-interface spec for observable CLI behavior.

## Goals / Non-Goals

**Goals:**

- Make the README the accurate entry point for downloading, verifying, using, and backing up `0.1.0`.
- Store screenshots as versioned repository assets with descriptive alt text so GitHub renders them without third-party hosting.
- Let shell users inspect help and version without requiring an interactive terminal.
- Make metadata and OSS guidance consistent with the existing dual-license project policy.

**Non-Goals:**

- Replacing the release workflow or defining its target matrix and artifact format.
- Publishing crates to crates.io, adding package-manager distribution, or changing database migrations.
- Relicensing the project to MIT-only.
- Adding a full command parser or runtime configuration flags beyond `-h`/`--help` and `-V`/`--version`.

## Decisions

### Keep documentation assets in the repository

Place curated PNG screenshots under `docs/images/` and reference them with relative README paths. Capture a wide dashboard, a compact layout, and a representative overlay or detail screen from a real release-candidate build. Repository-hosted assets stay available in forks and do not depend on external image infrastructure. Animated GIFs and externally hosted screenshots were rejected because they inflate the repository or risk link rot.

### Use GitHub Releases as the primary installation path

The README will lead with downloading the archive appropriate to the user's OS/architecture from GitHub Releases, unpacking it, and placing `tuibik` on `PATH`; it will link to SHA-256 verification instructions and also offer `cargo install --git`/source build as developer alternatives. Exact archive filenames and platform coverage must match artifacts made by `automate-github-releases`; documentation must not promise an artifact the release workflow does not publish.

### Document platform behavior, not unsupported guarantees

The compatibility section will distinguish release-supported operating systems and architectures from tested terminals. It will explicitly explain that Kitty keyboard-protocol support enables hold-to-start and that non-supporting terminals fall back to tap-to-start. It will list the minimum terminal dimensions already enforced by the app (40x12) and link to an issue-reporting path for untested environments. Claiming universal terminal compatibility was rejected because the input protocol varies by emulator.

### Expose only two informational CLI flags before TUI setup

Parse process arguments before `init_terminal`. `-h`/`--help` print fixed usage text; `-V`/`--version` print `tuibik` plus `env!("CARGO_PKG_VERSION")`; both return success without terminal effects. A small std-only parser is preferred over adding a CLI framework for two fixed flags. Default invocation remains unchanged. Any unsupported argument prints a usage hint to standard error and exits non-zero rather than silently entering the TUI.

### Treat the SQLite file as the user backup unit

README will document the platform data-directory conventions derived from `ProjectDirs` (including the common Linux XDG path) and instruct users to quit tuibik before copying the database file. Restore consists of replacing the file while the app is stopped; the existing automatic migrations run on next open. Export/import and cloud synchronization are intentionally not implied.

### Standard OSS documents stay concise and retain dual licensing

Add `CONTRIBUTING.md`, `CODE_OF_CONDUCT.md`, `SECURITY.md`, issue templates, and a pull-request template that point contributors toward tested changes and private security reporting. The README and new documents will retain the dual-license statement and standard Rust contribution clause; `LICENSE-MIT`, `LICENSE-APACHE`, and `MIT OR Apache-2.0` remain unchanged. `CODEOWNERS`, protected-branch rules, and review authorization are handled by the governance change.

## Risks / Trade-offs

- [Release workflow artifact names diverge from README commands] → Review the README against a draft release and make its archive examples match the release workflow exactly.
- [Screenshots become stale after visual changes] → Use named, purpose-specific assets and require screenshot review when README-visible layouts change.
- [A backup copy is taken while SQLite is writing] → Instruct users to exit the application first and avoid claiming live-copy safety.
- [A custom parser mishandles arguments] → Add integration tests that assert stdout, exit status, and no interactive initialization for all four informational flag forms.
- [Community documents conflict with remote governance policy] → Cross-reference governance guidance without duplicating CODEOWNERS or settings that GitHub alone enforces.

## Migration Plan

1. Add metadata and documentation assets/docs while retaining all existing license files and terms.
2. Add CLI information-flag parsing and automated non-interactive tests; confirm no-argument interactive startup still works.
3. Build a release candidate through the release workflow, verify README installation and checksum steps from its published artifacts, and capture the documented screenshots.
4. Publish the `0.1.0` changelog entry immediately before the `v0.1.0` tag; future work starts in the Unreleased section.

Rollback documentation independently if an artifact promise is inaccurate; rollback CLI parsing by reverting before-terminal argument handling, preserving normal timer startup.
