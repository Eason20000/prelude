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
    version: '0.2.0',
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

# GeneralUser GS is installed as ordinary data. Pixiewood copies non-lib
# install-tree files into APK assets; PreludeActivity materializes the SF2 in
# app-private storage and Rust loads it by path, exactly like desktop.
soundfont = custom_target(
    'generaluser-gs',
    output: 'GeneralUser-GS.sf2',
    command: [
        py3,
        repo_root / 'build-aux/fetch_soundfont.py',
        '--output', '@OUTPUT@',
        '--source-root', repo_root,
    ],
    build_by_default: true,
    install: true,
    install_dir: get_option('datadir') / 'prelude' / 'soundfonts',
    install_tag: 'runtime',
)

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
# Version truth stays Cargo.toml: setup fails loudly on drift (same gate as
# the desktop root meson.build).
cargo_version_check = run_command(
    py3,
    repo_root / 'build-aux/check_version.py',
    '--manifest', repo_root / 'Cargo.toml',
    '--meson-version', meson.project_version(),
    '--android-metainfo', repo_root / 'android/data/top.vikasmi.Prelude.metainfo.xml',
    check: true,
)
android_triple = host_machine.cpu_family() == 'aarch64' ? 'aarch64-linux-android' : 'x86_64-linux-android'
cargo_profile = run_command(
    py3,
    '-c', 'import os; print("release" if os.environ.get("PRELUDE_RELEASE") == "1" else "debug")',
    check: true,
).stdout().strip()
cargo_target_dir = meson.project_build_root() / 'cargo-target'
cargo_build = custom_target(
    'cargo-staticlib',
    # Narrowed always-stale inputs (shared with desktop src/meson.build):
    # Meson still cannot see Cargo's graph, so the target always runs and
    # lets cargo no-op when fresh — no `touch` hack needed in
    # build-android.sh anymore.
    input: files(
        '../Cargo.lock',
        '../Cargo.toml',
        '../build.rs',
        '../src/application.rs',
        '../src/config.rs',
        '../src/engine.rs',
        '../src/lib.rs',
        '../src/main.rs',
        '../src/midi_view.rs',
        '../src/page_view.rs',
        '../src/platform.rs',
        '../src/synth.rs',
    ),
    output: 'libprelude.a',
    depends: gres,
    build_always_stale: true,
    console: true,
    env: [
        'CARGO_TARGET_DIR=' + cargo_target_dir,
        'PRELUDE_GRESOURCE_BUNDLE=' + gres.full_path(),
    ],
    command: [
        py3,
        repo_root / 'build-aux/cargo_build.py',
        '--mode', 'lib',
        '--cargo', cargo_prog.full_path(),
        '--manifest', repo_root / 'Cargo.toml',
        '--target-dir', cargo_target_dir,
        '--triple', android_triple,
        '--profile', cargo_profile,
        '--artifact-name', 'libprelude.a',
        '--output', '@OUTPUT@',
    ],
)

# 4. Final link: C entry + JNI MIDI bridge + whole Rust staticlib, loaded
# by the JVM launcher.
android_sysroot = run_command(
    'sh',
    '-c', 'printf %s "$PRELUDE_ANDROID_SYSROOT"',
    check: true,
).stdout().strip()
assert(
    android_sysroot != '',
    'PRELUDE_ANDROID_SYSROOT must be set (source android/env.sh)',
)
android_libdir = android_sysroot / get_option('libdir')

# FluidSynth, Oboe and libc++ are prebuilt into the Pixiewood sysroot by
# android/build-fluidsynth.sh. Meson cannot install them as project targets,
# so copy them next to libprelude.so for Pixiewood's jniLibs symlink.
meson.add_install_script(
    'copy_android_libs.py',
    android_sysroot,
    get_option('libdir'),
    repo_root / 'data/licenses',
    get_option('datadir') / 'prelude' / 'licenses',
    install_tag: 'runtime',
)

executable(
    'prelude',
    'src/main.c',
    'src/midi_init.c',
    'src/platform_init.c',
    dependencies: [gtk_dep, adw_dep],
    # The Rust staticlib leaves NDK native deps unresolved by design (a
    # staticlib is an unlinked archive); the final link supplies them.
    # midir's Android backend needs libamidi (API 29+).
    # FluidSynth is a shared library so its own C++/Oboe deps stay packaged
    # beside it; libc++_shared is also linked explicitly for the C main shim.
    link_args: [
        cargo_target_dir / android_triple / cargo_profile / 'libprelude.a',
        '-L' + android_libdir,
        '-Wl,-rpath-link,' + android_libdir,
        '-Wl,-z,max-page-size=16384',
        '-lfluidsynth',
        '-lc++_shared',
        '-lamidi',
    ],
    # Ordering only: the .a above is produced by cargo, opaque to Meson.
    link_depends: cargo_build,
    android_exe_type: 'application',
    install: true,
)
