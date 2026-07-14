use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SignalType {
    Audio,
    Pitch,
    Gate,
    Control,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModuleKind {
    Oscillator,
    Adsr,
    Vca,
    LowPass,
    TempoDelay,
    AudioOutput,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PatchInput {
    id: String,
    signal: SignalType,
}

impl PatchInput {
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    #[must_use]
    pub const fn signal(&self) -> SignalType {
        self.signal
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ModuleDefinition {
    id: String,
    kind: ModuleKind,
    waveform: Option<String>,
    frequency_hz: Option<f64>,
    channels: Option<u16>,
    cutoff_hz: Option<f64>,
    resonance: Option<f64>,
    time: Option<String>,
    feedback: Option<f64>,
    mix: Option<f64>,
    attack: Option<f64>,
    decay: Option<f64>,
    sustain: Option<f64>,
    release: Option<f64>,
}

impl ModuleDefinition {
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    #[must_use]
    pub const fn kind(&self) -> ModuleKind {
        self.kind
    }

    #[must_use]
    pub fn waveform(&self) -> Option<&str> {
        self.waveform.as_deref()
    }

    #[must_use]
    pub const fn frequency_hz(&self) -> Option<f64> {
        self.frequency_hz
    }

    #[must_use]
    pub const fn channels(&self) -> Option<u16> {
        self.channels
    }

    #[must_use]
    pub const fn cutoff_hz(&self) -> Option<f64> {
        self.cutoff_hz
    }

    #[must_use]
    pub const fn resonance(&self) -> Option<f64> {
        self.resonance
    }

    #[must_use]
    pub fn time(&self) -> Option<&str> {
        self.time.as_deref()
    }

    #[must_use]
    pub const fn feedback(&self) -> Option<f64> {
        self.feedback
    }

    #[must_use]
    pub const fn mix(&self) -> Option<f64> {
        self.mix
    }

    #[must_use]
    pub const fn attack(&self) -> Option<f64> {
        self.attack
    }

    #[must_use]
    pub const fn decay(&self) -> Option<f64> {
        self.decay
    }

    #[must_use]
    pub const fn sustain(&self) -> Option<f64> {
        self.sustain
    }

    #[must_use]
    pub const fn release(&self) -> Option<f64> {
        self.release
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CableDefinition {
    from: String,
    to: String,
    signal: SignalType,
}

impl CableDefinition {
    #[must_use]
    pub fn from(&self) -> &str {
        &self.from
    }

    #[must_use]
    pub fn to(&self) -> &str {
        &self.to
    }

    #[must_use]
    pub const fn signal(&self) -> SignalType {
        self.signal
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SynthDefinition {
    id: String,
    polyphony: u16,
    inputs: Vec<PatchInput>,
    modules: Vec<ModuleDefinition>,
    cables: Vec<CableDefinition>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DspDefinition {
    id: String,
    inputs: Vec<PatchInput>,
    modules: Vec<ModuleDefinition>,
    cables: Vec<CableDefinition>,
}

impl DspDefinition {
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    #[must_use]
    pub fn inputs(&self) -> &[PatchInput] {
        &self.inputs
    }

    #[must_use]
    pub fn modules(&self) -> &[ModuleDefinition] {
        &self.modules
    }

    #[must_use]
    pub fn cables(&self) -> &[CableDefinition] {
        &self.cables
    }
}

impl SynthDefinition {
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    #[must_use]
    pub const fn polyphony(&self) -> u16 {
        self.polyphony
    }

    #[must_use]
    pub fn inputs(&self) -> &[PatchInput] {
        &self.inputs
    }

    #[must_use]
    pub fn modules(&self) -> &[ModuleDefinition] {
        &self.modules
    }

    #[must_use]
    pub fn cables(&self) -> &[CableDefinition] {
        &self.cables
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrackDefinition {
    id: String,
    synth_path: PathBuf,
    synth_id: String,
    pattern_ids: Vec<String>,
    initial_pattern: Option<String>,
    parameter_pattern_ids: Vec<String>,
    dsp_ids: Vec<String>,
}

impl TrackDefinition {
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    #[must_use]
    pub fn synth_path(&self) -> &Path {
        &self.synth_path
    }

    #[must_use]
    pub fn synth_id(&self) -> &str {
        &self.synth_id
    }

    #[must_use]
    pub fn pattern_ids(&self) -> &[String] {
        &self.pattern_ids
    }

    #[must_use]
    pub fn initial_pattern(&self) -> Option<&str> {
        self.initial_pattern.as_deref()
    }

    #[must_use]
    pub fn parameter_pattern_ids(&self) -> &[String] {
        &self.parameter_pattern_ids
    }

    #[must_use]
    pub fn dsp_ids(&self) -> &[String] {
        &self.dsp_ids
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct NoteEventDefinition {
    start_frame: u64,
    duration_frames: u64,
    note: u8,
    velocity: f64,
}

impl NoteEventDefinition {
    #[must_use]
    pub const fn start_frame(&self) -> u64 {
        self.start_frame
    }

    #[must_use]
    pub const fn duration_frames(&self) -> u64 {
        self.duration_frames
    }

    #[must_use]
    pub const fn note(&self) -> u8 {
        self.note
    }

    #[must_use]
    pub const fn velocity(&self) -> f64 {
        self.velocity
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct NotePatternDefinition {
    id: String,
    length_frames: u64,
    looped: bool,
    events: Vec<NoteEventDefinition>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ParameterInterpolation {
    Step,
    Linear,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ParameterOwner {
    Synth,
    Dsp,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParameterTargetDefinition {
    owner: ParameterOwner,
    definition_id: String,
    module_id: String,
    parameter: String,
}

impl ParameterTargetDefinition {
    #[must_use]
    pub const fn owner(&self) -> &ParameterOwner {
        &self.owner
    }

    #[must_use]
    pub fn definition_id(&self) -> &str {
        &self.definition_id
    }

    #[must_use]
    pub fn module_id(&self) -> &str {
        &self.module_id
    }

    #[must_use]
    pub fn parameter(&self) -> &str {
        &self.parameter
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ParameterPointDefinition {
    frame: u64,
    value: f64,
}

impl ParameterPointDefinition {
    #[must_use]
    pub const fn frame(&self) -> u64 {
        self.frame
    }

    #[must_use]
    pub const fn value(&self) -> f64 {
        self.value
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ParameterLaneDefinition {
    target: ParameterTargetDefinition,
    interpolation: ParameterInterpolation,
    points: Vec<ParameterPointDefinition>,
}

impl ParameterLaneDefinition {
    #[must_use]
    pub const fn target(&self) -> &ParameterTargetDefinition {
        &self.target
    }

    #[must_use]
    pub const fn interpolation(&self) -> ParameterInterpolation {
        self.interpolation
    }

    #[must_use]
    pub fn points(&self) -> &[ParameterPointDefinition] {
        &self.points
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ParameterPatternDefinition {
    id: String,
    length_frames: u64,
    looped: bool,
    lanes: Vec<ParameterLaneDefinition>,
}

impl ParameterPatternDefinition {
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    #[must_use]
    pub const fn length_frames(&self) -> u64 {
        self.length_frames
    }

    #[must_use]
    pub const fn is_looped(&self) -> bool {
        self.looped
    }

    #[must_use]
    pub fn lanes(&self) -> &[ParameterLaneDefinition] {
        &self.lanes
    }
}

impl NotePatternDefinition {
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    #[must_use]
    pub const fn length_frames(&self) -> u64 {
        self.length_frames
    }

    #[must_use]
    pub const fn is_looped(&self) -> bool {
        self.looped
    }

    #[must_use]
    pub fn events(&self) -> &[NoteEventDefinition] {
        &self.events
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PerformanceDefinition {
    id: String,
    title: String,
    bpm: f64,
    sample_rate: u32,
    midi_devices: Vec<MidiDeviceDefinition>,
    tracks: Vec<TrackDefinition>,
    lanes: Vec<PerformanceLaneDefinition>,
    pages: Vec<PerformancePageDefinition>,
    controls: Vec<CuratedControlDefinition>,
    actions: Vec<ActionControlDefinition>,
}

impl PerformanceDefinition {
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    #[must_use]
    pub fn title(&self) -> &str {
        &self.title
    }

    #[must_use]
    pub const fn bpm(&self) -> f64 {
        self.bpm
    }

    #[must_use]
    pub const fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    #[must_use]
    pub fn midi_devices(&self) -> &[MidiDeviceDefinition] {
        &self.midi_devices
    }

    #[must_use]
    pub fn tracks(&self) -> &[TrackDefinition] {
        &self.tracks
    }

    #[must_use]
    pub fn lanes(&self) -> &[PerformanceLaneDefinition] {
        &self.lanes
    }

    #[must_use]
    pub fn pages(&self) -> &[PerformancePageDefinition] {
        &self.pages
    }

    #[must_use]
    pub fn controls(&self) -> &[CuratedControlDefinition] {
        &self.controls
    }

    #[must_use]
    pub fn actions(&self) -> &[ActionControlDefinition] {
        &self.actions
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MidiDeviceDefinition {
    id: String,
    name_contains: String,
}

impl MidiDeviceDefinition {
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    #[must_use]
    pub fn name_contains(&self) -> &str {
        &self.name_contains
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PerformanceLaneDefinition {
    id: String,
    label: String,
    role: String,
    track_id: Option<String>,
    variation_ids: Vec<String>,
}

impl PerformanceLaneDefinition {
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    #[must_use]
    pub fn role(&self) -> &str {
        &self.role
    }

    #[must_use]
    pub fn track_id(&self) -> Option<&str> {
        self.track_id.as_deref()
    }

    #[must_use]
    pub fn variation_ids(&self) -> &[String] {
        &self.variation_ids
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PerformancePageDefinition {
    id: String,
    label: String,
    strips: Vec<PerformancePageStripDefinition>,
}

impl PerformancePageDefinition {
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    #[must_use]
    pub fn strips(&self) -> &[PerformancePageStripDefinition] {
        &self.strips
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PerformancePageStripDefinition {
    strip: u8,
    lane_id: String,
    control_ids: Vec<String>,
}

impl PerformancePageStripDefinition {
    #[must_use]
    pub const fn strip(&self) -> u8 {
        self.strip
    }

    #[must_use]
    pub fn lane_id(&self) -> &str {
        &self.lane_id
    }

    #[must_use]
    pub fn control_ids(&self) -> &[String] {
        &self.control_ids
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct CuratedControlDefinition {
    id: String,
    label: String,
    target: ParameterTargetDefinition,
    minimum: f64,
    maximum: f64,
    step: f64,
    binding: String,
    bindings: Vec<ControlBindingDefinition>,
}

impl CuratedControlDefinition {
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    #[must_use]
    pub const fn target(&self) -> &ParameterTargetDefinition {
        &self.target
    }

    #[must_use]
    pub const fn range(&self) -> (f64, f64) {
        (self.minimum, self.maximum)
    }

    #[must_use]
    pub const fn step(&self) -> f64 {
        self.step
    }

    #[must_use]
    pub fn binding(&self) -> &str {
        &self.binding
    }

    #[must_use]
    pub fn bindings(&self) -> &[ControlBindingDefinition] {
        &self.bindings
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ControlBindingAction {
    Absolute,
    Centered { center: f64 },
    Overdrive { normal: f64, normal_midi: u8 },
    Decrement,
    Increment,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ControlBindingDefinition {
    Key {
        key: String,
    },
    MidiCc {
        device: String,
        page: Option<String>,
        channel: Option<u8>,
        cc: u8,
        action: ControlBindingAction,
    },
    MidiNote {
        device: String,
        channel: Option<u8>,
        note: u8,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct ActionControlDefinition {
    id: String,
    label: String,
    action: PerformanceActionDefinition,
    bindings: Vec<ControlBindingDefinition>,
}

impl ActionControlDefinition {
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    #[must_use]
    pub const fn action(&self) -> &PerformanceActionDefinition {
        &self.action
    }

    #[must_use]
    pub fn bindings(&self) -> &[ControlBindingDefinition] {
        &self.bindings
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PerformanceActionDefinition {
    TogglePlayback,
    Stop,
    Rewind,
    ToggleCockpitMode,
    QueueNextScene,
    QueuePhrase { scene: u64 },
    QueuePhraseVariation { scene: u64, variation: usize },
    SelectPage { page: String },
    ToggleTrackMute,
    TriggerFill,
    CancelPerformance,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ValidatedSong {
    root: PathBuf,
    performance: PerformanceDefinition,
    synths: BTreeMap<String, SynthDefinition>,
    note_patterns: BTreeMap<String, NotePatternDefinition>,
    parameter_patterns: BTreeMap<String, ParameterPatternDefinition>,
    dsps: BTreeMap<String, DspDefinition>,
    fingerprint: u64,
}

impl ValidatedSong {
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    #[must_use]
    pub const fn performance(&self) -> &PerformanceDefinition {
        &self.performance
    }

    #[must_use]
    pub fn synths(&self) -> &BTreeMap<String, SynthDefinition> {
        &self.synths
    }

    #[must_use]
    pub fn note_patterns(&self) -> &BTreeMap<String, NotePatternDefinition> {
        &self.note_patterns
    }

    #[must_use]
    pub fn parameter_patterns(&self) -> &BTreeMap<String, ParameterPatternDefinition> {
        &self.parameter_patterns
    }

    #[must_use]
    pub fn dsps(&self) -> &BTreeMap<String, DspDefinition> {
        &self.dsps
    }

    #[must_use]
    pub const fn fingerprint(&self) -> u64 {
        self.fingerprint
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SongDiagnostic {
    path: PathBuf,
    message: String,
}

impl SongDiagnostic {
    fn new(path: impl Into<PathBuf>, message: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            message: message.into(),
        }
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SongLoadError {
    diagnostics: Vec<SongDiagnostic>,
}

impl SongLoadError {
    fn one(path: impl Into<PathBuf>, message: impl Into<String>) -> Self {
        Self {
            diagnostics: vec![SongDiagnostic::new(path, message)],
        }
    }

    #[must_use]
    pub fn diagnostics(&self) -> &[SongDiagnostic] {
        &self.diagnostics
    }
}

impl fmt::Display for SongLoadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, diagnostic) in self.diagnostics.iter().enumerate() {
            if index > 0 {
                writeln!(formatter)?;
            }
            write!(
                formatter,
                "{}: {}",
                diagnostic.path.display(),
                diagnostic.message
            )?;
        }
        Ok(())
    }
}

impl std::error::Error for SongLoadError {}

#[derive(Debug, Deserialize)]
struct RawHeader {
    kind: String,
    version: u32,
}

#[derive(Debug, Deserialize)]
struct RawPerformanceFile {
    meldritch: RawHeader,
    performance: RawPerformance,
    #[serde(default)]
    midi_devices: Vec<RawMidiDevice>,
    #[serde(default)]
    tracks: Vec<RawTrack>,
    #[serde(default)]
    lanes: Vec<RawPerformanceLane>,
    #[serde(default)]
    pages: Vec<RawPerformancePage>,
    #[serde(default)]
    controls: Vec<RawControl>,
    #[serde(default)]
    actions: Vec<RawActionControl>,
}

#[derive(Debug, Deserialize)]
struct RawPerformance {
    id: String,
    title: String,
    bpm: f64,
    sample_rate: u32,
}

#[derive(Debug, Deserialize)]
struct RawTrack {
    id: String,
    synth: PathBuf,
    #[serde(default)]
    patterns: Vec<PathBuf>,
    initial_pattern: Option<String>,
    #[serde(default)]
    parameter_patterns: Vec<String>,
    #[serde(default)]
    dsp: Vec<PathBuf>,
}

#[derive(Debug, Deserialize)]
struct RawMidiDevice {
    id: String,
    name_contains: String,
}

#[derive(Debug, Deserialize)]
struct RawPerformanceLane {
    id: String,
    label: String,
    role: String,
    #[serde(default)]
    track: Option<String>,
    #[serde(default)]
    variations: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct RawPerformancePage {
    id: String,
    label: String,
    #[serde(default)]
    strips: Vec<RawPerformancePageStrip>,
}

#[derive(Debug, Deserialize)]
struct RawPerformancePageStrip {
    strip: u8,
    lane: String,
    #[serde(default)]
    controls: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct RawControl {
    id: String,
    label: String,
    target: String,
    range: [f64; 2],
    step: f64,
    #[serde(default)]
    binding: String,
    #[serde(default)]
    bindings: Vec<RawControlBinding>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum RawControlBinding {
    Key {
        key: String,
    },
    MidiCc {
        device: String,
        #[serde(default)]
        page: Option<String>,
        #[serde(default)]
        channel: Option<u8>,
        cc: u8,
        action: RawControlBindingAction,
        #[serde(default)]
        center: Option<f64>,
        #[serde(default)]
        normal: Option<f64>,
        #[serde(default)]
        normal_midi: Option<u8>,
    },
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum RawControlBindingAction {
    Absolute,
    Centered,
    Overdrive,
    Decrement,
    Increment,
}

#[derive(Debug, Deserialize)]
struct RawActionControl {
    id: String,
    label: String,
    action: RawPerformanceAction,
    #[serde(default)]
    scene: Option<u64>,
    #[serde(default)]
    variation: Option<usize>,
    #[serde(default)]
    page: Option<String>,
    #[serde(default)]
    bindings: Vec<RawActionBinding>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum RawPerformanceAction {
    TogglePlayback,
    Stop,
    Rewind,
    ToggleCockpitMode,
    QueueNextScene,
    QueuePhrase,
    QueuePhraseVariation,
    SelectPage,
    ToggleTrackMute,
    TriggerFill,
    CancelPerformance,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum RawActionBinding {
    MidiCc {
        device: String,
        #[serde(default)]
        channel: Option<u8>,
        cc: u8,
    },
    MidiNote {
        device: String,
        #[serde(default)]
        channel: Option<u8>,
        note: u8,
    },
}

#[derive(Debug, Deserialize)]
struct RawSynthFile {
    meldritch: RawHeader,
    synth: RawSynth,
    #[serde(default)]
    inputs: Vec<RawInput>,
    #[serde(default)]
    modules: Vec<RawModule>,
    #[serde(default)]
    cables: Vec<RawCable>,
}

#[derive(Debug, Deserialize)]
struct RawInput {
    id: String,
    #[serde(rename = "type")]
    signal: String,
}

#[derive(Debug, Deserialize)]
struct RawSynth {
    id: String,
    polyphony: u16,
}

#[derive(Debug, Deserialize)]
struct RawModule {
    id: String,
    #[serde(rename = "type")]
    kind: String,
    waveform: Option<String>,
    frequency_hz: Option<f64>,
    channels: Option<u16>,
    cutoff_hz: Option<f64>,
    resonance: Option<f64>,
    time: Option<String>,
    feedback: Option<f64>,
    mix: Option<f64>,
    attack: Option<f64>,
    decay: Option<f64>,
    sustain: Option<f64>,
    release: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct RawCable {
    from: String,
    to: String,
}

#[derive(Debug, Deserialize)]
struct RawDspFile {
    meldritch: RawHeader,
    dsp: RawDsp,
    #[serde(default)]
    inputs: Vec<RawInput>,
    #[serde(default)]
    modules: Vec<RawModule>,
    #[serde(default)]
    cables: Vec<RawCable>,
}

#[derive(Debug, Deserialize)]
struct RawDsp {
    id: String,
}

#[derive(Debug, Deserialize)]
struct RawPatternFile {
    meldritch: RawHeader,
    pattern: RawPatternMeta,
    #[serde(default)]
    events: Vec<RawNoteEvent>,
}

#[derive(Debug, Deserialize)]
struct RawPatternMeta {
    id: String,
    #[serde(rename = "type")]
    kind: String,
    length: String,
    resolution: Option<String>,
    #[serde(rename = "loop")]
    looped: bool,
}

#[derive(Debug, Deserialize)]
struct RawNoteEvent {
    at: String,
    note: String,
    duration: String,
    velocity: f64,
}

#[derive(Debug, Deserialize)]
struct RawParameterPatternFile {
    meldritch: RawHeader,
    pattern: RawPatternMeta,
    #[serde(default)]
    lanes: Vec<RawParameterLane>,
}

#[derive(Debug, Deserialize)]
struct RawParameterLane {
    target: String,
    interpolation: String,
    #[serde(default)]
    points: Vec<RawParameterPoint>,
}

#[derive(Debug, Deserialize)]
struct RawParameterPoint {
    at: String,
    value: f64,
}

#[derive(Clone, Copy)]
enum PortDirection {
    Input,
    Output,
}

pub fn load_song_directory(path: impl AsRef<Path>) -> Result<ValidatedSong, SongLoadError> {
    let requested_root = path.as_ref();
    let root = requested_root.canonicalize().map_err(|error| {
        SongLoadError::one(
            requested_root,
            format!("cannot open song directory: {error}"),
        )
    })?;
    if !root.is_dir() {
        return Err(SongLoadError::one(&root, "song path is not a directory"));
    }

    let entry = root.join("main.mlperformance");
    let raw_performance: RawPerformanceFile = read_toml(&entry)?;
    validate_header(&entry, &raw_performance.meldritch, "performance")?;
    if raw_performance.performance.id.trim().is_empty() {
        return Err(SongLoadError::one(
            &entry,
            "performance.id must not be empty",
        ));
    }
    if !raw_performance.performance.bpm.is_finite() || raw_performance.performance.bpm <= 0.0 {
        return Err(SongLoadError::one(
            &entry,
            "performance.bpm must be finite and greater than zero",
        ));
    }
    if raw_performance.performance.sample_rate == 0 {
        return Err(SongLoadError::one(
            &entry,
            "performance.sample_rate must be greater than zero",
        ));
    }

    let mut track_ids = BTreeSet::new();
    let mut tracks = Vec::with_capacity(raw_performance.tracks.len());
    let mut synths = BTreeMap::new();
    let mut note_patterns = BTreeMap::new();
    let mut parameter_patterns = BTreeMap::new();
    let mut dsps = BTreeMap::new();
    for raw_track in raw_performance.tracks {
        if !track_ids.insert(raw_track.id.clone()) {
            return Err(SongLoadError::one(
                &entry,
                format!("track id '{}' is declared more than once", raw_track.id),
            ));
        }
        let synth_path = resolve_song_reference(&root, &entry, &raw_track.synth)?;
        let synth = load_synth(&synth_path)?;
        if let Some(existing) = synths.get(synth.id()) {
            if existing != &synth {
                return Err(SongLoadError::one(
                    &synth_path,
                    format!(
                        "synth id '{}' resolves to different definitions",
                        synth.id()
                    ),
                ));
            }
        } else {
            synths.insert(synth.id.clone(), synth.clone());
        }
        let mut track_dsps = BTreeMap::new();
        let mut dsp_ids = Vec::with_capacity(raw_track.dsp.len());
        for reference in &raw_track.dsp {
            let dsp_path = resolve_song_reference(&root, &entry, reference)?;
            let dsp = load_dsp(&dsp_path)?;
            if let Some(existing) = dsps.get(dsp.id()) {
                if existing != &dsp {
                    return Err(SongLoadError::one(
                        &dsp_path,
                        format!("DSP id '{}' resolves to different definitions", dsp.id()),
                    ));
                }
            } else {
                dsps.insert(dsp.id.clone(), dsp.clone());
            }
            track_dsps.insert(dsp.id.clone(), dsp.clone());
            dsp_ids.push(dsp.id);
        }
        let mut pattern_ids = Vec::with_capacity(raw_track.patterns.len());
        for reference in &raw_track.patterns {
            let pattern_path = resolve_song_reference(&root, &entry, reference)?;
            let kind = pattern_kind(&pattern_path)?;
            match kind.as_str() {
                "notes" => {
                    let pattern = load_note_pattern(
                        &pattern_path,
                        raw_performance.performance.bpm,
                        raw_performance.performance.sample_rate,
                    )?;
                    if let Some(existing) = note_patterns.get(pattern.id()) {
                        if existing != &pattern {
                            return Err(SongLoadError::one(
                                &pattern_path,
                                format!(
                                    "pattern id '{}' resolves to different definitions",
                                    pattern.id()
                                ),
                            ));
                        }
                    } else {
                        note_patterns.insert(pattern.id.clone(), pattern.clone());
                    }
                    pattern_ids.push(pattern.id);
                }
                "parameters" => {
                    let pattern = load_parameter_pattern(
                        &pattern_path,
                        raw_performance.performance.bpm,
                        raw_performance.performance.sample_rate,
                        &synth,
                        &track_dsps,
                    )?;
                    if let Some(existing) = parameter_patterns.get(pattern.id()) {
                        if existing != &pattern {
                            return Err(SongLoadError::one(
                                &pattern_path,
                                format!(
                                    "pattern id '{}' resolves to different definitions",
                                    pattern.id()
                                ),
                            ));
                        }
                    } else {
                        parameter_patterns.insert(pattern.id.clone(), pattern.clone());
                    }
                    pattern_ids.push(pattern.id);
                }
                kind => {
                    return Err(SongLoadError::one(
                        &pattern_path,
                        format!("pattern has unsupported type '{kind}'"),
                    ));
                }
            }
        }
        if let Some(initial) = &raw_track.initial_pattern {
            if !pattern_ids.contains(initial) || !note_patterns.contains_key(initial) {
                return Err(SongLoadError::one(
                    &entry,
                    format!(
                        "track '{}' initial_pattern '{}' is not among its referenced patterns",
                        raw_track.id, initial
                    ),
                ));
            }
        }
        for parameter_pattern in &raw_track.parameter_patterns {
            if !pattern_ids.contains(parameter_pattern)
                || !parameter_patterns.contains_key(parameter_pattern)
            {
                return Err(SongLoadError::one(
                    &entry,
                    format!(
                        "track '{}' parameter pattern '{}' is not a referenced parameter pattern",
                        raw_track.id, parameter_pattern
                    ),
                ));
            }
        }
        tracks.push(TrackDefinition {
            id: raw_track.id,
            synth_path: synth_path
                .strip_prefix(&root)
                .expect("resolved song reference stays inside root")
                .to_path_buf(),
            synth_id: synth.id,
            pattern_ids,
            initial_pattern: raw_track.initial_pattern,
            parameter_pattern_ids: raw_track.parameter_patterns,
            dsp_ids,
        });
    }

    let mut midi_device_ids = BTreeSet::new();
    let mut midi_devices = Vec::with_capacity(raw_performance.midi_devices.len());
    for raw in raw_performance.midi_devices {
        if !midi_device_ids.insert(raw.id.clone()) {
            return Err(SongLoadError::one(
                &entry,
                format!("MIDI device id '{}' is declared more than once", raw.id),
            ));
        }
        if raw.name_contains.trim().is_empty() {
            return Err(SongLoadError::one(
                &entry,
                format!("MIDI device '{}' has an empty name_contains", raw.id),
            ));
        }
        midi_devices.push(MidiDeviceDefinition {
            id: raw.id,
            name_contains: raw.name_contains,
        });
    }

    let mut lane_ids = BTreeSet::new();
    let mut lanes = Vec::with_capacity(raw_performance.lanes.len());
    for raw in raw_performance.lanes {
        if raw.id.trim().is_empty() {
            return Err(SongLoadError::one(&entry, "lane id must not be empty"));
        }
        if !lane_ids.insert(raw.id.clone()) {
            return Err(SongLoadError::one(
                &entry,
                format!("lane id '{}' is declared more than once", raw.id),
            ));
        }
        if raw.label.trim().is_empty() {
            return Err(SongLoadError::one(
                &entry,
                format!("lane '{}' has an empty label", raw.id),
            ));
        }
        if raw.role.trim().is_empty() {
            return Err(SongLoadError::one(
                &entry,
                format!("lane '{}' has an empty role", raw.id),
            ));
        }
        if let Some(track) = raw.track.as_deref() {
            if !track_ids.contains(track) {
                return Err(SongLoadError::one(
                    &entry,
                    format!("lane '{}' references unknown track '{}'", raw.id, track),
                ));
            }
            let track_definition = tracks
                .iter()
                .find(|definition| definition.id() == track)
                .expect("track id set was built from track definitions");
            let mut variation_ids = BTreeSet::new();
            for variation in &raw.variations {
                if !variation_ids.insert(variation.as_str()) {
                    return Err(SongLoadError::one(
                        &entry,
                        format!(
                            "lane '{}' variation '{}' is declared more than once",
                            raw.id, variation
                        ),
                    ));
                }
                if !track_definition
                    .pattern_ids()
                    .iter()
                    .any(|id| id == variation)
                {
                    return Err(SongLoadError::one(
                        &entry,
                        format!(
                            "lane '{}' variation '{}' is not among track '{}' patterns",
                            raw.id, variation, track
                        ),
                    ));
                }
            }
        }
        lanes.push(PerformanceLaneDefinition {
            id: raw.id,
            label: raw.label,
            role: raw.role,
            track_id: raw.track,
            variation_ids: raw.variations,
        });
    }

    let mut page_ids = BTreeSet::new();
    let mut pages = Vec::with_capacity(raw_performance.pages.len());
    for raw in raw_performance.pages {
        if raw.id.trim().is_empty() {
            return Err(SongLoadError::one(&entry, "page id must not be empty"));
        }
        if !page_ids.insert(raw.id.clone()) {
            return Err(SongLoadError::one(
                &entry,
                format!("page id '{}' is declared more than once", raw.id),
            ));
        }
        if raw.label.trim().is_empty() {
            return Err(SongLoadError::one(
                &entry,
                format!("page '{}' has an empty label", raw.id),
            ));
        }
        let mut strip_ids = BTreeSet::new();
        let mut strips = Vec::with_capacity(raw.strips.len());
        for strip in raw.strips {
            if !(1..=8).contains(&strip.strip) {
                return Err(SongLoadError::one(
                    &entry,
                    format!(
                        "page '{}' strip {} is outside the LaunchControl strip range 1..=8",
                        raw.id, strip.strip
                    ),
                ));
            }
            if !strip_ids.insert(strip.strip) {
                return Err(SongLoadError::one(
                    &entry,
                    format!(
                        "page '{}' strip {} is declared more than once",
                        raw.id, strip.strip
                    ),
                ));
            }
            if !lane_ids.contains(&strip.lane) {
                return Err(SongLoadError::one(
                    &entry,
                    format!("page '{}' references unknown lane '{}'", raw.id, strip.lane),
                ));
            }
            strips.push(PerformancePageStripDefinition {
                strip: strip.strip,
                lane_id: strip.lane,
                control_ids: strip.controls,
            });
        }
        pages.push(PerformancePageDefinition {
            id: raw.id,
            label: raw.label,
            strips,
        });
    }

    let mut control_ids = BTreeSet::new();
    let mut key_bindings = BTreeSet::new();
    let mut midi_cc_claims: BTreeSet<(String, Option<String>, Option<u8>, u8)> = BTreeSet::new();
    let mut midi_note_claims = BTreeSet::new();
    let mut controls = Vec::with_capacity(raw_performance.controls.len());
    for raw in raw_performance.controls {
        if !control_ids.insert(raw.id.clone()) {
            return Err(SongLoadError::one(
                &entry,
                format!("control id '{}' is declared more than once", raw.id),
            ));
        }
        let mut bindings = Vec::new();
        if !raw.binding.is_empty() {
            if !key_bindings.insert(raw.binding.clone()) {
                return Err(SongLoadError::one(
                    &entry,
                    format!(
                        "control key binding '{}' is declared more than once",
                        raw.binding
                    ),
                ));
            }
            bindings.push(ControlBindingDefinition::Key {
                key: raw.binding.clone(),
            });
        }
        for binding in raw.bindings {
            match binding {
                RawControlBinding::Key { key } => {
                    if key.trim().is_empty() {
                        return Err(SongLoadError::one(
                            &entry,
                            format!("control '{}' has an empty key binding", raw.id),
                        ));
                    }
                    if !key_bindings.insert(key.clone()) {
                        return Err(SongLoadError::one(
                            &entry,
                            format!("control key binding '{key}' is declared more than once"),
                        ));
                    }
                    if raw.binding.is_empty() {
                        bindings.push(ControlBindingDefinition::Key { key });
                    } else {
                        bindings.push(ControlBindingDefinition::Key { key });
                    }
                }
                RawControlBinding::MidiCc {
                    device,
                    page,
                    channel,
                    cc,
                    action,
                    center,
                    normal,
                    normal_midi,
                } => {
                    if !midi_device_ids.contains(&device) {
                        return Err(SongLoadError::one(
                            &entry,
                            format!(
                                "control '{}' references unknown MIDI device '{}'",
                                raw.id, device
                            ),
                        ));
                    }
                    if let Some(channel) = channel {
                        if !(1..=16).contains(&channel) {
                            return Err(SongLoadError::one(
                                &entry,
                                format!(
                                    "control '{}' has MIDI channel {} outside 1..=16",
                                    raw.id, channel
                                ),
                            ));
                        }
                    }
                    if let Some(page) = page.as_deref() {
                        if !page_ids.contains(page) {
                            return Err(SongLoadError::one(
                                &entry,
                                format!("control '{}' references unknown page '{}'", raw.id, page),
                            ));
                        }
                    }
                    let claim_conflicts = midi_cc_claims.iter().any(
                        |(claimed_device, claimed_page, claimed_channel, claimed_cc)| {
                            claimed_device == &device
                                && claimed_channel == &channel
                                && claimed_cc == &cc
                                && (claimed_page.is_none()
                                    || page.is_none()
                                    || claimed_page == &page)
                        },
                    );
                    if claim_conflicts {
                        return Err(SongLoadError::one(
                            &entry,
                            format!(
                                "MIDI CC binding device='{device}' page='{page:?}' channel='{channel:?}' cc={cc} conflicts with another binding"
                            ),
                        ));
                    }
                    midi_cc_claims.insert((device.clone(), page.clone(), channel, cc));
                    let action = match action {
                        RawControlBindingAction::Absolute => ControlBindingAction::Absolute,
                        RawControlBindingAction::Centered => {
                            let center = center.ok_or_else(|| {
                                SongLoadError::one(
                                    &entry,
                                    format!(
                                        "control '{}' centered MIDI binding requires center",
                                        raw.id
                                    ),
                                )
                            })?;
                            if !center.is_finite() || center < raw.range[0] || center > raw.range[1]
                            {
                                return Err(SongLoadError::one(
                                    &entry,
                                    format!(
                                        "control '{}' centered MIDI binding has invalid center",
                                        raw.id
                                    ),
                                ));
                            }
                            ControlBindingAction::Centered { center }
                        }
                        RawControlBindingAction::Overdrive => {
                            let normal = normal.ok_or_else(|| {
                                SongLoadError::one(
                                    &entry,
                                    format!(
                                        "control '{}' overdrive MIDI binding requires normal",
                                        raw.id
                                    ),
                                )
                            })?;
                            let normal_midi = normal_midi.ok_or_else(|| {
                                SongLoadError::one(
                                    &entry,
                                    format!(
                                        "control '{}' overdrive MIDI binding requires normal_midi",
                                        raw.id
                                    ),
                                )
                            })?;
                            if !normal.is_finite()
                                || normal < raw.range[0]
                                || normal > raw.range[1]
                                || !(1..=126).contains(&normal_midi)
                            {
                                return Err(SongLoadError::one(
                                    &entry,
                                    format!(
                                        "control '{}' overdrive MIDI binding has invalid normal or normal_midi",
                                        raw.id
                                    ),
                                ));
                            }
                            ControlBindingAction::Overdrive {
                                normal,
                                normal_midi,
                            }
                        }
                        RawControlBindingAction::Decrement => ControlBindingAction::Decrement,
                        RawControlBindingAction::Increment => ControlBindingAction::Increment,
                    };
                    bindings.push(ControlBindingDefinition::MidiCc {
                        device,
                        page,
                        channel,
                        cc,
                        action,
                    });
                }
            }
        }
        if bindings.is_empty() {
            return Err(SongLoadError::one(
                &entry,
                format!("control '{}' has no input bindings", raw.id),
            ));
        }
        if !raw.range[0].is_finite()
            || !raw.range[1].is_finite()
            || raw.range[0] >= raw.range[1]
            || !raw.step.is_finite()
            || raw.step <= 0.0
        {
            return Err(SongLoadError::one(
                &entry,
                format!("control '{}' has an invalid range or step", raw.id),
            ));
        }
        let target = parse_global_parameter_target(&entry, &raw.target, &synths, &dsps)?;
        validate_parameter_value(&entry, &target, raw.range[0])?;
        validate_parameter_value(&entry, &target, raw.range[1])?;
        controls.push(CuratedControlDefinition {
            id: raw.id,
            label: raw.label,
            target,
            minimum: raw.range[0],
            maximum: raw.range[1],
            step: raw.step,
            binding: raw.binding,
            bindings,
        });
    }
    let declared_control_ids = controls
        .iter()
        .map(|control| control.id())
        .collect::<BTreeSet<_>>();
    for page in &pages {
        for strip in page.strips() {
            let mut strip_control_ids = BTreeSet::new();
            for control_id in strip.control_ids() {
                if !strip_control_ids.insert(control_id.as_str()) {
                    return Err(SongLoadError::one(
                        &entry,
                        format!(
                            "page '{}' strip {} control '{}' is declared more than once",
                            page.id(),
                            strip.strip(),
                            control_id
                        ),
                    ));
                }
                if !declared_control_ids.contains(control_id.as_str()) {
                    return Err(SongLoadError::one(
                        &entry,
                        format!(
                            "page '{}' strip {} references unknown control '{}'",
                            page.id(),
                            strip.strip(),
                            control_id
                        ),
                    ));
                }
            }
        }
    }

    let mut action_ids = BTreeSet::new();
    let mut actions = Vec::with_capacity(raw_performance.actions.len());
    for raw in raw_performance.actions {
        if !action_ids.insert(raw.id.clone()) {
            return Err(SongLoadError::one(
                &entry,
                format!("action id '{}' is declared more than once", raw.id),
            ));
        }
        let action = match raw.action {
            RawPerformanceAction::TogglePlayback => PerformanceActionDefinition::TogglePlayback,
            RawPerformanceAction::Stop => PerformanceActionDefinition::Stop,
            RawPerformanceAction::Rewind => PerformanceActionDefinition::Rewind,
            RawPerformanceAction::ToggleCockpitMode => {
                PerformanceActionDefinition::ToggleCockpitMode
            }
            RawPerformanceAction::QueueNextScene => PerformanceActionDefinition::QueueNextScene,
            RawPerformanceAction::QueuePhrase => PerformanceActionDefinition::QueuePhrase {
                scene: raw.scene.ok_or_else(|| {
                    SongLoadError::one(
                        &entry,
                        format!("action '{}' queue_phrase requires scene", raw.id),
                    )
                })?,
            },
            RawPerformanceAction::QueuePhraseVariation => {
                PerformanceActionDefinition::QueuePhraseVariation {
                    scene: raw.scene.ok_or_else(|| {
                        SongLoadError::one(
                            &entry,
                            format!("action '{}' queue_phrase_variation requires scene", raw.id),
                        )
                    })?,
                    variation: raw.variation.ok_or_else(|| {
                        SongLoadError::one(
                            &entry,
                            format!(
                                "action '{}' queue_phrase_variation requires variation",
                                raw.id
                            ),
                        )
                    })?,
                }
            }
            RawPerformanceAction::SelectPage => {
                let page = raw.page.ok_or_else(|| {
                    SongLoadError::one(
                        &entry,
                        format!("action '{}' select_page requires page", raw.id),
                    )
                })?;
                if !page_ids.contains(&page) {
                    return Err(SongLoadError::one(
                        &entry,
                        format!("action '{}' references unknown page '{}'", raw.id, page),
                    ));
                }
                PerformanceActionDefinition::SelectPage { page }
            }
            RawPerformanceAction::ToggleTrackMute => PerformanceActionDefinition::ToggleTrackMute,
            RawPerformanceAction::TriggerFill => PerformanceActionDefinition::TriggerFill,
            RawPerformanceAction::CancelPerformance => {
                PerformanceActionDefinition::CancelPerformance
            }
        };
        let mut bindings = Vec::with_capacity(raw.bindings.len());
        for binding in raw.bindings {
            let (device, channel) = match &binding {
                RawActionBinding::MidiCc {
                    device, channel, ..
                }
                | RawActionBinding::MidiNote {
                    device, channel, ..
                } => (device, channel),
            };
            if !midi_device_ids.contains(device.as_str()) {
                return Err(SongLoadError::one(
                    &entry,
                    format!(
                        "action '{}' references unknown MIDI device '{}'",
                        raw.id, device
                    ),
                ));
            }
            if let Some(channel) = channel {
                if !(1..=16).contains(channel) {
                    return Err(SongLoadError::one(
                        &entry,
                        format!(
                            "action '{}' has MIDI channel {} outside 1..=16",
                            raw.id, channel
                        ),
                    ));
                }
            }
            match binding {
                RawActionBinding::MidiCc {
                    device,
                    channel,
                    cc,
                } => {
                    let claim_conflicts = midi_cc_claims.iter().any(
                        |(claimed_device, _claimed_page, claimed_channel, claimed_cc)| {
                            claimed_device == &device
                                && claimed_channel == &channel
                                && claimed_cc == &cc
                        },
                    );
                    if claim_conflicts {
                        return Err(SongLoadError::one(
                            &entry,
                            format!(
                                "MIDI CC action binding device='{device}' channel='{channel:?}' cc={cc} conflicts with another binding"
                            ),
                        ));
                    }
                    midi_cc_claims.insert((device.clone(), None, channel, cc));
                    bindings.push(ControlBindingDefinition::MidiCc {
                        device,
                        page: None,
                        channel,
                        cc,
                        action: ControlBindingAction::Increment,
                    });
                }
                RawActionBinding::MidiNote {
                    device,
                    channel,
                    note,
                } => {
                    if !midi_note_claims.insert((device.clone(), channel, note)) {
                        return Err(SongLoadError::one(
                            &entry,
                            format!(
                                "MIDI note binding device='{device}' channel='{channel:?}' note={note} is declared more than once"
                            ),
                        ));
                    }
                    bindings.push(ControlBindingDefinition::MidiNote {
                        device,
                        channel,
                        note,
                    });
                }
            }
        }
        if bindings.is_empty() {
            return Err(SongLoadError::one(
                &entry,
                format!("action '{}' has no input bindings", raw.id),
            ));
        }
        actions.push(ActionControlDefinition {
            id: raw.id,
            label: raw.label,
            action,
            bindings,
        });
    }

    let performance = PerformanceDefinition {
        id: raw_performance.performance.id,
        title: raw_performance.performance.title,
        bpm: raw_performance.performance.bpm,
        sample_rate: raw_performance.performance.sample_rate,
        midi_devices,
        tracks,
        lanes,
        pages,
        controls,
        actions,
    };
    let fingerprint = fingerprint_song(
        &performance,
        &synths,
        &note_patterns,
        &parameter_patterns,
        &dsps,
    );
    Ok(ValidatedSong {
        root,
        performance,
        synths,
        note_patterns,
        parameter_patterns,
        dsps,
        fingerprint,
    })
}

fn load_synth(path: &Path) -> Result<SynthDefinition, SongLoadError> {
    let raw: RawSynthFile = read_toml(path)?;
    validate_header(path, &raw.meldritch, "synth")?;
    if raw.synth.id.trim().is_empty() {
        return Err(SongLoadError::one(path, "synth.id must not be empty"));
    }
    if raw.synth.polyphony == 0 {
        return Err(SongLoadError::one(
            path,
            "synth.polyphony must be greater than zero",
        ));
    }

    let mut input_ids = BTreeSet::new();
    let mut inputs = Vec::with_capacity(raw.inputs.len());
    for input in raw.inputs {
        if !input_ids.insert(input.id.clone()) {
            return Err(SongLoadError::one(
                path,
                format!("input id '{}' is declared more than once", input.id),
            ));
        }
        inputs.push(PatchInput {
            id: input.id,
            signal: parse_signal_type(path, &input.signal)?,
        });
    }

    let mut module_ids = BTreeSet::new();
    let mut modules = Vec::with_capacity(raw.modules.len());
    for module in raw.modules {
        if !module_ids.insert(module.id.clone()) {
            return Err(SongLoadError::one(
                path,
                format!("module id '{}' is declared more than once", module.id),
            ));
        }
        let kind = match module.kind.as_str() {
            "oscillator" => ModuleKind::Oscillator,
            "adsr" => ModuleKind::Adsr,
            "vca" => ModuleKind::Vca,
            "low_pass" => ModuleKind::LowPass,
            "audio_output" => ModuleKind::AudioOutput,
            unknown => {
                return Err(SongLoadError::one(
                    path,
                    format!("module '{}' has unsupported type '{unknown}'", module.id),
                ));
            }
        };
        if module
            .frequency_hz
            .is_some_and(|value| !value.is_finite() || value <= 0.0)
        {
            return Err(SongLoadError::one(
                path,
                format!(
                    "module '{}'.frequency_hz must be finite and positive",
                    module.id
                ),
            ));
        }
        if module
            .cutoff_hz
            .is_some_and(|value| !value.is_finite() || value <= 0.0)
        {
            return Err(SongLoadError::one(
                path,
                format!(
                    "module '{}'.cutoff_hz must be finite and positive",
                    module.id
                ),
            ));
        }
        if module
            .resonance
            .is_some_and(|value| !value.is_finite() || !(0.0..=1.0).contains(&value))
        {
            return Err(SongLoadError::one(
                path,
                format!("module '{}'.resonance must be within 0.0..=1.0", module.id),
            ));
        }
        if kind == ModuleKind::Adsr {
            for (name, value) in [
                ("attack", module.attack),
                ("decay", module.decay),
                ("release", module.release),
            ] {
                let value = value.ok_or_else(|| {
                    SongLoadError::one(path, format!("ADSR module '{}' requires {name}", module.id))
                })?;
                if !value.is_finite() || value < 0.0 {
                    return Err(SongLoadError::one(
                        path,
                        format!(
                            "ADSR module '{}'.{name} must be finite and nonnegative",
                            module.id
                        ),
                    ));
                }
            }
            let sustain = module.sustain.ok_or_else(|| {
                SongLoadError::one(
                    path,
                    format!("ADSR module '{}' requires sustain", module.id),
                )
            })?;
            if !sustain.is_finite() || !(0.0..=1.0).contains(&sustain) {
                return Err(SongLoadError::one(
                    path,
                    format!(
                        "ADSR module '{}'.sustain must be within 0.0..=1.0",
                        module.id
                    ),
                ));
            }
        }
        modules.push(ModuleDefinition {
            id: module.id,
            kind,
            waveform: module.waveform,
            frequency_hz: module.frequency_hz,
            channels: module.channels,
            cutoff_hz: module.cutoff_hz,
            resonance: module.resonance,
            time: module.time,
            feedback: module.feedback,
            mix: module.mix,
            attack: module.attack,
            decay: module.decay,
            sustain: module.sustain,
            release: module.release,
        });
    }

    let module_map = modules
        .iter()
        .map(|module| (module.id.as_str(), module.kind))
        .collect::<BTreeMap<_, _>>();
    let input_map = inputs
        .iter()
        .map(|input| (input.id.as_str(), input.signal))
        .collect::<BTreeMap<_, _>>();
    let mut cables = Vec::with_capacity(raw.cables.len());
    for cable in raw.cables {
        let from_signal = resolve_port(
            path,
            &module_map,
            &input_map,
            &cable.from,
            PortDirection::Output,
        )?;
        let to_signal = resolve_port(
            path,
            &module_map,
            &input_map,
            &cable.to,
            PortDirection::Input,
        )?;
        if from_signal != to_signal {
            return Err(SongLoadError::one(
                path,
                format!(
                    "cable '{} -> {}' connects {from_signal:?} to {to_signal:?}",
                    cable.from, cable.to
                ),
            ));
        }
        cables.push(CableDefinition {
            from: cable.from,
            to: cable.to,
            signal: from_signal,
        });
    }

    Ok(SynthDefinition {
        id: raw.synth.id,
        polyphony: raw.synth.polyphony,
        inputs,
        modules,
        cables,
    })
}

fn load_dsp(path: &Path) -> Result<DspDefinition, SongLoadError> {
    let raw: RawDspFile = read_toml(path)?;
    validate_header(path, &raw.meldritch, "dsp")?;
    if raw.dsp.id.trim().is_empty() {
        return Err(SongLoadError::one(path, "dsp.id must not be empty"));
    }

    let mut input_ids = BTreeSet::new();
    let mut inputs = Vec::with_capacity(raw.inputs.len());
    for input in raw.inputs {
        if !input_ids.insert(input.id.clone()) {
            return Err(SongLoadError::one(
                path,
                format!("input id '{}' is declared more than once", input.id),
            ));
        }
        inputs.push(PatchInput {
            id: input.id,
            signal: parse_signal_type(path, &input.signal)?,
        });
    }

    let mut module_ids = BTreeSet::new();
    let mut modules = Vec::with_capacity(raw.modules.len());
    for module in raw.modules {
        if !module_ids.insert(module.id.clone()) {
            return Err(SongLoadError::one(
                path,
                format!("module id '{}' is declared more than once", module.id),
            ));
        }
        let kind = match module.kind.as_str() {
            "tempo_delay" => ModuleKind::TempoDelay,
            "audio_output" => ModuleKind::AudioOutput,
            unknown => {
                return Err(SongLoadError::one(
                    path,
                    format!(
                        "DSP module '{}' has unsupported type '{unknown}'",
                        module.id
                    ),
                ));
            }
        };
        if kind == ModuleKind::TempoDelay {
            let time = module.time.as_deref().ok_or_else(|| {
                SongLoadError::one(path, format!("delay module '{}' requires time", module.id))
            })?;
            parse_note_fraction(path, time)?;
            for (name, value) in [("feedback", module.feedback), ("mix", module.mix)] {
                let value = value.ok_or_else(|| {
                    SongLoadError::one(
                        path,
                        format!("delay module '{}' requires {name}", module.id),
                    )
                })?;
                if !value.is_finite() || !(0.0..=1.0).contains(&value) {
                    return Err(SongLoadError::one(
                        path,
                        format!(
                            "delay module '{}'.{name} must be within 0.0..=1.0",
                            module.id
                        ),
                    ));
                }
            }
        }
        modules.push(ModuleDefinition {
            id: module.id,
            kind,
            waveform: module.waveform,
            frequency_hz: module.frequency_hz,
            channels: module.channels,
            cutoff_hz: module.cutoff_hz,
            resonance: module.resonance,
            time: module.time,
            feedback: module.feedback,
            mix: module.mix,
            attack: module.attack,
            decay: module.decay,
            sustain: module.sustain,
            release: module.release,
        });
    }

    let module_map = modules
        .iter()
        .map(|module| (module.id.as_str(), module.kind))
        .collect::<BTreeMap<_, _>>();
    let input_map = inputs
        .iter()
        .map(|input| (input.id.as_str(), input.signal))
        .collect::<BTreeMap<_, _>>();
    let mut cables = Vec::with_capacity(raw.cables.len());
    for cable in raw.cables {
        let from_signal = resolve_port(
            path,
            &module_map,
            &input_map,
            &cable.from,
            PortDirection::Output,
        )?;
        let to_signal = resolve_port(
            path,
            &module_map,
            &input_map,
            &cable.to,
            PortDirection::Input,
        )?;
        if from_signal != to_signal {
            return Err(SongLoadError::one(
                path,
                format!(
                    "cable '{} -> {}' connects {from_signal:?} to {to_signal:?}",
                    cable.from, cable.to
                ),
            ));
        }
        cables.push(CableDefinition {
            from: cable.from,
            to: cable.to,
            signal: from_signal,
        });
    }

    Ok(DspDefinition {
        id: raw.dsp.id,
        inputs,
        modules,
        cables,
    })
}

fn resolve_port(
    path: &Path,
    modules: &BTreeMap<&str, ModuleKind>,
    inputs: &BTreeMap<&str, SignalType>,
    endpoint: &str,
    direction: PortDirection,
) -> Result<SignalType, SongLoadError> {
    let (module_id, port) = endpoint.split_once('.').ok_or_else(|| {
        SongLoadError::one(
            path,
            format!("cable endpoint '{endpoint}' must be module.port"),
        )
    })?;
    if module_id == "input" {
        if !matches!(direction, PortDirection::Output) {
            return Err(SongLoadError::one(
                path,
                format!("external input '{endpoint}' can only be a cable source"),
            ));
        }
        return inputs.get(port).copied().ok_or_else(|| {
            SongLoadError::one(
                path,
                format!("cable references unknown external input '{port}'"),
            )
        });
    }
    let kind = modules.get(module_id).ok_or_else(|| {
        SongLoadError::one(
            path,
            format!("cable endpoint '{endpoint}' references unknown module '{module_id}'"),
        )
    })?;
    let signal = match (*kind, direction, port) {
        (ModuleKind::Oscillator, PortDirection::Output, "audio")
        | (ModuleKind::Vca, PortDirection::Output, "audio")
        | (ModuleKind::LowPass, PortDirection::Output, "audio")
        | (ModuleKind::TempoDelay, PortDirection::Output, "audio")
        | (ModuleKind::AudioOutput, PortDirection::Input, "audio") => SignalType::Audio,
        (ModuleKind::Oscillator, PortDirection::Input, "pitch") => SignalType::Pitch,
        (ModuleKind::Adsr, PortDirection::Input, "gate") => SignalType::Gate,
        (ModuleKind::Adsr, PortDirection::Output, "control")
        | (ModuleKind::Vca, PortDirection::Input, "level") => SignalType::Control,
        (ModuleKind::Vca, PortDirection::Input, "audio") => SignalType::Audio,
        (ModuleKind::LowPass, PortDirection::Input, "audio") => SignalType::Audio,
        (ModuleKind::TempoDelay, PortDirection::Input, "audio") => SignalType::Audio,
        _ => {
            return Err(SongLoadError::one(
                path,
                format!("module '{module_id}' has no compatible {direction:?} port '{port}'"),
            ));
        }
    };
    Ok(signal)
}

fn parse_signal_type(path: &Path, value: &str) -> Result<SignalType, SongLoadError> {
    match value {
        "audio" => Ok(SignalType::Audio),
        "pitch" => Ok(SignalType::Pitch),
        "gate" => Ok(SignalType::Gate),
        "control" => Ok(SignalType::Control),
        _ => Err(SongLoadError::one(
            path,
            format!("unsupported signal type '{value}'"),
        )),
    }
}

impl fmt::Debug for PortDirection {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Input => "input",
            Self::Output => "output",
        })
    }
}

fn load_note_pattern(
    path: &Path,
    bpm: f64,
    sample_rate: u32,
) -> Result<NotePatternDefinition, SongLoadError> {
    let raw: RawPatternFile = read_toml(path)?;
    validate_header(path, &raw.meldritch, "pattern")?;
    if raw.pattern.kind != "notes" {
        return Err(SongLoadError::one(
            path,
            format!(
                "pattern '{}' has unsupported type '{}' in this implementation slice",
                raw.pattern.id, raw.pattern.kind
            ),
        ));
    }
    if raw.pattern.id.trim().is_empty() {
        return Err(SongLoadError::one(path, "pattern.id must not be empty"));
    }
    if let Some(resolution) = &raw.pattern.resolution {
        parse_note_fraction(path, resolution)?;
    }
    let beats = parse_pattern_length(path, &raw.pattern.length)?;
    let frames_per_beat = f64::from(sample_rate) * 60.0 / bpm;
    let length_frames = frames_from_beats(path, beats, frames_per_beat)?;
    let mut events = Vec::with_capacity(raw.events.len());
    let mut previous_start = None;
    for raw_event in raw.events {
        let start_beats = parse_position(path, &raw_event.at)?;
        let start_frame = frames_from_beats(path, start_beats, frames_per_beat)?;
        let duration_beats = parse_note_fraction(path, &raw_event.duration)?;
        let duration_frames = frames_from_beats(path, duration_beats, frames_per_beat)?;
        if duration_frames == 0 || start_frame.saturating_add(duration_frames) > length_frames {
            return Err(SongLoadError::one(
                path,
                format!(
                    "event at '{}' with duration '{}' exceeds pattern length '{}'",
                    raw_event.at, raw_event.duration, raw.pattern.length
                ),
            ));
        }
        if previous_start.is_some_and(|previous| start_frame < previous) {
            return Err(SongLoadError::one(
                path,
                "note events must be ordered by musical position",
            ));
        }
        previous_start = Some(start_frame);
        if !raw_event.velocity.is_finite() || !(0.0..=1.0).contains(&raw_event.velocity) {
            return Err(SongLoadError::one(
                path,
                format!("event velocity {} is outside 0.0..=1.0", raw_event.velocity),
            ));
        }
        events.push(NoteEventDefinition {
            start_frame,
            duration_frames,
            note: parse_note(path, &raw_event.note)?,
            velocity: raw_event.velocity,
        });
    }
    Ok(NotePatternDefinition {
        id: raw.pattern.id,
        length_frames,
        looped: raw.pattern.looped,
        events,
    })
}

fn pattern_kind(path: &Path) -> Result<String, SongLoadError> {
    let value: toml::Value = read_toml(path)?;
    value
        .get("pattern")
        .and_then(|pattern| pattern.get("type"))
        .and_then(toml::Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| SongLoadError::one(path, "pattern.type must be a string"))
}

fn load_parameter_pattern(
    path: &Path,
    bpm: f64,
    sample_rate: u32,
    synth: &SynthDefinition,
    dsps: &BTreeMap<String, DspDefinition>,
) -> Result<ParameterPatternDefinition, SongLoadError> {
    let raw: RawParameterPatternFile = read_toml(path)?;
    validate_header(path, &raw.meldritch, "pattern")?;
    if raw.pattern.kind != "parameters" {
        return Err(SongLoadError::one(
            path,
            format!("expected parameter pattern, found '{}'", raw.pattern.kind),
        ));
    }
    if raw.pattern.id.trim().is_empty() {
        return Err(SongLoadError::one(path, "pattern.id must not be empty"));
    }
    let frames_per_beat = f64::from(sample_rate) * 60.0 / bpm;
    let length_frames = frames_from_beats(
        path,
        parse_pattern_length(path, &raw.pattern.length)?,
        frames_per_beat,
    )?;
    let mut lanes = Vec::with_capacity(raw.lanes.len());
    let mut targets = BTreeSet::new();
    for raw_lane in raw.lanes {
        if !targets.insert(raw_lane.target.clone()) {
            return Err(SongLoadError::one(
                path,
                format!(
                    "parameter target '{}' is declared more than once",
                    raw_lane.target
                ),
            ));
        }
        let target = parse_parameter_target(path, &raw_lane.target, synth, dsps)?;
        let interpolation = match raw_lane.interpolation.as_str() {
            "step" => ParameterInterpolation::Step,
            "linear" => ParameterInterpolation::Linear,
            value => {
                return Err(SongLoadError::one(
                    path,
                    format!("unsupported parameter interpolation '{value}'"),
                ));
            }
        };
        if raw_lane.points.is_empty() {
            return Err(SongLoadError::one(
                path,
                format!("parameter lane '{}' has no points", raw_lane.target),
            ));
        }
        let mut points = Vec::with_capacity(raw_lane.points.len());
        let mut previous = None;
        for point in raw_lane.points {
            let frame = frames_from_beats(path, parse_position(path, &point.at)?, frames_per_beat)?;
            if frame > length_frames || previous.is_some_and(|value| frame <= value) {
                return Err(SongLoadError::one(
                    path,
                    format!(
                        "parameter points for '{}' must be strictly ordered within the pattern",
                        raw_lane.target
                    ),
                ));
            }
            validate_parameter_value(path, &target, point.value)?;
            previous = Some(frame);
            points.push(ParameterPointDefinition {
                frame,
                value: point.value,
            });
        }
        lanes.push(ParameterLaneDefinition {
            target,
            interpolation,
            points,
        });
    }
    Ok(ParameterPatternDefinition {
        id: raw.pattern.id,
        length_frames,
        looped: raw.pattern.looped,
        lanes,
    })
}

fn parse_parameter_target(
    path: &Path,
    value: &str,
    synth: &SynthDefinition,
    dsps: &BTreeMap<String, DspDefinition>,
) -> Result<ParameterTargetDefinition, SongLoadError> {
    let (owner, remainder) = if let Some(remainder) = value.strip_prefix("synth:") {
        (ParameterOwner::Synth, remainder)
    } else if let Some(remainder) = value.strip_prefix("dsp:") {
        (ParameterOwner::Dsp, remainder)
    } else {
        return Err(SongLoadError::one(
            path,
            format!("parameter target '{value}' must begin with 'synth:' or 'dsp:'"),
        ));
    };
    let (definition_id, endpoint) = remainder.split_once('/').ok_or_else(|| {
        SongLoadError::one(
            path,
            format!("parameter target '{value}' must be kind:id/module.parameter"),
        )
    })?;
    let (module_id, parameter) = endpoint.split_once('.').ok_or_else(|| {
        SongLoadError::one(
            path,
            format!("parameter target '{value}' must end in module.parameter"),
        )
    })?;
    let modules = match owner {
        ParameterOwner::Synth => {
            if definition_id != synth.id {
                return Err(SongLoadError::one(
                    path,
                    format!(
                        "parameter target '{value}' references unknown synth '{definition_id}'"
                    ),
                ));
            }
            &synth.modules
        }
        ParameterOwner::Dsp => &dsps
            .get(definition_id)
            .ok_or_else(|| {
                SongLoadError::one(
                    path,
                    format!(
                        "parameter target '{value}' references DSP '{definition_id}' not attached to this track"
                    ),
                )
            })?
            .modules,
    };
    let module = modules
        .iter()
        .find(|module| module.id == module_id)
        .ok_or_else(|| {
            SongLoadError::one(
                path,
                format!("parameter target '{value}' references unknown module '{module_id}'"),
            )
        })?;
    if !matches!(
        (module.kind, parameter),
        (ModuleKind::LowPass, "cutoff_hz")
            | (ModuleKind::LowPass, "resonance")
            | (ModuleKind::TempoDelay, "feedback")
            | (ModuleKind::TempoDelay, "mix")
    ) {
        return Err(SongLoadError::one(
            path,
            format!("module '{module_id}' has no automatable parameter '{parameter}'"),
        ));
    }
    Ok(ParameterTargetDefinition {
        owner,
        definition_id: definition_id.to_owned(),
        module_id: module_id.to_owned(),
        parameter: parameter.to_owned(),
    })
}

fn parse_global_parameter_target(
    path: &Path,
    value: &str,
    synths: &BTreeMap<String, SynthDefinition>,
    dsps: &BTreeMap<String, DspDefinition>,
) -> Result<ParameterTargetDefinition, SongLoadError> {
    let (owner, remainder) = if let Some(remainder) = value.strip_prefix("synth:") {
        (ParameterOwner::Synth, remainder)
    } else if let Some(remainder) = value.strip_prefix("dsp:") {
        (ParameterOwner::Dsp, remainder)
    } else {
        return Err(SongLoadError::one(
            path,
            format!("parameter target '{value}' must begin with 'synth:' or 'dsp:'"),
        ));
    };
    let (definition_id, endpoint) = remainder.split_once('/').ok_or_else(|| {
        SongLoadError::one(
            path,
            format!("parameter target '{value}' must be kind:id/module.parameter"),
        )
    })?;
    let (module_id, parameter) = endpoint.split_once('.').ok_or_else(|| {
        SongLoadError::one(
            path,
            format!("parameter target '{value}' must end in module.parameter"),
        )
    })?;
    let modules =
        match owner {
            ParameterOwner::Synth => &synths
                .get(definition_id)
                .ok_or_else(|| {
                    SongLoadError::one(
                        path,
                        format!(
                            "parameter target '{value}' references unknown synth '{definition_id}'"
                        ),
                    )
                })?
                .modules,
            ParameterOwner::Dsp => &dsps
                .get(definition_id)
                .ok_or_else(|| {
                    SongLoadError::one(
                        path,
                        format!(
                            "parameter target '{value}' references unknown DSP '{definition_id}'"
                        ),
                    )
                })?
                .modules,
        };
    let module = modules
        .iter()
        .find(|module| module.id == module_id)
        .ok_or_else(|| {
            SongLoadError::one(
                path,
                format!("parameter target '{value}' references unknown module '{module_id}'"),
            )
        })?;
    if !matches!(
        (module.kind, parameter),
        (ModuleKind::LowPass, "cutoff_hz")
            | (ModuleKind::LowPass, "resonance")
            | (ModuleKind::TempoDelay, "feedback")
            | (ModuleKind::TempoDelay, "mix")
    ) {
        return Err(SongLoadError::one(
            path,
            format!("module '{module_id}' has no automatable parameter '{parameter}'"),
        ));
    }
    Ok(ParameterTargetDefinition {
        owner,
        definition_id: definition_id.to_owned(),
        module_id: module_id.to_owned(),
        parameter: parameter.to_owned(),
    })
}

fn validate_parameter_value(
    path: &Path,
    target: &ParameterTargetDefinition,
    value: f64,
) -> Result<(), SongLoadError> {
    if !value.is_finite() {
        return Err(SongLoadError::one(path, "parameter value must be finite"));
    }
    if target.parameter == "cutoff_hz" && value <= 0.0 {
        return Err(SongLoadError::one(
            path,
            "filter cutoff parameter values must be positive",
        ));
    }
    if target.parameter == "feedback" && !(0.0..1.0).contains(&value) {
        return Err(SongLoadError::one(
            path,
            "delay feedback parameter values must be within 0.0..1.0",
        ));
    }
    if matches!(target.parameter.as_str(), "resonance" | "mix") && !(0.0..=1.0).contains(&value) {
        return Err(SongLoadError::one(
            path,
            format!(
                "{} parameter values must be within 0.0..=1.0",
                target.parameter
            ),
        ));
    }
    Ok(())
}

fn parse_pattern_length(path: &Path, value: &str) -> Result<f64, SongLoadError> {
    let Some(bars) = value.strip_suffix(" bar") else {
        return Err(SongLoadError::one(
            path,
            format!("unsupported pattern length '{value}'; expected '<count> bar'"),
        ));
    };
    let bars = bars.parse::<u32>().map_err(|_| {
        SongLoadError::one(
            path,
            format!("invalid bar count in pattern length '{value}'"),
        )
    })?;
    if bars == 0 {
        return Err(SongLoadError::one(path, "pattern length must be positive"));
    }
    Ok(f64::from(bars) * 4.0)
}

fn parse_note_fraction(path: &Path, value: &str) -> Result<f64, SongLoadError> {
    let (numerator, denominator) = value.split_once('/').ok_or_else(|| {
        SongLoadError::one(
            path,
            format!("musical duration '{value}' must be a fraction"),
        )
    })?;
    let numerator = numerator.parse::<u32>().map_err(|_| {
        SongLoadError::one(path, format!("invalid duration numerator in '{value}'"))
    })?;
    let denominator = denominator.parse::<u32>().map_err(|_| {
        SongLoadError::one(path, format!("invalid duration denominator in '{value}'"))
    })?;
    if numerator == 0 || denominator == 0 {
        return Err(SongLoadError::one(
            path,
            format!("musical duration '{value}' must be positive"),
        ));
    }
    Ok(4.0 * f64::from(numerator) / f64::from(denominator))
}

fn parse_position(path: &Path, value: &str) -> Result<f64, SongLoadError> {
    let parts = value.split(':').collect::<Vec<_>>();
    if parts.len() != 3 {
        return Err(SongLoadError::one(
            path,
            format!("musical position '{value}' must be bar:beat:tick"),
        ));
    }
    let bar = parts[0]
        .parse::<u32>()
        .map_err(|_| SongLoadError::one(path, format!("invalid bar in position '{value}'")))?;
    let beat = parts[1]
        .parse::<u32>()
        .map_err(|_| SongLoadError::one(path, format!("invalid beat in position '{value}'")))?;
    let tick = parts[2]
        .parse::<u32>()
        .map_err(|_| SongLoadError::one(path, format!("invalid tick in position '{value}'")))?;
    if bar == 0 || !(1..=4).contains(&beat) || tick >= 960 {
        return Err(SongLoadError::one(
            path,
            format!("musical position '{value}' is outside 4/4 bar:beat:tick bounds"),
        ));
    }
    Ok(f64::from((bar - 1) * 4 + (beat - 1)) + f64::from(tick) / 960.0)
}

fn frames_from_beats(path: &Path, beats: f64, frames_per_beat: f64) -> Result<u64, SongLoadError> {
    let frames = (beats * frames_per_beat).round();
    if !frames.is_finite() || frames < 0.0 || frames > u64::MAX as f64 {
        return Err(SongLoadError::one(
            path,
            "musical time cannot be represented on the u64 frame timeline",
        ));
    }
    Ok(frames as u64)
}

fn parse_note(path: &Path, value: &str) -> Result<u8, SongLoadError> {
    let octave_index = value
        .char_indices()
        .find_map(|(index, character)| {
            (character == '-' || character.is_ascii_digit()).then_some(index)
        })
        .ok_or_else(|| SongLoadError::one(path, format!("note '{value}' has no octave")))?;
    let (name, octave) = value.split_at(octave_index);
    let semitone = match name {
        "C" => 0,
        "C#" | "Db" => 1,
        "D" => 2,
        "D#" | "Eb" => 3,
        "E" => 4,
        "F" => 5,
        "F#" | "Gb" => 6,
        "G" => 7,
        "G#" | "Ab" => 8,
        "A" => 9,
        "A#" | "Bb" => 10,
        "B" => 11,
        _ => {
            return Err(SongLoadError::one(
                path,
                format!("unknown note name '{name}'"),
            ));
        }
    };
    let octave = octave
        .parse::<i16>()
        .map_err(|_| SongLoadError::one(path, format!("invalid octave in note '{value}'")))?;
    let midi = (octave + 1) * 12 + semitone;
    u8::try_from(midi)
        .ok()
        .filter(|note| *note <= 127)
        .ok_or_else(|| SongLoadError::one(path, format!("note '{value}' is outside MIDI range")))
}

fn read_toml<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, SongLoadError> {
    let input = fs::read_to_string(path)
        .map_err(|error| SongLoadError::one(path, format!("cannot read file: {error}")))?;
    toml::from_str(&input)
        .map_err(|error| SongLoadError::one(path, format!("TOML parse error: {error}")))
}

fn validate_header(path: &Path, header: &RawHeader, kind: &str) -> Result<(), SongLoadError> {
    if header.kind != kind {
        return Err(SongLoadError::one(
            path,
            format!(
                "expected meldritch.kind = '{kind}', found '{}'",
                header.kind
            ),
        ));
    }
    if header.version != 1 {
        return Err(SongLoadError::one(
            path,
            format!("unsupported {kind} format version {}", header.version),
        ));
    }
    Ok(())
}

fn resolve_song_reference(
    root: &Path,
    referring_file: &Path,
    reference: &Path,
) -> Result<PathBuf, SongLoadError> {
    if reference.is_absolute() {
        return Err(SongLoadError::one(
            referring_file,
            format!(
                "absolute reference '{}' is not allowed",
                reference.display()
            ),
        ));
    }
    let candidate = referring_file
        .parent()
        .expect("song entry has a parent")
        .join(reference);
    let resolved = candidate.canonicalize().map_err(|error| {
        SongLoadError::one(
            referring_file,
            format!(
                "cannot resolve reference '{}': {error}",
                reference.display()
            ),
        )
    })?;
    if !resolved.starts_with(root) {
        return Err(SongLoadError::one(
            referring_file,
            format!("reference '{}' escapes the song root", reference.display()),
        ));
    }
    Ok(resolved)
}

fn fingerprint_song(
    performance: &PerformanceDefinition,
    synths: &BTreeMap<String, SynthDefinition>,
    note_patterns: &BTreeMap<String, NotePatternDefinition>,
    parameter_patterns: &BTreeMap<String, ParameterPatternDefinition>,
    dsps: &BTreeMap<String, DspDefinition>,
) -> u64 {
    let mut fingerprint = Fnv64::new();
    fingerprint.string("meldritch-song-v1");
    fingerprint.string(&performance.id);
    fingerprint.string(&performance.title);
    fingerprint.u64(performance.bpm.to_bits());
    fingerprint.u64(u64::from(performance.sample_rate));
    for device in &performance.midi_devices {
        fingerprint.string(&device.id);
        fingerprint.string(&device.name_contains);
    }
    for track in &performance.tracks {
        fingerprint.string(&track.id);
        fingerprint.string(&track.synth_path.to_string_lossy());
        fingerprint.string(&track.synth_id);
        for pattern in &track.pattern_ids {
            fingerprint.string(pattern);
        }
        fingerprint.optional_string(track.initial_pattern.as_deref());
        for pattern in &track.parameter_pattern_ids {
            fingerprint.string(pattern);
        }
        for dsp in &track.dsp_ids {
            fingerprint.string(dsp);
        }
    }
    for lane in &performance.lanes {
        fingerprint.string(&lane.id);
        fingerprint.string(&lane.label);
        fingerprint.string(&lane.role);
        fingerprint.optional_string(lane.track_id.as_deref());
        for variation in &lane.variation_ids {
            fingerprint.string(variation);
        }
    }
    for page in &performance.pages {
        fingerprint.string(&page.id);
        fingerprint.string(&page.label);
        for strip in &page.strips {
            fingerprint.u64(u64::from(strip.strip));
            fingerprint.string(&strip.lane_id);
            for control in &strip.control_ids {
                fingerprint.string(control);
            }
        }
    }
    for control in &performance.controls {
        fingerprint.string(&control.id);
        fingerprint.string(&control.label);
        fingerprint.u64(control.target.owner as u64);
        fingerprint.string(&control.target.definition_id);
        fingerprint.string(&control.target.module_id);
        fingerprint.string(&control.target.parameter);
        fingerprint.u64(control.minimum.to_bits());
        fingerprint.u64(control.maximum.to_bits());
        fingerprint.u64(control.step.to_bits());
        fingerprint.string(&control.binding);
        for binding in &control.bindings {
            match binding {
                ControlBindingDefinition::Key { key } => {
                    fingerprint.string("key");
                    fingerprint.string(key);
                }
                ControlBindingDefinition::MidiCc {
                    device,
                    page,
                    channel,
                    cc,
                    action,
                } => {
                    fingerprint.string("midi_cc");
                    fingerprint.string(device);
                    fingerprint.optional_string(page.as_deref());
                    fingerprint.u64(channel.map_or(0, u64::from));
                    fingerprint.u64(u64::from(*cc));
                    fingerprint_control_binding_action(&mut fingerprint, *action);
                }
                ControlBindingDefinition::MidiNote {
                    device,
                    channel,
                    note,
                } => {
                    fingerprint.string("midi_note");
                    fingerprint.string(device);
                    fingerprint.u64(channel.map_or(0, u64::from));
                    fingerprint.u64(u64::from(*note));
                }
            }
        }
    }
    for action in &performance.actions {
        fingerprint.string(&action.id);
        fingerprint.string(&action.label);
        match &action.action {
            PerformanceActionDefinition::TogglePlayback => fingerprint.string("toggle_playback"),
            PerformanceActionDefinition::Stop => fingerprint.string("stop"),
            PerformanceActionDefinition::Rewind => fingerprint.string("rewind"),
            PerformanceActionDefinition::ToggleCockpitMode => {
                fingerprint.string("toggle_cockpit_mode");
            }
            PerformanceActionDefinition::QueueNextScene => fingerprint.string("queue_next_scene"),
            PerformanceActionDefinition::QueuePhrase { scene } => {
                fingerprint.string("queue_phrase");
                fingerprint.u64(*scene);
            }
            PerformanceActionDefinition::QueuePhraseVariation { scene, variation } => {
                fingerprint.string("queue_phrase_variation");
                fingerprint.u64(*scene);
                fingerprint.u64(*variation as u64);
            }
            PerformanceActionDefinition::SelectPage { page } => {
                fingerprint.string("select_page");
                fingerprint.string(page);
            }
            PerformanceActionDefinition::ToggleTrackMute => fingerprint.string("toggle_track_mute"),
            PerformanceActionDefinition::TriggerFill => fingerprint.string("trigger_fill"),
            PerformanceActionDefinition::CancelPerformance => {
                fingerprint.string("cancel_performance")
            }
        }
        for binding in &action.bindings {
            match binding {
                ControlBindingDefinition::Key { key } => {
                    fingerprint.string("key");
                    fingerprint.string(key);
                }
                ControlBindingDefinition::MidiCc {
                    device,
                    page,
                    channel,
                    cc,
                    action,
                } => {
                    fingerprint.string("midi_cc");
                    fingerprint.string(device);
                    fingerprint.optional_string(page.as_deref());
                    fingerprint.u64(channel.map_or(0, u64::from));
                    fingerprint.u64(u64::from(*cc));
                    fingerprint_control_binding_action(&mut fingerprint, *action);
                }
                ControlBindingDefinition::MidiNote {
                    device,
                    channel,
                    note,
                } => {
                    fingerprint.string("midi_note");
                    fingerprint.string(device);
                    fingerprint.u64(channel.map_or(0, u64::from));
                    fingerprint.u64(u64::from(*note));
                }
            }
        }
    }
    for synth in synths.values() {
        fingerprint.string(&synth.id);
        fingerprint.u64(u64::from(synth.polyphony));
        for input in &synth.inputs {
            fingerprint.string(&input.id);
            fingerprint.u64(input.signal as u64);
        }
        for module in &synth.modules {
            fingerprint.string(&module.id);
            fingerprint.u64(module.kind as u64);
            fingerprint.optional_string(module.waveform.as_deref());
            fingerprint.u64(module.frequency_hz.map_or(0, f64::to_bits));
            fingerprint.u64(u64::from(module.channels.unwrap_or(0)));
            fingerprint.u64(module.cutoff_hz.map_or(0, f64::to_bits));
            fingerprint.u64(module.resonance.map_or(0, f64::to_bits));
            fingerprint.optional_string(module.time.as_deref());
            fingerprint.u64(module.feedback.map_or(0, f64::to_bits));
            fingerprint.u64(module.mix.map_or(0, f64::to_bits));
            fingerprint.u64(module.attack.map_or(0, f64::to_bits));
            fingerprint.u64(module.decay.map_or(0, f64::to_bits));
            fingerprint.u64(module.sustain.map_or(0, f64::to_bits));
            fingerprint.u64(module.release.map_or(0, f64::to_bits));
        }
        for cable in &synth.cables {
            fingerprint.string(&cable.from);
            fingerprint.string(&cable.to);
            fingerprint.u64(cable.signal as u64);
        }
    }
    for pattern in note_patterns.values() {
        fingerprint.string(&pattern.id);
        fingerprint.u64(pattern.length_frames);
        fingerprint.u64(u64::from(pattern.looped));
        for event in &pattern.events {
            fingerprint.u64(event.start_frame);
            fingerprint.u64(event.duration_frames);
            fingerprint.u64(u64::from(event.note));
            fingerprint.u64(event.velocity.to_bits());
        }
    }
    for pattern in parameter_patterns.values() {
        fingerprint.string(&pattern.id);
        fingerprint.u64(pattern.length_frames);
        fingerprint.u64(u64::from(pattern.looped));
        for lane in &pattern.lanes {
            fingerprint.u64(lane.target.owner as u64);
            fingerprint.string(&lane.target.definition_id);
            fingerprint.string(&lane.target.module_id);
            fingerprint.string(&lane.target.parameter);
            fingerprint.u64(lane.interpolation as u64);
            for point in &lane.points {
                fingerprint.u64(point.frame);
                fingerprint.u64(point.value.to_bits());
            }
        }
    }
    for dsp in dsps.values() {
        fingerprint.string(&dsp.id);
        for input in &dsp.inputs {
            fingerprint.string(&input.id);
            fingerprint.u64(input.signal as u64);
        }
        for module in &dsp.modules {
            fingerprint.string(&module.id);
            fingerprint.u64(module.kind as u64);
            fingerprint.optional_string(module.time.as_deref());
            fingerprint.u64(module.feedback.map_or(0, f64::to_bits));
            fingerprint.u64(module.mix.map_or(0, f64::to_bits));
            fingerprint.u64(u64::from(module.channels.unwrap_or(0)));
        }
        for cable in &dsp.cables {
            fingerprint.string(&cable.from);
            fingerprint.string(&cable.to);
            fingerprint.u64(cable.signal as u64);
        }
    }
    fingerprint.finish()
}

fn fingerprint_control_binding_action(fingerprint: &mut Fnv64, action: ControlBindingAction) {
    match action {
        ControlBindingAction::Absolute => fingerprint.string("absolute"),
        ControlBindingAction::Centered { center } => {
            fingerprint.string("centered");
            fingerprint.u64(center.to_bits());
        }
        ControlBindingAction::Overdrive {
            normal,
            normal_midi,
        } => {
            fingerprint.string("overdrive");
            fingerprint.u64(normal.to_bits());
            fingerprint.u64(u64::from(normal_midi));
        }
        ControlBindingAction::Decrement => fingerprint.string("decrement"),
        ControlBindingAction::Increment => fingerprint.string("increment"),
    }
}

struct Fnv64(u64);

impl Fnv64 {
    const fn new() -> Self {
        Self(0xcbf2_9ce4_8422_2325)
    }

    fn bytes(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.0 ^= u64::from(*byte);
            self.0 = self.0.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }

    fn u64(&mut self, value: u64) {
        self.bytes(&value.to_le_bytes());
    }

    fn string(&mut self, value: &str) {
        self.u64(value.len() as u64);
        self.bytes(value.as_bytes());
    }

    fn optional_string(&mut self, value: Option<&str>) {
        match value {
            Some(value) => {
                self.u64(1);
                self.string(value);
            }
            None => self.u64(0),
        }
    }

    const fn finish(self) -> u64 {
        self.0
    }
}
