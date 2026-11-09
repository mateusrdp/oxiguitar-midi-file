// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Writing a score as a Standard MIDI File (docs/design.md, "Writing").
//!
//! A MIDI file holds what plays, so it is written from OxiGuitar's player
//! timeline (Mateus, 9 Nov 2026): repeats, endings and jumps played out,
//! every note at the time and velocity OxiGuitar plays it (swing, strums,
//! ghost notes, accents, hairpins, let ring, tremolo picking, trills,
//! grace notes), bends, slides and the whammy bar as pitch bend, and the
//! effects as OxiGuitar's MIDI sound plugin (oxiguitar-midi) plays them:
//! vibrato as a pitch curve of 30 cents at 5.5 Hz (60 at 4.5 Hz for the
//! whammy bar's), palm mutes ending after 0.15 s and dead notes after
//! 30 ms, softer, hammer-ons and pull-offs softer, harmonics on clean
//! guitars and slaps and pops on basses with General MIDI's programs for
//! them, fades as expression.
//!
//! The file is format 1: a first track with the tempo, time and key
//! signatures, markers and the header texts, then one track for each of
//! the score's tracks (and one more for a track whose chord diagrams
//! play). Bars are counted as played; the tempo map makes every tick fall
//! where OxiGuitar plays it.
//!
//! Channels: a fretted track gives each string a channel of its own, as
//! the sound plugin does, so a bend moves only its string, when the 15
//! channels besides the drums' are enough for that; otherwise each track
//! with bends or vibrato gets two (its notes that bend go on the second,
//! as Guitar Pro does), or failing that one, shared when there are more
//! than 15 tracks.

use std::collections::BTreeMap;

use midly::num::{u4, u7, u15, u24, u28};
use midly::{
    Format, Header, MetaMessage, MidiMessage, PitchBend, Smf, Timing, TrackEvent, TrackEventKind,
};
use oxiguitar_model::{Ratio, Score, TimeSignature};
use oxiguitar_play::events::{Attack, HarmonicKind, Sound, Vibrato};
use oxiguitar_play::{NoteEvent, PitchPoint, Timeline};

use crate::log::Log;

/// Ticks a quarter note.
pub(crate) const PER_QUARTER: u16 = 960;

/// The pitch bend range set on every pitched channel, in semitones each
/// way: the sound plugin's.
const BEND_RANGE: f64 = 24.0;

/// Vibrato: depth in cents and rate in hertz, and the time it takes to
/// reach full depth (the sound plugin's).
const VIBRATO: (f64, f64) = (30.0, 5.5);
const WIDE_VIBRATO: (f64, f64) = (60.0, 4.5);
const VIBRATO_RISE: f64 = 0.1;

/// How long palm mutes and dead notes sound, in seconds, and their
/// velocity against the written one; and a note not picked's.
const PALM_MUTE: (f64, f64) = (0.15, 0.9);
const DEAD: (f64, f64) = (0.03, 0.6);
const UNPICKED: f64 = 0.7;

/// A fade's quietest point: −60 dB.
const FADE_FLOOR: f64 = 0.001;

/// How often a pitch curve or a fade is sent, in seconds.
const CURVE_STEP: f64 = 0.01;

/// General MIDI programs the effects play.
const GUITAR_HARMONICS: u8 = 31;
const SLAP_BASS_1: u8 = 36;
const SLAP_BASS_2: u8 = 37;

/// The drums' channel (channel 10, counted from 0).
const DRUMS: u8 = 9;

/// The channels left for pitched tracks.
const CHANNELS: [u8; 15] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 10, 11, 12, 13, 14, 15];

/// An event to write, owning its texts.
#[derive(Clone, Debug)]
enum Ev {
    Name(String),
    Copyright(String),
    Text(String),
    Marker(String),
    Lyric(String),
    Tempo(u32),
    Time(u8, u8),
    Key(i8, bool),
    On(u8, u8, u8),
    Off(u8, u8),
    Program(u8, u8),
    Control(u8, u8, u8),
    Bend(u8, i16),
}

