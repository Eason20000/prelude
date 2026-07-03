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
        }
        // pkgs.lib.optionalAttrs (system == "x86_64-linux") (
          let
            rustWithWindows = fenix.packages.x86_64-linux.combine [
              fenix.packages.x86_64-linux.stable.cargo
              fenix.packages.x86_64-linux.stable.rustc
              fenix.packages.x86_64-linux.targets.x86_64-pc-windows-gnu.stable.rust-std
            ];
            rustPlatformWindows = pkgs.makeRustPlatform {
              cargo = rustWithWindows;
              rustc = rustWithWindows;
            };
            msys2Deps = pkgs.callPackage ./nix/windows-deps.nix {
              mcfgthreads = pkgs.pkgsCross.ucrt64.windows.mcfgthreads;
            };
            windowsTarget = "x86_64-pc-windows-gnu";
            mingwLinker = "${pkgs.pkgsCross.ucrt64.stdenv.cc}/bin/x86_64-w64-mingw32-gcc";
          in
          {
            prelude-windows =
              let
                cargoDeps = rustPlatform.importCargoLock { lockFile = ./Cargo.lock; };
              in
              pkgs.stdenv.mkDerivation {
                pname = "prelude";
                version = "0.1.0";
                src = self;

                nativeBuildInputs = [
                  rustWithWindows
                  pkgs.pkg-config
                  pkgs.alsa-lib.dev
                ];

                depsBuildBuild = [
                  pkgs.pkgsCross.ucrt64.buildPackages.gcc
                ];

                CARGO_BUILD_TARGET = windowsTarget;
                PKG_CONFIG_PATH = "${pkgs.alsa-lib.dev}/lib/pkgconfig:${msys2Deps}/ucrt64/lib/pkgconfig";

                configurePhase = ''
                    runHook preConfigure
                    export HOME=$TMPDIR

                    # Set up cargo config for vendored deps and cross-compilation
                    mkdir -p .cargo
                    cat > .cargo/config.toml << EOF
                  [source.crates-io]
                  replace-with = "vendored-sources"

                  [source.vendored-sources]
                  directory = "${cargoDeps}"

                  [target.${windowsTarget}]
                  linker = "${mingwLinker}"
                  rustflags = ["-L", "${msys2Deps}/ucrt64/lib"]
                  EOF
                    runHook postConfigure
                '';

                buildPhase = ''
                  runHook preBuild
                  cargo build --release --target ${windowsTarget} --offline
                  runHook postBuild
                '';

                installPhase = ''
                    runHook preInstall

                    # Install the executable and runtime DLLs
                    mkdir -p $out/bin
                    cp target/${windowsTarget}/release/prelude.exe $out/bin/
                    cp ${msys2Deps}/ucrt64/bin/*.dll $out/bin/ 2>/dev/null || true

                    # Install GdkPixbuf loader modules
                    mkdir -p $out/lib/gdk-pixbuf-2.0/2.10.0/loaders
                    cp ${msys2Deps}/ucrt64/lib/gdk-pixbuf-2.0/2.10.0/loaders/*.dll \
                      $out/lib/gdk-pixbuf-2.0/2.10.0/loaders/ 2>/dev/null || true

                    # Install share data (icons, schemas, mime, gtk data)
                    mkdir -p $out/share
                    cp -r ${msys2Deps}/ucrt64/share/glib-2.0 $out/share/ 2>/dev/null || true
                    cp -r ${msys2Deps}/ucrt64/share/icons $out/share/ 2>/dev/null || true
                    cp -r ${msys2Deps}/ucrt64/share/mime $out/share/ 2>/dev/null || true
                    cp -r ${msys2Deps}/ucrt64/share/gtk-4.0 $out/share/ 2>/dev/null || true
                    cp -r ${msys2Deps}/ucrt64/share/fontconfig $out/share/ 2>/dev/null || true

                    # Make copied files writable for cache generation tools
                    chmod -R u+w $out/share/glib-2.0/schemas/ 2>/dev/null || true
                    chmod -R u+w $out/share/mime/ 2>/dev/null || true

                    # Compile GSettings schemas
                    ${pkgs.glib.dev}/bin/glib-compile-schemas $out/share/glib-2.0/schemas/

                    # Generate GdkPixbuf loader cache
                    # Note: Linux gdk-pixbuf-query-loaders cannot introspect Windows PE .dll files.
                    # GDK-Pixbuf on Windows auto-discovers loaders from the standard directory.
                    # The loader .dll files are already installed in the correct location.

                    # Generate MIME database
                    ${pkgs.shared-mime-info}/bin/update-mime-database $out/share/mime/

                    # GTK4 settings for Windows
                    mkdir -p $out/etc/gtk-4.0
                    cat > $out/etc/gtk-4.0/settings.ini << GTKEOF
                  [Settings]
                  gtk-theme-name=Default
                  gtk-font-name=Segoe UI 9
                  GTKEOF

                    # Strip useless share data to reduce size
                    rm -rf $out/share/doc $out/share/man $out/share/info \
                      $out/share/licenses $out/share/locale $out/share/aclocal \
                      $out/share/gir-1.0 $out/share/gtk-doc $out/share/bash-completion \
                      $out/share/applications $out/share/metainfo $out/share/pkgconfig \
                      $out/share/gettext $out/share/graphite2 $out/share/libthai 2>/dev/null || true

                    runHook postInstall
                '';

                meta = {
                  description = "Prelude MIDI player (Windows cross-compiled)";
                  license = pkgs.lib.licenses.gpl3Only;
                  platforms = [ "x86_64-linux" ];
                };
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
          rust = fenix.packages.${system}.stable;
        in
        {
          default = pkgs.mkShell {
            packages =
              with pkgs;
              [
                gtk4
                libadwaita
                alsa-lib
                pkg-config
                wrapGAppsHook4
              ]
              ++ [
                rust.cargo
                rust.rustc
                rust.rustfmt
                rust.clippy
              ];
          };
        }
      );
    };
}
