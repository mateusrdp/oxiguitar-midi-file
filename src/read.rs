// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Reading a Standard MIDI File into OxiGuitar's score (docs/design.md,
//! "Reading").
//!
//! Each channel a track of the file plays notes on becomes a track of the
//! score (a format 0 file keeps all its channels in one track). The bars
//! come from the file's time signatures, the tempo, key signatures and
//! markers from its meta events, wherever they are.
//!
//! A MIDI file holds played times, not note values, so every note's start
//! and end are **quantised** (Mateus, 9 Nov 2026: automatically, with no
//! dialog): each beat of each track takes the grid that fits its notes
//! best, straight (down to a 32nd) or triplet, a finer grid paying a small
//! price so that a slightly late sixteenth stays a sixteenth. The notes of
//! a track are then cut at every start and end into one voice: notes
//! sounding on from an earlier cut, or across a bar line, continue as tied
//! notes, so what plays is what the file played, on the grid.
//!
//! Notes come with pitches only; OxiGuitar gives them strings and frets
//! as the file opens (#157).

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use midly::{Format, MetaMessage, MidiMessage, Smf, Timing, TrackEventKind};
use oxiguitar_model::{
    Bar, Beat, BeatKind, Color, Dynamic, Instrument, KeyMode, KeySignature, LyricLine, Marker,
    MasterBar, Mix, Note, NoteEffects, NotePlace, Pitch, Ratio, Score, ScoreInfo, Staff,
    TempoChange, TimeSignature, Track, Voice,
};

use crate::instruments::{Kind, program_name};
use crate::log::{Log, place};
use crate::text::decode;
use crate::time;

/// Time is counted in units of a 192nd of a whole note: every grid point
/// (straight down to a 64th, triplets down to a 32nd triplet) is a whole
/// number of them.
pub(crate) const UNITS: i64 = 192;

/// A quarter note, in units.
const QUARTER: f64 = 48.0;

/// The most bars a file is read to; what lies past them is left out. Long
/// songs have a few hundred.
const MAX_BARS: usize = 4096;

/// A score read from a file, with what the file held that it couldn't.
#[derive(Debug)]
pub(crate) struct Imported {
    /// The score.
    pub(crate) score: Score,
    /// The import log.
    pub(crate) log: Log,
}

/// A note as the file plays it, in units from the start (not yet on a grid).
#[derive(Clone, Copy, Debug)]
struct RawNote {
    key: u8,
    start: f64,
    end: f64,
    velocity: u8,
}

/// A note on the grid, in units.
#[derive(Clone, Copy, Debug)]
struct GridNote {
    key: u8,
    start: i64,
    end: i64,
    velocity: u8,
}

/// The notes of one channel of one track of the file: a track of the score.
#[derive(Debug, Default)]
struct Part {
    /// The file's track it is in.
    file_track: usize,
    channel: u8,
    program: Option<u8>,
    /// The program changed after the first note.
    program_changed: bool,
    volume: Option<u8>,
    pan: Option<u8>,
    notes: Vec<RawNote>,
}

/// One track of the file, as read.
#[derive(Debug, Default)]
struct FileTrack {
    name: String,
    instrument: String,
    /// Lyrics, in units from the start.
    lyrics: Vec<(f64, String)>,
    /// Texts at its start, for the header.
    texts: Vec<String>,
    /// Karaoke headers (`@T…`), from a .kar file.
    karaoke: Vec<String>,
}

/// The file's events that every track shares, in units from the start.
#[derive(Debug, Default)]
struct Conductor {
    /// Microseconds a quarter note.
    tempos: Vec<(f64, u32)>,
    /// Numerator and denominator.
    times: Vec<(f64, u8, u8)>,
    keys: Vec<(f64, i8, bool)>,
    markers: Vec<(f64, String)>,
    copyright: String,
}

