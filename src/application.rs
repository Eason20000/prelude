use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use gtk::gdk;
use gtk::gio;
use gtk::glib;
use gtk::prelude::*;

use glib::clone;

use adw::prelude::*;

use crate::engine::{LOAD_CANCELLED, LoadedSong, MidiEngine, State};
use crate::midi_view::MidiDensityView;
use crate::page_view::PageTurnView;

const TICK_INTERVAL_MS: u32 = 20;
const LOAD_POLL_INTERVAL_MS: u64 = 20;
const SEEK_STEP: f64 = 5.0;
const DENSITY_BINS: usize = 300;

macro_rules! get_object {
    ($builder:expr, $id:literal, $ty:ty) => {
        $builder
            .object::<$ty>($id)
            .expect(concat!("Failed to get ", $id))
    };
}

pub(crate) struct PreludeApplication {
    engine: Rc<RefCell<MidiEngine>>,
}

impl PreludeApplication {
    pub(crate) fn new() -> Self {
        Self {
            engine: Rc::new(RefCell::new(MidiEngine::new())),
        }
    }

    pub(crate) fn run(self, app: &adw::Application) {
        let engine = self.engine;
        app.connect_activate(move |app| on_activate(app, engine.clone()));
    }
}

fn select_port(row: &adw::ComboRow, ports: &[String], current: Option<&str>) {
    if ports.is_empty() {
        row.set_selected(gtk::INVALID_LIST_POSITION);
        return;
    }
    if let Some(name) = current {
        if let Some(i) = ports.iter().position(|p| p == name) {
            row.set_selected(i as u32);
            return;
        }
    }
    row.set_selected(0);
}

/// Completion message from a background parse: the requesting generation plus
/// its result. Stale generations (a newer load started, or cancel) are
/// discarded on arrival; the worker thread then simply exits.
type LoadResult = (u64, Result<LoadedSong, String>);

/// What the loader thread consumes. Files with a filesystem path take the
/// fast path; content-addressed files (Android `content://` URIs, whose
/// `GFile::path()` is `None`) travel as a GFile handle plus display name —
/// the worker reads them through GIO streams instead.
#[derive(Clone)]
enum LoadInput {
    Path(String),
    Remote { file: gio::File, name: String },
}

/// Prefer a filesystem path (keeps desktop behavior byte-identical);
/// fall back to the GFile handle for non-local files.
fn load_input_from_file(file: &gio::File) -> LoadInput {
    // Android: scoped storage makes raw paths unreadable (ENOENT) even when
    // the picker hands back a file:// URI, and content:// URIs have no path
    // at all. Always go through the ContentResolver stream there; the URI
    // permission grant travels with the GFile handle.
    #[cfg(target_os = "android")]
    {
        let name = display_name_of(file);
        return LoadInput::Remote {
            file: file.clone(),
            name,
        };
    }
    #[cfg(not(target_os = "android"))]
    if let Some(path) = file.path() {
        LoadInput::Path(path.to_string_lossy().to_string())
    } else {
        LoadInput::Remote {
            file: file.clone(),
            name: display_name_of(file),
        }
    }
}

fn display_name_of(file: &gio::File) -> String {
    file.query_info(
        "standard::display-name",
        gio::FileQueryInfoFlags::NONE,
        gio::Cancellable::NONE,
    )
    .ok()
    .map(|info| info.display_name().to_string())
    .filter(|s| !s.is_empty())
    .unwrap_or_else(|| "midi file".to_string())
}

/// What the parse worker consumes: plain bytes plus their display name.
/// Paths are passed through; remote content arrives as an already-opened
/// stream (opened on the main thread — see `start_load`).
enum ParseSource {
    Path(String),
    Stream { stream: SendStream, name: String },
}

/// `gio::FileInputStream` is not `Send` at the type level, but this
/// particular backend is thread-safe by construction: the Java stream object
/// is held by global ref (`gdk_android_java_file_input_stream_wrap`), and
/// every read acquires a thread-guarded JNI env (attaching on demand) and
/// drops the guard afterwards — including finalize. Reads never touch thread
/// coordinate state, so moving the handle into the parse worker is sound.
//
// SAFETY: see above; the handle is used from exactly one thread at a time.
struct SendStream(gio::FileInputStream);

