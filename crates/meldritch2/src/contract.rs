pub use meldritch_interface::{
    Contract, Domain, FanIn, InputPort, OutputPort, Param, ProcessFn, Role,
};

const CONTRACTS: [Contract; 39] = [
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
    crate::units::stereo_compressor::CONTRACT,
    crate::units::envelope_follower::CONTRACT,
    crate::units::compressor::CONTRACT,
    crate::units::adsr::CONTRACT,
    crate::units::vca::CONTRACT,
    crate::units::stereo_output::CONTRACT,
    crate::units::mixer2::CONTRACT,
    crate::units::mute::CONTRACT,
    crate::units::sample_player::CONTRACT,
    crate::units::sequencer_track::CONTRACT,
    crate::units::sequencer_tempo::CONTRACT,
    crate::units::stereo_mixer8::CONTRACT,
    crate::units::vst3::CONTRACT,
    crate::units::vst3_effect::CONTRACT,
];

pub fn all() -> &'static [Contract] {
    &CONTRACTS
}
pub fn find(name: &str) -> Option<&'static Contract> {
    CONTRACTS.iter().find(|c| c.name == name)
}