/// Reads a MIDI file (plain, or wrapped in RIFF as `.rmi`).
pub(crate) fn read(data: &[u8]) -> Result<Imported, String> {
    if division(data).is_some_and(|d| d & 0xff00 == 0x8000) {
        // midly 0.5.3 negates this frame rate (-128) as an i8, which
        // overflows where overflow is checked; it is no frame rate anyway.
        return Err("not a MIDI file that can be read: invalid SMPTE frame rate".to_owned());
    }
    let smf = Smf::parse(data).map_err(|e| format!("not a MIDI file that can be read: {e}"))?;
    let mut log = Log::default();
    // Ticks a quarter note. A file timed in frames a second (SMPTE) is
    // read at 120 quarter notes a minute.
    let per_quarter = match smf.header.timing {
        Timing::Metrical(t) => f64::from(t.as_int()),
        Timing::Timecode(fps, sub) => {
            log.note(
                "a file timed in frames a second is read at 120 beats a minute",
                "the whole file",
            );
            f64::from(fps.as_f32()) * f64::from(sub) / 2.0
        }
    };
    if per_quarter < 1.0 {
        return Err("not a MIDI file that can be read: its timing is zero".to_owned());
    }
    let units = |tick: u64| tick as f64 * QUARTER / per_quarter;
    let tracks: &[Vec<midly::TrackEvent<'_>>] = if smf.header.format == Format::Sequential {
        if smf.tracks.len() > 1 {
            log.lost(
                "the file holds several songs (format 2): only the first is read",
                "the whole file",
            );
        }
        &smf.tracks[..smf.tracks.len().min(1)]
    } else {
        &smf.tracks
    };
    let mut conductor = Conductor::default();
    let mut parts: Vec<Part> = Vec::new();
    let mut file_tracks: Vec<FileTrack> = Vec::new();
    let mut bends = false;
    for (ti, events) in tracks.iter().enumerate() {
        let mut ft = FileTrack::default();
        let mut tick: u64 = 0;
        // Notes sounding: (channel, key) to their starts and velocities,
        // first in first out.
        let mut open: BTreeMap<(u8, u8), VecDeque<(f64, u8)>> = BTreeMap::new();
        let first_part = parts.len();
        let part_of = |parts: &mut Vec<Part>, channel: u8| -> usize {
            match parts[first_part..]
                .iter()
                .position(|p| p.channel == channel)
            {
                Some(i) => first_part + i,
                None => {
                    parts.push(Part {
                        file_track: ti,
                        channel,
                        ..Part::default()
                    });
                    parts.len() - 1
                }
            }
        };
        for e in events {
            tick = tick.saturating_add(u64::from(e.delta.as_int()));
            let at = units(tick);
            match e.kind {
                TrackEventKind::Midi { channel, message } => {
                    let channel = channel.as_int();
                    let p = part_of(&mut parts, channel);
                    let part = &mut parts[p];
                    match message {
                        MidiMessage::NoteOn { key, vel } if vel.as_int() > 0 => {
                            open.entry((channel, key.as_int()))
                                .or_default()
                                .push_back((at, vel.as_int()));
                        }
                        MidiMessage::NoteOn { key, .. } | MidiMessage::NoteOff { key, .. } => {
                            if let Some((start, velocity)) = open
                                .get_mut(&(channel, key.as_int()))
                                .and_then(VecDeque::pop_front)
                            {
                                part.notes.push(RawNote {
                                    key: key.as_int(),
                                    start,
                                    end: at,
                                    velocity,
                                });
                            }
                        }
                        MidiMessage::ProgramChange { program } => {
                            let program = program.as_int();
                            if part.notes.is_empty() && !open.keys().any(|k| k.0 == channel) {
                                part.program = Some(program);
                            } else if part.program != Some(program) {
                                part.program_changed = true;
                            }
                        }
                        MidiMessage::Controller { controller, value } => {
                            let started = !part.notes.is_empty();
                            match controller.as_int() {
                                7 if !started => part.volume = Some(value.as_int()),
                                10 if !started => part.pan = Some(value.as_int()),
                                _ => {}
                            }
                        }
                        MidiMessage::PitchBend { bend } if bend.as_int() != 0 => bends = true,
                        _ => {}
                    }
                }
                TrackEventKind::Meta(meta) => match meta {
                    MetaMessage::Tempo(t) if t.as_int() > 0 => {
                        if matches!(smf.header.timing, Timing::Metrical(_)) {
                            conductor.tempos.push((at, t.as_int()));
                        }
                    }
                    MetaMessage::TimeSignature(n, d, _, _) if n > 0 => {
                        if d > 6 {
                            log.note(
                                "time signatures below a 64th are read as 64ths",
                                "the whole file",
                            );
                        }
                        conductor.times.push((at, n, 1u8 << d.min(6)));
                    }
                    MetaMessage::KeySignature(fifths, minor) => {
                        conductor.keys.push((at, fifths.clamp(-7, 7), minor));
                    }
                    MetaMessage::Marker(t) | MetaMessage::CuePoint(t) => {
                        let t = decode(t);
                        if !t.is_empty() {
                            conductor.markers.push((at, t));
                        }
                    }
                    MetaMessage::Copyright(t) if conductor.copyright.is_empty() => {
                        conductor.copyright = decode(t);
                    }
                    MetaMessage::TrackName(t) if ft.name.is_empty() => ft.name = decode(t),
                    MetaMessage::InstrumentName(t) if ft.instrument.is_empty() => {
                        ft.instrument = decode(t);
                    }
                    MetaMessage::Lyric(t) => {
                        let raw = String::from_utf8_lossy(t);
                        let ends_word = raw.ends_with([' ', '\r', '\n', '/', '\\']);
                        let mut t = decode(t);
                        if ends_word && !t.is_empty() && !t.ends_with('-') {
                            t.push(' ');
                        }
                        ft.lyrics.push((at, t));
                    }
                    MetaMessage::Text(raw) => {
                        let t = decode(raw);
                        if t.starts_with('@') {
                            ft.karaoke.push(t);
                        } else if !ft.karaoke.is_empty() {
                            // A karaoke file's syllables: a slash or a
                            // backslash starts a line, a space a word.
                            let word = matches!(raw.first(), Some(b' ' | b'/' | b'\\'));
                            let syllable = t.trim_start_matches(['/', '\\', ' ']).to_owned();
                            // The syllable before ends its word, or the
                            // word goes on.
                            if let Some(last) = ft.lyrics.last_mut()
                                && !last.1.ends_with(' ')
                                && !last.1.ends_with('-')
                            {
                                last.1.push(if word { ' ' } else { '-' });
                            }
                            ft.lyrics.push((at, syllable));
                        } else if tick == 0 && !t.is_empty() {
                            ft.texts.push(t);
                        }
                    }
                    _ => {}
                },
                _ => {}
            }
        }
        // Notes the track never ends: they sound to its end.
        let end = units(tick);
        for ((channel, key), starts) in open {
            let p = part_of(&mut parts, channel);
            for (start, velocity) in starts {
                parts[p].notes.push(RawNote {
                    key,
                    start,
                    end: end.max(start),
                    velocity,
                });
            }
        }
        file_tracks.push(ft);
    }
    if bends {
        log.lost(
            "pitch bends are left out (bends and slides in OxiGuitar are marks on notes)",
            "the whole file",
        );
    }
    parts.retain(|p| !p.notes.is_empty());
    let parts = merge(parts);
    Ok(Builder {
        conductor,
        parts,
        file_tracks,
        format: smf.header.format,
        log,
        cell_kinds: CellKinds::new(),
    }
    .build())
}

