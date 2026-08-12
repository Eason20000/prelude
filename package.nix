{
  self,
  lib,
  rustPlatform,
  pkg-config,
  wrapGAppsHook4,
  gtk4,
  libadwaita,
  alsa-lib,
}:

rustPlatform.buildRustPackage rec {
  pname = (lib.importTOML (src + "/Cargo.toml")).package.name;
  version = (lib.importTOML (src + "/Cargo.toml")).package.version;

  src = self;
  cargoLock.lockFile = src + "/Cargo.lock";

  nativeBuildInputs = [
    pkg-config
    wrapGAppsHook4
  ];
  buildInputs = [
    gtk4
    libadwaita
    alsa-lib
  ];

  meta = {
    description = "A MIDI file player built with GTK4 and libadwaita";
    license = lib.licenses.gpl3Only;
    mainProgram = "prelude";
    platforms = lib.platforms.linux;
  };
}
