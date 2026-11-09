# Fuzzing

Two targets, run with [cargo-fuzz](https://github.com/rust-fuzz/cargo-fuzz) on nightly Rust from this folder:

- `read`: any bytes as a MIDI file (plain or RIFF). An error or a score, never a panic.
- `write`: whatever reads is written back, and reads again.

```sh
cargo install cargo-fuzz
./seeds.sh
cargo +nightly fuzz run read corpus/read -- -max_total_time=600
```

`seeds.sh` copies the repository's fixtures into `corpus/` (which git ignores). Never put files from the private test corpus there.