/// The header's division word (timing), found as midly finds it: in the
/// file, or in a RIFF file's `data` chunk, in the first `MThd` chunk
/// before any track. `None` when there is none to read.
fn division(data: &[u8]) -> Option<u16> {
    let mut smf = data;
    if data.starts_with(b"RIFF") {
        // RIFF chunks: id, length (little-endian), data, a pad byte after
        // odd lengths. The first is RIFF, holding the form type and then
        // the chunks, among them `data`.
        let riff = chunks(data, true).next()?.1;
        let inner = riff.get(4..)?;
        smf = chunks(inner, true).find(|c| &c.0 == b"data")?.1;
    }
    for (id, chunk) in chunks(smf, false) {
        match &id {
            b"MThd" => return chunk.get(4..6).map(|d| u16::from_be_bytes([d[0], d[1]])),
            b"MTrk" => return None,
            _ => {}
        }
    }
    None
}

/// The chunks of `raw`: (id, data). RIFF chunks have little-endian
/// lengths and a pad byte after odd ones; MIDI chunks big-endian lengths
/// and no pad. A chunk running past the end holds what is left.
fn chunks(mut raw: &[u8], riff: bool) -> impl Iterator<Item = ([u8; 4], &[u8])> {
    std::iter::from_fn(move || {
        let id: [u8; 4] = raw.get(..4)?.try_into().ok()?;
        let len: [u8; 4] = raw.get(4..8)?.try_into().ok()?;
        let len = if riff {
            u32::from_le_bytes(len)
        } else {
            u32::from_be_bytes(len)
        };
        let len = usize::try_from(len).unwrap_or(usize::MAX);
        let rest = &raw[8..];
        let (chunk, after) = rest.split_at(len.min(rest.len()));
        raw = if riff && len % 2 == 1 {
            after.get(1..).unwrap_or(&[])
        } else {
            after
        };
        Some((id, chunk))
    })
}

/// One part for the channels of a track of the file that play the same
/// program: a guitar recorded with a channel for each string (as MIDI
/// guitars and OxiGuitar write them) is one track. Drums stay apart.
fn merge(parts: Vec<Part>) -> Vec<Part> {
    let mut out: Vec<Part> = Vec::with_capacity(parts.len());
    for p in parts {
        let same = out.iter_mut().find(|o| {
            o.file_track == p.file_track
                && (o.channel == 9) == (p.channel == 9)
                && o.program.unwrap_or(0) == p.program.unwrap_or(0)
        });
        match same {
            Some(o) => {
                o.program_changed |= p.program_changed;
                o.volume = o.volume.or(p.volume);
                o.pan = o.pan.or(p.pan);
                o.notes.extend(p.notes);
            }
            None => out.push(p),
        }
    }
    out
}

/// A bar of the score being built, in units.
#[derive(Clone, Copy, Debug)]
struct BarSpan {
    start: i64,
    end: i64,
    time: TimeSignature,
}

impl BarSpan {
    /// The length of a beat-sized cell of the grid: a dotted beat in a
    /// compound time (6/8, 9/8, 12/8, 6/16…), a quarter otherwise.
    fn cell(&self) -> i64 {
        let TimeSignature {
            numerator: n,
            denominator: d,
        } = self.time;
        if d >= 8 && n % 3 == 0 {
            3 * UNITS / i64::from(d)
        } else {
            UNITS / 4
        }
    }
}

/// A grid: how many parts a cell is divided into, and what a finer grid
/// costs per note (a fraction of the cell's length).
#[derive(Clone, Copy, Debug)]
struct Grid {
    parts: i64,
    price: f64,
}

/// The grids of a quarter-note cell: straight down to 32nds, triplets of
/// eighths and sixteenths.
const SIMPLE: [Grid; 6] = [
    Grid {
        parts: 1,
        price: 0.0,
    },
    Grid {
        parts: 2,
        price: 0.01,
    },
    Grid {
        parts: 4,
        price: 0.025,
    },
    Grid {
        parts: 8,
        price: 0.06,
    },
    Grid {
        parts: 3,
        price: 0.03,
    },
    Grid {
        parts: 6,
        price: 0.05,
    },
];

