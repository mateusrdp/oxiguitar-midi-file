# Testing

Every test of this repository runs with one command, on your own machine, and anyone can run it. When something fails, it tells you how to report it.

## Run everything

```sh
python3 scripts/test-all.py --corpus /path/to/your/files
```

On Windows, type `python` instead of `python3` if that's what you have.

You need:

- Rust, from [rustup](https://rustup.rs) (the version in `Cargo.toml`'s `rust-version` or newer);
- git, and Python 3.8 or newer (nothing to install with pip);
- on Linux, the libraries a build needs: `libasound2-dev`, `libpipewire-0.3-dev` and `libclang-dev` on Debian and Ubuntu (`alsa-lib-devel`, `pipewire-devel` and `clang-devel` on Fedora);

It runs:

1. every Rust test of the repository (`cargo test` over the whole workspace), with a progress bar;
1. the plugin against the **newest release of OxiGuitar**: it fetches that release's source with git (once per release, kept in a cache folder every OxiGuitar repository on your machine shares: `~/.cache/oxiguitar-test-all`, `~/Library/Caches/oxiguitar-test-all` on macOS, `%LOCALAPPDATA%\oxiguitar-test-all` on Windows, or `OXIGUITAR_TEST_CACHE`), builds its `oxiguitar-cli` and `oxiguitar-plugin-test`, builds this repository's plugin file for your system and runs the plugin contract on it;
1. every corpus file the plugin recognises, through that release's `oxiguitar-cli`: recognised, read, written as `.mid` and read back, a second round trip changing nothing;
1. a check of the [Features](features.md) page: every feature names a test, and every test it names exists.

It tests the system and processor it runs on, and only those (as Rust sees them: `rustc -vV`'s host). Tests for other systems are left out and the summary says so; they are not failures.

## Your corpus

The corpus is any folder of files you have (searched recursively, hidden files left out): give it with `--corpus DIR`, or set `OXIGUITAR_CORPUS=DIR`. Each corpus test takes the files it understands and skips the rest, and only checks what must hold for every file (it opens, nothing panics, a round trip is stable), so your corpus doesn't have to be ours to pass. Files are never uploaded, and never belong in the repository.

`cargo test` on its own uses `OXIGUITAR_CORPUS` too, or the repository's ignored `corpus/files/` folder, and quietly checks nothing when neither is there (that's how CI runs). `--no-corpus` runs the script without one.

## What you get

A line per stage while it runs, then the result. In `target/test-all/<date>-<time>/`:

- `summary.md`: the result, your system, what failed and why, short enough to paste into a GitHub issue;
- `test-all.log`: everything that happened, every command and its output, and your corpus's file names, sizes and SHA-256 sums;
- `corpus-*.json`: what each corpus test made of each file.

The script exits with 0 when everything passed, 1 when something failed, and 2 when it couldn't start.

## When something fails

1. Update to the newest `develop` (`git checkout develop && git pull`) and run it again: it may be fixed already.
2. Look for an issue about the same test at [github.com/mateusrdp/oxiguitar-midi-file/issues](https://github.com/mateusrdp/oxiguitar-midi-file/issues).
3. Otherwise [open one](https://github.com/mateusrdp/oxiguitar-midi-file/issues/new): call it `test-all: <the first failing test>`, paste the whole `summary.md`, and attach `test-all.log` (drag it onto the text box; zip it first if it's over 25 MB). Read the log first: it has your folder names and your corpus's file names, so change anything you'd rather keep private.
4. Don't attach corpus files unless they're yours to share. The summary lists each failing file's SHA-256, which is often enough for us to find the same file. If a failing file is yours, attaching it helps a lot.
5. You can also give `summary.md` and `test-all.log` to a language model along with this repository and ask it to find the cause.

## Options

- `--release`: an optimised test build: slower to build, faster corpus tests.
- `--ignore-rust-version`: for a Rust older than `rust-version` (unsupported, but handy).
- `--core DIR`: test against a checkout of OxiGuitar you already have, instead of its newest release; `--core-version v0.21.5` picks a release.
- `--out DIR`: where the results go.

## Adding a feature

A new feature comes with its tests and a row on the [Features](features.md) page, in the same pull request; a new corpus test uses `tests/support/corpus.rs`. The script fails until the row names tests that exist.
