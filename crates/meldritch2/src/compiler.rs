use std::collections::{BTreeMap, BTreeSet};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use std::time::{Duration, Instant};

use rayon::prelude::*;

use crate::contract::{self, FanIn, Role};
use crate::midi::NoteState;
use crate::model::{
    Action, AutomationInterpolation, Connection, ControlScale, Graph, MidiButton, MidiCc,
    MidiNotes, MidiPacket, ModuleRuntime, Navigation, ParameterId, Screen, Widget,
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
    StereoDelay(StereoDelayRuntime),
    Plaits(Box<PlaitsRuntime>),
    SequencerTrack {
        sequencer: usize,
        track: usize,
    },
    SequencerTempo(Arc<SequencerControl>),
}

const MAX_DELAY_SECONDS: f64 = 4.0;

#[derive(Debug)]
struct DelayRuntime {
    line: Vec<f64>,
    write: usize,
}

impl DelayRuntime {
    fn new(sample_rate: f64) -> Result<Self, String> {
        let frames = (sample_rate * MAX_DELAY_SECONDS).ceil() as usize + 2;
        let mut line = Vec::new();
        line.try_reserve_exact(frames)
            .map_err(|_| "not enough memory to prepare four-second delay line".to_string())?;
        line.resize(frames, 0.0);
        Ok(Self { line, write: 0 })
    }

