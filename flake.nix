{
  description = "Prelude - A MIDI file player";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    treefmt-nix.url = "github:numtide/treefmt-nix";
  };

  outputs =
    {
      self,
      nixpkgs,
      treefmt-nix,
    }:
    let
      system = "x86_64-linux";
      pkgs = nixpkgs.legacyPackages.${system};
      treefmtEval = treefmt-nix.lib.evalModule pkgs ./treefmt.nix;
    in
    {
      formatter.${system} = treefmtEval.config.build.wrapper;
      checks.${system}.formatting = treefmtEval.config.build.check self;
      packages.${system}.default = pkgs.callPackage ./package.nix { inherit self; };
      devShells.${system}.default = pkgs.mkShell {
        inputsFrom = [ self.packages.${system}.default ];
        packages = with pkgs; [
          rustc
          cargo
          rustfmt
          clippy
          # Meson helpers (version.py/cargo_build.py/pack_*.py) and the
          # branding generator run on python3; the build sandbox gets it
          # via meson, but the dev shell needs it explicitly.
          python3
        ];
      };
    };
}
