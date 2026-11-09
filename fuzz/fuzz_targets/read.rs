// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Any bytes, read as a MIDI file: an error or a score, never a panic, a
//! hang or an allocation of what the file claims.

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = oxiguitar_midi_file_plugin::read(data);
});
