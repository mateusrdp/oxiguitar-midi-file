// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! What kind of instrument a part of a MIDI file is, and so which strings it
//! gets (Mateus, 9 Nov 2026, as for MusicXML): General MIDI's guitars
//! (programs 24-31) and basses (32-39) get strings and show their
//! tablature; every other pitched part also gets a guitar's strings
//! (OxiGuitar fingers it as it opens, #157) but shows notation only until
//! the user turns its tablature on; channel 10 is drums, with no strings.

use oxiguitar_model::{Clef, Pitch};

/// What a part is played on, as far as its strings go.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    /// A guitar: tablature shown.
    Guitar,
    /// A bass: tablature shown.
    Bass,
    /// Any other pitched instrument: a guitar's strings, tablature hidden.
    Other,
    /// Drums (MIDI channel 10): no strings.
    Drums,
}

impl Kind {
    /// The kind of a part on `channel` (0-based) playing `program`
    /// (0-based), named `name`.
    pub(crate) fn of(channel: u8, program: u8, name: &str) -> Kind {
        if channel == 9 {
            return Kind::Drums;
        }
        match program {
            24..=31 => Kind::Guitar,
            32..=39 => Kind::Bass,
            _ if named_guitar(name) => Kind::Guitar,
            _ => Kind::Other,
        }
    }

    /// The strings, highest first, for a part whose lowest note is
    /// `lowest`: a seven-string guitar or a five-string bass when the
    /// usual strings don't reach it.
    pub(crate) fn tuning(self, lowest: Option<u8>) -> Vec<Pitch> {
        let low = lowest.unwrap_or(u8::MAX);
        match self {
            Kind::Guitar if (35..40).contains(&low) => {
                [64, 59, 55, 50, 45, 40, 35].map(Pitch).to_vec()
            }
            Kind::Guitar | Kind::Other => [64, 59, 55, 50, 45, 40].map(Pitch).to_vec(),
            Kind::Bass if (23..28).contains(&low) => [43, 38, 33, 28, 23].map(Pitch).to_vec(),
            Kind::Bass => [43, 38, 33, 28].map(Pitch).to_vec(),
            Kind::Drums => Vec::new(),
        }
    }

    /// Whether its tablature is shown.
    pub(crate) fn shows_tablature(self) -> bool {
        matches!(self, Kind::Guitar | Kind::Bass)
    }

    /// The clef its notation is written in, for a part whose notes lie
    /// around `middle` (the median key).
    pub(crate) fn clef(self, middle: Option<u8>) -> Clef {
        match self {
            Kind::Guitar => Clef::Treble8vb,
            Kind::Bass => Clef::Bass8vb,
            Kind::Drums => Clef::Neutral,
            Kind::Other if middle.is_some_and(|m| m < 60) => Clef::Bass,
            Kind::Other => Clef::Treble,
        }
    }
}

/// Whether a part's name says guitar, in the languages files usually come
/// in (for files that play a guitar on another program).
fn named_guitar(name: &str) -> bool {
    let n = name.to_lowercase();
    [
        "guitar", "gtr", "guit.", "violão", "violao", "guitarra", "gitarre", "chitarra", "guitare",
    ]
    .iter()
    .any(|w| n.contains(w))
}

/// General MIDI's name for a program (0-based), as tracks are named when
/// the file names them not.
pub(crate) fn program_name(program: u8) -> &'static str {
    PROGRAMS
        .get(usize::from(program))
        .copied()
        .unwrap_or("Instrument")
}

