project(
    'prelude',
    'rust',
    version: '0.1.3',
    license: 'GPL-3.0-only',
    meson_version: '>= 1.4',
)

gnome = import('gnome')
i18n = import('i18n')

dependency('gtk4', version: '>= 4.14')
dependency('libadwaita-1', version: '>= 1.8')

cargo = find_program('cargo', required: true)
python3 = find_program('python3', required: true)
blueprint_compiler = find_program('blueprint-compiler', required: true)

# Single source of truth: Cargo.toml [package] version. Meson project()
# cannot run commands, so the literal above must match; the assert below
# fails loudly at setup time instead of shipping drift.
cargo_version = run_command(
    python3,
    meson.project_source_root() / 'build-aux/version.py',
    check: true,
).stdout().strip()
assert(
    cargo_version == meson.project_version(),
    'version drift: meson.build @0@ vs Cargo.toml @1@'.format(
        meson.project_version(),
        cargo_version,
    ),
)
version = meson.project_version()

# App-id: one definition, derived everywhere (desktop/service/schema/icons).
# development profile appends .Devel so dev and release installs coexist
# (Loupe pattern); explicit -Dapp_id_suffix= wins when non-empty.
base_id = 'top.vikasmi.Prelude'
if get_option('app_id_suffix') != ''
    application_id = base_id + get_option('app_id_suffix')
elif get_option('profile') == 'development'
    application_id = base_id + '.Devel'
else
    application_id = base_id
endif

prefix = get_option('prefix')
bindir = prefix / get_option('bindir')
localedir = prefix / get_option('localedir')
datadir = prefix / get_option('datadir')
pkgdatadir = datadir / meson.project_name()
gettext_package = meson.project_name()

# Release date for metainfo <release>: SOURCE_DATE_EPOCH when set
# (reproducible CI), else today. Never hand-edited in data/.
release_date = run_command(
    python3,
    '-c', 'import datetime, os; print(datetime.datetime.fromtimestamp(int(os.environ.get("SOURCE_DATE_EPOCH", "0")) or 0, datetime.timezone.utc).date().isoformat() if os.environ.get("SOURCE_DATE_EPOCH") else datetime.date.today().isoformat())',
    check: true,
).stdout().strip()

cargo_target_dir = meson.project_build_root() / 'cargo-target'
cargo_home = meson.project_build_root() / 'cargo-home'
cargo_manifest = meson.project_source_root() / 'Cargo.toml'
cargo_build_helper = meson.project_source_root() / 'build-aux/cargo_build.py'
cargo_env = [
    'CARGO_HOME=' + cargo_home,
    'CARGO_TARGET_DIR=' + cargo_target_dir,
    'APP_ID=' + application_id,
    'PKGDATADIR=' + pkgdatadir,
    'VERSION=' + version,
    'GETTEXT_PACKAGE=' + gettext_package,
    'LOCALEDIR=' + localedir,
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
subdir('po')
subdir('src')

# macOS bundle plist derived from the same APP_ID/VERSION (pack_macos.py
# consumes the build-tree file, preferring it over the source template).
plist_conf = configuration_data()
plist_conf.set('APP_ID', application_id)
plist_conf.set('VERSION', version)
info_plist = configure_file(
    input: 'packaging/macos/Info.plist.in',
    output: 'Info.plist',
    configuration: plist_conf,
)

# Lint/test entry points shared by local, Nix checkPhase and CI
# (Rnote pattern): one spelling everywhere.
if get_option('tests')
    run_target(
        'cargo-fmt-check',
        command: [
            cargo,
            'fmt',
            '--all',
            '--check',
            '--manifest-path', cargo_manifest,
        ],
        env: cargo_env,
    )
    run_target(
        'cargo-test',
        command: [cargo, 'test', '--offline', '--manifest-path', cargo_manifest],
        env: cargo_env,
    )
    run_target(
        'cargo-clippy',
        command: [
            cargo,
            'clippy',
            '--offline',
            '--manifest-path', cargo_manifest,
            '--',
            '--deny', 'warnings',
        ],
        env: cargo_env,
    )
endif

meson.add_dist_script(
    'build-aux/dist-vendor.sh',
    meson.project_build_root() / 'meson-dist' / meson.project_name()
    + '-'
    + version,
    meson.project_source_root(),
)

# Branding governance as a build target so CI and local share one spelling.
branding_check = run_target(
    'check-branding',
    command: [
        python3,
        meson.project_source_root() / 'branding/generate.py',
        'check',
    ],
)

# Version drift gate for the files Meson cannot generate (Android metainfo
# is consumed by Pixiewood from the source tree). Desktop files are already
# derived via configure_file, so only the Android side needs checking here.
run_command(
    python3,
    meson.project_source_root() / 'build-aux/check_version.py',
    '--manifest', cargo_manifest,
    '--meson-version', version,
    '--android-metainfo', meson.project_source_root() / 'android/data/top.vikasmi.Prelude.metainfo.xml',
    check: true,
)

# Portable packaging entries: local and CI share one spelling
# (meson compile pack-windows/pack-linux/pack-macos). External downloaders
# (linuxdeploy) and platform tools (iscc/dylibbundler) run inside the
# scripts; CI only passes --fetch-external / --version-suffix.
pack_windows = custom_target(
    'pack-windows',
    input: [desktop_out, metainfo_out, gschema_out],
    depends: [cargo_bin, resources],
    build_by_default: false,
    console: true,
    output: 'prelude-windows-x64.zip',
    command: [
        python3,
        meson.project_source_root() / 'build-aux/pack_windows.py',
        '--builddir', meson.project_build_root(),
        '--source-root', meson.project_source_root(),
        '--stage', meson.project_build_root() / 'stage-win',
        '--output', meson.project_build_root() / '@OUTPUT@',
        '--version', version,
    ]
    + (get_option('installer') ? ['--installer'] : []),
)
pack_linux = custom_target(
    'pack-linux',
    input: [desktop_out, metainfo_out, gschema_out],
    depends: [cargo_bin, resources],
    build_by_default: false,
    console: true,
    output: 'Prelude-linux-x64.AppImage',
    command: [
        python3,
        meson.project_source_root() / 'build-aux/pack_linux.py',
        '--builddir', meson.project_build_root(),
        '--source-root', meson.project_source_root(),
        '--output', meson.project_build_root() / '@OUTPUT@',
    ]
    + (get_option('fetch_external') ? ['--fetch-external'] : []),
)
pack_macos = custom_target(
    'pack-macos',
    input: [desktop_out, metainfo_out, gschema_out],
    depends: [cargo_bin, resources],
    build_by_default: false,
    console: true,
    output: 'Prelude-macos-arm64.dmg',
    command: [
        python3,
        meson.project_source_root() / 'build-aux/pack_macos.py',
        '--builddir', meson.project_build_root(),
        '--source-root', meson.project_source_root(),
        '--output', meson.project_build_root() / '@OUTPUT@',
        '--version', version,
    ],
)

meson.add_install_script(
    'build-aux/meson_post_install.py',
    prefix,
)

gnome.post_install(
    gtk_update_icon_cache: true,
    glib_compile_schemas: true,
    update_desktop_database: true,
)
