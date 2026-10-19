use crate::contract::{Contract, Domain, OutputPort, Role};
fn unreachable_process(_: &mut [f64], _: &[f64], _: &[f64], _: f64, _: &mut [f64]) {
    unreachable!()
}
pub const CONTRACT: Contract = Contract {
    name: "sequencer_track",
    role: Role::Processor,
    process: unreachable_process,
    state_len: 0,
    inputs: &[],
    outputs: &[OutputPort {
        name: "out",
        domain: Domain::MonoAudio,
    }],
    params: &[],
};