/// General MIDI Level 1's instrument names.
const PROGRAMS: [&str; 128] = [
    "Acoustic Grand Piano",
    "Bright Acoustic Piano",
    "Electric Grand Piano",
    "Honky-tonk Piano",
    "Electric Piano 1",
    "Electric Piano 2",
    "Harpsichord",
    "Clavinet",
    "Celesta",
    "Glockenspiel",
    "Music Box",
    "Vibraphone",
    "Marimba",
    "Xylophone",
    "Tubular Bells",
    "Dulcimer",
    "Drawbar Organ",
    "Percussive Organ",
    "Rock Organ",
    "Church Organ",
    "Reed Organ",
    "Accordion",
    "Harmonica",
    "Tango Accordion",
    "Nylon Guitar",
    "Steel Guitar",
    "Jazz Guitar",
    "Clean Guitar",
    "Muted Guitar",
    "Overdriven Guitar",
    "Distortion Guitar",
    "Guitar Harmonics",
    "Acoustic Bass",
    "Fingered Bass",
    "Picked Bass",
    "Fretless Bass",
    "Slap Bass 1",
    "Slap Bass 2",
    "Synth Bass 1",
    "Synth Bass 2",
    "Violin",
    "Viola",
    "Cello",
    "Contrabass",
    "Tremolo Strings",
    "Pizzicato Strings",
    "Orchestral Harp",
    "Timpani",
    "String Ensemble 1",
    "String Ensemble 2",
    "Synth Strings 1",
    "Synth Strings 2",
    "Choir Aahs",
    "Voice Oohs",
    "Synth Voice",
    "Orchestra Hit",
    "Trumpet",
    "Trombone",
    "Tuba",
    "Muted Trumpet",
    "French Horn",
    "Brass Section",
    "Synth Brass 1",
    "Synth Brass 2",
    "Soprano Sax",
    "Alto Sax",
    "Tenor Sax",
    "Baritone Sax",
    "Oboe",
    "English Horn",
    "Bassoon",
    "Clarinet",
    "Piccolo",
    "Flute",
    "Recorder",
    "Pan Flute",
    "Blown Bottle",
    "Shakuhachi",
    "Whistle",
    "Ocarina",
    "Square Lead",
    "Sawtooth Lead",
    "Calliope Lead",
    "Chiff Lead",
    "Charang Lead",
    "Voice Lead",
    "Fifths Lead",
    "Bass and Lead",
    "New Age Pad",
    "Warm Pad",
    "Polysynth Pad",
    "Choir Pad",
    "Bowed Pad",
    "Metallic Pad",
    "Halo Pad",
    "Sweep Pad",
    "Rain",
    "Soundtrack",
    "Crystal",
    "Atmosphere",
    "Brightness",
    "Goblins",
    "Echoes",
    "Sci-fi",
    "Sitar",
    "Banjo",
    "Shamisen",
    "Koto",
    "Kalimba",
    "Bagpipe",
    "Fiddle",
    "Shanai",
    "Tinkle Bell",
    "Agogo",
    "Steel Drums",
    "Woodblock",
    "Taiko Drum",
    "Melodic Tom",
    "Synth Drum",
    "Reverse Cymbal",
    "Guitar Fret Noise",
    "Breath Noise",
    "Seashore",
    "Bird Tweet",
    "Telephone Ring",
    "Helicopter",
    "Applause",
    "Gunshot",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds() {
        assert_eq!(Kind::of(9, 25, "Guitar"), Kind::Drums);
        assert_eq!(Kind::of(0, 29, ""), Kind::Guitar);
        assert_eq!(Kind::of(1, 33, ""), Kind::Bass);
        assert_eq!(Kind::of(2, 0, "Piano"), Kind::Other);
        assert_eq!(Kind::of(2, 0, "Violão"), Kind::Guitar);
        assert_eq!(Kind::Guitar.tuning(Some(36)).len(), 7);
        assert_eq!(Kind::Guitar.tuning(Some(40)).len(), 6);
        assert_eq!(Kind::Bass.tuning(Some(24)).len(), 5);
        assert_eq!(Kind::Other.clef(Some(48)), Clef::Bass);
        assert_eq!(program_name(25), "Steel Guitar");
        assert_eq!(program_name(127), "Gunshot");
    }
}