/// The grids of a dotted cell: its three beats, their halves and quarters.
const COMPOUND: [Grid; 4] = [
    Grid {
        parts: 1,
        price: 0.0,
    },
    Grid {
        parts: 3,
        price: 0.01,
    },
    Grid {
        parts: 6,
        price: 0.025,
    },
    Grid {
        parts: 12,
        price: 0.06,
    },
];

struct Builder {
    conductor: Conductor,
    parts: Vec<Part>,
    file_tracks: Vec<FileTrack>,
    format: Format,
    log: Log,
    /// The current part's cells, triplet or not.
    cell_kinds: CellKinds,
}

impl Builder {
    fn build(mut self) -> Imported {
        let end = self
            .parts
            .iter()
            .flat_map(|p| p.notes.iter().map(|n| n.end.max(n.start)))
            .chain(
                self.file_tracks
                    .iter()
                    .flat_map(|t| t.lyrics.iter().map(|l| l.0)),
            )
            .fold(0.0_f64, f64::max);
        let bars = self.bars(end);
        let info = self.info();
        let mut master_bars: Vec<MasterBar> = bars
            .iter()
            .map(|b| MasterBar {
                time_signature: b.time,
                ..MasterBar::default()
            })
            .collect();
        self.keys(&bars, &mut master_bars);
        self.markers(&bars, &mut master_bars);
        let initial_tempo = self.tempos(&bars, &mut master_bars);
        // Which track the lyrics of a track without notes (or a karaoke
        // file's) go to: the first pitched one.
        let first_pitched = self.parts.iter().position(|p| p.channel != 9).unwrap_or(0);
        let mut lyrics_for: Vec<Option<usize>> = vec![None; self.parts.len()];
        for (fi, ft) in self.file_tracks.iter().enumerate() {
            if ft.lyrics.is_empty() {
                continue;
            }
            let own = self
                .parts
                .iter()
                .position(|p| p.file_track == fi && p.channel != 9);
            if let Some(slot) = lyrics_for.get_mut(own.unwrap_or(first_pitched))
                && slot.is_none()
            {
                *slot = Some(fi);
            }
        }
        let mut tracks = Vec::new();
        let parts = std::mem::take(&mut self.parts);
        let own_channels = |fi: usize| parts.iter().filter(|p| p.file_track == fi).count();
        for (pi, part) in parts.iter().enumerate() {
            let lyrics = lyrics_for[pi]
                .map(|fi| std::mem::take(&mut self.file_tracks[fi].lyrics))
                .unwrap_or_default();
            let alone = own_channels(part.file_track) == 1 && self.format != Format::SingleTrack;
            let track = self.track(pi, part, alone, &bars, &lyrics);
            tracks.push(track);
        }
        if tracks.is_empty() {
            tracks.push(Track {
                name: "Track 1".to_owned(),
                color: Color { r: 255, g: 0, b: 0 },
                staves: vec![Staff {
                    tuning: Kind::Guitar.tuning(None),
                    bars: bars.iter().map(|b| rest_bar(b, Kind::Guitar)).collect(),
                    ..Staff::default()
                }],
                ..Track::default()
            });
        }
        let score = Score {
            info,
            initial_tempo,
            tempo_label: String::new(),
            master_bars,
            tracks,
            layout: Vec::new(),
        };
        Imported {
            score,
            log: self.log,
        }
    }

    /// The bars, from the time signatures, until `end` (in units).
    fn bars(&mut self, end: f64) -> Vec<BarSpan> {
        // Time signatures on the finest written grid (a 64th, 3 units).
        let mut times: Vec<(i64, TimeSignature)> = self
            .conductor
            .times
            .iter()
            .map(|&(at, n, d)| {
                (
                    snap_to(at, 3),
                    TimeSignature {
                        numerator: n,
                        denominator: d,
                    },
                )
            })
            .collect();
        times.sort_by_key(|t| t.0);
        let end = end.min(1e12).ceil() as i64;
        let mut bars = Vec::new();
        let mut time = TimeSignature::COMMON;
        let mut next = 0;
        let mut start = 0i64;
        loop {
            while next < times.len() && times[next].0 <= start {
                time = times[next].1;
                next += 1;
            }
            let full = i64::from(time.numerator) * UNITS / i64::from(time.denominator.max(1));
            let mut bar = BarSpan {
                start,
                end: start + full.max(3),
                time,
            };
            // A time signature inside the bar ends it there: the bar is as
            // long as the file plays it, with a time signature to match.
            if let Some(&(at, _)) = times.get(next)
                && at < bar.end
            {
                bar.end = at;
                if let Some(t) = signature_of(at - start, time) {
                    bar.time = t;
                }
            }
            start = bar.end;
            bars.push(bar);
            if bars.len() >= MAX_BARS {
                if start < end {
                    self.log.lost(
                        format!("bars past the {MAX_BARS}th are left out"),
                        place(MAX_BARS, 0),
                    );
                }
                break;
            }
            if start >= end {
                break;
            }
        }
        bars
    }

