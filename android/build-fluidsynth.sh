#!/usr/bin/env bash
# Cross-compile the minimal FluidSynth + Oboe runtime for Android arm64.
#
# SPDX-License-Identifier: GPL-3.0-only
#
# The result is installed into the Pixiewood sysroot:
#   $PRELUDE_ANDROID_SYSROOT/lib/arm64-v8a/libfluidsynth.so
#   $PRELUDE_ANDROID_SYSROOT/lib/arm64-v8a/liboboe.so
#   $PRELUDE_ANDROID_SYSROOT/lib/arm64-v8a/libc++_shared.so
#   $PRELUDE_ANDROID_SYSROOT/lib/arm64-v8a/pkgconfig/fluidsynth.pc
#
# build-android.sh calls this automatically; run it directly with --force to
# rebuild. All versions and hashes are pinned for reproducible APKs.
set -euo pipefail

FLUID_VERSION=2.6.1
FLUID_URL="https://github.com/FluidSynth/fluidsynth/archive/refs/tags/v${FLUID_VERSION}.tar.gz"
FLUID_SHA256=3d258a3bf97cc20c59eeebfe62c2432fae88adda74d3ad098681c76e0ebf446b

OBOE_VERSION=1.10.0
OBOE_URL="https://github.com/google/oboe/archive/refs/tags/${OBOE_VERSION}.tar.gz"
OBOE_SHA256=0e4245f8860c4287040a5d76501c588490bcc9cb57614c486c0c201a5dde3e9f

ABI=arm64-v8a
TRIPLE=aarch64-linux-android
API=31

: "${ANDROID_NDK_HOME:?source android/env.sh first (ANDROID_NDK_HOME is unset)}"
: "${PRELUDE_ANDROID_SYSROOT:?source android/env.sh first (PRELUDE_ANDROID_SYSROOT is unset)}"

NDK_TOOLCHAIN="$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/linux-x86_64"
TOOLCHAIN="$ANDROID_NDK_HOME/build/cmake/android.toolchain.cmake"
SYSROOT="$PRELUDE_ANDROID_SYSROOT"
LIBDIR="$SYSROOT/lib/$ABI"
BUILD_ROOT="$(dirname "$PRELUDE_ANDROID_SYSROOT")/fluidsynth-build"

FORCE=0
if [ "${1:-}" = "--force" ]; then
    FORCE=1
fi

if [ "$FORCE" = 0 ] && [ -f "$LIBDIR/libfluidsynth.so" ] \
    && [ -f "$LIBDIR/liboboe.so" ] && [ -f "$LIBDIR/libc++_shared.so" ] \
    && [ -f "$LIBDIR/pkgconfig/fluidsynth.pc" ]; then
    echo "build-fluidsynth: using cached $LIBDIR"
    exit 0
fi

for tool in cmake ninja curl tar sha256sum; do
    command -v "$tool" >/dev/null 2>&1 || {
        echo "build-fluidsynth: missing tool: $tool" >&2
        exit 1
    }
done
[ -f "$TOOLCHAIN" ] || {
    echo "build-fluidsynth: NDK CMake toolchain missing: $TOOLCHAIN" >&2
    exit 1
}

mkdir -p "$BUILD_ROOT/src" "$LIBDIR" "$SYSROOT/include" \
    "$LIBDIR/pkgconfig"

fetch() {
    local url=$1
    local output=$2
    local expected=$3
    if [ ! -f "$output" ]; then
        echo "build-fluidsynth: fetching $url"
        curl -L --fail --retry 3 -o "$output.part" "$url"
        mv "$output.part" "$output"
    fi
    echo "$expected  $output" | sha256sum -c -
}

fetch "$FLUID_URL" "$BUILD_ROOT/src/fluidsynth.tar.gz" "$FLUID_SHA256"
fetch "$OBOE_URL" "$BUILD_ROOT/src/oboe.tar.gz" "$OBOE_SHA256"

rm -rf "$BUILD_ROOT/fluidsynth" "$BUILD_ROOT/oboe" \
    "$BUILD_ROOT/build-oboe" "$BUILD_ROOT/build-fluidsynth"
mkdir -p "$BUILD_ROOT/fluidsynth" "$BUILD_ROOT/oboe"
tar xf "$BUILD_ROOT/src/fluidsynth.tar.gz" -C "$BUILD_ROOT/fluidsynth" --strip-components=1
tar xf "$BUILD_ROOT/src/oboe.tar.gz" -C "$BUILD_ROOT/oboe" --strip-components=1

# ── Oboe ────────────────────────────────────────────────────────────────────
echo "build-fluidsynth: building Oboe $OBOE_VERSION"
cmake -S "$BUILD_ROOT/oboe" -B "$BUILD_ROOT/build-oboe" -G Ninja \
    -DCMAKE_TOOLCHAIN_FILE="$TOOLCHAIN" \
    -DANDROID_ABI="$ABI" \
    -DANDROID_PLATFORM="android-$API" \
    -DANDROID_STL=c++_shared \
    -DCMAKE_BUILD_TYPE=Release \
    -DBUILD_SHARED_LIBS=ON \
    -DCMAKE_SHARED_LINKER_FLAGS="-Wl,-z,max-page-size=16384"
