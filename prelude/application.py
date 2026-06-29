import sys
from pathlib import Path

import gi
gi.require_version("Gtk", "4.0")
gi.require_version("Adw", "1")
from gi.repository import Gtk, Adw, GLib, Gio

from .midi_engine import MidiEngine


class PreludeApplication(Adw.Application):
    def __init__(self, **kwargs):
        super().__init__(**kwargs)
        self.set_application_id("top.vikasmi.Prelude")
        self.connect("activate", self.on_activate)

        self.engine = MidiEngine()
        self._connect_engine_signals()

        self._setup_actions()
        self._setup_file_filters()
        self._setup_about()

    # ── engine signals ─────────────────────────────────────────

    def _connect_engine_signals(self):
        self.engine.connect(
            "position-changed", self._on_position_changed
        )
        self.engine.connect(
            "state-changed", self._on_state_changed
        )
        self.engine.connect(
            "file-loaded", self._on_file_loaded
        )
        self.engine.connect("error", self._on_error)

    def _on_position_changed(self, _engine, current, total):
        self._update_progress(current, total)

    def _on_state_changed(self, _engine, state):
        self._update_transport_button(state)
        if state == MidiEngine.STATE_STOPPED:
            self._update_progress(0.0, self.engine.total_length)

    def _on_file_loaded(self, _engine, filename, length):
        self._label_name.set_text(filename)
        self._update_progress(0.0, length)
        self._main_stack.set_visible_child_name("main-view")

    def _on_error(self, _engine, message):
        toast = Adw.Toast.new(message)
        toast.set_timeout(3)
        self._toast_overlay.add_toast(toast)

    # ── activation ──────────────────────────────────────────────

    def on_activate(self, app):
        builder = Gtk.Builder()
        ui_path = self._find_ui_file("window.ui")
        builder.add_from_file(str(ui_path))

        self._toast_overlay = builder.get_object("toast_overlay")
        self._main_stack = builder.get_object("main_stack")
        self._info_sheet = builder.get_object("info_sheet")
        self._label_info = builder.get_object("label_info")
        self._label_name = builder.get_object("label_name")
        self._label_position = builder.get_object("label_position")
        self._label_length = builder.get_object("label_length")
        self._progress_bar = builder.get_object("progress_bar")

        btn_info = builder.get_object("button_info")
        btn_info.connect("clicked", self._on_show_info)

        btn_info_close = builder.get_object("button_info_close")
        btn_info_close.connect("clicked", lambda _b: self._info_sheet.set_open(False))

        btn_open = builder.get_object("button_open")
        btn_open.connect("clicked", self._on_open)

        self._port_model = Gtk.StringList.new([])
        self._port_dropdown = Gtk.DropDown(
            model=self._port_model, enable_search=False
        )
        self._port_dropdown.connect(
            "notify::selected", self._on_port_selected
        )

        self._btn_start_stop = builder.get_object(
            "button_start_stop"
        )
        self._btn_start_stop.connect(
            "clicked", lambda _b: self.engine.toggle_play_pause()
        )

        btn_backward = builder.get_object("button_backward")
        btn_backward.connect("clicked", self._on_backward)

        btn_forward = builder.get_object("button_forward")
        btn_forward.connect("clicked", self._on_forward)

        btn_stop = builder.get_object("button_stop")
        btn_stop.connect("clicked", lambda _b: self.engine.stop())

        self.win = builder.get_object("window_main")
        self.win.set_application(self)

        self._setup_keyboard_shortcuts()

        self.win.present()

        self._refresh_ports()

    def _setup_actions(self):
        about_action = Gio.SimpleAction.new("about", None)
        about_action.connect("activate", lambda *_: self._on_menu())
        self.add_action(about_action)

        port_action = Gio.SimpleAction.new("port-settings", None)
        port_action.connect("activate", lambda *_: self._on_port_settings())
        self.add_action(port_action)

    def _setup_file_filters(self):
        self._dialog_file = Gtk.FileDialog.new()
        midi_filter = Gtk.FileFilter()
        midi_filter.set_name("MIDI files")
        midi_filter.add_pattern("*.mid")
        midi_filter.add_pattern("*.midi")
        all_filter = Gtk.FileFilter()
        all_filter.set_name("All files")
        all_filter.add_pattern("*")
        filters = Gio.ListStore.new(Gtk.FileFilter)
        filters.append(midi_filter)
        filters.append(all_filter)
        self._dialog_file.set_filters(filters)
        self._dialog_file.set_default_filter(midi_filter)

    def _setup_about(self):
        self._dialog_about = Adw.AboutDialog(
            application_name="Prelude",
            application_icon="application-graphics",
            version="0.1.0",
            developer_name="Eason20000",
            license_type=Gtk.License(Gtk.License.GPL_3_0),
        )

    def _setup_keyboard_shortcuts(self):
        shortcuts = Gtk.ShortcutController()
        space = Gtk.Shortcut(
            trigger=Gtk.ShortcutTrigger.parse_string("space"),
            action=Gtk.CallbackAction.new(
                lambda *_: self.engine.toggle_play_pause()
            ),
        )
        shortcuts.add_shortcut(space)
        self.win.add_controller(shortcuts)

    # ── file dialog ─────────────────────────────────────────────

    def _on_open(self, _button):
        self._dialog_file.open(self.win, None, self._on_file_selected)

    def _on_file_selected(self, dialog, result):
        try:
            file = dialog.open_finish(result)
            if file is None:
                return
            path = file.get_path()
            if path is not None and self.engine.load(path):
                self.engine.play()
        except GLib.GError:
            pass

    # ── transport ───────────────────────────────────────────────

    def _on_backward(self, _button):
        self.engine.seek(0.0)

    def _on_forward(self, _button):
        pass

    # ── info sheet ──────────────────────────────────────────────

    def _on_show_info(self, _button):
        self._label_info.set_label(
            f"File: {self._label_name.get_text()}\n"
            f"Length: {self._label_length.get_text()}\n"
            f"MIDI format: ?\nTracks: ?"
        )
        self._info_sheet.set_open(True)

    # ── menu ────────────────────────────────────────────────────

    def _on_menu(self):
        win = self.get_active_window()
        if win is not None:
            self._dialog_about.present(win)

    def _on_port_settings(self):
        win = self.get_active_window()
        if win is None:
            return
        dialog = Adw.AlertDialog(
            title="Port settings",
            body="Select MIDI output port:",
        )

        refresh_btn = Gtk.Button(
            icon_name="view-refresh-symbolic",
            tooltip_text="Refresh ports",
        )
        refresh_btn.connect("clicked", lambda _b: self._refresh_ports())

        box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=6)
        box.append(self._port_dropdown)
        box.append(refresh_btn)

        dialog.set_extra_child(box)
        dialog.add_response("close", "Close")
        dialog.set_default_response("close")
        dialog.present(win)

    # ── progress ────────────────────────────────────────────────

    def _update_progress(self, current, total):
        if total > 0:
            self._progress_bar.set_fraction(current / total)
        else:
            self._progress_bar.set_fraction(0.0)
        self._label_position.set_text(self._format_time(current))
        self._label_length.set_text(self._format_time(total))

    @staticmethod
    def _format_time(seconds: float) -> str:
        mins = int(seconds // 60)
        secs = int(seconds % 60)
        return f"{mins}:{secs:02d}"

    def _update_transport_button(self, state):
        content = self._btn_start_stop.get_first_child()
        if state == MidiEngine.STATE_PLAYING:
            content.set_icon_name("media-playback-pause-symbolic")
        else:
            content.set_icon_name("media-playback-start-symbolic")

    # ── MIDI port management ────────────────────────────────────

    def _refresh_ports(self):
        ports = MidiEngine.list_output_ports()
        self._port_model.splice(0, self._port_model.get_n_items(), [])

        if not ports:
            self._port_model.append("(no ports found)")
            self._port_dropdown.set_sensitive(False)
            return

        self._port_dropdown.set_sensitive(True)
        for name in ports:
            self._port_model.append(name)

        selected = self._port_dropdown.get_selected()
        if selected == GTK_INVALID_LIST_ITEM and ports:
            self._port_dropdown.set_selected(0)
            self._on_port_chosen(ports[0])

    def _on_port_selected(self, dropdown, _pspec):
        pos = dropdown.get_selected()
        if pos == GTK_INVALID_LIST_ITEM:
            return
        name = self._port_model.get_string(pos)
        self._on_port_chosen(name)

    def _on_port_chosen(self, name):
        if self.engine.port_name != name:
            self.engine.open_port(name)

    # ── resource path ───────────────────────────────────────────

    @staticmethod
    def _find_ui_file(name: str) -> Path:
        return Path(__file__).resolve().parent / "ui" / name


GTK_INVALID_LIST_ITEM = getattr(Gtk, 'INVALID_LIST_ITEM', 0xFFFFFFFF)
