#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Domain {
    MonoAudio,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FanIn {
    One,
    Sum,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Role {
    Processor,
    Output,
}

#[derive(Clone, Copy, Debug)]
pub struct InputPort {
    pub name: &'static str,
    pub domain: Domain,
    pub fan_in: FanIn,
    /// Neutral input sample when no cable is connected; None means required.
    pub unconnected: Option<f64>,
}

#[derive(Clone, Copy, Debug)]
pub struct OutputPort {
    pub name: &'static str,
    pub domain: Domain,
}

#[derive(Clone, Copy, Debug)]
pub struct Param {
    pub name: &'static str,
    pub default: f64,
    pub range: Option<(f64, f64)>,
}

#[derive(Clone, Copy, Debug)]
pub struct Contract {
    pub name: &'static str,
    pub role: Role,
    pub process: fn(&mut [f64], &[f64], &[f64], f64, &mut [f64]),
    pub state_len: usize,
    pub inputs: &'static [InputPort],
    pub outputs: &'static [OutputPort],
    pub params: &'static [Param],
}

const CONTRACTS: [Contract; 11] = [
    crate::units::constant::CONTRACT,
    crate::units::gain::CONTRACT,
    crate::units::output::CONTRACT,
    crate::units::mix::CONTRACT,
    crate::units::multiply::CONTRACT,
    crate::units::affine::CONTRACT,
    crate::units::sine::CONTRACT,
    crate::units::lag::CONTRACT,
    crate::units::switch::CONTRACT,
    crate::units::mirror::CONTRACT,
    crate::units::sample_hold::CONTRACT,
];

pub fn all() -> &'static [Contract] {
    &CONTRACTS
}
pub fn find(name: &str) -> Option<&'static Contract> {
    CONTRACTS.iter().find(|c| c.name == name)
}
