#!/usr/bin/env python3
"""Meson post-install helper (icons/schemas/desktop-db refresh).

Invoked via meson.add_install_script so `meson install --destdir` (packaging,
Flatpak, distro builds) refreshes caches against the staged DESTDIR instead
of the host. Replaces scattered shell in CI with one testable program.
"""

from __future__ import annotations

import os
import shutil
import subprocess
import sys


def run(prog: str, *args: str) -> None:
    exe = shutil.which(prog)
    if exe is None:
        print(f"post_install: {prog} not found, skipping")
        return
    print(f"post_install: {prog} {' '.join(args)}", flush=True)
    subprocess.run([exe, *args], check=False)


def main() -> int:
    destdir = os.environ.get("DESTDIR", "")
    prefix = sys.argv[1] if len(sys.argv) > 1 else "/usr/local"
    datadir = os.path.join(destdir + prefix, "share")
    run("glib-compile-schemas", os.path.join(datadir, "glib-2.0", "schemas"))
    run("gtk-update-icon-cache", "-q", "-t", "-f",
        os.path.join(datadir, "icons", "hicolor"))
    run("update-desktop-database", "-q",
        os.path.join(datadir, "applications"))
    return 0


if __name__ == "__main__":
    sys.exit(main())
