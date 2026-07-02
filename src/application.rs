use std::cell::RefCell;
use std::rc::Rc;

use gtk::gio;
use gtk::glib;
use gtk::prelude::*;

use adw::prelude::*;

use crate::engine::{MidiEngine, State};

const TICK_INTERVAL_MS: u32 = 20;

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

fn on_activate(app: &adw::Application, engine: Rc<RefCell<MidiEngine>>) {
    let builder = gtk::Builder::from_string(include_str!("../ui/window.ui"));

    let window: adw::ApplicationWindow = builder
        .object("window_main")
        .expect("Failed to get window_main");
    window.set_application(Some(app));

    let toast_overlay: adw::ToastOverlay = builder
        .object("toast_overlay")
        .expect("Failed to get toast_overlay");
    let main_stack: gtk::Stack = builder
        .object("main_stack")
        .expect("Failed to get main_stack");
    let info_sheet: adw::BottomSheet = builder
        .object("info_sheet")
        .expect("Failed to get info_sheet");
    let label_info: gtk::Label = builder
        .object("label_info")
        .expect("Failed to get label_info");
    let label_name: gtk::Label = builder
        .object("label_name")
        .expect("Failed to get label_name");
    let label_position: gtk::Label = builder
        .object("label_position")
        .expect("Failed to get label_position");
    let label_length: gtk::Label = builder
        .object("label_length")
        .expect("Failed to get label_length");
    let progress_bar: gtk::ProgressBar = builder
        .object("progress_bar")
        .expect("Failed to get progress_bar");
    let btn_start_stop: gtk::Button = builder
        .object("button_start_stop")
        .expect("Failed to get button_start_stop");
    let btn_info: gtk::Button = builder
        .object("button_info")
        .expect("Failed to get button_info");
    let btn_info_close: gtk::Button = builder
        .object("button_info_close")
        .expect("Failed to get button_info_close");
    let btn_open: gtk::Button = builder
        .object("button_open")
        .expect("Failed to get button_open");
    let btn_stop: gtk::Button = builder
        .object("button_stop")
        .expect("Failed to get button_stop");
    let btn_backward: gtk::Button = builder
        .object("button_backward")
        .expect("Failed to get button_backward");
    let btn_forward: gtk::Button = builder
        .object("button_forward")
        .expect("Failed to get button_forward");

    // ── Port model / dropdown ──
    let port_model = gtk::StringList::new(&[]);
    let port_dropdown = gtk::DropDown::new(Some(port_model.clone()), None::<&gtk::Expression>);

    let populate_ports = {
        let port_model = port_model.clone();
        move || {
            let ports = MidiEngine::list_ports();
            port_model.splice(0, port_model.n_items(), &ports.iter().map(|s| &**s).collect::<Vec<_>>());
            ports
        }
    };

    // ── Actions (menu) ──
    let about_action = gio::SimpleAction::new("about", None);
    let win = window.clone();
    about_action.connect_activate(move |_, _| show_about(&win));
    app.add_action(&about_action);

    let port_action = gio::SimpleAction::new("port-settings", None);
    let win = window.clone();
    let engine_for_dialog = engine.clone();
    let populate = populate_ports.clone();
    let populate_refresh = populate_ports.clone();
    let port_dropdown_for_dialog = port_dropdown.clone();
    port_action.connect_activate(move |_, _| {
        let dialog = adw::AlertDialog::builder()
            .title("Port settings")
            .body("Select MIDI output port:")
            .build();
        dialog.add_response("close", "Close");
        dialog.set_default_response(Some("close"));

        let ports = populate();
        if !ports.is_empty() {
            let current = engine_for_dialog.borrow();
            let current = current.port_name();
            if let Some(name) = &current {
                if let Some(i) = ports.iter().position(|p| p == name) {
                    port_dropdown_for_dialog.set_selected(i as u32);
                }
            } else {
                port_dropdown_for_dialog.set_selected(0);
            }
        }

        let refresh_btn = gtk::Button::builder()
            .icon_name("view-refresh-symbolic")
            .tooltip_text("Refresh ports")
            .build();

        {
            let ports_reload = populate_refresh.clone();
            let dd = port_dropdown_for_dialog.clone();
            refresh_btn.connect_clicked(move |_| {
                let ports = ports_reload();
                if !ports.is_empty() {
                    dd.set_selected(0);
                }
            });
        }

        let box_ = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        box_.append(&port_dropdown_for_dialog);
        box_.append(&refresh_btn);
        dialog.set_extra_child(Some(&box_));
        dialog.present(Some(&win));
    });
    app.add_action(&port_action);

    {
        let engine = engine.clone();
        port_dropdown.connect_selected_notify(move |dd| {
            let pos = dd.selected();
            if pos != u32::MAX {
                if let Some(name) = port_model.string(pos) {
                    let _ = engine.borrow_mut().open_port(name.as_ref());
                }
            }
        });
    }

    // ── File dialog ──
    let file_dialog = gtk::FileDialog::new();
    let midi_filter = gtk::FileFilter::new();
    midi_filter.set_name(Some("MIDI files"));
    midi_filter.add_pattern("*.mid");
    midi_filter.add_pattern("*.midi");
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
        let file_dialog = file_dialog.clone();
        let engine = engine.clone();
        let window = window.clone();
        let label_name = label_name.clone();
        let main_stack = main_stack.clone();
        let toast_overlay = toast_overlay.clone();

        btn_open.connect_clicked(move |_| {
            let file_dialog = file_dialog.clone();
            let engine = engine.clone();
            let window = window.clone();
            let label_name = label_name.clone();
            let main_stack = main_stack.clone();
            let toast_overlay = toast_overlay.clone();

            glib::MainContext::default().spawn_local(async move {
                if let Ok(file) = file_dialog.open_future(Some(&window)).await {
                    let path = file.path().unwrap_or_default().to_string_lossy().to_string();
                    let mut eng = engine.borrow_mut();
                    match eng.load(&path) {
                        Ok(name) => {
                            eng.play();
                            drop(eng);
                            label_name.set_text(&name);
                            main_stack.set_visible_child_name("main-view");
                        }
                        Err(e) => {
                            drop(eng);
                            toast(&toast_overlay, &e);
                        }
                    }
                }
            });
        });
    }

    {
        let engine = engine.clone();
        let label_info = label_info.clone();
        let info_sheet = info_sheet.clone();
        btn_info.connect_clicked(move |_| {
            let eng = engine.borrow();
            label_info.set_label(&format_info(&eng));
            info_sheet.set_open(true);
        });
    }

    {
        let info_sheet = info_sheet.clone();
        btn_info_close.connect_clicked(move |_| {
            info_sheet.set_open(false);
        });
    }

    {
        let engine = engine.clone();
        btn_start_stop.connect_clicked(move |_| {
            engine.borrow_mut().toggle_play_pause();
        });
    }

    {
        let engine = engine.clone();
        let label_position = label_position.clone();
        let progress_bar = progress_bar.clone();
        btn_stop.connect_clicked(move |_| {
            engine.borrow_mut().stop();
            label_position.set_text("0:00");
            progress_bar.set_fraction(0.0);
        });
    }

    {
        let engine = engine.clone();
        btn_backward.connect_clicked(move |_| {
            engine.borrow_mut().seek(0.0);
        });
    }

    {
        let engine = engine.clone();
        btn_forward.connect_clicked(move |_| {
            let total = engine.borrow().total_length();
            engine.borrow_mut().seek(total);
        });
    }

    // ── Keyboard shortcut: Space ──
    {
        let engine = engine.clone();
        let controller = gtk::ShortcutController::new();
        let shortcut = gtk::Shortcut::new(
            gtk::ShortcutTrigger::parse_string("space"),
            Some(gtk::CallbackAction::new(move |_, _| {
                engine.borrow_mut().toggle_play_pause();
                glib::Propagation::Proceed
            })),
        );
        controller.add_shortcut(shortcut);
        window.add_controller(controller);
    }

    // ── Close request ──
    {
        let engine = engine.clone();
        window.connect_close_request(move |_| {
            engine.borrow_mut().stop();
            glib::Propagation::Proceed
        });
    }

    // ── Refresh ports and select first one ──
    {
        let ports = populate_ports();
        if !ports.is_empty() {
            let current = engine.borrow();
            let current = current.port_name();
            if let Some(name) = &current {
                if let Some(i) = ports.iter().position(|p| p == name) {
                    port_dropdown.set_selected(i as u32);
                }
            } else {
                port_dropdown.set_selected(0);
            }
        }
    }

    // ── Tick loop ──
    start_tick_loop(
        &engine,
        &btn_start_stop,
        &progress_bar,
        &label_position,
        &label_length,
    );

    window.present();
}

