project(
    'prelude-android-smoke',
    'c',
    version: '0.1.0',
    license: 'GPL-3.0-only',
    # `android_exe_type` requires Meson 1.9 (Pixiewood README).
    meson_version: '>= 1.9',
)

gtk_dep = dependency('gtk4')
adw_dep = dependency('libadwaita-1')

metainfo = configure_file(
    input: 'data/top.vikasmi.PreludeSmoke.metainfo.xml',
    output: 'top.vikasmi.PreludeSmoke.metainfo.xml',
    copy: true,
)

executable(
    'prelude-android-smoke',
    'src/main.c',
    dependencies: [gtk_dep, adw_dep],
    android_exe_type: 'application',
    install: true,
)
