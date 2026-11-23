# Credits

Thank you to the people whose work this plugin is built on.

## Code we use

| Project | Authors | What it does here | Licence |
| --- | --- | --- | --- |
| [OxiGuitar's plugin SDK](https://github.com/mateusrdp/oxiguitar) (`oxiguitar-plugin`, `oxiguitar-model`, `oxiguitar-play`, and `oxiguitar-plugin-test` for the tests) | Mateus Rocha de Paula and the OxiGuitar contributors | The plugin interface, the score model, and the timeline OxiGuitar plays, which the writer saves | MIT OR Apache-2.0 for the interface and SDK; GPL-3.0-or-later for the model and player |
| [midly](https://github.com/negamartin/midly) | negamartin and contributors | Reading and writing the bytes of a MIDI file | Unlicense |
| [serde_json](https://github.com/serde-rs/json) | David Tolnay, Erick Tryzelaar and contributors | Tests only | MIT OR Apache-2.0 |
| [Rust](https://www.rust-lang.org/) and its standard library | The Rust Project | Everything else | MIT OR Apache-2.0 |

The quantiser, the lyrics, the strings and everything between the bytes and the score are our own code.

## Standards

| Standard | By |
| --- | --- |
| Standard MIDI Files and General MIDI | The MIDI Manufacturers Association, now [the MIDI Association](https://midi.org/) |
| RIFF MIDI (`.rmi`) | Microsoft and IBM's RIFF container |
| The karaoke `.kar` convention | Tune 1000 |

## Tools

| Tool | Use |
| --- | --- |
| [cargo-fuzz](https://github.com/rust-fuzz/cargo-fuzz) and [libfuzzer-sys](https://github.com/rust-fuzz/libfuzzer) | Fuzzing the reader and the writer (`fuzz/`) |
| [mdBook](https://github.com/rust-lang/mdBook) | Builds this manual |
| [cargo-deny](https://github.com/EmbarkStudios/cargo-deny) | Checks the licences of everything we build with |

## Thanks

To negamartin for midly, and to everyone who has kept MIDI files working for four decades.
