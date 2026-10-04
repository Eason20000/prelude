use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use midir::{MidiOutput, MidiOutputConnection};
use midly::{MetaMessage, MidiMessage, Timing, TrackEventKind};

const MIDI_CLIENT_NAME: &str = "prelude";

/// A MIDI event that can be sent to a port.
#[derive(Debug, Clone)]
pub(crate) enum MidiEvent {
    NoteOn { channel: u8, key: u8, velocity: u8 },
    NoteOff { channel: u8, key: u8, velocity: u8 },
    ControlChange { channel: u8, control: u8, value: u8 },
    ProgramChange { channel: u8, program: u8 },
    PitchBend { channel: u8, value: u16 },
    Aftertouch { channel: u8, key: u8, pressure: u8 },
    ChannelPressure { channel: u8, pressure: u8 },
    Sysex(Vec<u8>),
}

/// State of the engine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum State {
    Stopped,
    Playing,
    Paused,
}

/// A bar (measure) boundary in seconds, used by the page-turn view.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Measure {
    pub start: f64,
    pub end: f64,
    /// Seconds per quarter note within this measure (tempo-aware).
    pub quarter: f64,
}

/// A played note with its timing, used by the page-turn view.
#[derive(Debug, Clone, Copy)]
pub(crate) struct NoteData {
    pub time: f64,
    pub duration: f64,
    pub midi: u8,
    pub channel: u8,
}

/// A fully parsed song. Built off the GTK main thread by [`MidiEngine::parse_file`]
/// (all fields are `Send`) and swapped into the engine on completion.
pub(crate) struct LoadedSong {
    pub events: Vec<(f64, MidiEvent)>,
    pub total_length: f64,
    pub measures: Vec<Measure>,
    pub notes: Vec<NoteData>,
    pub peaks: Vec<f64>,
    pub file_name: String,
}

/// Sentinel error when a background parse observes cancellation. The caller
/// treats it as silent: the requesting generation is already stale.
pub(crate) const LOAD_CANCELLED: &str = "Loading cancelled";

/// How often the long parse loops poll the cancellation flag.
const CANCEL_POLL_EVERY: usize = 4096;

/// Timing data needed to rebuild the measure map after flattening.
struct SmfTiming {
    tpq: u16,
    tempos: Vec<(u64, u32)>,
    meters: Vec<(u64, u8, u8)>,
}

/// A parsed SMF: sorted event stream, total length and timing data.
struct ParsedSmf {
    events: Vec<(f64, MidiEvent)>,
    total_length: f64,
    timing: SmfTiming,
}

pub(crate) struct MidiEngine {
    events: Vec<(f64, MidiEvent)>,
    next_idx: usize,
    port: Option<MidiOutputConnection>,
    port_name: Option<String>,
    start: Option<Instant>,
    state: State,
    paused_since: Option<Instant>,
    elapsed: f64,
    total_length: f64,
    file_name: String,
    measures: Vec<Measure>,
    notes: Vec<NoteData>,
}

impl MidiEngine {
    pub(crate) fn new() -> Self {
        Self {
            events: Vec::new(),
            next_idx: 0,
            port: None,
            port_name: None,
            start: None,
            state: State::Stopped,
            paused_since: None,
            elapsed: 0.0,
            total_length: 0.0,
            file_name: String::new(),
            measures: Vec::new(),
            notes: Vec::new(),
        }
    }

    // ── Public API ──────────────────────────────────────────────

    // ── Loading ───────────────────────────────────────────────
    // `parse_file` is a pure function: it touches no `self`, so a background
    // thread can run it while the UI thread stays responsive. The caller swaps
    // the result in via `apply_loaded`. `midly::Smf::parse` itself cannot be
    // interrupted, so cancellation is cooperative: it is observed before and
    // after parsing plus at polling points in the long loops below.

