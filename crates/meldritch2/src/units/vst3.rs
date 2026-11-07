use crate::contract::{Contract, Domain, FanIn, InputPort, OutputPort, Role};

fn prepared_runtime_only(_: &mut [f64], _: &[f64], _: &[f64], _: f64, _: &mut [f64]) {
    unreachable!("VST3 modules are lowered to their prepared block runtime")
}

pub const CONTRACT: Contract = Contract {
    name: "vst3",
    role: Role::Processor,
    process: prepared_runtime_only,
    state_len: 0,
    inputs: &[
        InputPort {
            name: "pitch",
            domain: Domain::MonoAudio,
            fan_in: FanIn::One,
            unconnected: Some(440.0),
        },
        InputPort {
            name: "gate",
            domain: Domain::MonoAudio,
            fan_in: FanIn::One,
            unconnected: Some(0.0),
        },
        InputPort {
            name: "velocity",
            domain: Domain::MonoAudio,
            fan_in: FanIn::One,
            unconnected: Some(1.0),
        },
        InputPort {
            name: "tempo",
            domain: Domain::MonoAudio,
            fan_in: FanIn::One,
            unconnected: Some(120.0),
        },
        InputPort {
            name: "pitch_2",
            domain: Domain::MonoAudio,
            fan_in: FanIn::One,
            unconnected: Some(440.0),
        },
        InputPort {
            name: "gate_2",
            domain: Domain::MonoAudio,
            fan_in: FanIn::One,
            unconnected: Some(0.0),
        },
        InputPort {
            name: "velocity_2",
            domain: Domain::MonoAudio,
            fan_in: FanIn::One,
            unconnected: Some(1.0),
        },
        InputPort {
            name: "pitch_3",
            domain: Domain::MonoAudio,
            fan_in: FanIn::One,
            unconnected: Some(440.0),
        },
        InputPort {
            name: "gate_3",
            domain: Domain::MonoAudio,
            fan_in: FanIn::One,
            unconnected: Some(0.0),
        },
        InputPort {
            name: "velocity_3",
            domain: Domain::MonoAudio,
            fan_in: FanIn::One,
            unconnected: Some(1.0),
        },
        InputPort {
            name: "pitch_4",
            domain: Domain::MonoAudio,
            fan_in: FanIn::One,
            unconnected: Some(440.0),
        },
        InputPort {
            name: "gate_4",
            domain: Domain::MonoAudio,
            fan_in: FanIn::One,
            unconnected: Some(0.0),
        },
        InputPort {
            name: "velocity_4",
            domain: Domain::MonoAudio,
            fan_in: FanIn::One,
            unconnected: Some(1.0),
        },
        InputPort {
            name: "pitch_5",
            domain: Domain::MonoAudio,
            fan_in: FanIn::One,
            unconnected: Some(440.0),
        },
        InputPort {
            name: "gate_5",
            domain: Domain::MonoAudio,
            fan_in: FanIn::One,
            unconnected: Some(0.0),
        },
        InputPort {
            name: "velocity_5",
            domain: Domain::MonoAudio,
            fan_in: FanIn::One,
            unconnected: Some(1.0),
        },
        InputPort {
            name: "pitch_6",
            domain: Domain::MonoAudio,
            fan_in: FanIn::One,
            unconnected: Some(440.0),
        },
        InputPort {
            name: "gate_6",
            domain: Domain::MonoAudio,
            fan_in: FanIn::One,
            unconnected: Some(0.0),
        },
        InputPort {
            name: "velocity_6",
            domain: Domain::MonoAudio,
            fan_in: FanIn::One,
            unconnected: Some(1.0),
        },
        InputPort {
            name: "pitch_7",
            domain: Domain::MonoAudio,
            fan_in: FanIn::One,
            unconnected: Some(440.0),
        },
        InputPort {
            name: "gate_7",
            domain: Domain::MonoAudio,
            fan_in: FanIn::One,
            unconnected: Some(0.0),
        },
        InputPort {
            name: "velocity_7",
            domain: Domain::MonoAudio,
            fan_in: FanIn::One,
            unconnected: Some(1.0),
        },
        InputPort {
            name: "pitch_8",
            domain: Domain::MonoAudio,
            fan_in: FanIn::One,
            unconnected: Some(440.0),
        },
        InputPort {
            name: "gate_8",
            domain: Domain::MonoAudio,
            fan_in: FanIn::One,
            unconnected: Some(0.0),
        },
        InputPort {
            name: "velocity_8",
            domain: Domain::MonoAudio,
            fan_in: FanIn::One,
            unconnected: Some(1.0),
        },
    ],
    outputs: &[
        OutputPort {
            name: "left",
            domain: Domain::MonoAudio,
        },
        OutputPort {
            name: "right",
            domain: Domain::MonoAudio,
        },
    ],
    params: &[],
};
