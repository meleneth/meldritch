use crate::contract::{Contract, Domain, OutputPort, Param, Role};
fn unreachable_process(_: &mut [f64], _: &[f64], _: &[f64], _: f64, _: &mut [f64]) {
    unreachable!()
}
pub const CONTRACT: Contract = Contract {
    name: "sequencer_tempo",
    role: Role::Processor,
    process: unreachable_process,
    state_len: 0,
    inputs: &[],
    outputs: &[OutputPort {
        name: "out",
        domain: Domain::MonoAudio,
    }],
    params: &[Param {
        name: "bpm",
        default: 120.0,
        range: Some((1.0, 999.0)),
    }],
};