    pub(crate) fn parse_file(
        path: &str,
        bins: usize,
        cancel: &AtomicBool,
    ) -> Result<LoadedSong, String> {
        let data = std::fs::read(path).map_err(|e| format!("Failed to read file: {e}"))?;
        if cancel.load(Ordering::Relaxed) {
            return Err(LOAD_CANCELLED.to_string());
        }
        let smf =
            midly::Smf::parse(&data).map_err(|e| format!("Failed to parse MIDI file: {e}"))?;
        if cancel.load(Ordering::Relaxed) {
            return Err(LOAD_CANCELLED.to_string());
        }
        let parsed = Self::flatten(&smf, cancel)?;
        let measures = Self::calculate_measure_map(
            parsed.timing.tpq,
            &parsed.timing.tempos,
            &parsed.timing.meters,
            parsed.total_length,
            cancel,
        )?;
        let notes = Self::note_intervals(&parsed.events, parsed.total_length, cancel)?;
        let peaks = Self::density_from_notes(&notes, parsed.total_length, bins, cancel)?;
        let file_name = std::path::Path::new(path)
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| path.to_string());

        Ok(LoadedSong {
            events: parsed.events,
            total_length: parsed.total_length,
            measures,
            notes,
            peaks,
            file_name,
        })
    }

    /// Swap a background-parsed song in. Resets the transport to Stopped;
    /// the caller decides whether to `play()` immediately.
    pub(crate) fn apply_loaded(&mut self, song: LoadedSong) {
        self.events = song.events;
        self.total_length = song.total_length;
        self.measures = song.measures;
        self.notes = song.notes;
        self.file_name = song.file_name;
        self.next_idx = 0;
        self.elapsed = 0.0;
        self.start = None;
        self.paused_since = None;
        self.state = State::Stopped;
    }

    pub(crate) fn play(&mut self) {
        if self.events.is_empty() || self.state == State::Playing {
            return;
        }

        // Rewind when starting from the end of the file: without this,
        // pressing play at EOF would anchor start = now - total and hit EOF
        // again on the next tick, appearing completely unresponsive.
        if self.total_length > 0.0 && self.elapsed >= self.total_length {
            self.elapsed = 0.0;
            self.next_idx = 0;
        }

        // Anchor playback on the current elapsed — independent of how long we
        // were paused or whether we sought while paused.
        self.start = Some(Instant::now() - Duration::from_secs_f64(self.elapsed));
        self.paused_since = None;
        self.state = State::Playing;
    }

    pub(crate) fn pause(&mut self) {
        if self.state != State::Playing {
            return;
        }
        self.all_notes_off();
        // Snap elapsed to frame precision instead of freezing it at the last
        // 20 ms tick value (up to one tick of drift). Advance next_idx past
        // the skipped span so resume doesn't burst-play the unheard events.
        if let Some(start) = self.start {
            self.elapsed = Instant::now()
                .saturating_duration_since(start)
                .as_secs_f64()
                .min(self.total_length);
            self.next_idx = self.events.partition_point(|(t, _)| *t <= self.elapsed);
        }
        self.paused_since = Some(Instant::now());
        self.state = State::Paused;
    }

    pub(crate) fn toggle_play_pause(&mut self) {
        match self.state {
            State::Playing => self.pause(),
            _ => self.play(),
        }
    }

    pub(crate) fn stop(&mut self) {
        self.start = None;
        self.paused_since = None;
        self.elapsed = 0.0;
        self.next_idx = 0;
        self.state = State::Stopped;
        self.all_notes_off();
    }

    pub(crate) fn seek(&mut self, position: f64) {
        // clamp() lets NaN through and Duration::from_secs_f64 panics on it.
        if !position.is_finite() {
            return;
        }
        let position = position.clamp(0.0, self.total_length);
        self.all_notes_off();

        self.paused_since = None;
        self.elapsed = position;
        self.next_idx = self.events.partition_point(|(t, _)| *t < position);

        match self.state {
            State::Playing => {
                self.start = Some(Instant::now() - Duration::from_secs_f64(position));
            }
            State::Stopped => {
                self.start = None;
            }
            // Paused: nothing to anchor — play() re-anchors from elapsed.
            State::Paused => {}
        }
    }

    /// Returns all MIDI events due up to the current time.
    /// Called from the main (UI) thread.
    pub(crate) fn tick(&mut self) -> Vec<MidiEvent> {
        if self.state != State::Playing {
            return Vec::new();
        }

        let Some(start) = self.start else {
            return Vec::new();
        };

        let new_elapsed = Instant::now()
            .saturating_duration_since(start)
            .as_secs_f64();
        let mut due = Vec::new();

        // Advance through events that are due
        while self.next_idx < self.events.len() {
            let (t, _) = &self.events[self.next_idx];
            if *t > new_elapsed {
                break;
            }
            due.push(self.events[self.next_idx].1.clone());
            self.next_idx += 1;
        }

        self.elapsed = if self.next_idx < self.events.len() {
            new_elapsed
        } else {
            self.state = State::Stopped;
            self.total_length
        };

        due
    }

    // ── Properties ──────────────────────────────────────────────

    pub(crate) fn state(&self) -> State {
        self.state.clone()
    }

    pub(crate) fn elapsed(&self) -> f64 {
        self.elapsed
    }

    pub(crate) fn total_length(&self) -> f64 {
        self.total_length
    }

    pub(crate) fn file_name(&self) -> &str {
        &self.file_name
    }

    pub(crate) fn measures(&self) -> &[Measure] {
        &self.measures
    }

    pub(crate) fn notes(&self) -> &[NoteData] {
        &self.notes
    }

    /// Density histogram over the cached note intervals (shares the pairing
    /// logic with the page view via `note_intervals`; do not fork a second
    /// pairing implementation here).
    fn density_from_notes(
        notes: &[NoteData],
        total_length: f64,
        bins: usize,
        cancel: &AtomicBool,
    ) -> Result<Vec<f64>, String> {
        if notes.is_empty() || total_length <= 0.0 || bins == 0 {
            return Ok(vec![0.0; bins]);
        }

        let bin_width = total_length / bins as f64;
        let mut density = vec![0.0; bins];

        for (i, n) in notes.iter().enumerate() {
            if i.is_multiple_of(CANCEL_POLL_EVERY) && cancel.load(Ordering::Relaxed) {
                return Err(LOAD_CANCELLED.to_string());
            }
            let start_bin = (n.time / bin_width).floor() as isize;
            let end_bin = ((n.time + n.duration) / bin_width).ceil() as isize;
            for b in start_bin.max(0)..end_bin.min(bins as isize) {
                density[b as usize] += 1.0;
            }
        }

        let max_density = density.iter().cloned().fold(0.0_f64, f64::max);
        if max_density > 0.0 {
            for d in &mut density {
                *d /= max_density;
            }
        }

        Ok(density)
    }

    pub(crate) fn list_ports() -> Vec<String> {
        match MidiOutput::new(MIDI_CLIENT_NAME) {
            Ok(midi) => midi
                .ports()
                .iter()
                .filter_map(|p| midi.port_name(p).ok())
                .collect(),
            Err(_) => Vec::new(),
        }
    }

    pub(crate) fn open_port(&mut self, name: &str) -> Result<(), String> {
        self.port = None;

        let midi = MidiOutput::new(MIDI_CLIENT_NAME)
            .map_err(|e| format!("Failed to create MIDI output: {e}"))?;

        let ports = midi.ports();
        let port = ports
            .iter()
            .find_map(|p| {
                midi.port_name(p)
                    .ok()
                    .and_then(|n| (n == name).then_some(p))
            })
            .ok_or_else(|| format!("Port not found: {name}"))?;

        let conn = midi
            .connect(port, "prelude-output")
            .map_err(|e| format!("Failed to connect to port: {e}"))?;

        self.port = Some(conn);
        self.port_name = Some(name.to_string());
        Ok(())
    }

    pub(crate) fn port_name(&self) -> Option<&str> {
        self.port_name.as_deref()
    }

    /// Send a single event through the port if connected.
    pub(crate) fn send_event(&mut self, ev: &MidiEvent) {
        if let Some(ref mut port) = self.port {
            let bytes = encode_event(ev);
            if let Err(e) = port.send(&bytes) {
                eprintln!("MIDI send error: {e}");
            }
        }
    }

    // ── Internal ────────────────────────────────────────────────

    fn all_notes_off(&mut self) {
        if self.port.is_none() {
            return;
        }
        if let Some(ref mut port) = self.port {
            for ch in 0..16u8 {
                for (cc, desc) in [(123, "all notes off"), (120, "all sound off")] {
                    if let Err(e) = port.send(&encode_cc(ch, cc, 0)) {
                        eprintln!("MIDI send error ({desc}): {e}");
                    }
                }
            }
        }
    }

    // ── SMF flattening (adapted from rust-vst3-host/midi_player.rs) ──

    /// Flatten the SMF into a sorted event stream plus the timing data needed
    /// to build the measure map: ticks per quarter, tempo changes (tick domain)
    /// and time signature changes (tick domain).
    fn flatten(smf: &midly::Smf, cancel: &AtomicBool) -> Result<ParsedSmf, String> {
        let tpq = match smf.header.timing {
            Timing::Metrical(t) => t.as_int(),
            Timing::Timecode { .. } => {
                return Err("SMPTE timecode MIDI files are not supported".to_string());
            }
        };
        if tpq == 0 {
            return Err("invalid MIDI timing: ticks per quarter note is 0".to_string());
        }

        // First pass: gather tempo changes, time signatures and raw MIDI events
        let mut tempo_changes: Vec<(u64, u32)> = vec![(0, 500_000)];
        let mut meters: Vec<(u64, u8, u8)> = Vec::new();
        let mut raw: Vec<(u64, MidiEvent)> = Vec::new();

        let mut seen: usize = 0;
        for track in &smf.tracks {
            let mut abs_tick: u64 = 0;
            for ev in track {
                if seen.is_multiple_of(CANCEL_POLL_EVERY) && cancel.load(Ordering::Relaxed) {
                    return Err(LOAD_CANCELLED.to_string());
                }
                seen += 1;
                abs_tick += ev.delta.as_int() as u64;
                match &ev.kind {
                    TrackEventKind::Meta(MetaMessage::Tempo(us)) => {
                        tempo_changes.push((abs_tick, us.as_int()));
                    }
                    TrackEventKind::Meta(MetaMessage::TimeSignature(num, den, _, _)) => {
                        meters.push((abs_tick, *num, *den));
                    }
                    TrackEventKind::Midi { channel, message } => {
                        raw.push((abs_tick, midi_message_to_event(channel.as_int(), message)));
                    }
                    TrackEventKind::SysEx(data) => {
                        let mut sysex = Vec::with_capacity(data.len() + 2);
                        sysex.push(0xF0);
                        sysex.extend_from_slice(data);
                        if sysex.last() != Some(&0xF7) {
                            sysex.push(0xF7);
                        }
                        raw.push((abs_tick, MidiEvent::Sysex(sysex)));
                    }
                    _ => {}
                }
            }
        }

        tempo_changes.sort_by_key(|&(t, _)| t);
        meters.sort_by_key(|&(t, _, _)| t);

        // Second pass: convert ticks to seconds, then sort by time
        let mut events: Vec<(f64, MidiEvent)> = Vec::with_capacity(raw.len());
        for (i, (tick, ev)) in raw.into_iter().enumerate() {
            if i.is_multiple_of(CANCEL_POLL_EVERY) && cancel.load(Ordering::Relaxed) {
                return Err(LOAD_CANCELLED.to_string());
            }
            events.push((Self::seconds_for_tick(tick, tpq, &tempo_changes), ev));
        }

        events.sort_by(|a, b| a.0.total_cmp(&b.0));

        let total_length = events.last().map(|(t, _)| *t).unwrap_or(0.0);
        Ok(ParsedSmf {
            events,
            total_length,
            timing: SmfTiming {
                tpq,
                tempos: tempo_changes,
                meters,
            },
        })
    }

    /// Build the measure map in seconds, honoring tempo and meter changes.
    /// Falls back to 4/4 at 120 BPM when no tempo/meter is present.
    fn calculate_measure_map(
        tpq: u16,
        tempo_changes: &[(u64, u32)],
        meters: &[(u64, u8, u8)],
        total_length: f64,
        cancel: &AtomicBool,
    ) -> Result<Vec<Measure>, String> {
        let mut sorted_meters: Vec<(u64, u8, u8)> = meters.to_vec();
        sorted_meters.sort_by_key(|&(t, _, _)| t);
        if sorted_meters.is_empty() {
            sorted_meters.push((0, 4, 2));
        }
        if sorted_meters[0].0 > 0 {
            sorted_meters.insert(0, (0, 4, 2));
        }

        let mut measures = Vec::new();
        let mut current_tick: u64 = 0;
        let mut current_time: f64 = 0.0;
        let mut meter_idx = 0usize;

        loop {
            if measures.len().is_multiple_of(CANCEL_POLL_EVERY) && cancel.load(Ordering::Relaxed) {
                return Err(LOAD_CANCELLED.to_string());
            }
            while meter_idx + 1 < sorted_meters.len()
                && current_tick >= sorted_meters[meter_idx + 1].0
            {
                meter_idx += 1;
            }
            let (_, num, den_pow) = sorted_meters[meter_idx];
            let num = (num as u64).max(1);
            let den = 1u64 << den_pow.min(16);
            let ticks_per_measure = num * tpq as u64 * 4 / den;
            if ticks_per_measure == 0 {
                // Degenerate timing (e.g. tiny PPQ with a large denominator):
                // don't loop forever appending zero-length measures.
                break;
            }

            let end_tick = current_tick + ticks_per_measure;
            let end_time = Self::seconds_for_tick(end_tick, tpq, tempo_changes).max(current_time);
            let beats = num as f64 * 4.0 / den as f64;

            measures.push(Measure {
                start: current_time,
                end: end_time,
                quarter: (end_time - current_time) / beats,
            });

            current_tick = end_tick;
            current_time = end_time;

            if current_time > total_length + 5.0 {
                break;
            }
        }

        if measures.is_empty() {
            measures.push(Measure {
                start: 0.0,
                end: total_length.max(2.0),
                quarter: total_length.max(2.0) / 4.0,
            });
        }
        Ok(measures)
    }

    /// Pair NoteOn/NoteOff events into note intervals; dangling NoteOns are
    /// extended to the end of the file.
    fn note_intervals(
        events: &[(f64, MidiEvent)],
        total_length: f64,
        cancel: &AtomicBool,
    ) -> Result<Vec<NoteData>, String> {
        let mut pending: Vec<(u8, u8, f64)> = Vec::new();
        let mut notes = Vec::new();

        for (i, (t, ev)) in events.iter().enumerate() {
            if i.is_multiple_of(CANCEL_POLL_EVERY) && cancel.load(Ordering::Relaxed) {
                return Err(LOAD_CANCELLED.to_string());
            }
            match ev {
                MidiEvent::NoteOn {
                    channel,
                    key,
                    velocity,
                } if *velocity > 0 => pending.push((*channel, *key, *t)),
                MidiEvent::NoteOff { channel, key, .. } => {
                    if let Some(pos) = pending
                        .iter()
                        .position(|(ch, k, _)| *ch == *channel && *k == *key)
                    {
                        let (channel, key, start) = pending.remove(pos);
                        notes.push(NoteData {
                            time: start,
                            duration: (t - start).max(0.0),
                            midi: key,
                            channel,
                        });
                    }
                }
                _ => {}
            }
        }

        for (channel, key, start) in &pending {
            notes.push(NoteData {
                time: *start,
                duration: (total_length - start).max(0.0),
                midi: *key,
                channel: *channel,
            });
        }

        notes.sort_by(|a, b| a.time.total_cmp(&b.time));
        Ok(notes)
    }

    fn seconds_for_tick(tick: u64, tpq: u16, tempo_map: &[(u64, u32)]) -> f64 {
        let mut secs = 0.0_f64;
        let mut last_tick = 0u64;
        let mut cur_tempo = tempo_map.first().map(|&(_, us)| us).unwrap_or(500_000) as f64;

        for &(t, us) in tempo_map {
            if t >= tick {
                break;
            }
            if t > last_tick {
                secs += Self::ticks_to_seconds(t - last_tick, tpq, cur_tempo);
                last_tick = t;
            }
            cur_tempo = us as f64;
        }
        secs + Self::ticks_to_seconds(tick - last_tick, tpq, cur_tempo)
    }

    fn ticks_to_seconds(delta_ticks: u64, tpq: u16, tempo_us_per_quarter: f64) -> f64 {
        if tpq == 0 {
            return 0.0;
        }
        delta_ticks as f64 * (tempo_us_per_quarter / 1_000_000.0) / tpq as f64
    }
}

