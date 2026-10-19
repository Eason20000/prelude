//! FluidSynth backend.
//!
//! All FluidSynth handles live on one worker thread. The UI only owns a
//! command sender, so no raw pointer ever crosses the GTK main thread and the
//! 30 MB GeneralUser GS load cannot block the UI.
//!
//! The settings below are the GeneralUser GS upstream recipe. They are
//! deliberately not user-configurable: the soundfont is tuned for this exact
//! engine configuration and exposing it would only let people make it worse.

use std::ffi::{c_char, c_double, c_int};
use std::path::PathBuf;
use std::ptr;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

use crate::engine::MidiEvent;
use crate::platform;

const SOUNDFONT_NAME: &str = "GeneralUser GS";
const SOUNDFONT_GAIN: f64 = 0.5;
const REVERB_DAMP: f64 = 0.3;
const REVERB_LEVEL: f64 = 0.7;
const REVERB_ROOM_SIZE: f64 = 0.5;
const REVERB_WIDTH: f64 = 0.8;
const CHORUS_DEPTH: f64 = 3.6;
const CHORUS_LEVEL: f64 = 0.55;
const CHORUS_NR: c_int = 4;
const CHORUS_SPEED: f64 = 0.36;
const AUDIO_PERIOD_SIZE: c_int = 64;
const AUDIO_PERIODS: c_int = 16;

const FLUID_OK: c_int = 0;

#[repr(C)]
struct fluid_settings_t {
    _private: [u8; 0],
}

#[repr(C)]
struct fluid_synth_t {
    _private: [u8; 0],
}

#[repr(C)]
struct fluid_audio_driver_t {
    _private: [u8; 0],
}

// Link flags come from build.rs (pkg-config); do not add #[link] here: on
// Windows the import library is libfluidsynth-3.dll.a, not libfluidsynth.a.
extern "C" {
    fn new_fluid_settings() -> *mut fluid_settings_t;
    fn delete_fluid_settings(settings: *mut fluid_settings_t);
    fn fluid_settings_setint(
        settings: *mut fluid_settings_t,
        name: *const c_char,
        value: c_int,
    ) -> c_int;
    fn fluid_settings_setnum(
        settings: *mut fluid_settings_t,
        name: *const c_char,
        value: c_double,
    ) -> c_int;
    fn fluid_settings_setstr(
        settings: *mut fluid_settings_t,
        name: *const c_char,
        value: *const c_char,
    ) -> c_int;

    fn new_fluid_synth(settings: *mut fluid_settings_t) -> *mut fluid_synth_t;
    fn delete_fluid_synth(synth: *mut fluid_synth_t);
    fn fluid_synth_sfload(
        synth: *mut fluid_synth_t,
        filename: *const c_char,
        reset_presets: c_int,
    ) -> c_int;

    fn new_fluid_audio_driver(
        settings: *mut fluid_settings_t,
        synth: *mut fluid_synth_t,
    ) -> *mut fluid_audio_driver_t;
    fn delete_fluid_audio_driver(driver: *mut fluid_audio_driver_t);

    fn fluid_synth_noteon(synth: *mut fluid_synth_t, chan: c_int, key: c_int, vel: c_int) -> c_int;
    fn fluid_synth_noteoff(synth: *mut fluid_synth_t, chan: c_int, key: c_int) -> c_int;
    fn fluid_synth_cc(synth: *mut fluid_synth_t, chan: c_int, ctrl: c_int, val: c_int) -> c_int;
    fn fluid_synth_program_change(synth: *mut fluid_synth_t, chan: c_int, program: c_int) -> c_int;
    fn fluid_synth_pitch_bend(synth: *mut fluid_synth_t, chan: c_int, val: c_int) -> c_int;
    fn fluid_synth_key_pressure(
        synth: *mut fluid_synth_t,
        chan: c_int,
        key: c_int,
        val: c_int,
    ) -> c_int;
    fn fluid_synth_channel_pressure(synth: *mut fluid_synth_t, chan: c_int, val: c_int) -> c_int;
    fn fluid_synth_sysex(
        synth: *mut fluid_synth_t,
        data: *const c_char,
        len: c_int,
        response: *mut c_char,
        response_len: *mut c_int,
        handled: *mut c_int,
        dryrun: c_int,
    ) -> c_int;
    fn fluid_synth_all_notes_off(synth: *mut fluid_synth_t, chan: c_int) -> c_int;
    fn fluid_synth_all_sounds_off(synth: *mut fluid_synth_t, chan: c_int) -> c_int;
    fn fluid_synth_system_reset(synth: *mut fluid_synth_t) -> c_int;

    fn fluid_version(major: *mut c_int, minor: *mut c_int, micro: *mut c_int);
    fn fluid_version_str() -> *const c_char;
}

