# Bundled icons (GResource fallback, all platforms)

Compiled into `prelude.gresource` under `icons/scalable/<context>/` and
registered via `IconTheme::add_resource_path` on every platform. Installs with a
system icon theme keep resolving the same names from that theme; platforms
without one (Android, portable trees) fall back to this bundle.

Sources: Adwaita icon theme (CC-BY-SA-3.0), except
`media-playback-pause-symbolic.svg`, which is drawn in the same style to
complete the transport set, and `apps/top.vikasmi.Prelude.svg`, which is a
generated copy of the single brand source (see `branding/`) for the About
dialog.

The set must exactly cover every `icon-name` used in `ui/*.blp` and every
`application_icon` / `*-symbolic` literal in `src/*.rs` — enforced by
`branding/generate.py check-icons`. When adding an icon, drop the SVG here, list
it in `src/prelude.gresource.xml`, and re-run the check.