    /// The header: the title is the first track's name when that track
    /// plays nothing (or the only track's, in a format 0 file).
    fn info(&mut self) -> ScoreInfo {
        let mut info = ScoreInfo {
            copyright: self.conductor.copyright.clone(),
            ..ScoreInfo::default()
        };
        let first_plays = self.parts.iter().any(|p| p.file_track == 0);
        if let Some(first) = self.file_tracks.first()
            && (self.format == Format::SingleTrack || !first_plays)
        {
            info.title = first.name.clone();
        }
        // A karaoke file's headers: titles first, then the rest.
        let karaoke: Vec<String> = self
            .file_tracks
            .iter()
            .flat_map(|t| t.karaoke.iter())
            .filter_map(|k| k.strip_prefix("@T").map(str::trim).map(str::to_owned))
            .filter(|k| !k.is_empty())
            .collect();
        let mut karaoke = karaoke.into_iter();
        if let Some(t) = karaoke.next() {
            info.title = t;
        }
        if let Some(a) = karaoke.next() {
            info.artist = a;
        }
        // Texts at the start of tracks without notes: "Artist: …" as
        // OxiGuitar writes them, anything else as a notice.
        for (fi, ft) in self.file_tracks.iter().enumerate() {
            if self.parts.iter().any(|p| p.file_track == fi) {
                continue;
            }
            for t in &ft.texts {
                let field = |prefix: &str| t.strip_prefix(prefix).map(|v| v.trim().to_owned());
                if let Some(v) = field("Subtitle:") {
                    info.subtitle = v;
                } else if let Some(v) = field("Artist:") {
                    info.artist = v;
                } else if let Some(v) = field("Album:") {
                    info.album = v;
                } else if let Some(v) = field("Words:") {
                    info.words = v;
                } else if let Some(v) = field("Music:") {
                    info.music = v;
                } else if let Some(v) = field("Transcriber:") {
                    info.transcriber = v;
                } else if let Some(v) = field("Instructions:") {
                    info.instructions = v;
                } else if info.notices.len() < 32 {
                    info.notices.push(t.clone());
                }
            }
        }
        info
    }

    /// Key signatures, each from the bar it falls nearest to the start of.
    fn keys(&mut self, bars: &[BarSpan], master: &mut [MasterBar]) {
        let mut keys = self.conductor.keys.clone();
        keys.sort_by(|a, b| a.0.total_cmp(&b.0));
        for (at, fifths, minor) in keys {
            let from = nearest_bar(bars, at);
            let key = KeySignature {
                fifths,
                mode: if minor {
                    KeyMode::Minor
                } else {
                    KeyMode::Major
                },
            };
            for m in master.iter_mut().skip(from) {
                m.key_signature = key;
            }
        }
    }

    /// Markers, on the bar they fall nearest to the start of.
    fn markers(&mut self, bars: &[BarSpan], master: &mut [MasterBar]) {
        let mut markers = self.conductor.markers.clone();
        markers.sort_by(|a, b| a.0.total_cmp(&b.0));
        for (at, text) in markers {
            let Some(m) = master.get_mut(nearest_bar(bars, at)) else {
                continue;
            };
            match &mut m.marker {
                Some(marker) if marker.text != text => {
                    marker.text.push_str(" / ");
                    marker.text.push_str(&text);
                }
                Some(_) => {}
                None => {
                    m.marker = Some(Marker {
                        letter: String::new(),
                        text,
                        color: Color { r: 255, g: 0, b: 0 },
                    });
                }
            }
        }
    }

    /// The tempo changes, on a sixteenth's grid inside their bars; returns
    /// the tempo the song starts in.
    fn tempos(&mut self, bars: &[BarSpan], master: &mut [MasterBar]) -> f64 {
        let mut tempos = self.conductor.tempos.clone();
        tempos.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut initial = 120.0;
        let mut current: Option<f64> = None;
        for (at, micros) in tempos {
            let bpm = (60_000_000.0 / f64::from(micros) * 100.0).round() / 100.0;
            let at = snap_to(at, UNITS / 16);
            let b = bars.partition_point(|b| b.end <= at).min(bars.len() - 1);
            let offset = (at - bars[b].start).max(0);
            if current == Some(bpm) {
                continue;
            }
            current = Some(bpm);
            if b == 0 && offset == 0 {
                initial = bpm;
                master[0].tempo_changes.clear();
                continue;
            }
            let Some(position) = Ratio::new(offset, UNITS) else {
                continue;
            };
            let changes = &mut master[b].tempo_changes;
            changes.retain(|c| c.position != position);
            changes.push(TempoChange {
                position,
                bpm,
                label: String::new(),
                linear: false,
            });
        }
        initial
    }

