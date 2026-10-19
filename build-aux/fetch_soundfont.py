#!/usr/bin/env python3
"""Fetch the pinned GeneralUser GS SoundFont into the build tree.

The file is intentionally not committed: it is 32 MB and changes rarely.
Nix passes PRELUDE_SF2 so its sandbox build stays offline; Flatpak and CI use
the same script (or a manifest source) with the pinned URL and hash.
"""

from __future__ import annotations

import argparse
import hashlib
import os
import shutil
import sys
import tempfile
import urllib.request
from pathlib import Path

GUGS_VERSION = "2.0.3"
GUGS_COMMIT = "684543d5e5efaef08d02be50dcda8d552478fa60"
GUGS_URL = (
    "https://raw.githubusercontent.com/mrbumpy409/GeneralUser-GS/"
    f"{GUGS_COMMIT}/GeneralUser-GS.sf2"
)
GUGS_SHA256 = "9575028c7a1f589f5770fccc8cff2734566af40cd26ed836944e9a5152688cfe"
GUGS_SIZE = 32319396


def digest(path: Path) -> str:
    hasher = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            hasher.update(chunk)
    return hasher.hexdigest()


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--source-root", type=Path, default=Path.cwd())
    args = parser.parse_args(argv)

    output = args.output
    output.parent.mkdir(parents=True, exist_ok=True)

    override = os.environ.get("PRELUDE_SF2")
    if override:
        source = Path(override)
        if not source.is_absolute():
            source = args.source_root / source
        if not source.is_file():
            print(f"fetch_soundfont: PRELUDE_SF2 is not a file: {source}", file=sys.stderr)
            return 1
        shutil.copyfile(source, output)
    else:
        print(f"fetch_soundfont: downloading GeneralUser GS {GUGS_VERSION}", flush=True)
        with tempfile.NamedTemporaryFile(
            dir=output.parent, prefix="GeneralUser-GS.", suffix=".part", delete=False
        ) as handle:
            temporary = Path(handle.name)
            with urllib.request.urlopen(GUGS_URL) as response:
                shutil.copyfileobj(response, handle)
        os.replace(temporary, output)

    size = output.stat().st_size
    if size != GUGS_SIZE:
        print(
            f"fetch_soundfont: expected {GUGS_SIZE} bytes, got {size}",
            file=sys.stderr,
        )
        return 1
    actual = digest(output)
    if actual != GUGS_SHA256:
        print(
            "fetch_soundfont: SHA256 mismatch\n"
            f"  expected {GUGS_SHA256}\n  actual   {actual}",
            file=sys.stderr,
        )
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
