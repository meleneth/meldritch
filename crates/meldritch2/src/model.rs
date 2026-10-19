use std::collections::BTreeMap;

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
}

#[derive(Clone, Debug)]
pub struct Screen {
    pub id: String,
    pub title: String,
    pub widgets: Vec<Widget>,
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
    pub midi_poly: Vec<MidiPoly>,
    pub screens: Vec<Screen>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DevicePreferences {
    pub audio_output: Option<String>,
    pub midi_input: Option<String>,
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
