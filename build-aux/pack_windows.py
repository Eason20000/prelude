#!/usr/bin/env python3
"""Stage + zip the Windows prefix layout (ported from build.yml).

Replaces the inline YAML shell so local MSYS2-UCRT64 runs and CI run the
same program: prefix tree (bin/lib/share), DLL harvest via ldd, gresource,
compiled schemas, hicolor + Adwaita icons, gdk-pixbuf loaders + cache
assertions, ICO rendered from the brand SVG, then zip + sha256.

Usage:
    python3 build-aux/pack_windows.py --builddir builddir --source-root . \\
        --stage stage --output prelude-windows-x64.zip [--msys-prefix /ucrt64]
        [--installer]   # also build Inno Setup installer when iscc is present
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
    print(f"pack_windows: {' '.join(cmd)}", flush=True)
    return subprocess.run(cmd, check=True, **kw)


def main(argv: list[str]) -> int:
    p = argparse.ArgumentParser()
    p.add_argument("--builddir", type=Path, required=True)
    p.add_argument("--source-root", type=Path, required=True)
    p.add_argument("--stage", type=Path, required=True)
    p.add_argument("--output", type=Path, required=True)
    p.add_argument("--msys-prefix", type=Path, default=Path("/ucrt64"))
    p.add_argument("--installer", action="store_true")
    p.add_argument("--version", default="")
    args = p.parse_args(argv)

    builddir = args.builddir
    src = args.source_root
    stage = args.stage
    msys = args.msys_prefix
    exe = builddir / "src" / "prelude"
    # cargo names the binary prelude.exe on Windows hosts.
    if not exe.with_suffix(".exe").is_file() and exe.is_file():
        pass

    (stage / "bin").mkdir(parents=True, exist_ok=True)
    (stage / "share" / "prelude").mkdir(parents=True, exist_ok=True)
    (stage / "share" / "glib-2.0" / "schemas").mkdir(parents=True, exist_ok=True)
    (stage / "share" / "icons").mkdir(parents=True, exist_ok=True)
    (stage / "lib" / "gdk-pixbuf-2.0").mkdir(parents=True, exist_ok=True)

    built_exe = builddir / "src" / "prelude.exe"
    if not built_exe.is_file():
        built_exe = builddir / "src" / "prelude"
    shutil.copy(built_exe, stage / "bin" / "prelude.exe")

    ldd = shutil.which("ldd")
    if ldd is None:
        print("pack_windows: ldd not found (run inside MSYS2-UCRT64)",
              file=sys.stderr)
        return 1
    out = subprocess.run([ldd, str(stage / "bin" / "prelude.exe")],
                         capture_output=True, text=True, check=True).stdout
    dlls = sorted(set(re.findall(r"/ucrt64/bin/[^ ]*\\.dll", out)))
    for dll in dlls:
        shutil.copy(msys / "bin" / Path(dll).name, stage / "bin" / Path(dll).name)

    shutil.copy(builddir / "src" / "prelude.gresource",
                stage / "share" / "prelude" / "prelude.gresource")
    # GSchema: prefer the configured (APP_ID-substituted) file from the
    # build tree, fall back to the source template for exotic setups.
    schema_candidates = sorted((builddir / "data").glob("*.gschema.xml"))
    if schema_candidates:
        schema_src = schema_candidates[0]
    else:
        schema_src = src / "data" / "top.vikasmi.Prelude.gschema.xml"
    shutil.copy(schema_src,
                stage / "share" / "glib-2.0" / "schemas" / schema_src.name)
    run(["glib-compile-schemas",
         str(stage / "share" / "glib-2.0" / "schemas")])
    shutil.copytree(src / "data" / "icons" / "hicolor",
                    stage / "share" / "icons" / "hicolor", dirs_exist_ok=True)

    # ICO from the single brand source (branding/ owns the conversion).
    svg = src / "data" / "icons" / "hicolor" / "scalable" / "apps" / "top.vikasmi.Prelude.svg"
    pngs = []
    for s in (16, 24, 32, 48, 64, 128, 256):
        png = Path(f"icon-{s}.png").resolve()
        run(["rsvg-convert", "-w", str(s), "-h", str(s), str(svg),
             "-o", str(png)])
        pngs.append(str(png))
    run([sys.executable, str(src / "branding" / "generate.py"),
         "png-to-ico", *pngs, "-o",
         str((stage / "top.vikasmi.Prelude.ico").resolve())])

    adwaita = msys / "share" / "icons" / "Adwaita"
    if not adwaita.is_dir():
        print("pack_windows: Adwaita icons not installed", file=sys.stderr)
        return 1
    shutil.copytree(adwaita, stage / "share" / "icons" / "Adwaita",
                    dirs_exist_ok=True)
    # gdk-pixbuf loaders: collect + validate relocatability.
    loader_globs = sorted((msys / "lib" / "gdk-pixbuf-2.0").glob("*/loaders"))
    if not loader_globs:
        print("pack_windows: no gdk-pixbuf loaders found", file=sys.stderr)
        return 1
    shutil.copytree(loader_globs[0], stage / "lib" / "gdk-pixbuf-2.0" / "loaders",
                    dirs_exist_ok=True)
    caches = sorted((msys / "lib" / "gdk-pixbuf-2.0").glob("*/loaders.cache"))
    shutil.copy(caches[0], stage / "lib" / "gdk-pixbuf-2.0" / "loaders.cache")
    loaders = list((stage / "lib" / "gdk-pixbuf-2.0" / "loaders").iterdir())
    if not any("svg" in p.name for p in loaders):
        print("pack_windows: no SVG pixbuf loader bundled", file=sys.stderr)
        return 1
    cache_text = (stage / "lib" / "gdk-pixbuf-2.0" / "loaders.cache").read_text()
    if "svg" not in cache_text:
        print("pack_windows: SVG loader missing from cache", file=sys.stderr)
        return 1
    if '"/ucrt64' in cache_text:
        print("pack_windows: loaders.cache is not relocatable", file=sys.stderr)
        return 1

    for f in ("README.md", "LICENSE"):
        if (src / f).is_file():
            shutil.copy(src / f, stage / f)

    # Zip + sha256 (shutil keeps it portable, no pwsh dependency for local).
    base = str(args.output.with_suffix(""))
    if base.endswith(".zip"):
        base = base[: -len(".zip")]
    zip_path = shutil.make_archive(base, "zip", root_dir=stage)
    digest = hashlib.sha256(Path(zip_path).read_bytes()).hexdigest()
    Path(zip_path + ".sha256").write_text(f"{digest}  {Path(zip_path).name}\n")
    print(f"pack_windows: OK -> {zip_path}")

    if args.installer:
        iss_in = src / "build-aux" / "prelude_inno.iss.in"
        iss = Path("prelude_inno.iss")
        text = iss_in.read_text()
        text = text.replace("@VERSION@", args.version or "0.0.0")
        text = text.replace("@STAGE@", stage.resolve().as_posix())
        iss.write_text(text)
        iscc = shutil.which("iscc") or shutil.which("ISCC.exe")
        if iscc is None:
            print("pack_windows: iscc not found, skipping installer "
                  "(install Inno Setup to build it)")
        else:
            run([iscc, str(iss)])
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
