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
pub struct Control {
    pub id: String,
    pub target: ParameterId,
    pub min: f64,
    pub max: f64,
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

#[derive(Clone, Debug, Default)]
pub struct Graph {
    pub modules: Vec<Module>,
    pub connections: Vec<Connection>,
    pub normalizations: Vec<Connection>,
    pub controls: Vec<Control>,
    pub midi_ccs: Vec<MidiCc>,
    pub midi_notes: Vec<MidiNotes>,
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
