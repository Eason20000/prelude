#!/usr/bin/env python3
"""Single source of truth for the project version.

Reads ``[package] version`` from the repo-root ``Cargo.toml`` (tomllib,
stdlib only) and prints it. Meson calls this at setup time and asserts
``meson.project_version()`` matches, so ``Cargo.toml`` stays the only
hand-edited version while drift fails loudly instead of shipping.

Usage: python3 build-aux/version.py [--manifest PATH]
"""

from __future__ import annotations

import argparse
import sys
import tomllib
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--manifest",
        type=Path,
        default=REPO / "Cargo.toml",
        help="Cargo.toml to read version from",
    )
    args = parser.parse_args(argv)
    try:
        data = tomllib.loads(args.manifest.read_text())
        version = data["package"]["version"]
    except (OSError, KeyError, tomllib.TOMLDecodeError) as e:
        print(f"version.py: cannot read version from {args.manifest}: {e}",
              file=sys.stderr)
        return 1
    if not version:
        print("version.py: empty version", file=sys.stderr)
        return 1
    print(version)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