unsafe impl Send for SendStream {}

/// Chunk size for worker-side stream reads; each chunk is also a
/// cancellation polling point, so huge files stay responsive to cancel.
const READ_CHUNK_SIZE: usize = 1024 * 1024;

fn spawn_parse_worker(
    source: ParseSource,
    generation: u64,
    flag: Arc<AtomicBool>,
    tx: std::sync::mpsc::Sender<LoadResult>,
) {
    std::thread::spawn(move || {
        let result = match source {
            ParseSource::Path(path) => MidiEngine::parse_file(&path, DENSITY_BINS, &flag),
            ParseSource::Stream { stream, name } => {
                let stream = stream.0;
                let mut bytes = Vec::new();
                let mut chunk = vec![0u8; READ_CHUNK_SIZE];
                loop {
                    if flag.load(Ordering::Relaxed) {
                        break Err(LOAD_CANCELLED.to_string());
                    }
                    match stream.read(&mut chunk, gio::Cancellable::NONE) {
                        Ok(0) => break MidiEngine::parse_bytes(&bytes, name, DENSITY_BINS, &flag),
                        Ok(n) => bytes.extend_from_slice(&chunk[..n]),
                        Err(e) => break Err(format!("Failed to read file: {e}")),
                    }
                }
            }
        };
        let _ = tx.send((generation, result));
    });
}

/// Strip the extension for the title label (`engine.file_name` keeps it for
/// the info sheet).
fn display_stem(name: &str) -> String {
    std::path::Path::new(name)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| name.to_string())
}

