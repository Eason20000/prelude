#!/usr/bin/env bash
# One-command Android build for Prelude (debug APK).
#
# SPDX-License-Identifier: GPL-3.0-only
#
# Flow: pixiewood prepare (first run / --reprepare) → generate →
# apply-android-patches.sh → build → verify. Everything Android-specific
# lives in experiments/; the repo root build is untouched.
#
# Prerequisites: source ./env.sh first (plus the toolchain shell, see
# README.md). The whole flow must run with the same environment.
#
# Usage: ./build-android.sh [--reprepare]
set -euo pipefail

cd "$(dirname "$0")"

PIXIEWOOD_DIR="${PIXIEWOOD_DIR:-$HOME/android-work/gtk-android-builder}"
MESON_BIN="${MESON_BIN:-$HOME/android-work/pyenv/bin/meson}"
APK="app-arm64-v8a-debug.apk"

# --- tool checks -----------------------------------------------------------
for tool in perl "$MESON_BIN" cargo java adb; do
    command -v "$tool" >/dev/null 2>&1 || {
        echo "build-android: missing tool: $tool (source ./env.sh + toolchain shell first)" >&2
        exit 1
    }
done
[ -d "$ANDROID_HOME/ndk/27.2.12479018" ] || {
    echo "build-android: NDK 27.2.12479018 missing under \$ANDROID_HOME (see pixiewood.lock)" >&2
    exit 1
}

# --- pixiewood revision pin -------------------------------------------------
want_rev=$(grep -oP '^pixiewood\s*=\s*"\K[0-9a-f]+' pixiewood.lock)
have_rev=$(git -C "$PIXIEWOOD_DIR" rev-parse HEAD)
[ "$have_rev" = "$want_rev" ] || {
    echo "build-android: pixiewood rev $have_rev != lock $want_rev; bump pixiewood.lock deliberately" >&2
    exit 1
}

# --- version sync: Cargo.toml vs metainfo ----------------------------------
cargo_ver=$(grep -oP '^version = "\K[^"]+' ../../Cargo.toml)
meta_ver=$(grep -oP -m1 '<release version="\K[^"]+' data/top.vikasmi.Prelude.metainfo.xml)
[ "$cargo_ver" = "$meta_ver" ] || {
    echo "build-android: version drift: Cargo.toml $cargo_ver vs metainfo $meta_ver" >&2
    exit 1
}

# --- cargo freshness guard ---------------------------------------------------
# Meson cannot see Rust sources as custom_target inputs, and cargo has
# missed mtime-only changes before (stale .a linked into a fresh APK).
# Touching is cheap (incremental rebuild) and forces a real recheck.
touch ../../src/lib.rs ../../src/engine.rs ../../src/application.rs \
    ../../src/main.rs ../../src/config.rs

# --- prepare (first run or forced) ------------------------------------------
# The Android sysroot must NOT leak into configure: Meson resolves
# build-machine tools (e.g. glib-mkenums for harfbuzz) through pkg-config,
# and a sysroot-prefixed tool path from a foreign .pc fails the build on a
# fresh sysroot. Cargo (at build time) is the only consumer of the sysroot.
if [ ! -f .pixiewood/bin-aarch64/build.ninja ] || [ "${1:-}" = "--reprepare" ]; then
    env -u PKG_CONFIG_PATH -u PKG_CONFIG_SYSROOT_DIR \
        perl "$PIXIEWOOD_DIR/pixiewood" prepare --meson "$MESON_BIN" \
        -s "$ANDROID_HOME" -t "$ANDROID_NDK_HOME" pixiewood.xml
else
    echo "build-android: reusing configured .pixiewood (pass --reprepare to redo)"
fi

# --- generate + patches ------------------------------------------------------
perl "$PIXIEWOOD_DIR/pixiewood" generate
./apply-android-patches.sh

# --- pkg-config staging for cargo --------------------------------------------
# The build tree already contains a full `-uninstalled.pc` set (52 files,
# plain-name Requires chains, absolute build-tree paths) right after
# prepare — no `meson install` involved. That matters: install would first
# rebuild every target including our cargo staticlib, which needs these very
# files to link: a hard deadlock on fresh machines.
#
# The staged copies keep their absolute -I/-L paths, so PKG_CONFIG_SYSROOT_DIR
# must stay unset (it would prefix-garble them); see env.sh.
PC_SRC=".pixiewood/bin-aarch64/meson-uninstalled"
PC_DST="$PRELUDE_ANDROID_SYSROOT/lib/arm64-v8a/pkgconfig"
[ -d "$PC_SRC" ] || {
    echo "build-android: $PC_SRC missing; run with --reprepare" >&2
    exit 1
}
mkdir -p "$PC_DST"
for pc in "$PC_SRC"/*-uninstalled.pc; do
    base=$(basename "$pc" -uninstalled.pc)
    cp "$pc" "$PC_DST/$base.pc"
done
[ -f "$PC_DST/gtk4.pc" ] || {
    echo "build-android: staging produced no gtk4.pc" >&2
    exit 1
}

perl "$PIXIEWOOD_DIR/pixiewood" build

# --- verify ------------------------------------------------------------------
APK_PATH=".pixiewood/android/app/build/outputs/apk/debug/$APK"
[ -f "$APK_PATH" ] || {
    echo "build-android: expected $APK_PATH missing" >&2
    exit 1
}
grep -q 'android:name="top.vikasmi.prelude.PreludeActivity"' \
    .pixiewood/android/app/src/main/AndroidManifest.xml || {
    echo "build-android: manifest patch missing (PreludeActivity)" >&2
    exit 1
}
"$ANDROID_HOME/build-tools/36.0.0/aapt" dump badging "$APK_PATH" 2>/dev/null \
    | grep -q "package: name='top.vikasmi.prelude'" || {
    echo "build-android: badging check failed for $APK_PATH" >&2
    exit 1
}
echo "build-android: OK -> $APK_PATH ($(du -h "$APK_PATH" | cut -f1))"
