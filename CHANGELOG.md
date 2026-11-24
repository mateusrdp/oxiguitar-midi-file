# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project
follows [Semantic Versioning](https://semver.org/). A release's number is
the plugin's own; the plugin interface it uses is in `manifest.json` and in
the release notes.

## [Unreleased]

### Added

- **Every test with one command** (mateusrdp/oxiguitar#273): `scripts/test-all.py` builds and runs all of this repository's tests on your own corpus (`--corpus DIR` or `OXIGUITAR_CORPUS`), and tests the plugin against the newest OxiGuitar release, with a progress bar, a summary to paste into an issue and a log to diagnose from. The manual has a Features page naming the tests of every feature (CI checks it) and a Testing page.

### Changed

- **CI outlasts a slow Ubuntu mirror**: it installs packages with `scripts/apt-install.sh`, the same file as in every OxiGuitar repository. apt is stopped only when its downloads have not grown for 2 minutes, then tried again on the next mirror; while they grow, it may take as long as it needs (24 Nov 2026: fixed 5-minute tries failed jobs on mirrors at 80 to 185 kB/s).
- **The manual explains the ideas behind the plugin** (its overview, with pictures), shows how to use it with screenshots, says in Architecture and strategy what it can't do yet, what is left out on purpose and where help is wanted, and has a Credits page listing every project it uses, with links and thanks. The README has a plain-words introduction and a Credits section.

## [0.1.1] - 2026-11-16

### Added

- **A Flatpak extension**, `io.github.mateusrdp.OxiGuitar.Plugin.midi-file`, in
  the release and in OxiGuitar's Flatpak repository, for OxiGuitar
  installed as a Flatpak. 0.1.0 was released without it: its release was
  built before the packaging gained Flatpak extensions.

## [0.1.0] - 2026-11-09

### Added

- **The MIDI file plugin** (`midi-file.oxp`, `oxiguitar.midi-file`, #223 in
  OxiGuitar): reads Standard MIDI Files (.mid, .midi, karaoke .kar) and
  RIFF MIDI files (.rmi), and writes Standard MIDI Files. Reading quantises
  what the file plays onto a grid each beat chooses for itself (straight
  down to a 32nd, or triplets), ties notes over bar lines, keeps tempo,
  time and key signatures, markers and lyrics; General MIDI guitars and
  basses open as tablature, other pitched tracks are fingered with their
  tablature hidden, channel 10 opens as drums. Writing writes what
  OxiGuitar plays: repeats played out, bends, slides and vibrato as pitch
  bend (each string on its own channel where the channels allow), the
  effects as OxiGuitar's MIDI sound plugin plays them. Built on the midly
  crate. Needs OxiGuitar 0.18 (plugin interface 1.7). Design:
  `docs/design.md`.