// Builder lookups must succeed — a missing widget means the UI template and
// this code have drifted apart, so panicking is the correct failure mode.
#[allow(clippy::expect_used)]
fn on_activate(app: &adw::Application, engine: Rc<RefCell<MidiEngine>>) {
    let builder = gtk::Builder::from_resource("/top/vikasmi/Prelude/window.ui");

    let window = get_object!(builder, "window_main", adw::ApplicationWindow);
    window.set_application(Some(app));

    let provider = gtk::CssProvider::new();
    provider.load_from_string(include_str!("../ui/style.css"));
    let Some(display) = gdk::Display::default() else {
        panic!("no display available");
    };
    gtk::style_context_add_provider_for_display(
        &display,
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );

    // Android ships no system icon theme; the APK carries app-bundled Adwaita
    // symbolic icons (ui/icons → GResource). Desktop resolves from the
    // system theme and never registers this path, so nothing is shadowed.
    #[cfg(target_os = "android")]
    gtk::IconTheme::for_display(&display).add_resource_path("/top/vikasmi/Prelude/icons");

    let drag_revealer = get_object!(builder, "drag_revealer", gtk::Revealer);
    let main_content = get_object!(builder, "main_content", adw::ToolbarView);
    let error_page = get_object!(builder, "error_page", adw::StatusPage);
    let button_error_retry = get_object!(builder, "button_error_retry", gtk::Button);
    let button_loading_cancel = get_object!(builder, "button_loading_cancel", gtk::Button);
    let loading_status = get_object!(builder, "loading_status", adw::StatusPage);
    loading_status.set_paintable(Some(&adw::SpinnerPaintable::new(Some(&loading_status))));
    let main_stack = get_object!(builder, "main_stack", gtk::Stack);
    let info_sheet = get_object!(builder, "info_sheet", adw::BottomSheet);
    let label_info = get_object!(builder, "label_info", gtk::Label);
    let label_name = get_object!(builder, "label_name", gtk::Label);
    let label_position = get_object!(builder, "label_position", gtk::Label);
    let label_length = get_object!(builder, "label_length", gtk::Label);
    let seek_scale = get_object!(builder, "seek_scale", gtk::Scale);
    let seek_adjustment = get_object!(builder, "seek_adjustment", gtk::Adjustment);
    let btn_start_stop = get_object!(builder, "button_start_stop", gtk::Button);
    let btn_info = get_object!(builder, "button_info", gtk::Button);
    let btn_info_close = get_object!(builder, "button_info_close", gtk::Button);
    let btn_open = get_object!(builder, "button_open", gtk::Button);
    let btn_open_initial = get_object!(builder, "button_open_initial", gtk::Button);
    let btn_stop = get_object!(builder, "button_stop", gtk::Button);

    let page_placeholder = get_object!(builder, "page_view_placeholder", gtk::Box);
    let density_placeholder = get_object!(builder, "density_view_placeholder", gtk::Box);
    let density_view = MidiDensityView::new();
    let page_view = PageTurnView::new(engine.clone());
    // Placeholders are parents — Blueprint order is the view order, no remove/reorder.
    page_placeholder.append(page_view.widget());
    density_placeholder.append(density_view.widget());

    // ── Async MIDI loading ──
    // Parsing runs on a worker thread; the main thread only switches the
    // stack and applies the result, so menus and port settings stay usable
    // while a huge file parses. `load_gen` discards stale completions and
    // `load_cancel` asks the running worker to bail out at its polling
    // points (midly parsing itself is not interruptible). Completion travels
    // back over a std mpsc channel polled from the main loop, so no GTK
    // object ever crosses threads.
    let load_gen = Rc::new(Cell::new(0u64));
    let load_cancel: Rc<RefCell<Arc<AtomicBool>>> =
        Rc::new(RefCell::new(Arc::new(AtomicBool::new(false))));

    // Start loading `input` on a worker thread; returns whether the load was
    // accepted (false only for an unresolvable path). Stops the old song
    // immediately and supersedes any load still in flight.
    let start_load = clone!(
        #[strong]
        engine,
        #[strong]
        label_name,
        #[strong]
        main_stack,
        #[strong]
        seek_adjustment,
        #[strong]
        density_view,
        #[strong]
        page_view,
        #[strong]
        error_page,
        #[strong]
        label_position,
        #[strong]
        label_length,
        #[strong]
        load_gen,
        #[strong]
        load_cancel,
        move |input: LoadInput| -> bool {
            // An empty path means the file was never resolvable (replaces
            // the old path().unwrap_or_default() == "" check at call sites);
            // report that directly, not as a read error.
            if matches!(&input, LoadInput::Path(path) if path.is_empty()) {
                error_page.set_description(Some(
                    "Could not resolve the dropped/selected file path (non-local file?).",
                ));
                main_stack.set_visible_child_name("error-view");
                return false;
            }
            load_cancel.borrow().store(true, Ordering::Relaxed);
            let gen = load_gen.get() + 1;
            load_gen.set(gen);
            let flag = Arc::new(AtomicBool::new(false));
            *load_cancel.borrow_mut() = flag.clone();

            engine.borrow_mut().stop();
            label_position.set_text("0:00");
            label_length.set_text("0:00");
            seek_adjustment.set_value(0.0);
            density_view.set_position(0.0);
            page_view.reset();
            main_stack.set_visible_child_name("loading-view");

            let (tx, rx) = std::sync::mpsc::channel::<LoadResult>();
            match input {
                LoadInput::Path(path) => {
                    spawn_parse_worker(ParseSource::Path(path), gen, flag, tx);
                }
                LoadInput::Remote { file, name } => {
                    // Opening the stream goes through JNI, which is only
                    // valid on attached threads: open here on the main thread
                    // (fast regardless of size — no data moves yet). The
                    // per-read path auto-attaches instead, so the worker may
                    // stream from the open handle on any thread.
                    match file.read(gio::Cancellable::NONE) {
                        Ok(stream) => spawn_parse_worker(
                            ParseSource::Stream {
                                stream: SendStream(stream),
                                name,
                            },
                            gen,
                            flag,
                            tx,
                        ),
                        Err(e) => {
                            error_page
                                .set_description(Some(&format!("Failed to read file: {e}")));
                            main_stack.set_visible_child_name("error-view");
                        }
                    }
                }
            }

            // Poll for completion on the main loop: cheap, non-blocking, and
            // it self-removes once this generation resolves or is superseded.
            let poll_rx = Rc::new(RefCell::new(rx));
            glib::timeout_add_local(
                std::time::Duration::from_millis(LOAD_POLL_INTERVAL_MS),
                clone!(
                    #[strong]
                    engine,
                    #[strong]
                    label_name,
                    #[strong]
                    main_stack,
                    #[strong]
                    seek_adjustment,
                    #[strong]
                    density_view,
                    #[strong]
                    page_view,
                    #[strong]
                    error_page,
                    #[strong]
                    label_position,
                    #[strong]
                    label_length,
                    #[strong]
                    load_gen,
                    #[strong]
                    poll_rx,
                    move || {
                        if gen != load_gen.get() {
                            return glib::ControlFlow::Break;
                        }
                        match poll_rx.borrow().try_recv() {
                            Ok((msg_gen, result)) => {
                                if msg_gen != load_gen.get() {
                                    return glib::ControlFlow::Break;
                                }
                                match result {
                                    Ok(song) => {
                                        let total = song.total_length;
                                        let peaks = song.peaks.clone();
                                        let name = song.file_name.clone();
                                        {
                                            let mut eng = engine.borrow_mut();
                                            eng.apply_loaded(song);
                                            eng.play();
                                        }
                                        label_name.set_text(&display_stem(&name));
                                        density_view.set_peaks(peaks);
                                        density_view.set_position(0.0);
                                        page_view.reset();
                                        seek_adjustment.set_upper(total);
                                        seek_adjustment.set_value(0.0);
                                        label_position.set_text("0:00");
                                        label_length.set_text(&format_time(total));
                                        main_stack.set_visible_child_name("main-view");
                                    }
                                    Err(e) => {
                                        // A cancelled worker arrives with a
                                        // stale `gen` and is dropped above;
                                        // reaching this means a real error.
                                        if e != LOAD_CANCELLED {
                                            error_page.set_description(Some(&e));
                                            main_stack.set_visible_child_name("error-view");
                                        }
                                    }
                                }
                                glib::ControlFlow::Break
                            }
                            Err(std::sync::mpsc::TryRecvError::Empty) => {
                                glib::ControlFlow::Continue
                            }
                            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                                error_page
                                    .set_description(Some("Loader thread failed unexpectedly."));
                                main_stack.set_visible_child_name("error-view");
                                glib::ControlFlow::Break
                            }
                        }
                    },
                ),
            );
            true
        },
    );

    button_loading_cancel.connect_clicked(clone!(
        #[strong]
        main_stack,
        #[strong]
        load_gen,
        #[strong]
        load_cancel,
        move |_| {
            load_cancel.borrow().store(true, Ordering::Relaxed);
            load_gen.set(load_gen.get() + 1);
            main_stack.set_visible_child_name("initial-view");
        },
    ));

    // ── Density view position changed → seek ──
    density_view.set_on_position_changed(clone!(
        #[strong]
        engine,
        move |pos| {
            let total = engine.borrow().total_length();
            engine.borrow_mut().seek(pos * total);
        },
    ));

    // ── Drag & drop overlay ──
    {
        let drop_target = gtk::DropTarget::new(gdk::FileList::static_type(), gdk::DragAction::COPY);

        drop_target.connect_notify_local(
            Some("current-drop"),
            clone!(
                #[strong]
                drag_revealer,
                #[strong]
                main_content,
                #[strong]
                drop_target,
                move |_, _| {
                    let is_dragging = drop_target.current_drop().is_some();
                    drag_revealer.set_reveal_child(is_dragging);
                    if is_dragging {
                        main_content.add_css_class("blurred");
                    } else {
                        main_content.remove_css_class("blurred");
                    }
                },
            ),
        );

        drop_target.connect_drop(clone!(
            #[strong]
            start_load,
            move |_target, value, _x, _y| {
                let Ok(file_list) = value.get::<gdk::FileList>() else {
                    return false;
                };
                if let Some(file) = file_list.files().first() {
                    return start_load(load_input_from_file(file));
                }
                false
            },
        ));

        window.add_controller(drop_target);
    }

    // ── Port settings Blueprint dialog (adaptive: floating on desktop, bottom-sheet on mobile) ──
    let port_model = gtk::StringList::new(&[]);
    let port_builder = gtk::Builder::from_resource("/top/vikasmi/Prelude/port_settings.ui");
    let port_dialog = get_object!(port_builder, "port_dialog", adw::PreferencesDialog);
    let port_row = get_object!(port_builder, "port_row", adw::ComboRow);
    let btn_port_refresh = get_object!(port_builder, "btn_port_refresh", gtk::Button);

    // ComboRow renders StringList's StringObject via the "string" property
    let port_expr = gtk::PropertyExpression::new(
        gtk::StringObject::static_type(),
        None::<&gtk::Expression>,
        "string",
    );
    port_row.set_expression(Some(port_expr));
    port_row.set_model(Some(&port_model));

    let populate_ports = clone!(
        #[strong]
        port_model,
        move || {
            let ports = MidiEngine::list_ports();
            port_model.splice(
                0,
                port_model.n_items(),
                &ports.iter().map(|s| &**s).collect::<Vec<_>>(),
            );
            ports
        },
    );

    // ── Actions (menu) ──
    let about_action = gio::SimpleAction::new("about", None);
    about_action.connect_activate(clone!(
        #[strong]
        window,
        move |_, _| show_about(&window),
    ));
    app.add_action(&about_action);

    let port_action = gio::SimpleAction::new("port-settings", None);
    port_action.connect_activate(clone!(
        #[strong]
        engine,
        #[strong]
        populate_ports,
        #[strong]
        port_dialog,
        #[strong]
        port_row,
        #[strong]
        window,
        move |_, _| {
            let ports = populate_ports();
            {
                let current = engine.borrow();
                select_port(&port_row, &ports, current.port_name());
            }
            let has_ports = !ports.is_empty();
            port_row.set_sensitive(has_ports);
            if has_ports {
                port_row.set_subtitle("");
            } else {
                port_row.set_subtitle("No MIDI ports available");
            }
            port_dialog.present(Some(&window));
        },
    ));
    app.add_action(&port_action);

    port_row.connect_selected_notify(clone!(
        #[strong]
        engine,
        #[strong]
        port_model,
        move |row| {
            let pos = row.selected();
            if pos != gtk::INVALID_LIST_POSITION {
                if let Some(name) = port_model.string(pos) {
                    let _ = engine.borrow_mut().open_port(name.as_ref());
                }
            }
        },
    ));

    btn_port_refresh.connect_clicked(clone!(
        #[strong]
        populate_ports,
        #[strong]
        port_row,
        move |_| {
            let ports = populate_ports();
            let has_ports = !ports.is_empty();
            port_row.set_sensitive(has_ports);
            if has_ports {
                port_row.set_selected(0);
                port_row.set_subtitle("");
            } else {
                port_row.set_selected(gtk::INVALID_LIST_POSITION);
                port_row.set_subtitle("No MIDI ports available");
            }
        },
    ));

    // ── File dialog ──
    let file_dialog = gtk::FileDialog::new();
    let midi_filter = gtk::FileFilter::new();
    midi_filter.set_name(Some("MIDI files"));
    midi_filter.add_pattern("*.mid");
    midi_filter.add_pattern("*.MID");
    midi_filter.add_pattern("*.midi");
    midi_filter.add_pattern("*.MIDI");
    midi_filter.add_pattern("*.smf");
    midi_filter.add_pattern("*.SMF");
    // MIME types are what the Android document picker filters on
    // (EXTRA_MIME_TYPES); glob patterns are ignored there. They also make
    // desktop portals more precise, so they are unconditional.
    midi_filter.add_mime_type("audio/midi");
    midi_filter.add_mime_type("audio/mid");
    midi_filter.add_mime_type("audio/x-midi");
    let all_filter = gtk::FileFilter::new();
    all_filter.set_name(Some("All files"));
    all_filter.add_pattern("*");
    let filters = gio::ListStore::new::<gtk::FileFilter>();
    filters.append(&midi_filter);
    filters.append(&all_filter);
    file_dialog.set_filters(Some(&filters));
    file_dialog.set_default_filter(Some(&midi_filter));

    // ── Button callbacks ──
    {
        let perform_load = clone!(
            #[strong]
            file_dialog,
            #[strong]
            window,
            #[strong]
            start_load,
            move || {
                glib::MainContext::default().spawn_local(clone!(
                    #[strong]
                    file_dialog,
                    #[strong]
                    window,
                    #[strong]
                    start_load,
                    async move {
                        if let Ok(file) = file_dialog.open_future(Some(&window)).await {
                            start_load(load_input_from_file(&file));
                        }
                    },
                ));
            },
        );

        btn_open.connect_clicked(clone!(
            #[strong]
            perform_load,
            move |_| perform_load(),
        ));
        btn_open_initial.connect_clicked(clone!(
            #[strong]
            perform_load,
            move |_| perform_load(),
        ));
        button_error_retry.connect_clicked(clone!(
            #[strong]
            perform_load,
            move |_| perform_load(),
        ));
    }

    btn_info.connect_clicked(clone!(
        #[strong]
        engine,
        #[strong]
        label_info,
        #[strong]
        info_sheet,
        move |_| {
            let eng = engine.borrow();
            label_info.set_label(&format_info(&eng));
            info_sheet.set_open(true);
        },
    ));

    btn_info_close.connect_clicked(clone!(
        #[strong]
        info_sheet,
        move |_| {
            info_sheet.set_open(false);
        },
    ));

    btn_start_stop.connect_clicked(clone!(
        #[strong]
        engine,
        move |_| {
            engine.borrow_mut().toggle_play_pause();
        },
    ));

    btn_stop.connect_clicked(clone!(
        #[strong]
        engine,
        #[strong]
        label_position,
        #[strong]
        seek_adjustment,
        #[strong]
        density_view,
        #[strong]
        page_view,
        move |_| {
            engine.borrow_mut().stop();
            label_position.set_text("0:00");
            seek_adjustment.set_value(0.0);
            density_view.set_position(0.0);
            // "Stop" means back to the initial state, not a frozen frame.
            page_view.reset();
        },
    ));

    // ── Scale seek (change-value signal) ──
    // While the user is dragging the slider, deferred to release.
    // The tick loop detects ACTIVE → !ACTIVE and seeks once.
    seek_scale.connect_change_value(clone!(
        #[strong]
        engine,
        #[strong]
        seek_scale,
        move |_scale, _scroll, new_value| {
            if seek_scale.state_flags().contains(gtk::StateFlags::ACTIVE) {
                return gtk::glib::Propagation::Proceed;
            }
            let total = engine.borrow().total_length();
            engine.borrow_mut().seek(new_value.clamp(0.0, total));
            gtk::glib::Propagation::Proceed
        },
    ));

    // ── Keyboard seek: left/right arrows ──
    {
        let key_controller = gtk::EventControllerKey::new();
        key_controller.connect_key_pressed(clone!(
            #[strong]
            engine,
            move |_controller, keyval, _keycode, _modifier| {
                let mut eng = engine.borrow_mut();
                let current = eng.elapsed();
                let total = eng.total_length();
                if keyval == gdk::Key::Left {
                    eng.seek((current - SEEK_STEP).max(0.0));
                } else if keyval == gdk::Key::Right {
                    eng.seek((current + SEEK_STEP).min(total));
                }
                gtk::glib::Propagation::Proceed
            },
        ));
        window.add_controller(key_controller);
    }

    // ── Keyboard shortcut: Space ──
    {
        let controller = gtk::ShortcutController::new();
        let shortcut = gtk::Shortcut::new(
            gtk::ShortcutTrigger::parse_string("space"),
            Some(gtk::CallbackAction::new(clone!(
                #[strong]
                engine,
                move |_, _| {
                    engine.borrow_mut().toggle_play_pause();
                    glib::Propagation::Proceed
                },
            ))),
        );
        controller.add_shortcut(shortcut);
        window.add_controller(controller);
    }

    // ── Close request ──
    window.connect_close_request(clone!(
        #[strong]
        engine,
        move |_| {
            engine.borrow_mut().stop();
            glib::Propagation::Proceed
        },
    ));

    // ── Refresh ports and select first one ──
    {
        let ports = populate_ports();
        {
            let current = engine.borrow();
            select_port(&port_row, &ports, current.port_name());
        }
        let has_ports = !ports.is_empty();
        port_row.set_sensitive(has_ports);
        if has_ports {
            port_row.set_subtitle("");
        } else {
            port_row.set_subtitle("No MIDI ports available");
        }
    }

    // ── Tick loop ──
    start_tick_loop(
        engine.clone(),
        btn_start_stop.clone(),
        seek_scale.clone(),
        seek_adjustment.clone(),
        density_view.clone(),
        label_position.clone(),
        label_length.clone(),
    );

    window.present();
}

