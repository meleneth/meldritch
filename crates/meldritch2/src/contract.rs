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
    StereoOutput,
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
    pub process: ProcessFn,
    pub state_len: usize,
    pub inputs: &'static [InputPort],
    pub outputs: &'static [OutputPort],
    pub params: &'static [Param],
}

pub type ProcessFn = fn(&mut [f64], &[f64], &[f64], f64, &mut [f64]);

const CONTRACTS: [Contract; 35] = [
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
    crate::units::envelope::CONTRACT,
    crate::units::saw::CONTRACT,
    crate::units::oscillator_sine::CONTRACT,
    crate::units::triangle::CONTRACT,
    crate::units::square::CONTRACT,
    crate::units::pulse::CONTRACT,
    crate::units::plaits::CONTRACT,
    crate::units::noise::CONTRACT,
    crate::units::lowpass::CONTRACT,
    crate::units::svf::CONTRACT,
    crate::units::drive::CONTRACT,
    crate::units::ducker::CONTRACT,
    crate::units::delay::CONTRACT,
    crate::units::stereo_delay::CONTRACT,
    crate::units::envelope_follower::CONTRACT,
    crate::units::compressor::CONTRACT,
    crate::units::adsr::CONTRACT,
    crate::units::vca::CONTRACT,
    crate::units::stereo_output::CONTRACT,
    crate::units::mixer2::CONTRACT,
    crate::units::sample_player::CONTRACT,
    crate::units::sequencer_track::CONTRACT,
    crate::units::sequencer_tempo::CONTRACT,
    crate::units::stereo_mixer8::CONTRACT,
];

pub fn all() -> &'static [Contract] {
    &CONTRACTS
}
pub fn find(name: &str) -> Option<&'static Contract> {
    CONTRACTS.iter().find(|c| c.name == name)
}
