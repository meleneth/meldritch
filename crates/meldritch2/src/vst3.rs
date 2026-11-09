//! Native Linux VST3 discovery and in-process instrument hosting.
//!
//! This module is deliberately independent of the Lua and graph layers.  A
//! future process-isolated adapter can implement the same prepared-instance
//! contract without changing authored compositions.

use std::{
    collections::BTreeMap,
    env,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex, OnceLock, Weak,
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
        mpsc::{Receiver, SyncSender, TryRecvError, TrySendError, sync_channel},
    },
};

use serde::{Deserialize, Serialize};

#[cfg(not(target_os = "linux"))]
use vst3_host::PluginWindow;
use vst3_host::{
    Plugin, PluginInfo, ProcessMode, Vst3Host,
    audio::{AudioBuffers, BusAudioBuffers, BusDirection, MediaType},
    midi::{MidiChannel, MidiEvent},
};

#[derive(Debug, Default)]
pub struct EditorControl {
    requested: AtomicBool,
    plugin: Mutex<Weak<Mutex<Plugin>>>,
}

impl EditorControl {
    pub fn request(&self) {
        self.requested.store(true, Ordering::Release);
    }

    pub fn take_request(&self) -> bool {
        self.requested.swap(false, Ordering::AcqRel)
    }

    fn attach(&self, plugin: &Arc<Mutex<Plugin>>) {
        *self
            .plugin
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Arc::downgrade(plugin);
    }

    pub fn plugin(&self) -> Option<Arc<Mutex<Plugin>>> {
        self.plugin
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .upgrade()
    }
}

pub struct EditorSession {
    pub control: Arc<EditorControl>,
    plugin: Arc<Mutex<Plugin>>,
    #[cfg(not(target_os = "linux"))]
    window: PluginWindow,
}

impl EditorSession {
    pub fn open(control: Arc<EditorControl>) -> Result<Self, String> {
        let plugin = control
            .plugin()
            .ok_or("VST3 processor is not ready for its editor")?;
        #[cfg(target_os = "linux")]
        {
            plugin
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .open_editor(vst3_host::WindowHandle::from_x11(0))
                .map_err(|error| error.to_string())?;
            return Ok(Self { control, plugin });
        }
        #[cfg(not(target_os = "linux"))]
        let mut window = PluginWindow::new(Arc::clone(&plugin));
        #[cfg(not(target_os = "linux"))]
        window.open().map_err(|error| error.to_string())?;
        #[cfg(not(target_os = "linux"))]
        Ok(Self {
            control,
            plugin,
            window,
        })
    }

    pub fn service(&mut self) -> Result<bool, String> {
        #[cfg(not(target_os = "linux"))]
        self.window
            .service_platform_events()
            .map_err(|error| error.to_string())?;
        let replaced = self
            .control
            .plugin()
            .is_none_or(|current| !Arc::ptr_eq(&current, &self.plugin));
        #[cfg(target_os = "linux")]
        return Ok(replaced);
        #[cfg(not(target_os = "linux"))]
        Ok(replaced || self.window.closed_by_user())
    }
}

#[cfg(target_os = "linux")]
impl Drop for EditorSession {
    fn drop(&mut self) {
        let _ = self
            .plugin
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .close_editor();
    }
}

pub const MAX_BLOCK_SIZE: usize = 1024;
pub const NOTE_LANES: usize = 8;
pub const EXTRA_NOTE_INPUTS: usize = (NOTE_LANES - 1) * 3;
static ZERO_SIGNAL: [f64; MAX_BLOCK_SIZE] = [0.0; MAX_BLOCK_SIZE];
static ZERO_NOTE_INPUTS: [f64; MAX_BLOCK_SIZE * EXTRA_NOTE_INPUTS] =
    [0.0; MAX_BLOCK_SIZE * EXTRA_NOTE_INPUTS];
const STATE_FILE_MAGIC: &[u8; 16] = b"MELDRITCH_VST3\0\0";
const STATE_FILE_VERSION: u32 = 1;
const STATE_FILE_HEADER_BYTES: usize = 16 + 4 + 4 + 8;
const MAX_STATE_BYTES: usize = 64 * 1024 * 1024;
static PLUGIN_LIFECYCLE: OnceLock<Mutex<()>> = OnceLock::new();

