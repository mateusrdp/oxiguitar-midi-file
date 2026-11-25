// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The test corpus (`tests/support/corpus.rs`: `OXIGUITAR_CORPUS`, or
//! `corpus/files/`; never committed): every MIDI file in it, recognised by
//! its contents as the manifest's signatures do (a standard MIDI file, or a
//! RIFF MIDI file), reads, and from the second trip writes and reads back
//! the same. Any other file is left alone. `cargo test --test corpus --
//! --nocapture`; with no corpus it checks nothing (unless
//! `OXIGUITAR_CORPUS_REQUIRED` is set).

#[path = "support/corpus.rs"]
mod corpus;

use std::path::Path;

use corpus::Outcome;
use oxiguitar_model::wire;

/// `MThd` at the start, or `RMID` at 8 (the manifest's signatures).
fn is_midi(data: &[u8]) -> bool {
    data.starts_with(b"MThd") || (data.starts_with(b"RIFF") && data.get(8..12) == Some(b"RMID"))
}

fn round_trip(data: &[u8]) -> Result<Outcome, String> {
    if !is_midi(data) {
        return Ok(Outcome::NotTaken);
    }
    let read = |data: &[u8], what: &str| {
        oxiguitar_midi_file_plugin::read(data).map_err(|e| format!("{what} doesn't read: {e}"))
    };
    let write = |score: &_, what: &str| {
        oxiguitar_midi_file_plugin::write(score).map_err(|e| format!("{what} doesn't write: {e}"))
    };
    let first = read(data, "the file")?;
    let second = read(&write(&first.score, "it")?.data, "what it wrote")?;
    let third = read(&write(&second.score, "it")?.data, "what it wrote again")?;
    let a = wire::from_wire2(second.score).map_err(|e| e.to_string())?;
    let b = wire::from_wire2(third.score).map_err(|e| e.to_string())?;
    if a.master_bars.len() != b.master_bars.len() {
        return Err(format!(
            "the second trip changed the bars: {} became {}",
            a.master_bars.len(),
            b.master_bars.len()
        ));
    }
    if a != b {
        return Err("doesn't settle after one round trip".into());
    }
    Ok(Outcome::Passed)
}

#[test]
fn every_corpus_midi_file_round_trips() {
    let fallback = Path::new(env!("CARGO_MANIFEST_DIR")).join("corpus/files");
    corpus::check("midi file round trip", &fallback, |_, data| {
        round_trip(data)
    });
}

/// The picking itself: what the signatures take and what they leave.
#[test]
fn midi_files_are_recognised_by_their_contents() {
    assert!(is_midi(b"MThd\0\0\0\x06\0\x01\0\x01\x01\xe0"));
    assert!(is_midi(b"RIFF\x04\0\0\0RMIDdata"));
    assert!(!is_midi(b"RIFF\x04\0\0\0WAVEfmt "));
    assert!(!is_midi(b"FICHIER GUITAR PRO v3.00"));
    assert!(!is_midi(b""));
}
