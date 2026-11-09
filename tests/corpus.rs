// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The private test corpus, when it is here (never committed): every MIDI
//! file in `corpus/files/` reads, and what it reads into writes and reads
//! back the same from the second trip on (the first trip quantises and
//! plays out repeats). `cargo test --test corpus -- --nocapture` locally;
//! with no corpus it checks nothing. Guitar Pro files of the corpus become
//! MIDI files with `oxiguitar-cli convert` (see docs/design.md).

use oxiguitar_model::wire;

#[test]
fn every_corpus_file_round_trips() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("corpus/files");
    let Ok(entries) = std::fs::read_dir(&dir) else {
        println!("no corpus in {}", dir.display());
        return;
    };
    let mut checked = 0;
    let mut paths: Vec<_> = entries.flatten().map(|e| e.path()).collect();
    paths.sort();
    for path in paths {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        if !matches!(ext.as_str(), "mid" | "midi" | "kar" | "rmi") {
            continue;
        }
        let data = std::fs::read(&path).unwrap();
        let first = oxiguitar_midi_file_plugin::read(&data)
            .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let written = oxiguitar_midi_file_plugin::write(&first.score).unwrap();
        let second = oxiguitar_midi_file_plugin::read(&written.data).unwrap();
        let written = oxiguitar_midi_file_plugin::write(&second.score).unwrap();
        let third = oxiguitar_midi_file_plugin::read(&written.data).unwrap();
        let (a, b) = (
            wire::from_wire2(second.score).unwrap(),
            wire::from_wire2(third.score).unwrap(),
        );
        assert_eq!(
            a.master_bars.len(),
            b.master_bars.len(),
            "{}",
            path.display()
        );
        assert!(a == b, "{} doesn't round-trip", path.display());
        println!(
            "{}: {} bars, {} tracks, {} log entries",
            path.display(),
            a.master_bars.len(),
            a.tracks.len(),
            first.log.len()
        );
        checked += 1;
    }
    println!("{checked} files");
}