/// The order of events at one tick: what ends, then what is set up, then
/// what starts.
mod order {
    pub(super) const META: u8 = 0;
    pub(super) const OFF: u8 = 1;
    pub(super) const RESET: u8 = 2;
    pub(super) const PROGRAM: u8 = 3;
    pub(super) const BEFORE: u8 = 4;
    pub(super) const ON: u8 = 5;
    pub(super) const DURING: u8 = 6;
}

/// One track of the file being written.
#[derive(Debug, Default)]
struct FileTrack {
    events: Vec<(u64, u8, Ev)>,
}

impl FileTrack {
    fn push(&mut self, tick: u64, order: u8, ev: Ev) {
        self.events.push((tick, order, ev));
    }
}

/// Where a time in seconds falls in ticks: the tempo map, piece by piece.
#[derive(Debug)]
struct TickMap {
    /// (seconds, tick, seconds a tick) where each piece starts.
    pieces: Vec<(f64, f64, f64)>,
}

impl TickMap {
    fn tick(&self, seconds: f64) -> u64 {
        let i = self
            .pieces
            .partition_point(|p| p.0 <= seconds)
            .saturating_sub(1);
        let Some(&(s0, t0, per)) = self.pieces.get(i) else {
            return 0;
        };
        let t = t0 + (seconds - s0) / per;
        if t.is_finite() {
            t.round().max(0.0) as u64
        } else {
            0
        }
    }
}

/// Writes `score` as a MIDI file, with what the file can't hold.
pub(crate) fn write(score: &Score) -> Result<(Vec<u8>, Log), String> {
    let mut log = Log::default();
    losses(score, &mut log);
    let timeline = Timeline::new(score);
    let mut conductor = FileTrack::default();
    let (map, visit_ticks) = tempo_map(&timeline, &mut conductor);
    header(score, &mut conductor);
    signatures(score, &timeline, &visit_ticks, &mut conductor, &mut log);
    let plan = channels(score, &timeline, &mut log);
    let mut tracks = vec![conductor];
    for (t, track) in score.tracks.iter().enumerate() {
        let mut ft = FileTrack::default();
        ft.push(0, order::META, Ev::Name(track.name.clone()));
        let notes: Vec<&NoteEvent> = timeline.notes.iter().filter(|n| n.track == t).collect();
        play(score, t, &plan.tracks[t], &notes, &map, &mut ft);
        lyrics(score, t, &timeline, &map, &mut ft, &mut log);
        tracks.push(ft);
        let chords: Vec<&NoteEvent> = timeline.chords.iter().filter(|n| n.track == t).collect();
        if !chords.is_empty() {
            let mut ft = FileTrack::default();
            ft.push(0, order::META, Ev::Name(format!("{} (chords)", track.name)));
            let lane = plan.chords.get(&t).unwrap_or(&plan.tracks[t]);
            play(score, t, lane, &chords, &map, &mut ft);
            tracks.push(ft);
        }
    }
    Ok((encode(tracks)?, log))
}

/// What the file can't hold, named before it is written.
fn losses(score: &Score, log: &mut Log) {
    let jumps = score.master_bars.iter().any(|m| {
        m.repeat_start
            || m.repeat_end.is_some()
            || m.alternate_endings != 0
            || !m.directions.is_empty()
    });
    if jumps {
        log.note(
            "repeats, endings and jumps are written out as they play",
            "the whole score",
        );
    }
    let fretted = score.tracks.iter().any(|t| {
        t.staves.iter().any(|s| {
            s.bars.iter().any(|b| {
                b.voices.iter().any(|v| {
                    v.beats
                        .iter()
                        .any(|bt| bt.notes.iter().any(|n| n.string().is_some()))
                })
            })
        })
    });
    if fretted {
        log.lost(
            "strings and frets: a MIDI file holds pitches only (OxiGuitar fingers them again when it opens the file)",
            "the whole score",
        );
    }
    log.lost(
        "the notation (note values as written, marks, texts, chord diagrams, spellings): a MIDI file holds what plays",
        "the whole score",
    );
}

