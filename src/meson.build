gnome = import('gnome')

# Bundle the Meson-compiled .ui files (generated into the build root, hence the
# explicit source_dir) into an installed .gresource bundle. ui/icons holds
# app-bundled icons (Adwaita symbolic set + launcher glyph copy, all under
# icons/scalable/<context>/); the bundle is registered as an icon-theme
# fallback on every platform, and the Android staticlib embeds it.
resources = gnome.compile_resources(
    'prelude',
    'prelude.gresource.xml',
    dependencies: blueprints,
    source_dir: [
        meson.current_source_dir(),
        meson.project_build_root(),
        meson.project_source_root() / 'ui',
    ],
    gresource_bundle: true,
    install: true,
    install_dir: pkgdatadir,
)

# Cargo owns all Rust compilation and dependency resolution; Meson only fixes
# the target dir and forwards build metadata (APP_ID) via the environment,
# read in src/config.rs with option_env!().
cargo_build_options = ['--manifest-path', meson.project_source_root() / 'Cargo.toml']
if get_option('buildtype') == 'release'
    cargo_build_options += ['--release']
    rust_target = 'release'
else
    rust_target = 'debug'
endif

cargo_build = custom_target(
    'cargo-build',
    build_by_default: true,
    build_always_stale: true,
    # NOTE: cargo owns its output layout under CARGO_TARGET_DIR, which Meson
    # cannot enumerate; this name is ordering-only. The real artifact is
    # consumed below by absolute path, ordered via depends:.
    output: rust_target,
    console: true,
    depends: resources,
    env: cargo_env,
    command: [cargo, 'build', cargo_build_options],
)

# Cargo appends .exe to binaries on Windows (cf. rnote, NixOS/nix, Vala).
bin_suffix = ''
if host_machine.system() == 'windows'
    bin_suffix = '.exe'
endif

custom_target(
    'cp-binary',
    depends: cargo_build,
    build_by_default: true,
    build_always_stale: true,
    install: true,
    install_dir: get_option('bindir'),
    output: meson.project_name(),
    # Single program, no shell operators: 'cp' does not exist on Windows cmd.
    command: [
        python3,
        '-c', 'import shutil,sys;shutil.copy(sys.argv[1],sys.argv[2])',
        cargo_target_dir / rust_target / (meson.project_name() + bin_suffix),
        '@OUTPUT@',
    ],
)
