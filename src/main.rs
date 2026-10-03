mod application;
mod config;
mod engine;
mod midi_view;
mod page_view;

use adw::prelude::*;
use gtk::gio;

fn main() -> gtk::glib::ExitCode {
    // UI templates live in the installed GResource bundle (compiled from
    // ui/*.blp by Meson); without it no window can be built.
    let resource_path = format!("{}/prelude.gresource", config::pkgdatadir());
    let resources = match gio::Resource::load(&resource_path) {
        Ok(res) => res,
        Err(e) => {
            eprintln!("prelude: cannot load resources from {resource_path}: {e}");
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