    /// One part of the file as a track of the score.
    fn track(
        &mut self,
        index: usize,
        part: &Part,
        alone: bool,
        bars: &[BarSpan],
        lyrics: &[(f64, String)],
    ) -> Track {
        let program = part.program.unwrap_or(0);
        let ft = &self.file_tracks[part.file_track];
        let given = [&ft.name, &ft.instrument]
            .into_iter()
            .find(|n| !n.is_empty())
            .cloned();
        let kind = Kind::of(part.channel, program, given.as_deref().unwrap_or(""));
        let name = match (alone, given) {
            (true, Some(n)) => n,
            _ if kind == Kind::Drums => "Drums".to_owned(),
            _ => program_name(program).to_owned(),
        };
        if part.program_changed {
            self.log.lost(
                "program changes after a track's first note are left out",
                format!("track {}", index + 1),
            );
        }
        let notes = self.quantise(part, bars, kind == Kind::Drums, index);
        let mut keys: Vec<u8> = notes.iter().map(|n| n.key).collect();
        keys.sort_unstable();
        let lowest = keys.first().copied();
        let middle = keys.get(keys.len() / 2).copied();
        let clef = kind.clef(middle);
        let mut staff = Staff {
            tuning: kind.tuning(lowest),
            show_standard: true,
            show_tablature: kind.shows_tablature(),
            ..Staff::default()
        };
        staff.bars = slices(&notes, bars, kind == Kind::Drums, &self.cell_kinds, clef);
        let lyrics = lyric_line(&staff, bars, lyrics);
        Track {
            name,
            short_name: String::new(),
            color: Color { r: 255, g: 0, b: 0 },
            instrument: Instrument {
                midi_program: program,
                percussion: kind == Kind::Drums,
            },
            mix: Mix {
                volume: part
                    .volume
                    .map_or(0.8, |v| f32::from(v) / 127.0)
                    .clamp(0.0, 1.0),
                pan: part
                    .pan
                    .map_or(0.0, |p| (f32::from(p) - 64.0) / 63.0)
                    .clamp(-1.0, 1.0),
                ..Mix::default()
            },
            staves: vec![staff],
            lyrics: lyrics.into_iter().collect(),
            ..Track::default()
        }
    }
}

/// The time signature that a bar of `len` units can show: its own
/// denominator if it fits, else the smallest that does. `None` when none
/// does (a numerator past 255).
fn signature_of(len: i64, like: TimeSignature) -> Option<TimeSignature> {
    let r = Ratio::new(len, UNITS)?;
    let mut den = i64::from(like.denominator.max(1));
    while r.denom() > den && den < 256 {
        den *= 2;
    }
    let num = r.numer().checked_mul(den)? / r.denom();
    Some(TimeSignature {
        numerator: u8::try_from(num).ok().filter(|&n| n > 0)?,
        denominator: u8::try_from(den).ok()?,
    })
}

/// `at` units to the nearest multiple of `step`.
fn snap_to(at: f64, step: i64) -> i64 {
    let step = step.max(1) as f64;
    ((at / step).round() * step).clamp(0.0, 1e15) as i64
}

/// The bar whose start is nearest `at`.
fn nearest_bar(bars: &[BarSpan], at: f64) -> usize {
    let b = bars
        .partition_point(|b| (b.end as f64) <= at)
        .min(bars.len().saturating_sub(1));
    match bars.get(b) {
        Some(bar) if at - bar.start as f64 > (bar.end - bar.start) as f64 / 2.0 => {
            (b + 1).min(bars.len() - 1)
        }
        _ => b,
    }
}

/// Whether a cell's chosen grid is a triplet one (in simple time): where
/// it meets a straight cell, the notes are cut so each is written in its
/// own grid.
type CellKinds = BTreeMap<(usize, i64), bool>;

impl Builder {
    /// The part's notes on their cells' grids. Fills `self.cell_kinds`
    /// for the part.
    fn quantise(
        &mut self,
        part: &Part,
        bars: &[BarSpan],
        drums: bool,
        index: usize,
    ) -> Vec<GridNote> {
        let total = bars.last().map_or(0, |b| b.end);
        // The points each cell must hold: starts weigh more than ends,
        // which players leave loosely.
        let mut points: BTreeMap<(usize, i64), Vec<(f64, f64)>> = BTreeMap::new();
        for n in &part.notes {
            let mut add = |at: f64, weight: f64| {
                if let Some((cell, offset)) = cell_of(bars, at) {
                    points.entry(cell).or_default().push((offset, weight));
                }
            };
            add(n.start, 1.0);
            if !drums {
                add(n.end, 0.5);
            }
        }
        let mut grids: BTreeMap<(usize, i64), i64> = BTreeMap::new();
        self.cell_kinds.clear();
        for (&(bar, cell_start), pts) in &points {
            let span = bars[bar];
            let cell = span.cell();
            let compound = cell != UNITS / 4;
            let choices: &[Grid] = if compound { &COMPOUND } else { &SIMPLE };
            let len = cell.min(span.end - (span.start + cell_start));
            let best = choices
                .iter()
                .filter(|g| cell % g.parts == 0)
                .map(|g| {
                    let step = cell / g.parts;
                    let cost: f64 = pts
                        .iter()
                        .map(|&(x, w)| w * (distance(x, step, len) / cell as f64 + g.price))
                        .sum();
                    (cost, g.parts, step)
                })
                .min_by(|a, b| a.0.total_cmp(&b.0))
                .map_or(cell, |c| c.2);
            grids.insert((bar, cell_start), best);
            let triplet = !compound && (cell / best) % 3 == 0;
            self.cell_kinds.insert((bar, cell_start), triplet);
        }
        let snap = |at: f64| -> Option<(i64, i64)> {
            let ((bar, cell_start), offset) = cell_of(bars, at)?;
            let span = bars[bar];
            let step = grids
                .get(&(bar, cell_start))
                .copied()
                .unwrap_or(span.cell());
            let len = span.cell().min(span.end - (span.start + cell_start));
            let k = (offset / step as f64)
                .round()
                .clamp(0.0, (len / step) as f64) as i64;
            let on_grid = k * step;
            let snapped = if (offset - len as f64).abs() < (offset - on_grid as f64).abs() {
                len
            } else {
                on_grid
            };
            Some((span.start + cell_start + snapped, step))
        };
        let mut out: Vec<GridNote> = Vec::new();
        let mut past_end = false;
        for n in &part.notes {
            let Some((start, step)) = snap(n.start) else {
                past_end = true;
                continue;
            };
            if start >= total {
                past_end = true;
                continue;
            }
            let end = if drums {
                start + step
            } else {
                match snap(n.end) {
                    Some((e, _)) if e > start => e,
                    Some(_) => start + step,
                    None => total,
                }
            };
            out.push(GridNote {
                key: n.key,
                start,
                end: end.min(total).max(start + 1),
                velocity: n.velocity,
            });
        }
        if past_end {
            self.log.lost(
                "notes past the last bar read are left out",
                format!("track {}", index + 1),
            );
        }
        // One note at a time on each key: a note struck again ends the
        // one before it; two struck at once are one.
        out.sort_by_key(|n| (n.key, n.start, std::cmp::Reverse(n.end)));
        let mut kept: Vec<GridNote> = Vec::with_capacity(out.len());
        for n in out {
            if let Some(prev) = kept.last_mut()
                && prev.key == n.key
                && n.start < prev.end
            {
                if n.start == prev.start {
                    prev.velocity = prev.velocity.max(n.velocity);
                    continue;
                }
                prev.end = n.start;
            }
            kept.push(n);
        }
        kept.sort_by_key(|n| (n.start, n.key));
        kept
    }
}

