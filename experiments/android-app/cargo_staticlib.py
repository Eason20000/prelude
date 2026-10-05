"""Build the Prelude Rust core as an Android staticlib.

Meson cannot track cargo's output layout, so this helper runs cargo and
copies the resulting `libprelude.a` to the Meson-declared @OUTPUT@.
The Android cross environment (PKG_CONFIG_*, CC_*, linker wrap, Rust
toolchain) is inherited from the ambient environment (android-env.sh);
only Cargo-specific inputs come from argv.
"""

import os
import shutil
import subprocess
import sys


def main() -> int:
    cargo, manifest, target_dir, triple, out = sys.argv[1:6]
    env = dict(os.environ)
    env["CARGO_TARGET_DIR"] = target_dir
    subprocess.run(
        [cargo, "build", "--target", triple, "--lib",
         "--manifest-path", manifest],
        env=env,
        check=True,
    )
    built = os.path.join(target_dir, triple, "debug", "libprelude.a")
    shutil.copy(built, out)
    return 0


if __name__ == "__main__":
    sys.exit(main())