/// The tempo map: each played bar sampled every 192nd of a whole note,
/// a tempo event wherever the tempo changes. Returns the map and the tick
/// each played bar starts at.
fn tempo_map(timeline: &Timeline, conductor: &mut FileTrack) -> (TickMap, Vec<u64>) {
    let per_whole = 4.0 * f64::from(PER_QUARTER);
    let mut pieces: Vec<(f64, f64, f64)> = Vec::new();
    let mut starts = Vec::with_capacity(timeline.map.visits.len());
    let mut tick = 0u64;
    let mut current: Option<u32> = None;
    for (k, v) in timeline.map.visits.iter().enumerate() {
        starts.push(tick);
        let len = timeline.map.bar_length(v.visit.bar).to_f64();
        let ticks = (len * per_whole).round().max(1.0) as u64;
        let samples = ((len * 192.0).ceil() as usize).max(1);
        let mut s0 = timeline.map.time_f64(k, 0.0);
        let mut t0 = tick as f64;
        for j in 1..=samples {
            let p = (j as f64 / 192.0).min(len);
            let t1 = if j == samples {
                (tick + ticks) as f64
            } else {
                (tick as f64 + p * per_whole).round()
            };
            if t1 <= t0 {
                continue;
            }
            let s1 = timeline.map.time_f64(k, p);
            let per = ((s1 - s0) / (t1 - t0)).max(1e-9);
            let micros = (per * f64::from(PER_QUARTER) * 1e6)
                .round()
                .clamp(1.0, 16_777_215.0) as u32;
            if current != Some(micros) {
                current = Some(micros);
                conductor.push(t0 as u64, order::META, Ev::Tempo(micros));
            }
            // Ticks follow the tempo written, so they never drift.
            let per = f64::from(micros) / 1e6 / f64::from(PER_QUARTER);
            if pieces.last().map(|p| p.2) != Some(per) {
                pieces.push((s0, t0, per));
            }
            s0 = s1;
            t0 = t1;
        }
        tick += ticks;
    }
    if pieces.is_empty() {
        pieces.push((0.0, 0.0, 0.5 / f64::from(PER_QUARTER)));
        conductor.push(0, order::META, Ev::Tempo(500_000));
    }
    // Keep the pieces continuous in seconds: each starts where the one
    // before ends at its own tempo.
    for i in 1..pieces.len() {
        let (s, t, p) = pieces[i - 1];
        pieces[i].0 = s + (pieces[i].1 - t) * p;
    }
    (TickMap { pieces }, starts)
}

/// The header texts on the first track, as the reader reads them back.
fn header(score: &Score, conductor: &mut FileTrack) {
    let info = &score.info;
    conductor.push(0, order::META, Ev::Name(info.title.clone()));
    if !info.copyright.is_empty() {
        conductor.push(0, order::META, Ev::Copyright(info.copyright.clone()));
    }
    for (label, value) in [
        ("Subtitle", &info.subtitle),
        ("Artist", &info.artist),
        ("Album", &info.album),
        ("Words", &info.words),
        ("Music", &info.music),
        ("Transcriber", &info.transcriber),
        ("Instructions", &info.instructions),
    ] {
        if !value.is_empty() {
            conductor.push(0, order::META, Ev::Text(format!("{label}: {value}")));
        }
    }
    for n in &info.notices {
        if !n.is_empty() {
            conductor.push(0, order::META, Ev::Text(n.clone()));
        }
    }
}

