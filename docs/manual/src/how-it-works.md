# Architecture and strategy

The plugin is one file. Reading turns the times at which notes were played into notation by quantising them beat by beat. Writing doesn't copy the score. It takes the timeline OxiGuitar plays and writes that, which is why what you hear is what the file holds.

Parsing and writing the file itself is done by the `midly` library, used as published. The strategy is to fix that library's gaps in our own code, and to decide the file type from the first bytes: `MThd` for a MIDI file, `RMID` in the header for RIFF MIDI.

`docs/design.md` in the repository says what is read and written and what is left out.
