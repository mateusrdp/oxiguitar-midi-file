// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The MIDI file format plugin (`midi-file.oxp`, id `oxiguitar.midi-file`):
//! reads and writes Standard MIDI Files (`.mid`, `.midi`, karaoke `.kar`)
//! and reads RIFF MIDI files (`.rmi`), #223. Not to be confused with the
//! MIDI sound plugin (`oxiguitar.midi`), which plays sound sets.
//!
//! The files are parsed and written with the midly crate (Mateus,
//! 9 Nov 2026). [`read()`] quantises what the file plays into OxiGuitar's
//! score; [`write()`] writes what OxiGuitar's player plays. The host
//! recognises the files from the signatures in `manifest.json` (`MThd` at
//! the start, or `RMID` in a RIFF file), without loading the plugin.
//! Design and decisions: `docs/design.md`.

mod instruments;
mod log;
mod read;
mod text;
mod time;
mod write;

use oxiguitar_model::wire;
use oxiguitar_plugin::{
    Error, ErrorKind, FormatReader2, FormatWriter2, Host, Plugin, Read2, Written, score2,
};

/// The id of Standard MIDI Files in `manifest.json`.
pub const MID: &str = "mid";

/// The id of RIFF MIDI files in `manifest.json` (read only).
pub const RMI: &str = "rmi";

/// Reads a MIDI file, plain or RIFF, into OxiGuitar's score and its
/// import log.
pub fn read(data: &[u8]) -> Result<Read2, Error> {
    let imported = read::read(data).map_err(invalid)?;
    imported.score.check_structure().map_err(invalid)?;
    Ok(Read2 {
        score: wire::to_wire2(&imported.score),
        log: imported.log.into_entries(),
    })
}

/// Writes `score` as a Standard MIDI File (format 1), with what the file
/// can't hold.
pub fn write(score: &score2::Score2) -> Result<Written, Error> {
    let score = wire::from_wire2(score.clone()).map_err(invalid)?;
    let (data, losses) = write::write(&score).map_err(invalid)?;
    Ok(Written {
        data,
        losses: losses.into_entries(),
    })
}

/// A file or score that can't be read or written, as the interface carries
/// it.
fn invalid(message: String) -> Error {
    Error {
        kind: ErrorKind::Invalid,
        message,
        offset: None,
        context: String::new(),
    }
}

/// The plugin.
#[derive(Debug)]
pub struct MidiFile;

impl Plugin for MidiFile {
    fn start(_host: Host) -> Result<MidiFile, Error> {
        Ok(MidiFile)
    }

    // The score version 2 (interface 1.2 and later), which holds the
    // palette's marks (1.6) the writer plays.
    fn reader2(&self, format: &str) -> Option<&dyn FormatReader2> {
        matches!(format, MID | RMI).then_some(self as &dyn FormatReader2)
    }

    fn writer2(&self, format: &str) -> Option<&dyn FormatWriter2> {
        (format == MID).then_some(self as &dyn FormatWriter2)
    }
}

impl FormatReader2 for MidiFile {
    fn read(&self, data: &[u8]) -> Result<Read2, Error> {
        read(data)
    }
}

impl FormatWriter2 for MidiFile {
    fn write(&self, score: &score2::Score2) -> Result<Written, Error> {
        write(score)
    }
}

oxiguitar_plugin::export_plugin!(MidiFile);
