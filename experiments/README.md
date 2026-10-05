# experiments/ — Android spike area (not part of the desktop build)

- `android-gtk-smoke/`: minimal C GTK4/libadwaita app. Toolchain regression
  probe — if the full app stops packaging, build this first to tell toolchain
  breakage apart from app breakage. `top.vikasmi.PreludeSmoke`, debug only.
- `android-app/`: Prelude itself for Android (see its README for the build).
  Rust core as staticlib + C entry, Pixiewood packaging, PreludeActivity
  subclass for MIDI bootstrap.

Generated dirs (`.pixiewood/`, `subprojects/`, `build*/`, `*.apk`) are
git-ignored; only the hand-written manifests, sources and scripts belong here.
