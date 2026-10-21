use std::collections::{BTreeMap, BTreeSet};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

use crate::contract::{self, FanIn, Role};
use crate::midi::NoteState;
use crate::model::{
    Action, Connection, ControlScale, Graph, MidiButton, MidiCc, MidiNotes, MidiPacket,
    ModuleRuntime, Navigation, ParameterId, Screen, Widget,
};
use crate::plaits::PlaitsRuntime;
use crate::sample::PreparedSample;
use crate::sequencer::{SequencerControl, SequencerRuntime};

#[derive(Debug)]
struct Op {
    processor: OpProcessor,
    inputs: Vec<InputPlan>,
    outputs: std::ops::Range<usize>,
    params: Vec<usize>,
}

#[derive(Debug)]
enum OpProcessor {
    Contract {
        process: contract::ProcessFn,
        state: Vec<f64>,
    },
    Sample(SamplePlayerRuntime),
    Delay(DelayRuntime),
    Plaits(Box<PlaitsRuntime>),
    SequencerTrack {
        sequencer: usize,
        track: usize,
    },
    SequencerTempo(Arc<SequencerControl>),
}

#[derive(Debug)]
struct DelayRuntime {
    line: Vec<f64>,
    write: usize,
}

impl DelayRuntime {
    fn new(sample_rate: f64) -> Result<Self, String> {
        let frames = (sample_rate * 4.0).ceil() as usize + 2;
        let mut line = Vec::new();
        line.try_reserve_exact(frames)
            .map_err(|_| "not enough memory to prepare four-second delay line".to_string())?;
        line.resize(frames, 0.0);
        Ok(Self { line, write: 0 })
    }

    fn process(&mut self, input: &[f64], params: &[f64], sample_rate: f64, out: &mut [f64]) {
        let dry = if input[0].is_finite() { input[0] } else { 0.0 };
        let modulation = |value: f64| if value.is_finite() { value } else { 0.0 };
        let time = (params[0] + modulation(input[1])).clamp(1.0 / sample_rate, 4.0);
        let feedback = (params[1] + modulation(input[2])).clamp(-0.99, 0.99);
        let mix = (params[2] + modulation(input[3])).clamp(0.0, 1.0);
        let delay = time * sample_rate;
        let read = (self.write as f64 - delay).rem_euclid(self.line.len() as f64);
        let first = read.floor() as usize;
        let second = (first + 1) % self.line.len();
        let fraction = read - first as f64;
        let wet = self.line[first] + (self.line[second] - self.line[first]) * fraction;
        self.line[self.write] = dry + wet * feedback;
        self.write = (self.write + 1) % self.line.len();
        out[0] = dry * (1.0 - mix) + wet * mix;
    }
}

#[cfg(test)]
mod delay_runtime_tests {
    use super::DelayRuntime;

    #[test]
    fn impulse_repeats_at_requested_time_with_bounded_feedback_and_reset() {
        let mut delay = DelayRuntime::new(100.0).unwrap();
        let params = [0.1, 0.5, 1.0];
        let mut rendered = Vec::new();
        for frame in 0..31 {
            let mut out = [0.0];
            delay.process(
                &[f64::from(frame == 0), 0.0, 0.0, 0.0],
                &params,
                100.0,
                &mut out,
            );
            rendered.push(out[0]);
        }
        assert_eq!(rendered[0], 0.0);
        assert!((rendered[10] - 1.0).abs() < 1e-12);
        assert!((rendered[20] - 0.5).abs() < 1e-12);
        assert!((rendered[30] - 0.25).abs() < 1e-12);
        let mut reset = DelayRuntime::new(100.0).unwrap();
        let mut out = [f64::NAN];
        reset.process(&[0.0, 0.0, 9.0, 0.0], &params, 100.0, &mut out);
        assert_eq!(out[0], 0.0);
    }

