# The MIDI file plugin: design

The MIDI file plugin (`midi-file.oxp`, id `oxiguitar.midi-file`) reads and writes Standard MIDI Files for OxiGuitar (#223 in the OxiGuitar repository). It is a format plugin only: it has no sound. Playing sound sets is a different plugin, [oxiguitar-midi](https://github.com/mateusrdp/oxiguitar-midi) (`oxiguitar.midi`); the two share the word MIDI and nothing else.

| Format id | Name | Extensions | Read | Write | Recognised by |
| --- | --- | --- | --- | --- | --- |
| `mid` | MIDI file | .mid, .midi, .kar | yes | yes | `MThd` at the start |
| `rmi` | RIFF MIDI file | .rmi | yes | no | `RMID` at byte 8 (a RIFF file holding a MIDI file) |

Writing `.rmi` is left out on purpose: it is the same file in a wrapper few programs need, and a `.mid` opens everywhere `.rmi` does.

## Decided by Mateus (9 Nov 2026)

- **Writing: what OxiGuitar plays.** A MIDI file has no repeats, tablature or notation, only timed notes, so the file is written from OxiGuitar's player timeline (`oxiguitar-play`): it sounds in other programs as it does in OxiGuitar.
- **Reading: an automatic grid.** Most MIDI files are played performances. Each beat of each track picks the grid that fits its notes best, straight down to a 32nd or triplet; notes crossing a bar line become tied pieces. No dialog: a reader is given the file and nothing else.
- **Tablature as for MusicXML.** General MIDI's guitars (programs 24-31) and basses (32-39) get strings and show their tablature; every other pitched track gets a guitar's strings with its tablature hidden; channel 10 is drums. OxiGuitar fingers the notes as the file opens (#157).
- **The midly crate** (0.5.3, Unlicense), as published, parses and writes the files.

## Reading

`src/read.rs`.

- **Tracks.** Each channel a track of the file plays notes on becomes a track of the score: a format 1 file's tracks usually play one channel each, a format 0 file holds every channel in its one track. A track keeps the file's track name when it is the only part of that track; otherwise it is named after its General MIDI program ("Drums" for channel 10). The program is the last one set before the first note; later program changes are named in the log and left out. The volume (CC 7) and pan (CC 10) set before the first note become the track's mix. A format 2 file (several songs in one) is read as its first song.
- **The header.** The title is the name of the first track when that track plays nothing (the conductor track of a format 1 file), or of the only track of a format 0 file. Texts at the start of tracks that play nothing become the header's fields when they read `Artist: …`, `Album: …` and so on (as this plugin writes them), notices otherwise; the copyright meta event is the copyright. A karaoke file's `@T` headers give the title and the artist.
- **Time.** Ticks become units of a 192nd of a whole note: every grid point is a whole number of them. A file timed in frames a second (SMPTE) is read at 120 beats a minute.
- **Bars** follow the time signatures from the start, 4/4 before the first. A time signature inside a bar ends the bar there: the short bar gets a time signature of its own length (2/4, 3/8…), so it adds up. Time signatures with units below a 64th are read as 64ths. At most 4096 bars are read; the log names anything past them.
- **Tempo** changes go on a sixteenth's grid inside their bars (the first is the song's tempo). **Key signatures** and **markers** (and cue points) go on the bar they fall nearest the start of.
- **Quantising** (per track). Each bar is cut into cells: a quarter note, or a dotted beat in a compound time (6/8, 9/8, 12/8, 6/16). For each cell the reader tries each grid (a quarter cell: 1, 2, 4 or 8 parts, or triplets of 3 or 6; a dotted cell: 1, 3, 6 or 12 parts) and keeps the one with the smallest cost: the distance of the cell's note starts and ends to the grid, plus a small price per note for a finer grid, so a slightly late sixteenth stays a sixteenth and a run of triplets finds its grid. Starts weigh twice as much as ends, which players leave loosely. A note snaps its start and end to its cells' grids; a note that snaps to nothing lasts one grid step; a note struck again on the same key ends the one before it; two of one key struck at once are one.
- **Voices.** A track's notes are cut into one voice at every start and end, at every bar line and at the edges of a triplet cell: what sounds through a cut continues as a tied note, nothing else is changed, so what plays is what the file played, on the grid. Drum hits are cut at their starts only and last to the next hit. Lengths are written with `time::fill`: plain, dotted and tuplet values (the same function as the MusicXML plugin's).
- **Velocity** becomes each note's dynamic, the nearest of OxiGuitar's (15 for ppp to 127 for fff in steps of 16, as Guitar Pro plays them).
- **Lyrics** (lyric meta events, and a karaoke file's text events) go on the sung beats of the track they are in, or of the first pitched track when theirs plays nothing: each syllable on the beat sung when the file sings it (a 32nd early still counts), beats sung without one left empty, a trailing hyphen continuing the word.
- **Strings.** A guitar's lowest note below E2 makes it a seven-string guitar (from B1), a bass's below E1 a five-string bass. Notes come with pitches only (`NotePlace::Pitched`) and OxiGuitar fingers them as the file opens. Clefs: guitars treble 8vb, basses bass 8vb, drums neutral, any other track treble or bass by its middle note.
- **Left out, and named in the log:** pitch bends (OxiGuitar's bends and slides are marks on notes; turning curves back into them is possible later work); program changes after a track's first note; songs after the first of a format 2 file; bars past the 4096th.
- **Texts** are UTF-8 when they are valid UTF-8 and Latin-1 otherwise (`src/text.rs`): the format names no encoding.

## Writing

`src/write.rs`. The file is format 1 at 960 ticks a quarter note.

- **The first track** holds the title (its track name), the copyright, the header's other fields as texts (`Artist: …`), the tempo map, time and key signatures and markers.
- **The tempo map.** OxiGuitar's timeline gives every played bar's time in seconds (tempo changes, gradual ones, fermatas). Each played bar is sampled every 192nd of a whole note and a tempo event is written wherever the tempo changes, so bars and beats fall where OxiGuitar plays them, and the notes' times (swing included) are turned into ticks along that same map.
- **Bars as played.** Repeats, endings and jumps are played out (the log says so). A bar played longer or shorter than its time signature (one that doesn't add up, a free-time bar) gets a time signature of its own length, so the file reads back bar for bar; a length no time signature can show (a triplet's) keeps the bar's own and is named.
- **One track of the file for each of the score's tracks**, named after it, and one more for a track whose chord diagrams play (#151), named "NAME (chords)". Each channel used is set up at the start: the pitch bend range (24 semitones, as the sound plugin uses), the program, volume and pan.
- **Channels.** A fretted track gives each string a channel, as the sound plugin does, so a bend moves only its string, when the 15 channels besides the drums' (channel 10) hold that for every track. Otherwise each track with bends or vibrato gets two channels, its notes that bend on the second (as Guitar Pro does), and failing that one; with more than 15 pitched tracks some share a channel, and the log says so.
- **Notes as the sound plugin plays them** (oxiguitar-midi's `play.rs`): a new note on a string ends the one before it; bends, slides and the whammy bar are the timeline's pitch curves, sent as pitch bend every 10 ms; vibrato is a curve of 30 cents at 5.5 Hz (60 cents at 4.5 Hz for the whammy bar's) reaching full depth in 0.1 s; palm mutes end after 0.15 s at 90 % velocity, dead notes after 30 ms at 60 %; hammer-ons, pull-offs and legato slides are softer (70 %); natural, artificial and tapped harmonics on clean guitars play General MIDI's Guitar Harmonics (program 31), slaps and pops on basses Slap Bass 1 and 2; fades are expression (CC 11). Ghost notes, accents, dynamics, hairpins, let ring, grace notes, strums, tremolo picking and trills come from the timeline as they are.
- **Lyrics.** A track's first lyric line, each syllable as a lyric event on the first note of the beat that sings it, every time that bar plays, with a hyphen when the word goes on or a space when it ends. Further lines are named as lost.
- **Named as lost** before saving: the notation (note values as written, marks, texts, chord diagrams, spellings) and the strings and frets (OxiGuitar fingers the notes again when it opens the file).

## Tests

- `tests/read_write.rs`: files made event by event as a player records them (a little off the beat), with triplets, sixteenths, a tie over a bar line, a time-signature change, a key, a marker, lyrics, drums and a bass, read onto the grid; what is read is written and reads back the same; repeats written out; RIFF files; files that are not MIDI.
- `tests/contract.rs`: the `.oxp` file through OxiGuitar's plugin host (the contract suite), and the signatures against a MIDI file, a RIFF MIDI file and a WAV file.
- `tests/corpus.rs`: every MIDI file in `corpus/files/` (never committed) reads, and from the second trip writes and reads back the same. The private corpus has no MIDI files: its Guitar Pro files become MIDI files with OxiGuitar's command-line tool and the Guitar Pro plugins (`OXIGUITAR_HOME` holding both plugins: `oxiguitar-cli convert SONG.gp corpus/files/SONG.mid`).
- `fuzz/`: the reader, and the writer on whatever reads.
- `fixtures/oxiguitar-test.mid`: OxiGuitar's own test song (the core's `fixtures/gp/oxiguitar-test-song.gp`) written by this plugin.

## Possible later work (not asked for)

- Pitch bends read back as bends, slides and vibrato on the notes they move.
- Several voices when a track's notes overlap with different lengths (now one voice with ties).
- A track's program changes as separate tracks or as instrument changes.
- Writing `.rmi`.
