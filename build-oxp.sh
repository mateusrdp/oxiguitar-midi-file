#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors
#
# Builds the MIDI file plugin's release file for the system this runs on:
#
#   dist/midi-file-VERSION-SYSTEM-ARCH.oxp
#
# such as dist/midi-file-0.1.0-linux-x86_64.oxp (0.0.0 until the first release), named the way OxiGuitar's
# plugins are (docs/plugins/template in github.com/mateusrdp/oxiguitar). On
# macOS it builds both processors and joins them into one universal file
# (needs both Rust targets: rustup target add aarch64-apple-darwin
# x86_64-apple-darwin). On Windows, run it from Git Bash.
# .github/workflows/release.yml runs it on each system.

set -euo pipefail

NAME=midi-file
LIBRARY=oxiguitar_midi_file_plugin

cd "$(dirname "$0")"
version="$(cargo pkgid)"
version="${version##*[#@]}"
host="$(rustc -vV | sed -n 's/^host: //p')"
rm -rf dist
mkdir -p dist

build() {
    cargo rustc --locked --release --features oxp --crate-type cdylib "$@"
}

case "$host" in
    *apple*)
        out="dist/$NAME-$version-macos-universal.oxp"
        for target in aarch64-apple-darwin x86_64-apple-darwin; do
            build --target "$target"
        done
        lipo -create -output "$out" \
            "target/aarch64-apple-darwin/release/lib$LIBRARY.dylib" \
            "target/x86_64-apple-darwin/release/lib$LIBRARY.dylib"
        ;;
    *windows*)
        arch="${host%%-*}"
        [[ "$arch" == aarch64 ]] && arch=arm64
        out="dist/$NAME-$version-windows-$arch.oxp"
        build
        cp "target/release/$LIBRARY.dll" "$out"
        ;;
    *)
        out="dist/$NAME-$version-linux-${host%%-*}.oxp"
        build
        cp "target/release/lib$LIBRARY.so" "$out"
        ;;
esac
echo "$out"
