# Branding (placeholder phase)

No final artwork exists yet. Until it lands, every launcher asset below is a
deliberate placeholder governed as follows.

## Single source

`data/icons/hicolor/scalable/apps/top.vikasmi.Prelude.svg` (brand blue `#3584e4`
\+ white note) is the only hand-drawn file.

Derived files are generated, never hand-edited:

- `data/icons/hicolor/symbolic/apps/top.vikasmi.Prelude-symbolic.svg`
- `android/data/ic_launcher_foreground.xml` (adaptive-icon foreground;
  background `#3584e4` in `android/pixiewood.xml`)

Regenerate / verify with:

```bash
python3 branding/generate.py write
python3 branding/generate.py check
```

CI runs `check` on Linux/macOS plus `desktop-file-validate`.

## Per-surface wiring (all placeholder, none blank)

- Freedesktop: `data/top.vikasmi.Prelude.desktop` +
  `data/top.vikasmi.Prelude.metainfo.xml` (installed to `applications/` +
  `metainfo/`).
- Android: `android/data/top.vikasmi.Prelude.metainfo.xml` stays minimal for
  Pixiewood; display content mirrors the desktop metainfo. The package name is
  lowercase `top.vikasmi.prelude` by platform convention.
- Linux AppImage: PNG rendered from the source in CI.
- macOS: `Prelude.icns` rendered from the source in CI; `Info.plist` versions
  are asserted by `android/build-android.sh`.
- Windows: `top.vikasmi.Prelude.ico` shipped in the zip, rendered from the
  source in CI. Embedding it into the exe would need a `build.rs`, which the
  project forbids, so the exe itself keeps the generic icon for now.
- In-app About dialog uses `application-icon` `top.vikasmi.Prelude`, so it picks
  up the placeholder.

## When the final artwork arrives

Replace the single source SVG, run `write`, and update the background color next
to it. No other file needs design input.