/// Time and key signatures and markers, at the played bars where they
/// change. A bar played longer or shorter than its time signature gets
/// one of its own length, so bars read back as they play.
fn signatures(
    score: &Score,
    timeline: &Timeline,
    starts: &[u64],
    conductor: &mut FileTrack,
    log: &mut Log,
) {
    let mut time: Option<(u8, u8)> = None;
    let mut key: Option<(i8, bool)> = None;
    for (k, v) in timeline.map.visits.iter().enumerate() {
        let Some(mb) = score.master_bars.get(v.visit.bar) else {
            continue;
        };
        let tick = starts[k];
        let len = timeline.map.bar_length(v.visit.bar);
        let written = if mb.time_signature.bar_length() == Some(len) {
            Some(mb.time_signature)
        } else {
            signature_of(len, mb.time_signature)
        };
        let ts = written.unwrap_or_else(|| {
            log.lost(
                "bars of a length no time signature can show keep their own",
                crate::log::place(v.visit.bar, 0),
            );
            mb.time_signature
        });
        let pair = (ts.numerator, ts.denominator.max(1).ilog2() as u8);
        if time != Some(pair) {
            time = Some(pair);
            conductor.push(tick, order::META, Ev::Time(pair.0, pair.1));
        }
        let pair = (
            mb.key_signature.fifths.clamp(-7, 7),
            mb.key_signature.mode == oxiguitar_model::KeyMode::Minor,
        );
        if key != Some(pair) {
            key = Some(pair);
            conductor.push(tick, order::META, Ev::Key(pair.0, pair.1));
        }
        if let Some(m) = &mb.marker {
            let text = format!("{} {}", m.letter, m.text).trim().to_owned();
            if !text.is_empty() {
                conductor.push(tick, order::META, Ev::Marker(text));
            }
        }
    }
}

/// A time signature for a bar of length `len` (whole notes): `like`'s
/// denominator if it fits, else the smallest power of two that does.
fn signature_of(len: Ratio, like: TimeSignature) -> Option<TimeSignature> {
    let mut den = i64::from(like.denominator.max(1));
    if den & (den - 1) != 0 || len.denom() & (len.denom() - 1) != 0 {
        return None;
    }
    while len.denom() > den && den < 128 {
        den *= 2;
    }
    let num = len.numer().checked_mul(den)? / len.denom();
    Some(TimeSignature {
        numerator: u8::try_from(num).ok().filter(|&n| n > 0)?,
        denominator: u8::try_from(den).ok()?,
    })
}

/// The channels of one track of the file.
#[derive(Clone, Debug)]
enum Lane {
    /// Drums, on channel 10.
    Drums,
    /// Each string its channel (string 1 first).
    Strings(Vec<u8>),
    /// Notes on the first; those that bend on the second, if given.
    Track(u8, Option<u8>),
}

/// The channels of every track.
#[derive(Debug)]
struct Plan {
    tracks: Vec<Lane>,
    /// For tracks whose chord diagrams play.
    chords: BTreeMap<usize, Lane>,
}

/// Whether a note's pitch moves.
fn bends(n: &NoteEvent) -> bool {
    !n.pitch.is_empty() || n.vibrato != Vibrato::None
}

/// Picks the channels (see the module's documentation).
fn channels(score: &Score, timeline: &Timeline, log: &mut Log) -> Plan {
    let tracks = score.tracks.len();
    let mut strings = vec![0usize; tracks];
    let mut bending = vec![false; tracks];
    let mut chords = vec![false; tracks];
    for n in &timeline.notes {
        if let Some(s) = n.string {
            strings[n.track] = strings[n.track].max(usize::from(s));
        }
        bending[n.track] |= bends(n);
    }
    for n in &timeline.chords {
        chords[n.track] = true;
    }
    let drums: Vec<bool> = score
        .tracks
        .iter()
        .map(|t| t.instrument.percussion)
        .collect();
    let pitched = (0..tracks).filter(|&t| !drums[t]);
    let chord_count = (0..tracks).filter(|&t| chords[t] && !drums[t]).count();
    let per_string: usize = pitched.clone().map(|t| strings[t].max(1)).sum::<usize>() + chord_count;
    let doubled: usize = pitched
        .clone()
        .map(|t| 1 + usize::from(bending[t]))
        .sum::<usize>()
        + chord_count;
    let mut free = CHANNELS.iter().copied().cycle();
    let mut take = || free.next().unwrap_or(0);
    let mut plan = Plan {
        tracks: Vec::with_capacity(tracks),
        chords: BTreeMap::new(),
    };
    let fits_strings = per_string <= CHANNELS.len();
    let fits_doubled = doubled <= CHANNELS.len();
    if !fits_doubled && pitched.clone().count() + chord_count > CHANNELS.len() {
        log.lost(
            "more than 15 instrument tracks: some share a MIDI channel (and its sound)",
            "the whole score",
        );
    }
    for t in 0..tracks {
        let lane = if drums[t] {
            Lane::Drums
        } else if fits_strings && strings[t] > 0 {
            Lane::Strings((0..strings[t]).map(|_| take()).collect())
        } else if fits_doubled && bending[t] {
            Lane::Track(take(), Some(take()))
        } else {
            Lane::Track(take(), None)
        };
        plan.tracks.push(lane);
        if chords[t] {
            let lane = if drums[t] {
                Lane::Drums
            } else {
                Lane::Track(take(), None)
            };
            plan.chords.insert(t, lane);
        }
    }
    if !fits_strings && !fits_doubled && bending.iter().any(|&b| b) {
        log.lost(
            "bends and vibrato also move the other notes sounding on their track (not enough MIDI channels for each its own)",
            "the whole score",
        );
    }
    plan
}

