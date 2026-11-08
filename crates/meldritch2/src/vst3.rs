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

use serde::{Deserialize, Serialize};

use vst3_host::{
    Plugin, PluginInfo, ProcessMode, Vst3Host,
    audio::{AudioBuffers, BusAudioBuffers, BusDirection, MediaType},
    midi::{MidiChannel, MidiEvent},
};

pub const MAX_BLOCK_SIZE: usize = 1024;
pub const NOTE_LANES: usize = 8;
pub const EXTRA_NOTE_INPUTS: usize = (NOTE_LANES - 1) * 3;
const STATE_FILE_MAGIC: &[u8; 16] = b"MELDRITCH_VST3\0\0";
const STATE_FILE_VERSION: u32 = 1;
const STATE_FILE_HEADER_BYTES: usize = 16 + 4 + 4 + 8;
const MAX_STATE_BYTES: usize = 64 * 1024 * 1024;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct StateMetadata {
    pub class_id: String,
    pub vendor: String,
    pub plugin_name: String,
    pub plugin_version: String,
    pub patch_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<serde_json::Value>,
    pub component_state_bytes: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub controller_state_bytes: Option<usize>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PluginState {
    pub format_version: u32,
    pub metadata: StateMetadata,
    /// The byte-exact, versioned component/controller snapshot produced by vst3-host.
    pub state: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PatchBankPlugin {
    pub class_id: String,
    pub vendor: String,
    pub name: String,
    pub version: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PatchEntry {
    pub id: String,
    pub name: String,
    pub file: PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<serde_json::Value>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PatchBank {
    pub format_version: u32,
    pub plugin: PatchBankPlugin,
    pub patches: Vec<PatchEntry>,
}

impl PatchBank {
    pub fn new(descriptor: &Descriptor) -> Self {
        Self {
            format_version: 1,
            plugin: PatchBankPlugin {
                class_id: descriptor.class_id.clone(),
                vendor: descriptor.vendor.clone(),
                name: descriptor.name.clone(),
                version: descriptor.version.clone(),
            },
            patches: Vec::new(),
        }
    }

    pub fn read(directory: &Path, descriptor: &Descriptor) -> Result<Self, String> {
        let manifest_path = directory.join("manifest.json");
        let bytes = std::fs::read(&manifest_path).map_err(|error| {
            format!(
                "failed to read patch-bank manifest '{}': {error}",
                manifest_path.display()
            )
        })?;
        let bank: Self = serde_json::from_slice(&bytes).map_err(|error| {
            format!(
                "invalid patch-bank manifest '{}': {error}",
                manifest_path.display()
            )
        })?;
        bank.validate(directory, descriptor)?;
        Ok(bank)
    }

    pub fn validate(&self, directory: &Path, descriptor: &Descriptor) -> Result<(), String> {
        if self.format_version != 1 {
            return Err(format!(
                "unsupported VST3 patch-bank format version {}",
                self.format_version
            ));
        }
        if !self
            .plugin
            .class_id
            .eq_ignore_ascii_case(&descriptor.class_id)
        {
            return Err(format!(
                "patch bank is for {} ({}) but the configured plugin is {} ({})",
                self.plugin.name, self.plugin.class_id, descriptor.name, descriptor.class_id
            ));
        }
        if self.patches.len() < 2 {
            return Err("a VST3 patch bank requires at least two patches".into());
        }
        let mut ids = std::collections::BTreeSet::new();
        let mut names = std::collections::BTreeSet::new();
        for patch in &self.patches {
            if patch.id.is_empty()
                || patch.name.is_empty()
                || !ids.insert(&patch.id)
                || !names.insert(&patch.name)
            {
                return Err("VST3 patch IDs and names must be non-empty and unique".into());
            }
            if patch.file.is_absolute()
                || patch
                    .file
                    .components()
                    .any(|part| matches!(part, std::path::Component::ParentDir))
            {
                return Err(format!(
                    "patch '{}' must use a relative file inside its bank",
                    patch.id
                ));
            }
            let state = PluginState::from_bytes(
                &std::fs::read(directory.join(&patch.file))
                    .map_err(|error| format!("failed to read patch '{}': {error}", patch.id))?,
            )?;
            state.validate(descriptor)?;
        }
        Ok(())
    }

    pub fn write(&self, directory: &Path) -> Result<(), String> {
        std::fs::create_dir_all(directory).map_err(|error| error.to_string())?;
        let bytes = serde_json::to_vec_pretty(self).map_err(|error| error.to_string())?;
        let temporary = directory.join(".manifest.json.tmp");
        std::fs::write(&temporary, bytes).map_err(|error| error.to_string())?;
        std::fs::rename(temporary, directory.join("manifest.json"))
            .map_err(|error| error.to_string())
    }
}

impl PluginState {
    pub fn capture(
        descriptor: &Descriptor,
        patch_name: String,
        state: Vec<u8>,
    ) -> Result<Self, String> {
        let (component_state_bytes, controller_state_bytes) = state_stream_sizes(&state)?;
        Ok(Self {
            format_version: STATE_FILE_VERSION,
            metadata: StateMetadata {
                class_id: descriptor.class_id.clone(),
                vendor: descriptor.vendor.clone(),
                plugin_name: descriptor.name.clone(),
                plugin_version: descriptor.version.clone(),
                patch_name,
                source: None,
                component_state_bytes,
                controller_state_bytes,
            },
            state,
        })
    }

    pub fn validate(&self, descriptor: &Descriptor) -> Result<(), String> {
        if self.format_version != STATE_FILE_VERSION {
            return Err(format!(
                "unsupported Meldritch VST3 state format version {}",
                self.format_version
            ));
        }
        if !self
            .metadata
            .class_id
            .eq_ignore_ascii_case(&descriptor.class_id)
        {
            return Err(format!(
                "VST3 state is for {} ({}) but the configured plugin is {} ({})",
                self.metadata.plugin_name,
                self.metadata.class_id,
                descriptor.name,
                descriptor.class_id
            ));
        }
        if self.metadata.vendor != descriptor.vendor
            || self.metadata.plugin_name != descriptor.name
            || self.metadata.plugin_version != descriptor.version
        {
            return Err(format!(
                "VST3 state metadata identifies {} {} by {}, but the installed plugin is {} {} by {}",
                self.metadata.plugin_name,
                self.metadata.plugin_version,
                self.metadata.vendor,
                descriptor.name,
                descriptor.version,
                descriptor.vendor
            ));
        }
        let (component, controller) = state_stream_sizes(&self.state)?;
        if component != self.metadata.component_state_bytes
            || controller != self.metadata.controller_state_bytes
        {
            return Err("VST3 state stream sizes do not match its metadata".into());
        }
        Ok(())
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>, String> {
        if self.state.len() > MAX_STATE_BYTES {
            return Err("VST3 state exceeds the 64 MiB limit".into());
        }
        let metadata = serde_json::to_vec(&self.metadata).map_err(|error| error.to_string())?;
        let metadata_len =
            u32::try_from(metadata.len()).map_err(|_| "VST3 state metadata is too large")?;
        let state_len = u64::try_from(self.state.len()).map_err(|_| "VST3 state is too large")?;
        let mut bytes =
            Vec::with_capacity(STATE_FILE_HEADER_BYTES + metadata.len() + self.state.len());
        bytes.extend_from_slice(STATE_FILE_MAGIC);
        bytes.extend_from_slice(&self.format_version.to_le_bytes());
        bytes.extend_from_slice(&metadata_len.to_le_bytes());
        bytes.extend_from_slice(&state_len.to_le_bytes());
        bytes.extend_from_slice(&metadata);
        bytes.extend_from_slice(&self.state);
        Ok(bytes)
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        if !bytes.starts_with(STATE_FILE_MAGIC) {
            return Err("not a Meldritch VST3 state file".into());
        }
        if bytes.len() < STATE_FILE_HEADER_BYTES {
            return Err("truncated Meldritch VST3 state header".into());
        }
        let version = u32::from_le_bytes(bytes[16..20].try_into().unwrap());
        let metadata_len = u32::from_le_bytes(bytes[20..24].try_into().unwrap()) as usize;
        let state_len = usize::try_from(u64::from_le_bytes(bytes[24..32].try_into().unwrap()))
            .map_err(|_| "VST3 state size is not representable")?;
        if state_len > MAX_STATE_BYTES {
            return Err("VST3 state exceeds the 64 MiB limit".into());
        }
        let metadata_end = STATE_FILE_HEADER_BYTES
            .checked_add(metadata_len)
            .ok_or("VST3 state size overflow")?;
        let expected = metadata_end
            .checked_add(state_len)
            .ok_or("VST3 state size overflow")?;
        if expected != bytes.len() {
            return Err(format!(
                "invalid VST3 state file size: header describes {expected} bytes, got {}",
                bytes.len()
            ));
        }
        let metadata = serde_json::from_slice(&bytes[STATE_FILE_HEADER_BYTES..metadata_end])
            .map_err(|error| format!("invalid VST3 state metadata: {error}"))?;
        Ok(Self {
            format_version: version,
            metadata,
            state: bytes[metadata_end..].to_vec(),
        })
    }
}

fn state_stream_sizes(state: &[u8]) -> Result<(usize, Option<usize>), String> {
    const HOST_MAGIC: &[u8; 16] = b"VST3HOST_STATE\0\0";
    if !state.starts_with(HOST_MAGIC) {
        return Ok((state.len(), None));
    }
    if state.len() < 28 {
        return Err("truncated vst3-host state snapshot".into());
    }
    let version = u32::from_le_bytes(state[16..20].try_into().unwrap());
    if version != 1 {
        return Err(format!(
            "unsupported vst3-host state snapshot version {version}"
        ));
    }
    let component = u32::from_le_bytes(state[20..24].try_into().unwrap()) as usize;
    let encoded_controller = u32::from_le_bytes(state[24..28].try_into().unwrap());
    let controller = (encoded_controller != u32::MAX).then_some(encoded_controller as usize);
    let expected = 28usize
        .checked_add(component)
        .and_then(|size| size.checked_add(controller.unwrap_or(0)))
        .ok_or("VST3 state size overflow")?;
    if expected != state.len() {
        return Err("invalid vst3-host state snapshot size".into());
    }
    Ok((component, controller))
}

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
    /// Plugin-formatted representation of the current normalized value.
    pub display: String,
    pub bypass: bool,
    pub flags: u32,
}

impl Parameter {
    fn from_host(parameter: vst3_host::parameters::Parameter, display: String) -> Self {
        Self {
            id: parameter.id,
            name: parameter.name,
            normalized: parameter.value,
            default: parameter.default,
            unit: parameter.unit,
            step_count: parameter.step_count,
            automatable: parameter.can_automate,
            read_only: parameter.is_read_only,
            display,
            bypass: parameter.is_bypass,
            flags: parameter.flags,
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
    /// Human-readable MIDI channel (1..=16) assigned to each logical note lane.
    pub midi_channels: [u8; NOTE_LANES],
    /// Number of channels on the main input bus. Zero identifies an instrument.
    pub audio_inputs: usize,
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

fn module_descriptors(path: &Path) -> Option<Vec<Descriptor>> {
    const MAX_MODULE_INFO_BYTES: u64 = 4 * 1024 * 1024;
    let file = path.join("Contents/Resources/moduleinfo.json");
    if std::fs::metadata(&file).ok()?.len() > MAX_MODULE_INFO_BYTES {
        return None;
    }
    let value: serde_json::Value = serde_json::from_slice(&std::fs::read(file).ok()?).ok()?;
    let factory_vendor = value
        .get("Factory Info")
        .and_then(|factory| factory.get("Vendor"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    let classes = value.get("Classes")?.as_array()?;
    let mut descriptors = Vec::new();
    for class in classes {
        let class_category = class
            .get("Category")
            .or_else(|| class.get("Catetory"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        if class_category != "Audio Module Class" {
            continue;
        }
        let sub_categories = class
            .get("Sub Categories")
            .and_then(serde_json::Value::as_array)
            .map(|values| {
                values
                    .iter()
                    .filter_map(serde_json::Value::as_str)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let class_id = class.get("CID")?.as_str()?.to_owned();
        let name = class.get("Name")?.as_str()?.to_owned();
        let vendor = class
            .get("Vendor")
            .and_then(serde_json::Value::as_str)
            .filter(|vendor| !vendor.is_empty())
            .unwrap_or(factory_vendor)
            .to_owned();
        let version = class
            .get("Version")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_owned();
        descriptors.push(Descriptor {
            path: path.to_owned(),
            class_id,
            name,
            vendor,
            category: sub_categories.join("|"),
            version,
            instrument: sub_categories
                .iter()
                .any(|part| part.eq_ignore_ascii_case("instrument")),
        });
    }
    (!descriptors.is_empty()).then_some(descriptors)
}

pub fn discover(paths: &[PathBuf]) -> Result<Vec<Descriptor>, String> {
    let selected = if paths.is_empty() {
        configured_paths()
    } else {
        paths.to_vec()
    };
    let host = host_builder(&selected)
        .build()
        .map_err(|error| error.to_string())?;
    let mut plugins = Vec::new();
    for path in host.scan_plugin_paths() {
        if let Some(mut descriptors) = module_descriptors(&path) {
            plugins.append(&mut descriptors);
            continue;
        }
        match vst3_host::discovery::read_module_info(&path) {
            Ok(Some(module)) => {
                for class in module
                    .classes
                    .iter()
                    .filter(|class| class.category == "Audio Module Class")
                {
                    let category = class.sub_categories.join("|");
                    let instrument = class
                        .sub_categories
                        .iter()
                        .any(|part| part.eq_ignore_ascii_case("instrument"));
                    plugins.push(Descriptor {
                        path: path.clone(),
                        class_id: class.class_id.clone(),
                        name: class.name.clone(),
                        vendor: if class.vendor.is_empty() {
                            module.factory.vendor.clone()
                        } else {
                            class.vendor.clone()
                        },
                        category,
                        version: class.version.clone(),
                        instrument,
                    });
                }
            }
            Ok(None) => {
                if let Ok(plugin) = vst3_host::discovery::get_plugin_info(&path) {
                    plugins.push(Descriptor::from(plugin));
                }
            }
            Err(_) => {}
        }
    }
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
    let audio_inputs = if descriptor.instrument { 0 } else { 2 };
    let mut host = Vst3Host::builder()
        .sample_rate(48_000.0)
        .block_size(MAX_BLOCK_SIZE)
        .input_channels(audio_inputs)
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
    let layout = plugin.audio_bus_layout().map_err(|error| {
        format!(
            "failed to inspect '{}' audio buses: {error}",
            descriptor.name
        )
    })?;
    if audio_inputs == 2 {
        let input_channels = layout.inputs.first().map(|bus| bus.channel_count);
        let output_channels = layout.outputs.first().map(|bus| bus.channel_count);
        if input_channels != Some(2) || output_channels != Some(2) {
            return Err(format!(
                "VST3 effect '{}' requires unsupported main buses: input={input_channels:?}, output={output_channels:?}; stereo 2-in/2-out is required",
                descriptor.name
            ));
        }
    }
    let host_parameters = plugin
        .get_parameters()
        .map_err(|error| format!("failed to enumerate '{}': {error}", descriptor.name))?;
    let mut parameters = Vec::with_capacity(host_parameters.len());
    for parameter in host_parameters {
        let display = plugin
            .format_parameter(parameter.id, parameter.value)
            .unwrap_or_else(|_| parameter.format_value(parameter.value));
        parameters.push(Parameter::from_host(parameter, display));
    }
    Ok(Spec {
        descriptor,
        parameters,
        initial_values: BTreeMap::new(),
        state: None,
        activate_all_outputs: false,
        midi_channels: [1, 2, 3, 4, 5, 6, 7, 8],
        audio_inputs,
    })
}

pub fn load(selector: &str, paths: &[PathBuf]) -> Result<Spec, String> {
    inspect(resolve(selector, paths)?)
}

pub struct Instance {
    plugin: Plugin,
    buffers: ProcessingBuffers,
    parameters: Vec<(usize, u32)>,
    previous_parameters: Vec<f64>,
    restored_baseline: Option<Vec<f64>>,
    active_notes: [Option<u8>; NOTE_LANES],
    midi_channels: [MidiChannel; NOTE_LANES],
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
            .input_channels(spec.audio_inputs)
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
        for (index, parameter) in spec.parameters.iter().enumerate() {
            let value = spec.initial_value(parameter);
            if !parameter.read_only
                && (parameter.automatable || spec.initial_values.contains_key(&parameter.id))
            {
                plugin.set_parameter(parameter.id, value).map_err(|error| {
                    format!(
                        "failed to initialize VST3 parameter {} on '{}': {error}",
                        parameter.id, spec.descriptor.name
                    )
                })?;
                if parameter.automatable {
                    parameters.push((index, parameter.id));
                    previous_parameters.push(value);
                }
            }
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
            for (previous, (_, id)) in previous_parameters.iter_mut().zip(&parameters) {
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
            ProcessingBuffers::Flat(AudioBuffers::new(
                spec.audio_inputs,
                2,
                MAX_BLOCK_SIZE,
                sample_rate,
            ))
        };
        plugin.start_processing().map_err(|error| {
            format!(
                "failed to start VST3 plugin '{}': {error}",
                spec.descriptor.name
            )
        })?;
        let mut midi_channels = [MidiChannel::Ch1; NOTE_LANES];
        for (lane, channel) in spec.midi_channels.iter().copied().enumerate() {
            midi_channels[lane] = channel
                .checked_sub(1)
                .and_then(MidiChannel::from_index)
                .ok_or_else(|| {
                    format!(
                        "VST3 MIDI channel for lane {} must be in 1..16, got {channel}",
                        lane + 1
                    )
                })?;
        }
        Ok(Self {
            plugin,
            buffers,
            parameters,
            previous_parameters,
            restored_baseline,
            active_notes: [None; NOTE_LANES],
            midi_channels,
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

    pub fn parameter_values(&self, ids: &[u32]) -> Result<Vec<f64>, String> {
        ids.iter()
            .map(|id| {
                self.plugin
                    .get_parameter(*id)
                    .map_err(|error| error.to_string())
            })
            .collect()
    }

    pub fn latency_samples(&self) -> u32 {
        self.plugin.latency_samples()
    }

    fn apply_parameters(&mut self, parameter_values: &[f64]) -> Result<(), ()> {
        for (active_index, (&(parameter_index, id), previous)) in self
            .parameters
            .iter()
            .zip(&mut self.previous_parameters)
            .enumerate()
        {
            let value = parameter_values[parameter_index];
            if self
                .restored_baseline
                .as_ref()
                .is_some_and(|baseline| value.to_bits() == baseline[active_index].to_bits())
            {
                continue;
            }
            if value.to_bits() != previous.to_bits() {
                if self.plugin.set_parameter_at(id, value, 0).is_err() {
                    self.failed = true;
                    return Err(());
                }
                *previous = value;
            }
            if let Some(baseline) = &mut self.restored_baseline {
                baseline[active_index] = f64::NAN;
            }
        }
        Ok(())
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

        if self.apply_parameters(parameter_values).is_err() {
            output.fill(0.0);
            return;
        }

        for frame in 0..frames {
            for (lane, channel) in self.midi_channels.iter().copied().enumerate() {
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

    pub fn process_effect(
        &mut self,
        left_input: &[f64],
        right_input: &[f64],
        parameter_values: &[f64],
        output: &mut [f64],
    ) {
        let frames = left_input.len();
        debug_assert_eq!(right_input.len(), frames);
        debug_assert_eq!(output.len(), frames * 2);
        if self.failed || frames == 0 || frames > MAX_BLOCK_SIZE {
            output.fill(0.0);
            self.failed |= frames > MAX_BLOCK_SIZE;
            return;
        }
        if self.apply_parameters(parameter_values).is_err() {
            output.fill(0.0);
            return;
        }

        match &mut self.buffers {
            ProcessingBuffers::Flat(buffers) => {
                buffers.block_size = frames;
                if buffers.inputs.len() != 2 {
                    self.failed = true;
                    output.fill(0.0);
                    return;
                }
                for frame in 0..frames {
                    buffers.inputs[0][frame] = left_input[frame] as f32;
                    buffers.inputs[1][frame] = right_input[frame] as f32;
                }
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
                let Some(input) = buffers.inputs.iter_mut().find(|bus| bus.active) else {
                    self.failed = true;
                    output.fill(0.0);
                    return;
                };
                if input.channels.len() != 2 {
                    self.failed = true;
                    output.fill(0.0);
                    return;
                }
                for frame in 0..frames {
                    input.channels[0][frame] = left_input[frame] as f32;
                    input.channels[1][frame] = right_input[frame] as f32;
                }
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
                display: "50 %".into(),
                bypass: false,
                flags: 0,
            }],
            initial_values: BTreeMap::new(),
            state: None,
            activate_all_outputs: false,
            midi_channels: [1, 2, 3, 4, 5, 6, 7, 8],
            audio_inputs: 0,
        };
        assert_eq!(Spec::parameter_name(42), "parameter_42");
        assert_eq!(spec.parameter_id("parameter_42"), Some(42));
        assert_eq!(spec.parameter_id("parameter_7"), None);
    }

    #[test]
    fn meldritch_state_file_preserves_component_and_controller_bytes() {
        let descriptor = Descriptor {
            path: "fixture.vst3".into(),
            class_id: "00112233445566778899AABBCCDDEEFF".into(),
            name: "Fixture".into(),
            vendor: "Meldritch".into(),
            category: "Instrument".into(),
            version: "1.2.3".into(),
            instrument: true,
        };
        let component = [0, 1, 2, 0xff];
        let controller = [9, 8, 0, 7, 6];
        let mut snapshot = Vec::from(&b"VST3HOST_STATE\0\0"[..]);
        snapshot.extend_from_slice(&1u32.to_le_bytes());
        snapshot.extend_from_slice(&(component.len() as u32).to_le_bytes());
        snapshot.extend_from_slice(&(controller.len() as u32).to_le_bytes());
        snapshot.extend_from_slice(&component);
        snapshot.extend_from_slice(&controller);

        let state = PluginState::capture(&descriptor, "Opaque patch".into(), snapshot.clone())
            .expect("capture state document");
        assert_eq!(state.metadata.component_state_bytes, component.len());
        assert_eq!(
            state.metadata.controller_state_bytes,
            Some(controller.len())
        );
        let decoded = PluginState::from_bytes(&state.to_bytes().expect("serialize state"))
            .expect("decode state");
        assert_eq!(decoded.state, snapshot);
        decoded.validate(&descriptor).expect("validate identity");

        let mut other = descriptor.clone();
        other.class_id = "FFEEDDCCBBAA99887766554433221100".into();
        assert!(decoded.validate(&other).is_err());
        let mut other_version = descriptor.clone();
        other_version.version = "2.0.0".into();
        assert!(decoded.validate(&other_version).is_err());
        assert!(PluginState::from_bytes(STATE_FILE_MAGIC).is_err());
    }
}
