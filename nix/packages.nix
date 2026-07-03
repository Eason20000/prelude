{
  rustPlatform,
  src,
  stdenv,
  lib,
  pkg-config,
  wrapGAppsHook4,
  autoPatchelfHook,
  gtk4,
  libadwaita,
  alsa-lib,
}:

rustPlatform.buildRustPackage {
  pname = "prelude";
  version = "0.1.0";
  inherit src;

  cargoLock.lockFile = src + /Cargo.lock;

  nativeBuildInputs = [
    pkg-config
    wrapGAppsHook4
    autoPatchelfHook
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
  };
}