/// The channel a note plays on.
fn channel_of(lane: &Lane, n: &NoteEvent) -> u8 {
    match lane {
        Lane::Drums => DRUMS,
        Lane::Strings(c) => n
            .string
            .and_then(|s| c.get(usize::from(s).saturating_sub(1)))
            .or(c.first())
            .copied()
            .unwrap_or(0),
        Lane::Track(first, second) => match second {
            Some(s) if bends(n) => *s,
            _ => *first,
        },
    }
}

/// The program a note plays: the track's, unless its effect has a General
/// MIDI sound of its own (as the sound plugin plays it).
fn program_for(program: u8, n: &NoteEvent) -> u8 {
    let clean_guitar = (24..=28).contains(&program);
    let bass = (32..=35).contains(&program);
    match (n.sound, n.attack) {
        (
            Sound::Harmonic(
                HarmonicKind::Natural | HarmonicKind::Artificial | HarmonicKind::Tapped,
            ),
            _,
        ) if clean_guitar => GUITAR_HARMONICS,
        (_, Attack::Slapped) if bass => SLAP_BASS_1,
        (_, Attack::Popped) if bass => SLAP_BASS_2,
        _ => program,
    }
}

/// Cents from the note's key at `t`, along its pitch curve (straight lines
/// between its points, holding the ends).
fn cents_at(curve: &[PitchPoint], t: f64) -> f64 {
    match curve.iter().position(|p| p.time > t) {
        None => curve.last().map_or(0.0, |p| f64::from(p.cents)),
        Some(0) => f64::from(curve[0].cents),
        Some(i) => {
            let (a, b) = (curve[i - 1], curve[i]);
            let span = b.time - a.time;
            if span <= 0.0 {
                return f64::from(b.cents);
            }
            let f = (t - a.time) / span;
            f64::from(a.cents) + f * f64::from(b.cents - a.cents)
        }
    }
}

/// A pitch bend value for `cents` in the bend range.
fn bend_value(cents: f64) -> i16 {
    (cents / (BEND_RANGE * 100.0) * 8192.0)
        .round()
        .clamp(-8192.0, 8191.0) as i16
}

/// The expression (0 to 127) of a fading note at `t`.
fn expression(n: &NoteEvent, t: f64) -> u8 {
    let len = (n.end - n.start).max(1e-6);
    let p = ((t - n.start) / len).clamp(0.0, 1.0);
    let mut level = 1.0;
    if n.fade_in {
        level *= FADE_FLOOR.powf(1.0 - p);
    }
    if n.fade_out && p > 0.5 {
        level *= FADE_FLOOR.powf((p - 0.5) * 2.0);
    }
    (level * 127.0).round().clamp(0.0, 127.0) as u8
}

