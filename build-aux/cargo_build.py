"""Unified Cargo driver for desktop (binary) and Android (staticlib).

Meson cannot track Cargo's output layout or dependency graph, so this
helper is the only imperative seam: it runs ``cargo build`` with an
explicit argv and copies the produced artifact to Meson's ``@OUTPUT@``.
The ambient environment (PKG_CONFIG_*, CC_*, linker wraps, CARGO_TARGET_DIR)
is inherited untouched; only Cargo-specific inputs come from argv.

Desktop:
    cargo_build.py --mode bin --cargo cargo --manifest Cargo.toml \\
        --target-dir DIR --profile release --output @OUTPUT@ [-- APP_ID=...]

Android:
    cargo_build.py --mode lib --cargo cargo --manifest Cargo.toml \\
        --target-dir DIR --triple aarch64-linux-android --profile release \\
        --output @OUTPUT@ [--extra-arg ...]

Profile flag and artifact path derive from the same ``--profile`` value so
the two can never skew (the Meson side passes one string).
"""

from __future__ import annotations

import argparse
import os
import shutil
import subprocess
import sys
from pathlib import Path


def build_argv(args: argparse.Namespace) -> list[str]:
    cmd = [
        args.cargo,
        "build",
        "--manifest-path",
        args.manifest,
    ]
    if args.mode == "lib":
        cmd += ["--lib", "--target", args.triple]
    if args.bin_name:
        cmd += ["--bin", args.bin_name]
    if args.profile == "release":
        cmd.append("--release")
    cmd += args.extra_arg
    return cmd


def produced_artifact(args: argparse.Namespace) -> Path:
    target = Path(args.target_dir)
    if args.mode == "lib":
        if not args.triple:
            raise SystemExit("cargo_build.py: --triple required in lib mode")
        return target / args.triple / args.profile / "libprelude.a"
    name = args.bin_name or "prelude"
    if os.name == "nt" or sys.platform == "win32":
        name += ".exe"
    # Cargo appends .exe on Windows hosts even when cross-reporting; the
    # Meson side passes the same host check, so mirror it here by probing
    # for the suffixed file first when unstated.
    candidate = target / args.profile / name
    if candidate.is_file():
        return candidate
    # Fallback for Windows cross builds reporting without suffix knowledge.
    plain = target / args.profile / name.removesuffix(".exe")
    if plain.is_file():
        return plain
    return candidate


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--mode", choices=["bin", "lib"], required=True)
    parser.add_argument("--cargo", required=True)
    parser.add_argument("--manifest", required=True)
    parser.add_argument("--target-dir", required=True)
    parser.add_argument("--profile", choices=["debug", "release"],
                        default="debug")
    parser.add_argument("--triple", default="")
    parser.add_argument("--bin-name", default="prelude")
    parser.add_argument("--output", required=True)
    parser.add_argument("--extra-arg", action="append", default=[])
    args = parser.parse_args(argv)

    env = dict(os.environ)
    env["CARGO_TARGET_DIR"] = args.target_dir
    cmd = build_argv(args)
    print(f"cargo_build.py: {' '.join(cmd)}", flush=True)
    subprocess.run(cmd, env=env, check=True)
    built = produced_artifact(args)
    if not built.is_file():
        print(f"cargo_build.py: expected artifact missing: {built}",
              file=sys.stderr)
        return 1
    shutil.copy(built, args.output)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
