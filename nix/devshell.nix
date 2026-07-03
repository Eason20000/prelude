{
  rust,
  mkShell,
  gtk4,
  libadwaita,
  alsa-lib,
  pkg-config,
  wrapGAppsHook4,
}:

mkShell {
  packages = [
    gtk4
    libadwaita
    alsa-lib
    pkg-config
    wrapGAppsHook4
    rust.cargo
    rust.rustc
    rust.rustfmt
    rust.clippy
  ];
}
