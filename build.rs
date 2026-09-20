use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=ui/window.blp");
    println!("cargo:rerun-if-changed=ui/port_settings.blp");

    // OUT_DIR is always set by cargo; failing means the build is misinvoked.
    #[allow(clippy::expect_used)]
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR not set"));

    for (src, dst) in [
        ("ui/window.blp", "window.ui"),
        ("ui/port_settings.blp", "port_settings.ui"),
    ] {
        let output = out_dir.join(dst);
        let status = Command::new("blueprint-compiler")
            .args(["compile", "--output"])
            .arg(&output)
            .arg(src)
            .status()
            .unwrap_or_else(|e| {
                panic!(
                    "failed to run blueprint-compiler for {src} ({e}); \
                     install it via `nix develop` or add it to nativeBuildInputs"
                )
            });

        assert!(
            status.success(),
            "blueprint-compiler failed to compile {src}"
        );
    }
}
