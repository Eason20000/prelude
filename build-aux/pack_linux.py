#!/usr/bin/env python3
"""Assemble the Linux AppImage (ported from build.yml).

Builds AppDir from the Meson build tree, then drives linuxdeploy +
linuxdeploy-plugin-gtk. External binaries are fetched only with
--fetch-external (CI passes it); local runs reuse vendored copies when
present so the build stays offline-friendly.

Usage:
    python3 build-aux/pack_linux.py --builddir builddir --source-root . \\
        --output Prelude-linux-x64.AppImage [--fetch-external]
"""

from __future__ import annotations

import argparse
import hashlib
import os
import shutil
import stat
import subprocess
import sys
import urllib.request
from pathlib import Path

LINUXDEPLOY = ("https://github.com/linuxdeploy/linuxdeploy/releases/"
               "download/continuous/linuxdeploy-x86_64.AppImage")
GTK_PLUGIN = ("https://raw.githubusercontent.com/linuxdeploy/"
              "linuxdeploy-plugin-gtk/master/linuxdeploy-plugin-gtk.sh")


def run(cmd: list[str], **kw) -> None:
    print(f"pack_linux: {' '.join(cmd)}", flush=True)
    subprocess.run(cmd, check=True, **kw)


def fetch(url: str, dst: Path) -> None:
    print(f"pack_linux: fetching {url} -> {dst}", flush=True)
    urllib.request.urlretrieve(url, dst)
    dst.chmod(dst.stat().st_mode | stat.S_IEXEC)


def main(argv: list[str]) -> int:
    p = argparse.ArgumentParser()
    p.add_argument("--builddir", type=Path, required=True)
    p.add_argument("--source-root", type=Path, required=True)
    p.add_argument("--output", type=Path, required=True)
    p.add_argument("--fetch-external", action="store_true")
    args = p.parse_args(argv)

    builddir = args.builddir
    src = args.source_root
    appdir = Path("AppDir")
    (appdir / "usr" / "bin").mkdir(parents=True, exist_ok=True)
    (appdir / "usr" / "share" / "prelude").mkdir(parents=True, exist_ok=True)
    (appdir / "usr" / "share" / "glib-2.0" / "schemas").mkdir(
        parents=True, exist_ok=True)

    shutil.copy(builddir / "src" / "prelude", appdir / "usr" / "bin" / "prelude")
    desktops = sorted((builddir / "data").glob("*.desktop"))
    if not desktops:
        print("pack_linux: no .desktop in builddir/data", file=sys.stderr)
        return 1
    # App-id derives from the generated desktop name, so development
    # (.Devel) builds stay consistent everywhere without a flag.
    app_id = desktops[0].stem
    shutil.copy(desktops[0], appdir / desktops[0].name)
    # Icon is rendered at pack time from the single brand SVG (see
    # branding/generate.py); nothing derived is committed. 512px is the
    # largest resolution linuxdeploy accepts into hicolor.
    icon_png = appdir / f"{app_id}.png"
    run(["rsvg-convert", "-w", "512", "-h", "512",
         str(src / "data" / "icons" / "hicolor" / "scalable" / "apps"
             / "top.vikasmi.Prelude.svg"),
         "-o", str(icon_png)])
    shutil.copy(src / "packaging" / "appimage" / "AppRun", appdir / "AppRun")
    shutil.copy(builddir / "src" / "prelude.gresource",
                appdir / "usr" / "share" / "prelude" / "prelude.gresource")
    schemas = sorted((builddir / "data").glob("*.gschema.xml"))
    if not schemas:
        print("pack_linux: no .gschema.xml in builddir/data", file=sys.stderr)
        return 1
    shutil.copy(schemas[0],
                appdir / "usr" / "share" / "glib-2.0" / "schemas"
                / schemas[0].name)
    # Upstream metadata: silences appimagetool's AppStream warning and ships
    # real store metadata inside the AppImage.
    metainfos = sorted((builddir / "data").glob("*.metainfo.xml"))
    if metainfos:
        metainfo_dir = appdir / "usr" / "share" / "metainfo"
        metainfo_dir.mkdir(parents=True, exist_ok=True)
        shutil.copy(metainfos[0], metainfo_dir / metainfos[0].name)
    run(["glib-compile-schemas",
         str(appdir / "usr" / "share" / "glib-2.0" / "schemas")])
    for b in (appdir / "AppRun", appdir / "usr" / "bin" / "prelude"):
        b.chmod(b.stat().st_mode | stat.S_IEXEC)

    ld = Path("linuxdeploy-x86_64.AppImage")
    plugin = Path("linuxdeploy-plugin-gtk.sh")
    if args.fetch_external:
        if not ld.is_file():
            fetch(LINUXDEPLOY, ld)
        if not plugin.is_file():
            fetch(GTK_PLUGIN, plugin)
            plugin.chmod(plugin.stat().st_mode | stat.S_IEXEC)
    if not ld.is_file() or not plugin.is_file():
        print("pack_linux: linuxdeploy binaries missing "
              "(pass --fetch-external)", file=sys.stderr)
        return 1

    env = dict(os.environ)
    env["PATH"] = f"{Path.cwd()}{os.pathsep}{env.get('PATH', '')}"
    env["APPIMAGE_EXTRACT_AND_RUN"] = "1"
    # --desktop-file/--icon-file are what INSTALL the files linuxdeploy then
    # links into the AppDir root; hand-dropping them (done above, still
    # needed for the custom AppRun) is not recognized on its own.
    run([str(ld), "--appdir", "AppDir",
         "--desktop-file", str(appdir / desktops[0].name),
         "--icon-file", str(icon_png),
         "--icon-filename", app_id,
         "--plugin", "gtk",
         "--output", "appimage"], env=env)
    produced = sorted(Path(".").glob("Prelude-x86_64.AppImage"))
    if not produced:
        print("pack_linux: linuxdeploy produced no AppImage", file=sys.stderr)
        return 1
    shutil.move(str(produced[0]), str(args.output))
    digest = hashlib.sha256(args.output.read_bytes()).hexdigest()
    args.output.with_suffix(args.output.suffix + ".sha256").write_text(
        f"{digest}  {args.output.name}\n")
    print(f"pack_linux: OK -> {args.output}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