fn midi_message_to_event(channel: u8, message: &MidiMessage) -> MidiEvent {
    let ch = channel;
    match message {
        MidiMessage::NoteOn { key, vel } if vel.as_int() == 0 => MidiEvent::NoteOff {
            channel: ch,
            key: key.as_int(),
            velocity: 0,
        },
        MidiMessage::NoteOn { key, vel } => MidiEvent::NoteOn {
            channel: ch,
            key: key.as_int(),
            velocity: vel.as_int(),
        },
        MidiMessage::NoteOff { key, vel } => MidiEvent::NoteOff {
            channel: ch,
            key: key.as_int(),
            velocity: vel.as_int(),
        },
        MidiMessage::Controller { controller, value } => MidiEvent::ControlChange {
            channel: ch,
            control: controller.as_int(),
            value: value.as_int(),
        },
        MidiMessage::ProgramChange { program } => MidiEvent::ProgramChange {
            channel: ch,
            program: program.as_int(),
        },
        MidiMessage::PitchBend { bend } => MidiEvent::PitchBend {
            channel: ch,
            value: (bend.as_int() + 0x2000) as u16,
        },
        MidiMessage::Aftertouch { key, vel } => MidiEvent::Aftertouch {
            channel: ch,
            key: key.as_int(),
            pressure: vel.as_int(),
        },
        MidiMessage::ChannelAftertouch { vel } => MidiEvent::ChannelPressure {
            channel: ch,
            pressure: vel.as_int(),
        },
    }
}

