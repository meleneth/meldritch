use std::collections::BTreeMap;
use std::sync::Arc;

use crate::sample::PreparedSample;
use crate::sequencer::SequencerControl;

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct Endpoint {
    pub module: String,
    pub port: String,
}

#[derive(Clone, Debug)]
pub struct Module {
    pub id: String,
    pub kind: String,
    pub values: BTreeMap<String, f64>,
    pub runtime: ModuleRuntime,
}

#[derive(Clone, Debug, Default)]
pub enum ModuleRuntime {
    #[default]
    Contract,
    Sample(Arc<PreparedSample>),
    Plaits(crate::plaits::PlaitsModel),
    SequencerTrack {
        sequencer: Arc<SequencerControl>,
        track: usize,
    },
    SequencerTempo(Arc<SequencerControl>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Connection {
    pub from: Endpoint,
    pub to: Endpoint,
}

#[derive(Clone, Debug)]
pub struct ExternalPort {
    pub name: String,
    pub endpoint: Endpoint,
}

#[derive(Clone, Debug)]
pub struct ExternalInput {
    pub name: String,
    pub destinations: Vec<Endpoint>,
}

#[derive(Clone, Debug)]
pub struct ExternalParameter {
    pub name: String,
    pub target: ParameterId,
}

#[derive(Clone, Debug)]
pub struct SubpatchInstance {
    pub id: String,
    pub definition: String,
    pub values: BTreeMap<String, f64>,
}

#[derive(Clone, Debug)]
pub struct Subpatch {
    pub graph: Graph,
    pub inputs: Vec<ExternalInput>,
    pub outputs: Vec<ExternalPort>,
    pub parameters: Vec<ExternalParameter>,
}

#[derive(Clone, Debug)]
pub struct Control {
    pub id: String,
    pub label: String,
    pub target: ParameterId,
    pub additional_targets: Vec<ParameterId>,
    pub min: f64,
    pub max: f64,
    pub scale: ControlScale,
    pub hidden: bool,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ControlScale {
    #[default]
    Linear,
    Logarithmic,
}

#[derive(Clone, Debug)]
pub struct MidiCc {
    pub control_id: String,
    pub channel: u8,
    pub number: u8,
}

#[derive(Clone, Debug)]
pub struct MidiNotes {
    pub channel: u8,
    pub pitch_control_id: String,
    pub gate_control_id: String,
    pub velocity_control_id: Option<String>,
}

#[derive(Clone, Debug)]
pub struct MidiPolyVoice {
    pub pitch_control_id: String,
    pub gate_control_id: String,
    pub velocity_control_id: String,
}

#[derive(Clone, Debug)]
pub struct MidiPoly {
    pub channel: u8,
    pub voices: Vec<MidiPolyVoice>,
    pub overflow: PolyOverflow,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum PolyOverflow {
    #[default]
    Reject,
    StealOldest,
}

#[derive(Clone, Debug)]
pub enum Widget {
    Label(String),
    Control {
        label: String,
        control_id: String,
    },
    Link {
        label: String,
        screen_id: String,
    },
    Button {
        label: String,
        control_id: String,
        value: f64,
    },
    PerformanceControls,
    PerformanceStatus,
    PatternGrid,
    TransportStatus,
    PerformanceSurface(PerformanceSurface),
    EventActivity {
        label: String,
        state: Arc<crate::event_bridge::EventUiState>,
    },
}

#[derive(Clone, Debug)]
pub struct PerformanceSurface {
    pub title: String,
    pub theme: String,
    pub tracks: Vec<PerformanceTrackView>,
    pub event_state: Option<Arc<crate::event_bridge::EventUiState>>,
}

#[derive(Clone, Debug)]
pub struct PerformanceTrackView {
    pub label: String,
    pub volume_control: String,
    pub control_ids: Vec<String>,
    pub color: String,
    pub melodic: bool,
}

#[derive(Clone, Debug)]
pub struct Screen {
    pub id: String,
    pub title: String,
    pub widgets: Vec<Widget>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NavigationEvent {
    pub name: String,
    pub from: String,
    pub to: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NavigationBinding {
    pub key: String,
    pub event: String,
}

/// Lua-authored UI navigation executed by the UI-thread Lua FSM runtime.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Navigation {
    pub initial: String,
    pub events: Vec<NavigationEvent>,
    pub bindings: Vec<NavigationBinding>,
}

#[derive(Clone, Debug, Default)]
pub struct Graph {
    pub devices: DevicePreferences,
    pub modules: Vec<Module>,
    pub instances: Vec<SubpatchInstance>,
    pub subpatches: BTreeMap<String, Subpatch>,
    pub connections: Vec<Connection>,
    pub normalizations: Vec<Connection>,
    pub controls: Vec<Control>,
    pub midi_ccs: Vec<MidiCc>,
    pub midi_notes: Vec<MidiNotes>,
    pub midi_buttons: Vec<MidiButton>,
    pub midi_packets: Vec<MidiPacket>,
    pub midi_poly: Vec<MidiPoly>,
    pub screens: Vec<Screen>,
    pub navigation: Option<Navigation>,
    pub performance: Option<Performance>,
    pub actions: Vec<Action>,
    pub control_surfaces: Vec<ControlSurface>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ControlSurface {
    pub id: String,
    pub device: String,
    pub initial_page: String,
    pub pages: Vec<ControlPage>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ControlPage {
    pub id: String,
    pub buttons: Vec<SurfaceButton>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SurfaceButtonMode {
    Momentary,
    Toggle,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SurfaceButton {
    pub id: String,
    pub index: u8,
    pub label: String,
    pub icon: Option<String>,
    pub color: [u8; 3],
    pub active_color: [u8; 3],
    pub mode: SurfaceButtonMode,
    pub press: Option<SurfaceAction>,
    pub release: Option<SurfaceAction>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum SurfaceAction {
    Command(String),
    Control { id: String, value: f64 },
    Page(String),
    Refresh,
}

#[derive(Clone, Debug)]
pub struct Performance {
    pub layout: String,
    pub sequencer: Arc<SequencerControl>,
    pub key_bindings: Vec<PerformanceBinding>,
    pub apc: Option<ApcMapping>,
    pub project_file: Option<String>,
}

#[derive(Clone, Debug)]
pub struct PerformanceBinding {
    pub key: String,
    pub command: String,
}

#[derive(Clone, Debug)]
pub struct ApcMapping {
    pub pages: [String; 2],
    pub performance_rows: [String; 8],
    pub fader_controls: [String; 9],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TransportAction {
    Toggle,
    Start,
    Stop,
    Restart,
}

#[derive(Clone, Debug)]
pub struct Action {
    pub label: String,
    pub sequencer: Arc<SequencerControl>,
    pub action: TransportAction,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MidiButtonMessage {
    Note,
    ControlChange,
    ProgramChange,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MidiButtonEdge {
    Press,
    Release,
}

/// A device-specific MIDI edge authored in Lua and connected to an action.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MidiButton {
    pub name: String,
    pub message: MidiButtonMessage,
    pub channel: u8,
    pub number: u8,
    pub edge: MidiButtonEdge,
    pub action_label: String,
}

/// An exact raw MIDI packet, including SysEx and system messages, authored in Lua.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MidiPacket {
    pub name: String,
    pub bytes: Vec<u8>,
    pub action_label: String,
}

impl Action {
    pub fn invoke(&self) {
        match self.action {
            TransportAction::Toggle => self.sequencer.toggle_transport(),
            TransportAction::Start => self.sequencer.start(),
            TransportAction::Stop => self.sequencer.stop(),
            TransportAction::Restart => self.sequencer.restart(),
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DevicePreferences {
    pub audio_output: Option<String>,
    pub midi_input: Option<String>,
    pub midi_controls: Vec<String>,
    pub apc_mini: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct ParameterId {
    pub module: String,
    pub name: String,
}

impl ParameterId {
    pub fn new(module: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            module: module.into(),
            name: name.into(),
        }
    }
}
