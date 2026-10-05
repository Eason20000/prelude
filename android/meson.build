# Android platform shell for Prelude (not part of the desktop
# build). Pixiewood drives this project per-arch (bin-aarch64):
#   pixiewood prepare -s $ANDROID_HOME -t $ANDROID_NDK_HOME pixiewood.xml
#   pixiewood generate && pixiewood build
# Cargo (with the Android cross env from android-env.sh) builds the Rust core
# as a staticlib embedding the GResource bundle; a tiny C `main` forwards to
# it and Meson performs the final link as an Android application `.so`.
project(
    'prelude-android-app',
    'c',
    version: '0.1.3',
    license: 'GPL-3.0-only',
    # `android_exe_type` requires Meson 1.9 (Pixiewood README).
    meson_version: '>= 1.9',
)

gtk_dep = dependency('gtk4')
adw_dep = dependency('libadwaita-1')

blueprint_compiler = find_program('blueprint-compiler')
glib_compile_resources = find_program('glib-compile-resources')
cargo_prog = find_program('cargo')
py3 = find_program('python3')

# android/ sits at the repo root, so one level up is the repo itself.
# Keep this in sync with the directory layout (it used to be two).
repo_root = meson.project_source_root() / '..'

# 1. Blueprint ui/*.blp -> build-root/*.ui (host tools, same as desktop).
window_ui = custom_target(
    'window-ui',
    input: repo_root / 'ui/window.blp',
    output: 'window.ui',
    command: [blueprint_compiler, 'compile', '--output', '@OUTPUT@', '@INPUT@'],
)
port_settings_ui = custom_target(
    'port-settings-ui',
    input: repo_root / 'ui/port_settings.blp',
    output: 'port_settings.ui',
    command: [blueprint_compiler, 'compile', '--output', '@OUTPUT@', '@INPUT@'],
)

# 2. Bundle into prelude.gresource (consumed by cargo via env below).
gres = custom_target(
    'gres',
    input: repo_root / 'src/prelude.gresource.xml',
    output: 'prelude.gresource',
    depends: [window_ui, port_settings_ui],
    command: [
        glib_compile_resources,
        '--sourcedir', meson.project_build_root(),
        '--sourcedir', repo_root / 'ui',
        '--target', '@OUTPUT@',
        '@INPUT@',
    ],
)

# 3. Rust core as staticlib. The arch-specific bits (cargo --target and the
# .a path) follow the Meson host machine Pixiewood configures per build dir.
# The pkg-config sysroot comes from the ambient PRELUDE_ANDROID_SYSROOT env
# (android-env.sh), one sysroot per arch.
#
# Build profile follows PRELUDE_RELEASE (CI sets 1; local default is debug,
# mirroring the desktop convention of debug-by-default). Single source of
# truth: the cargo flag and the .a path below both derive from this one read.
android_triple = host_machine.cpu_family() == 'aarch64' ? 'aarch64-linux-android' : 'x86_64-linux-android'
cargo_profile = run_command(
    py3,
    '-c', 'import os; print("release" if os.environ.get("PRELUDE_RELEASE") == "1" else "debug")',
    check: true,
).stdout().strip()
cargo_extra_args = cargo_profile == 'release' ? ['--release'] : []
cargo_target_dir = meson.project_build_root() / 'cargo-target'
cargo_build = custom_target(
    'cargo-staticlib',
    output: 'libprelude.a',
    depends: gres,
    # Cargo owns incremental rebuild detection; Meson cannot see Rust
    # sources as inputs, so always run and let cargo no-op when fresh.
    build_always_stale: true,
    console: true,
    env: [
        'CARGO_TARGET_DIR=' + cargo_target_dir,
        'PRELUDE_GRESOURCE_BUNDLE=' + gres.full_path(),
    ],
    command: [
        py3,
        files('cargo_staticlib.py'),
        cargo_prog.full_path(),
        repo_root / 'Cargo.toml',
        cargo_target_dir,
        android_triple,
        cargo_profile,
        '@OUTPUT@',
    ]
    + cargo_extra_args,
)

# 4. Final link: C entry + JNI MIDI bridge + whole Rust staticlib, loaded
# by the JVM launcher.
executable(
    'prelude',
    'src/main.c',
    'src/midi_init.c',
    dependencies: [gtk_dep, adw_dep],
    # The Rust staticlib leaves NDK native deps unresolved by design (a
    # staticlib is an unlinked archive); the final link supplies them.
    # midir's Android backend needs libamidi (API 29+).
    # Release profile: cargo_staticlib.py builds --release; keep in sync.
    link_args: [
        cargo_target_dir / android_triple / cargo_profile / 'libprelude.a',
        '-lamidi',
    ],
    # Ordering only: the .a above is produced by cargo, opaque to Meson.
    link_depends: cargo_build,
    android_exe_type: 'application',
    install: true,
)
