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

# pack LIBRARY OUT [MANIFEST]: the plugin file OUT (mateusrdp/oxiguitar#347),
# an uncompressed tar archive holding MANIFEST (default manifest.json) as
# manifest.json first, the library second as plugin.so, plugin.dll or
# plugin.dylib, then the ui and assets folders beside the manifest, if
# there are any. COPYFILE_DISABLE keeps macOS's tar from adding ._ files.
pack() {
    local ext="${1##*.}" manifest="${3:-manifest.json}" stage folder
    local more=()
    stage="$(mktemp -d)"
    cp "$manifest" "$stage/manifest.json"
    cp "$1" "$stage/plugin.$ext"
    for folder in ui assets; do
        if [[ -d "$(dirname "$manifest")/$folder" ]]; then
            cp -R "$(dirname "$manifest")/$folder" "$stage/"
            more+=("$folder")
        fi
    done
    (cd "$stage" && COPYFILE_DISABLE=1 tar --format=ustar -cf - manifest.json "plugin.$ext" ${more[@]+"${more[@]}"}) > "$2"
    rm -rf "$stage"
}

build() {
    cargo rustc --locked --release --features oxp --crate-type cdylib "$@"
}

case "$host" in
    *apple*)
        out="dist/$NAME-$version-macos-universal.oxp"
        for target in aarch64-apple-darwin x86_64-apple-darwin; do
            build --target "$target"
        done
        lipo -create -output "target/$(basename "${out%.oxp}").dylib" \
            "target/aarch64-apple-darwin/release/lib$LIBRARY.dylib" \
            "target/x86_64-apple-darwin/release/lib$LIBRARY.dylib"
        pack "target/$(basename "${out%.oxp}").dylib" "$out"
        ;;
    *windows*)
        arch="${host%%-*}"
        [[ "$arch" == aarch64 ]] && arch=arm64
        out="dist/$NAME-$version-windows-$arch.oxp"
        build
        pack "target/release/$LIBRARY.dll" "$out"
        ;;
    *)
        out="dist/$NAME-$version-linux-${host%%-*}.oxp"
        build
        pack "target/release/lib$LIBRARY.so" "$out"
        ;;
esac
echo "$out"
