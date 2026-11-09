# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project
follows [Semantic Versioning](https://semver.org/). A release's number is
the plugin's own; the plugin interface it uses is in `manifest.json` and in
the release notes.

## [Unreleased]

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
