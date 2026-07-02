mod application;
mod engine;
mod midi_view;

use adw::prelude::*;

fn main() -> gtk::glib::ExitCode {
    let app = adw::Application::builder()
        .application_id("top.vikasmi.Prelude")
        .build();

    let prelude_app = application::PreludeApplication::new();
    prelude_app.run(&app);

    app.run()
}
