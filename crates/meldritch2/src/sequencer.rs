//! Shared authored pattern state and audio-domain sequencer transport.

use serde::{Deserialize, Serialize};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicU8, Ordering},
};

use crate::event_bridge::{EventKind, EventPayload, EventSource};

pub const MAX_TRACKS: usize = 8;
pub const MAX_STEPS: usize = 64;
pub const MAX_PATTERNS: usize = 16;
const NONE: u8 = u8::MAX;

#[derive(Serialize, Deserialize)]
struct SavedPattern {
    name: String,
    velocities: Vec<Vec<f32>>,
    pitches_hz: Vec<Vec<f32>>,
    gates: Vec<Vec<f32>>,
}
#[derive(Serialize, Deserialize)]
struct SavedProject {
    version: u32,
    steps: usize,
    patterns: Vec<SavedPattern>,
}

#[derive(Clone, Copy, Debug)]
pub struct Pattern {
    pub velocities: [[f32; MAX_STEPS]; MAX_TRACKS],
    pub pitches_hz: [[f32; MAX_STEPS]; MAX_TRACKS],
    pub gates: [[f32; MAX_STEPS]; MAX_TRACKS],
}

impl Default for Pattern {
    fn default() -> Self {
        Self {
            velocities: [[0.0; MAX_STEPS]; MAX_TRACKS],
            pitches_hz: [[440.0; MAX_STEPS]; MAX_TRACKS],
            gates: [[0.5; MAX_STEPS]; MAX_TRACKS],
        }
    }
}

#[derive(Debug)]
struct PublishedPattern {
    names: Vec<String>,
    authored: Vec<Pattern>,
    pending_revision: bool,
}

/// Control-side state shared by Lua, TUI, MIDI and the compiled audio runtime.
#[derive(Debug)]
pub struct SequencerControl {
    pub name: String,
    pub steps: usize,
    pub steps_per_beat: u32,
    pub bpm: std::sync::atomic::AtomicU64,
    track_names: Mutex<Vec<String>>,
    pattern: Mutex<PublishedPattern>,
    command: AtomicU8,
    playing: AtomicBool,
    current_step: AtomicU8,
    active_pattern: AtomicU8,
    queued_pattern: AtomicU8,
    muted: [AtomicBool; MAX_TRACKS],
    soloed: [AtomicBool; MAX_TRACKS],
    selected_track: AtomicU8,
    selected_step: AtomicU8,
    step_bank: AtomicU8,
    status: Mutex<String>,
    arrangement: Mutex<Vec<(usize, u32)>>,
    position_steps: std::sync::atomic::AtomicU64,
    events: Arc<EventSource>,
}

