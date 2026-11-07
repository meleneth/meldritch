use crate::contract::{Contract, Domain, FanIn, InputPort, OutputPort, Role};

fn prepared_runtime_only(_: &mut [f64], _: &[f64], _: &[f64], _: f64, _: &mut [f64]) {
    unreachable!("VST3 effects are lowered to their prepared block runtime")
}

pub const CONTRACT: Contract = Contract {
    name: "vst3_effect",
    role: Role::Processor,
    process: prepared_runtime_only,
    state_len: 0,
    inputs: &[
        InputPort {
            name: "input_left",
            domain: Domain::MonoAudio,
            fan_in: FanIn::One,
            unconnected: None,
        },
        InputPort {
            name: "input_right",
            domain: Domain::MonoAudio,
            fan_in: FanIn::One,
            unconnected: None,
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
