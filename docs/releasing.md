# Releasing tuibik

The release workflow runs when a `v*` tag is pushed and can be re-run from the
Actions page with an existing tag. It accepts only a `v<semver>` tag whose
version matches `tuibik-tui` and whose commit is reachable from `main`.

Before the first release, create an active repository ruleset for tags matching
`v*` in **Settings → Rules → Rulesets**. Restrict tag creation and updates to
the maintainer, and prevent tag deletion and force updates. This keeps a
released version immutable.

To release a version:

1. Merge the release-ready changelog and version changes to `main`.
2. Create and push an annotated tag from that merge commit, for example
   `git tag -a v0.1.0 -m "v0.1.0" && git push origin v0.1.0`.
3. Watch the **Release** workflow. It builds native archives for Linux x86_64,
   macOS Intel, macOS Apple Silicon, and Windows x86_64.
4. Download an archive and `SHA256SUMS` from the GitHub Release; verify it with
   `sha256sum -c SHA256SUMS --ignore-missing` on Linux.

If a workflow needs to be retried, use **Run workflow** with the same existing
tag. It refuses a tag or release asset that points to a different commit or has
different contents.
