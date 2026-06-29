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
          mido
          python-rtmidi
          pygobject3
        ];

        nativeBuildInputs = with pkgs; [
          wrapGAppsHook4
          gobject-introspection
        ];

        buildInputs = with pkgs; [
          gtk4
          libadwaita
        ];

        dontWrapGApps = true;

        postInstall = ''
          cp -r ui $out/${pythonPkgs.python.sitePackages}/prelude/
        '';

        preFixup = ''
          makeWrapperArgs+=("''${gappsWrapperArgs[@]}")
        '';
      };

      devShells.default = pkgs.mkShell {
        inputsFrom = [self.packages.${system}.default];
        shellHook = ''
          export PS1="(prelude)$PS1"
        '';
      };
    });
}
