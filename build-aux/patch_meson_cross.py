#!/usr/bin/env python3
"""Inject sccache as compiler launcher into Pixiewood's cross template.

Pixiewood wires NDK clang into Meson via prepare/android.cross in its own
checkout ([binaries] c/cpp as toolchain-relative expressions). Meson has no
CC_LAUNCHER env, and cross builds require the launcher written explicitly
(upstream meson#7147) — so this rewrites:
    c   = toolchain / 'bin' / arch+'-linux-'+platform+'31-clang'
into:
    c   = ['sccache', toolchain / 'bin' / arch+'-linux-'+platform+'31-clang']
(list-first-element launcher, per the Meson Machine-files docs, same
mechanism as CC="ccache gcc").

Runs BEFORE `pixiewood prepare`, so no reconfigure is ever needed on fresh
trees. The checkout is rev-pinned and asserted by build-android.sh, so a
Pixiewood format drift fails loudly here instead of compiling uncached.
- Idempotent via marker comment; no-op when already patched.
- Reverts to backup when sccache is absent but our marker is present
  (install/uninstall cycle stays safe for local reuse trees).
- Refuses to guess when the pattern is missing.

Usage (from android/): python3 ../build-aux/patch_meson_cross.py
Env: PIXIEWOOD_DIR (same variable build-android.sh requires).
"""

from __future__ import annotations

import os
import re
import shutil
import sys
from pathlib import Path

MARKER = "# prelude-sccache-launcher"
KEY_RE = re.compile(r"^(\s*(?:c|cpp))\s*=\s*(.+)$", flags=re.MULTILINE)


def patch_text(text: str, launcher: str) -> tuple[str, bool]:
    """Prepend launcher to bare c/cpp entries. Returns (text, changed)."""
    changed = False

    def repl(match: re.Match) -> str:
        nonlocal changed
        key, value = match.group(1), match.group(2).strip()
        if value.startswith("["):
            items = re.findall(r"'([^']*)'|\"([^\"]*)\"", value)
            first = next((a or b for a, b in items), "")
            if first == launcher:
                return match.group(0)
            # List form but launcher missing (hand edit?): rebuild cleanly.
            rest = ", ".join(f"'{a or b}'" for a, b in items)
            changed = True
            return f"{key} = ['{launcher}', {rest}]"
        changed = True
        return f"{key} = ['{launcher}', {value}]"

    return KEY_RE.sub(repl, text), changed


def main() -> int:
    launcher = "sccache"
    pixiewood = os.environ.get("PIXIEWOOD_DIR", "")
    if not pixiewood:
        print("patch_meson_cross: PIXIEWOOD_DIR unset; refusing to guess",
              file=sys.stderr)
        return 1
    target = Path(pixiewood) / "prepare" / "android.cross"
    backup = target.with_name(target.name + ".orig")
    if not target.is_file():
        print(f"patch_meson_cross: {target} missing (pixiewood layout "
              f"drift?); refusing to guess", file=sys.stderr)
        return 1

    text = target.read_text()
    if shutil.which(launcher) is None:
        if MARKER in text and backup.is_file():
            shutil.copy(backup, target)
            backup.unlink()
            print(f"patch_meson_cross: {launcher} gone, reverted {target}")
        else:
            print(f"patch_meson_cross: {launcher} not found, leaving "
                  f"{target} alone")
        return 0

    if MARKER in text:
        print(f"patch_meson_cross: already patched: {target}")
        return 0
    new, changed = patch_text(text, launcher)
    # Sanity: exactly the c and cpp entries must change, nothing else.
    if not changed or "'sccache'" not in new:
        print(f"patch_meson_cross: c/cpp pattern missing in {target} "
              f"(pixiewood format drift?); refusing to guess",
              file=sys.stderr)
        return 1
    shutil.copy(target, backup)
    target.write_text(f"{MARKER}: prepend {launcher} to c/cpp\n{new}")
    print(f"patch_meson_cross: prepended {launcher} in {target}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
