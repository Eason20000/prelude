project(
    'prelude',
    'rust',
    version: '0.1.0',
    license: 'GPL-3.0-only',
    meson_version: '>= 1.0.0',
)

gnome = import('gnome')

dependency('gtk4', version: '>= 4.14')
dependency('libadwaita-1', version: '>= 1.8')

cargo = find_program('cargo', required: true)
python3 = find_program('python3', required: true)
blueprint_compiler = find_program('blueprint-compiler', required: true)

prefix = get_option('prefix')
bindir = prefix / get_option('bindir')
pkgdatadir = prefix / get_option('datadir') / meson.project_name()
application_id = 'top.vikasmi.Prelude'

cargo_target_dir = meson.project_build_root() / 'cargo-target'
cargo_home = meson.project_build_root() / 'cargo-home'
cargo_env = [
    'CARGO_HOME=' + cargo_home,
    'CARGO_TARGET_DIR=' + cargo_target_dir,
    'APP_ID=' + application_id,
    'PKGDATADIR=' + pkgdatadir,
]

# Blueprint ui/*.blp -> build-root/*.ui. One explicit single-output target per
# file keeps ninja dependencies precise; the generated files land in the build
# root, where src/meson.build picks them up via source_dir.
window_ui = custom_target(
    'window-ui',
    input: files('ui/window.blp'),
    output: 'window.ui',
    command: [blueprint_compiler, 'compile', '--output', '@OUTPUT@', '@INPUT@'],
)
port_settings_ui = custom_target(
    'port-settings-ui',
    input: files('ui/port_settings.blp'),
    output: 'port_settings.ui',
    command: [blueprint_compiler, 'compile', '--output', '@OUTPUT@', '@INPUT@'],
)
blueprints = [window_ui, port_settings_ui]

subdir('data')
subdir('src')

gnome.post_install(
    gtk_update_icon_cache: true,
    glib_compile_schemas: true,
    update_desktop_database: true,
)
