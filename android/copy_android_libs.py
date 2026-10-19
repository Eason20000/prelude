#!/usr/bin/env python3
"""Install prebuilt FluidSynth/Oboe/libc++ and license texts for Android.

Pixiewood installs the app with `meson install --destdir root --tags runtime`
and symlinks app/src/main/jniLibs to root/lib. Anything placed in the Meson
libdir by this script is therefore packaged into the APK next to
libprelude.so. The license files land in the asset tree."""

from __future__ import annotations

import os
import shutil
import stat
import sys
from pathlib import Path

LIBRARIES = ("libfluidsynth.so", "liboboe.so", "libc++_shared.so")


def main(argv: list[str]) -> int:
    if len(argv) != 5:
        print(
            "usage: copy_android_libs.py <sysroot> <libdir> <licenses-src> <licenses-dest>",
            file=sys.stderr,
        )
        return 2

    sysroot = Path(argv[1])
    libdir = Path(argv[2])
    licenses_src = Path(argv[3])
    licenses_dest = Path(argv[4])
    prefix = os.environ.get("MESON_INSTALL_DESTDIR_PREFIX")
    if not prefix:
        print("copy_android_libs: MESON_INSTALL_DESTDIR_PREFIX is unset", file=sys.stderr)
        return 1

    destination = Path(prefix) / libdir
    destination.mkdir(parents=True, exist_ok=True)

    for name in LIBRARIES:
        source = sysroot / libdir / name
        if not source.is_file():
            print(f"copy_android_libs: missing {source}", file=sys.stderr)
            return 1
        target = destination / name
        shutil.copyfile(source, target)
        target.chmod(target.stat().st_mode | stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH)
        print(f"copy_android_libs: {source} -> {target}")

    license_dir = Path(prefix) / licenses_dest
    license_dir.mkdir(parents=True, exist_ok=True)
    for license_file in sorted(licenses_src.glob("*.txt")):
        shutil.copyfile(license_file, license_dir / license_file.name)
        print(f"copy_android_libs: {license_file} -> {license_dir / license_file.name}")

    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
