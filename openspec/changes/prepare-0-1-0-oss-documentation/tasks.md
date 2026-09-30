# Tasks

## 1. Release metadata and command-line discovery

- [x] 1.1 Complete `[workspace.package]` metadata in `Cargo.toml` with `https://github.com/Bubbl33s/tuibik`, homepage/documentation fields as applicable, keywords/categories, and an explicitly verified MSRV; verify `cargo metadata --no-deps` reports the inherited values for every workspace crate.
- [x] 1.2 Add pre-terminal argument handling for `-h`/`--help`, `-V`/`--version`, and unsupported arguments, using the package version for version output; verify each informational flag exits 0 with stdout output, an unknown flag exits non-zero with a stderr usage hint, and none initializes raw/alternate-screen terminal state.
- [x] 1.3 Add automated tests for the command-line-interface spec and retain no-argument interactive startup; verify the new tests plus `cargo test --workspace --locked` pass.

## 2. Public release documentation

- [x] 2.1 Rewrite the README opening and installation section around GitHub Release archives, checksum verification, `PATH` setup, source build, and `cargo install --git`; verify every command and archive example matches an artifact from `automate-github-releases`.
- [x] 2.2 Add README sections for supported release platforms, tested terminal behavior, Kitty-protocol hold-to-start versus tap-to-start fallback, 40x12 minimum size, and how to report an unsupported environment; verify claims match application behavior and release CI coverage.
- [x] 2.3 Document platform data-directory conventions, `tuibik.sqlite`, and stopped-app backup/restore steps in README; verify the Linux path agrees with `ProjectDirs`/`$XDG_DATA_HOME` behavior and the guidance does not promise live SQLite-copy safety.
- [ ] 2.4 Capture a wide dashboard, compact dashboard, and overlay/detail screen from a release-candidate build into `docs/images/`; embed them with useful alt text and verify all relative image links render on GitHub.
- [ ] 2.5 Add `CHANGELOG.md` in Keep a Changelog format with `Unreleased` and a fact-checked `0.1.0` section based on the merged history; verify the `0.1.0` date and content are finalized immediately before tagging `v0.1.0`.

## 3. Open-source repository materials

- [ ] 3.1 Add `CONTRIBUTING.md` with local setup, formatting/lint/test commands, focused-PR expectations, and the existing dual-license contribution terms; verify its commands work from a clean checkout.
- [x] 3.2 Add `CODE_OF_CONDUCT.md` and `SECURITY.md`, including a private GitHub Security Advisory reporting route and the supported-version policy for `0.1.x`; verify links target the canonical repository and sensitive reports are not directed to public issues.
- [x] 3.3 Add GitHub issue forms/templates and a pull-request template that collect reproducible environment details, test evidence, and changelog impact; verify GitHub recognizes the template layout and no template duplicates branch-rule or CODEOWNERS policy owned by the governance change.
- [x] 3.4 Preserve `LICENSE-MIT`, `LICENSE-APACHE`, `MIT OR Apache-2.0` manifest text, README licensing links, and the Rust contribution clause while adding OSS materials; verify `openspec validate project-licensing --strict` and `cargo metadata --no-deps` still report the dual license.

## 4. Release-candidate acceptance

- [ ] 4.1 From a clean checkout, run the documented source install/build path and `tuibik --help`, `tuibik --version`, and an unknown-option check; verify documentation, CLI behavior, and the release version agree.
- [ ] 4.2 Review the rendered GitHub README, changelog, screenshots, community-health files, and release download flow together; verify no content advertises MIT-only terms, unsupported platforms, or artifacts that do not exist.
