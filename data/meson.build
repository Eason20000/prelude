# All installed metadata derives from the single APP_ID/VERSION defined in
# the root meson.build via configure_file; nothing here is hand-versioned.
# i18n skeleton: desktop/metainfo go through i18n.merge_file so a future po/
# can translate them without restructuring (requires po/meson.build).
conf = configuration_data()
conf.set('APP_ID', application_id)
conf.set('VERSION', version)
conf.set('RELEASE_DATE', release_date)
conf.set('BINDIR', bindir)

desktop_template = configure_file(
    input: 'top.vikasmi.Prelude.desktop.in',
    output: '@0@.desktop.in'.format(application_id),
    configuration: conf,
)
desktop_out = i18n.merge_file(
    input: desktop_template,
    output: '@0@.desktop'.format(application_id),
    po_dir: meson.project_source_root() / 'po',
    type: 'desktop',
    install: true,
    install_dir: datadir / 'applications',
)

metainfo_template = configure_file(
    input: 'top.vikasmi.Prelude.metainfo.xml.in',
    output: '@0@.metainfo.xml.in'.format(application_id),
    configuration: conf,
)
metainfo_out = i18n.merge_file(
    input: metainfo_template,
    output: '@0@.metainfo.xml'.format(application_id),
    po_dir: meson.project_source_root() / 'po',
    type: 'xml',
    install: true,
    install_dir: datadir / 'metainfo',
)

gschema_out = configure_file(
    input: 'top.vikasmi.Prelude.gschema.xml.in',
    output: '@0@.gschema.xml'.format(application_id),
    configuration: conf,
    install: true,
    install_dir: datadir / 'glib-2.0' / 'schemas',
)

service_out = configure_file(
    input: 'top.vikasmi.Prelude.service.in',
    output: '@0@.service'.format(application_id),
    configuration: conf,
    install: true,
    install_dir: datadir / 'dbus-1' / 'services',
)

# Linux-only integration files stay out of Windows/macOS bundles.
if host_machine.system() == 'linux'
    subdir('icons')
endif
