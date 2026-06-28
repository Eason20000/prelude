import threading
import time
from pathlib import Path

import gi
gi.require_version("Gtk", "4.0")
gi.require_version("Adw", "1")
from gi.repository import GLib, GObject

import mido


class MidiEngine(GObject.GObject):
    __gsignals__ = {
        "position-changed": (
            GObject.SIGNAL_RUN_FIRST,
            None,
            (float, float),
        ),
        "state-changed": (
            GObject.SIGNAL_RUN_FIRST,
            None,
            (str,),
        ),
        "file-loaded": (
            GObject.SIGNAL_RUN_FIRST,
            None,
            (str, float),
        ),
        "error": (GObject.SIGNAL_RUN_FIRST, None, (str,)),
        "device-list-changed": (
            GObject.SIGNAL_RUN_FIRST,
            None,
            (object,),
        ),
    }

    STATE_STOPPED = "stopped"
    STATE_PLAYING = "playing"
    STATE_PAUSED = "paused"

    def __init__(self):
        super().__init__()
        self._midi: mido.MidiFile | None = None
        self._port: mido.ports.BaseOutput | None = None
        self._port_name: str | None = None
        self._thread: threading.Thread | None = None
        self._stop_ev = threading.Event()
        self._pause_ev = threading.Event()
        self._seek_position = 0.0
        self._current_time = 0.0
        self._total_length = 0.0
        self._file_path: str | None = None
        self._state = self.STATE_STOPPED

    # ── public API ──────────────────────────────────────────────

    def load(self, path: str) -> bool:
        self.stop()

        if not Path(path).exists():
            self._emit_signal("error", f"File not found: {path}")
            return False

        try:
            self._midi = mido.MidiFile(path)
            self._total_length = self._midi.length
            self._current_time = 0.0
            self._file_path = path
            self._seek_position = 0.0

            self._emit_signal(
                "file-loaded",
                Path(path).name,
                self._total_length,
            )
            return True

        except Exception as e:
            self._midi = None
            self._emit_signal("error", f"Cannot open MIDI file: {e}")
            return False

    def play(self):
        if self._state == self.STATE_PAUSED:
            self._pause_ev.set()
            self._set_state(self.STATE_PLAYING)
            return

        if self._midi is None or self._state == self.STATE_PLAYING:
            return

        self._stop_ev.clear()
        self._pause_ev.set()
        self._thread = threading.Thread(
            target=self._playback_loop, daemon=True
        )
        self._thread.start()
        self._set_state(self.STATE_PLAYING)

    def pause(self):
        if self._state == self.STATE_PLAYING:
            self._pause_ev.clear()
            self._set_state(self.STATE_PAUSED)

    def toggle_play_pause(self):
        if self._state == self.STATE_PLAYING:
            self.pause()
        elif self._state == self.STATE_PAUSED:
            self.play()
        else:
            self.play()

    def stop(self):
        self._stop_ev.set()
        self._pause_ev.set()
        if self._thread and self._thread.is_alive():
            self._thread.join(timeout=2.0)
        self._current_time = 0.0
        self._seek_position = 0.0
        self._set_state(self.STATE_STOPPED)

    def seek(self, position: float):
        if self._midi is None:
            return
        position = max(0.0, min(position, self._total_length))
        was_playing = self._state == self.STATE_PLAYING
        self._seek_position = position
        self._stop_ev.set()
        self._pause_ev.set()
        if self._thread and self._thread.is_alive():
            self._thread.join(timeout=2.0)
        self._current_time = position
        if was_playing:
            self._stop_ev.clear()
            self._pause_ev.set()
            self._thread = threading.Thread(
                target=self._playback_loop, daemon=True
            )
            self._thread.start()
            self._set_state(self.STATE_PLAYING)
        else:
            self._emit_position()

    def open_port(self, port_name: str) -> bool:
        try:
            if self._port is not None:
                self._port.close()
            self._port = mido.open_output(port_name)
            self._port_name = port_name
            return True
        except Exception as e:
            self._emit_signal("error", f"Cannot open MIDI port: {e}")
            return False

    def close_port(self):
        if self._port is not None:
            self._port.close()
            self._port = None
            self._port_name = None

    @property
    def state(self):
        return self._state

    @property
    def current_time(self):
        return self._current_time

    @property
    def total_length(self):
        return self._total_length

    @property
    def file_path(self):
        return self._file_path

    @property
    def port_name(self):
        return self._port_name

    # ── internal: playback thread ───────────────────────────────

    def _playback_loop(self):
        try:
            if self._midi is None:
                return

            if self._port is None:
                self._emit_signal(
                    "error", "No MIDI output port selected"
                )
                return

            self._fast_forward_to(self._seek_position)

            for msg in self._midi:
                if self._stop_ev.is_set():
                    return

                self._wait_while_paused()

                if self._stop_ev.is_set():
                    return

                if msg.time > 0:
                    self._sleep_with_events(msg.time)
                    if self._stop_ev.is_set():
                        return

                if not msg.is_meta:
                    try:
                        self._port.send(msg)
                    except Exception:
                        self._emit_signal(
                            "error", "MIDI port disconnected"
                        )
                        return

                self._current_time += msg.time
                self._emit_position()

        except Exception as e:
            self._emit_signal("error", f"Playback error: {e}")

        finally:
            self._set_state(self.STATE_STOPPED)

    def _fast_forward_to(self, position: float):
        if position <= 0:
            return
        accumulated = 0.0
        for msg in self._midi:
            if self._stop_ev.is_set():
                return
            accumulated += msg.time
            if accumulated >= position:
                diff = accumulated - position
                msg.time = diff
                self._seek_position = msg.time
                return

    def _sleep_with_events(self, seconds: float):
        chunk = 0.05
        while seconds > 0 and not self._stop_ev.is_set():
            self._wait_while_paused()
            if self._stop_ev.is_set():
                return
            sleep_for = min(seconds, chunk)
            time.sleep(sleep_for)
            seconds -= sleep_for
            self._current_time += sleep_for
            self._emit_position()

    def _wait_while_paused(self):
        while not self._pause_ev.is_set():
            if self._stop_ev.is_set():
                return
            time.sleep(0.05)

    # ── helpers ─────────────────────────────────────────────────

    def _set_state(self, state: str):
        self._state = state
        self._emit_signal("state-changed", state)

    def _emit_position(self):
        self._emit_signal(
            "position-changed",
            self._current_time,
            self._total_length,
        )

    def _emit_signal(self, name: str, *args):
        GLib.idle_add(self.emit, name, *args)

    @staticmethod
    def list_output_ports():
        try:
            return mido.get_output_names()
        except Exception:
            return []

    def refresh_port(self) -> bool:
        if self._port_name is None:
            return False
        return self.open_port(self._port_name)
