# Working on oxiguitar-midi-file with Claude

This repository is part of OxiGuitar. Before anything else, every session
reads `claude/START-HERE.md` in the OxiGuitar project on claude.ai (the
project's files, not this repository), then the brief and roadmap it links
to, then this repository's `develop` branch and the OxiGuitar repository's
(github.com/mateusrdp/oxiguitar). START-HERE.md is the source of truth for
decisions; this file only repeats the standing rules.

## Standing rules

- Libraries exactly as published: no forks, no [patch], no vendored or edited copies; never plan on or wait for upstream changes.
- Never commit the corpus (tabs.tar.gz, scores.tar.gz) or publish renders of it.
- No telemetry, no uploads, no downloads by the app.
- No emojis anywhere, including PR attribution lines.
- No signatures or credit anywhere on GitHub: **never** write "Generated with Claude", "Generated with Claude Code" or any variant in a comment, an issue, a PR body or a review. The author line already says it. Commits carry only `Signed-off-by: Claude <noreply@anthropic.com>`.
- Keep README.md up to date with screenshots and example sounds.
- I merge PRs and create v* tags. Whenever I need to do something on git, give me step-by-step instructions with the exact commands.
- Stack PRs to save CI runs.
- Open PRs with the REST API (gh pr create fails in your sessions).
- Send test-build links, with their expiry dates, for each PR.
- Locally you only have rustc 1.97: lower rust-version to 1.97 for nested plugin builds, always restore 1.99, and delete target/lib after build-plugins.sh.
- When checking a local test run, look at the exit code and every FAILED line.
- Never rewrite a pushed branch without asking me; never rebase a commit of mine into a branch (the DCO check fails on it).
- Every new feature or plugin comes with its tests and a row on `docs/manual/src/features.md` naming them, in the same PR; corpus tests use `tests/support/corpus.rs` and must pass on any corpus. Run `python3 scripts/test-all.py --corpus DIR` before every PR (locally: `--ignore-rust-version`, rust-version lowered for nested builds) and read its summary.md. `scripts/test-all.py` is the same file in every repository: change it in all of them at once.
- Update claude/START-HERE.md at the end of the session.

## This repository

- Branches: `develop` (default; pull requests, squash merges, `ci-success`
  required) and `main` (merge commits from `develop` only); `v*` tags are
  the maintainer's.
- Nothing from this repository goes on crates.io, ever. OxiGuitar's plugin
  SDK is a git dependency on a tag of github.com/mateusrdp/oxiguitar.
- Rust 1.99 from rustup (`rust-version` in Cargo.toml, `MSRV` in ci.yml).
