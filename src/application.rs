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

fn select_port(dropdown: &gtk::DropDown, ports: &[String], current: Option<&str>) {
    if let Some(name) = current {
        if let Some(i) = ports.iter().position(|p| p == name) {
            dropdown.set_selected(i as u32);
        }
    } else {
        dropdown.set_selected(0);
    }
}

/// Load a MIDI file into the engine and update the UI; returns whether the load succeeded.
fn load_file(
    engine: &Rc<RefCell<MidiEngine>>,
    path: &str,
    label_name: &gtk::Label,
    main_stack: &gtk::Stack,
    seek_adjustment: &gtk::Adjustment,
    density_view: &MidiDensityView,
    error_page: &adw::StatusPage,
) -> bool {
    let mut eng = engine.borrow_mut();
    match eng.load(path) {
        Ok(name) => {
            let total = eng.total_length();
            let peaks = eng.note_density_data(DENSITY_BINS);
            eng.play();
            drop(eng);
            label_name.set_text(&name);
            density_view.set_peaks(peaks);
            density_view.set_position(0.0);
            seek_adjustment.set_upper(total);
            seek_adjustment.set_value(0.0);
            main_stack.set_visible_child_name("main-view");
            true
        }
        Err(e) => {
            drop(eng);
            error_page.set_description(Some(&e));
            main_stack.set_visible_child_name("error-view");
            false
        }
    }
}

fn on_activate(app: &adw::Application, engine: Rc<RefCell<MidiEngine>>) {
    let builder = gtk::Builder::from_string(include_str!(concat!(env!("OUT_DIR"), "/window.ui")));

    let window = get_object!(builder, "window_main", adw::ApplicationWindow);
    window.set_application(Some(app));

    let provider = gtk::CssProvider::new();
    provider.load_from_string(include_str!("../ui/style.css"));
    gtk::style_context_add_provider_for_display(
        &gdk::Display::default().expect("no display"),
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

    let main_inner_box = get_object!(builder, "main_inner_box", gtk::Box);
    let density_view = MidiDensityView::new();
    main_inner_box.insert_child_after(density_view.widget(), Some(&label_name));

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
                        &label_name,
                        &main_stack,
                        &seek_adjustment,
                        &density_view,
                        &error_page,
                    );
                }
                false
            },
        ));

        window.add_controller(drop_target);
    }

    // ── Port model / dropdown ──
    let port_model = gtk::StringList::new(&[]);
    let port_dropdown = gtk::DropDown::new(Some(port_model.clone()), None::<&gtk::Expression>);

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
        port_dropdown,
        #[strong]
        window,
        move |_, _| {
            let dialog = adw::AlertDialog::builder()
                .title("Port settings")
                .body("Select MIDI output port:")
                .build();
            dialog.add_response("close", "Close");
            dialog.set_default_response(Some("close"));

            let ports = populate_ports();
            if !ports.is_empty() {
                let current = engine.borrow();
                select_port(&port_dropdown, &ports, current.port_name());
            }

            let refresh_btn = gtk::Button::builder()
                .icon_name("view-refresh-symbolic")
                .tooltip_text("Refresh ports")
                .build();

            refresh_btn.connect_clicked(clone!(
                #[strong]
                populate_ports,
                #[strong]
                port_dropdown,
                move |_| {
                    let ports = populate_ports();
                    if !ports.is_empty() {
                        port_dropdown.set_selected(0);
                    }
                },
            ));

            let box_ = gtk::Box::new(gtk::Orientation::Horizontal, 6);
            box_.append(&port_dropdown);
            box_.append(&refresh_btn);
            dialog.set_extra_child(Some(&box_));
            dialog.present(Some(&window));
        },
    ));
    app.add_action(&port_action);

    port_dropdown.connect_selected_notify(clone!(
        #[strong]
        engine,
        #[strong]
        port_model,
        move |dd| {
            let pos = dd.selected();
            if pos != u32::MAX {
                if let Some(name) = port_model.string(pos) {
                    let _ = engine.borrow_mut().open_port(name.as_ref());
                }
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
                                &label_name,
                                &main_stack,
                                &seek_adjustment,
                                &density_view,
                                &error_page,
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
        move |_| {
            engine.borrow_mut().stop();
            label_position.set_text("0:00");
            seek_adjustment.set_value(0.0);
            density_view.set_position(0.0);
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
        if !ports.is_empty() {
            let current = engine.borrow();
            select_port(&port_dropdown, &ports, current.port_name());
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
                let mut eng = engine.borrow_mut();
                let state = eng.state();

                if state == State::Playing {
                    let due = eng.tick();
                    for ev in &due {
                        eng.send_event(ev);
                    }
                }

                let total = eng.total_length();
                let scale_active = seek_scale.state_flags().contains(gtk::StateFlags::ACTIVE);

                if !scale_active {
                    // dragged then released — seek once to final position
                    if was_scale_active.get() {
                        was_scale_active.set(false);
                        eng.seek(seek_adjustment.value());
                    }
                    if !density_view.is_dragging() {
                        if state == State::Playing {
                            let elapsed = eng.elapsed();
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

                if state == State::Playing && eng.state() != State::Playing {
                    eng.stop();
                }

                let elapsed = eng.elapsed();
                label_position.set_text(&format_time(elapsed));
                label_length.set_text(&format_time(total));
                update_transport_button(&eng, &btn_start_stop);

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
