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
- Keep README.md up to date with screenshots and example sounds.
- I merge PRs and create v* tags. Whenever I need to do something on git, give me step-by-step instructions with the exact commands.
- Stack PRs to save CI runs; I merge stacks with my merge-stack.sh.
- Open PRs with the REST API (gh pr create fails in your sessions).
- Send test-build links, with their expiry dates, for each PR.
- Locally you only have rustc 1.97: lower rust-version to 1.97 for nested plugin builds, always restore 1.99, and delete target/lib after build-plugins.sh.
- When checking a local test run, look at the exit code and every FAILED line.
- Never rewrite a pushed branch without asking me; never rebase a commit of mine into a branch (the DCO check fails on it).
- Update claude/START-HERE.md at the end of the session.

## This repository

- Branches: `develop` (default; pull requests, squash merges, `ci-success`
  required) and `main` (merge commits from `develop` only); `v*` tags are
  the maintainer's.
- Nothing from this repository goes on crates.io, ever. OxiGuitar's plugin
  SDK is a git dependency on a tag of github.com/mateusrdp/oxiguitar.
- Rust 1.99 from rustup (`rust-version` in Cargo.toml, `MSRV` in ci.yml).