cmake --build "$BUILD_ROOT/build-oboe"

OBOE_SO=$(find "$BUILD_ROOT/build-oboe" -name 'liboboe.so' -type f | head -1)
[ -n "$OBOE_SO" ] || {
    echo "build-fluidsynth: liboboe.so was not produced" >&2
    exit 1
}
cp "$OBOE_SO" "$LIBDIR/liboboe.so"
cp -R "$BUILD_ROOT/oboe/include/oboe" "$SYSROOT/include/"

cat > "$LIBDIR/pkgconfig/oboe-1.0.pc" <<EOF
prefix=$SYSROOT
exec_prefix=\${prefix}
libdir=\${prefix}/lib/$ABI
includedir=\${prefix}/include

Name: Oboe
Description: Oboe library
Version: $OBOE_VERSION
Libs: -L\${libdir} -loboe -landroid -llog
Cflags: -I\${includedir}
EOF

# ── FluidSynth ──────────────────────────────────────────────────────────────
# GeneralUser GS is a plain SF2 bank: no libsndfile, shell, network, LADSPA or
# Signalsmith reverb is needed. Oboe is the only audio backend.
echo "build-fluidsynth: building FluidSynth $FLUID_VERSION"
export PKG_CONFIG_PATH="$LIBDIR/pkgconfig"
export PKG_CONFIG_LIBDIR="$LIBDIR/pkgconfig"
export PKG_CONFIG_ALLOW_CROSS=1

cmake -S "$BUILD_ROOT/fluidsynth" -B "$BUILD_ROOT/build-fluidsynth" -G Ninja \
    -DCMAKE_TOOLCHAIN_FILE="$TOOLCHAIN" \
    -DANDROID_ABI="$ABI" \
    -DANDROID_PLATFORM="android-$API" \
    -DANDROID_STL=c++_shared \
    -DCMAKE_BUILD_TYPE=Release \
    -DBUILD_SHARED_LIBS=ON \
    -DCMAKE_INSTALL_PREFIX="$SYSROOT" \
    -DCMAKE_STAGING_PREFIX="$SYSROOT" \
    -DLIB_SUFFIX= \
    -Denable-oboe=1 \
    -Denable-opensles=1 \
    -Denable-alsa=0 \
    -Denable-pulseaudio=0 \
    -Denable-pipewire=0 \
    -Denable-jack=0 \
    -Denable-oss=0 \
    -Denable-sdl3=0 \
    -Denable-portaudio=0 \
    -Denable-libsndfile=0 \
    -Denable-aufile=0 \
    -Denable-readline=0 \
    -Denable-dbus=0 \
    -Denable-network=0 \
    -Denable-ladspa=0 \
    -Denable-native-dls=0 \
    -Denable-signalsmith=0 \
    -Denable-openmp=0 \
    -Denable-systemd=0 \
    -Denable-ipv6=0 \
    -DCMAKE_SHARED_LINKER_FLAGS="-Wl,-z,max-page-size=16384" \
    -DCMAKE_EXE_LINKER_FLAGS="-Wl,-z,max-page-size=16384"
cmake --build "$BUILD_ROOT/build-fluidsynth"
cmake --install "$BUILD_ROOT/build-fluidsynth"

# CMake's GNUInstallDirs may install flat into lib/ on Android. Normalize to
# Pixiewood's ABI libdir so cargo's pkg-config and jniLibs both see it.
if [ -f "$SYSROOT/lib/libfluidsynth.so" ] \
    && [ ! -f "$LIBDIR/libfluidsynth.so" ]; then
    cp "$SYSROOT/lib/libfluidsynth.so" "$LIBDIR/libfluidsynth.so"
fi
if [ -d "$SYSROOT/lib/pkgconfig" ]; then
    mkdir -p "$LIBDIR/pkgconfig"
    cp "$SYSROOT/lib/pkgconfig/"*.pc "$LIBDIR/pkgconfig/" 2>/dev/null || true
fi

# ── C++ runtime ─────────────────────────────────────────────────────────────
LIBCXX="$NDK_TOOLCHAIN/sysroot/usr/lib/$TRIPLE/libc++_shared.so"
[ -f "$LIBCXX" ] || {
    echo "build-fluidsynth: missing NDK libc++_shared.so at $LIBCXX" >&2
    exit 1
}
cp "$LIBCXX" "$LIBDIR/libc++_shared.so"

# Keep the APK small; all three are release builds.
for library in libfluidsynth.so liboboe.so libc++_shared.so; do
    "$NDK_TOOLCHAIN/bin/llvm-strip" --strip-unneeded "$LIBDIR/$library"
done

PKG_CONFIG_PATH="$LIBDIR/pkgconfig" PKG_CONFIG_LIBDIR="$LIBDIR/pkgconfig" PKG_CONFIG_ALLOW_CROSS=1 \
    pkg-config --modversion fluidsynth >/dev/null || {
        echo "build-fluidsynth: pkg-config cannot find the target FluidSynth" >&2
        exit 1
    }

echo "build-fluidsynth: installed into $LIBDIR"
