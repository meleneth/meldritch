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