    #[test]
    fn fractional_and_changing_times_remain_finite() {
        let mut delay = DelayRuntime::new(44_100.0).unwrap();
        for frame in 0..20_000 {
            let mut out = [0.0];
            let modulation = (frame as f64 / 101.0).sin() * 0.02;
            delay.process(
                &[if frame == 0 { 1.0 } else { 0.0 }, modulation, 0.0, 0.0],
                &[0.1, 0.98, 0.75],
                44_100.0,
                &mut out,
            );
            assert!(out[0].is_finite());
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct SampleVoice {
    position: f64,
}

#[derive(Debug)]
struct SamplePlayerRuntime {
    sample: Arc<PreparedSample>,
    voices: Vec<SampleVoice>,
    max_voices: usize,
    last_trigger: f64,
}

impl SamplePlayerRuntime {
    fn new(sample: Arc<PreparedSample>, output_rate: f64) -> Result<Self, String> {
        let rendered_frames =
            (sample.frames() as f64 * output_rate / f64::from(sample.sample_rate)).ceil();
        if !rendered_frames.is_finite() || rendered_frames > usize::MAX as f64 {
            return Err("sample is too long to prepare its overlap storage".into());
        }
        // A new rising edge requires at least one nonpositive frame between
        // triggers. Preallocate every overlap that can exist during this
        // sample's rendered lifetime, so the audio callback never allocates.
        let max_voices = (rendered_frames as usize).div_ceil(2).max(1);
        let mut voices = Vec::new();
        voices.try_reserve_exact(max_voices).map_err(|_| {
            format!(
                "not enough memory to prepare {max_voices} overlapping voices for sample '{}'",
                sample.identity
            )
        })?;
        Ok(Self {
            sample,
            voices,
            max_voices,
            last_trigger: 0.0,
        })
    }
    fn process(&mut self, trigger: f64, gain: f64, output_rate: f64, out: &mut [f64]) {
        if trigger > 0.0 && self.last_trigger <= 0.0 {
            assert!(
                self.voices.len() < self.max_voices,
                "prepared sample overlap bound must cover every possible trigger"
            );
            self.voices.push(SampleVoice { position: 0.0 });
        }
        self.last_trigger = trigger;
        let increment = self.sample.sample_rate as f64 / output_rate;
        let mut left = 0.0;
        let mut right = 0.0;
        for voice in &mut self.voices {
            let index = voice.position.floor() as usize;
            debug_assert!(index < self.sample.frames());
            let fraction = voice.position - index as f64;
            let next = (index + 1).min(self.sample.frames() - 1);
            left += self.sample.left[index]
                + (self.sample.left[next] - self.sample.left[index]) * fraction;
            right += self.sample.right[index]
                + (self.sample.right[next] - self.sample.right[index]) * fraction;
            voice.position += increment;
        }
        self.voices
            .retain(|voice| voice.position < self.sample.frames() as f64);
        out[0] = left * gain;
        out[1] = right * gain;
    }
}

#[derive(Clone, Debug)]
struct InputPlan {
    sources: Vec<usize>,
    unconnected: f64,
}

#[derive(Debug)]
struct NotePair {
    channel: u8,
    pitch_index: usize,
    gate_index: usize,
    velocity_index: Option<usize>,
    state: Arc<NoteState>,
    last_pitch: f64,
    last_gate: f64,
    last_velocity: f64,
}

const NO_OVERRIDE: u64 = f64::NAN.to_bits();

pub(crate) fn validate_parameter_value(
    id: &ParameterId,
    range: Option<(f64, f64)>,
    value: f64,
) -> Result<(), String> {
    if !value.is_finite() {
        return Err(format!("{}.{} must be finite", id.module, id.name));
    }
    if let Some((min, max)) = range {
        if value < min || value > max {
            return Err(format!("{}.{} must be in {min}..{max}", id.module, id.name));
        }
    }
    Ok(())
}

/// A control-side handle for one parameter in one compiled graph revision.
#[derive(Clone, Debug)]
pub struct ParameterHandle {
    id: ParameterId,
    cell: Arc<AtomicU64>,
    range: Option<(f64, f64)>,
}

impl ParameterHandle {
    pub fn id(&self) -> &ParameterId {
        &self.id
    }

    pub fn publish(&self, value: Option<f64>) -> Result<(), String> {
        let bits = match value {
            Some(value) => {
                validate_parameter_value(&self.id, self.range, value)?;
                value.to_bits()
            }
            None => NO_OVERRIDE,
        };
        self.cell.store(bits, Ordering::Release);
        Ok(())
    }

    pub fn override_value(&self) -> Option<f64> {
        let bits = self.cell.load(Ordering::Acquire);
        (bits != NO_OVERRIDE).then(|| f64::from_bits(bits))
    }
}

#[derive(Clone, Debug)]
struct ControlBinding {
    label: String,
    target: ParameterId,
    additional_targets: Vec<ParameterId>,
    min: f64,
    max: f64,
    scale: ControlScale,
    hidden: bool,
}

/// A named instrument control bound to a parameter in one graph revision.
#[derive(Clone, Debug)]
pub struct ControlHandle {
    id: String,
    label: String,
    parameters: Vec<ParameterHandle>,
    min: f64,
    max: f64,
    scale: ControlScale,
}

impl ControlHandle {
    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn label(&self) -> &str {
        &self.label
    }
    pub fn target(&self) -> &ParameterId {
        self.parameters[0].id()
    }
    pub fn targets(&self) -> impl Iterator<Item = &ParameterId> {
        self.parameters.iter().map(ParameterHandle::id)
    }
    pub fn range(&self) -> (f64, f64) {
        (self.min, self.max)
    }
    pub fn scale(&self) -> ControlScale {
        self.scale
    }
    pub fn override_value(&self) -> Option<f64> {
        self.parameters[0].override_value()
    }
    pub fn publish(&self, value: f64) -> Result<(), String> {
        if !value.is_finite() || value < self.min || value > self.max {
            return Err(format!(
                "control '{}' requires a finite value in {}..{}",
                self.id, self.min, self.max
            ));
        }
        for parameter in &self.parameters {
            parameter.publish(Some(value))?;
        }
        Ok(())
    }
    pub fn clear(&self) -> Result<(), String> {
        for parameter in &self.parameters {
            parameter.publish(None)?;
        }
        Ok(())
    }
}

/// An accepted, executable graph. Its processing buffers are allocated at compile time.
#[derive(Debug)]
pub struct Machine {
    revision: Arc<()>,
    ops: Vec<Op>,
    slots: Vec<f64>,
    input_scratch: Vec<f64>,
    param_scratch: Vec<f64>,
    params: Vec<f64>,
    authored: Vec<f64>,
    published: Vec<Arc<AtomicU64>>,
    param_ranges: Vec<Option<(f64, f64)>>,
    param_ids: BTreeMap<ParameterId, usize>,
    controls: BTreeMap<String, ControlBinding>,
    screens: Vec<Screen>,
    midi_ccs: Vec<MidiCc>,
    midi_notes: Vec<MidiNotes>,
    midi_buttons: Vec<MidiButton>,
    midi_packets: Vec<MidiPacket>,
    actions: Vec<Action>,
    poly_notes: Vec<PolyMapping>,
    note_pairs: Vec<NotePair>,
    output_slots: [usize; 2],
    stereo: bool,
    order: Vec<String>,
    authored_normalizations: Vec<Connection>,
    effective_connections: Vec<Connection>,
    sample_rate: f64,
    sequencers: Vec<SequencerRuntime>,
    sequencer_controls: Vec<Arc<SequencerControl>>,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct TimedNoteTarget {
    pub pitch_index: usize,
    pub gate_index: usize,
    pub velocity_index: Option<usize>,
    pub min_pitch: f64,
    pub max_pitch: f64,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct PolyVoiceTarget {
    pub pitch_index: usize,
    pub gate_index: usize,
    pub velocity_index: usize,
}

pub(crate) const MAX_POLY_VOICES: usize = 4;

#[derive(Debug)]
struct PolyMapping {
    channel: u8,
    voices: [Option<PolyVoiceTarget>; MAX_POLY_VOICES],
    overflow: crate::model::PolyOverflow,
}

fn validate_screens(
    screens: &[Screen],
    controls: &BTreeMap<String, ControlBinding>,
    has_performance: bool,
) -> Result<(), String> {
    fn text(value: &str, context: &str) -> Result<(), String> {
        if value.is_empty() || value.len() > 120 || value.chars().any(char::is_control) {
            return Err(format!(
                "{context} must be nonempty printable text of at most 120 bytes"
            ));
        }
        Ok(())
    }
    if screens.len() > 16 {
        return Err("graph permits at most 16 screens".into());
    }
    let mut ids = BTreeSet::new();
    for screen in screens {
        text(&screen.id, "screen ID")?;
        text(&screen.title, "screen title")?;
        if !ids.insert(screen.id.as_str()) {
            return Err(format!("duplicate screen ID '{}'", screen.id));
        }
        if screen.widgets.len() > 64 {
            return Err(format!("screen '{}' permits at most 64 widgets", screen.id));
        }
    }
    for screen in screens {
        for widget in &screen.widgets {
            match widget {
                Widget::Label(value) => text(value, "widget text")?,
                Widget::Control { label, control_id } => {
                    text(label, "widget label")?;
                    if !controls.contains_key(control_id) {
                        return Err(format!(
                            "screen '{}' references unknown control '{control_id}'",
                            screen.id
                        ));
                    }
                }
                Widget::Link { label, screen_id } => {
                    text(label, "widget label")?;
                    if !ids.contains(screen_id.as_str()) {
                        return Err(format!(
                            "screen '{}' links to unknown screen '{screen_id}'",
                            screen.id
                        ));
                    }
                }
                Widget::Button {
                    label,
                    control_id,
                    value,
                } => {
                    text(label, "widget label")?;
                    let control = controls.get(control_id).ok_or_else(|| {
                        format!(
                            "screen '{}' button references unknown control '{control_id}'",
                            screen.id
                        )
                    })?;
                    if !value.is_finite() || *value < control.min || *value > control.max {
                        return Err(format!(
                            "screen '{}' button value must be in {}..{} for control '{control_id}'",
                            screen.id, control.min, control.max
                        ));
                    }
                }
                Widget::PerformanceControls => {}
                Widget::PerformanceStatus | Widget::PatternGrid | Widget::TransportStatus => {
                    if !has_performance {
                        return Err(format!(
                            "screen '{}' uses a sequencer widget without a performance sequencer",
                            screen.id
                        ));
                    }
                }
            }
        }
    }
    Ok(())
}

fn validate_navigation(navigation: &Navigation, screens: &[Screen]) -> Result<(), String> {
    let screen_ids = screens
        .iter()
        .map(|screen| screen.id.as_str())
        .collect::<BTreeSet<_>>();
    if !screen_ids.contains(navigation.initial.as_str()) {
        return Err(format!(
            "navigation initial screen '{}' is unknown",
            navigation.initial
        ));
    }
    let mut event_names = BTreeSet::new();
    for event in &navigation.events {
        if event.name.is_empty() {
            return Err("navigation event name must be nonempty".into());
        }
        event_names.insert(event.name.as_str());
        if event.from != "*" && !screen_ids.contains(event.from.as_str()) {
            return Err(format!(
                "navigation event '{}' has unknown source screen '{}'",
                event.name, event.from
            ));
        }
        if !screen_ids.contains(event.to.as_str()) {
            return Err(format!(
                "navigation event '{}' has unknown target screen '{}'",
                event.name, event.to
            ));
        }
    }
    let mut keys = BTreeSet::new();
    for binding in &navigation.bindings {
        if binding.key.is_empty() || !keys.insert(binding.key.as_str()) {
            return Err(format!(
                "navigation key '{}' must be nonempty and unique",
                binding.key
            ));
        }
        if !event_names.contains(binding.event.as_str()) {
            return Err(format!(
                "navigation key '{}' references unknown event '{}'",
                binding.key, binding.event
            ));
        }
    }
    Ok(())
}

impl Machine {
    pub fn screens(&self) -> &[Screen] {
        &self.screens
    }
    pub fn process(&mut self, buffer: &mut [f64]) {
        self.prepare_buffer();
        self.process_prepared(buffer);
    }

    pub fn process_stereo(&mut self, left: &mut [f64], right: &mut [f64]) {
        self.prepare_buffer();
        self.process_stereo_prepared(left, right);
    }

    pub(crate) fn prepare_buffer(&mut self) {
        // Snapshot all published values once, before processing this buffer.
        for (index, cell) in self.published.iter().enumerate() {
            let bits = cell.load(Ordering::Acquire);
            self.params[index] = if bits == NO_OVERRIDE {
                self.authored[index]
            } else {
                f64::from_bits(bits)
            };
        }
        for pair in &mut self.note_pairs {
            let before = pair.state.epoch();
            if before & 1 == 0 {
                let read = |index: usize| {
                    let bits = self.published[index].load(Ordering::Acquire);
                    if bits == NO_OVERRIDE {
                        self.authored[index]
                    } else {
                        f64::from_bits(bits)
                    }
                };
                let pitch = read(pair.pitch_index);
                let gate = read(pair.gate_index);
                let velocity = pair.velocity_index.map(&read);
                if before == pair.state.epoch() {
                    pair.last_pitch = pitch;
                    pair.last_gate = gate;
                    if let Some(velocity) = velocity {
                        pair.last_velocity = velocity;
                    }
                }
            }
            self.params[pair.pitch_index] = pair.last_pitch;
            self.params[pair.gate_index] = pair.last_gate;
            if let Some(index) = pair.velocity_index {
                self.params[index] = pair.last_velocity;
            }
        }
    }

    pub(crate) fn process_prepared(&mut self, buffer: &mut [f64]) {
        for sample in buffer {
            self.process_one();
            *sample = self.slots[self.output_slots[0]];
        }
    }

    pub(crate) fn process_stereo_prepared(&mut self, left: &mut [f64], right: &mut [f64]) {
        assert_eq!(left.len(), right.len());
        for (left, right) in left.iter_mut().zip(right) {
            self.process_one();
            *left = self.slots[self.output_slots[0]];
            *right = self.slots[self.output_slots[1]];
        }
    }

    fn process_one(&mut self) {
        for sequencer in &mut self.sequencers {
            sequencer.begin_frame(self.sample_rate);
        }
        for op in &mut self.ops {
            for (slot, input) in op.inputs.iter().enumerate() {
                self.input_scratch[slot] = if input.sources.is_empty() {
                    input.unconnected
                } else {
                    input
                        .sources
                        .iter()
                        .fold(0.0, |sum, source| sum + self.slots[*source])
                };
            }
            for (slot, index) in op.params.iter().enumerate() {
                self.param_scratch[slot] = self.params[*index];
            }
            let inputs = &self.input_scratch[..op.inputs.len()];
            let params = &self.param_scratch[..op.params.len()];
            let outputs = &mut self.slots[op.outputs.clone()];
            match &mut op.processor {
                OpProcessor::Contract { process, state } => {
                    process(state, inputs, params, self.sample_rate, outputs)
                }
                OpProcessor::Sample(player) => {
                    player.process(inputs[0], params[0], self.sample_rate, outputs)
                }
                OpProcessor::Delay(delay) => {
                    delay.process(inputs, params, self.sample_rate, outputs)
                }
                OpProcessor::Plaits(plaits) => plaits.process(inputs, params, outputs),
                OpProcessor::SequencerTrack { sequencer, track } => {
                    outputs[0] = self.sequencers[*sequencer].trigger(*track)
                }
                OpProcessor::SequencerTempo(control) => {
                    control.bpm.store(params[0].to_bits(), Ordering::Release);
                    outputs[0] = params[0];
                }
            }
        }
    }

    pub fn order(&self) -> &[String] {
        &self.order
    }
    pub fn authored_normalizations(&self) -> &[Connection] {
        &self.authored_normalizations
    }
    pub fn effective_connections(&self) -> &[Connection] {
        &self.effective_connections
    }
    pub fn sample_rate(&self) -> f64 {
        self.sample_rate
    }
    pub fn is_stereo(&self) -> bool {
        self.stereo
    }
    pub(crate) fn revision_token(&self) -> Arc<()> {
        Arc::clone(&self.revision)
    }
    pub(crate) fn is_revision(&self, token: &Arc<()>) -> bool {
        Arc::ptr_eq(&self.revision, token)
    }
    pub(crate) fn resolve_timed_note(
        &self,
        channel: u8,
        pitch_hz: f64,
    ) -> Result<(usize, usize), String> {
        let target = self.timed_note_target(channel)?;
        if !pitch_hz.is_finite() || pitch_hz < target.min_pitch || pitch_hz > target.max_pitch {
            return Err(format!(
                "timed note pitch must be finite and within control range for channel {channel}: {}..{}",
                target.min_pitch, target.max_pitch
            ));
        }
        Ok((target.pitch_index, target.gate_index))
    }
    pub(crate) fn timed_note_target(&self, channel: u8) -> Result<TimedNoteTarget, String> {
        let binding = self
            .midi_notes
            .iter()
            .find(|binding| binding.channel == channel)
            .ok_or_else(|| format!("no MIDI note mapping for channel {channel}"))?;
        let pitch = &self.controls[&binding.pitch_control_id];
        let gate = &self.controls[&binding.gate_control_id];
        Ok(TimedNoteTarget {
            pitch_index: self.param_ids[&pitch.target],
            gate_index: self.param_ids[&gate.target],
            velocity_index: binding
                .velocity_control_id
                .as_ref()
                .map(|id| self.param_ids[&self.controls[id].target]),
            min_pitch: pitch.min,
            max_pitch: pitch.max,
        })
    }
    pub(crate) fn poly_note_targets(
        &self,
        channel: u8,
    ) -> Result<[Option<PolyVoiceTarget>; MAX_POLY_VOICES], String> {
        self.poly_notes
            .iter()
            .find(|mapping| mapping.channel == channel)
            .map(|mapping| mapping.voices)
            .ok_or_else(|| format!("no polyphonic MIDI mapping for channel {channel}"))
    }
    pub fn poly_note_channels(&self) -> impl Iterator<Item = u8> + '_ {
        self.poly_notes.iter().map(|mapping| mapping.channel)
    }
    pub(crate) fn poly_note_overflow(&self, channel: u8) -> Option<crate::model::PolyOverflow> {
        self.poly_notes
            .iter()
            .find(|mapping| mapping.channel == channel)
            .map(|mapping| mapping.overflow)
    }
    pub fn has_poly_notes(&self) -> bool {
        !self.poly_notes.is_empty()
    }
    pub(crate) fn apply_timed_note(
        &mut self,
        pitch_index: usize,
        gate_index: usize,
        pitch_hz: f64,
        gate: bool,
    ) {
        self.params[pitch_index] = pitch_hz;
        self.params[gate_index] = if gate { 1.0 } else { 0.0 };
    }
    pub(crate) fn apply_timed_velocity(&mut self, index: Option<usize>, velocity: f64) {
        if let Some(index) = index {
            self.params[index] = velocity;
        }
    }
    pub fn parameter_ids(&self) -> impl Iterator<Item = &ParameterId> {
        self.param_ids.keys()
    }
    pub fn authored_value(&self, id: &ParameterId) -> Option<f64> {
        self.param_ids.get(id).map(|&i| self.authored[i])
    }
    pub fn effective_value(&self, id: &ParameterId) -> Option<f64> {
        self.param_ids.get(id).map(|&i| {
            let bits = self.published[i].load(Ordering::Acquire);
            if bits == NO_OVERRIDE {
                self.authored[i]
            } else {
                f64::from_bits(bits)
            }
        })
    }
    pub fn parameter_handle(&self, id: &ParameterId) -> Option<ParameterHandle> {
        self.param_ids.get(id).map(|&i| ParameterHandle {
            id: id.clone(),
            cell: Arc::clone(&self.published[i]),
            range: self.param_ranges[i],
        })
    }
    pub fn control_ids(&self) -> impl Iterator<Item = &str> {
        self.controls
            .iter()
            .filter(|(_, binding)| !binding.hidden)
            .map(|(id, _)| id.as_str())
    }
    pub fn midi_ccs(&self) -> &[MidiCc] {
        &self.midi_ccs
    }
    pub fn midi_notes(&self) -> &[MidiNotes] {
        &self.midi_notes
    }
    pub fn midi_buttons(&self) -> &[MidiButton] {
        &self.midi_buttons
    }
    pub fn midi_packets(&self) -> &[MidiPacket] {
        &self.midi_packets
    }
    pub fn actions(&self) -> &[Action] {
        &self.actions
    }
    pub(crate) fn midi_note_state(&self, channel: u8) -> Option<Arc<NoteState>> {
        self.note_pairs
            .iter()
            .find(|pair| pair.channel == channel)
            .map(|pair| Arc::clone(&pair.state))
    }
    pub fn control_handle(&self, id: &str) -> Option<ControlHandle> {
        let binding = self.controls.get(id)?;
        let parameters = std::iter::once(&binding.target)
            .chain(&binding.additional_targets)
            .map(|target| self.parameter_handle(target))
            .collect::<Option<Vec<_>>>()?;
        Some(ControlHandle {
            id: id.to_owned(),
            label: binding.label.clone(),
            parameters,
            min: binding.min,
            max: binding.max,
            scale: binding.scale,
        })
    }
    pub fn set_override(&self, id: &ParameterId, value: Option<f64>) -> Result<(), String> {
        self.parameter_handle(id)
            .ok_or_else(|| format!("unknown parameter {}.{}", id.module, id.name))?
            .publish(value)
    }
    pub fn sequencer_controls(&self) -> &[Arc<SequencerControl>] {
        &self.sequencer_controls
    }
}

pub fn compile(graph: &Graph) -> Result<Machine, String> {
    compile_at_rate(graph, 48_000.0)
}

pub fn compile_at_rate(graph: &Graph, sample_rate: f64) -> Result<Machine, String> {
    if !sample_rate.is_finite() || sample_rate < 1.0 {
        return Err("sample rate must be finite and at least 1 Hz".into());
    }
    let expanded = crate::expand::expand(graph)?;
    let graph = &expanded;
    let mut ids = BTreeMap::new();
    for (index, module) in graph.modules.iter().enumerate() {
        if module.id.is_empty() {
            return Err("module ID must not be empty".into());
        }
        if ids.insert(module.id.as_str(), index).is_some() {
            return Err(format!("duplicate module ID '{}'", module.id));
        }
        let contract = contract::find(&module.kind)
            .ok_or_else(|| format!("unknown module type '{}'", module.kind))?;
        for (name, value) in &module.values {
            let param = contract
                .params
                .iter()
                .find(|p| p.name == name)
                .ok_or_else(|| format!("{}.{} is not a parameter", module.id, name))?;
            validate_parameter_value(&ParameterId::new(&module.id, name), param.range, *value)?;
        }
    }
    let output_slots: Vec<usize> = graph
        .modules
        .iter()
        .enumerate()
        .filter(|(_, module)| {
            contract::find(&module.kind).expect("module checked").role != Role::Processor
        })
        .map(|(index, _)| index)
        .collect();
    if output_slots.len() != 1 {
        return Err(format!(
            "expected exactly one output module, found {}",
            output_slots.len()
        ));
    }

    // Slots belong to declared output ports. Terminal output roles have no
    // connectable outputs and own one or two private final-sample slots.
    let mut slot_bases = Vec::with_capacity(graph.modules.len());
    let mut slot_count = 0;
    for module in &graph.modules {
        slot_bases.push(slot_count);
        let c = contract::find(&module.kind).expect("module checked");
        slot_count += match c.role {
            Role::StereoOutput => 2,
            _ => c.outputs.len().max(1),
        };
    }

    // Validate both authored edge sets before selecting effective edges. An
    // overridden normalization is still an authored connection and must be valid.
    let resolve = |edge: &Connection| -> Result<(usize, usize, usize, FanIn), String> {
        let &src = ids
            .get(edge.from.module.as_str())
            .ok_or_else(|| format!("unknown source module '{}'", edge.from.module))?;
        let &dst = ids
            .get(edge.to.module.as_str())
            .ok_or_else(|| format!("unknown destination module '{}'", edge.to.module))?;
        let sc = contract::find(&graph.modules[src].kind).expect("module checked");
        let dc = contract::find(&graph.modules[dst].kind).expect("module checked");
        let sp = sc
            .outputs
            .iter()
            .enumerate()
            .find(|(_, p)| p.name == edge.from.port)
            .ok_or_else(|| {
                format!(
                    "invalid output port '{}.{}'",
                    edge.from.module, edge.from.port
                )
            })?;
        let dp = dc
            .inputs
            .iter()
            .find(|p| p.name == edge.to.port)
            .ok_or_else(|| format!("invalid input port '{}.{}'", edge.to.module, edge.to.port))?;
        if sp.1.domain != dp.domain {
            return Err(format!(
                "incompatible ports '{}.{}' and '{}.{}'",
                edge.from.module, edge.from.port, edge.to.module, edge.to.port
            ));
        }
        Ok((src, dst, slot_bases[src] + sp.0, dp.fan_in))
    };
    for edge in graph.connections.iter().chain(&graph.normalizations) {
        resolve(edge)?;
    }
    let mut normalized_destinations = BTreeSet::new();
    for edge in &graph.normalizations {
        if !normalized_destinations.insert(&edge.to) {
            return Err(format!(
                "ambiguous normalization for input '{}.{}'",
                edge.to.module, edge.to.port
            ));
        }
    }
    let explicit_destinations: BTreeSet<_> =
        graph.connections.iter().map(|edge| &edge.to).collect();
    let effective_connections: Vec<Connection> = graph
        .connections
        .iter()
        .chain(
            graph
                .normalizations
                .iter()
                .filter(|edge| !explicit_destinations.contains(&edge.to)),
        )
        .cloned()
        .collect();

    let mut input_edges: BTreeMap<(usize, &str), Vec<(usize, usize)>> = BTreeMap::new();
    let mut dependents = vec![Vec::<usize>::new(); graph.modules.len()];
    let mut indegree = vec![0usize; graph.modules.len()];
    for edge in &effective_connections {
        let (src, dst, source_slot, fan_in) = resolve(edge)?;
        let key = (dst, edge.to.port.as_str());
        let sources = input_edges.entry(key).or_default();
        if !sources.is_empty() && fan_in == FanIn::One {
            return Err(format!(
                "input '{}.{}' permits one connection",
                edge.to.module, edge.to.port
            ));
        }
        sources.push((src, source_slot));
        dependents[src].push(dst);
        indegree[dst] += 1;
    }
    for (i, module) in graph.modules.iter().enumerate() {
        let c = contract::find(&module.kind).expect("module checked");
        for port in c.inputs {
            if !input_edges.contains_key(&(i, port.name)) && port.unconnected.is_none() {
                return Err(format!(
                    "required input '{}.{}' is disconnected",
                    module.id, port.name
                ));
            }
        }
    }
    let mut ready: BTreeSet<(String, usize)> = graph
        .modules
        .iter()
        .enumerate()
        .filter(|(i, _)| indegree[*i] == 0)
        .map(|(i, m)| (m.id.clone(), i))
        .collect();
    let mut schedule = Vec::with_capacity(graph.modules.len());
    while let Some((_, i)) = ready.pop_first() {
        schedule.push(i);
        for &next in &dependents[i] {
            indegree[next] -= 1;
            if indegree[next] == 0 {
                ready.insert((graph.modules[next].id.clone(), next));
            }
        }
    }
    if schedule.len() != graph.modules.len() {
        return Err("graph contains an unsupported cycle".into());
    }

    let mut params = Vec::new();
    let mut param_ids = BTreeMap::new();
    let mut param_ranges = Vec::new();
    for &i in &schedule {
        let module = &graph.modules[i];
        let c = contract::find(&module.kind).expect("module checked");
        for p in c.params {
            let index = params.len();
            params.push(*module.values.get(p.name).unwrap_or(&p.default));
            param_ranges.push(p.range);
            param_ids.insert(ParameterId::new(&module.id, p.name), index);
        }
    }
    let mut controls = BTreeMap::new();
    for control in &graph.controls {
        if control.id.is_empty() {
            return Err("control ID must not be empty".into());
        }
        if control.label.is_empty() || control.label.chars().any(char::is_control) {
            return Err(format!(
                "control '{}' requires a printable label",
                control.id
            ));
        }
        if !control.min.is_finite()
            || !control.max.is_finite()
            || control.min >= control.max
            || !(control.max - control.min).is_finite()
        {
            return Err(format!(
                "control '{}' requires finite min < max",
                control.id
            ));
        }
        if control.scale == ControlScale::Logarithmic && control.min <= 0.0 {
            return Err(format!(
                "logarithmic control '{}' requires a positive minimum",
                control.id
            ));
        }
        let mut unique_targets = BTreeSet::new();
        for target in std::iter::once(&control.target).chain(&control.additional_targets) {
            if !unique_targets.insert(target) {
                return Err(format!(
                    "control '{}' repeats a parameter target",
                    control.id
                ));
            }
            let Some(&parameter_index) = param_ids.get(target) else {
                return Err(format!(
                    "control '{}' targets unknown parameter '{}.{}'",
                    control.id, target.module, target.name
                ));
            };
            if let Some((min, max)) = param_ranges[parameter_index]
                && (control.min < min || control.max > max)
            {
                return Err(format!(
                    "control '{}' range must stay within {}.{} {min}..{max}",
                    control.id, target.module, target.name
                ));
            }
        }
        if controls
            .insert(
                control.id.clone(),
                ControlBinding {
                    label: control.label.clone(),
                    target: control.target.clone(),
                    additional_targets: control.additional_targets.clone(),
                    min: control.min,
                    max: control.max,
                    scale: control.scale,
                    hidden: control.hidden,
                },
            )
            .is_some()
        {
            return Err(format!("duplicate control ID '{}'", control.id));
        }
    }
    let mut mapped_ccs = BTreeSet::new();
    for binding in &graph.midi_ccs {
        if !(1..=16).contains(&binding.channel) || binding.number > 127 {
            return Err(format!(
                "MIDI CC mapping for '{}' requires channel 1..16 and number 0..127",
                binding.control_id
            ));
        }
        if !controls.contains_key(&binding.control_id) {
            return Err(format!(
                "MIDI CC mapping targets unknown control '{}'",
                binding.control_id
            ));
        }
        if !mapped_ccs.insert((binding.channel, binding.number)) {
            return Err(format!(
                "duplicate MIDI CC mapping for channel {} CC {}",
                binding.channel, binding.number
            ));
        }
    }
    let action_labels = graph
        .actions
        .iter()
        .map(|action| action.label.as_str())
        .collect::<BTreeSet<_>>();
    if action_labels.len() != graph.actions.len() {
        return Err("action labels must be unique".into());
    }
    let mut mapped_buttons = BTreeSet::new();
    for button in &graph.midi_buttons {
        if button.name.is_empty() || !(1..=16).contains(&button.channel) || button.number > 127 {
            return Err(format!(
                "MIDI button '{}' requires a name, channel 1..16, and number 0..127",
                button.name
            ));
        }
        if !action_labels.contains(button.action_label.as_str()) {
            return Err(format!(
                "MIDI button '{}' references unknown action '{}'",
                button.name, button.action_label
            ));
        }
        if button.message == crate::model::MidiButtonMessage::ProgramChange
            && button.edge == crate::model::MidiButtonEdge::Release
        {
            return Err(format!(
                "MIDI Program Change button '{}' has no release edge",
                button.name
            ));
        }
        let message = match button.message {
            crate::model::MidiButtonMessage::Note => 0u8,
            crate::model::MidiButtonMessage::ControlChange => 1,
            crate::model::MidiButtonMessage::ProgramChange => 2,
        };
        let edge = match button.edge {
            crate::model::MidiButtonEdge::Press => 0u8,
            crate::model::MidiButtonEdge::Release => 1,
        };
        if !mapped_buttons.insert((message, button.channel, button.number, edge)) {
            return Err(format!("duplicate MIDI button edge for '{}'", button.name));
        }
    }
    let mut mapped_packets = BTreeSet::new();
    for packet in &graph.midi_packets {
        if packet.name.is_empty() || packet.bytes.is_empty() {
            return Err("raw MIDI packet hooks require a name and at least one byte".into());
        }
        if !action_labels.contains(packet.action_label.as_str()) {
            return Err(format!(
                "raw MIDI hook '{}' references unknown action '{}'",
                packet.name, packet.action_label
            ));
        }
        if !mapped_packets.insert(packet.bytes.as_slice()) {
            return Err(format!(
                "duplicate raw MIDI packet hook for '{}'",
                packet.name
            ));
        }
    }
    let mut mapped_note_channels = BTreeSet::new();
    let mut mapped_note_targets = BTreeSet::new();
    for binding in &graph.midi_notes {
        if !(1..=16).contains(&binding.channel) {
            return Err(format!(
                "MIDI note mapping requires channel 1..16, got {}",
                binding.channel
            ));
        }
        if !mapped_note_channels.insert(binding.channel) {
            return Err(format!(
                "duplicate MIDI note mapping for channel {}",
                binding.channel
            ));
        }
        let pitch = controls.get(&binding.pitch_control_id).ok_or_else(|| {
            format!(
                "MIDI note mapping targets unknown pitch control '{}'",
                binding.pitch_control_id
            )
        })?;
        let gate = controls.get(&binding.gate_control_id).ok_or_else(|| {
            format!(
                "MIDI note mapping targets unknown gate control '{}'",
                binding.gate_control_id
            )
        })?;
        if pitch.target == gate.target {
            return Err(
                "MIDI note pitch and gate controls must target different parameters".into(),
            );
        }
        if !mapped_note_targets.insert(&pitch.target) || !mapped_note_targets.insert(&gate.target) {
            return Err("MIDI note mappings must use distinct pitch and gate targets".into());
        }
        let lowest_hz = 440.0 * 2.0_f64.powf((0.0 - 69.0) / 12.0);
        let highest_hz = 440.0 * 2.0_f64.powf((127.0 - 69.0) / 12.0);
        if pitch.min > lowest_hz || pitch.max < highest_hz {
            return Err(format!(
                "MIDI note pitch control '{}' must cover all MIDI note frequencies",
                binding.pitch_control_id
            ));
        }
        if gate.min > 0.0 || gate.max < 1.0 {
            return Err(format!(
                "MIDI note gate control '{}' must cover 0..1",
                binding.gate_control_id
            ));
        }
        if let Some(id) = &binding.velocity_control_id {
            let velocity = controls.get(id).ok_or_else(|| {
                format!("MIDI note mapping targets unknown velocity control '{id}'")
            })?;
            if velocity.target == pitch.target
                || velocity.target == gate.target
                || !mapped_note_targets.insert(&velocity.target)
            {
                return Err("MIDI note velocity must target a distinct parameter".into());
            }
            if velocity.min > 0.0 || velocity.max < 1.0 {
                return Err(format!("MIDI note velocity control '{id}' must cover 0..1"));
            }
        }
    }
    let mut poly_notes = Vec::new();
    for binding in &graph.midi_poly {
        if !(1..=16).contains(&binding.channel) {
            return Err(format!(
                "polyphonic MIDI mapping requires channel 1..16, got {}",
                binding.channel
            ));
        }
        if !mapped_note_channels.insert(binding.channel) {
            return Err(format!(
                "duplicate MIDI note mapping for channel {}",
                binding.channel
            ));
        }
        if !(2..=MAX_POLY_VOICES).contains(&binding.voices.len()) {
            return Err("polyphonic MIDI mapping requires two to four voices".into());
        }
        let mut voices = [None; MAX_POLY_VOICES];
        for (number, voice) in binding.voices.iter().enumerate() {
            let resolve = |id: &str| {
                controls.get(id).ok_or_else(|| {
                    format!(
                        "polyphonic voice {} targets unknown control '{id}'",
                        number + 1
                    )
                })
            };
            let pitch = resolve(&voice.pitch_control_id)?;
            let gate = resolve(&voice.gate_control_id)?;
            let velocity = resolve(&voice.velocity_control_id)?;
            for target in [&pitch.target, &gate.target, &velocity.target] {
                if !mapped_note_targets.insert(target) {
                    return Err("polyphonic MIDI voices must use distinct parameter targets".into());
                }
            }
            let lowest_hz = 440.0 * 2.0_f64.powf((0.0 - 69.0) / 12.0);
            let highest_hz = 440.0 * 2.0_f64.powf((127.0 - 69.0) / 12.0);
            if pitch.min > lowest_hz || pitch.max < highest_hz {
                return Err(format!(
                    "polyphonic pitch control '{}' must cover all MIDI note frequencies",
                    voice.pitch_control_id
                ));
            }
            if gate.min > 0.0 || gate.max < 1.0 {
                return Err(format!(
                    "polyphonic gate control '{}' must cover 0..1",
                    voice.gate_control_id
                ));
            }
            if velocity.min > 0.0 || velocity.max < 1.0 {
                return Err(format!(
                    "polyphonic velocity control '{}' must cover 0..1",
                    voice.velocity_control_id
                ));
            }
            voices[number] = Some(PolyVoiceTarget {
                pitch_index: param_ids[&pitch.target],
                gate_index: param_ids[&gate.target],
                velocity_index: param_ids[&velocity.target],
            });
        }
        poly_notes.push(PolyMapping {
            channel: binding.channel,
            voices,
            overflow: binding.overflow,
        });
    }
    validate_screens(&graph.screens, &controls, graph.performance.is_some())?;
    if let Some(navigation) = &graph.navigation {
        validate_navigation(navigation, &graph.screens)?;
    }
    let note_pairs = graph
        .midi_notes
        .iter()
        .map(|binding| {
            let pitch_index = param_ids[&controls[&binding.pitch_control_id].target];
            let gate_index = param_ids[&controls[&binding.gate_control_id].target];
            let velocity_index = binding
                .velocity_control_id
                .as_ref()
                .map(|id| param_ids[&controls[id].target]);
            NotePair {
                channel: binding.channel,
                pitch_index,
                gate_index,
                velocity_index,
                state: Arc::new(NoteState::new()),
                last_pitch: params[pitch_index],
                last_gate: params[gate_index],
                last_velocity: velocity_index.map_or(1.0, |index| params[index]),
            }
        })
        .collect();
    let mut ops = Vec::with_capacity(schedule.len());
    let mut sequencer_controls: Vec<Arc<SequencerControl>> = Vec::new();
    let mut sequencer_indexes = BTreeMap::<usize, usize>::new();
    for &i in &schedule {
        let module = &graph.modules[i];
        let c = contract::find(&module.kind).expect("module checked");
        let inputs = c
            .inputs
            .iter()
            .map(|port| {
                let mut sources = input_edges
                    .get(&(i, port.name))
                    .cloned()
                    .unwrap_or_default();
                sources.sort_by(|a, b| {
                    graph.modules[a.0]
                        .id
                        .cmp(&graph.modules[b.0].id)
                        .then(a.1.cmp(&b.1))
                });
                InputPlan {
                    sources: sources.into_iter().map(|(_, slot)| slot).collect(),
                    unconnected: port.unconnected.unwrap_or(0.0),
                }
            })
            .collect();
        let params = c
            .params
            .iter()
            .map(|p| param_ids[&ParameterId::new(&module.id, p.name)])
            .collect();
        let processor = if module.kind == "delay"
            && matches!(module.runtime, ModuleRuntime::Contract)
        {
            OpProcessor::Delay(DelayRuntime::new(sample_rate)?)
        } else {
            match &module.runtime {
                ModuleRuntime::Contract => OpProcessor::Contract {
                    process: c.process,
                    state: vec![0.0; c.state_len],
                },
                ModuleRuntime::Sample(sample) if module.kind == "sample_player" => {
                    OpProcessor::Sample(SamplePlayerRuntime::new(Arc::clone(sample), sample_rate)?)
                }
                ModuleRuntime::Plaits(model) if module.kind == "plaits" => {
                    OpProcessor::Plaits(Box::new(PlaitsRuntime::new(*model, sample_rate)))
                }
                ModuleRuntime::SequencerTrack { sequencer, track }
                    if module.kind == "sequencer_track" =>
                {
                    let key = Arc::as_ptr(sequencer) as usize;
                    let index = *sequencer_indexes.entry(key).or_insert_with(|| {
                        sequencer_controls.push(Arc::clone(sequencer));
                        sequencer_controls.len() - 1
                    });
                    OpProcessor::SequencerTrack {
                        sequencer: index,
                        track: *track,
                    }
                }
                ModuleRuntime::SequencerTempo(sequencer) if module.kind == "sequencer_tempo" => {
                    OpProcessor::SequencerTempo(Arc::clone(sequencer))
                }
                _ => {
                    return Err(format!(
                        "module '{}' has incompatible runtime data",
                        module.id
                    ));
                }
            }
        };
        ops.push(Op {
            processor,
            inputs,
            outputs: slot_bases[i]
                ..slot_bases[i]
                    + match c.role {
                        Role::StereoOutput => 2,
                        _ => c.outputs.len().max(1),
                    },
            params,
        });
    }
    let output_base = slot_bases[output_slots[0]];
    let stereo = contract::find(&graph.modules[output_slots[0]].kind)
        .expect("module checked")
        .role
        == Role::StereoOutput;
    let sequencers = sequencer_controls
        .iter()
        .cloned()
        .map(SequencerRuntime::new)
        .collect();
    Ok(Machine {
        revision: Arc::new(()),
        ops,
        slots: vec![0.0; slot_count],
        input_scratch: vec![
            0.0;
            graph
                .modules
                .iter()
                .map(|m| contract::find(&m.kind)
                    .expect("module checked")
                    .inputs
                    .len())
                .max()
                .unwrap_or(0)
        ],
        param_scratch: vec![
            0.0;
            graph
                .modules
                .iter()
                .map(|m| contract::find(&m.kind)
                    .expect("module checked")
                    .params
                    .len())
                .max()
                .unwrap_or(0)
        ],
        authored: params.clone(),
        published: (0..params.len())
            .map(|_| Arc::new(AtomicU64::new(NO_OVERRIDE)))
            .collect(),
        params,
        param_ranges,
        param_ids,
        controls,
        screens: graph.screens.clone(),
        midi_ccs: graph.midi_ccs.clone(),
        midi_notes: graph.midi_notes.clone(),
        midi_buttons: graph.midi_buttons.clone(),
        midi_packets: graph.midi_packets.clone(),
        actions: graph.actions.clone(),
        poly_notes,
        note_pairs,
        output_slots: [
            output_base,
            if stereo { output_base + 1 } else { output_base },
        ],
        stereo,
        order: schedule
            .iter()
            .map(|&i| graph.modules[i].id.clone())
            .collect(),
        authored_normalizations: graph.normalizations.clone(),
        effective_connections,
        sample_rate,
        sequencers,
        sequencer_controls,
    })
}

#[derive(Default)]
pub struct Host {
    accepted: Option<Machine>,
}

impl Host {
    pub fn submit(&mut self, candidate: &Graph) -> Result<(), String> {
        let machine = compile(candidate)?;
        self.accepted = Some(machine);
        Ok(())
    }
    pub fn accepted(&mut self) -> Option<&mut Machine> {
        self.accepted.as_mut()
    }
}

#[cfg(test)]
mod sample_player_tests {
    use super::*;
    fn sample(rate: u32, left: &[f64], right: &[f64]) -> Arc<PreparedSample> {
        Arc::new(PreparedSample {
            identity: "test".into(),
            source_version: "v1".into(),
            sample_rate: rate,
            channels: 2,
            left: left.to_vec().into(),
            right: right.to_vec().into(),
        })
    }
    #[test]
    fn player_outputs_stereo_resamples_and_completes() {
        let mut player =
            SamplePlayerRuntime::new(sample(2, &[1.0, 0.5], &[-1.0, -0.5]), 4.0).unwrap();
        let mut out = [0.0; 2];
        player.process(1.0, 0.5, 4.0, &mut out);
        assert_eq!(out, [0.5, -0.5]);
        player.process(0.0, 0.5, 4.0, &mut out);
        assert_eq!(out, [0.375, -0.375]);
        for _ in 0..3 {
            player.process(0.0, 0.5, 4.0, &mut out);
        }
        player.process(0.0, 0.5, 4.0, &mut out);
        assert_eq!(out, [0.0, 0.0]);
        assert!(out.iter().all(|v| v.is_finite()));
    }
    #[test]
    fn retriggers_are_limited_by_sample_lifetime_instead_of_a_voice_cap() {
        let mut player =
            SamplePlayerRuntime::new(sample(48_000, &[1.0; 64], &[1.0; 64]), 48_000.0).unwrap();
        let mut out = [0.0; 2];
        for overlap in 1..=20 {
            player.process(1.0, 1.0, 48_000.0, &mut out);
            assert_eq!(out, [f64::from(overlap), f64::from(overlap)]);
            player.process(0.0, 1.0, 48_000.0, &mut out);
        }
        assert_eq!(player.voices.len(), 20);
        assert_eq!(player.max_voices, 32);
    }
}
