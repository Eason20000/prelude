//! Build metadata.
//!
//! Under Meson, `APP_ID`/`PKGDATADIR`/`VERSION`/`GETTEXT_PACKAGE`/`LOCALEDIR`
//! are injected through the environment (see the `cargo_env` list in the root
//! `meson.build`); a plain `cargo build` falls back to the defaults below
//! (which match a default `meson install` with no `--prefix`).

use std::path::PathBuf;

#[cfg(target_os = "android")]
use std::sync::OnceLock;

const APP_ID_ENV: Option<&str> = option_env!("APP_ID");
// Desktop-only: Android embeds the bundle (see `EMBEDDED_GRESOURCE` in
// lib.rs) and never resolves install-tree paths.
#[cfg(not(target_os = "android"))]
const PKGDATADIR_ENV: Option<&str> = option_env!("PKGDATADIR");
const VERSION_ENV: Option<&str> = option_env!("VERSION");
const GETTEXT_PACKAGE_ENV: Option<&str> = option_env!("GETTEXT_PACKAGE");
const LOCALEDIR_ENV: Option<&str> = option_env!("LOCALEDIR");

pub(crate) fn app_id() -> &'static str {
    match APP_ID_ENV {
        Some(id) => id,
        None => "top.vikasmi.Prelude",
    }
}

pub(crate) fn version() -> &'static str {
    match VERSION_ENV {
        Some(v) => v,
        None => env!("CARGO_PKG_VERSION"),
    }
}

// i18n skeleton (P5): no gettext-rs dependency yet; the first .po wires
// these into the startup init. Allowed dead until then so the Meson
// GETTEXT_PACKAGE/LOCALEDIR plumbing stays exercised.
#[allow(dead_code)]
pub(crate) fn gettext_package() -> &'static str {
    match GETTEXT_PACKAGE_ENV {
        Some(p) => p,
        None => env!("CARGO_PKG_NAME"),
    }
}

#[cfg(not(target_os = "android"))]
#[allow(dead_code)]
pub(crate) fn localedir() -> &'static str {
    match LOCALEDIR_ENV {
        Some(d) => d,
        None => "/usr/local/share/locale",
    }
}

#[cfg(not(target_os = "android"))]
pub(crate) fn pkgdatadir() -> &'static str {
    match PKGDATADIR_ENV {
        Some(dir) => dir,
        None => "/usr/local/share/prelude",
    }
}

/// Locate `prelude.gresource`, preferring paths relative to the running
/// executable so portable trees (Linux AppDir, Windows zip, macOS .app)
/// work without installing to the build-time prefix. Falls back to the
/// Meson-baked `pkgdatadir()` above (which keeps the previous behavior of
/// failing loudly when nothing is found).
#[cfg(not(target_os = "android"))]
pub(crate) fn gresource_path() -> PathBuf {
    const FILE: &str = "prelude.gresource";

    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Ok(dir) = std::env::var("PRELUDE_DATADIR") {
        candidates.push(PathBuf::from(dir).join(FILE));
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.join("../share/prelude").join(FILE));
            candidates.push(dir.join("../Resources").join(FILE));
            candidates.push(dir.join(FILE));
        }
    }
    candidates.push(PathBuf::from(pkgdatadir()).join(FILE));

    candidates
        .into_iter()
        .find(|p| p.is_file())
        .unwrap_or_else(|| PathBuf::from(pkgdatadir()).join(FILE))
}

/// Bundled GeneralUser GS SoundFont.
///
/// Desktop looks next to the executable and in pkgdatadir (same portable-tree
/// strategy as the GResource bundle). Android materializes the APK asset to
/// app-private storage and hands the path to Rust through JNI, so the synth
/// worker still receives a normal filesystem path on every platform.
pub(crate) fn soundfont_path() -> Result<PathBuf, String> {
    #[cfg(target_os = "android")]
    {
        // Once Java has handed us a path, let FluidSynth try it even if the
        // file is missing: that produces a real load error/toast instead of
        // waiting forever for a copy that failed.
        return ANDROID_SOUNDFONT
            .get()
            .cloned()
            .ok_or_else(|| "The built-in instrument is still being prepared".to_string());
    }

    #[cfg(not(target_os = "android"))]
    {
        const FILE: &str = "soundfonts/GeneralUser-GS.sf2";

        if let Ok(path) = std::env::var("PRELUDE_SF2") {
            let path = PathBuf::from(path);
            if path.is_file() {
                return Ok(path);
            }
        }

        let mut candidates: Vec<PathBuf> = Vec::new();
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                candidates.push(dir.join("../share/prelude").join(FILE));
                candidates.push(dir.join("../Resources").join(FILE));
                candidates.push(dir.join(FILE));
            }
        }
        candidates.push(PathBuf::from(pkgdatadir()).join(FILE));

        candidates
            .into_iter()
            .find(|path| path.is_file())
            .ok_or_else(|| "Bundled GeneralUser GS SoundFont is missing".to_string())
    }
}

#[cfg(target_os = "android")]
static ANDROID_SOUNDFONT: OnceLock<PathBuf> = OnceLock::new();

/// Called from `PreludeActivity` after the APK asset has been copied to
/// app-private storage. Empty means "preparation failed"; the synth worker
/// will surface a normal load error instead of silently staying muted.
#[cfg(target_os = "android")]
pub(crate) fn set_android_soundfont_path(path: PathBuf) {
    if let Some(existing) = ANDROID_SOUNDFONT.get() {
        if existing != &path {
            eprintln!("prelude: Android SoundFont path changed; keeping the first");
        }
        return;
    }
    let _ = ANDROID_SOUNDFONT.set(path);
}
