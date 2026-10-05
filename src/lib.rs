mod application;
mod config;
mod engine;
mod midi_view;
mod page_view;

use adw::prelude::*;
use gtk::gio;

/// Desktop bootstrap, shared with future platform entries.
///
/// A future Android entry (cdylib/staticlib driven by Meson) must reuse
/// this exact initialization instead of duplicating it, so it lives in the
/// library behind a single function while `main` stays a thin wrapper.
pub fn run() -> gtk::glib::ExitCode {
    if let Err(msg) = register_resources() {
        eprintln!("{msg}");
        return gtk::glib::ExitCode::FAILURE;
    }

    let app = adw::Application::builder()
        .application_id(config::app_id())
        .build();

    let prelude_app = application::PreludeApplication::new();
    prelude_app.run(&app);

    app.run()
}

/// Android entry point, called from the C `main` that the Pixiewood
/// launcher loads (Meson `android_exe_type: 'application'`). Arguments are
/// intentionally not forwarded: parity with the desktop `app.run()`.
#[cfg(target_os = "android")]
#[no_mangle]
pub extern "C" fn prelude_android_main(
    _argc: std::os::raw::c_int,
    _argv: *mut *mut std::os::raw::c_char,
) -> std::os::raw::c_int {
    i32::from(run())
}

/// One-time Android MIDI bootstrap, called from `PreludeActivity.onCreate`
/// (UI thread) before GTK starts, via the JNI bridge in `midi_init.c`.
/// Hands the application context to ndk-context so midir's Android backend
/// can reach MidiManager. Must run before any midir call; our MIDI use is
/// lazy (port dialog), so onCreate ordering is sufficient by construction.
#[cfg(target_os = "android")]
#[no_mangle]
pub extern "C" fn prelude_android_init_midi(
    java_vm: *mut std::os::raw::c_void,
    context: *mut std::os::raw::c_void,
) {
    // SAFETY: called once from Java with a NewGlobalRef'd application
    // context that outlives the process; ndk-context only stores the
    // pointers. A Java-side static flag prevents re-init (which ndk-context
    // would abort on).
    unsafe {
        ndk_context::initialize_android_context(java_vm, context);
    }
}

// UI templates live in the GResource bundle (compiled from ui/*.blp by
// Meson); without it no window can be built. The bundle is located
// relative to the executable so portable trees (AppImage, zip, .app)
// run without installing to the build-time prefix.
#[cfg(not(target_os = "android"))]
fn register_resources() -> Result<(), String> {
    let resource_path = config::gresource_path();
    match gio::Resource::load(&resource_path) {
        Ok(res) => {
            gio::resources_register(&res);
            Ok(())
        }
        Err(e) => Err(format!(
            "prelude: cannot load resources from {}: {e}",
            resource_path.display()
        )),
    }
}

// Android has no relocatable install tree, so the bundle compiled by Meson
// is embedded into the staticlib instead. `PRELUDE_GRESOURCE_BUNDLE` is
// injected by the Android Meson build only; desktop builds never see it.
#[cfg(target_os = "android")]
const EMBEDDED_GRESOURCE: &[u8] = include_bytes!(env!("PRELUDE_GRESOURCE_BUNDLE"));

#[cfg(target_os = "android")]
fn register_resources() -> Result<(), String> {
    let bytes = gtk::glib::Bytes::from_static(EMBEDDED_GRESOURCE);
    match gio::Resource::from_data(&bytes) {
        Ok(res) => {
            gio::resources_register(&res);
            Ok(())
        }
        Err(e) => Err(format!("prelude: cannot load embedded resources: {e}")),
    }
}
