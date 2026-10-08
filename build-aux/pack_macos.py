#!/usr/bin/env python3
"""Assemble the macOS .app + DMG (ported from build.yml).

Ports the macOS packaging shell verbatim: bundle layout, dylibbundler,
gdk-pixbuf loader collection from both Homebrew prefixes (SVG lives in
librsvg's), @rpath rewrite to absolute brew paths, loaders.cache.in
template with @BUNDLE_DIR@, iconset/icns, DMG + sha256.

Usage:
    python3 build-aux/pack_macos.py --builddir builddir --source-root . \\
        --output Prelude-macos-arm64.dmg [--version 0.1.3]
"""

from __future__ import annotations

import argparse
import hashlib
import re
import shutil
import subprocess
import sys
from pathlib import Path


def run(cmd: list[str], **kw) -> subprocess.CompletedProcess:
    print(f"pack_macos: {' '.join(cmd)}", flush=True)
    return subprocess.run(cmd, check=True, **kw)


def brew_prefix(formula: str) -> Path:
    out = subprocess.run(["brew", "--prefix", formula], capture_output=True,
                         text=True, check=True).stdout.strip()
    return Path(out)


def main(argv: list[str]) -> int:
    p = argparse.ArgumentParser()
    p.add_argument("--builddir", type=Path, required=True)
    p.add_argument("--source-root", type=Path, required=True)
    p.add_argument("--output", type=Path, required=True)
    p.add_argument("--version", default="")
    args = p.parse_args(argv)

    builddir = args.builddir
    src = args.source_root
    app = Path("stage/Prelude.app/Contents")
    (app / "MacOS").mkdir(parents=True, exist_ok=True)
    (app / "Resources").mkdir(parents=True, exist_ok=True)
    (app / "Frameworks").mkdir(parents=True, exist_ok=True)
    (app / "Resources" / "share" / "glib-2.0" / "schemas").mkdir(
        parents=True, exist_ok=True)

    shutil.copy(builddir / "src" / "prelude", app / "MacOS" / "prelude-bin")
    shutil.copy(src / "packaging" / "macos" / "Prelude", app / "MacOS" / "prelude")
    (app / "MacOS" / "prelude").chmod(0o755)
    # Info.plist: prefer the configured build-tree file, fall back to source.
    plists = sorted((builddir / "packaging").glob("Info.plist"))
    if not plists:
        plists = sorted((builddir).glob("Info.plist"))
    plist_src = plists[0] if plists else src / "packaging" / "macos" / "Info.plist"
    shutil.copy(plist_src, app / "Info.plist")
    run(["dylibbundler", "-od", "-b", "-x", str(app / "MacOS" / "prelude-bin"),
         "-d", str(app / "Frameworks"), "-p", "@executable_path/../Frameworks"])
    shutil.copy(builddir / "src" / "prelude.gresource", app / "Resources" / "prelude.gresource")
    schemas = sorted((builddir / "data").glob("*.gschema.xml"))
    shutil.copy(schemas[0] if schemas else
                src / "data" / "top.vikasmi.Prelude.gschema.xml",
                app / "Resources" / "share" / "glib-2.0" / "schemas" / "schema.xml")
    if schemas:
        (app / "Resources" / "share" / "glib-2.0" / "schemas" / "schema.xml").rename(
            app / "Resources" / "share" / "glib-2.0" / "schemas" / schemas[0].name)
    run(["glib-compile-schemas",
         str(app / "Resources" / "share" / "glib-2.0" / "schemas")])
    shutil.copytree(src / "data" / "icons" / "hicolor",
                    app / "Resources" / "share" / "icons" / "hicolor",
                    dirs_exist_ok=True)
    shutil.copytree(brew_prefix("adwaita-icon-theme") / "share" / "icons" / "Adwaita",
                    app / "Resources" / "share" / "icons" / "Adwaita",
                    dirs_exist_ok=True)
    loader_dir = app / "Frameworks" / "gdk-pixbuf" / "loaders"
    loader_dir.mkdir(parents=True, exist_ok=True)
    for formula in ("gdk-pixbuf", "librsvg"):
        for pattern in ("lib/gdk-pixbuf-2.0/*/loaders/*.so",):
            for so in sorted((brew_prefix(formula)).glob(pattern)):
                shutil.copy(so, loader_dir / so.name)
    if not any("svg" in p.name for p in loader_dir.iterdir()):
        print("pack_macos: no SVG pixbuf loader bundled", file=sys.stderr)
        return 1
    for loader in sorted(loader_dir.iterdir()):
        loader.chmod(0o644 | 0o200)
        otool = subprocess.run(["otool", "-L", str(loader)],
                               capture_output=True, text=True,
                               check=True).stdout
        for ref in sorted(set(re.findall(r"@rpath/[^ ]*", otool))):
            base = ref.split("/")[-1]
            found = ""
            for d in ("/opt/homebrew/opt/*/lib", "/opt/homebrew/lib"):
                import glob as _glob
                for cand in _glob.glob(f"{d}/{base}"):
                    found = cand
                    break
                if found:
                    break
            if not found:
                print(f"pack_macos: cannot resolve {ref} in {loader}",
                      file=sys.stderr)
                return 1
            print(f"pack_macos: rewriting {ref} -> {found}")
            run(["install_name_tool", "-change", ref, found, str(loader)])
    for loader in sorted(loader_dir.iterdir()):
        run(["dylibbundler", "-of", "-b", "-x", str(loader),
             "-d", str(app / "Frameworks"),
             "-p", "@executable_path/../Frameworks"])
    app_abs = (Path.cwd() / app).resolve()
    loaders = sorted(loader_dir.glob("*"))
    ql = brew_prefix("gdk-pixbuf") / "bin" / "gdk-pixbuf-query-loaders"
    cache = subprocess.run([str(ql), *[str(x) for x in loaders]],
                           capture_output=True, text=True, check=True).stdout
    (app / "Resources" / "loaders.cache.in").write_text(
        cache.replace(str(app_abs), "@BUNDLE_DIR@"))

    iconset = Path("Prelude.iconset")
    iconset.mkdir(exist_ok=True)
    svg = (src / "data" / "icons" / "hicolor" / "scalable" / "apps"
           / "top.vikasmi.Prelude.svg")
    for spec in ("16:icon_16x16", "32:icon_16x16@2x", "32:icon_32x32",
                 "64:icon_32x32@2x", "128:icon_128x128", "256:icon_128x128@2x",
                 "256:icon_256x256", "512:icon_256x256@2x", "512:icon_512x512",
                 "1024:icon_512x512@2x"):
        s, n = spec.split(":")
        run(["rsvg-convert", "-w", s, "-h", s, str(svg),
             "-o", str(iconset / f"{n}.png")])
    run(["iconutil", "-c", "icns", str(iconset), "-o",
         str(app / "Resources" / "Prelude.icns")])
    dmgstage = Path("dmgstage")
    dmgstage.mkdir(exist_ok=True)
    if (dmgstage / "Prelude.app").exists():
        shutil.rmtree(dmgstage / "Prelude.app")
    shutil.copytree(Path("stage/Prelude.app"), dmgstage / "Prelude.app")
    link = dmgstage / "Applications"
    if not link.exists():
        link.symlink_to("/Applications")
    run(["hdiutil", "create", "-volname", "Prelude", "-srcfolder",
         str(dmgstage), "-ov", "-format", "UDZO", str(args.output)])
    digest = hashlib.sha256(args.output.read_bytes()).hexdigest()
    args.output.with_suffix(args.output.suffix + ".sha256").write_text(
        f"{digest}  {args.output.name}\n")
    print(f"pack_macos: OK -> {args.output}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
