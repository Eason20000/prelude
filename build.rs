use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=ui/window.blp");

    let output = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR not set")).join("window.ui");

    let status = Command::new("blueprint-compiler")
        .args(["compile", "--output"])
        .arg(&output)
        .arg("ui/window.blp")
        .status()
        .unwrap_or_else(|e| {
            panic!(
                "failed to run blueprint-compiler ({e}); \
                 install it via `nix develop` or add it to nativeBuildInputs"
            )
        });

    assert!(
        status.success(),
        "blueprint-compiler failed to compile ui/window.blp"
    );
}
