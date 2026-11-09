#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors
#
# Seeds both fuzz targets with the repository's own fixtures.
set -euo pipefail
cd "$(dirname "$0")"
for target in read write; do
    mkdir -p "corpus/$target"
    cp ../fixtures/*.mid "corpus/$target/"
done
