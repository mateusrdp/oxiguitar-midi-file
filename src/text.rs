// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Texts in MIDI files. The format names no encoding: older files are
//! mostly Latin-1 (ISO 8859-1), newer ones UTF-8. A text that is valid
//! UTF-8 is read as UTF-8, any other as Latin-1, which every byte string
//! is. OxiGuitar writes UTF-8.

/// A text from a file, without control characters or surrounding space.
pub(crate) fn decode(bytes: &[u8]) -> String {
    let text = match std::str::from_utf8(bytes) {
        Ok(s) => s.to_owned(),
        Err(_) => bytes.iter().map(|&b| char::from(b)).collect(),
    };
    text.chars()
        .filter(|c| !c.is_control() || *c == '\n')
        .collect::<String>()
        .trim()
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf8_or_latin1() {
        assert_eq!(decode("Violão".as_bytes()), "Violão");
        assert_eq!(decode(b"Viol\xe3o"), "Violão");
        assert_eq!(decode(b" Verse\0\r "), "Verse");
    }
}