fn plugin_lifecycle_lock() -> std::sync::MutexGuard<'static, ()> {
    PLUGIN_LIFECYCLE
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CatalogPatch {
    pub id: String,
    pub name: String,
    pub bank: String,
    pub category: String,
    pub file: PathBuf,
}

#[derive(Debug)]
pub struct PresetStatus {
    requested: AtomicUsize,
    active: AtomicUsize,
    pending: AtomicBool,
    generation: AtomicU64,
    request_epoch: AtomicU64,
    failure: Mutex<Option<(u64, String)>>,
}

impl PresetStatus {
    fn new(active: usize) -> Self {
        Self {
            requested: AtomicUsize::new(active),
            active: AtomicUsize::new(active),
            pending: AtomicBool::new(false),
            generation: AtomicU64::new(0),
            request_epoch: AtomicU64::new(0),
            failure: Mutex::new(None),
        }
    }

    pub fn request(&self, index: usize) {
        self.request_realtime(index);
        if let Ok(mut failure) = self.failure.lock() {
            *failure = None;
        }
    }

    fn request_realtime(&self, index: usize) {
        self.request_epoch.fetch_add(1, Ordering::AcqRel);
        self.requested.store(index, Ordering::Release);
        self.pending
            .store(index != self.active(), Ordering::Release);
    }

    pub fn active(&self) -> usize {
        self.active.load(Ordering::Acquire)
    }
    pub fn requested(&self) -> usize {
        self.requested.load(Ordering::Acquire)
    }
    pub fn pending(&self) -> bool {
        self.pending.load(Ordering::Acquire)
    }
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }
    pub fn request_epoch(&self) -> u64 {
        self.request_epoch.load(Ordering::Acquire)
    }
    pub fn failure(&self) -> Option<String> {
        self.failure
            .lock()
            .ok()
            .and_then(|value| value.as_ref().map(|(_, message)| message.clone()))
    }
}

#[derive(Clone, Debug)]
pub struct PresetCatalog {
    pub patches: Vec<CatalogPatch>,
    pub initial: usize,
    pub status: Arc<PresetStatus>,
}

#[derive(Clone, Debug)]
pub struct ManagedPresetBank {
    pub catalog: Arc<PresetCatalog>,
    pub values: Vec<BTreeMap<u32, f64>>,
}

impl PresetCatalog {
    pub fn new(patches: Vec<CatalogPatch>, initial: usize) -> Result<Self, String> {
        if patches.len() < 2 || initial >= patches.len() {
            return Err(
                "a preset catalog requires at least two entries and a valid initial preset".into(),
            );
        }
        Ok(Self {
            patches,
            initial,
            status: Arc::new(PresetStatus::new(initial)),
        })
    }
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
        for patch in &self.patches {
            if patch.id.is_empty() || patch.name.is_empty() || !ids.insert(&patch.id) {
                return Err(
                    "VST3 patch IDs must be non-empty and unique; names must be non-empty".into(),
                );
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
    /// Resident-bank index selected by the `patch_selector` input. Ordinary
    /// VST3 modules leave this unset and process every block.
    pub patch_index: Option<u32>,
    /// A large, hot-swappable state catalog. Only one processor is resident;
    /// replacement processors are prepared by a bounded background worker.
    pub preset_catalog: Option<Arc<PresetCatalog>>,
    /// Small host-managed parameter presets applied to one persistent plugin.
    pub managed_presets: Option<Arc<ManagedPresetBank>>,
    /// Optional host-native editor access for explicitly opted-in compositions.
    pub editor: Option<Arc<EditorControl>>,
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
        patch_index: None,
        preset_catalog: None,
        managed_presets: None,
        editor: None,
    })
}

pub fn load(selector: &str, paths: &[PathBuf]) -> Result<Spec, String> {
    inspect(resolve(selector, paths)?)
}

pub struct Instance {
    plugin: Arc<Mutex<Plugin>>,
    buffers: ProcessingBuffers,
    parameters: Vec<(usize, u32)>,
    previous_parameters: Vec<f64>,
    restored_baseline: Option<Vec<f64>>,
    active_notes: [Option<u8>; NOTE_LANES],
    midi_channels: [MidiChannel; NOTE_LANES],
    failed: bool,
    offline_fallback: bool,
    previous_tempo: f64,
    patch_index: Option<u32>,
    was_selected: bool,
}