fn start_tick_loop(
    engine: Rc<RefCell<MidiEngine>>,
    btn_start_stop: gtk::Button,
    seek_scale: gtk::Scale,
    seek_adjustment: gtk::Adjustment,
    density_view: MidiDensityView,
    label_position: gtk::Label,
    label_length: gtk::Label,
) {
    let was_scale_active = Rc::new(Cell::new(false));

    glib::timeout_add_local(
        std::time::Duration::from_millis(TICK_INTERVAL_MS.into()),
        clone!(
            #[strong]
            engine,
            #[strong]
            btn_start_stop,
            #[strong]
            seek_scale,
            #[strong]
            seek_adjustment,
            #[strong]
            density_view,
            #[strong]
            label_position,
            #[strong]
            label_length,
            move || {
                // Short borrows only: never hold the engine across MIDI I/O
                // or widget updates.
                let state = engine.borrow().state();

                if state == State::Playing {
                    let due = engine.borrow_mut().tick();
                    for ev in &due {
                        engine.borrow_mut().send_event(ev);
                    }
                }

                let total = engine.borrow().total_length();
                let scale_active = seek_scale.state_flags().contains(gtk::StateFlags::ACTIVE);

                if !scale_active {
                    // dragged then released — seek once to final position
                    if was_scale_active.get() {
                        was_scale_active.set(false);
                        engine.borrow_mut().seek(seek_adjustment.value());
                    }
                    if !density_view.is_dragging() {
                        if state == State::Playing {
                            let elapsed = engine.borrow().elapsed();
                            seek_adjustment.set_value(elapsed);
                            if total > 0.0 {
                                density_view.set_position(elapsed / total);
                            }
                        }
                    } else {
                        seek_adjustment.set_value(density_view.position() * total);
                    }
                } else {
                    was_scale_active.set(true);
                    if !density_view.is_dragging() && total > 0.0 {
                        density_view.set_position(seek_adjustment.value() / total);
                    }
                }

                // NOTE: deliberately no re-stop() at EOF. engine.tick() parks
                // the state at Stopped/elapsed=total ("stay at the end"); an
                // extra stop() here would rewind to 0:00 and flash the labels
                // and the page view back to the first page every time a file
                // ends. Replay-from-end rewinds in play() instead.
                let elapsed = engine.borrow().elapsed();
                label_position.set_text(&format_time(elapsed));
                label_length.set_text(&format_time(total));
                {
                    let eng = engine.borrow();
                    update_transport_button(&eng, &btn_start_stop);
                }

                glib::ControlFlow::Continue
            },
        ),
    );
}

