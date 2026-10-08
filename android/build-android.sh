#!/usr/bin/env bash
# One-command Android build for Prelude.
#
# SPDX-License-Identifier: GPL-3.0-only
#
# Debug APK by default (desktop convention); PRELUDE_RELEASE=1 selects
# release (fresh-configured runtime, cargo --release, Gradle assembleRelease,
# debug-keystore signing). Everything Android-specific lives in android/;
# the repo root build is untouched.
#
# Prerequisites: source ./env.sh first (plus the toolchain shell, see
# README.md). The whole flow must run with the same environment.
#
# Usage: ./build-android.sh [--reprepare]
#        PRELUDE_RELEASE=1 ./build-android.sh
set -euo pipefail

cd "$(dirname "$0")"

PIXIEWOOD_DIR="${PIXIEWOOD_DIR:-$HOME/android-work/gtk-android-builder}"
MESON_BIN="${MESON_BIN:-$HOME/android-work/pyenv/bin/meson}"
if [ -n "${PRELUDE_RELEASE:-}" ]; then
    APK_SUBDIR="release"
    APK="app-arm64-v8a-release.apk"
else
    APK_SUBDIR="debug"
    APK="app-arm64-v8a-debug.apk"
fi

# --- tool checks -----------------------------------------------------------
for tool in perl "$MESON_BIN" cargo java adb keytool; do
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

# --- version sync (single source: Cargo.toml) -------------------------------
# Desktop files are derived by Meson configure_file (nothing to compare);
# the Android metainfo is consumed by Pixiewood from the source tree, so it
# is checked here via the shared helper (same gate runs at Meson setup).
python3 ../build-aux/check_version.py \
    --manifest ../Cargo.toml \
    --android-metainfo data/top.vikasmi.Prelude.metainfo.xml

# NOTE (retired touch hack): cargo freshness used to need `touch` because
# Meson listed no Rust inputs. Both meson.build files now declare narrowed
# inputs + build_always_stale, so cargo rechecks every build by itself.

# --- sccache launcher for the NDK compilers ---------------------------------
# Pixiewood wires NDK clang into Meson via prepare/android.cross in its own
# checkout (not our ANDROID_WRAP_DIR shims, which only serve cargo linking),
# and Meson cross builds require the launcher written explicitly (no
# CC_LAUNCHER env). Patched here, BEFORE prepare configures anything, so no
# reconfigure is ever needed on fresh trees. CI provides sccache; local
# builds without it leave the template alone (install + --reprepare to pick
# it up later; uninstall reverts via backup).
python3 ../build-aux/patch_meson_cross.py

# --- prepare (first run, forced, or release) ----------------------------------
# The staged pkg-config path must NOT leak into configure: Meson resolves
# build-machine tools through pkg-config, and the Android .pc files would
# shadow the host ones. Cargo (at build time) is the only consumer.
# Release always starts from a clean configure: `meson setup` cannot flip
# buildtype in place, and the Gradle assemble type follows this flag too.
if [ -n "${PRELUDE_RELEASE:-}" ]; then
    rm -rf .pixiewood/bin-aarch64
fi
if [ ! -f .pixiewood/bin-aarch64/build.ninja ] || [ "${1:-}" = "--reprepare" ]; then
    prepare_args=()
    if [ -n "${PRELUDE_RELEASE:-}" ]; then
        prepare_args+=(--release)
    fi
    env -u PKG_CONFIG_PATH -u PKG_CONFIG_SYSROOT_DIR \
        perl "$PIXIEWOOD_DIR/pixiewood" prepare "${prepare_args[@]}" --meson "$MESON_BIN" \
        -s "$ANDROID_HOME" -t "$ANDROID_NDK_HOME" pixiewood.xml
else
    echo "build-android: reusing configured .pixiewood (pass --reprepare to redo)"
fi

# --- stale-tree guard for the sccache launcher --------------------------------
# A reused .pixiewood configured before the template was patched (or after a
# revert) still points at the bare compiler; reconfigure once so ninja picks
# up the current template. Fresh prepares never land here.
if [ -f .pixiewood/bin-aarch64/build.ninja ] \
    && command -v sccache >/dev/null 2>&1 \
    && ! grep -q sccache .pixiewood/bin-aarch64/build.ninja 2>/dev/null; then
    echo "build-android: cross template changed since configure; reconfiguring"
    env -u PKG_CONFIG_PATH -u PKG_CONFIG_SYSROOT_DIR \
        "$MESON_BIN" setup --reconfigure .pixiewood/bin-aarch64
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

# --- sign release (debug keystore; publishing keys are future work) ---------
if [ -n "${PRELUDE_RELEASE:-}" ]; then
    KEYSTORE="${ANDROID_DEBUG_KEYSTORE:-$HOME/.android/debug.keystore}"
    if [ ! -f "$KEYSTORE" ]; then
        keytool -genkeypair -keystore "$KEYSTORE" -alias androiddebugkey \
            -storepass android -keypass android -keyalg RSA -keysize 2048 \
            -validity 10000 -dname "CN=Android Debug,O=Android,C=US"
    fi
    APKSIGNER="$ANDROID_HOME/build-tools/36.0.0/apksigner"
    [ -x "$APKSIGNER" ] || {
        echo "build-android: apksigner missing under \$ANDROID_HOME/build-tools/36.0.0" >&2
        exit 1
    }
    UNSIGNED=".pixiewood/android/app/build/outputs/apk/release/app-arm64-v8a-release-unsigned.apk"
    SIGNED=".pixiewood/android/app/build/outputs/apk/release/$APK"
    [ -f "$UNSIGNED" ] || {
        echo "build-android: expected $UNSIGNED missing" >&2
        exit 1
    }
    "$APKSIGNER" sign --ks "$KEYSTORE" --ks-pass pass:android --key-pass pass:android \
        --ks-key-alias androiddebugkey --out "$SIGNED" "$UNSIGNED"
    "$APKSIGNER" verify "$SIGNED"
fi

# --- verify ------------------------------------------------------------------
APK_PATH=".pixiewood/android/app/build/outputs/apk/$APK_SUBDIR/$APK"
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
