// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! What a file holds that the score can't carry (reading), or what a score
//! holds that the file can't (writing): one entry per kind, counted, with
//! the first place it happened, as the plugin guide asks.

use oxiguitar_plugin::{Level, LogEntry};

/// A log being written.
#[derive(Debug, Default)]
pub(crate) struct Log {
    entries: Vec<LogEntry>,
}

impl Log {
    /// Something was lost: it is not in the result.
    pub(crate) fn lost(&mut self, message: impl Into<String>, place: impl Into<String>) {
        self.add(Level::Warning, message.into(), place.into());
    }

    /// Something was changed or approximated, nothing lost.
    pub(crate) fn note(&mut self, message: impl Into<String>, place: impl Into<String>) {
        self.add(Level::Info, message.into(), place.into());
    }

    fn add(&mut self, level: Level, message: String, place: String) {
        if let Some(e) = self
            .entries
            .iter_mut()
            .find(|e| e.message == message && e.level == level)
        {
            e.count = e.count.saturating_add(1);
            return;
        }
        self.entries.push(LogEntry {
            level,
            message,
            place,
            count: 1,
        });
    }

    /// The entries, in the order they first happened.
    pub(crate) fn into_entries(self) -> Vec<LogEntry> {
        self.entries
    }
}

/// "bar 12, track 2", as places read.
pub(crate) fn place(bar: usize, track: usize) -> String {
    format!("bar {}, track {}", bar + 1, track + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_entry_per_kind_counted() {
        let mut log = Log::default();
        log.lost("a", "bar 1, track 1");
        log.lost("a", "bar 2, track 1");
        log.note("b", "bar 3, track 1");
        let e = log.into_entries();
        assert_eq!(e.len(), 2);
        assert_eq!(e[0].count, 2);
        assert_eq!(e[0].place, "bar 1, track 1");
        assert_eq!(e[1].level, Level::Info);
    }
}
