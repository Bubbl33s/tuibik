# Contributing to tuibik

Thanks for helping improve tuibik. Please open an issue before beginning a
large change so the intended behaviour can be discussed first.

## Local setup

Install Rust 1.88 or newer, clone the repository, and run:

```bash
cargo run --release --bin tuibik
```

Before opening a pull request, run the same checks enforced in CI:

```bash
cargo fmt --check
cargo clippy --workspace -- -D warnings
cargo test --workspace --locked
```

Keep pull requests focused. Describe the user-visible change, include tests
for behavioural changes, and explain any impact on `CHANGELOG.md`. Do not mix
unrelated formatting, refactors, or generated files into a feature or bug-fix
pull request.

## Reporting issues and proposing changes

Use the repository's issue forms and include your operating system, terminal
emulator, tuibik version, and steps to reproduce the problem. For a security
issue, follow [SECURITY.md](SECURITY.md) instead of opening a public issue.

## License

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in the work by you, as defined in the Apache-2.0
license, shall be dual licensed under the [Apache License, Version
2.0](LICENSE-APACHE) and the [MIT license](LICENSE-MIT), without any additional
terms or conditions.
