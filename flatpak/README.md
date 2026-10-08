# Flatpak (Flathub-ready skeleton, P5)

Manifest `top.vikasmi.Prelude.yml` follows the standard GNOME Rust pattern
(Meson buildsystem + `rust-stable` SDK extension + offline vendored sources).
Not yet submitted to Flathub; the desktop release train stays Nix + AppImage /
zip / dmg + APK.

## Regenerate vendored sources

```bash
# From the repo root, after any Cargo.lock change:
python3 flatpak/flatpak-cargo-generator.py Cargo.lock \
  -o flatpak/cargo-sources.json
```

(`flatpak-cargo-generator.py` is
`flatpak/flatpak-builder-tools/cargo/flatpak-cargo-generator.py` upstream;
vendored copy intentionally not committed — fetch it when cutting a release.)

Alternatively ship the `meson dist` tarball, which already embeds `vendor/` via
`build-aux/dist-vendor.sh`, and point the manifest `sources` at the release
archive instead of `cargo-sources.json`.

## Local build

```bash
flatpak-builder --user --install-deps-from flathub --force-clean \
  flatpak-build-dir flatpak/top.vikasmi.Prelude.yml
```
