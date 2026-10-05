"""Build the Prelude Rust core as an Android staticlib.

Meson cannot track cargo's output layout, so this helper runs cargo and
copies the resulting `libprelude.a` to the Meson-declared @OUTPUT@.
The Android cross environment (PKG_CONFIG_*, CC_*, linker wrap, Rust
toolchain) is inherited from the ambient environment (android-env.sh);
only Cargo-specific inputs come from argv.

argv: cargo, manifest, target-dir, triple, profile-dir, out, [extra-args...]
The profile dir and any extra cargo flags (e.g. --release) are decided by
meson.build from PRELUDE_RELEASE, so the two can never skew.
"""

import os
import shutil
import subprocess
import sys


def main() -> int:
    cargo, manifest, target_dir, triple, profile_dir, out, *extra_args = \
        sys.argv[1:]
    env = dict(os.environ)
    env["CARGO_TARGET_DIR"] = target_dir
    subprocess.run(
        [cargo, "build", "--lib", "--target", triple,
         "--manifest-path", manifest] + extra_args,
        env=env,
        check=True,
    )
    built = os.path.join(target_dir, triple, profile_dir, "libprelude.a")
    shutil.copy(built, out)
    return 0


if __name__ == "__main__":
    sys.exit(main())