fn start_tick_loop(
    engine: &Rc<RefCell<MidiEngine>>,
    btn_start_stop: &gtk::Button,
    progress_bar: &gtk::ProgressBar,
    label_position: &gtk::Label,
    label_length: &gtk::Label,
) {
    let engine = engine.clone();
    let btn_ss = btn_start_stop.clone();
    let pb = progress_bar.clone();
    let lp = label_position.clone();
    let ll = label_length.clone();

    glib::timeout_add_local(
        std::time::Duration::from_millis(TICK_INTERVAL_MS.into()),
        move || {
            let mut eng = engine.borrow_mut();
            let state = eng.state();

            if state == State::Playing {
                let due = eng.tick();
                for ev in &due {
                    eng.send_event(ev);
                }

                update_progress(&eng, &pb, &lp, &ll);
                update_transport_button(&eng, &btn_ss);

                if eng.state() != State::Playing {
                    eng.stop();
                }
            }

            glib::ControlFlow::Continue
        },
    );
}

fn update_progress(engine: &MidiEngine, progress_bar: &gtk::ProgressBar, label_position: &gtk::Label, label_length: &gtk::Label) {
    let elapsed = engine.elapsed();
    let total = engine.total_length();
    if total > 0.0 {
        progress_bar.set_fraction(elapsed / total);
    }
    label_position.set_text(&format_time(elapsed));
    label_length.set_text(&format_time(total));
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
        .version("0.1.0")
        .developer_name("Eason20000")
        .license_type(gtk::License::Gpl30)
        .build();
    dialog.present(Some(window));
}

fn toast(overlay: &adw::ToastOverlay, message: &str) {
    let toast = adw::Toast::new(message);
    toast.set_timeout(3);
    overlay.add_toast(toast);
}