/// Encode a MidiEvent into raw MIDI bytes for sending via midir.
pub(crate) fn encode_event(ev: &MidiEvent) -> Vec<u8> {
    match ev {
        MidiEvent::NoteOn {
            channel,
            key,
            velocity,
        } => encode_note_on(*channel, *key, *velocity),
        MidiEvent::NoteOff {
            channel,
            key,
            velocity,
        } => encode_note_off(*channel, *key, *velocity),
        MidiEvent::ControlChange {
            channel,
            control,
            value,
        } => encode_cc(*channel, *control, *value),
        MidiEvent::ProgramChange { channel, program } => encode_program_change(*channel, *program),
        MidiEvent::PitchBend { channel, value } => encode_pitch_bend(*channel, *value),
        MidiEvent::Aftertouch {
            channel,
            key,
            pressure,
        } => encode_aftertouch(*channel, *key, *pressure),
        MidiEvent::ChannelPressure { channel, pressure } => {
            encode_channel_pressure(*channel, *pressure)
        }
        MidiEvent::Sysex(bytes) => bytes.clone(),
    }
}

fn encode_note_on(ch: u8, key: u8, vel: u8) -> Vec<u8> {
    vec![0x90 | ch, key, vel]
}
fn encode_note_off(ch: u8, key: u8, vel: u8) -> Vec<u8> {
    vec![0x80 | ch, key, vel]
}
fn encode_cc(ch: u8, control: u8, value: u8) -> Vec<u8> {
    vec![0xB0 | ch, control, value]
}
fn encode_program_change(ch: u8, program: u8) -> Vec<u8> {
    vec![0xC0 | ch, program]
}
fn encode_pitch_bend(ch: u8, value: u16) -> Vec<u8> {
    vec![0xE0 | ch, (value & 0x7F) as u8, ((value >> 7) & 0x7F) as u8]
}
fn encode_aftertouch(ch: u8, key: u8, pressure: u8) -> Vec<u8> {
    vec![0xA0 | ch, key, pressure]
}
fn encode_channel_pressure(ch: u8, pressure: u8) -> Vec<u8> {
    vec![0xD0 | ch, pressure]
}