/// The cell a point falls in, as (bar, cell start from the bar's start),
/// and where in the cell, in units. `None` past the last bar.
fn cell_of(bars: &[BarSpan], at: f64) -> Option<((usize, i64), f64)> {
    let at = at.max(0.0);
    let b = bars.partition_point(|b| (b.end as f64) <= at);
    let span = bars.get(b)?;
    let into = at - span.start as f64;
    let cell = span.cell();
    let cell_start = ((into / cell as f64).floor() as i64).max(0) * cell;
    Some(((b, cell_start), into - cell_start as f64))
}

/// How far `x` is from the nearest point of a grid of `step` in a cell of
/// `len` (whose end is a point too).
fn distance(x: f64, step: i64, len: i64) -> f64 {
    let step = step as f64;
    let k = (x / step).round().clamp(0.0, (len as f64 / step).floor());
    (x - k * step).abs().min((x - len as f64).abs())
}

/// A velocity as the nearest dynamic of OxiGuitar's (15 for ppp to 127
/// for fff, in steps of 16, as Guitar Pro plays them).
fn dynamic(velocity: u8) -> Dynamic {
    const ALL: [Dynamic; 8] = [
        Dynamic::PPP,
        Dynamic::PP,
        Dynamic::P,
        Dynamic::MP,
        Dynamic::MF,
        Dynamic::F,
        Dynamic::FF,
        Dynamic::FFF,
    ];
    let step = ((f64::from(velocity) - 15.0) / 16.0)
        .round()
        .clamp(0.0, 7.0) as usize;
    ALL[step]
}

/// A bar holding one rest of its length.
fn rest_bar(span: &BarSpan, kind: Kind) -> Bar {
    let mut voice = Voice::default();
    push_rest(&mut voice, span.end - span.start);
    Bar {
        clef: kind.clef(None),
        voices: vec![voice],
        simile: None,
    }
}

/// Rests lasting `len` units.
fn push_rest(voice: &mut Voice, len: i64) {
    let Some(r) = Ratio::new(len, UNITS) else {
        return;
    };
    for d in time::fill(r) {
        voice.beats.push(Beat {
            kind: BeatKind::Rest,
            duration: d,
            ..Beat::default()
        });
    }
}

