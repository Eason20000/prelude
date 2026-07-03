{
  stdenv,
  fenix,
  rustPlatform,
  lib,
  glib,
  shared-mime-info,
  alsa-lib,
  pkg-config,
  windowsDeps,
  windowsTarget ? "x86_64-pc-windows-gnu",
  mingwLinker,
  gcc,
  src,
}:

let
  rustWithWindows = fenix.packages.x86_64-linux.combine [
    fenix.packages.x86_64-linux.stable.cargo
    fenix.packages.x86_64-linux.stable.rustc
    fenix.packages.x86_64-linux.targets.${windowsTarget}.stable.rust-std
  ];

  cargoDeps = rustPlatform.importCargoLock { lockFile = src + /Cargo.lock; };
in
stdenv.mkDerivation {
  pname = "prelude";
  version = "0.1.0";
  inherit src;

  nativeBuildInputs = [
    rustWithWindows
    pkg-config
    alsa-lib.dev
  ];

  depsBuildBuild = [
    gcc
  ];

  CARGO_BUILD_TARGET = windowsTarget;
  PKG_CONFIG_PATH = "${alsa-lib.dev}/lib/pkgconfig:${windowsDeps}/ucrt64/lib/pkgconfig";

  configurePhase = ''
      runHook preConfigure
      export HOME=$TMPDIR

      mkdir -p .cargo
      cat > .cargo/config.toml << EOF
    [source.crates-io]
    replace-with = "vendored-sources"

    [source.vendored-sources]
    directory = "${cargoDeps}"

    [target.${windowsTarget}]
    linker = "${mingwLinker}"
    rustflags = ["-L", "${windowsDeps}/ucrt64/lib"]
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

      mkdir -p $out/bin
      cp target/${windowsTarget}/release/prelude.exe $out/bin/
      cp ${windowsDeps}/ucrt64/bin/*.dll $out/bin/ 2>/dev/null || true

      mkdir -p $out/lib/gdk-pixbuf-2.0/2.10.0/loaders
      cp ${windowsDeps}/ucrt64/lib/gdk-pixbuf-2.0/2.10.0/loaders/*.dll \
        $out/lib/gdk-pixbuf-2.0/2.10.0/loaders/ 2>/dev/null || true

      mkdir -p $out/share
      cp -r ${windowsDeps}/ucrt64/share/glib-2.0 $out/share/ 2>/dev/null || true
      cp -r ${windowsDeps}/ucrt64/share/icons $out/share/ 2>/dev/null || true
      cp -r ${windowsDeps}/ucrt64/share/mime $out/share/ 2>/dev/null || true
      cp -r ${windowsDeps}/ucrt64/share/gtk-4.0 $out/share/ 2>/dev/null || true
      cp -r ${windowsDeps}/ucrt64/share/fontconfig $out/share/ 2>/dev/null || true

      chmod -R u+w $out/share/glib-2.0/schemas/ 2>/dev/null || true
      chmod -R u+w $out/share/mime/ 2>/dev/null || true

      ${glib.dev}/bin/glib-compile-schemas $out/share/glib-2.0/schemas/

      ${shared-mime-info}/bin/update-mime-database $out/share/mime/

      mkdir -p $out/etc/gtk-4.0
      cat > $out/etc/gtk-4.0/settings.ini << GTKEOF
    [Settings]
    gtk-theme-name=Default
    gtk-font-name=Segoe UI 9
    GTKEOF

      rm -rf $out/share/doc $out/share/man $out/share/info \
        $out/share/licenses $out/share/locale $out/share/aclocal \
        $out/share/gir-1.0 $out/share/gtk-doc $out/share/bash-completion \
        $out/share/applications $out/share/metainfo $out/share/pkgconfig \
        $out/share/gettext $out/share/graphite2 $out/share/libthai 2>/dev/null || true

      runHook postInstall
  '';

  meta = {
    description = "Prelude MIDI player (Windows cross-compiled)";
    license = lib.licenses.gpl3Only;
    platforms = [ "x86_64-linux" ];
  };
}
