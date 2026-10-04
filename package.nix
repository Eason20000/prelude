# Meson is the primary build system (see meson.build); Nix only provides the
# dependencies and drives `meson setup/compile/install`. The Rust side is
# still compiled by Cargo, with vendored crates supplied via `cargoDeps`.
{
  self,
  lib,
  stdenv,
  rustPlatform,
  cargo,
  rustc,
  clippy,
  meson,
  ninja,
  pkg-config,
  wrapGAppsHook4,
  blueprint-compiler,
  desktop-file-utils,
  glib,
  gtk4,
  libadwaita,
  alsa-lib,
}:

let
  cargoToml = lib.importTOML (self + "/Cargo.toml");
in
stdenv.mkDerivation {
  pname = cargoToml.package.name;
  version = cargoToml.package.version;

  src = self;

  cargoDeps = rustPlatform.fetchCargoVendor {
    inherit (cargoToml.package) version;
    pname = cargoToml.package.name;
    src = self;
    hash = "sha256-PU7z4eEIZtuR3NbhVUqnsejpQQyIoSIvl99hc16wFeU=";
  };

  nativeBuildInputs = [
    meson
    ninja
    pkg-config
    wrapGAppsHook4
    blueprint-compiler
    desktop-file-utils
    glib
    gtk4
    rustPlatform.cargoSetupHook
    cargo
    rustc
    clippy
  ];
  buildInputs = [
    gtk4
    libadwaita
    alsa-lib
  ];

  doCheck = true;

  # Meson defaults to a plain (debug) build; ship an optimized binary.
  mesonBuildType = "release";

  checkPhase = ''
    runHook preCheck
    cargo test --offline
    cargo clippy --profile release --offline -- --deny warnings
    runHook postCheck
  '';

  meta = {
    description = "A MIDI file player built with GTK4 and libadwaita";
    license = lib.licenses.gpl3Only;
    mainProgram = "prelude";
    platforms = lib.platforms.linux;
  };
}