/// A track's notes as channel events: its channels set up at the start,
/// then every note with its program, pitch curve and expression.
fn play(
    score: &Score,
    t: usize,
    lane: &Lane,
    notes: &[&NoteEvent],
    map: &TickMap,
    ft: &mut FileTrack,
) {
    let Some(track) = score.tracks.get(t) else {
        return;
    };
    let program = track.instrument.midi_program.min(127);
    let used: Vec<u8> = match lane {
        Lane::Drums => vec![DRUMS],
        Lane::Strings(c) => c.clone(),
        Lane::Track(a, b) => std::iter::once(*a).chain(*b).collect(),
    };
    let volume = (track.mix.volume.clamp(0.0, 1.0) * 127.0).round() as u8;
    let pan = (64.0 + track.mix.pan.clamp(-1.0, 1.0) * 63.0).round() as u8;
    let mut programs: BTreeMap<u8, u8> = BTreeMap::new();
    for &c in &used {
        if c != DRUMS {
            // The pitch bend range: RPN 0, then the null RPN.
            for (cc, v) in [
                (101, 0),
                (100, 0),
                (6, BEND_RANGE as u8),
                (38, 0),
                (101, 127),
                (100, 127),
            ] {
                ft.push(0, order::RESET, Ev::Control(c, cc, v));
            }
            ft.push(0, order::PROGRAM, Ev::Program(c, program));
            programs.insert(c, program);
        }
        ft.push(0, order::RESET, Ev::Control(c, 7, volume));
        ft.push(0, order::RESET, Ev::Control(c, 10, pan));
    }
    // A new note on a string ends the one before it, as on the instrument.
    let mut notes: Vec<NoteEvent> = notes.iter().map(|&n| n.clone()).collect();
    notes.sort_by(|a, b| a.start.total_cmp(&b.start));
    let mut last_on: BTreeMap<(usize, u8), usize> = BTreeMap::new();
    for i in 0..notes.len() {
        if let Some(s) = notes[i].string {
            let start = notes[i].start;
            if let Some(&j) = last_on.get(&(notes[i].staff, s))
                && notes[j].end > start
            {
                notes[j].end = start;
            }
            last_on.insert((notes[i].staff, s), i);
        }
    }
    for (i, n) in notes.iter().enumerate() {
        let c = channel_of(lane, n);
        let mut end = n.end;
        let mut velocity = f64::from(n.velocity);
        match n.sound {
            Sound::PalmMute => {
                end = end.min(n.start + PALM_MUTE.0);
                velocity *= PALM_MUTE.1;
            }
            Sound::Dead => {
                end = end.min(n.start + DEAD.0);
                velocity *= DEAD.1;
            }
            _ => {}
        }
        if n.attack == Attack::Legato {
            velocity *= UNPICKED;
        }
        let velocity = velocity.round().clamp(1.0, 127.0) as u8;
        let on = map.tick(n.start);
        let off = map.tick(end).max(on + 1);
        if c != DRUMS {
            let wanted = program_for(program, n);
            if programs.get(&c) != Some(&wanted) {
                programs.insert(c, wanted);
                ft.push(on, order::PROGRAM, Ev::Program(c, wanted));
            }
        }
        let curve = bends(n);
        if curve && c != DRUMS {
            let vibrato = match n.vibrato {
                Vibrato::None => None,
                Vibrato::Normal => Some(VIBRATO),
                Vibrato::Wide => Some(WIDE_VIBRATO),
            };
            let mut last: Option<i16> = None;
            let mut s = n.start;
            while s < end {
                let mut cents = cents_at(&n.pitch, s);
                if let Some((depth, hz)) = vibrato {
                    let since = s - n.start;
                    let rise = (since / VIBRATO_RISE).min(1.0);
                    cents += depth * rise * (std::f64::consts::TAU * hz * since).sin();
                }
                let value = bend_value(cents);
                if last != Some(value) {
                    last = Some(value);
                    let (tick, ord) = if s == n.start {
                        (on, order::BEFORE)
                    } else {
                        (map.tick(s).max(on), order::DURING)
                    };
                    ft.push(tick, ord, Ev::Bend(c, value));
                }
                s += CURVE_STEP;
            }
            // Back to its key, unless another moving note on the channel
            // still sounds.
            let other = notes.iter().enumerate().any(|(j, m)| {
                j != i && bends(m) && channel_of(lane, m) == c && m.start < end && m.end > end
            });
            if !other {
                ft.push(off, order::RESET, Ev::Bend(c, 0));
            }
        }
        if n.fade_in || n.fade_out {
            let mut last: Option<u8> = None;
            let mut s = n.start;
            while s < end {
                let value = expression(n, s);
                if last != Some(value) {
                    last = Some(value);
                    let (tick, ord) = if s == n.start {
                        (on, order::BEFORE)
                    } else {
                        (map.tick(s).max(on), order::DURING)
                    };
                    ft.push(tick, ord, Ev::Control(c, 11, value));
                }
                s += CURVE_STEP * 2.0;
            }
            ft.push(off, order::RESET, Ev::Control(c, 11, 127));
        }
        ft.push(on, order::ON, Ev::On(c, n.key.min(127), velocity));
        ft.push(off, order::OFF, Ev::Off(c, n.key.min(127)));
    }
}

