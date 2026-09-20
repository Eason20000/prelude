use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk::gdk;
use gtk::gio;
use gtk::glib;
use gtk::prelude::*;

use glib::clone;

use adw::prelude::*;

use crate::engine::{MidiEngine, State};
use crate::midi_view::MidiDensityView;
use crate::page_view::PageTurnView;

const TICK_INTERVAL_MS: u32 = 20;
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

/// Widgets that `load_file` updates after a successful load.
struct FileLoadUi<'a> {
    label_name: &'a gtk::Label,
    main_stack: &'a gtk::Stack,
    seek_adjustment: &'a gtk::Adjustment,
    density_view: &'a MidiDensityView,
    page_view: &'a PageTurnView,
    error_page: &'a adw::StatusPage,
}

/// Load a MIDI file into the engine and update the UI; returns whether the load succeeded.
fn load_file(engine: &Rc<RefCell<MidiEngine>>, path: &str, ui: FileLoadUi<'_>) -> bool {
    // Non-local GFiles surface as "" via path().unwrap_or_default() at the
    // call sites; report that directly instead of a generic read error.
    if path.is_empty() {
        ui.error_page.set_description(Some(
            "Could not resolve the dropped/selected file path (non-local file?).",
        ));
        ui.main_stack.set_visible_child_name("error-view");
        return false;
    }
    let mut eng = engine.borrow_mut();
    match eng.load(path) {
        Ok(name) => {
            let total = eng.total_length();
            let peaks = eng.note_density_data(DENSITY_BINS);
            eng.play();
            drop(eng);
            let display = std::path::Path::new(&name)
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .filter(|s| !s.is_empty())
                .unwrap_or(name);
            ui.label_name.set_text(&display);
            ui.density_view.set_peaks(peaks);
            ui.density_view.set_position(0.0);
            ui.page_view.reset();
            ui.seek_adjustment.set_upper(total);
            ui.seek_adjustment.set_value(0.0);
            ui.main_stack.set_visible_child_name("main-view");
            true
        }
        Err(e) => {
            drop(eng);
            ui.error_page.set_description(Some(&e));
            ui.main_stack.set_visible_child_name("error-view");
            false
        }
    }
}

// Builder lookups must succeed — a missing widget means the UI template and
// this code have drifted apart, so panicking is the correct failure mode.
#[allow(clippy::expect_used)]
fn on_activate(app: &adw::Application, engine: Rc<RefCell<MidiEngine>>) {
    let builder = gtk::Builder::from_string(include_str!(concat!(env!("OUT_DIR"), "/window.ui")));

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

    let drag_revealer = get_object!(builder, "drag_revealer", gtk::Revealer);
    let main_content = get_object!(builder, "main_content", adw::ToolbarView);
    let error_page = get_object!(builder, "error_page", adw::StatusPage);
    let button_error_retry = get_object!(builder, "button_error_retry", gtk::Button);
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
            move |_target, value, _x, _y| {
                let Ok(file_list) = value.get::<gdk::FileList>() else {
                    return false;
                };
                if let Some(file) = file_list.files().first() {
                    let path = file
                        .path()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string();
                    return load_file(
                        &engine,
                        &path,
                        FileLoadUi {
                            label_name: &label_name,
                            main_stack: &main_stack,
                            seek_adjustment: &seek_adjustment,
                            density_view: &density_view,
                            page_view: &page_view,
                            error_page: &error_page,
                        },
                    );
                }
                false
            },
        ));

        window.add_controller(drop_target);
    }

    // ── Port settings Blueprint dialog (adaptive: floating on desktop, bottom-sheet on mobile) ──
    let port_model = gtk::StringList::new(&[]);
    let port_builder =
        gtk::Builder::from_string(include_str!(concat!(env!("OUT_DIR"), "/port_settings.ui")));
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
            engine,
            #[strong]
            window,
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
            move || {
                glib::MainContext::default().spawn_local(clone!(
                    #[strong]
                    file_dialog,
                    #[strong]
                    engine,
                    #[strong]
                    window,
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
                    async move {
                        if let Ok(file) = file_dialog.open_future(Some(&window)).await {
                            let path = file
                                .path()
                                .unwrap_or_default()
                                .to_string_lossy()
                                .to_string();
                            load_file(
                                &engine,
                                &path,
                                FileLoadUi {
                                    label_name: &label_name,
                                    main_stack: &main_stack,
                                    seek_adjustment: &seek_adjustment,
                                    density_view: &density_view,
                                    page_view: &page_view,
                                    error_page: &error_page,
                                },
                            );
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
        .version(env!("CARGO_PKG_VERSION"))
        .developer_name("Eason20000")
        .license_type(gtk::License::Gpl30)
        .build();
    dialog.present(Some(window));
}