#[derive(Debug, Clone)]
pub(crate) enum SynthCommand {
    Event(MidiEvent),
    Reset,
    AllNotesOff,
    SuspendAudio,
    ResumeAudio,
    Shutdown,
}

#[derive(Debug, Clone)]
pub(crate) enum SynthEvent {
    Ready { version: String },
    Error(String),
    AudioSuspended,
    AudioResumed,
}

pub(crate) struct SynthHandle {
    tx: Sender<SynthCommand>,
}

impl SynthHandle {
    pub(crate) fn send(&self, command: SynthCommand) -> bool {
        self.tx.send(command).is_ok()
    }
}

/// Start the FluidSynth worker. The returned receiver carries ready/error and
/// audio suspend notifications; the caller polls it from the GTK loop.
pub(crate) fn spawn(
    soundfont: PathBuf,
    polyphony: c_int,
) -> Result<(SynthHandle, Receiver<SynthEvent>), String> {
    let (command_tx, command_rx) = mpsc::channel();
    let (event_tx, event_rx) = mpsc::channel();

    thread::Builder::new()
        .name("prelude-synth".to_string())
        .spawn(move || match Worker::new(&soundfont, polyphony) {
            Ok(mut worker) => {
                let _ = event_tx.send(SynthEvent::Ready {
                    version: worker.version.clone(),
                });
                worker.run(command_rx, event_tx);
            }
            Err(error) => {
                let _ = event_tx.send(SynthEvent::Error(error));
            }
        })
        .map_err(|error| format!("Failed to start audio engine: {error}"))?;

    Ok((SynthHandle { tx: command_tx }, event_rx))
}

struct Worker {
    settings: *mut fluid_settings_t,
    synth: *mut fluid_synth_t,
    driver: Option<*mut fluid_audio_driver_t>,
    version: String,
}

impl Worker {
    fn new(soundfont: &std::path::Path, polyphony: c_int) -> Result<Self, String> {
        // SAFETY: all pointers are checked for null before use; settings and
        // synth are owned by this worker and released in Drop.
        unsafe {
            let settings = new_fluid_settings();
            if settings.is_null() {
                return Err("Failed to create FluidSynth settings".to_string());
            }

            let mut major = 0;
            let mut minor = 0;
            let mut micro = 0;
            fluid_version(&mut major, &mut minor, &mut micro);

            if let Err(error) = configure_settings(settings, polyphony, major, minor) {
                delete_fluid_settings(settings);
                return Err(error);
            }

            let synth = new_fluid_synth(settings);
            if synth.is_null() {
                delete_fluid_settings(settings);
                return Err("Failed to create FluidSynth synthesizer".to_string());
            }

            let soundfont_c = match std::ffi::CString::new(soundfont.to_string_lossy().as_bytes()) {
                Ok(path) => path,
                Err(_) => {
                    delete_fluid_synth(synth);
                    delete_fluid_settings(settings);
                    return Err("SoundFont path contains a NUL byte".to_string());
                }
            };
            let sfid = fluid_synth_sfload(synth, soundfont_c.as_ptr(), 1);
            if sfid < 0 {
                delete_fluid_synth(synth);
                delete_fluid_settings(settings);
                return Err(format!(
                    "Failed to load {SOUNDFONT_NAME} from {}",
                    soundfont.display()
                ));
            }
            // Start every song from a clean GM/GS channel state.
            fluid_synth_system_reset(synth);

            let version = version_string(major, minor, micro);
            let mut worker = Self {
                settings,
                synth,
                driver: None,
                version,
            };

            worker.driver = Some(worker.create_audio_driver()?);
            Ok(worker)
        }
    }

    fn run(&mut self, commands: Receiver<SynthCommand>, events: Sender<SynthEvent>) {
        loop {
            match commands.recv() {
                Ok(SynthCommand::Event(event)) => self.send_event(&event),
                Ok(SynthCommand::Reset) => self.reset(),
                Ok(SynthCommand::AllNotesOff) => self.all_notes_off(),
                Ok(SynthCommand::SuspendAudio) => {
                    self.suspend_audio();
                    let _ = events.send(SynthEvent::AudioSuspended);
                }
                Ok(SynthCommand::ResumeAudio) => match self.resume_audio() {
                    Ok(()) => {
                        let _ = events.send(SynthEvent::AudioResumed);
                    }
                    Err(error) => {
                        let _ = events.send(SynthEvent::Error(error));
                    }
                },
                Ok(SynthCommand::Shutdown) | Err(_) => break,
            }
        }
    }

