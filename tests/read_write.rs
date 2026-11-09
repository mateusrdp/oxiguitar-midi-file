// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Reading MIDI files made here event by event (as a player would record
//! them, a little off the beat), writing what was read and reading it back.

use midly::num::{u4, u7, u15, u24, u28};
use midly::{Format, Header, MetaMessage, MidiMessage, Smf, Timing, TrackEvent, TrackEventKind};
use oxiguitar_model::{BeatKind, KeyMode, NotePlace, NoteValue, Ratio, Score, Tuplet, wire};

/// An event at an absolute tick.
enum E {
    On(u8, u8, u8),
    Off(u8, u8),
    Program(u8, u8),
    Name(&'static str),
    Lyric(&'static str),
    Text(&'static str),
    Marker(&'static str),
    Tempo(u32),
    Time(u8, u8),
    Key(i8, bool),
}

/// A format 1 file of `tracks`, each a list of (tick, event).
fn smf(per_quarter: u16, tracks: Vec<Vec<(u32, E)>>) -> Vec<u8> {
    let mut smf = Smf::new(Header::new(
        Format::Parallel,
        Timing::Metrical(u15::new(per_quarter)),
    ));
    for mut events in tracks {
        events.sort_by_key(|e| e.0);
        let mut out = Vec::new();
        let mut last = 0;
        for (tick, e) in events {
            let delta = u28::new(tick - last);
            last = tick;
            let midi = |c: u8, message| TrackEventKind::Midi {
                channel: u4::new(c),
                message,
            };
            let kind = match e {
                E::On(c, k, v) => midi(
                    c,
                    MidiMessage::NoteOn {
                        key: u7::new(k),
                        vel: u7::new(v),
                    },
                ),
                E::Off(c, k) => midi(
                    c,
                    MidiMessage::NoteOff {
                        key: u7::new(k),
                        vel: u7::new(0),
                    },
                ),
                E::Program(c, p) => midi(
                    c,
                    MidiMessage::ProgramChange {
                        program: u7::new(p),
                    },
                ),
                E::Name(n) => TrackEventKind::Meta(MetaMessage::TrackName(n.as_bytes())),
                E::Lyric(n) => TrackEventKind::Meta(MetaMessage::Lyric(n.as_bytes())),
                E::Text(n) => TrackEventKind::Meta(MetaMessage::Text(n.as_bytes())),
                E::Marker(n) => TrackEventKind::Meta(MetaMessage::Marker(n.as_bytes())),
                E::Tempo(t) => TrackEventKind::Meta(MetaMessage::Tempo(u24::new(t))),
                E::Time(n, d) => TrackEventKind::Meta(MetaMessage::TimeSignature(n, d, 24, 8)),
                E::Key(f, m) => TrackEventKind::Meta(MetaMessage::KeySignature(f, m)),
            };
            out.push(TrackEvent { delta, kind });
        }
        out.push(TrackEvent {
            delta: u28::new(0),
            kind: TrackEventKind::Meta(MetaMessage::EndOfTrack),
        });
        smf.tracks.push(out);
    }
    let mut data = Vec::new();
    smf.write_std(&mut data).unwrap();
    data
}

/// A note from `at` for `len` ticks on channel `c`.
fn note(at: u32, len: u32, c: u8, key: u8, vel: u8) -> [(u32, E); 2] {
    [(at, E::On(c, key, vel)), (at + len, E::Off(c, key))]
}

/// The test song: a guitar, a bass and drums at 480 ticks a quarter, 100
/// beats a minute, played a little early and late.
fn song() -> Vec<u8> {
    let q = 480;
    let conductor = vec![
        (0, E::Name("Test Song")),
        (0, E::Tempo(600_000)),
        (0, E::Time(4, 2)),
        (0, E::Key(-1, true)),
        (4 * q, E::Marker("Verse")),
        (8 * q, E::Time(3, 2)),
    ];
    let mut guitar = vec![
        (0, E::Name("Lead")),
        (0, E::Program(0, 29)),
        (0, E::Lyric("Hel-")),
        (q + 7, E::Lyric("lo ")),
        (2 * q - 5, E::Lyric("world ")),
    ];
    // Bar 1: three quarters a little off, then eighth-note triplets.
    for (i, k) in [60u8, 64, 67].into_iter().enumerate() {
        let jitter = [0i32, 7, -5][i];
        let at = (i as i32 * q as i32 + jitter) as u32;
        guitar.extend(note(at, q - 20, 0, k, 96));
    }
    for i in 0..3 {
        guitar.extend(note(3 * q + i * q / 3 + 3, q / 3 - 10, 0, 69 + i as u8, 80));
    }
    // Bar 2: sixteenths, then a half note held into bar 3.
    for i in 0..8 {
        let off = if i % 2 == 0 { 4 } else { 0 };
        guitar.extend(note(4 * q + i * q / 4 + off, q / 4 - 15, 0, 72, 100));
    }
    guitar.extend(note(7 * q, 2 * q, 0, 76, 110));
    // Bar 3 (3/4): a chord.
    for k in [52u8, 59, 64] {
        guitar.extend(note(9 * q + 2, q, 0, k, 90));
    }
    let mut bass = vec![(0, E::Name("Bass")), (0, E::Program(1, 33))];
    for b in 0..3 {
        bass.extend(note(b * 4 * q, 2 * q, 1, 28, 100));
    }
    let mut drums = vec![(0, E::Name("Kit"))];
    for i in 0..8 {
        drums.extend(note(i * q, 30, 9, if i % 2 == 0 { 36 } else { 38 }, 110));
    }
    smf(q as u16, vec![conductor, guitar, bass, drums])
}

/// The score a file reads into.
fn read(data: &[u8]) -> Score {
    let read = oxiguitar_midi_file_plugin::read(data).expect("the file reads");
    wire::from_wire2(read.score).expect("the score converts")
}

/// Each note struck on a track's first staff: (bar, position in the bar,
/// key, length to the end of its ties), in order.
fn struck(score: &Score, track: usize) -> Vec<(usize, Ratio, u8, Ratio)> {
    let staff = &score.tracks[track].staves[0];
    let mut out: Vec<(usize, Ratio, u8, Ratio)> = Vec::new();
    // Index into `out` of the note each key is sounding.
    let mut sounding: std::collections::BTreeMap<u8, usize> = Default::default();
    for (b, bar) in staff.bars.iter().enumerate() {
        let mut at = Ratio::ZERO;
        for beat in &bar.voices[0].beats {
            let len = beat.duration.length().unwrap();
            for n in &beat.notes {
                let NotePlace::Pitched(p) = n.place else {
                    panic!("MIDI notes come with pitches only");
                };
                match (n.tie, sounding.get(&p.0)) {
                    (true, Some(&i)) => out[i].3 = out[i].3 + len,
                    _ => {
                        sounding.insert(p.0, out.len());
                        out.push((b, at, p.0, len));
                    }
                }
            }
            at = at + len;
        }
    }
    out
}

fn r(n: i64, d: i64) -> Ratio {
    Ratio::new(n, d).unwrap()
}

#[test]
fn a_played_file_reads_on_the_grid() {
    let score = read(&song());
    assert_eq!(score.info.title, "Test Song");
    assert_eq!(score.initial_tempo, 100.0);
    assert_eq!(score.tracks.len(), 3);
    let names: Vec<&str> = score.tracks.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(names, ["Lead", "Bass", "Kit"]);
    let guitar = &score.tracks[0];
    assert_eq!(guitar.instrument.midi_program, 29);
    assert!(
        guitar.staves[0].show_tablature,
        "a guitar shows its tablature"
    );
    assert_eq!(guitar.staves[0].tuning.len(), 6);
    let bass = &score.tracks[1];
    assert_eq!(bass.staves[0].tuning.len(), 4);
    assert!(bass.staves[0].show_tablature);
    let drums = &score.tracks[2];
    assert!(drums.instrument.percussion);
    assert!(drums.staves[0].tuning.is_empty());
    // Bars: 4/4, 4/4, then 3/4, in D minor; the marker on bar 2.
    let times: Vec<(u8, u8)> = score
        .master_bars
        .iter()
        .map(|m| (m.time_signature.numerator, m.time_signature.denominator))
        .collect();
    assert_eq!(times, [(4, 4), (4, 4), (3, 4)]);
    assert_eq!(score.master_bars[0].key_signature.fifths, -1);
    assert_eq!(score.master_bars[0].key_signature.mode, KeyMode::Minor);
    assert_eq!(
        score.master_bars[1]
            .marker
            .as_ref()
            .map(|m| m.text.as_str()),
        Some("Verse")
    );
    let notes = struck(&score, 0);
    // The quarters, on the beat, as long as they were held (a little
    // short of a quarter: on the grid, a quarter).
    assert_eq!(notes[0], (0, r(0, 1), 60, r(1, 4)));
    assert_eq!(notes[1], (0, r(1, 4), 64, r(1, 4)));
    assert_eq!(notes[2], (0, r(1, 2), 67, r(1, 4)));
    // The triplets.
    assert_eq!(notes[3], (0, r(3, 4), 69, r(1, 12)));
    assert_eq!(notes[4], (0, r(3, 4) + r(1, 12), 70, r(1, 12)));
    let beats = &guitar.staves[0].bars[0].voices[0].beats;
    let last = beats.last().unwrap();
    assert_eq!(last.duration.value, NoteValue::Eighth);
    assert_eq!(last.duration.tuplets, vec![Tuplet::TRIPLET]);
    // The sixteenths, then the half note tied into bar 3.
    for i in 0..6 {
        assert_eq!(notes[6 + i], (1, r(i as i64, 16), 72, r(1, 16)));
    }
    let held = notes.iter().find(|n| n.2 == 76).unwrap();
    assert_eq!(*held, (1, r(3, 4), 76, r(1, 2)));
    assert!(guitar.staves[0].bars[2].voices[0].beats[0].notes[0].tie);
    // The chord in bar 3, a quarter after the tie.
    let chord: Vec<_> = notes.iter().filter(|n| n.0 == 2 && n.2 != 76).collect();
    assert_eq!(chord.len(), 3);
    assert!(chord.iter().all(|n| n.1 == r(1, 4) && n.3 == r(1, 4)));
    // Lyrics on the sung beats.
    assert_eq!(guitar.lyrics[0].text, "Hel-lo world");
    // Drums: a hit each beat, lasting to the next.
    let hits = struck(&score, 2);
    assert_eq!(hits.len(), 8);
    assert!(hits.iter().all(|h| h.3 == r(1, 4)));
    assert_eq!(hits[1].2, 38);
    // Rests where nothing plays.
    let bass_beats = &bass.staves[0].bars[0].voices[0].beats;
    assert_eq!(bass_beats[1].kind, BeatKind::Rest);
}

#[test]
fn what_is_read_is_written_and_reads_back_the_same() {
    let score = read(&song());
    let written = oxiguitar_midi_file_plugin::write(&wire::to_wire2(&score)).expect("writes");
    assert!(
        written
            .losses
            .iter()
            .any(|l| l.message.contains("notation"))
    );
    let again = read(&written.data);
    assert_eq!(again.tracks.len(), score.tracks.len());
    for t in 0..score.tracks.len() {
        assert_eq!(struck(&again, t), struck(&score, t), "track {}", t + 1);
        assert_eq!(again.tracks[t].name, score.tracks[t].name);
        assert_eq!(
            again.tracks[t].instrument,
            score.tracks[t].instrument,
            "track {}",
            t + 1
        );
    }
    assert_eq!(again.initial_tempo, score.initial_tempo);
    assert_eq!(again.info.title, score.info.title);
    let bars = |s: &Score| {
        s.master_bars
            .iter()
            .map(|m| (m.time_signature, m.key_signature, m.marker.clone()))
            .collect::<Vec<_>>()
    };
    assert_eq!(bars(&again), bars(&score));
    assert_eq!(again.tracks[0].lyrics, score.tracks[0].lyrics);
}

#[test]
fn repeats_are_written_out() {
    let mut score = read(&song());
    score.master_bars[0].repeat_start = true;
    score.master_bars[0].repeat_end = Some(2);
    let written = oxiguitar_midi_file_plugin::write(&wire::to_wire2(&score)).unwrap();
    assert!(written.losses.iter().any(|l| l.message.contains("repeats")));
    let again = read(&written.data);
    assert_eq!(again.master_bars.len(), 4, "bar 1 twice, then bars 2 and 3");
    let first: Vec<_> = struck(&again, 0).into_iter().filter(|n| n.0 == 0).collect();
    let second: Vec<_> = struck(&again, 0).into_iter().filter(|n| n.0 == 1).collect();
    assert_eq!(first.len(), second.len());
}

#[test]
fn a_riff_file_reads_like_the_file_inside() {
    let inner = song();
    let mut riff = Vec::new();
    let size = u32::try_from(inner.len() + 12).unwrap();
    riff.extend_from_slice(b"RIFF");
    riff.extend_from_slice(&size.to_le_bytes());
    riff.extend_from_slice(b"RMIDdata");
    riff.extend_from_slice(&u32::try_from(inner.len()).unwrap().to_le_bytes());
    riff.extend_from_slice(&inner);
    assert_eq!(struck(&read(&riff), 0), struck(&read(&inner), 0));
}

#[test]
fn other_files_are_refused() {
    for data in [
        &b""[..],
        b"MThd",
        b"RIFF\0\0\0\0WAVEfmt ",
        b"not music at all",
    ] {
        assert!(oxiguitar_midi_file_plugin::read(data).is_err());
    }
}

#[test]
fn an_empty_file_reads_as_one_empty_bar() {
    let score = read(&smf(480, vec![vec![(0, E::Name("Nothing"))]]));
    assert_eq!(score.master_bars.len(), 1);
    assert_eq!(score.tracks.len(), 1);
}

#[test]
fn a_karaoke_file_reads_its_title_and_words() {
    let q = 480;
    let words = vec![
        (0, E::Name("Words")),
        (0, E::Text("@KMIDI KARAOKE FILE")),
        (0, E::Text("@TSong of Tests")),
        (0, E::Text("@TThe Testers")),
        (0, E::Text("/Hel")),
        (q, E::Text("lo")),
        (2 * q, E::Text(" world")),
    ];
    let mut melody = vec![(0, E::Name("Melody")), (0, E::Program(0, 73))];
    for i in 0..3 {
        melody.extend(note(i * q, q, 0, 72, 90));
    }
    let score = read(&smf(q as u16, vec![words, melody]));
    assert_eq!(score.info.title, "Song of Tests");
    assert_eq!(score.info.artist, "The Testers");
    assert_eq!(score.tracks.len(), 1);
    let flute = &score.tracks[0];
    assert_eq!(flute.name, "Melody");
    assert!(!flute.staves[0].show_tablature, "a flute shows notation only");
    assert_eq!(flute.staves[0].tuning.len(), 6, "with a guitar's strings to finger");
    assert_eq!(flute.lyrics[0].text, "Hel-lo world");
}
