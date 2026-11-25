# Requirements and assumptions

## Requirements

- OxiGuitar 0.23 or later, which reads plugin files as archives (the plugin asks for interface 1.7). See [the OxiGuitar manual](https://mateusrdp.github.io/oxiguitar/).
- Windows, macOS or Linux.
- To hear a song you also need a plugin that plays scores and an audio output plugin.

## Assumptions

- A MIDI file records when notes were played, not what their values are. The grid OxiGuitar chooses is a good guess, not a certainty, so check the result before you rely on it.
- A MIDI file can't hold everything a score can. Saving one is for playing the song elsewhere, not for keeping the score.
