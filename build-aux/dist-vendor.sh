#!/bin/sh
# Bundle Cargo vendored deps into `meson dist` (Fractal pattern).
# Args: DIST_DIR SOURCE_ROOT. Creates $DIST/.cargo/config pointing at
# $DIST/vendor so offline/Flatpak builds need no network.
set -eu
DIST="$1"
SOURCE_ROOT="$2"
cd "$SOURCE_ROOT"
mkdir -p "$DIST/.cargo"
cargo vendor --locked 2>/dev/null | sed 's/^directory = ".*"/directory = "vendor"/' > "$DIST/.cargo/config"
mv vendor "$DIST/vendor"