/// The part's notes cut into one voice of beats, bar by bar: at every
/// start and end (only starts, for drums, which ring as they ring), at
/// every bar line and where a triplet cell meets a straight one.
fn slices(
    notes: &[GridNote],
    bars: &[BarSpan],
    drums: bool,
    kinds: &CellKinds,
    clef: oxiguitar_model::Clef,
) -> Vec<Bar> {
    let mut cuts: BTreeSet<i64> = BTreeSet::new();
    for b in bars {
        cuts.insert(b.start);
        cuts.insert(b.end);
    }
    for n in notes {
        cuts.insert(n.start);
        if !drums {
            cuts.insert(n.end);
        }
    }
    // A triplet cell's notes are cut at its edges, so a note held into or
    // out of it is written in each cell's own grid.
    for (&(bar, cell), &triplet) in kinds {
        if triplet {
            let span = bars[bar];
            let start = span.start + cell;
            cuts.insert(start);
            cuts.insert((start + span.cell()).min(span.end));
        }
    }
    let mut out = Vec::with_capacity(bars.len());
    let mut next = 0usize;
    let mut sounding: Vec<GridNote> = Vec::new();
    for span in bars {
        let mut voice = Voice::default();
        let mut rest = 0i64;
        let points: Vec<i64> = cuts.range(span.start..=span.end).copied().collect();
        for w in points.windows(2) {
            let (a, b) = (w[0], w[1]);
            if drums {
                sounding.clear();
            } else {
                sounding.retain(|n| n.end > a);
            }
            while next < notes.len() && notes[next].start <= a {
                if notes[next].start == a || (!drums && notes[next].end > a) {
                    sounding.push(notes[next]);
                }
                next += 1;
            }
            if sounding.is_empty() {
                rest += b - a;
                continue;
            }
            if rest > 0 {
                push_rest(&mut voice, rest);
                rest = 0;
            }
            let Some(len) = Ratio::new(b - a, UNITS) else {
                continue;
            };
            let mut ordered = sounding.clone();
            ordered.sort_by_key(|n| n.key);
            for (i, d) in time::fill(len).into_iter().enumerate() {
                voice.beats.push(Beat {
                    kind: BeatKind::Notes,
                    duration: d,
                    notes: ordered
                        .iter()
                        .map(|n| Note {
                            place: NotePlace::Pitched(Pitch(n.key)),
                            tie: i > 0 || n.start < a,
                            dynamic: dynamic(n.velocity),
                            effects: NoteEffects::default(),
                            spelling: None,
                        })
                        .collect(),
                    ..Beat::default()
                });
            }
        }
        if rest > 0 {
            push_rest(&mut voice, rest);
        }
        out.push(Bar {
            clef,
            voices: vec![voice],
            simile: None,
        });
    }
    out
}

/// A track's lyrics as a line of OxiGuitar's: each syllable on the beat
/// sung when the file sings it, beats sung without one left empty.
fn lyric_line(staff: &Staff, bars: &[BarSpan], lyrics: &[(f64, String)]) -> Option<LyricLine> {
    if lyrics.iter().all(|l| l.1.trim().is_empty()) {
        return None;
    }
    // Where each beat the first voice sings starts, in units.
    let mut sung: Vec<i64> = Vec::new();
    for (bar, span) in staff.bars.iter().zip(bars) {
        let mut at = Ratio::ZERO;
        for beat in bar.voices.first().map_or(&[][..], |v| &v.beats) {
            if oxiguitar_model::lyrics::is_sung(beat) {
                let units = at
                    .checked_mul(Ratio::from_integer(UNITS))
                    .map_or(0, |u| u.numer() / u.denom().max(1));
                sung.push(span.start + units);
            }
            at = beat
                .duration
                .length()
                .and_then(|l| at.checked_add(l))
                .unwrap_or(at);
        }
    }
    if sung.is_empty() {
        return None;
    }
    let mut on: Vec<Vec<&str>> = vec![Vec::new(); sung.len()];
    for (at, text) in lyrics {
        // The sung beat it falls in (the first, before it); a syllable a
        // 32nd early is on its beat.
        let at = at.round() as i64 + UNITS / 32;
        let i = sung.partition_point(|&s| s <= at).saturating_sub(1);
        let t = text.as_str();
        if !t.trim().is_empty() {
            on[i].push(t);
        }
    }
    let mut text = String::new();
    for syllables in &on {
        let joined: String = syllables.concat();
        let hyphen = joined.trim_end().ends_with('-');
        let word: String = joined
            .trim()
            .trim_end_matches('-')
            .chars()
            .map(|c| match c {
                ' ' | '\t' | '\n' => '+',
                '-' => '\u{2010}',
                '[' => '(',
                ']' => ')',
                c => c,
            })
            .collect();
        text.push_str(&word);
        text.push(if hyphen && !word.is_empty() { '-' } else { ' ' });
    }
    let text = text.trim_end().to_owned();
    (!text.is_empty()).then_some(LyricLine { start_bar: 0, text })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_frame_rate_midly_cant_negate_is_refused() {
        let mut file = b"MThd\0\0\0\x06\0\0\0\x01\x80\x28".to_vec();
        assert_eq!(division(&file), Some(0x8028));
        assert!(read(&file).is_err());
        file[12] = 0xe7; // 25 frames a second
        assert!(read(&file).is_ok());
        let mut riff = b"RIFF\x22\0\0\0RMIDdata\x0e\0\0\0".to_vec();
        riff.extend_from_slice(&file);
        riff[12 + 8 + 12] = 0x80;
        assert_eq!(division(&riff), Some(0x8028));
    }

    #[test]
    fn grid_distances() {
        assert_eq!(distance(13.0, 12, 48), 1.0);
        assert_eq!(distance(47.0, 16, 48), 1.0);
        assert_eq!(distance(40.0, 48, 48), 8.0);
    }

    #[test]
    fn signatures_of_short_bars() {
        let t = signature_of(96, TimeSignature::COMMON).unwrap();
        assert_eq!((t.numerator, t.denominator), (2, 4));
        let t = signature_of(72, TimeSignature::COMMON).unwrap();
        assert_eq!((t.numerator, t.denominator), (3, 8));
    }

    #[test]
    fn velocities_as_dynamics() {
        assert_eq!(dynamic(127), Dynamic::FFF);
        assert_eq!(dynamic(95), Dynamic::F);
        assert_eq!(dynamic(80), Dynamic::MF);
        assert_eq!(dynamic(1), Dynamic::PPP);
    }
}