enum ProcessingBuffers {
    Flat(AudioBuffers),
    Buses(BusAudioBuffers),
}

impl std::fmt::Debug for Instance {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let plugin = self
            .plugin
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        formatter
            .debug_struct("Vst3Instance")
            .field("plugin", &plugin.info().name)
            .field("class_id", &plugin.info().uid)
            .field("failed", &self.failed)
            .field("offline_fallback", &self.offline_fallback)
            .finish()
    }
}

impl Instance {
    pub fn new(spec: &Spec, sample_rate: f64) -> Result<Self, String> {
        // Some third-party factories (notably Serum 2.1.5) are not safe to
        // construct concurrently even though their prepared processors are
        // independent. This lock is used only by control/background threads.
        let _creation = plugin_lifecycle_lock();
        let mut host = Vst3Host::builder()
            .sample_rate(sample_rate)
            .block_size(MAX_BLOCK_SIZE)
            .input_channels(spec.audio_inputs)
            .output_channels(2)
            .with_process_isolation(spec.editor.is_some())
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
        let plugin = Arc::new(Mutex::new(plugin));
        if let Some(editor) = &spec.editor {
            editor.attach(&plugin);
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
            patch_index: spec.patch_index,
            was_selected: false,
        })
    }

    pub fn set_offline(&mut self) {
        if self.failed {
            return;
        }
        let mut plugin = self
            .plugin
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if plugin.stop_processing().is_err() {
            self.failed = true;
            return;
        }
        if plugin.set_process_mode(ProcessMode::Offline).is_err() {
            self.offline_fallback = true;
            let _ = plugin.set_process_mode(ProcessMode::Realtime);
        }
        if plugin.start_processing().is_err() {
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
        self.plugin
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .save_state()
            .map_err(|error| error.to_string())
    }

    pub fn parameter_values(&self, ids: &[u32]) -> Result<Vec<f64>, String> {
        let plugin = self
            .plugin
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        ids.iter()
            .map(|id| plugin.get_parameter(*id).map_err(|error| error.to_string()))
            .collect()
    }

    pub fn latency_samples(&self) -> u32 {
        self.plugin
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .latency_samples()
    }

    fn apply_parameters(
        &mut self,
        plugin: &mut Plugin,
        parameter_values: &[f64],
    ) -> Result<(), ()> {
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
                if plugin.set_parameter_at(id, value, 0).is_err() {
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
        let plugin = Arc::clone(&self.plugin);
        let Ok(mut plugin) = plugin.try_lock() else {
            output.fill(0.0);
            return;
        };

        let tempo = tempo[0];
        if tempo.is_finite() && tempo > 0.0 && tempo.to_bits() != self.previous_tempo.to_bits() {
            if plugin.set_tempo(tempo).is_err() {
                self.failed = true;
                output.fill(0.0);
                return;
            }
            self.previous_tempo = tempo;
        }

        if self
            .apply_parameters(&mut plugin, parameter_values)
            .is_err()
        {
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
                        if plugin.send_midi_event_at(event, frame as i32).is_err() {
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
                        if plugin.send_midi_event_at(event, frame as i32).is_err() {
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
                        if plugin.send_midi_event_at(off, frame as i32).is_err()
                            || plugin.send_midi_event_at(on, frame as i32).is_err()
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
                if plugin.process_audio(buffers).is_err() {
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
                if plugin.process_bus_audio(buffers).is_err() {
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

    pub fn process_selected(
        &mut self,
        note_inputs: [&[f64]; 5],
        parameter_values: &[f64],
        patch_selector: &[f64],
        output: &mut [f64],
    ) {
        let selected = self.patch_index.is_none_or(|index| {
            patch_selector
                .first()
                .copied()
                .filter(|value| value.is_finite())
                .is_some_and(|value| value.round().clamp(0.0, f64::from(u32::MAX)) as u32 == index)
        });
        if selected {
            self.was_selected = true;
            self.process(note_inputs, parameter_values, output);
            return;
        }
        if self.was_selected {
            self.was_selected = false;
            let frames = note_inputs[0].len();
            let pitch = note_inputs[0];
            let tempo = note_inputs[3];
            self.process(
                [
                    pitch,
                    &ZERO_SIGNAL[..frames],
                    &ZERO_SIGNAL[..frames],
                    tempo,
                    &ZERO_NOTE_INPUTS[..frames * EXTRA_NOTE_INPUTS],
                ],
                parameter_values,
                output,
            );
        }
        output.fill(0.0);
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
        let plugin = Arc::clone(&self.plugin);
        let Ok(mut plugin) = plugin.try_lock() else {
            output.fill(0.0);
            return;
        };
        if self
            .apply_parameters(&mut plugin, parameter_values)
            .is_err()
        {
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
                if plugin.process_audio(buffers).is_err() {
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
                if plugin.process_bus_audio(buffers).is_err() {
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
        let _lifecycle = plugin_lifecycle_lock();
        let _ = self
            .plugin
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .stop_processing();
    }
}

pub struct ManagedPresetInstance {
    current: Box<Instance>,
    bank: Arc<ManagedPresetBank>,
    parameter_indexes: Vec<Vec<(usize, f64)>>,
    authored: Vec<f64>,
    scratch: Vec<f64>,
    selected: usize,
}

impl std::fmt::Debug for ManagedPresetInstance {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Vst3ManagedPresetInstance")
            .field("selected", &self.selected)
            .field("presets", &self.bank.values.len())
            .finish()
    }
}

impl ManagedPresetInstance {
    pub fn new(spec: &Spec, sample_rate: f64) -> Result<Self, String> {
        let bank = Arc::clone(
            spec.managed_presets
                .as_ref()
                .ok_or("missing managed preset bank")?,
        );
        let parameter_indexes = bank
            .values
            .iter()
            .map(|values| {
                values
                    .iter()
                    .map(|(id, value)| {
                        spec.parameters
                            .iter()
                            .position(|parameter| parameter.id == *id)
                            .map(|index| (index, *value))
                            .ok_or_else(|| {
                                format!("managed preset references unknown parameter {id}")
                            })
                    })
                    .collect::<Result<Vec<_>, _>>()
            })
            .collect::<Result<Vec<_>, _>>()?;
        let authored = spec
            .parameters
            .iter()
            .map(|parameter| spec.initial_value(parameter))
            .collect::<Vec<_>>();
        Ok(Self {
            current: Box::new(Instance::new(spec, sample_rate)?),
            selected: bank.catalog.initial,
            bank,
            parameter_indexes,
            scratch: authored.clone(),
            authored,
        })
    }

    pub fn process(
        &mut self,
        note_inputs: [&[f64]; 5],
        parameter_values: &[f64],
        patch_selector: &[f64],
        output: &mut [f64],
    ) {
        let selected = patch_selector
            .first()
            .copied()
            .filter(|value| value.is_finite())
            .map(|value| {
                value
                    .round()
                    .clamp(0.0, self.parameter_indexes.len().saturating_sub(1) as f64)
                    as usize
            })
            .unwrap_or(self.selected);
        if selected != self.selected {
            self.selected = selected;
            self.bank.catalog.status.request_realtime(selected);
            self.bank
                .catalog
                .status
                .active
                .store(selected, Ordering::Release);
            self.bank
                .catalog
                .status
                .pending
                .store(false, Ordering::Release);
        }
        self.scratch.copy_from_slice(parameter_values);
        for &(index, value) in &self.parameter_indexes[self.selected] {
            if parameter_values[index].to_bits() == self.authored[index].to_bits() {
                self.scratch[index] = value;
            }
        }
        self.current.process(note_inputs, &self.scratch, output);
    }
    pub fn set_offline(&mut self) {
        self.current.set_offline();
    }
    pub fn failed(&self) -> bool {
        self.current.failed()
    }
    pub fn offline_fallback(&self) -> bool {
        self.current.offline_fallback()
    }
    pub fn save_state(&self) -> Result<Vec<u8>, String> {
        self.current.save_state()
    }
    pub fn latency_samples(&self) -> u32 {
        self.current.latency_samples()
    }
    pub fn status(&self) -> Arc<PresetStatus> {
        Arc::clone(&self.bank.catalog.status)
    }
}

enum CatalogRequest {
    Load { index: usize, generation: u64 },
    Retire(Box<Instance>),
}

struct CatalogResult {
    index: usize,
    generation: u64,
    instance: Result<Box<Instance>, String>,
}

/// One live processor backed by a potentially large on-disk preset catalog.
/// The audio callback only uses bounded `try_send`/`try_recv` operations.
pub struct CatalogInstance {
    current: Box<Instance>,
    catalog: Arc<PresetCatalog>,
    requests: SyncSender<CatalogRequest>,
    results: Receiver<CatalogResult>,
    desired: usize,
    submitted: Option<(usize, u64)>,
    failed_selection: Option<usize>,
    retired: Vec<Box<Instance>>,
    failure_update: Option<Option<(u64, String)>>,
    offline: Arc<AtomicBool>,
    seen_request_epoch: u64,
}

impl std::fmt::Debug for CatalogInstance {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Vst3CatalogInstance")
            .field("active", &self.catalog.status.active())
            .field("desired", &self.desired)
            .field("entries", &self.catalog.patches.len())
            .finish()
    }
}

impl CatalogInstance {
    pub fn new(spec: &Spec, sample_rate: f64) -> Result<Self, String> {
        let catalog = Arc::clone(
            spec.preset_catalog
                .as_ref()
                .ok_or("missing preset catalog")?,
        );
        let current = Box::new(Instance::new(spec, sample_rate)?);
        let (request_tx, request_rx) = sync_channel::<CatalogRequest>(4);
        let (result_tx, result_rx) = sync_channel::<CatalogResult>(2);
        let mut base = spec.clone();
        base.preset_catalog = None;
        base.state = None;
        let worker_catalog = Arc::clone(&catalog);
        let offline = Arc::new(AtomicBool::new(false));
        let worker_offline = Arc::clone(&offline);
        std::thread::Builder::new()
            .name(format!("meldritch-preset-{}", spec.descriptor.name))
            .spawn(move || {
                while let Ok(request) = request_rx.recv() {
                    match request {
                        CatalogRequest::Retire(instance) => drop(instance),
                        CatalogRequest::Load {
                            mut index,
                            mut generation,
                        } => {
                            // Coalesce queued selections before doing expensive work.
                            for queued in request_rx.try_iter() {
                                match queued {
                                    CatalogRequest::Load {
                                        index: next,
                                        generation: next_generation,
                                    } => {
                                        index = next;
                                        generation = next_generation;
                                    }
                                    CatalogRequest::Retire(instance) => drop(instance),
                                }
                            }
                            let result = (|| {
                                let patch = worker_catalog
                                    .patches
                                    .get(index)
                                    .ok_or_else(|| format!("unknown preset index {index}"))?;
                                let bytes = std::fs::read(&patch.file).map_err(|error| {
                                    format!("failed to read preset '{}': {error}", patch.id)
                                })?;
                                let document = PluginState::from_bytes(&bytes)?;
                                document.validate(&base.descriptor)?;
                                let mut selected = base.clone();
                                selected.state = Some(document.state);
                                let mut instance = Box::new(Instance::new(&selected, sample_rate)?);
                                if worker_offline.load(Ordering::Acquire) {
                                    instance.set_offline();
                                }
                                Ok(instance)
                            })();
                            if result_tx
                                .send(CatalogResult {
                                    index,
                                    generation,
                                    instance: result,
                                })
                                .is_err()
                            {
                                break;
                            }
                        }
                    }
                }
            })
            .map_err(|error| format!("failed to start preset loader: {error}"))?;
        let seen_request_epoch = catalog.status.request_epoch();
        Ok(Self {
            current,
            desired: catalog.initial,
            catalog,
            requests: request_tx,
            results: result_rx,
            submitted: None,
            failed_selection: None,
            // At most the bounded request/result capacity can await retirement.
            retired: Vec::with_capacity(8),
            failure_update: None,
            offline,
            seen_request_epoch,
        })
    }

    fn update_selection(&mut self, patch_selector: &[f64]) {
        self.try_publish_failure();
        let request_epoch = self.catalog.status.request_epoch();
        if request_epoch != self.seen_request_epoch {
            self.seen_request_epoch = request_epoch;
            self.failed_selection = None;
        }
        let requested = patch_selector
            .first()
            .copied()
            .filter(|value| value.is_finite())
            .map(|value| {
                value
                    .round()
                    .clamp(0.0, self.catalog.patches.len().saturating_sub(1) as f64)
                    as usize
            })
            .unwrap_or(self.desired);
        if requested != self.desired {
            self.desired = requested;
            self.submitted = None;
            self.failed_selection = None;
            self.catalog.status.request_realtime(requested);
            self.failure_update = Some(None);
        }
        if self.desired != self.catalog.status.active()
            && self.submitted.is_none()
            && self.failed_selection != Some(self.desired)
        {
            let generation = self
                .catalog
                .status
                .generation
                .fetch_add(1, Ordering::AcqRel)
                + 1;
            match self.requests.try_send(CatalogRequest::Load {
                index: self.desired,
                generation,
            }) {
                Ok(()) => self.submitted = Some((self.desired, generation)),
                Err(TrySendError::Full(_)) => {}
                Err(TrySendError::Disconnected(_)) => {
                    self.catalog.status.pending.store(false, Ordering::Release)
                }
            }
        }
        loop {
            match self.results.try_recv() {
                Ok(result) => {
                    if self.submitted == Some((result.index, result.generation)) {
                        self.submitted = None;
                    }
                    match result.instance {
                        Ok(instance) if result.index == self.desired => {
                            let old = std::mem::replace(&mut self.current, instance);
                            self.retired.push(old);
                            self.catalog
                                .status
                                .active
                                .store(result.index, Ordering::Release);
                            self.catalog.status.pending.store(false, Ordering::Release);
                            self.failed_selection = None;
                            self.failure_update = Some(None);
                        }
                        Ok(instance) => self.retired.push(instance),
                        Err(error) if result.index == self.desired => {
                            self.catalog.status.pending.store(false, Ordering::Release);
                            self.failure_update = Some(Some((result.generation, error)));
                            self.failed_selection = Some(result.index);
                        }
                        Err(_) => {}
                    }
                }
                Err(TryRecvError::Empty | TryRecvError::Disconnected) => break,
            }
        }
        while let Some(instance) = self.retired.pop() {
            match self.requests.try_send(CatalogRequest::Retire(instance)) {
                Ok(()) => {}
                Err(
                    TrySendError::Full(CatalogRequest::Retire(instance))
                    | TrySendError::Disconnected(CatalogRequest::Retire(instance)),
                ) => {
                    self.retired.push(instance);
                    break;
                }
                Err(_) => unreachable!(),
            }
        }
        self.try_publish_failure();
    }

    fn try_publish_failure(&mut self) {
        let Some(update) = self.failure_update.take() else {
            return;
        };
        match self.catalog.status.failure.try_lock() {
            Ok(mut failure) => *failure = update,
            Err(_) => self.failure_update = Some(update),
        }
    }

    pub fn process(
        &mut self,
        note_inputs: [&[f64]; 5],
        parameter_values: &[f64],
        patch_selector: &[f64],
        output: &mut [f64],
    ) {
        self.update_selection(patch_selector);
        self.current.process(note_inputs, parameter_values, output);
    }

    pub fn set_offline(&mut self) {
        self.offline.store(true, Ordering::Release);
        self.current.set_offline();
    }
    pub fn failed(&self) -> bool {
        self.current.failed()
    }
    pub fn offline_fallback(&self) -> bool {
        self.current.offline_fallback()
    }
    pub fn save_state(&self) -> Result<Vec<u8>, String> {
        self.current.save_state()
    }
    pub fn latency_samples(&self) -> u32 {
        self.current.latency_samples()
    }
    pub fn status(&self) -> Arc<PresetStatus> {
        Arc::clone(&self.catalog.status)
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
            patch_index: None,
            preset_catalog: None,
            managed_presets: None,
            editor: None,
        };
        assert_eq!(Spec::parameter_name(42), "parameter_42");
        assert_eq!(spec.parameter_id("parameter_42"), Some(42));
        assert_eq!(spec.parameter_id("parameter_7"), None);
    }

    #[test]
    fn catalog_keeps_stable_ids_when_display_names_collide() {
        let patches = vec![
            CatalogPatch {
                id: "bank_a_same".into(),
                name: "Same".into(),
                bank: "A".into(),
                category: "Lead".into(),
                file: "a.state".into(),
            },
            CatalogPatch {
                id: "bank_b_same".into(),
                name: "Same".into(),
                bank: "B".into(),
                category: "Pad".into(),
                file: "b.state".into(),
            },
        ];
        let catalog = PresetCatalog::new(patches, 0).expect("duplicate display names are valid");
        assert_ne!(catalog.patches[0].id, catalog.patches[1].id);
        catalog.status.request(1);
        assert_eq!(catalog.status.active(), 0);
        assert_eq!(catalog.status.requested(), 1);
        assert!(catalog.status.pending());
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
