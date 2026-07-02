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
    file_path: String,
    file_name: String,
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
            file_path: String::new(),
            file_name: String::new(),
        }
    }

    // ── Public API ──────────────────────────────────────────────

    pub(crate) fn load(&mut self, path: &str) -> Result<String, String> {
        self.stop();

        let data = std::fs::read(path).map_err(|e| format!("Failed to read file: {e}"))?;
        let smf =
            midly::Smf::parse(&data).map_err(|e| format!("Failed to parse MIDI file: {e}"))?;
        let (events, total_length) = Self::flatten(&smf);

        let file_name = std::path::Path::new(path)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        self.events = events;
        self.total_length = total_length;
        self.file_path = path.to_string();
        self.file_name = file_name;

        Ok(self.file_name.clone())
    }

    pub(crate) fn play(&mut self) {
        if self.events.is_empty() || self.state == State::Playing {
            return;
        }

        match self.state {
            State::Paused => {
                if let Some(paused_at) = self.paused_since {
                    // Advance start by pause duration so elapsed doesn't drift
                    self.start = self.start.map(|s| s + paused_at.elapsed());
                }
                self.paused_since = None;
            }
            State::Stopped => {
                self.start = Some(Instant::now() - Duration::from_secs_f64(self.elapsed));
            }
            _ => {}
        }

        self.state = State::Playing;
    }

    pub(crate) fn pause(&mut self) {
        if self.state != State::Playing {
            return;
        }
        self.all_notes_off();
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
        let position = position.clamp(0.0, self.total_length);
        self.all_notes_off();

        self.paused_since = None;
        self.elapsed = position;
        self.next_idx = self.events.partition_point(|(t, _)| *t < position);

        match self.state {
            State::Playing => {
                self.start = Some(Instant::now() - Duration::from_secs_f64(position));
            }
            State::Paused => {
                self.start = Some(Instant::now() - Duration::from_secs_f64(position));
            }
            State::Stopped => {
                self.start = None;
            }
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

    pub(crate) fn note_density_data(&self, bins: usize) -> Vec<f64> {
        if self.events.is_empty() || self.total_length <= 0.0 || bins == 0 {
            return vec![0.0; bins];
        }

        let mut intervals: Vec<(f64, f64)> = Vec::new();
        let mut pending: Vec<(u8, u8, f64)> = Vec::new();

        for (t, ev) in &self.events {
            match ev {
                MidiEvent::NoteOn {
                    channel,
                    key,
                    velocity,
                } if *velocity > 0 => {
                    pending.push((*channel, *key, *t));
                }
                MidiEvent::NoteOff { channel, key, .. } => {
                    if let Some(pos) = pending
                        .iter()
                        .position(|(ch, k, _)| *ch == *channel && *k == *key)
                    {
                        let (_, _, start) = pending.remove(pos);
                        intervals.push((start, *t));
                    }
                }
                _ => {}
            }
        }

        for (_, _, start) in &pending {
            intervals.push((*start, self.total_length));
        }

        if intervals.is_empty() {
            return vec![0.0; bins];
        }

        let bin_width = self.total_length / bins as f64;
        let mut density = vec![0.0; bins];

        for (start, end) in intervals {
            let start_bin = (start / bin_width).floor() as isize;
            let end_bin = (end / bin_width).ceil() as isize;
            for i in start_bin.max(0)..end_bin.min(bins as isize) {
                density[i as usize] += 1.0;
            }
        }

        let max_density = density.iter().cloned().fold(0.0_f64, f64::max);
        if max_density > 0.0 {
            for d in &mut density {
                *d /= max_density;
            }
        }

        density
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

    fn flatten(smf: &midly::Smf) -> (Vec<(f64, MidiEvent)>, f64) {
        let tpq = match smf.header.timing {
            Timing::Metrical(t) => t.as_int(),
            Timing::Timecode { .. } => return (Vec::new(), 0.0),
        };
        if tpq == 0 {
            return (Vec::new(), 0.0);
        }

        // First pass: gather tempo changes and raw MIDI events
        let mut tempo_changes: Vec<(u64, u32)> = vec![(0, 500_000)];
        let mut raw: Vec<(u64, MidiEvent)> = Vec::new();

        for track in &smf.tracks {
            let mut abs_tick: u64 = 0;
            for ev in track {
                abs_tick += ev.delta.as_int() as u64;
                match &ev.kind {
                    TrackEventKind::Meta(MetaMessage::Tempo(us)) => {
                        tempo_changes.push((abs_tick, us.as_int()));
                    }
                    TrackEventKind::Midi { channel, message } => {
                        if let Some(e) = midi_message_to_event(channel.as_int(), message) {
                            raw.push((abs_tick, e));
                        }
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

        // Second pass: convert ticks to seconds, then sort by time
        let mut events: Vec<(f64, MidiEvent)> = raw
            .into_iter()
            .map(|(tick, ev)| (Self::seconds_for_tick(tick, tpq, &tempo_changes), ev))
            .collect();

        events.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());

        let total_length = events.last().map(|(t, _)| *t).unwrap_or(0.0);
        (events, total_length)
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

fn midi_message_to_event(channel: u8, message: &MidiMessage) -> Option<MidiEvent> {
    let ch = channel;
    Some(match message {
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
    })
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
