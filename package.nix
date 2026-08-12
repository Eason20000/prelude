{
  self,
  lib,
  rustPlatform,
  pkg-config,
  wrapGAppsHook4,
  blueprint-compiler,
  clippy,
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
    blueprint-compiler
    clippy
  ];
  buildInputs = [
    gtk4
    libadwaita
    alsa-lib
  ];

  doCheck = true;

  postCheck = ''
    cargo clippy --profile release --offline -- -D warnings
  '';

  meta = {
    description = "A MIDI file player built with GTK4 and libadwaita";
    license = lib.licenses.gpl3Only;
    mainProgram = "prelude";
    platforms = lib.platforms.linux;
  };
}
