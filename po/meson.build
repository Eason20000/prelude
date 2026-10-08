# i18n skeleton (P5): LINGUAS/POTFILES.in are present and data/meson.build
# flips to i18n.merge_file automatically, but the gettext wiring itself
# stays dormant until the first .po lands — so no platform needs
# xgettext/msgfmt on the critical path yet. Generates the template target
# once active: meson compile prelude-pot.
if have_po
    i18n.gettext(
        meson.project_name(),
        preset: 'glib',
        args: '--add-comments',
    )
endif
