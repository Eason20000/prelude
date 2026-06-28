{
  description = "Prelude";

  inputs = {
    utils.url = "github:numtide/flake-utils";
  };

  outputs = {
    self,
    nixpkgs,
    utils,
    ...
  }:
    utils.lib.eachDefaultSystem (system: let
      pkgs = import nixpkgs {inherit system;};
      pythonPkgs = pkgs.python313Packages;
    in {
      packages.default = pythonPkgs.buildPythonPackage {
        pname = "prelude";
        version = "0.1.0";
        format = "pyproject";

        src = ./.;

        allowSubstitutes = false;

        build-system = [ pythonPkgs.hatchling ];

        dependencies = with pythonPkgs; [
          # Python dependencies
          mido
          python-rtmidi
          pygobject3
        ];

        buildInputs = with pkgs; [
          # Non Python dependencies
          gtk4
          libadwaita
          wrapGAppsHook4
        ];
      };

      devShells.default = pkgs.mkShell {
        inputsFrom = [self.packages.${system}.default];
        shellHook = ''
          export PS1="(prelude)$PS1"
        '';
      };
    });
}
