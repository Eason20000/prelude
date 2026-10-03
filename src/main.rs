mod application;
mod config;
mod engine;
mod midi_view;
mod page_view;

use adw::prelude::*;
use gtk::gio;

fn main() -> gtk::glib::ExitCode {
    // UI templates live in the GResource bundle (compiled from ui/*.blp by
    // Meson); without it no window can be built. The bundle is located
    // relative to the executable so portable trees (AppImage, zip, .app)
    // run without installing to the build-time prefix.
    let resource_path = config::gresource_path();
    let resources = match gio::Resource::load(&resource_path) {
        Ok(res) => res,
        Err(e) => {
            eprintln!(
                "prelude: cannot load resources from {}: {e}",
                resource_path.display()
            );
            return gtk::glib::ExitCode::FAILURE;
        }
    };
    gio::resources_register(&resources);

    let app = adw::Application::builder()
        .application_id(config::app_id())
        .build();

    let prelude_app = application::PreludeApplication::new();
    prelude_app.run(&app);

    app.run()
}