    fn create_audio_driver(&self) -> Result<*mut fluid_audio_driver_t, String> {
        let candidates = platform::audio_driver_candidates();
        let mut errors = Vec::new();

        for driver in candidates {
            if let Err(error) = set_str(self.settings, "audio.driver", driver) {
                errors.push(format!("{driver}: {error}"));
                continue;
            }
            // SAFETY: settings and synth are valid for the lifetime of self.
            let pointer = unsafe { new_fluid_audio_driver(self.settings, self.synth) };
            if !pointer.is_null() {
                return Ok(pointer);
            }
            errors.push(format!("{driver}: driver did not start"));
        }

        Err(format!("No audio output available ({})", errors.join("; ")))
    }

    fn suspend_audio(&mut self) {
        if let Some(driver) = self.driver.take() {
            // SAFETY: driver was created by create_audio_driver and is only
            // deleted here or in Drop.
            unsafe { delete_fluid_audio_driver(driver) };
        }
    }

    fn resume_audio(&mut self) -> Result<(), String> {
        if self.driver.is_none() {
            self.driver = Some(self.create_audio_driver()?);
        }
        Ok(())
    }

    fn all_notes_off(&self) {
        // SAFETY: synth is valid for the worker lifetime.
        unsafe {
            fluid_synth_all_notes_off(self.synth, -1);
            fluid_synth_all_sounds_off(self.synth, -1);
        }
    }

    fn reset(&self) {
        // SAFETY: synth is valid for the worker lifetime.
        unsafe {
            fluid_synth_all_sounds_off(self.synth, -1);
            fluid_synth_system_reset(self.synth);
        }
    }

    fn send_event(&self, event: &MidiEvent) {
        // SAFETY: synth is valid for the worker lifetime; all channel numbers
        // and values originate from a parsed MIDI file (midly validates the
        // 7-bit ranges) and are passed as c_int.
        unsafe {
            match event {
                MidiEvent::NoteOn {
                    channel,
                    key,
                    velocity,
                } => {
                    fluid_synth_noteon(
                        self.synth,
                        *channel as c_int,
                        *key as c_int,
                        *velocity as c_int,
                    );
                }
                MidiEvent::NoteOff { channel, key, .. } => {
                    fluid_synth_noteoff(self.synth, *channel as c_int, *key as c_int);
                }
                MidiEvent::ControlChange {
                    channel,
                    control,
                    value,
                } => {
                    fluid_synth_cc(
                        self.synth,
                        *channel as c_int,
                        *control as c_int,
                        *value as c_int,
                    );
                }
                MidiEvent::ProgramChange { channel, program } => {
                    fluid_synth_program_change(self.synth, *channel as c_int, *program as c_int);
                }
                MidiEvent::PitchBend { channel, value } => {
                    fluid_synth_pitch_bend(self.synth, *channel as c_int, *value as c_int);
                }
                MidiEvent::Aftertouch {
                    channel,
                    key,
                    pressure,
                } => {
                    fluid_synth_key_pressure(
                        self.synth,
                        *channel as c_int,
                        *key as c_int,
                        *pressure as c_int,
                    );
                }
                MidiEvent::ChannelPressure { channel, pressure } => {
                    fluid_synth_channel_pressure(self.synth, *channel as c_int, *pressure as c_int);
                }
                MidiEvent::Sysex(bytes) => {
                    let mut payload = bytes.as_slice();
                    if payload.first() == Some(&0xF0) {
                        payload = &payload[1..];
                    }
                    if payload.last() == Some(&0xF7) {
                        payload = &payload[..payload.len() - 1];
                    }
                    if payload.is_empty() {
                        return;
                    }
                    let mut response_len = 0;
                    let mut handled = 0;
                    fluid_synth_sysex(
                        self.synth,
                        payload.as_ptr().cast(),
                        payload.len() as c_int,
                        ptr::null_mut(),
                        &mut response_len,
                        &mut handled,
                        0,
                    );
                }
            }
        }
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        // Order matters: drivers use the synth, the synth uses the settings.
        // SAFETY: each pointer is owned by this worker and released once.
        unsafe {
            if let Some(driver) = self.driver.take() {
                delete_fluid_audio_driver(driver);
            }
            if !self.synth.is_null() {
                delete_fluid_synth(self.synth);
            }
            if !self.settings.is_null() {
                delete_fluid_settings(self.settings);
            }
        }
    }
}