/// The first lyric line of a track, each syllable on the first note of
/// the beat that sings it, every time that bar plays.
fn lyrics(
    score: &Score,
    t: usize,
    timeline: &Timeline,
    map: &TickMap,
    ft: &mut FileTrack,
    log: &mut Log,
) {
    let Some(track) = score.tracks.get(t) else {
        return;
    };
    let lines: Vec<_> = track
        .lyrics
        .iter()
        .filter(|l| !l.text.trim().is_empty())
        .collect();
    let Some(line) = lines.first() else {
        return;
    };
    if lines.len() > 1 {
        log.lost("lyric lines after the first", format!("track {}", t + 1));
    }
    let Some(staff) = track.staves.first() else {
        return;
    };
    let syllables = oxiguitar_model::lyrics::syllables(&line.text);
    let beats = oxiguitar_model::lyrics::sung_beats(staff, line.start_bar);
    // Each sung (bar, beat) and its syllable.
    let mut sung: BTreeMap<(usize, usize), String> = BTreeMap::new();
    for (s, at) in syllables.iter().zip(beats) {
        if s.text.is_empty() {
            continue;
        }
        let mut text = s.text.clone();
        text.push(if s.hyphen { '-' } else { ' ' });
        sung.insert(at, text);
    }
    // When each beat of the first voice is first heard, in each played bar.
    let mut heard: BTreeMap<(usize, usize), f64> = BTreeMap::new();
    for n in timeline
        .notes
        .iter()
        .filter(|n| n.track == t && n.staff == 0 && n.voice == 0 && !n.grace)
    {
        let e = heard.entry((n.visit, n.beat)).or_insert(n.start);
        *e = e.min(n.start);
    }
    for (&(visit, beat), &start) in &heard {
        let Some(bar) = timeline.map.visits.get(visit).map(|v| v.visit.bar) else {
            continue;
        };
        if let Some(text) = sung.get(&(bar, beat)) {
            ft.push(map.tick(start), order::BEFORE, Ev::Lyric(text.clone()));
        }
    }
}

