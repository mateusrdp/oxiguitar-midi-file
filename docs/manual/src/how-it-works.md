# Architecture and strategy

The plugin is one file. Reading turns the times at which notes were played into notation by quantising them beat by beat. Writing doesn't copy the score. It takes the timeline OxiGuitar plays and writes that, which is why what you hear is what the file holds.

Parsing and writing the file itself is done by the `midly` library, used as published. The strategy is to fix that library's gaps in our own code, and to decide the file type from the first bytes: `MThd` for a MIDI file, `RMID` in the header for RIFF MIDI.

`docs/design.md` in the repository says what is read and written and what is left out.

## Current limitations

- **Bends and slides read back as plain notes.** Pitch bend in a file is named in the import log and left out; turning a bend curve back into OxiGuitar's bend marks is possible later work.
- **One instrument per track.** A program change after a track's first note is named in the log and left out.
- **The first song only** of a format 2 file (several songs in one), and **at most 4096 bars**.
- **Files timed in frames** (SMPTE) are read at 120 beats a minute.
- **Swing and rubato** quantise to straight notes as well as the grid allows; there is no swing detection.
- **No `.rmi` writing**: it is the same file in a wrapper few programs need, and a `.mid` opens everywhere `.rmi` does.

## Left out on purpose

- **Writing the score's notation.** A MIDI file can't hold it, so the writer doesn't pretend: it writes what plays. Keep the score in a Guitar Pro, MusicXML or TuxGuitar file.
- **A MIDI parser of our own.** `midly` reads and writes the bytes well; we use it as published and handle its gaps in our code.
- **Guessing from the file name.** The file type is decided from its first bytes, never from the extension.

## Where you can help

- **Bends from pitch bend**: recognising bend, release and slide shapes in a pitch-bend curve (`src/`, see `docs/design.md`).
- **Swing**: detecting a swung feel and writing straight eighths with a swing marking.
- **MIDI files from real players**, especially MIDI guitars and karaoke files, to test with. Never attach files you aren't allowed to share.

Every change needs its tests and a row on the [Features](features.md) page; the [Testing](testing.md) page says how to run them.