impl SequencerControl {
    pub fn new(
        name: String,
        bpm: f64,
        steps: usize,
        steps_per_beat: u32,
    ) -> Result<Arc<Self>, String> {
        if !(1..=MAX_STEPS).contains(&steps) {
            return Err(format!("sequencer steps must be 1..={MAX_STEPS}"));
        }
        if steps_per_beat == 0 || !bpm.is_finite() || bpm <= 0.0 {
            return Err("sequencer requires positive BPM and resolution".into());
        }
        let events = EventSource::new(
            name.clone(),
            &[
                EventKind::StepAdvanced,
                EventKind::NoteOn,
                EventKind::NoteOff,
                EventKind::VoiceTriggered,
                EventKind::PatternChanged,
            ],
        );
        Ok(Arc::new(Self {
            name,
            steps,
            steps_per_beat,
            bpm: std::sync::atomic::AtomicU64::new(bpm.to_bits()),
            track_names: Mutex::new(Vec::new()),
            pattern: Mutex::new(PublishedPattern {
                names: vec!["Default".into()],
                authored: vec![Pattern::default()],
                pending_revision: true,
            }),
            command: AtomicU8::new(1),
            playing: AtomicBool::new(true),
            current_step: AtomicU8::new(0),
            active_pattern: AtomicU8::new(0),
            queued_pattern: AtomicU8::new(NONE),
            muted: std::array::from_fn(|_| AtomicBool::new(false)),
            soloed: std::array::from_fn(|_| AtomicBool::new(false)),
            selected_track: AtomicU8::new(0),
            selected_step: AtomicU8::new(0),
            step_bank: AtomicU8::new(0),
            status: Mutex::new("Ready".into()),
            arrangement: Mutex::new(Vec::new()),
            position_steps: std::sync::atomic::AtomicU64::new(0),
            events,
        }))
    }
    pub fn add_track(&self, name: String) -> Result<usize, String> {
        let mut names = self
            .track_names
            .lock()
            .map_err(|_| "sequencer track state poisoned")?;
        if names.len() >= MAX_TRACKS {
            return Err(format!("sequencer permits at most {MAX_TRACKS} tracks"));
        }
        if names.iter().any(|n| n == &name) {
            return Err(format!("duplicate sequencer track '{name}'"));
        }
        names.push(name);
        Ok(names.len() - 1)
    }
    pub fn track_names(&self) -> Vec<String> {
        self.track_names
            .lock()
            .map(|v| v.clone())
            .unwrap_or_default()
    }
    pub fn set_steps(&self, track: usize, values: &[f32]) -> Result<(), String> {
        self.set_pattern_steps(self.active_pattern(), track, values)
    }
    pub fn define_pattern(&self, name: String) -> Result<usize, String> {
        if name.is_empty() {
            return Err("pattern name must be nonempty".into());
        }
        let mut state = self
            .pattern
            .lock()
            .map_err(|_| "sequencer pattern state poisoned")?;
        if let Some(index) = state.names.iter().position(|candidate| candidate == &name) {
            return Ok(index);
        }
        if state.names.len() == 1 && state.names[0] == "Default" {
            state.names[0] = name;
            return Ok(0);
        }
        if state.names.len() >= MAX_PATTERNS {
            return Err(format!("sequencer permits at most {MAX_PATTERNS} patterns"));
        }
        state.names.push(name);
        state.authored.push(Pattern::default());
        Ok(state.names.len() - 1)
    }
    pub fn pattern_names(&self) -> Vec<String> {
        self.pattern
            .lock()
            .map(|s| s.names.clone())
            .unwrap_or_default()
    }
    pub fn set_pattern_steps(
        &self,
        pattern: usize,
        track: usize,
        values: &[f32],
    ) -> Result<(), String> {
        if values.len() != self.steps {
            return Err(format!("pattern requires exactly {} steps", self.steps));
        }
        if values
            .iter()
            .any(|velocity| !velocity.is_finite() || !(0.0..=1.0).contains(velocity))
        {
            return Err("step velocities must be finite values in 0..1".into());
        }
        let mut state = self
            .pattern
            .lock()
            .map_err(|_| "sequencer pattern state poisoned")?;
        let target = state.authored.get_mut(pattern).ok_or("unknown pattern")?;
        target.velocities[track][..self.steps].copy_from_slice(values);
        state.pending_revision = true;
        Ok(())
    }
    pub fn set_pattern_notes(
        &self,
        pattern: usize,
        track: usize,
        pitches: &[f32],
        gates: &[f32],
        velocities: &[f32],
    ) -> Result<(), String> {
        if pitches.len() != self.steps
            || gates.len() != self.steps
            || velocities.len() != self.steps
        {
            return Err(format!(
                "melodic pattern requires exactly {} pitch, gate, and velocity values",
                self.steps
            ));
        }
        if pitches
            .iter()
            .any(|v| !v.is_finite() || *v < 1.0 || *v > 24_000.0)
        {
            return Err("step pitches must be finite values in 1..24000 Hz".into());
        }
        if gates
            .iter()
            .any(|v| !v.is_finite() || !(0.0..=1.0).contains(v))
        {
            return Err("step gates must be finite values in 0..1".into());
        }
        self.set_pattern_steps(pattern, track, velocities)?;
        let mut state = self
            .pattern
            .lock()
            .map_err(|_| "sequencer pattern state poisoned")?;
        let target = state.authored.get_mut(pattern).ok_or("unknown pattern")?;
        target.pitches_hz[track][..self.steps].copy_from_slice(pitches);
        target.gates[track][..self.steps].copy_from_slice(gates);
        state.pending_revision = true;
        Ok(())
    }
    pub fn toggle(&self, track: usize, step: usize) -> Result<(), String> {
        if track >= self.track_names().len() || step >= self.steps {
            return Err("sequencer cell is out of range".into());
        }
        let mut state = self
            .pattern
            .lock()
            .map_err(|_| "sequencer pattern state poisoned")?;
        let active = usize::from(self.active_pattern.load(Ordering::Acquire));
        let enabled = state.authored[active].velocities[track][step] <= 0.0;
        state.authored[active].velocities[track][step] = if enabled { 1.0 } else { 0.0 };
        state.pending_revision = true;
        self.set_status(format!(
            "{} step {} {}",
            self.track_names().get(track).cloned().unwrap_or_default(),
            step + 1,
            if enabled { "on" } else { "off" }
        ));
        Ok(())
    }
    pub fn clear_track(&self, track: usize) -> Result<(), String> {
        let mut state = self
            .pattern
            .lock()
            .map_err(|_| "sequencer pattern state poisoned")?;
        let active = usize::from(self.active_pattern.load(Ordering::Acquire));
        state.authored[active].velocities[track][..self.steps].fill(0.0);
        state.pending_revision = true;
        self.set_status(format!(
            "Cleared {}",
            self.track_names().get(track).cloned().unwrap_or_default()
        ));
        Ok(())
    }
    pub fn copy_pattern(&self, from: usize, to: usize) -> Result<(), String> {
        let mut state = self
            .pattern
            .lock()
            .map_err(|_| "sequencer pattern state poisoned")?;
        let source = *state.authored.get(from).ok_or("unknown source pattern")?;
        let target = state.authored.get_mut(to).ok_or("unknown target pattern")?;
        *target = source;
        state.pending_revision = true;
        Ok(())
    }
    pub fn pattern(&self) -> Pattern {
        let active = self.active_pattern();
        self.pattern
            .lock()
            .ok()
            .and_then(|s| s.authored.get(active).copied())
            .unwrap_or_default()
    }
    pub fn pattern_at(&self, index: usize) -> Option<Pattern> {
        self.pattern
            .lock()
            .ok()
            .and_then(|s| s.authored.get(index).copied())
    }
    pub fn active_pattern(&self) -> usize {
        usize::from(self.active_pattern.load(Ordering::Acquire))
    }
    pub fn queued_pattern(&self) -> Option<usize> {
        let value = self.queued_pattern.load(Ordering::Acquire);
        (value != NONE).then_some(usize::from(value))
    }
    pub fn queue_pattern(&self, index: usize) -> Result<(), String> {
        if index >= self.pattern_names().len() {
            return Err("unknown pattern".into());
        }
        self.queued_pattern.store(index as u8, Ordering::Release);
        self.set_status(format!("Queued {}", self.pattern_names()[index]));
        Ok(())
    }
    pub fn set_arrangement(&self, sections: &[(usize, u32)]) -> Result<(), String> {
        if sections.is_empty()
            || sections
                .iter()
                .any(|(pattern, repeats)| *pattern >= self.pattern_names().len() || *repeats == 0)
        {
            return Err("arrangement requires known patterns with positive repeats".into());
        }
        *self
            .arrangement
            .lock()
            .map_err(|_| "sequencer arrangement state poisoned")? = sections.to_vec();
        self.active_pattern
            .store(sections[0].0 as u8, Ordering::Release);
        Ok(())
    }
    pub fn selected_track(&self) -> usize {
        usize::from(self.selected_track.load(Ordering::Acquire))
    }
    pub fn selected_step(&self) -> usize {
        usize::from(self.selected_step.load(Ordering::Acquire))
    }
    pub fn step_bank(&self) -> usize {
        usize::from(self.step_bank.load(Ordering::Acquire))
    }
    pub fn select_track(&self, track: usize) -> Result<(), String> {
        if track >= self.track_names().len() {
            return Err("track is out of range".into());
        }
        self.selected_track.store(track as u8, Ordering::Release);
        Ok(())
    }
    pub fn set_step_bank(&self, bank: usize) -> Result<(), String> {
        if bank * 8 >= self.steps {
            return Err("step bank is out of range".into());
        }
        self.step_bank.store(bank as u8, Ordering::Release);
        Ok(())
    }
    pub fn toggle_mute(&self, track: usize) -> Result<(), String> {
        let cell = self.muted.get(track).ok_or("track is out of range")?;
        cell.store(!cell.load(Ordering::Acquire), Ordering::Release);
        Ok(())
    }
    pub fn toggle_solo(&self, track: usize) -> Result<(), String> {
        let cell = self.soloed.get(track).ok_or("track is out of range")?;
        cell.store(!cell.load(Ordering::Acquire), Ordering::Release);
        Ok(())
    }
    pub fn muted(&self, track: usize) -> bool {
        self.muted
            .get(track)
            .is_some_and(|v| v.load(Ordering::Acquire))
    }
    pub fn soloed(&self, track: usize) -> bool {
        self.soloed
            .get(track)
            .is_some_and(|v| v.load(Ordering::Acquire))
    }
    pub fn track_enabled(&self, track: usize) -> bool {
        let any_solo = self.soloed.iter().any(|v| v.load(Ordering::Acquire));
        !self.muted(track) && (!any_solo || self.soloed(track))
    }
    pub fn status(&self) -> String {
        self.status.lock().map(|v| v.clone()).unwrap_or_default()
    }
    pub fn position_steps(&self) -> u64 {
        self.position_steps.load(Ordering::Acquire)
    }
    pub fn event_source(&self) -> &Arc<EventSource> {
        &self.events
    }
    fn set_status(&self, value: String) {
        if let Ok(mut status) = self.status.lock() {
            *status = value
        }
    }
    pub fn execute(&self, command: &str) -> Result<(), String> {
        if command == "transport" {
            self.toggle_transport();
            return Ok(());
        }
        if command == "start" {
            self.start();
            return Ok(());
        }
        if command == "stop" {
            self.stop();
            return Ok(());
        }
        if command == "restart" {
            self.restart();
            return Ok(());
        }
        if command == "clear" {
            return self.clear_track(self.selected_track());
        }
        if command == "copy:next" {
            let active = self.active_pattern();
            let count = self.pattern_names().len();
            if count < 2 {
                return Err("copy:next requires at least two patterns".into());
            }
            let target = (active + 1) % count;
            self.copy_pattern(active, target)?;
            self.set_status(format!(
                "Copied {} to {}",
                self.pattern_names()[active],
                self.pattern_names()[target]
            ));
            return Ok(());
        }
        if let Some(value) = command.strip_prefix("bank:") {
            return self.set_step_bank(value.parse().map_err(|_| "invalid bank")?);
        }
        if let Some(value) = command.strip_prefix("toggle:") {
            let column: usize = value.parse().map_err(|_| "invalid step column")?;
            if !(1..=8).contains(&column) {
                return Err("step column must be 1..8".into());
            }
            let step = self.step_bank() * 8 + column - 1;
            self.selected_step.store(step as u8, Ordering::Release);
            return self.toggle(self.selected_track(), step);
        }
        if command == "track:up" {
            return self.select_track(self.selected_track().saturating_sub(1));
        }
        if command == "track:down" {
            return self.select_track(
                (self.selected_track() + 1).min(self.track_names().len().saturating_sub(1)),
            );
        }
        for (prefix, action) in [("track:", 0), ("mute:", 1), ("solo:", 2), ("pattern:", 3)] {
            if let Some(value) = command.strip_prefix(prefix) {
                let index: usize = value.parse().map_err(|_| "invalid command index")?;
                return match action {
                    0 => self.select_track(index),
                    1 => self.toggle_mute(index),
                    2 => self.toggle_solo(index),
                    _ => self.queue_pattern(index),
                };
            }
        }
        if command == "mute" {
            return self.toggle_mute(self.selected_track());
        }
        if command == "solo" {
            return self.toggle_solo(self.selected_track());
        }
        let edit = |velocity: f32, pitch: f32, gate: f32| -> Result<(), String> {
            let mut state = self
                .pattern
                .lock()
                .map_err(|_| "sequencer pattern state poisoned")?;
            let active = self.active_pattern();
            let track = self.selected_track();
            let step = self.selected_step();
            let pattern = state.authored.get_mut(active).ok_or("unknown pattern")?;
            pattern.velocities[track][step] =
                (pattern.velocities[track][step] + velocity).clamp(0.0, 1.0);
            pattern.pitches_hz[track][step] =
                (pattern.pitches_hz[track][step] * pitch).clamp(1.0, 24_000.0);
            pattern.gates[track][step] = (pattern.gates[track][step] + gate).clamp(0.0, 1.0);
            state.pending_revision = true;
            Ok(())
        };
        match command {
            "velocity:up" => edit(0.1, 1.0, 0.0),
            "velocity:down" => edit(-0.1, 1.0, 0.0),
            "pitch:up" => edit(0.0, 2.0_f32.powf(1.0 / 12.0), 0.0),
            "pitch:down" => edit(0.0, 2.0_f32.powf(-1.0 / 12.0), 0.0),
            "gate:up" => edit(0.0, 1.0, 0.1),
            "gate:down" => edit(0.0, 1.0, -0.1),
            _ => Err(format!("unknown performance command '{command}'")),
        }
    }
    pub fn save_project(&self, path: &std::path::Path) -> Result<(), String> {
        let state = self
            .pattern
            .lock()
            .map_err(|_| "sequencer pattern state poisoned")?;
        let patterns = state
            .names
            .iter()
            .zip(&state.authored)
            .map(|(name, p)| SavedPattern {
                name: name.clone(),
                velocities: p
                    .velocities
                    .iter()
                    .map(|row| row[..self.steps].to_vec())
                    .collect(),
                pitches_hz: p
                    .pitches_hz
                    .iter()
                    .map(|row| row[..self.steps].to_vec())
                    .collect(),
                gates: p
                    .gates
                    .iter()
                    .map(|row| row[..self.steps].to_vec())
                    .collect(),
            })
            .collect();
        let json = serde_json::to_vec_pretty(&SavedProject {
            version: 1,
            steps: self.steps,
            patterns,
        })
        .map_err(|e| e.to_string())?;
        let temporary = path.with_extension("json.tmp");
        std::fs::write(&temporary, json).map_err(|e| e.to_string())?;
        std::fs::rename(&temporary, path).map_err(|e| e.to_string())?;
        self.set_status(format!("Saved {}", path.display()));
        Ok(())
    }
    pub fn load_project(&self, path: &std::path::Path) -> Result<(), String> {
        let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
        let saved: SavedProject = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        if saved.version != 1
            || saved.steps != self.steps
            || saved.patterns.is_empty()
            || saved.patterns.len() > MAX_PATTERNS
        {
            return Err("saved groovebox project is incompatible".into());
        }
        let tracks = self.track_names().len();
        let mut names = Vec::new();
        let mut patterns = Vec::new();
        for item in saved.patterns {
            if item.name.is_empty() || names.contains(&item.name) {
                return Err("saved patterns require unique nonempty names".into());
            }
            if item.velocities.len() != MAX_TRACKS
                || item.pitches_hz.len() != MAX_TRACKS
                || item.gates.len() != MAX_TRACKS
            {
                return Err("saved pattern requires eight track rows".into());
            }
            let mut pattern = Pattern::default();
            for track in 0..tracks {
                if item.velocities[track].len() != self.steps
                    || item.pitches_hz[track].len() != self.steps
                    || item.gates[track].len() != self.steps
                {
                    return Err("saved pattern row length is incompatible".into());
                }
                if item.velocities[track]
                    .iter()
                    .any(|value| !value.is_finite() || !(0.0..=1.0).contains(value))
                    || item.pitches_hz[track]
                        .iter()
                        .any(|value| !value.is_finite() || !(1.0..=24_000.0).contains(value))
                    || item.gates[track]
                        .iter()
                        .any(|value| !value.is_finite() || !(0.0..=1.0).contains(value))
                {
                    return Err("saved pattern contains an invalid musical event".into());
                }
                pattern.velocities[track][..self.steps].copy_from_slice(&item.velocities[track]);
                pattern.pitches_hz[track][..self.steps].copy_from_slice(&item.pitches_hz[track]);
                pattern.gates[track][..self.steps].copy_from_slice(&item.gates[track]);
            }
            names.push(item.name);
            patterns.push(pattern)
        }
        let mut state = self
            .pattern
            .lock()
            .map_err(|_| "sequencer pattern state poisoned")?;
        state.names = names;
        state.authored = patterns;
        state.pending_revision = true;
        self.active_pattern.store(0, Ordering::Release);
        self.queued_pattern.store(NONE, Ordering::Release);
        Ok(())
    }
    pub fn publish_bpm(&self, bpm: f64) -> Result<(), String> {
        if !bpm.is_finite() || bpm <= 0.0 {
            return Err("BPM must be positive".into());
        }
        self.bpm.store(bpm.to_bits(), Ordering::Release);
        Ok(())
    }
    pub fn bpm(&self) -> f64 {
        f64::from_bits(self.bpm.load(Ordering::Acquire))
    }
    pub fn start(&self) {
        self.command.store(1, Ordering::Release);
    }
    pub fn stop(&self) {
        self.command.store(2, Ordering::Release);
    }
    pub fn restart(&self) {
        self.command.store(3, Ordering::Release);
    }
    pub fn toggle_transport(&self) {
        if self.playing() {
            self.stop()
        } else {
            self.start()
        }
    }
    pub fn playing(&self) -> bool {
        self.playing.load(Ordering::Acquire)
    }
    pub fn current_step(&self) -> usize {
        self.current_step.load(Ordering::Acquire) as usize
    }
}

