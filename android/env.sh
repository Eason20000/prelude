#!/usr/bin/env bash
# Android spike environment. Source this before ./build-android.sh:
#   source ./env.sh
#
# Two supported hosts:
# - NixOS: enter a shell with the toolchain first (one-liner in README.md),
#   plus steam-run for the NDK/SDK native binaries.
# - Plain Linux (e.g. CI ubuntu): install the README's apt list instead;
#   NIX-specific globs below then stay empty and harmless.
set -u

: "${ANDROID_HOME:=$HOME/Android/Sdk}"
export ANDROID_HOME
export ANDROID_SDK_ROOT="$ANDROID_HOME"
export ANDROID_NDK_HOME="$ANDROID_HOME/ndk/27.2.12479018"

if [ -z "${JAVA_HOME:-}" ]; then
    if command -v nix >/dev/null 2>&1; then
        JAVA_HOME="$(nix build --no-link --print-out-paths nixpkgs#openjdk17_headless 2>/dev/null | tail -1)"
        export JAVA_HOME
    fi
fi

# Nix perl modules are invisible to @INC by default; glob them in.
# Guarded: no /nix/store outside NixOS, and an unmatched glob must not fail
# under the caller's `set -e`.
PERL_GLOB=""
if [ -d /nix/store ]; then
    PERL_GLOB="$(ls -d /nix/store/*-perl5.42.3-*/lib/perl5/site_perl 2>/dev/null | tr '\n' ':')"
fi
if [ -n "$PERL_GLOB" ]; then
    export PERL5LIB="${PERL5LIB:-}:$PERL_GLOB"
fi

# GObject-Introspection typelibs (new lib/girepository-1.0 layout).
GI_GLOB=""
if [ -d /nix/store ]; then
    GI_GLOB="$(ls -d /nix/store/*/lib/girepository-1.0 2>/dev/null | tr '\n' ':')"
fi
if [ -n "$GI_GLOB" ]; then
    export GI_TYPELIB_PATH="${GI_TYPELIB_PATH:-}:$GI_GLOB"
fi

# Rust-for-Android cross env (aarch64-linux-android, API 31 floor).
# The clang/ar wrappers route NDK binaries through steam-run on NixOS;
# on plain Linux they can point at the NDK binaries directly.
: "${ANDROID_WRAP_DIR:=$HOME/android-work/wrap}"
: "${PRELUDE_ANDROID_SYSROOT:=$HOME/android-work/sysroot-aarch64}"
export PRELUDE_ANDROID_SYSROOT
export PKG_CONFIG_PATH="$PRELUDE_ANDROID_SYSROOT/lib/arm64-v8a/pkgconfig"
# NOTE: PKG_CONFIG_SYSROOT_DIR is deliberately *unset*. The staged .pc files
# point at absolute build-tree paths (see build-android.sh); a sysroot prefix
# would garble them into nonexistence. Nothing in this flow needs it.
export PKG_CONFIG_ALLOW_CROSS=1
export CC_aarch64_linux_android="$ANDROID_WRAP_DIR/clang-aarch64"
export CXX_aarch64_linux_android="$ANDROID_WRAP_DIR/clang-aarch64"
export AR_aarch64_linux_android="$ANDROID_WRAP_DIR/llvm-ar-aarch64"
export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER="$ANDROID_WRAP_DIR/clang-aarch64"

export PATH="$ANDROID_HOME/cmdline-tools/latest/bin:$ANDROID_HOME/platform-tools:$ANDROID_HOME/build-tools/36.0.0:$HOME/.cargo/bin:$HOME/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin:$PATH"

export NIXPKGS_ALLOW_UNFREE=1
