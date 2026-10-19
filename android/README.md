# Prelude for Android

Android platform shell for Prelude: same Rust core as desktop, packaged via
[Pixiewood](https://github.com/sp1ritCS/gtk-android-builder) (pinned revision in
`pixiewood.lock`). The desktop build in the repo root is untouched; everything
Android-specific lives in this directory.

## How it works

- Rust core compiles to a staticlib embedding the GResource bundle
  (`src/lib.rs:prelude_android_main`), with a tiny C `main` shim (`src/main.c`)
  that Pixiewood links as the `.so` the JVM launcher loads (Meson
  `android_exe_type: 'application'`, see `meson.build`).
- `PreludeActivity` (`android-java/`, installed by `apply-android-patches.sh`)
  subclasses GTK's activity and hands the application context to ndk-context
  before GTK starts, so midir's Android backend can reach MidiManager.
- `LoadInput`/`ParseSource` (`src/application.rs`): content URIs are opened on
  the main thread and streamed to the parse worker in 1MB chunks (GDK's content
  backend may only run its open step on attached threads).

## Prerequisites

- JDK 17, Android SDK (cmdline-tools, platform-36, build-tools 36.0.0), NDK
  27.2.12479018, Rust stable + `aarch64-linux-android` target.
- Pixiewood checkout + Meson >= 1.9 (we use 1.12.1) + blueprint-compiler + cmake
  \+ ninja (FluidSynth and Oboe are CMake projects).
- NixOS: enter the toolchain shell first (one-liner below); plain Linux:
  `apt install` the equivalents (openjdk-17-jdk, perl + Glib/XML/JSON/Set
  modules, meson, ninja, pkg-config, sassc, glslc, gcc, glib dev tools,
  libxml2-utils, python3, git, blueprint-compiler, appstream,
  gobject-introspection) and skip steam-run (native binaries run as-is).

```bash
# NixOS toolchain shell (also provides steam-run for NDK/SDK binaries):
nix shell --impure nixpkgs#steam-run nixpkgs#perl \
  nixpkgs#perlPackages.Glib nixpkgs#perlPackages.GlibObjectIntrospection \
  nixpkgs#perlPackages.IPCRun nixpkgs#perlPackages.JSON \
  nixpkgs#perlPackages.SetScalar nixpkgs#perlPackages.XMLLibXML \
  nixpkgs#perlPackages.XMLLibXSLT nixpkgs#meson nixpkgs#ninja \
  nixpkgs#pkg-config nixpkgs#cmake nixpkgs#ninja nixpkgs#sassc nixpkgs#shaderc nixpkgs#gcc nixpkgs#glib \
  nixpkgs#glib.dev nixpkgs#libxml2 nixpkgs#python3 nixpkgs#git \
  nixpkgs#blueprint-compiler nixpkgs#appstream \
  nixpkgs#gobject-introspection nixpkgs#openjdk17_headless \
  --command steam-run bash
```

## Build

```bash
cd android
source ./env.sh
./build-android.sh            # prepare → stage → FluidSynth+Oboe → build → verify
adb install -r .pixiewood/android/app/build/outputs/apk/debug/app-arm64-v8a-debug.apk
```

Debug is the default (desktop convention).
`PRELUDE_RELEASE=1 ./build-android.sh` selects release end to end:
fresh-configured runtime (`--strip`), cargo `--release`, Gradle
`assembleRelease`, signed with a throwaway debug keystore (publishing keys are
future work — see TODOS.md). CI always builds release.

`--reprepare` forces a Pixiewood prepare rerun. Versions (`Cargo.toml` single
source of truth vs `meson.project_version()` vs metainfo) are asserted by
`build-aux/check_version.py` — at Meson setup and from `build-android.sh`; bump
`Cargo.toml` only, never the derivatives deliberately.

arm64 only (`pixiewood.xml`): real devices. The x86_64 emulator flow is gone and
its cargo link never had a matching sysroot — one line to bring back.

CI (`.github/workflows/android.yml`) runs this same script on ubuntu-26.04 with
identical triggers to the desktop build; `release.yml` ships its artifact
alongside the desktop ones.

## Constraints

- `generate` rewrites the manifest every run: custom bits must go through
  `apply-android-patches.sh` (idempotent), never hand-edited.
- Meson cannot see Cargo's dep graph: both `meson.build` files declare narrowed
  Rust inputs + `build_always_stale`, so cargo rechecks every build by itself
  (no `touch` hack).
- `src://` metainfo includes need the freedesktop `xmlns` + a `<releases>` entry
  or `generate` dies.
- `build-fluidsynth.sh` caches FluidSynth/Oboe under the sysroot; `--force`
  rebuilds. It installs `libfluidsynth.so`, `liboboe.so`, `libc++_shared.so` and
  the target `fluidsynth.pc`; `copy_android_libs.py` puts the runtime libs next
  to `libprelude.so` for Pixiewood's `jniLibs` symlink.
- The GUGS SF2 is installed as a normal data file; Pixiewood copies it into APK
  assets and `PreludeActivity` materializes it in `filesDir`. Rust then uses the
  same path-based `fluid_synth_sfload()` as desktop.
- Android 15+ requires 16 KB-compatible native code; `build-android.sh` checks
  both ELF LOAD alignment and APK zip alignment.
- The GDK content backend crashes on unattached threads; keep opens on the main
  thread (see `ParseSource`).
- Cargo's `.pc` files come straight from the build tree
  (`meson-uninstalled/*.pc`, renamed): never `meson install` for them — install
  rebuilds every target including our cargo staticlib, which needs those very
  files to link. The staged copies carry absolute build-tree paths, so
  `PKG_CONFIG_SYSROOT_DIR` must stay unset or it garbles them.
- The sysroot `PKG_CONFIG_PATH` must not leak into `pixiewood prepare`: Meson
  resolves build-machine tools through pkg-config, and a foreign path fails
  configure. Only the cargo step consumes it.
