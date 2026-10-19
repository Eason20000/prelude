fn main() {
    let mut config = pkg_config::Config::new();
    config.atleast_version("2.4");
    if let Err(error) = config.probe("fluidsynth") {
        panic!("libfluidsynth >= 2.4 is required: {error}");
    }
    println!("cargo:rerun-if-changed=build.rs");
}