/// The tracks as a Standard MIDI File.
fn encode(tracks: Vec<FileTrack>) -> Result<Vec<u8>, String> {
    let mut sorted: Vec<Vec<(u64, u8, Ev)>> = tracks
        .into_iter()
        .map(|mut t| {
            // Stable: events at one tick and order keep their sequence.
            t.events.sort_by_key(|e| (e.0, e.1));
            t.events
        })
        .collect();
    for t in &mut sorted {
        // Delta times are 28-bit: a gap longer than that can't be written.
        let mut last = 0u64;
        for e in t.iter_mut() {
            if e.0 - last > u64::from(u28::max_value().as_int()) {
                e.0 = last + u64::from(u28::max_value().as_int());
            }
            last = e.0;
        }
    }
    let texts: Vec<Vec<Vec<u8>>> = sorted
        .iter()
        .map(|t| {
            t.iter()
                .map(|(_, _, e)| match e {
                    Ev::Name(s) | Ev::Copyright(s) | Ev::Text(s) | Ev::Marker(s) | Ev::Lyric(s) => {
                        s.as_bytes().to_vec()
                    }
                    _ => Vec::new(),
                })
                .collect()
        })
        .collect();
    let mut smf = Smf::new(Header::new(
        Format::Parallel,
        Timing::Metrical(u15::new(PER_QUARTER)),
    ));
    for (events, texts) in sorted.iter().zip(&texts) {
        let mut out: Vec<TrackEvent<'_>> = Vec::with_capacity(events.len() + 1);
        let mut last = 0u64;
        for ((tick, _, e), bytes) in events.iter().zip(texts) {
            let delta = u28::new(u32::try_from(tick - last).unwrap_or(0));
            last = *tick;
            let midi = |c: u8, message: MidiMessage| TrackEventKind::Midi {
                channel: u4::new(c),
                message,
            };
            let kind = match e {
                Ev::Name(_) => TrackEventKind::Meta(MetaMessage::TrackName(bytes)),
                Ev::Copyright(_) => TrackEventKind::Meta(MetaMessage::Copyright(bytes)),
                Ev::Text(_) => TrackEventKind::Meta(MetaMessage::Text(bytes)),
                Ev::Marker(_) => TrackEventKind::Meta(MetaMessage::Marker(bytes)),
                Ev::Lyric(_) => TrackEventKind::Meta(MetaMessage::Lyric(bytes)),
                Ev::Tempo(m) => TrackEventKind::Meta(MetaMessage::Tempo(u24::new(*m))),
                Ev::Time(n, d) => TrackEventKind::Meta(MetaMessage::TimeSignature(*n, *d, 24, 8)),
                Ev::Key(f, m) => TrackEventKind::Meta(MetaMessage::KeySignature(*f, *m)),
                Ev::On(c, k, v) => midi(
                    *c,
                    MidiMessage::NoteOn {
                        key: u7::new(*k),
                        vel: u7::new((*v).max(1)),
                    },
                ),
                Ev::Off(c, k) => midi(
                    *c,
                    MidiMessage::NoteOff {
                        key: u7::new(*k),
                        vel: u7::new(64),
                    },
                ),
                Ev::Program(c, p) => midi(
                    *c,
                    MidiMessage::ProgramChange {
                        program: u7::new(*p),
                    },
                ),
                Ev::Control(c, cc, v) => midi(
                    *c,
                    MidiMessage::Controller {
                        controller: u7::new(*cc),
                        value: u7::new(*v),
                    },
                ),
                Ev::Bend(c, b) => midi(
                    *c,
                    MidiMessage::PitchBend {
                        bend: PitchBend::from_int(*b),
                    },
                ),
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
    smf.write_std(&mut data)
        .map_err(|e| format!("the MIDI file can't be written: {e}"))?;
    Ok(data)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn curves() {
        let p = |time, cents| PitchPoint { time, cents };
        let curve = [p(1.0, 0), p(2.0, 200)];
        assert_eq!(cents_at(&curve, 0.0), 0.0);
        assert_eq!(cents_at(&curve, 1.5), 100.0);
        assert_eq!(cents_at(&curve, 3.0), 200.0);
        assert_eq!(bend_value(0.0), 0);
        assert_eq!(bend_value(2400.0), 8191);
        assert_eq!(bend_value(-1200.0), -4096);
    }

    #[test]
    fn signatures_of_played_lengths() {
        let r = |n, d| Ratio::new(n, d).unwrap();
        let t = signature_of(r(5, 8), TimeSignature::COMMON).unwrap();
        assert_eq!((t.numerator, t.denominator), (5, 8));
        let t = signature_of(r(3, 4), TimeSignature::COMMON).unwrap();
        assert_eq!((t.numerator, t.denominator), (3, 4));
        assert!(signature_of(r(1, 3), TimeSignature::COMMON).is_none());
    }
}
