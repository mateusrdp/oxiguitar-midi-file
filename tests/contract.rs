// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The contract suite: builds this crate as an `.oxp` file and checks it
//! through OxiGuitar's own plugin host, as the app would load it. `cargo
//! test` runs it with the rest.

use std::path::Path;

#[test]
fn the_plugin_file_keeps_the_contract() {
    let crate_folder = Path::new(env!("CARGO_MANIFEST_DIR"));
    let oxp =
        oxiguitar_plugin_test::build(crate_folder, env!("CARGO_PKG_NAME"), "midi-file", false)
            .expect("the .oxp file builds");
    let report = oxiguitar_plugin_test::check(&oxp);
    for passed in &report.passed {
        println!("ok    {passed}");
    }
    assert!(report.ok(), "failed: {:#?}", report.failed);
}

#[test]
fn its_signatures_recognise_midi_files_only() {
    use oxiguitar_plugin::api::manifest::Manifest;
    use oxiguitar_plugin::api::signature::Sniff;
    let manifest: Manifest =
        serde_json::from_str(include_str!("../manifest.json")).expect("the manifest reads");
    let mid =
        std::fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/oxiguitar-test.mid"))
            .unwrap();
    let len = u32::try_from(mid.len()).unwrap();
    let mut rmi = b"RIFF".to_vec();
    rmi.extend_from_slice(&(len + 12).to_le_bytes());
    rmi.extend_from_slice(b"RMIDdata");
    rmi.extend_from_slice(&len.to_le_bytes());
    rmi.extend_from_slice(&mid);
    let recognises = |format: usize, data: &[u8]| {
        let sniff = Sniff::new(data);
        manifest.formats[format]
            .signatures
            .iter()
            .any(|s| s.matches(&sniff))
    };
    assert!(recognises(0, &mid) && !recognises(1, &mid));
    assert!(recognises(1, &rmi) && !recognises(0, &rmi));
    let wav = b"RIFF\x24\0\0\0WAVEfmt \x10\0\0\0";
    assert!(!recognises(0, wav) && !recognises(1, wav));
    assert!(oxiguitar_midi_file_plugin::read(&rmi).is_ok());
}