fn configure_settings(
    settings: *mut fluid_settings_t,
    polyphony: c_int,
    major: c_int,
    minor: c_int,
) -> Result<(), String> {
    set_int(settings, "synth.device-id", 16)?;
    set_int(settings, "synth.polyphony", polyphony)?;
    set_num(settings, "synth.gain", SOUNDFONT_GAIN)?;
    set_str(settings, "synth.midi-bank-select", "gs")?;

    set_int(settings, "synth.reverb.active", 1)?;
    set_num(settings, "synth.reverb.damp", REVERB_DAMP)?;
    set_num(settings, "synth.reverb.level", REVERB_LEVEL)?;
    set_num(settings, "synth.reverb.room-size", REVERB_ROOM_SIZE)?;
    set_num(settings, "synth.reverb.width", REVERB_WIDTH)?;

    // FluidSynth 2.6 changed the default reverb engine to Dattorro. GeneralUser
    // GS's published recipe was tuned on the FDN engine, so pin FDN whenever
    // the runtime supports the selector. Older versions default to FDN.
    if (major, minor) >= (2, 6) {
        if let Err(error) = set_str(settings, "synth.reverb.engine", "fdn") {
            eprintln!("prelude: cannot force FDN reverb engine: {error}");
        }
    }

    set_int(settings, "synth.chorus.active", 1)?;
    set_num(settings, "synth.chorus.depth", CHORUS_DEPTH)?;
    set_num(settings, "synth.chorus.level", CHORUS_LEVEL)?;
    set_int(settings, "synth.chorus.nr", CHORUS_NR)?;
    set_num(settings, "synth.chorus.speed", CHORUS_SPEED)?;

    set_int(settings, "audio.period-size", AUDIO_PERIOD_SIZE)?;
    set_int(settings, "audio.periods", AUDIO_PERIODS)?;

    #[cfg(target_os = "android")]
    {
        // Oboe reconnects after headset/route changes by default; set it
        // explicitly so a future FluidSynth default cannot silently change
        // Android behaviour.
        if let Err(error) = set_str(settings, "audio.oboe.error-recovery-mode", "Reconnect") {
            eprintln!("prelude: cannot configure Oboe recovery mode: {error}");
        }
    }

    Ok(())
}

fn set_int(settings: *mut fluid_settings_t, name: &str, value: c_int) -> Result<(), String> {
    let name = std::ffi::CString::new(name).map_err(|_| "setting name contains NUL".to_string())?;
    // SAFETY: settings is valid and name is a NUL-terminated C string.
    let result = unsafe { fluid_settings_setint(settings, name.as_ptr(), value) };
    if result == FLUID_OK {
        Ok(())
    } else {
        Err(format!("cannot set {}", name.to_string_lossy()))
    }
}

fn set_num(settings: *mut fluid_settings_t, name: &str, value: f64) -> Result<(), String> {
    let name = std::ffi::CString::new(name).map_err(|_| "setting name contains NUL".to_string())?;
    // SAFETY: settings is valid and name is a NUL-terminated C string.
    let result = unsafe { fluid_settings_setnum(settings, name.as_ptr(), value) };
    if result == FLUID_OK {
        Ok(())
    } else {
        Err(format!("cannot set {}", name.to_string_lossy()))
    }
}

fn set_str(settings: *mut fluid_settings_t, name: &str, value: &str) -> Result<(), String> {
    let name = std::ffi::CString::new(name).map_err(|_| "setting name contains NUL".to_string())?;
    let value =
        std::ffi::CString::new(value).map_err(|_| "setting value contains NUL".to_string())?;
    // SAFETY: settings is valid and both strings are NUL-terminated.
    let result = unsafe { fluid_settings_setstr(settings, name.as_ptr(), value.as_ptr()) };
    if result == FLUID_OK {
        Ok(())
    } else {
        Err(format!(
            "cannot set {} to {}",
            name.to_string_lossy(),
            value.to_string_lossy()
        ))
    }
}

fn version_string(major: c_int, minor: c_int, micro: c_int) -> String {
    // SAFETY: fluid_version_str returns a static NUL-terminated string.
    let pointer = unsafe { fluid_version_str() };
    if pointer.is_null() {
        return format!("{major}.{minor}.{micro}");
    }
    // SAFETY: pointer is a valid NUL-terminated string owned by FluidSynth.
    unsafe { std::ffi::CStr::from_ptr(pointer) }
        .to_string_lossy()
        .into_owned()
}
