//! Platform defaults that are genuinely platform-specific.
//!
//! Keep this small: the synth worker, event mapping and GeneralUser GS
//! settings are shared by every target. Only the audio backend names and the
//! mobile voice cap differ here.

use std::ffi::c_int;

pub(crate) fn audio_driver_candidates() -> &'static [&'static str] {
    #[cfg(target_os = "linux")]
    {
        // PulseAudio works with both PulseAudio and PipeWire's pulse server.
        // ALSA is the fallback for bare systems. The native PipeWire driver
        // needs the application to call pw_init(), which is not worth the
        // extra dependency here.
        &["pulseaudio", "alsa"]
    }
    #[cfg(target_os = "macos")]
    {
        &["coreaudio"]
    }
    #[cfg(target_os = "windows")]
    {
        &["wasapi", "dsound", "waveout"]
    }
    #[cfg(target_os = "android")]
    {
        &["oboe", "opensles"]
    }
    #[cfg(not(any(
        target_os = "linux",
        target_os = "macos",
        target_os = "windows",
        target_os = "android"
    )))]
    {
        &[]
    }
}

pub(crate) fn default_polyphony() -> c_int {
    #[cfg(target_os = "android")]
    {
        // GeneralUser GS recommends 256 voices on mobile.
        256
    }
    #[cfg(not(target_os = "android"))]
    {
        512
    }
}
