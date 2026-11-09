// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Lengths as note values: what a quantised length is written as.

use oxiguitar_model::{Duration, NoteValue, Ratio, Tuplet};

/// Every note value, longest first.
const VALUES: [NoteValue; 10] = [
    NoteValue::DoubleWhole,
    NoteValue::Whole,
    NoteValue::Half,
    NoteValue::Quarter,
    NoteValue::Eighth,
    NoteValue::Sixteenth,
    NoteValue::ThirtySecond,
    NoteValue::SixtyFourth,
    NoteValue::OneTwentyEighth,
    NoteValue::TwoFiftySixth,
];

/// Durations, longest first, that together last exactly `len`: for rests
/// and gaps the file leaves, where only the length is known. Lengths that
/// need a tuplet (a third of a beat) get triplets, quintuplets, sextuplets
/// or septuplets. Empty when `len` is zero or can't be made of at most 16
/// pieces.
pub(crate) fn fill(len: Ratio) -> Vec<Duration> {
    if len <= Ratio::ZERO {
        return Vec::new();
    }
    let mut out = Vec::new();
    // The dyadic part plainly; what is left in the first tuplet that
    // makes it dyadic.
    let tuplets = [
        None,
        Some(Tuplet::TRIPLET),
        Some(Tuplet {
            notes: 5,
            in_time_of: 4,
        }),
        Some(Tuplet {
            notes: 6,
            in_time_of: 4,
        }),
        Some(Tuplet {
            notes: 7,
            in_time_of: 4,
        }),
    ];
    for tuplet in tuplets {
        // The length as written inside the tuplet.
        let factor = match tuplet {
            None => Ratio::ONE,
            Some(t) => match t.factor() {
                Some(f) => f,
                None => continue,
            },
        };
        let Some(written) = len.checked_div(factor) else {
            continue;
        };
        if !is_dyadic(written) {
            continue;
        }
        let mut left = written;
        while left > Ratio::ZERO && out.len() < 16 {
            let Some(value) = VALUES.iter().copied().find(|v| v.length() <= left) else {
                break;
            };
            // A dot or two where they fit exactly in what is left.
            let mut d = Duration::new(value);
            for dots in [2u8, 1] {
                let dotted = Duration::new(value);
                let dotted = Duration { dots, ..dotted };
                if dotted.length().is_some_and(|l| l <= left) {
                    d = dotted;
                    break;
                }
            }
            let Some(next) = d.length().and_then(|l| left.checked_sub(l)) else {
                break;
            };
            left = next;
            out.push(Duration {
                tuplets: tuplet.into_iter().collect(),
                ..d
            });
        }
        if left == Ratio::ZERO {
            return out;
        }
        out.clear();
    }
    out
}

/// Whether `r`'s denominator is a power of two (and so a sum of plain
/// note values).
fn is_dyadic(r: Ratio) -> bool {
    let d = r.denom();
    d > 0 && d & (d - 1) == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(n: i64, d: i64) -> Ratio {
        Ratio::new(n, d).unwrap()
    }

    #[test]
    fn fills_lengths() {
        let lens = |v: Vec<Duration>| v.iter().map(|d| d.length().unwrap()).collect::<Vec<_>>();
        assert_eq!(lens(fill(r(1, 1))), vec![r(1, 1)]);
        assert_eq!(fill(r(3, 4)).len(), 1, "a dotted half");
        assert_eq!(lens(fill(r(5, 8))), vec![r(1, 2), r(1, 8)]);
        let triplet = fill(r(1, 12));
        assert_eq!(triplet.len(), 1);
        assert_eq!(triplet[0].tuplets, vec![Tuplet::TRIPLET]);
        assert!(fill(Ratio::ZERO).is_empty());
        assert!(fill(r(1, 11)).is_empty(), "no tuplet here makes elevenths");
        for n in 1..64 {
            let len = r(n, 32);
            assert_eq!(
                fill(len).iter().map(|d| d.length().unwrap()).sum::<Ratio>(),
                len
            );
        }
    }
}
