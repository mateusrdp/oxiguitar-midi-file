// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Whatever a file reads into is written back, and what was written reads
//! again.

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(read) = oxiguitar_midi_file_plugin::read(data) else {
        return;
    };
    let written =
        oxiguitar_midi_file_plugin::write(&read.score).expect("a score that was read can be written");
    oxiguitar_midi_file_plugin::read(&written.data).expect("what was written reads");
});
