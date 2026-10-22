//! Shared authored pattern state and audio-domain sequencer transport.

use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicU8, Ordering},
};

use crate::event_bridge::{EventKind, EventPayload, EventSource};

/// Bounded prepared capacity; interfaces are free to project smaller viewports.
pub const MAX_TRACKS: usize = 32;
pub const MAX_STEPS: usize = 64;
/// Index 255 remains the lock-free "none" sentinel.
pub const MAX_PATTERNS: usize = 255;
const NONE: u8 = u8::MAX;

#[derive(Clone, Copy, Debug)]
pub struct Pattern {
    pub velocities: [[f32; MAX_STEPS]; MAX_TRACKS],
    pub pitches_hz: [[f32; MAX_STEPS]; MAX_TRACKS],
    pub gates: [[f32; MAX_STEPS]; MAX_TRACKS],
    /// Authored loop length for each track. The executor wraps every track
    /// independently; this is musical state, not a viewport size.
    pub lengths: [u8; MAX_TRACKS],
}

impl Default for Pattern {
    fn default() -> Self {
        Self {
            velocities: [[0.0; MAX_STEPS]; MAX_TRACKS],
            pitches_hz: [[440.0; MAX_STEPS]; MAX_TRACKS],
            gates: [[0.5; MAX_STEPS]; MAX_TRACKS],
            lengths: [16; MAX_TRACKS],
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
    track_count: AtomicU8,
    pattern: Mutex<PublishedPattern>,
    command: AtomicU8,
    playing: AtomicBool,
    current_step: AtomicU8,
    active_pattern: AtomicU8,
    queued_pattern: AtomicU8,
    viewed_pattern: AtomicU8,
    muted: [AtomicBool; MAX_TRACKS],
    soloed: [AtomicBool; MAX_TRACKS],
    selected_track: AtomicU8,
    selected_step: AtomicU8,
    step_bank: AtomicU8,
    step_zoom: AtomicU8,
    key_root: AtomicU8,
    key_track: AtomicU8,
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
        let mut initial = Pattern::default();
        initial.lengths.fill(steps as u8);
        Ok(Arc::new(Self {
            name,
            steps,
            steps_per_beat,
            bpm: std::sync::atomic::AtomicU64::new(bpm.to_bits()),
            track_names: Mutex::new(Vec::new()),
            track_count: AtomicU8::new(0),
            pattern: Mutex::new(PublishedPattern {
                names: vec!["Default".into()],
                authored: vec![initial],
                pending_revision: true,
            }),
            command: AtomicU8::new(1),
            playing: AtomicBool::new(true),
            current_step: AtomicU8::new(0),
            active_pattern: AtomicU8::new(0),
            queued_pattern: AtomicU8::new(NONE),
            viewed_pattern: AtomicU8::new(0),
            muted: std::array::from_fn(|_| AtomicBool::new(false)),
            soloed: std::array::from_fn(|_| AtomicBool::new(false)),
            selected_track: AtomicU8::new(0),
            selected_step: AtomicU8::new(0),
            step_bank: AtomicU8::new(0),
            step_zoom: AtomicU8::new(1),
            key_root: AtomicU8::new(36),
            key_track: AtomicU8::new(0),
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
        self.track_count.store(names.len() as u8, Ordering::Release);
        Ok(names.len() - 1)
    }
    pub fn track_names(&self) -> Vec<String> {
        self.track_names
            .lock()
            .map(|v| v.clone())
            .unwrap_or_default()
    }
    pub fn track_count(&self) -> usize {
        usize::from(self.track_count.load(Ordering::Acquire))
    }
    pub fn set_steps(&self, track: usize, values: &[f32]) -> Result<(), String> {
        self.set_pattern_steps(self.viewed_pattern(), track, values)
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
        let mut pattern = Pattern::default();
        pattern.lengths.fill(self.steps as u8);
        state.authored.push(pattern);
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
        if values.is_empty() || values.len() > MAX_STEPS {
            return Err(format!("pattern row length must be 1..={MAX_STEPS}"));
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
        if track >= self.track_names().len() {
            return Err("track is out of range".into());
        }
        target.velocities[track].fill(0.0);
        target.velocities[track][..values.len()].copy_from_slice(values);
        target.lengths[track] = values.len() as u8;
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
        if pitches.is_empty()
            || pitches.len() > MAX_STEPS
            || gates.len() != pitches.len()
            || velocities.len() != pitches.len()
        {
            return Err(format!(
                "melodic pattern requires 1..={MAX_STEPS} equally sized pitch, gate, and velocity values"
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
            .any(|v| !v.is_finite() || !(0.0..=MAX_STEPS as f32).contains(v))
        {
            return Err(format!(
                "step gates must be finite durations in 0..={MAX_STEPS} steps"
            ));
        }
        self.set_pattern_steps(pattern, track, velocities)?;
        let mut state = self
            .pattern
            .lock()
            .map_err(|_| "sequencer pattern state poisoned")?;
        let target = state.authored.get_mut(pattern).ok_or("unknown pattern")?;
        target.pitches_hz[track][..pitches.len()].copy_from_slice(pitches);
        target.gates[track][..gates.len()].copy_from_slice(gates);
        target.lengths[track] = pitches.len() as u8;
        state.pending_revision = true;
        Ok(())
    }
    pub fn toggle(&self, track: usize, step: usize) -> Result<(), String> {
        if track >= self.track_names().len() || step >= MAX_STEPS {
            return Err("sequencer cell is out of range".into());
        }
        let mut state = self
            .pattern
            .lock()
            .map_err(|_| "sequencer pattern state poisoned")?;
        let viewed = self.viewed_pattern();
        let length = usize::from(state.authored[viewed].lengths[track]);
        if step >= length {
            return Err(format!(
                "step {} is beyond this track's {length}-step pattern",
                step + 1
            ));
        }
        let enabled = state.authored[viewed].velocities[track][step] <= 0.0;
        state.authored[viewed].velocities[track][step] = if enabled { 1.0 } else { 0.0 };
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
        let viewed = self.viewed_pattern();
        let length = usize::from(state.authored[viewed].lengths[track]);
        state.authored[viewed].velocities[track][..length].fill(0.0);
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
    pub fn copy_pattern_track(&self, from: usize, to: usize, track: usize) -> Result<(), String> {
        if track >= self.track_names().len() {
            return Err("track is out of range".into());
        }
        let mut state = self
            .pattern
            .lock()
            .map_err(|_| "sequencer pattern state poisoned")?;
        let source = *state.authored.get(from).ok_or("unknown source pattern")?;
        let target = state.authored.get_mut(to).ok_or("unknown target pattern")?;
        target.velocities[track] = source.velocities[track];
        target.pitches_hz[track] = source.pitches_hz[track];
        target.gates[track] = source.gates[track];
        target.lengths[track] = source.lengths[track];
        state.pending_revision = true;
        Ok(())
    }
    pub fn duplicate_pattern(&self, from: usize, name: String) -> Result<usize, String> {
        if self
            .pattern_names()
            .iter()
            .any(|candidate| candidate == &name)
        {
            return Err(format!("pattern '{name}' already exists"));
        }
        let target = self.define_pattern(name)?;
        self.copy_pattern(from, target)?;
        self.viewed_pattern.store(target as u8, Ordering::Release);
        Ok(target)
    }
    pub fn rotate_track(&self, track: usize, amount: i32) -> Result<(), String> {
        if track >= self.track_names().len() {
            return Err("track is out of range".into());
        }
        let mut state = self
            .pattern
            .lock()
            .map_err(|_| "sequencer pattern state poisoned")?;
        let viewed = self.viewed_pattern();
        let pattern = state
            .authored
            .get_mut(viewed)
            .ok_or("unknown viewed pattern")?;
        let length = usize::from(pattern.lengths[track]);
        if length == 0 {
            return Err("track pattern has no steps".into());
        }
        let right = amount.rem_euclid(length as i32) as usize;
        pattern.velocities[track][..length].rotate_right(right);
        pattern.pitches_hz[track][..length].rotate_right(right);
        pattern.gates[track][..length].rotate_right(right);
        state.pending_revision = true;
        Ok(())
    }
    pub fn set_pattern_length(
        &self,
        pattern: usize,
        track: usize,
        length: usize,
    ) -> Result<(), String> {
        if !(1..=MAX_STEPS).contains(&length) {
            return Err(format!("pattern length must be 1..={MAX_STEPS}"));
        }
        if track >= self.track_names().len() {
            return Err("track is out of range".into());
        }
        let mut state = self
            .pattern
            .lock()
            .map_err(|_| "sequencer pattern state poisoned")?;
        state
            .authored
            .get_mut(pattern)
            .ok_or("unknown pattern")?
            .lengths[track] = length as u8;
        state.pending_revision = true;
        Ok(())
    }
    pub fn set_note(
        &self,
        track: usize,
        step: usize,
        pitch_hz: f32,
        velocity: f32,
        gate_steps: f32,
    ) -> Result<(), String> {
        if track >= self.track_count() || step >= MAX_STEPS {
            return Err("sequencer note target is out of range".into());
        }
        if !pitch_hz.is_finite()
            || !(1.0..=24_000.0).contains(&pitch_hz)
            || !velocity.is_finite()
            || !(0.0..=1.0).contains(&velocity)
            || !gate_steps.is_finite()
            || !(0.0..=MAX_STEPS as f32).contains(&gate_steps)
        {
            return Err("sequencer note values are invalid".into());
        }
        let mut state = self
            .pattern
            .lock()
            .map_err(|_| "sequencer pattern state poisoned")?;
        let pattern = state
            .authored
            .get_mut(self.viewed_pattern())
            .ok_or("unknown viewed pattern")?;
        if step >= usize::from(pattern.lengths[track]) {
            return Err("sequencer note is beyond the track pattern length".into());
        }
        pattern.pitches_hz[track][step] = pitch_hz;
        pattern.velocities[track][step] = velocity;
        pattern.gates[track][step] = gate_steps;
        state.pending_revision = true;
        Ok(())
    }
    pub fn pattern(&self) -> Pattern {
        let active = self.viewed_pattern();
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
    pub fn viewed_pattern(&self) -> usize {
        usize::from(self.viewed_pattern.load(Ordering::Acquire))
    }
    pub fn view_pattern(&self, index: usize) -> Result<(), String> {
        if index >= self.pattern_names().len() {
            return Err("unknown pattern".into());
        }
        self.viewed_pattern.store(index as u8, Ordering::Release);
        self.set_status(format!("Viewing {}", self.pattern_names()[index]));
        Ok(())
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
    pub fn step_zoom(&self) -> usize {
        usize::from(self.step_zoom.load(Ordering::Acquire))
    }
    pub fn key_root(&self) -> u8 {
        self.key_root.load(Ordering::Acquire)
    }
    pub fn set_key_root(&self, root: u8) -> Result<(), String> {
        if root > 120 {
            return Err("sequencer key root must be a MIDI note in 0..120".into());
        }
        self.key_root.store(root, Ordering::Release);
        Ok(())
    }
    pub fn set_key_track(&self, track: usize) -> Result<(), String> {
        if track >= self.track_count() {
            return Err("sequencer key track is out of range".into());
        }
        self.key_track.store(track as u8, Ordering::Release);
        Ok(())
    }
    fn transpose_key(&self, semitones: i8) -> Result<(), String> {
        let previous = self.key_root();
        let next = (i16::from(previous) + i16::from(semitones)).clamp(0, 120) as u8;
        let actual = i16::from(next) - i16::from(previous);
        if actual == 0 {
            return Ok(());
        }
        let ratio = 2.0_f32.powf(actual as f32 / 12.0);
        let mut state = self
            .pattern
            .lock()
            .map_err(|_| "sequencer pattern state poisoned")?;
        let track = usize::from(self.key_track.load(Ordering::Acquire));
        for pattern in &mut state.authored {
            for step in 0..usize::from(pattern.lengths[track]) {
                if pattern.velocities[track][step] > 0.0 {
                    pattern.pitches_hz[track][step] =
                        (pattern.pitches_hz[track][step] * ratio).clamp(1.0, 24_000.0);
                }
            }
        }
        state.pending_revision = true;
        self.key_root.store(next, Ordering::Release);
        self.set_status(format!("Key root MIDI {next}"));
        Ok(())
    }
    pub fn select_track(&self, track: usize) -> Result<(), String> {
        if track >= self.track_names().len() {
            return Err("track is out of range".into());
        }
        self.selected_track.store(track as u8, Ordering::Release);
        Ok(())
    }
    pub fn set_step_bank(&self, bank: usize) -> Result<(), String> {
        let length = self.pattern().lengths[self.selected_track()] as usize;
        if bank * 8 >= length {
            return Err("step bank is out of range".into());
        }
        self.step_bank.store(bank as u8, Ordering::Release);
        let selected = self.selected_step();
        if selected / 8 != bank {
            self.selected_step
                .store((bank * 8) as u8, Ordering::Release);
        }
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
        if command == "zoom:in" || command == "zoom:out" {
            let current = self.step_zoom();
            let next = if command.ends_with("in") {
                (current / 2).max(1)
            } else {
                (current * 2).min(8)
            };
            self.step_zoom.store(next as u8, Ordering::Release);
            self.set_status(format!("Piano roll zoom: {next} step(s) per cell"));
            return Ok(());
        }
        if command == "key:up" || command == "key:down" {
            return self.transpose_key(if command.ends_with("up") { 1 } else { -1 });
        }
        if command == "length:increase" || command == "length:decrease" {
            let pattern = self.viewed_pattern();
            let track = self.selected_track();
            let current = usize::from(self.pattern().lengths[track]);
            let next = if command.ends_with("increase") {
                (current + 8).min(MAX_STEPS)
            } else {
                current.saturating_sub(8).max(8)
            };
            self.set_pattern_length(pattern, track, next)?;
            self.set_status(format!("Pattern length: {next} steps"));
            return Ok(());
        }
        if command == "view:next" || command == "view:previous" {
            let length = usize::from(self.pattern().lengths[self.selected_track()]);
            let banks_per_view = self.step_zoom();
            let max_bank = ((length.saturating_sub(1)) / (8 * banks_per_view)) * banks_per_view;
            let next = if command.ends_with("next") {
                (self.step_bank() + banks_per_view).min(max_bank)
            } else {
                self.step_bank().saturating_sub(banks_per_view)
            };
            return self.set_step_bank(next);
        }
        if command == "copy:next" {
            let active = self.viewed_pattern();
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
        if command == "duplicate" {
            let viewed = self.viewed_pattern();
            let names = self.pattern_names();
            let base = format!("{} Copy", names[viewed]);
            let mut name = base.clone();
            let mut suffix = 2;
            while names.contains(&name) {
                name = format!("{base} {suffix}");
                suffix += 1;
            }
            let created = self.duplicate_pattern(viewed, name.clone())?;
            self.set_status(format!("Duplicated as {name}"));
            self.viewed_pattern.store(created as u8, Ordering::Release);
            return Ok(());
        }
        if let Some(value) = command.strip_prefix("bank:") {
            return self.set_step_bank(value.parse().map_err(|_| "invalid bank")?);
        }
        if command == "page:next" {
            let length = usize::from(self.pattern().lengths[self.selected_track()]);
            return self.set_step_bank((self.step_bank() + 2).min((length - 1) / 8));
        }
        if command == "page:previous" {
            return self.set_step_bank(self.step_bank().saturating_sub(2));
        }
        if command == "bank:next" {
            let length = usize::from(self.pattern().lengths[self.selected_track()]);
            return self.set_step_bank((self.step_bank() + 1).min((length - 1) / 8));
        }
        if command == "bank:previous" {
            return self.set_step_bank(self.step_bank().saturating_sub(1));
        }
        if command == "pattern:next" || command == "pattern:previous" {
            let count = self.pattern_names().len();
            let viewed = self.viewed_pattern();
            let next = if command.ends_with("next") {
                (viewed + 1) % count
            } else {
                (viewed + count - 1) % count
            };
            return self.view_pattern(next);
        }
        if command == "rotate:left" {
            return self.rotate_track(self.selected_track(), -1);
        }
        if command == "rotate:right" {
            return self.rotate_track(self.selected_track(), 1);
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
        if command == "step:previous" || command == "step:next" {
            let length = usize::from(self.pattern().lengths[self.selected_track()]);
            let current = self.selected_step().min(length - 1);
            let step = if command.ends_with("next") {
                (current + 1) % length
            } else {
                (current + length - 1) % length
            };
            self.selected_step.store(step as u8, Ordering::Release);
            self.step_bank.store((step / 8) as u8, Ordering::Release);
            return Ok(());
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
            let active = self.viewed_pattern();
            let track = self.selected_track();
            let step = self.selected_step();
            let pattern = state.authored.get_mut(active).ok_or("unknown pattern")?;
            pattern.velocities[track][step] =
                (pattern.velocities[track][step] + velocity).clamp(0.0, 1.0);
            pattern.pitches_hz[track][step] =
                (pattern.pitches_hz[track][step] * pitch).clamp(1.0, 24_000.0);
            pattern.gates[track][step] =
                (pattern.gates[track][step] + gate).clamp(0.0, MAX_STEPS as f32);
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
        let quote = |value: &str| format!("{value:?}");
        let tracks = self.track_names();
        let mut lua = String::from(
            "-- Meldritch 2 canonical musical project; generated deliberately.\nreturn {\n  version = 2,\n  tracks = {",
        );
        for name in &tracks {
            lua.push_str(&format!("{},", quote(name)));
        }
        lua.push_str("},\n");
        lua.push_str(&format!(
            "  active_pattern = {},\n  viewed_pattern = {},\n",
            quote(&state.names[self.active_pattern()]),
            quote(&state.names[self.viewed_pattern()])
        ));
        lua.push_str("  patterns = {\n");
        for (name, pattern) in state.names.iter().zip(&state.authored) {
            lua.push_str(&format!("    {{ id = {}, tracks = {{\n", quote(name)));
            for (track, track_name) in tracks.iter().enumerate() {
                let length = usize::from(pattern.lengths[track]);
                lua.push_str(&format!(
                    "      [{}] = {{ length = {length}, events = {{",
                    quote(track_name)
                ));
                for step in 0..length {
                    let velocity = pattern.velocities[track][step];
                    if velocity > 0.0 {
                        lua.push_str(&format!(
                            "{{step={},velocity={velocity:?},pitch={:?},gate={:?}}},",
                            step + 1,
                            pattern.pitches_hz[track][step],
                            pattern.gates[track][step]
                        ));
                    }
                }
                lua.push_str("} },\n");
            }
            lua.push_str("    } },\n");
        }
        lua.push_str("  },\n  arrangement = {");
        if let Ok(arrangement) = self.arrangement.lock() {
            for (pattern, repeats) in arrangement.iter().copied() {
                if let Some(name) = state.names.get(pattern) {
                    lua.push_str(&format!("{{pattern={},repeats={repeats}}},", quote(name)));
                }
            }
        }
        lua.push_str("},\n}\n");
        drop(state);
        let temporary = path.with_extension("lua.tmp");
        use std::io::Write;
        let mut file = std::fs::File::create(&temporary).map_err(|e| e.to_string())?;
        file.write_all(lua.as_bytes()).map_err(|e| e.to_string())?;
        file.sync_all().map_err(|e| e.to_string())?;
        std::fs::rename(&temporary, path).map_err(|e| e.to_string())?;
        self.set_status(format!("Saved {}", path.display()));
        Ok(())
    }
    pub fn load_project(&self, path: &std::path::Path) -> Result<(), String> {
        use mlua::{Lua, Table};
        let source =
            std::fs::read_to_string(path).map_err(|e| format!("load {}: {e}", path.display()))?;
        let vm = Lua::new();
        let saved: Table = vm
            .load(&source)
            .set_name(path.to_string_lossy())
            .eval()
            .map_err(|e| format!("evaluate saved Lua project: {e}"))?;
        if saved.get::<u32>("version").map_err(|e| e.to_string())? != 2 {
            return Err("saved Lua project version is incompatible".into());
        }
        let saved_tracks: Vec<String> = saved
            .get::<Table>("tracks")
            .map_err(|e| e.to_string())?
            .sequence_values()
            .collect::<mlua::Result<_>>()
            .map_err(|e| e.to_string())?;
        let track_names = self.track_names();
        if saved_tracks != track_names {
            return Err(format!(
                "saved project track identities {:?} do not match loaded program {:?}",
                saved_tracks, track_names
            ));
        }
        let tracks = track_names.len();
        let mut names = Vec::new();
        let mut patterns = Vec::new();
        let saved_patterns: Table = saved.get("patterns").map_err(|e| e.to_string())?;
        for item in saved_patterns.sequence_values::<Table>() {
            let item = item.map_err(|e| e.to_string())?;
            let name: String = item.get("id").map_err(|e| e.to_string())?;
            if name.is_empty() || names.contains(&name) {
                return Err("saved patterns require unique nonempty names".into());
            }
            let mut pattern = Pattern::default();
            let rows: Table = item.get("tracks").map_err(|e| e.to_string())?;
            for (track, track_name) in track_names.iter().enumerate().take(tracks) {
                let row: Table = rows.get(track_name.as_str()).map_err(|_| {
                    format!("saved pattern '{name}' is missing track '{track_name}'")
                })?;
                let length: usize = row.get("length").map_err(|e| e.to_string())?;
                if !(1..=MAX_STEPS).contains(&length) {
                    return Err(format!(
                        "saved pattern '{name}' track '{track_name}' has invalid length {length}"
                    ));
                }
                pattern.lengths[track] = length as u8;
                let events: Table = row.get("events").map_err(|e| e.to_string())?;
                for event in events.sequence_values::<Table>() {
                    let event = event.map_err(|e| e.to_string())?;
                    let step: usize = event.get("step").map_err(|e| e.to_string())?;
                    let velocity: f32 = event.get("velocity").map_err(|e| e.to_string())?;
                    let pitch: f32 = event.get("pitch").map_err(|e| e.to_string())?;
                    let gate: f32 = event.get("gate").map_err(|e| e.to_string())?;
                    if step == 0
                        || step > length
                        || !velocity.is_finite()
                        || !(0.0..=1.0).contains(&velocity)
                        || !pitch.is_finite()
                        || !(1.0..=24_000.0).contains(&pitch)
                        || !gate.is_finite()
                        || !(0.0..=MAX_STEPS as f32).contains(&gate)
                    {
                        return Err(format!(
                            "saved pattern '{name}' track '{track_name}' contains an invalid event"
                        ));
                    }
                    pattern.velocities[track][step - 1] = velocity;
                    pattern.pitches_hz[track][step - 1] = pitch;
                    pattern.gates[track][step - 1] = gate;
                }
            }
            names.push(name);
            patterns.push(pattern)
        }
        if names.is_empty() || names.len() > MAX_PATTERNS {
            return Err(format!(
                "saved project requires 1..={MAX_PATTERNS} patterns"
            ));
        }
        let active_name: String = saved.get("active_pattern").map_err(|e| e.to_string())?;
        let viewed_name: String = saved.get("viewed_pattern").map_err(|e| e.to_string())?;
        let active = names
            .iter()
            .position(|name| name == &active_name)
            .ok_or_else(|| format!("saved active pattern target '{active_name}' is missing"))?;
        let viewed = names
            .iter()
            .position(|name| name == &viewed_name)
            .ok_or_else(|| format!("saved viewed pattern target '{viewed_name}' is missing"))?;
        let mut arrangement = Vec::new();
        if let Some(sections) = saved
            .get::<Option<Table>>("arrangement")
            .map_err(|e| e.to_string())?
        {
            for section in sections.sequence_values::<Table>() {
                let section = section.map_err(|e| e.to_string())?;
                let target: String = section.get("pattern").map_err(|e| e.to_string())?;
                let repeats: u32 = section.get("repeats").map_err(|e| e.to_string())?;
                let target = names
                    .iter()
                    .position(|name| name == &target)
                    .ok_or_else(|| format!("saved arrangement target '{target}' is missing"))?;
                if repeats == 0 {
                    return Err("arrangement repeats must be positive".into());
                }
                arrangement.push((target, repeats));
            }
        }
        let mut state = self
            .pattern
            .lock()
            .map_err(|_| "sequencer pattern state poisoned")?;
        state.names = names;
        state.authored = patterns;
        state.pending_revision = true;
        self.active_pattern.store(active as u8, Ordering::Release);
        self.viewed_pattern.store(viewed as u8, Ordering::Release);
        self.queued_pattern.store(NONE, Ordering::Release);
        *self
            .arrangement
            .lock()
            .map_err(|_| "sequencer arrangement state poisoned")? = arrangement;
        self.set_status(format!("Reloaded {}", path.display()));
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
        for track in 0..self.control.track_count() {
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
            let cycle = self.pattern.lengths[..self.control.track_count()]
                .iter()
                .copied()
                .map(usize::from)
                .max()
                .unwrap_or(1)
                .max(1);
            let boundary = self.initialized && self.step > 0 && self.step.is_multiple_of(cycle);
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
                if (state.pending_revision || boundary || !self.initialized)
                    && let Some(next) = state.authored.get(active)
                {
                    self.pattern = *next;
                    state.pending_revision = false;
                }
            }
            let active_pattern = self.control.active_pattern();
            if active_pattern != self.active_pattern {
                self.active_pattern = active_pattern;
                self.step = 0;
                self.control
                    .events
                    .publish(EventKind::PatternChanged, sample_position, || {
                        EventPayload::Pattern {
                            pattern: active_pattern as u8,
                        }
                    });
            }
            let cycle = self.pattern.lengths[..self.control.track_count()]
                .iter()
                .copied()
                .map(usize::from)
                .max()
                .unwrap_or(1)
                .max(1);
            let display_step = self.step % cycle;
            self.trigger_step = Some(display_step);
            self.control
                .current_step
                .store(display_step as u8, Ordering::Release);
            let bpm = self.control.bpm().max(1.0);
            let step_frames = sample_rate * 60.0 / (bpm * self.control.steps_per_beat as f64);
            let step = display_step;
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
            for track in 0..self.control.track_count() {
                let length = usize::from(self.pattern.lengths[track]).max(1);
                let track_step = self.step % length;
                let velocity = self.pattern.velocities[track][track_step] as f64;
                self.velocities[track] = velocity;
                if velocity > 0.0 {
                    self.pitches[track] = self.pattern.pitches_hz[track][track_step] as f64;
                    self.gates[track] = self.pattern.gates[track][track_step] as f64 * step_frames;
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
            self.step = self.step.wrapping_add(1);
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
    pub(crate) fn track_count(&self) -> usize {
        self.control.track_count()
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
    #[test]
    fn independent_lengths_wrap_and_duplicated_patterns_do_not_alias() {
        let control = SequencerControl::new("polymeter".into(), 120.0, 64, 4).unwrap();
        control.add_track("Three".into()).unwrap();
        control.add_track("Five".into()).unwrap();
        let source = control.define_pattern("Source".into()).unwrap();
        control
            .set_pattern_steps(source, 0, &[1.0, 0.0, 0.0])
            .unwrap();
        control
            .set_pattern_steps(source, 1, &[1.0, 0.0, 0.0, 0.0, 0.0])
            .unwrap();
        let copy = control.duplicate_pattern(source, "Copy".into()).unwrap();
        control.toggle(0, 1).unwrap();
        assert_eq!(control.pattern_at(source).unwrap().velocities[0][1], 0.0);
        assert_eq!(control.pattern_at(copy).unwrap().velocities[0][1], 1.0);
        control.queue_pattern(source).unwrap();
        let mut runtime = SequencerRuntime::new(control);
        let mut three = Vec::new();
        let mut five = Vec::new();
        for frame in 0..16 {
            runtime.begin_frame(8.0);
            if runtime.trigger(0) > 0.0 {
                three.push(frame);
            }
            if runtime.trigger(1) > 0.0 {
                five.push(frame);
            }
        }
        assert_eq!(three, [0, 3, 6, 9, 12, 15]);
        assert_eq!(five, [0, 5, 10, 15]);
    }

    #[test]
    fn eight_tracks_is_a_viewport_not_a_musical_model_limit() {
        let control = SequencerControl::new("wide".into(), 120.0, 16, 4).unwrap();
        for track in 0..12 {
            control.add_track(format!("Track {track}")).unwrap();
        }
        control.set_steps(11, &[0.75]).unwrap();
        let mut runtime = SequencerRuntime::new(Arc::clone(&control));
        runtime.begin_frame(8.0);
        assert_eq!(control.track_count(), 12);
        assert_eq!(runtime.trigger(11), 0.75);
    }

    #[test]
    fn viewed_pattern_and_sixteen_step_page_are_independent_of_playback() {
        let control = SequencerControl::new("editing".into(), 120.0, 64, 4).unwrap();
        control.add_track("Track".into()).unwrap();
        let playing = control.define_pattern("Playing".into()).unwrap();
        let viewed = control.define_pattern("Viewed".into()).unwrap();
        control.set_pattern_steps(playing, 0, &[0.0; 32]).unwrap();
        control.set_pattern_steps(viewed, 0, &[0.0; 32]).unwrap();
        control.view_pattern(viewed).unwrap();
        control.execute("page:next").unwrap();
        control.execute("toggle:1").unwrap();
        assert_eq!(control.step_bank(), 2);
        assert_eq!(control.pattern_at(viewed).unwrap().velocities[0][16], 1.0);
        assert_eq!(control.pattern_at(playing).unwrap().velocities[0][16], 0.0);
        control.execute("toggle:2").unwrap();
        assert_eq!(control.pattern_at(viewed).unwrap().velocities[0][17], 1.0);
        assert_eq!(control.active_pattern(), playing);
    }

    #[test]
    fn canonical_lua_save_reload_is_deterministic_and_uses_stable_targets() {
        let control = SequencerControl::new("persist".into(), 123.0, 64, 4).unwrap();
        control.add_track("Bass".into()).unwrap();
        let a = control.define_pattern("A".into()).unwrap();
        control.set_pattern_steps(a, 0, &[1.0, 0.0, 0.5]).unwrap();
        control.set_arrangement(&[(a, 2)]).unwrap();
        let path =
            std::env::temp_dir().join(format!("meldritch2-project-{}.lua", std::process::id()));
        control.save_project(&path).unwrap();
        let first = std::fs::read_to_string(&path).unwrap();
        control.save_project(&path).unwrap();
        assert_eq!(first, std::fs::read_to_string(&path).unwrap());
        control.clear_track(0).unwrap();
        control.load_project(&path).unwrap();
        assert_eq!(control.pattern_at(a).unwrap().velocities[0][2], 0.5);
        let wrong = SequencerControl::new("wrong".into(), 123.0, 64, 4).unwrap();
        wrong.add_track("Renamed".into()).unwrap();
        assert!(
            wrong
                .load_project(&path)
                .unwrap_err()
                .contains("identities")
        );
        std::fs::remove_file(path).unwrap();
    }
}
