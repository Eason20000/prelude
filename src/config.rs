//! Build metadata.
//!
//! Under Meson, `APP_ID` and `PKGDATADIR` are injected through the
//! environment (see the `cargo_env` list in the root `meson.build`); a plain
//! `cargo build` falls back to the defaults below (which match a default
//! `meson install` with no `--prefix`).

const APP_ID_ENV: Option<&str> = option_env!("APP_ID");
const PKGDATADIR_ENV: Option<&str> = option_env!("PKGDATADIR");

pub(crate) fn app_id() -> &'static str {
    match APP_ID_ENV {
        Some(id) => id,
        None => "top.vikasmi.Prelude",
    }
}

pub(crate) fn pkgdatadir() -> &'static str {
    match PKGDATADIR_ENV {
        Some(dir) => dir,
        None => "/usr/local/share/prelude",
    }
}