fn update_transport_button(engine: &MidiEngine, btn: &gtk::Button) {
    let icon = match engine.state() {
        State::Playing => "media-playback-pause-symbolic",
        _ => "media-playback-start-symbolic",
    };
    if let Some(child) = btn.first_child() {
        if let Some(content) = child.downcast_ref::<adw::ButtonContent>() {
            content.set_icon_name(icon);
        }
    }
}

fn format_time(seconds: f64) -> String {
    let seconds = seconds.max(0.0) as u64;
    let mins = seconds / 60;
    let secs = seconds % 60;
    format!("{mins}:{secs:02}")
}

fn format_info(engine: &MidiEngine) -> String {
    format!(
        "File: {}\nLength: {:.2}s",
        engine.file_name(),
        engine.total_length(),
    )
}

fn show_about(window: &adw::ApplicationWindow) {
    let dialog = adw::AboutDialog::builder()
        .application_name("Prelude")
        .application_icon("top.vikasmi.Prelude")
        .version(env!("CARGO_PKG_VERSION"))
        .developer_name("Eason20000")
        .copyright("© Eason20000")
        .website("https://github.com/Eason20000/prelude")
        .issue_url("https://github.com/Eason20000/prelude/issues")
        .license_type(gtk::License::Gpl30)
        .build();
    dialog.present(Some(window));
}
