# Bundled symbolic icons

Fallback for platforms without a system icon theme (Android): compiled into
`prelude.gresource` under `icons/scalable/<context>/` and registered via
`IconTheme::add_resource_path` on Android only. Desktop builds resolve the same
names from the system theme instead.

Sources: Adwaita icon theme (CC-BY-SA-3.0), except
`media-playback-pause-symbolic.svg`, which is drawn in the same style to
complete the transport set.

The set must exactly cover every `icon-name` used in `ui/*.blp` and `src/*.rs` —
enforced by `branding/generate.py check-icons`. When adding an icon, drop the
SVG here, list it in `src/prelude.gresource.xml`, and re-run the check.
