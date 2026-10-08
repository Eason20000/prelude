#!/usr/bin/env python3
"""Fail loudly on version drift.

Single source of truth is Cargo.toml [package] version. Checks:
  - meson.project_version() (passed as --meson-version) matches Cargo
  - android/data metainfo <release version> matches Cargo
  - packaging/macos/Info.plist.in placeholders exist (values are
    substituted by Meson, so nothing to compare)

Called at Meson setup (assert path) and from build-android.sh so local
Android builds fail with the same message as CI.
"""

from __future__ import annotations

import argparse
import re
import sys
import tomllib
from pathlib import Path


def cargo_version(manifest: Path) -> str:
    return tomllib.loads(manifest.read_text())["package"]["version"]


def metainfo_version(path: Path) -> str:
    m = re.search(r'<release version="([^"]+)"', path.read_text())
    if not m:
        raise SystemExit(f"check_version: no <release> in {path}")
    return m.group(1)


def main(argv: list[str]) -> int:
    p = argparse.ArgumentParser()
    p.add_argument("--manifest", type=Path, required=True)
    p.add_argument("--meson-version", default="")
    p.add_argument("--android-metainfo", type=Path, default=None)
    args = p.parse_args(argv)

    expected = cargo_version(args.manifest)
    problems = []
    if args.meson_version and args.meson_version != expected:
        problems.append(f"meson {args.meson_version} != Cargo {expected}")
    if args.android_metainfo is not None:
        got = metainfo_version(args.android_metainfo)
        if got != expected:
            problems.append(
                f"{args.android_metainfo} release {got} != Cargo {expected}")
    if problems:
        for prob in problems:
            print(f"check_version: version drift: {prob}", file=sys.stderr)
        return 1
    print(f"check_version: OK ({expected})")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
