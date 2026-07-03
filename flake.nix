{
  description = "Prelude - A MIDI file player";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    fenix.url = "github:nix-community/fenix";
    fenix.inputs.nixpkgs.follows = "nixpkgs";
    treefmt-nix.url = "github:numtide/treefmt-nix";
  };

  outputs =
    {
      self,
      nixpkgs,
      fenix,
      treefmt-nix,
    }:
    let
      forAllSystems = nixpkgs.lib.genAttrs [
        "x86_64-linux"
        "aarch64-linux"
      ];
      treefmtEval = forAllSystems (
        system:
        treefmt-nix.lib.evalModule (import nixpkgs { inherit system; }) {
          projectRootFile = "flake.nix";
          programs.nixfmt.enable = true;
          programs.rustfmt.enable = true;
          programs.rustfmt.edition = "2021";
        }
      );
    in
    {
      formatter = forAllSystems (system: treefmtEval.${system}.config.build.wrapper);
      checks = forAllSystems (system: {
        formatting = treefmtEval.${system}.config.build.check self;
      });

      packages = forAllSystems (
        system:
        let
          pkgs = import nixpkgs { inherit system; };
          rust = fenix.packages.${system}.stable;
          rustPlatform = pkgs.makeRustPlatform {
            cargo = rust.cargo;
            rustc = rust.rustc;
          };
        in
        {
          prelude = pkgs.callPackage ./nix/packages.nix {
            inherit rustPlatform;
            src = self;
          };
          default = self.packages.${system}.prelude;
        }
        // pkgs.lib.optionalAttrs (system == "x86_64-linux") (
          let
            windowsDeps = pkgs.callPackage ./nix/windows-deps.nix {
              mcfgthreads = pkgs.pkgsCross.ucrt64.windows.mcfgthreads;
            };
          in
          {
            prelude-windows = pkgs.callPackage ./nix/windows-build.nix {
              inherit fenix rustPlatform;
              src = self;
              inherit windowsDeps;
              mingwLinker = "${pkgs.pkgsCross.ucrt64.stdenv.cc}/bin/x86_64-w64-mingw32-gcc";
              gcc = pkgs.pkgsCross.ucrt64.buildPackages.gcc;
            };
          }
        )
      );

      apps = forAllSystems (system: {
        default = {
          type = "app";
          program = "${self.packages.${system}.default}/bin/prelude";
        };
      });

      devShells = forAllSystems (
        system:
        let
          pkgs = import nixpkgs { inherit system; };
        in
        {
          default = pkgs.callPackage ./nix/devshell.nix {
            rust = fenix.packages.${system}.stable;
          };
        }
      );
    };
}
