fn main() -> gtk::glib::ExitCode {
    // Thin desktop entry point: the full bootstrap lives in the library
    // (`prelude::run`) so a future Android entry can reuse it without
    // duplicating GTK/GResource initialization.
    prelude::run()
}