    fn process(&mut self, input: &[f64], params: &[f64], sample_rate: f64, out: &mut [f64]) {
        let dry = if input[0].is_finite() { input[0] } else { 0.0 };
        let modulation = |value: f64| if value.is_finite() { value } else { 0.0 };
        let time = (params[0] + modulation(input[1])).clamp(1.0 / sample_rate, MAX_DELAY_SECONDS);
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

#[derive(Debug)]
struct StereoDelayRuntime {
    lines: [Vec<f64>; 2],
    write: usize,
    delay_samples: [f64; 2],
    initialized: bool,
    feedback_filter: [f64; 2],
}

impl StereoDelayRuntime {
    fn new(sample_rate: f64) -> Result<Self, String> {
        let frames = (sample_rate * MAX_DELAY_SECONDS).ceil() as usize + 2;
        let prepare = || -> Result<Vec<f64>, String> {
            let mut line = Vec::new();
            line.try_reserve_exact(frames).map_err(|_| {
                "not enough memory to prepare four-second stereo delay lines".to_string()
            })?;
            line.resize(frames, 0.0);
            Ok(line)
        };
        Ok(Self {
            lines: [prepare()?, prepare()?],
            write: 0,
            delay_samples: [0.0; 2],
            initialized: false,
            feedback_filter: [0.0; 2],
        })
    }

    fn finite(value: f64, fallback: f64) -> f64 {
        if value.is_finite() { value } else { fallback }
    }

    fn read(line: &[f64], write: usize, delay_samples: f64) -> f64 {
        let read = (write as f64 - delay_samples).rem_euclid(line.len() as f64);
        let first = read.floor() as usize;
        let second = (first + 1) % line.len();
        let fraction = read - first as f64;
        line[first] + (line[second] - line[first]) * fraction
    }

    fn process(&mut self, input: &[f64], params: &[f64], sample_rate: f64, out: &mut [f64]) {
        let dry = [Self::finite(input[0], 0.0), Self::finite(input[1], 0.0)];
        let bpm = Self::finite(input[2], 120.0).clamp(1.0, 1_000.0);
        let seconds_per_beat = 60.0 / bpm;
        let target = [
            (params[0] * seconds_per_beat + Self::finite(input[3], 0.0))
                .clamp(1.0 / sample_rate, MAX_DELAY_SECONDS)
                * sample_rate,
            (params[1] * seconds_per_beat + Self::finite(input[4], 0.0))
                .clamp(1.0 / sample_rate, MAX_DELAY_SECONDS)
                * sample_rate,
        ];
        if !self.initialized {
            self.delay_samples = target;
            self.initialized = true;
        } else {
            // A 20 ms glide prevents an instantaneous read-head jump while
            // retaining slow chorus-style modulation and tempo following.
            let slew = 1.0 - (-1.0 / (0.020 * sample_rate)).exp();
            for (current, target) in self.delay_samples.iter_mut().zip(target) {
                *current += (target - *current) * slew;
            }
        }

        let wet = [
            Self::read(&self.lines[0], self.write, self.delay_samples[0]),
            Self::read(&self.lines[1], self.write, self.delay_samples[1]),
        ];
        let feedback = (params[2] + Self::finite(input[5], 0.0)).clamp(-0.95, 0.95);
        let crossfeed = params[3].clamp(0.0, 1.0);
        let mix = (params[4] + Self::finite(input[6], 0.0)).clamp(0.0, 1.0);
        let cutoff = (params[5] + Self::finite(input[7], 0.0)).clamp(20.0, sample_rate * 0.49);
        let filter_coefficient = 1.0 - (-std::f64::consts::TAU * cutoff / sample_rate).exp();
        let crossed = [
            wet[0] * (1.0 - crossfeed) + wet[1] * crossfeed,
            wet[1] * (1.0 - crossfeed) + wet[0] * crossfeed,
        ];
        let drive = params[6].clamp(0.1, 8.0);
        let normalization = drive.tanh();
        for channel in 0..2 {
            self.feedback_filter[channel] +=
                (crossed[channel] - self.feedback_filter[channel]) * filter_coefficient;
            let saturated = (self.feedback_filter[channel] * drive).tanh() / normalization;
            self.lines[channel][self.write] = dry[channel] + saturated * feedback;
            out[channel] = dry[channel] * (1.0 - mix) + wet[channel] * mix;
            if !out[channel].is_finite() {
                out[channel] = 0.0;
            }
        }
        self.write = (self.write + 1) % self.lines[0].len();
    }
}

#[cfg(test)]
mod delay_runtime_tests {
    use super::{DelayRuntime, StereoDelayRuntime};

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

    #[test]
    fn stereo_fractional_taps_and_independent_routing_are_exact() {
        let mut delay = StereoDelayRuntime::new(100.0).unwrap();
        // At 120 BPM, 0.05 beat is 2.5 samples and 0.1 beat is 5 samples.
        let params = [0.05, 0.1, 0.0, 0.0, 1.0, 20_000.0, 1.0];
        let mut rendered = Vec::new();
        for frame in 0..8 {
            let mut out = [0.0; 2];
            delay.process(
                &[f64::from(frame == 0), 0.0, 120.0, 0.0, 0.0, 0.0, 0.0, 0.0],
                &params,
                100.0,
                &mut out,
            );
            rendered.push(out);
        }
        assert!((rendered[2][0] - 0.5).abs() < 1e-12);
        assert!((rendered[3][0] - 0.5).abs() < 1e-12);
        assert_eq!(rendered[5][1], 0.0);
    }

    #[test]
    fn ping_pong_feedback_crosses_channels_and_stays_bounded() {
        let mut delay = StereoDelayRuntime::new(100.0).unwrap();
        let params = [0.2, 0.2, 0.9, 1.0, 1.0, 49.0, 1.5];
        let mut rendered = Vec::new();
        for frame in 0..80 {
            let mut out = [0.0; 2];
            delay.process(
                &[f64::from(frame == 0), 0.0, 120.0, 0.0, 0.0, 0.0, 0.0, 0.0],
                &params,
                100.0,
                &mut out,
            );
            assert!(
                out.iter()
                    .all(|sample| sample.is_finite() && sample.abs() <= 2.0)
            );
            rendered.push(out);
        }
        assert!(rendered[10][0] > 0.9);
        assert!(rendered[20][1].abs() > 0.2);
        assert!(rendered[20][0].abs() < 1e-9);
    }

    #[test]
    fn tempo_modulation_and_parameter_changes_glide_without_resetting_state() {
        let mut delay = StereoDelayRuntime::new(1_000.0).unwrap();
        let mut params = [0.25, 0.375, 0.8, 0.7, 0.7, 1_500.0, 2.0];
        let mut energy_after_change = 0.0;
        for frame in 0..4_000 {
            if frame == 900 {
                params[0] = 0.75;
                params[1] = 0.5;
            }
            let modulation = (frame as f64 / 300.0).sin() * 0.005;
            let mut out = [0.0; 2];
            delay.process(
                &[
                    f64::from(frame == 0),
                    0.0,
                    if frame < 2_000 { 120.0 } else { 90.0 },
                    modulation,
                    -modulation,
                    f64::NAN,
                    0.0,
                    0.0,
                ],
                &params,
                1_000.0,
                &mut out,
            );
            assert!(
                out.iter()
                    .all(|sample| sample.is_finite() && sample.abs() <= 2.0)
            );
            if frame > 900 {
                energy_after_change += out[0].abs() + out[1].abs();
            }
        }
        assert!(
            energy_after_change > 0.01,
            "delay state was lost after parameter edit"
        );
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct SampleVoice {
    position: f64,
    increment: f64,
    low: usize,
    high: usize,
}

#[derive(Debug)]
struct SamplePlayerRuntime {
    sample: Arc<PreparedSample>,
    voices: Vec<SampleVoice>,
    max_voices: usize,
    last_trigger: f64,
}

impl SamplePlayerRuntime {
    const MAX_VOICES: usize = 128;

    fn new(sample: Arc<PreparedSample>, output_rate: f64) -> Result<Self, String> {
        if sample.frames() == 0 || !output_rate.is_finite() || output_rate <= 0.0 {
            return Err("sample player requires nonempty audio and a positive output rate".into());
        }
        // Bound both callback work and storage for long field recordings. At
        // overflow the oldest voice is stolen deterministically.
        let max_voices = Self::MAX_VOICES;
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
    fn process(&mut self, trigger: f64, params: &[f64], output_rate: f64, out: &mut [f64]) {
        if trigger > 0.0 && self.last_trigger <= 0.0 {
            if self.voices.len() == self.max_voices {
                self.voices.remove(0);
            }
            let frames = self.sample.frames();
            let start = params[1].clamp(0.0, 1.0);
            let finish = params[2].clamp(0.0, 1.0);
            let low = ((start.min(finish) * frames as f64).floor() as usize).min(frames - 1);
            let high = ((start.max(finish) * frames as f64).ceil() as usize).clamp(low + 1, frames);
            let rate = if params[3].abs() < 0.01 {
                if params[3] < 0.0 { -0.01 } else { 0.01 }
            } else {
                params[3]
            };
            let increment = f64::from(self.sample.sample_rate) / output_rate * rate;
            let position = if increment < 0.0 {
                (high - 1) as f64
            } else {
                low as f64
            };
            self.voices.push(SampleVoice {
                position,
                increment,
                low,
                high,
            });
        }
        self.last_trigger = trigger;
        let mut left = 0.0;
        let mut right = 0.0;
        for voice in &mut self.voices {
            let index = (voice.position.floor() as usize).clamp(voice.low, voice.high - 1);
            let fraction = voice.position - index as f64;
            let next = (index + 1).min(voice.high - 1);
            left += self.sample.left[index]
                + (self.sample.left[next] - self.sample.left[index]) * fraction;
            right += self.sample.right[index]
                + (self.sample.right[next] - self.sample.right[index]) * fraction;
            voice.position += voice.increment;
        }
        self.voices.retain(|voice| {
            voice.position >= voice.low as f64 && voice.position < voice.high as f64
        });
        out[0] = left * params[0];
        out[1] = right * params[0];
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
    observed: Arc<AtomicU64>,
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

    pub fn effective_value(&self) -> f64 {
        self.override_value()
            .unwrap_or_else(|| f64::from_bits(self.observed.load(Ordering::Acquire)))
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
    hardware_pickup: Arc<std::sync::atomic::AtomicI8>,
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
    pub fn effective_value(&self) -> f64 {
        self.parameters[0].effective_value()
    }
    pub(crate) fn set_hardware_pickup(&self, direction: i8) {
        self.hardware_pickup
            .store(direction.clamp(-1, 2), Ordering::Release);
    }
    pub(crate) fn hardware_pickup(&self) -> i8 {
        self.hardware_pickup.load(Ordering::Acquire)
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
    observed: Vec<Arc<AtomicU64>>,
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
    op_ids: Vec<String>,
    authored_normalizations: Vec<Connection>,
    effective_connections: Vec<Connection>,
    sample_rate: f64,
    sequencers: Vec<SequencerRuntime>,
    sequencer_controls: Vec<Arc<SequencerControl>>,
    level_ranges: Vec<std::ops::Range<usize>>,
    automations: Vec<AutomationRuntime>,
}

#[derive(Debug)]
struct AutomationRuntime {
    sequencer: Arc<SequencerControl>,
    targets: Vec<usize>,
    interpolation: AutomationInterpolation,
    points: Vec<(u64, f64)>,
}

impl AutomationRuntime {
    fn value(&self, position: u64) -> f64 {
        let next = self.points.partition_point(|(at, _)| *at <= position);
        if next == 0 {
            return self.points[0].1;
        }
        if next == self.points.len() || self.interpolation == AutomationInterpolation::Step {
            return self.points[next - 1].1;
        }
        let (from_at, from) = self.points[next - 1];
        let (to_at, to) = self.points[next];
        let mut phase = (position - from_at) as f64 / (to_at - from_at) as f64;
        if self.interpolation == AutomationInterpolation::Smooth {
            phase = phase * phase * (3.0 - 2.0 * phase);
        }
        from + (to - from) * phase
    }
}

#[cfg(test)]
mod automation_runtime_tests {
    use super::*;
    use crate::sequencer::SequencerControl;

    fn lane(interpolation: AutomationInterpolation) -> AutomationRuntime {
        AutomationRuntime {
            sequencer: SequencerControl::new("automation".into(), 120.0, 16, 4).unwrap(),
            targets: vec![0],
            interpolation,
            points: vec![(0, 0.0), (8, 1.0)],
        }
    }

    #[test]
    fn step_linear_and_smooth_automation_are_deterministic() {
        assert_eq!(lane(AutomationInterpolation::Step).value(4), 0.0);
        assert_eq!(lane(AutomationInterpolation::Linear).value(4), 0.5);
        assert!((lane(AutomationInterpolation::Smooth).value(2) - 0.15625).abs() < 1e-12);
        assert_eq!(lane(AutomationInterpolation::Linear).value(8), 1.0);
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SignalStats {
    pub samples: u64,
    pub active_samples: u64,
    pub non_finite_samples: u64,
    pub peak: f64,
    pub sum_squares: f64,
}

impl SignalStats {
    fn observe(&mut self, value: f64) {
        self.samples += 1;
        if !value.is_finite() {
            self.non_finite_samples += 1;
            return;
        }
        self.peak = self.peak.max(value.abs());
        self.sum_squares += value * value;
        if value.abs() > 1e-9 {
            self.active_samples += 1;
        }
    }

    pub fn rms(&self) -> f64 {
        if self.samples == 0 {
            0.0
        } else {
            (self.sum_squares / self.samples as f64).sqrt()
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ModuleActivity {
    pub id: String,
    pub inputs: Vec<SignalStats>,
    pub outputs: Vec<SignalStats>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TrackActivity {
    pub source: String,
    pub track: String,
    pub events: u64,
    pub audible_frames: u64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct RenderDiagnostics {
    pub frames: u64,
    pub modules: Vec<ModuleActivity>,
    pub tracks: Vec<TrackActivity>,
}

impl RenderDiagnostics {
    pub fn module(&self, id: &str) -> Option<&ModuleActivity> {
        self.modules.iter().find(|module| module.id == id)
    }

    pub fn track(&self, name: &str) -> Option<&TrackActivity> {
        self.tracks.iter().find(|track| track.track == name)
    }

    pub fn expect_track_events(&self, name: &str, minimum: u64) -> Result<(), String> {
        let track = self
            .track(name)
            .ok_or_else(|| format!("musical diagnostics have no track named '{name}'"))?;
        if track.events < minimum {
            return Err(format!(
                "track '{}' from '{}' produced {} events; expected at least {minimum}",
                track.track, track.source, track.events
            ));
        }
        Ok(())
    }

    pub fn expect_output(
        &self,
        module: &str,
        output: usize,
        minimum_rms: f64,
    ) -> Result<(), String> {
        let activity = self
            .module(module)
            .ok_or_else(|| format!("musical diagnostics have no module '{module}'"))?;
        let signal = activity
            .outputs
            .get(output)
            .ok_or_else(|| format!("module '{module}' has no diagnostic output {output}"))?;
        if signal.non_finite_samples > 0 || signal.rms() < minimum_rms {
            return Err(format!(
                "module '{module}' output {output} RMS {} is below expected {minimum_rms} (peak {}, {} active samples, {} non-finite samples)",
                signal.rms(),
                signal.peak,
                signal.active_samples,
                signal.non_finite_samples
            ));
        }
        Ok(())
    }

    pub fn expect_input(&self, module: &str, input: usize, minimum_rms: f64) -> Result<(), String> {
        let activity = self
            .module(module)
            .ok_or_else(|| format!("musical diagnostics have no module '{module}'"))?;
        let signal = activity
            .inputs
            .get(input)
            .ok_or_else(|| format!("module '{module}' has no diagnostic input {input}"))?;
        if signal.non_finite_samples > 0 || signal.rms() < minimum_rms {
            return Err(format!(
                "module '{module}' input {input} RMS {} is below expected {minimum_rms} (peak {}, {} active samples, {} non-finite samples)",
                signal.rms(),
                signal.peak,
                signal.active_samples,
                signal.non_finite_samples
            ));
        }
        Ok(())
    }
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
                Widget::PerformanceControls | Widget::ControlFocus => {}
                Widget::PerformanceStatus
                | Widget::ArrangementStatus
                | Widget::PatternGrid
                | Widget::TransportStatus => {
                    if !has_performance {
                        return Err(format!(
                            "screen '{}' uses a sequencer widget without a performance sequencer",
                            screen.id
                        ));
                    }
                }
                Widget::PerformanceSurface(surface) => {
                    if !has_performance {
                        return Err(format!(
                            "screen '{}' uses a performance surface without a performance sequencer",
                            screen.id
                        ));
                    }
                    crate::performance_ui::validate_surface(surface)?;
                    text(&surface.title, "performance surface title")?;
                    for track in &surface.tracks {
                        text(&track.label, "performance track label")?;
                        for id in std::iter::once(&track.volume_control).chain(&track.control_ids) {
                            if !controls.contains_key(id) {
                                return Err(format!(
                                    "screen '{}' performance surface references unknown control '{id}'",
                                    screen.id
                                ));
                            }
                        }
                    }
                }
                Widget::EventActivity { label, .. } => {
                    text(label, "event activity label")?;
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

    pub fn process_stereo_diagnostic(
        &mut self,
        left: &mut [f64],
        right: &mut [f64],
        diagnostics: &mut RenderDiagnostics,
    ) {
        assert_eq!(left.len(), right.len());
        self.prepare_buffer();
        if diagnostics.modules.is_empty() {
            diagnostics.modules = self
                .ops
                .iter()
                .zip(&self.op_ids)
                .map(|(op, id)| ModuleActivity {
                    id: id.clone(),
                    inputs: vec![SignalStats::default(); op.inputs.len()],
                    outputs: vec![SignalStats::default(); op.outputs.len()],
                })
                .collect();
            for control in &self.sequencer_controls {
                diagnostics
                    .tracks
                    .extend(
                        control
                            .track_names()
                            .into_iter()
                            .map(|track| TrackActivity {
                                source: control.name.clone(),
                                track,
                                ..TrackActivity::default()
                            }),
                    );
            }
        }
        for (left, right) in left.iter_mut().zip(right) {
            self.process_one_diagnostic(diagnostics);
            *left = self.slots[self.output_slots[0]];
            *right = self.slots[self.output_slots[1]];
        }
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
        for automation in &self.automations {
            let value = automation.value(automation.sequencer.position_steps());
            for &index in &automation.targets {
                if self.published[index].load(Ordering::Acquire) == NO_OVERRIDE {
                    self.params[index] = value;
                }
            }
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
        for (index, observed) in self.observed.iter().enumerate() {
            observed.store(self.params[index].to_bits(), Ordering::Release);
        }
        // Tempo is control state observed by the sequencer before its first
        // frame. Publish it at the same buffer boundary as every other
        // parameter so block and scalar executors share exact timing.
        for op in &self.ops {
            if let OpProcessor::SequencerTempo(control) = &op.processor {
                control
                    .bpm
                    .store(self.params[op.params[0]].to_bits(), Ordering::Release);
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
                    player.process(inputs[0], params, self.sample_rate, outputs)
                }
                OpProcessor::Delay(delay) => {
                    delay.process(inputs, params, self.sample_rate, outputs)
                }
                OpProcessor::StereoDelay(delay) => {
                    delay.process(inputs, params, self.sample_rate, outputs)
                }
                OpProcessor::Plaits(plaits) => plaits.process(inputs, params, outputs),
                OpProcessor::SequencerTrack { sequencer, track } => {
                    let runtime = &self.sequencers[*sequencer];
                    outputs[0] = runtime.trigger(*track);
                    outputs[1] = runtime.pitch(*track);
                    outputs[2] = runtime.gate(*track);
                    outputs[3] = runtime.velocity(*track);
                    outputs[4] = runtime.enabled(*track);
                }
                OpProcessor::SequencerTempo(control) => {
                    control.bpm.store(params[0].to_bits(), Ordering::Release);
                    outputs[0] = params[0];
                }
            }
        }
    }

    fn process_one_diagnostic(&mut self, diagnostics: &mut RenderDiagnostics) {
        let mut track_offset = 0;
        for sequencer in &mut self.sequencers {
            sequencer.begin_frame(self.sample_rate);
            for track in 0..sequencer.track_count() {
                let activity = &mut diagnostics.tracks[track_offset + track];
                if sequencer.trigger(track) > 0.0 {
                    activity.events += 1;
                }
                if sequencer.enabled(track) > 0.0 && sequencer.gate(track) > 0.0 {
                    activity.audible_frames += 1;
                }
            }
            track_offset += sequencer.track_count();
        }
        for (op_index, op) in self.ops.iter_mut().enumerate() {
            for (slot, input) in op.inputs.iter().enumerate() {
                self.input_scratch[slot] = if input.sources.is_empty() {
                    input.unconnected
                } else {
                    input
                        .sources
                        .iter()
                        .fold(0.0, |sum, source| sum + self.slots[*source])
                };
                diagnostics.modules[op_index].inputs[slot].observe(self.input_scratch[slot]);
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
                    player.process(inputs[0], params, self.sample_rate, outputs)
                }
                OpProcessor::Delay(delay) => {
                    delay.process(inputs, params, self.sample_rate, outputs)
                }
                OpProcessor::StereoDelay(delay) => {
                    delay.process(inputs, params, self.sample_rate, outputs)
                }
                OpProcessor::Plaits(plaits) => plaits.process(inputs, params, outputs),
                OpProcessor::SequencerTrack { sequencer, track } => {
                    let runtime = &self.sequencers[*sequencer];
                    outputs[0] = runtime.trigger(*track);
                    outputs[1] = runtime.pitch(*track);
                    outputs[2] = runtime.gate(*track);
                    outputs[3] = runtime.velocity(*track);
                    outputs[4] = runtime.enabled(*track);
                }
                OpProcessor::SequencerTempo(control) => {
                    control.bpm.store(params[0].to_bits(), Ordering::Release);
                    outputs[0] = params[0];
                }
            }
            for (signal, value) in diagnostics.modules[op_index]
                .outputs
                .iter_mut()
                .zip(outputs)
            {
                signal.observe(*value);
            }
        }
        diagnostics.frames += 1;
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
                if self
                    .automations
                    .iter()
                    .any(|lane| lane.targets.contains(&i))
                {
                    f64::from_bits(self.observed[i].load(Ordering::Acquire))
                } else if self.note_pairs.iter().any(|pair| {
                    pair.pitch_index == i || pair.gate_index == i || pair.velocity_index == Some(i)
                }) {
                    self.params[i]
                } else {
                    self.authored[i]
                }
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
            observed: Arc::clone(&self.observed[i]),
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
            hardware_pickup: Arc::new(std::sync::atomic::AtomicI8::new(2)),
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

/// Block-oriented executor for file rendering. The live callback continues to
/// use `Machine`'s scalar path; this executor is allowed to own worker threads
/// and block-sized scratch allocated during preparation.
pub struct OfflineRenderer {
    machine: Machine,
    block_size: usize,
    workers: usize,
    pool: Option<rayon::ThreadPool>,
    slot_buffers: Vec<Vec<f64>>,
    op_outputs: Vec<Vec<f64>>,
    op_inputs: Vec<Vec<f64>>,
    op_params: Vec<Vec<f64>>,
    sequencer_ops: Vec<usize>,
    profile: OfflineRenderProfile,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct OfflineRenderProfile {
    pub blocks: u64,
    pub frames: u64,
    pub sequencing: Duration,
    pub graph_execution: Duration,
}

impl OfflineRenderer {
    pub fn new(machine: Machine, block_size: usize, workers: usize) -> Result<Self, String> {
        if block_size == 0 || workers == 0 {
            return Err("offline renderer requires positive block size and worker count".into());
        }
        let pool = (workers > 1)
            .then(|| {
                rayon::ThreadPoolBuilder::new()
                    .num_threads(workers)
                    .thread_name(|index| format!("meldritch-render-{index}"))
                    .build()
                    .map_err(|error| error.to_string())
            })
            .transpose()?;
        let slot_buffers = (0..machine.slots.len())
            .map(|_| vec![0.0; block_size])
            .collect();
        let op_outputs = machine
            .ops
            .iter()
            .map(|op| vec![0.0; block_size * op.outputs.len()])
            .collect();
        let op_inputs = machine
            .ops
            .iter()
            .map(|op| vec![0.0; op.inputs.len()])
            .collect();
        let op_params = machine
            .ops
            .iter()
            .map(|op| vec![0.0; op.params.len()])
            .collect();
        let sequencer_ops = machine
            .ops
            .iter()
            .enumerate()
            .filter_map(|(index, op)| {
                matches!(
                    op.processor,
                    OpProcessor::SequencerTrack { .. } | OpProcessor::SequencerTempo(_)
                )
                .then_some(index)
            })
            .collect();
        Ok(Self {
            machine,
            block_size,
            workers,
            pool,
            slot_buffers,
            op_outputs,
            op_inputs,
            op_params,
            sequencer_ops,
            profile: OfflineRenderProfile::default(),
        })
    }

    pub fn workers(&self) -> usize {
        self.workers
    }

    pub fn maximum_parallel_width(&self) -> usize {
        self.machine
            .level_ranges
            .iter()
            .map(std::ops::Range::len)
            .max()
            .unwrap_or(1)
    }

    pub fn module_count(&self) -> usize {
        self.machine.ops.len()
    }

    pub fn dependency_levels(&self) -> usize {
        self.machine.level_ranges.len()
    }

    pub fn machine(&self) -> &Machine {
        &self.machine
    }

    pub fn profile(&self) -> OfflineRenderProfile {
        self.profile
    }

    pub fn process_stereo(&mut self, left: &mut [f64], right: &mut [f64]) {
        assert_eq!(left.len(), right.len());
        assert!(left.len() <= self.block_size);
        let frames = left.len();
        self.machine.prepare_buffer();
        for (op, values) in self.machine.ops.iter().zip(&mut self.op_params) {
            for (target, index) in values.iter_mut().zip(&op.params) {
                *target = self.machine.params[*index];
            }
        }

        // Musical scheduling remains single-threaded and sample ordered. Its
        // prepared signal buffers become ordinary dependencies for DSP levels.
        let sequencing_started = Instant::now();
        for frame in 0..frames {
            for sequencer in &mut self.machine.sequencers {
                sequencer.begin_frame(self.machine.sample_rate);
            }
            for &op_index in &self.sequencer_ops {
                let op = &mut self.machine.ops[op_index];
                let outputs = &mut self.op_outputs[op_index];
                match &mut op.processor {
                    OpProcessor::SequencerTrack { sequencer, track } => {
                        let runtime = &self.machine.sequencers[*sequencer];
                        let base = frame * 5;
                        outputs[base] = runtime.trigger(*track);
                        outputs[base + 1] = runtime.pitch(*track);
                        outputs[base + 2] = runtime.gate(*track);
                        outputs[base + 3] = runtime.velocity(*track);
                        outputs[base + 4] = runtime.enabled(*track);
                    }
                    OpProcessor::SequencerTempo(control) => {
                        let bpm = self.op_params[op_index][0];
                        control.bpm.store(bpm.to_bits(), Ordering::Release);
                        outputs[frame] = bpm;
                    }
                    _ => unreachable!("prepared sequencer op list contains only sequencer ops"),
                }
            }
        }
        self.profile.sequencing += sequencing_started.elapsed();

        let graph_started = Instant::now();
        for range in self.machine.level_ranges.clone() {
            let slots = &self.slot_buffers;
            let params = &self.op_params[range.clone()];
            let sample_rate = self.machine.sample_rate;
            let op_slice = &mut self.machine.ops[range.clone()];
            let output_slice = &mut self.op_outputs[range.clone()];
            let input_slice = &mut self.op_inputs[range.clone()];
            let worthwhile = self.pool.is_some()
                && range.len() > 1
                && frames.saturating_mul(range.len()) >= 2_048;
            if worthwhile {
                self.pool.as_ref().unwrap().install(|| {
                    op_slice
                        .par_iter_mut()
                        .zip(output_slice.par_iter_mut())
                        .zip(input_slice.par_iter_mut())
                        .zip(params.par_iter())
                        .for_each(|(((op, outputs), inputs), params)| {
                            process_offline_op(
                                op,
                                inputs,
                                params,
                                outputs,
                                slots,
                                sample_rate,
                                frames,
                            );
                        });
                });
            } else {
                for (((op, outputs), inputs), params) in op_slice
                    .iter_mut()
                    .zip(output_slice.iter_mut())
                    .zip(input_slice.iter_mut())
                    .zip(params)
                {
                    process_offline_op(op, inputs, params, outputs, slots, sample_rate, frames);
                }
            }
            for (local_index, op) in self.machine.ops[range.clone()].iter().enumerate() {
                let outputs = &self.op_outputs[range.start + local_index];
                let output_count = op.outputs.len();
                for (output, slot) in op.outputs.clone().enumerate() {
                    for frame in 0..frames {
                        self.slot_buffers[slot][frame] = outputs[frame * output_count + output];
                    }
                }
            }
        }
        left.copy_from_slice(&self.slot_buffers[self.machine.output_slots[0]][..frames]);
        right.copy_from_slice(&self.slot_buffers[self.machine.output_slots[1]][..frames]);
        self.profile.graph_execution += graph_started.elapsed();
        self.profile.blocks += 1;
        self.profile.frames += frames as u64;
    }
}

fn process_offline_op(
    op: &mut Op,
    inputs: &mut [f64],
    params: &[f64],
    outputs: &mut [f64],
    slots: &[Vec<f64>],
    sample_rate: f64,
    frames: usize,
) {
    if matches!(
        op.processor,
        OpProcessor::SequencerTrack { .. } | OpProcessor::SequencerTempo(_)
    ) {
        return;
    }
    let output_count = op.outputs.len();
    for frame in 0..frames {
        for (target, input) in inputs.iter_mut().zip(&op.inputs) {
            *target = if input.sources.is_empty() {
                input.unconnected
            } else {
                input
                    .sources
                    .iter()
                    .fold(0.0, |sum, source| sum + slots[*source][frame])
            };
        }
        let frame_outputs =
            &mut outputs[frame * output_count..frame.saturating_add(1) * output_count];
        match &mut op.processor {
            OpProcessor::Contract { process, state } => {
                process(state, inputs, params, sample_rate, frame_outputs)
            }
            OpProcessor::Sample(player) => {
                player.process(inputs[0], params, sample_rate, frame_outputs)
            }
            OpProcessor::Delay(delay) => delay.process(inputs, params, sample_rate, frame_outputs),
            OpProcessor::StereoDelay(delay) => {
                delay.process(inputs, params, sample_rate, frame_outputs)
            }
            OpProcessor::Plaits(plaits) => plaits.process(inputs, params, frame_outputs),
            OpProcessor::SequencerTrack { .. } | OpProcessor::SequencerTempo(_) => unreachable!(),
        }
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
    let mut levels = vec![0usize; graph.modules.len()];
    while let Some((_, i)) = ready.pop_first() {
        schedule.push(i);
        for &next in &dependents[i] {
            levels[next] = levels[next].max(levels[i] + 1);
            indegree[next] -= 1;
            if indegree[next] == 0 {
                ready.insert((graph.modules[next].id.clone(), next));
            }
        }
    }
    if schedule.len() != graph.modules.len() {
        return Err("graph contains an unsupported cycle".into());
    }
    let public_order: Vec<String> = schedule
        .iter()
        .map(|&index| graph.modules[index].id.clone())
        .collect();
    schedule.sort_by(|left, right| {
        levels[*left]
            .cmp(&levels[*right])
            .then_with(|| graph.modules[*left].id.cmp(&graph.modules[*right].id))
    });
    let mut level_ranges = Vec::new();
    let mut start = 0;
    while start < schedule.len() {
        let level = levels[schedule[start]];
        let mut end = start + 1;
        while end < schedule.len() && levels[schedule[end]] == level {
            end += 1;
        }
        level_ranges.push(start..end);
        start = end;
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
    if let Some(focus) = graph
        .performance
        .as_ref()
        .and_then(|performance| performance.control_focus.as_ref())
    {
        if focus.controls.is_empty() {
            return Err("performance focus requires at least one control".into());
        }
        let mut focused = BTreeSet::new();
        for id in &focus.controls {
            if !controls.contains_key(id) {
                return Err(format!("performance focus targets unknown control '{id}'"));
            }
            if !focused.insert(id) {
                return Err(format!("performance focus repeats control '{id}'"));
            }
        }
        for mapping in &focus.midi_ccs {
            if !(1..=16).contains(&mapping.channel) || mapping.number > 127 {
                return Err(
                    "performance focus MIDI mapping requires channel 1..16 and CC 0..127".into(),
                );
            }
            if !mapped_ccs.insert((mapping.channel, mapping.number)) {
                return Err(format!(
                    "duplicate MIDI CC mapping for channel {} CC {}",
                    mapping.channel, mapping.number
                ));
            }
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
    if let Some(apc) = graph
        .performance
        .as_ref()
        .and_then(|value| value.apc.as_ref())
    {
        if apc.pages.iter().any(String::is_empty) {
            return Err("APC page names must be nonempty".into());
        }
        for role in &apc.performance_rows {
            if !matches!(
                role.as_str(),
                "" | "pattern" | "scene" | "clip" | "mute" | "solo" | "track" | "restart"
            ) {
                return Err(format!("unknown APC performance row role '{role}'"));
            }
        }
        for id in &apc.fader_controls {
            if !controls.contains_key(id) {
                return Err(format!("APC fader targets unknown control '{id}'"));
            }
        }
        for bank in &apc.fader_banks {
            for id in bank {
                if !controls.contains_key(id) {
                    return Err(format!("APC fader bank targets unknown control '{id}'"));
                }
            }
        }
        if !controls.contains_key(&apc.master_control) {
            return Err(format!(
                "APC master fader targets unknown control '{}'",
                apc.master_control
            ));
        }
        if let Some(roll) = &apc.piano_roll {
            let tracks = graph
                .performance
                .as_ref()
                .expect("validated performance")
                .sequencer
                .track_count();
            if roll
                .drum_tracks
                .iter()
                .chain(std::iter::once(&roll.melodic_track))
                .any(|track| *track >= tracks)
            {
                return Err("APC piano roll contains an invalid track index".into());
            }
            if roll.scale.windows(2).any(|pair| pair[0] >= pair[1]) || roll.scale[4] > 24 {
                return Err(
                    "APC piano roll scale must contain five ascending offsets up to 24 semitones"
                        .into(),
                );
            }
        }
    }
    let automations = graph
        .automations
        .iter()
        .map(|automation| {
            let control = controls.get(&automation.control_id).ok_or_else(|| {
                format!(
                    "automation targets unknown control '{}'",
                    automation.control_id
                )
            })?;
            if automation.points.is_empty()
                || automation
                    .points
                    .windows(2)
                    .any(|pair| pair[0].0 >= pair[1].0)
                || automation.points.iter().any(|(_, value)| {
                    !value.is_finite() || *value < control.min || *value > control.max
                })
            {
                return Err(format!(
                    "automation '{}' requires ordered points inside {}..{}",
                    automation.control_id, control.min, control.max
                ));
            }
            let targets = std::iter::once(&control.target)
                .chain(&control.additional_targets)
                .map(|target| param_ids[target])
                .collect();
            Ok(AutomationRuntime {
                sequencer: Arc::clone(&automation.sequencer),
                targets,
                interpolation: automation.interpolation,
                points: automation.points.clone(),
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    for surface in &graph.control_surfaces {
        if surface.id.is_empty() || surface.device != "stream_deck_xl" {
            return Err(format!(
                "control surface '{}' requires device='stream_deck_xl'",
                surface.id
            ));
        }
        let pages = surface
            .pages
            .iter()
            .map(|page| page.id.as_str())
            .collect::<BTreeSet<_>>();
        if pages.len() != surface.pages.len()
            || pages.contains("")
            || !pages.contains(surface.initial_page.as_str())
        {
            return Err(format!(
                "control surface '{}' requires unique page IDs and a valid initial_page",
                surface.id
            ));
        }
        let mut button_ids = BTreeSet::new();
        for page in &surface.pages {
            let mut indexes = BTreeSet::new();
            for button in &page.buttons {
                if button.id.is_empty()
                    || !button_ids.insert(button.id.as_str())
                    || button.index >= 32
                    || !indexes.insert(button.index)
                {
                    return Err(format!(
                        "control surface '{}' requires stable unique button IDs and indexes 0..31 per page",
                        surface.id
                    ));
                }
                if button
                    .icon
                    .as_deref()
                    .is_some_and(|icon| !matches!(icon, "circle" | "square" | "triangle" | "bars"))
                {
                    return Err(format!(
                        "surface button '{}' has an unknown generated icon",
                        button.id
                    ));
                }
                for action in [&button.press, &button.release].into_iter().flatten() {
                    match action {
                        crate::model::SurfaceAction::Command(_) if graph.performance.is_none() => {
                            return Err(format!(
                                "surface button '{}' uses a command without a performance model",
                                button.id
                            ));
                        }
                        crate::model::SurfaceAction::Control { id, value } => {
                            let binding = controls.get(id).ok_or_else(|| {
                                format!(
                                    "surface button '{}' targets unknown control '{id}'",
                                    button.id
                                )
                            })?;
                            if !value.is_finite() || *value < binding.min || *value > binding.max {
                                return Err(format!(
                                    "surface button '{}' value {value} is outside control '{id}' range {}..{}",
                                    button.id, binding.min, binding.max
                                ));
                            }
                        }
                        crate::model::SurfaceAction::Page(id) if !pages.contains(id.as_str()) => {
                            return Err(format!(
                                "surface button '{}' targets unknown page '{id}'",
                                button.id
                            ));
                        }
                        _ => {}
                    }
                }
            }
        }
    }
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
        let processor = if matches!(module.runtime, ModuleRuntime::Contract) {
            match module.kind.as_str() {
                "delay" => OpProcessor::Delay(DelayRuntime::new(sample_rate)?),
                "stereo_delay" => OpProcessor::StereoDelay(StereoDelayRuntime::new(sample_rate)?),
                _ => OpProcessor::Contract {
                    process: c.process,
                    state: vec![0.0; c.state_len],
                },
            }
        } else {
            match &module.runtime {
                ModuleRuntime::Contract => unreachable!("contract runtime handled above"),
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
        observed: params
            .iter()
            .map(|value| Arc::new(AtomicU64::new(value.to_bits())))
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
        op_ids: schedule
            .iter()
            .map(|&i| graph.modules[i].id.clone())
            .collect(),
        order: public_order,
        authored_normalizations: graph.normalizations.clone(),
        effective_connections,
        sample_rate,
        sequencers,
        sequencer_controls,
        level_ranges,
        automations,
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
        let params = [0.5, 0.0, 1.0, 1.0];
        player.process(1.0, &params, 4.0, &mut out);
        assert_eq!(out, [0.5, -0.5]);
        player.process(0.0, &params, 4.0, &mut out);
        assert_eq!(out, [0.375, -0.375]);
        for _ in 0..3 {
            player.process(0.0, &params, 4.0, &mut out);
        }
        player.process(0.0, &params, 4.0, &mut out);
        assert_eq!(out, [0.0, 0.0]);
        assert!(out.iter().all(|v| v.is_finite()));
    }
    #[test]
    fn ordinary_retriggers_overlap_without_stealing() {
        let mut player =
            SamplePlayerRuntime::new(sample(48_000, &[1.0; 64], &[1.0; 64]), 48_000.0).unwrap();
        let mut out = [0.0; 2];
        for overlap in 1..=20 {
            let params = [1.0, 0.0, 1.0, 1.0];
            player.process(1.0, &params, 48_000.0, &mut out);
            assert_eq!(out, [f64::from(overlap), f64::from(overlap)]);
            player.process(0.0, &params, 48_000.0, &mut out);
        }
        assert_eq!(player.voices.len(), 20);
        assert_eq!(player.max_voices, SamplePlayerRuntime::MAX_VOICES);
    }

    #[test]
    fn player_latches_forward_and_reverse_slices_at_each_trigger() {
        let sample = sample(4, &[0.0, 1.0, 2.0, 3.0], &[0.0, 1.0, 2.0, 3.0]);
        let mut player = SamplePlayerRuntime::new(sample, 4.0).unwrap();
        let mut out = [0.0; 2];
        let forward = [1.0, 0.25, 0.75, 1.0];
        player.process(1.0, &forward, 4.0, &mut out);
        assert_eq!(out, [1.0, 1.0]);
        player.process(0.0, &forward, 4.0, &mut out);
        assert_eq!(out, [2.0, 2.0]);
        player.process(0.0, &forward, 4.0, &mut out);
        assert_eq!(out, [0.0, 0.0]);
        let reverse = [1.0, 0.25, 0.75, -1.0];
        player.process(1.0, &reverse, 4.0, &mut out);
        assert_eq!(out, [2.0, 2.0]);
        player.process(0.0, &reverse, 4.0, &mut out);
        assert_eq!(out, [1.0, 1.0]);
    }

    #[test]
    fn player_bounds_pathological_overlap_by_stealing_the_oldest_voice() {
        let mut player =
            SamplePlayerRuntime::new(sample(48_000, &[1.0; 1024], &[1.0; 1024]), 48_000.0).unwrap();
        let mut out = [0.0; 2];
        let params = [1.0, 0.0, 1.0, 0.01];
        for _ in 0..SamplePlayerRuntime::MAX_VOICES + 20 {
            player.process(1.0, &params, 48_000.0, &mut out);
            player.process(0.0, &params, 48_000.0, &mut out);
        }
        assert_eq!(player.voices.len(), SamplePlayerRuntime::MAX_VOICES);
        assert!(out.iter().all(|value| value.is_finite()));
    }
}
