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
# the target dir and forwards build metadata (APP_ID/VERSION/...) via the
# environment, read in src/config.rs with option_env!().
if get_option('buildtype') == 'release'
    cargo_profile = 'release'
else
    cargo_profile = 'debug'
endif

# Narrowed always-stale: Meson still cannot see Cargo's dep graph, so the
# target always runs and lets cargo no-op when fresh — but listing the
# load-bearing inputs keeps renames/deletions visible in introspection
# instead of a blind unconditional rebuild.
cargo_inputs = files(
    '../Cargo.lock',
    '../Cargo.toml',
    'application.rs',
    'config.rs',
    'engine.rs',
    'lib.rs',
    'main.rs',
    'midi_view.rs',
    'page_view.rs',
)

cargo_bin = custom_target(
    'cargo-build',
    input: cargo_inputs,
    build_by_default: true,
    build_always_stale: true,
    install: true,
    install_dir: get_option('bindir'),
    output: meson.project_name(),
    console: true,
    depends: resources,
    env: cargo_env,
    command: [
        python3,
        cargo_build_helper,
        '--mode', 'bin',
        '--cargo', cargo.full_path(),
        '--manifest', cargo_manifest,
        '--target-dir', cargo_target_dir,
        '--profile', cargo_profile,
        '--bin-name', meson.project_name(),
        '--output', '@OUTPUT@',
    ],
)
