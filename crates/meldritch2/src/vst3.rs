//! Native Linux VST3 discovery and in-process instrument hosting.
//!
//! This module is deliberately independent of the Lua and graph layers.  A
//! future process-isolated adapter can implement the same prepared-instance
//! contract without changing authored compositions.

use std::{
    collections::BTreeMap,
    env,
    path::{Path, PathBuf},
};

use vst3_host::{
    Plugin, PluginInfo, ProcessMode, Vst3Host,
    audio::{AudioBuffers, BusAudioBuffers, BusDirection, MediaType},
    midi::{MidiChannel, MidiEvent},
};

pub const MAX_BLOCK_SIZE: usize = 1024;
pub const NOTE_LANES: usize = 8;
pub const EXTRA_NOTE_INPUTS: usize = (NOTE_LANES - 1) * 3;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Descriptor {
    pub path: PathBuf,
    pub class_id: String,
    pub name: String,
    pub vendor: String,
    pub category: String,
    pub version: String,
    pub instrument: bool,
}

impl From<PluginInfo> for Descriptor {
    fn from(info: PluginInfo) -> Self {
        let instrument = info
            .category
            .split(['|', ';'])
            .any(|part| part.trim().eq_ignore_ascii_case("instrument"));
        Self {
            path: info.path,
            class_id: info.uid,
            name: info.name,
            vendor: info.vendor,
            category: info.category,
            version: info.version,
            instrument,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Parameter {
    pub id: u32,
    pub name: String,
    pub normalized: f64,
    pub default: f64,
    pub unit: String,
    pub step_count: i32,
    pub automatable: bool,
    pub read_only: bool,
}

impl From<vst3_host::parameters::Parameter> for Parameter {
    fn from(parameter: vst3_host::parameters::Parameter) -> Self {
        Self {
            id: parameter.id,
            name: parameter.name,
            normalized: parameter.value,
            default: parameter.default,
            unit: parameter.unit,
            step_count: parameter.step_count,
            automatable: parameter.can_automate,
            read_only: parameter.is_read_only,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Spec {
    pub descriptor: Descriptor,
    pub parameters: Vec<Parameter>,
    pub initial_values: BTreeMap<u32, f64>,
    pub state: Option<Vec<u8>>,
    /// Activate and retain every output bus. Multi-output instruments such as
    /// Geonkick require every advertised bus to be present during processing.
    pub activate_all_outputs: bool,
}

impl Spec {
    pub fn parameter_name(id: u32) -> String {
        format!("parameter_{id}")
    }

    pub fn parameter_id(&self, name: &str) -> Option<u32> {
        let id = name.strip_prefix("parameter_")?.parse().ok()?;
        self.parameters
            .iter()
            .any(|parameter| parameter.id == id)
            .then_some(id)
    }

    pub fn initial_value(&self, parameter: &Parameter) -> f64 {
        self.initial_values
            .get(&parameter.id)
            .copied()
            .unwrap_or(parameter.normalized)
    }
}

fn configured_paths() -> Vec<PathBuf> {
    env::var_os("MELDRITCH_VST3_PATH")
        .map(|value| env::split_paths(&value).collect())
        .unwrap_or_default()
}

fn host_builder(paths: &[PathBuf]) -> vst3_host::Vst3HostBuilder {
    let mut builder = Vst3Host::builder();
    if paths.is_empty() {
        builder = builder.scan_default_paths();
    } else {
        for path in paths {
            builder = builder.add_scan_path(path);
        }
    }
    builder.with_process_isolation(false)
}

pub fn discover(paths: &[PathBuf]) -> Result<Vec<Descriptor>, String> {
    let selected = if paths.is_empty() {
        configured_paths()
    } else {
        paths.to_vec()
    };
    let mut host = host_builder(&selected)
        .build()
        .map_err(|error| error.to_string())?;
    let mut plugins = host
        .discover_plugins()
        .map_err(|error| format!("VST3 discovery failed: {error}"))?
        .into_iter()
        .map(Descriptor::from)
        .collect::<Vec<_>>();
    plugins.sort_by(|left, right| {
        left.name
            .cmp(&right.name)
            .then(left.vendor.cmp(&right.vendor))
            .then(left.class_id.cmp(&right.class_id))
    });
    Ok(plugins)
}

pub fn resolve(selector: &str, paths: &[PathBuf]) -> Result<Descriptor, String> {
    let plugins = discover(paths)?;
    let by_id = plugins
        .iter()
        .filter(|plugin| plugin.class_id.eq_ignore_ascii_case(selector))
        .cloned()
        .collect::<Vec<_>>();
    let matches = if by_id.is_empty() {
        plugins
            .into_iter()
            .filter(|plugin| plugin.name.eq_ignore_ascii_case(selector))
            .collect::<Vec<_>>()
    } else {
        by_id
    };
    match matches.as_slice() {
        [plugin] => Ok(plugin.clone()),
        [] => Err(format!("VST3 plugin '{selector}' was not found")),
        _ => Err(format!(
            "VST3 plugin name '{selector}' is ambiguous; use a stable class ID"
        )),
    }
}

pub fn inspect(descriptor: Descriptor) -> Result<Spec, String> {
    let mut host = Vst3Host::builder()
        .sample_rate(48_000.0)
        .block_size(MAX_BLOCK_SIZE)
        .input_channels(0)
        .output_channels(2)
        .with_process_isolation(false)
        .build()
        .map_err(|error| error.to_string())?;
    let plugin = host
        .load_plugin_class(&descriptor.path, &descriptor.class_id)
        .map_err(|error| {
            format!(
                "failed to inspect VST3 plugin '{}': {error}",
                descriptor.name
            )
        })?;
    let parameters = plugin
        .get_parameters()
        .map_err(|error| format!("failed to enumerate '{}': {error}", descriptor.name))?
        .into_iter()
        .map(Parameter::from)
        .collect();
    Ok(Spec {
        descriptor,
        parameters,
        initial_values: BTreeMap::new(),
        state: None,
        activate_all_outputs: false,
    })
}

pub fn load(selector: &str, paths: &[PathBuf]) -> Result<Spec, String> {
    inspect(resolve(selector, paths)?)
}

pub struct Instance {
    plugin: Plugin,
    buffers: ProcessingBuffers,
    parameters: Vec<u32>,
    previous_parameters: Vec<f64>,
    restored_baseline: Option<Vec<f64>>,
    active_notes: [Option<u8>; NOTE_LANES],
    failed: bool,
    offline_fallback: bool,
    previous_tempo: f64,
}

enum ProcessingBuffers {
    Flat(AudioBuffers),
    Buses(BusAudioBuffers),
}

impl std::fmt::Debug for Instance {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Vst3Instance")
            .field("plugin", &self.plugin.info().name)
            .field("class_id", &self.plugin.info().uid)
            .field("failed", &self.failed)
            .field("offline_fallback", &self.offline_fallback)
            .finish()
    }
}

impl Instance {
    pub fn new(spec: &Spec, sample_rate: f64) -> Result<Self, String> {
        let mut host = Vst3Host::builder()
            .sample_rate(sample_rate)
            .block_size(MAX_BLOCK_SIZE)
            .input_channels(0)
            .output_channels(2)
            .with_process_isolation(false)
            .build()
            .map_err(|error| error.to_string())?;
        let mut plugin = host
            .load_plugin_class(&spec.descriptor.path, &spec.descriptor.class_id)
            .map_err(|error| {
                format!(
                    "failed to load VST3 plugin '{}' ({}): {error}",
                    spec.descriptor.name, spec.descriptor.class_id
                )
            })?;
        let mut parameters = Vec::with_capacity(spec.parameters.len());
        let mut previous_parameters = Vec::with_capacity(spec.parameters.len());
        for parameter in &spec.parameters {
            let value = spec.initial_value(parameter);
            if !parameter.read_only {
                plugin.set_parameter(parameter.id, value).map_err(|error| {
                    format!(
                        "failed to initialize VST3 parameter {} on '{}': {error}",
                        parameter.id, spec.descriptor.name
                    )
                })?;
            }
            parameters.push(parameter.id);
            previous_parameters.push(value);
        }
        let restored_baseline = spec.state.as_ref().map(|_| previous_parameters.clone());
        if let Some(state) = &spec.state {
            plugin.load_state(state).map_err(|error| {
                format!(
                    "failed to restore VST3 state for '{}': {error}",
                    spec.descriptor.name
                )
            })?;
        }
        if spec.state.is_some() {
            for (previous, id) in previous_parameters.iter_mut().zip(&parameters) {
                *previous = plugin.get_parameter(*id).map_err(|error| {
                    format!(
                        "failed to read restored VST3 parameter {id} on '{}': {error}",
                        spec.descriptor.name
                    )
                })?;
            }
        }
        let buffers = if spec.activate_all_outputs {
            let output_count = plugin
                .audio_bus_layout()
                .map_err(|error| format!("failed to inspect VST3 audio buses: {error}"))?
                .outputs
                .len();
            for bus in 0..output_count {
                plugin
                    .set_bus_active(MediaType::Audio, BusDirection::Output, bus as i32, true)
                    .map_err(|error| {
                        format!("failed to activate VST3 output bus {bus}: {error}")
                    })?;
            }
            ProcessingBuffers::Buses(
                plugin
                    .create_bus_audio_buffers(MAX_BLOCK_SIZE)
                    .map_err(|error| format!("failed to allocate VST3 bus buffers: {error}"))?,
            )
        } else {
            ProcessingBuffers::Flat(AudioBuffers::new(0, 2, MAX_BLOCK_SIZE, sample_rate))
        };
        plugin.start_processing().map_err(|error| {
            format!(
                "failed to start VST3 plugin '{}': {error}",
                spec.descriptor.name
            )
        })?;
        Ok(Self {
            plugin,
            buffers,
            parameters,
            previous_parameters,
            restored_baseline,
            active_notes: [None; NOTE_LANES],
            failed: false,
            offline_fallback: false,
            previous_tempo: 120.0,
        })
    }

    pub fn set_offline(&mut self) {
        if self.failed {
            return;
        }
        if self.plugin.stop_processing().is_err() {
            self.failed = true;
            return;
        }
        if self.plugin.set_process_mode(ProcessMode::Offline).is_err() {
            self.offline_fallback = true;
            let _ = self.plugin.set_process_mode(ProcessMode::Realtime);
        }
        if self.plugin.start_processing().is_err() {
            self.failed = true;
        }
    }

    pub fn offline_fallback(&self) -> bool {
        self.offline_fallback
    }

    pub fn failed(&self) -> bool {
        self.failed
    }

    pub fn save_state(&self) -> Result<Vec<u8>, String> {
        self.plugin.save_state().map_err(|error| error.to_string())
    }

    pub fn process(
        &mut self,
        note_inputs: [&[f64]; 5],
        parameter_values: &[f64],
        output: &mut [f64],
    ) {
        let [pitch, gate, velocity, tempo, extra_note_inputs] = note_inputs;
        let frames = pitch.len();
        debug_assert_eq!(gate.len(), frames);
        debug_assert_eq!(velocity.len(), frames);
        debug_assert_eq!(tempo.len(), frames);
        debug_assert_eq!(output.len(), frames * 2);
        debug_assert_eq!(extra_note_inputs.len(), frames * EXTRA_NOTE_INPUTS);
        if self.failed || frames == 0 || frames > MAX_BLOCK_SIZE {
            output.fill(0.0);
            self.failed |= frames > MAX_BLOCK_SIZE;
            return;
        }

        let tempo = tempo[0];
        if tempo.is_finite() && tempo > 0.0 && tempo.to_bits() != self.previous_tempo.to_bits() {
            if self.plugin.set_tempo(tempo).is_err() {
                self.failed = true;
                output.fill(0.0);
                return;
            }
            self.previous_tempo = tempo;
        }

        for (index, ((&id, previous), &value)) in self
            .parameters
            .iter()
            .zip(&mut self.previous_parameters)
            .zip(parameter_values)
            .enumerate()
        {
            if self
                .restored_baseline
                .as_ref()
                .is_some_and(|baseline| value.to_bits() == baseline[index].to_bits())
            {
                continue;
            }
            if value.to_bits() != previous.to_bits() {
                if self.plugin.set_parameter_at(id, value, 0).is_err() {
                    self.failed = true;
                    output.fill(0.0);
                    return;
                }
                *previous = value;
            }
            if let Some(baseline) = &mut self.restored_baseline {
                baseline[index] = f64::NAN;
            }
        }

        let channels = [
            MidiChannel::Ch1,
            MidiChannel::Ch2,
            MidiChannel::Ch3,
            MidiChannel::Ch4,
            MidiChannel::Ch5,
            MidiChannel::Ch6,
            MidiChannel::Ch7,
            MidiChannel::Ch8,
        ];
        for frame in 0..frames {
            for (lane, channel) in channels.iter().copied().enumerate() {
                let (lane_pitch, lane_gate, lane_velocity) = if lane == 0 {
                    (pitch[frame], gate[frame], velocity[frame])
                } else {
                    let base = (lane - 1) * frames * 3;
                    (
                        extra_note_inputs[base + frame],
                        extra_note_inputs[base + frames + frame],
                        extra_note_inputs[base + frames * 2 + frame],
                    )
                };
                let gated = lane_gate.is_finite() && lane_gate > 0.0;
                let note = frequency_to_note(lane_pitch);
                match (self.active_notes[lane], gated) {
                    (None, true) => {
                        let event = MidiEvent::NoteOn {
                            channel,
                            note,
                            velocity: normalized_velocity(lane_velocity),
                        };
                        if self.plugin.send_midi_event_at(event, frame as i32).is_err() {
                            self.failed = true;
                            break;
                        }
                        self.active_notes[lane] = Some(note);
                    }
                    (Some(active), false) => {
                        let event = MidiEvent::NoteOff {
                            channel,
                            note: active,
                            velocity: 0,
                        };
                        if self.plugin.send_midi_event_at(event, frame as i32).is_err() {
                            self.failed = true;
                            break;
                        }
                        self.active_notes[lane] = None;
                    }
                    (Some(active), true) if active != note => {
                        let off = MidiEvent::NoteOff {
                            channel,
                            note: active,
                            velocity: 0,
                        };
                        let on = MidiEvent::NoteOn {
                            channel,
                            note,
                            velocity: normalized_velocity(lane_velocity),
                        };
                        if self.plugin.send_midi_event_at(off, frame as i32).is_err()
                            || self.plugin.send_midi_event_at(on, frame as i32).is_err()
                        {
                            self.failed = true;
                            break;
                        }
                        self.active_notes[lane] = Some(note);
                    }
                    _ => {}
                }
            }
            if self.failed {
                break;
            }
        }
        if self.failed {
            output.fill(0.0);
            return;
        }

        match &mut self.buffers {
            ProcessingBuffers::Flat(buffers) => {
                buffers.block_size = frames;
                for channel in &mut buffers.outputs {
                    channel[..frames].fill(0.0);
                }
                if self.plugin.process_audio(buffers).is_err() {
                    self.failed = true;
                    output.fill(0.0);
                    return;
                }
                let left = buffers.outputs.first();
                let right = buffers.outputs.get(1).or(left);
                for frame in 0..frames {
                    output[frame * 2] = left.map_or(0.0, |channel| f64::from(channel[frame]));
                    output[frame * 2 + 1] = right.map_or(0.0, |channel| f64::from(channel[frame]));
                }
            }
            ProcessingBuffers::Buses(buffers) => {
                buffers.block_size = frames;
                buffers.clear();
                if self.plugin.process_bus_audio(buffers).is_err() {
                    self.failed = true;
                    output.fill(0.0);
                    return;
                }
                for frame in 0..frames {
                    let mut left = 0.0;
                    let mut right = 0.0;
                    for bus in &buffers.outputs {
                        let first = bus.channels.first();
                        left += first.map_or(0.0, |channel| f64::from(channel[frame]));
                        right += bus
                            .channels
                            .get(1)
                            .or(first)
                            .map_or(0.0, |channel| f64::from(channel[frame]));
                    }
                    output[frame * 2] = left;
                    output[frame * 2 + 1] = right;
                }
            }
        }
    }
}

impl Drop for Instance {
    fn drop(&mut self) {
        let _ = self.plugin.stop_processing();
    }
}

fn frequency_to_note(frequency: f64) -> u8 {
    if !frequency.is_finite() || frequency <= 0.0 {
        return 0;
    }
    (69.0 + 12.0 * (frequency / 440.0).log2())
        .round()
        .clamp(0.0, 127.0) as u8
}

fn normalized_velocity(velocity: f64) -> u8 {
    (velocity.clamp(0.0, 1.0) * 127.0).round() as u8
}

pub fn is_bundle(path: &Path) -> bool {
    path.extension()
        .is_some_and(|extension| extension == "vst3")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn note_and_velocity_conversion_are_bounded() {
        assert_eq!(frequency_to_note(440.0), 69);
        assert_eq!(frequency_to_note(8.175_798_915_6), 0);
        assert_eq!(frequency_to_note(f64::INFINITY), 0);
        assert_eq!(normalized_velocity(0.5), 64);
        assert_eq!(normalized_velocity(2.0), 127);
    }

    #[test]
    fn stable_parameter_names_round_trip() {
        let spec = Spec {
            descriptor: Descriptor {
                path: "fixture.vst3".into(),
                class_id: "CLASS".into(),
                name: "Fixture".into(),
                vendor: "Meldritch".into(),
                category: "Instrument|Synth".into(),
                version: "1".into(),
                instrument: true,
            },
            parameters: vec![Parameter {
                id: 42,
                name: "Tone".into(),
                normalized: 0.5,
                default: 0.5,
                unit: "%".into(),
                step_count: 0,
                automatable: true,
                read_only: false,
            }],
            initial_values: BTreeMap::new(),
            state: None,
            activate_all_outputs: false,
        };
        assert_eq!(Spec::parameter_name(42), "parameter_42");
        assert_eq!(spec.parameter_id("parameter_42"), Some(42));
        assert_eq!(spec.parameter_id("parameter_7"), None);
    }
}
