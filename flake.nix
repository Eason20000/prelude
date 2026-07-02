{
  description = "Prelude - A MIDI file player";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    fenix.url = "github:nix-community/fenix";
    fenix.inputs.nixpkgs.follows = "nixpkgs";
  };

  outputs = { self, nixpkgs, fenix }:
    let
      forAllSystems = nixpkgs.lib.genAttrs [ "x86_64-linux" "aarch64-linux" ];
    in
    {
      packages = forAllSystems (system:
        let
          pkgs = import nixpkgs { inherit system; };
          rust = fenix.packages.${system}.stable;
          rustPlatform = pkgs.makeRustPlatform {
            cargo = rust.cargo;
            rustc = rust.rustc;
          };
          rust-toolchain = rust.toolchain;
        in
        rec {
          prelude = rustPlatform.buildRustPackage {
            pname = "prelude";
            version = "0.1.0";
            src = self;

            cargoLock.lockFile = ./Cargo.lock;

            nativeBuildInputs = with pkgs; [
              pkg-config
              wrapGAppsHook4
              autoPatchelfHook
            ];

            buildInputs = with pkgs; [
              gtk4
              libadwaita
              alsa-lib
            ];

            meta = {
              description = "A MIDI file player built with GTK4 and libadwaita";
              license = pkgs.lib.licenses.gpl3Only;
              mainProgram = "prelude";
            };
          };

          default = prelude;
        });

      apps = forAllSystems (system: {
        default = {
          type = "app";
          program = "${self.packages.${system}.default}/bin/prelude";
        };
      });

      devShells = forAllSystems (system:
        let
          pkgs = import nixpkgs { inherit system; };
          rust = fenix.packages.${system}.stable;
        in
        {
          default = pkgs.mkShell {
            packages = with pkgs; [
              gtk4
              libadwaita
              alsa-lib
              pkg-config
              wrapGAppsHook4
            ] ++ [ rust.cargo rust.rustc rust.rustfmt rust.clippy ];
          };
        });
    };
}