#[derive(Debug)]
pub(crate) struct SequencerRuntime {
    control: Arc<SequencerControl>,
    pattern: Pattern,
    step: usize,
    frames_until_step: f64,
    trigger_step: Option<usize>,
    initialized: bool,
    gates: [f64; MAX_TRACKS],
    pitches: [f64; MAX_TRACKS],
    velocities: [f64; MAX_TRACKS],
    audible_notes: [bool; MAX_TRACKS],
    arrangement: Vec<(usize, u32)>,
    arrangement_section: usize,
    arrangement_repeats: u32,
    sample_position: u64,
    active_pattern: usize,
}

impl SequencerRuntime {
    pub fn new(control: Arc<SequencerControl>) -> Self {
        let arrangement = control
            .arrangement
            .lock()
            .map(|value| value.clone())
            .unwrap_or_default();
        let active_pattern = control.active_pattern();
        Self {
            control,
            pattern: Pattern::default(),
            step: 0,
            frames_until_step: 0.0,
            trigger_step: None,
            initialized: false,
            gates: [0.0; MAX_TRACKS],
            pitches: [440.0; MAX_TRACKS],
            velocities: [0.0; MAX_TRACKS],
            audible_notes: [false; MAX_TRACKS],
            arrangement,
            arrangement_section: 0,
            arrangement_repeats: 0,
            sample_position: 0,
            active_pattern,
        }
    }
    pub fn begin_frame(&mut self, sample_rate: f64) {
        let sample_position = self.sample_position;
        self.sample_position = self.sample_position.wrapping_add(1);
        for track in 0..MAX_TRACKS {
            self.gates[track] = (self.gates[track] - 1.0).max(0.0);
            if self.audible_notes[track]
                && (self.gates[track] == 0.0 || !self.control.track_enabled(track))
            {
                self.audible_notes[track] = false;
                let pitch = self.pitches[track];
                self.control
                    .events
                    .publish(EventKind::NoteOff, sample_position, || EventPayload::Note {
                        track: track as u8,
                        pitch,
                        velocity: 0.0,
                    });
            }
        }
        match self.control.command.swap(0, Ordering::AcqRel) {
            1 => {
                self.control.playing.store(true, Ordering::Release);
                if !self.initialized {
                    self.frames_until_step = 0.0;
                }
            }
            2 => self.control.playing.store(false, Ordering::Release),
            3 => {
                self.step = 0;
                self.frames_until_step = 0.0;
                self.control.position_steps.store(0, Ordering::Release);
                self.control.playing.store(true, Ordering::Release);
            }
            _ => {}
        }
        self.trigger_step = None;
        if !self.control.playing.load(Ordering::Acquire) {
            return;
        }
        if self.frames_until_step <= 0.0 {
            let boundary = self.initialized && self.step == 0;
            if boundary {
                if !self.arrangement.is_empty() {
                    self.arrangement_repeats += 1;
                    let repeats = self.arrangement[self.arrangement_section].1;
                    if self.arrangement_repeats >= repeats
                        && self.arrangement_section + 1 < self.arrangement.len()
                    {
                        self.arrangement_section += 1;
                        self.arrangement_repeats = 0;
                        self.control.active_pattern.store(
                            self.arrangement[self.arrangement_section].0 as u8,
                            Ordering::Release,
                        );
                    }
                }
                let queued = self.control.queued_pattern.swap(NONE, Ordering::AcqRel);
                if queued != NONE {
                    self.control.active_pattern.store(queued, Ordering::Release);
                }
            }
            if let Ok(mut state) = self.control.pattern.try_lock() {
                let active = self.control.active_pattern();
                if state.pending_revision || boundary || !self.initialized {
                    if let Some(next) = state.authored.get(active) {
                        self.pattern = *next;
                        state.pending_revision = false;
                    }
                }
            }
            let active_pattern = self.control.active_pattern();
            if active_pattern != self.active_pattern {
                self.active_pattern = active_pattern;
                self.control
                    .events
                    .publish(EventKind::PatternChanged, sample_position, || {
                        EventPayload::Pattern {
                            pattern: active_pattern as u8,
                        }
                    });
            }
            self.trigger_step = Some(self.step);
            self.control
                .current_step
                .store(self.step as u8, Ordering::Release);
            let bpm = self.control.bpm().max(1.0);
            let step_frames = sample_rate * 60.0 / (bpm * self.control.steps_per_beat as f64);
            let step = self.step;
            let position = self.control.position_steps.load(Ordering::Relaxed);
            self.control
                .events
                .publish(EventKind::StepAdvanced, sample_position, || {
                    EventPayload::Step {
                        step: step as u8,
                        pattern: active_pattern as u8,
                        position,
                    }
                });
            for track in 0..MAX_TRACKS {
                let velocity = self.pattern.velocities[track][self.step] as f64;
                self.velocities[track] = velocity;
                if velocity > 0.0 {
                    self.pitches[track] = self.pattern.pitches_hz[track][self.step] as f64;
                    self.gates[track] = self.pattern.gates[track][self.step] as f64 * step_frames;
                    if self.control.track_enabled(track) {
                        self.audible_notes[track] = true;
                        let pitch = self.pitches[track];
                        self.control
                            .events
                            .publish(EventKind::NoteOn, sample_position, || EventPayload::Note {
                                track: track as u8,
                                pitch,
                                velocity,
                            });
                        self.control.events.publish(
                            EventKind::VoiceTriggered,
                            sample_position,
                            || EventPayload::Note {
                                track: track as u8,
                                pitch,
                                velocity,
                            },
                        );
                    }
                }
            }
            self.step = (self.step + 1) % self.control.steps;
            self.control.position_steps.fetch_add(1, Ordering::AcqRel);
            self.frames_until_step += step_frames;
            self.initialized = true;
        }
        self.frames_until_step -= 1.0;
    }
    pub fn trigger(&self, track: usize) -> f64 {
        if !self.control.track_enabled(track) {
            return 0.0;
        }
        self.trigger_step.map_or(0.0, |_| self.velocities[track])
    }
    pub fn pitch(&self, track: usize) -> f64 {
        self.pitches[track]
    }
    pub fn gate(&self, track: usize) -> f64 {
        if self.control.track_enabled(track) && self.gates[track] > 0.0 {
            1.0
        } else {
            0.0
        }
    }
    pub fn velocity(&self, track: usize) -> f64 {
        if self.control.track_enabled(track) {
            self.velocities[track]
        } else {
            0.0
        }
    }
    pub fn enabled(&self, track: usize) -> f64 {
        f64::from(self.control.track_enabled(track))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (Arc<SequencerControl>, SequencerRuntime) {
        let control = SequencerControl::new("test".into(), 120.0, 16, 4).unwrap();
        control.add_track("Kick".into()).unwrap();
        control.add_track("Snare".into()).unwrap();
        control
            .set_steps(
                0,
                &[
                    1.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
                ],
            )
            .unwrap();
        control
            .set_steps(
                1,
                &[
                    1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
                ],
            )
            .unwrap();
        (Arc::clone(&control), SequencerRuntime::new(control))
    }
    #[test]
    fn boundaries_are_sample_accurate_and_simultaneous() {
        let (_, mut runtime) = fixture();
        let mut events = Vec::new();
        for frame in 0..250 {
            runtime.begin_frame(800.0);
            if runtime.trigger(0) > 0.0 {
                events.push((frame, 0));
            }
            if runtime.trigger(1) > 0.0 {
                events.push((frame, 1));
            }
        }
        assert_eq!(events, vec![(0, 0), (0, 1), (200, 0)]);
    }
    #[test]
    fn tempo_stop_restart_and_boundary_edits_are_coherent() {
        let (control, mut runtime) = fixture();
        runtime.begin_frame(800.0);
        control.stop();
        for _ in 0..200 {
            runtime.begin_frame(800.0);
            assert_eq!(runtime.trigger(0), 0.0);
        }
        control
            .set_steps(
                0,
                &[
                    0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
                ],
            )
            .unwrap();
        control.publish_bpm(240.0).unwrap();
        control.restart();
        runtime.begin_frame(800.0);
        assert_eq!(runtime.trigger(0), 0.0);
        for _ in 0..49 {
            runtime.begin_frame(800.0);
        }
        runtime.begin_frame(800.0);
        assert_eq!(runtime.trigger(0), 1.0);
    }
    #[test]
    fn melodic_pitch_gate_velocity_and_note_off_are_sample_accurate() {
        let control = SequencerControl::new("melody".into(), 120.0, 16, 4).unwrap();
        control.add_track("Bass".into()).unwrap();
        let mut pitches = [440.0; 16];
        let mut gates = [0.5; 16];
        let mut velocities = [0.0; 16];
        pitches[0] = 110.0;
        gates[0] = 0.25;
        velocities[0] = 0.7;
        control
            .set_pattern_notes(0, 0, &pitches, &gates, &velocities)
            .unwrap();
        let mut runtime = SequencerRuntime::new(control);
        let mut observed = Vec::new();
        for frame in 0..101 {
            runtime.begin_frame(800.0);
            if runtime.trigger(0) > 0.0 || runtime.gate(0) > 0.0 {
                observed.push((
                    frame,
                    runtime.trigger(0),
                    runtime.pitch(0),
                    runtime.gate(0),
                    runtime.velocity(0),
                ));
            }
        }
        assert_eq!(observed.len(), 25);
        assert_eq!(observed[0], (0, 0.7_f32 as f64, 110.0, 1.0, 0.7_f32 as f64));
        assert_eq!(observed[1].1, 0.0);
        assert_eq!(runtime.gate(0), 0.0);
    }
    #[test]
    fn queued_patterns_switch_only_at_boundary_for_sixteen_and_sixty_four_steps() {
        for steps in [16, 64] {
            let control = SequencerControl::new("patterns".into(), 120.0, steps, 4).unwrap();
            control.add_track("T".into()).unwrap();
            let first_index = control.define_pattern("First".into()).unwrap();
            let second = control.define_pattern("Second".into()).unwrap();
            let mut first = vec![0.0; steps];
            first[0] = 0.25;
            let mut next = vec![0.0; steps];
            next[0] = 0.75;
            control.set_pattern_steps(first_index, 0, &first).unwrap();
            control.set_pattern_steps(second, 0, &next).unwrap();
            let mut runtime = SequencerRuntime::new(Arc::clone(&control));
            runtime.begin_frame(8.0);
            assert_eq!(runtime.trigger(0), 0.25);
            control.queue_pattern(second).unwrap();
            for _ in 0..steps - 1 {
                runtime.begin_frame(8.0);
                assert_eq!(control.active_pattern(), 0);
            }
            runtime.begin_frame(8.0);
            assert_eq!(control.active_pattern(), second);
            assert_eq!(runtime.trigger(0), 0.75);
        }
    }
    #[test]
    fn authored_arrangement_advances_after_requested_pattern_repeats() {
        let control = SequencerControl::new("arrangement".into(), 120.0, 16, 4).unwrap();
        control.add_track("T".into()).unwrap();
        let foundation = control.define_pattern("Foundation".into()).unwrap();
        let broken = control.define_pattern("Broken".into()).unwrap();
        let mut first = [0.0; 16];
        first[0] = 0.25;
        let mut second = [0.0; 16];
        second[0] = 0.75;
        control.set_pattern_steps(foundation, 0, &first).unwrap();
        control.set_pattern_steps(broken, 0, &second).unwrap();
        control
            .set_arrangement(&[(foundation, 2), (broken, 1)])
            .unwrap();
        let mut runtime = SequencerRuntime::new(Arc::clone(&control));
        let mut starts = Vec::new();
        for _ in 0..49 {
            runtime.begin_frame(8.0);
            if runtime.trigger(0) > 0.0 {
                starts.push(runtime.trigger(0));
            }
        }
        assert_eq!(starts, [0.25, 0.25, 0.75, 0.75]);
        assert_eq!(control.active_pattern(), broken);
    }
    #[test]
    fn mute_and_solo_preserve_pattern_and_have_coherent_precedence() {
        let (control, mut runtime) = fixture();
        let before = control.pattern();
        control.toggle_mute(0).unwrap();
        runtime.begin_frame(800.0);
        assert_eq!(runtime.trigger(0), 0.0);
        assert_eq!(control.pattern().velocities, before.velocities);
        control.toggle_solo(1).unwrap();
        assert!(!control.track_enabled(0));
        assert!(control.track_enabled(1));
        control.toggle_mute(1).unwrap();
        assert!(!control.track_enabled(1));
        control.toggle_solo(1).unwrap();
        assert!(!control.track_enabled(0));
        control.toggle_mute(0).unwrap();
        assert!(control.track_enabled(0));
    }
}
