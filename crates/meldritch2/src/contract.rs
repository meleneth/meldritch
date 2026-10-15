#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Domain {
    MonoAudio,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FanIn {
    One,
    Sum,
}

#[derive(Clone, Copy, Debug)]
pub struct Port {
    pub name: &'static str,
    pub domain: Domain,
    pub fan_in: FanIn,
}

#[derive(Clone, Copy, Debug)]
pub struct Param {
    pub name: &'static str,
    pub default: f64,
}

#[derive(Clone, Copy, Debug)]
pub struct Contract {
    pub name: &'static str,
    pub process: fn(&mut [f64], &[f64], f64, f64) -> f64,
    pub state_len: usize,
    pub inputs: &'static [Port],
    pub outputs: &'static [Port],
    pub params: &'static [Param],
}

fn constant(_: &mut [f64], _: &[f64], value: f64, _: f64) -> f64 {
    value
}
fn gain(_: &mut [f64], input: &[f64], gain: f64, _: f64) -> f64 {
    input[0] * gain
}
fn output(_: &mut [f64], input: &[f64], _: f64, _: f64) -> f64 {
    input[0]
}
fn mix(_: &mut [f64], input: &[f64], _: f64, _: f64) -> f64 {
    input[0] + input[1]
}
fn multiply(_: &mut [f64], input: &[f64], _: f64, _: f64) -> f64 {
    input[0] * input[1]
}
fn sine(state: &mut [f64], _: &[f64], frequency: f64, sample_rate: f64) -> f64 {
    let sample = (std::f64::consts::TAU * state[0]).sin();
    state[0] = (state[0] + frequency / sample_rate).rem_euclid(1.0);
    sample
}

const INPUT: Port = Port {
    name: "input",
    domain: Domain::MonoAudio,
    fan_in: FanIn::One,
};
const SUM_INPUT: Port = Port {
    name: "input",
    domain: Domain::MonoAudio,
    fan_in: FanIn::Sum,
};
const LEFT: Port = Port {
    name: "left",
    domain: Domain::MonoAudio,
    fan_in: FanIn::One,
};
const RIGHT: Port = Port {
    name: "right",
    domain: Domain::MonoAudio,
    fan_in: FanIn::One,
};
const OUT: Port = Port {
    name: "out",
    domain: Domain::MonoAudio,
    fan_in: FanIn::One,
};
const VALUE: Param = Param {
    name: "value",
    default: 0.0,
};
const GAIN: Param = Param {
    name: "gain",
    default: 1.0,
};
const FREQUENCY: Param = Param {
    name: "frequency",
    default: 440.0,
};
const CONTRACTS: [Contract; 6] = [
    Contract {
        name: "constant",
        process: constant,
        state_len: 0,
        inputs: &[],
        outputs: &[OUT],
        params: &[VALUE],
    },
    Contract {
        name: "gain",
        process: gain,
        state_len: 0,
        inputs: &[SUM_INPUT],
        outputs: &[OUT],
        params: &[GAIN],
    },
    Contract {
        name: "output",
        process: output,
        state_len: 0,
        inputs: &[INPUT],
        outputs: &[],
        params: &[],
    },
    Contract {
        name: "mix",
        process: mix,
        state_len: 0,
        inputs: &[LEFT, RIGHT],
        outputs: &[OUT],
        params: &[],
    },
    Contract {
        name: "multiply",
        process: multiply,
        state_len: 0,
        inputs: &[LEFT, RIGHT],
        outputs: &[OUT],
        params: &[],
    },
    Contract {
        name: "sine",
        process: sine,
        state_len: 1,
        inputs: &[],
        outputs: &[OUT],
        params: &[FREQUENCY],
    },
];

pub fn all() -> &'static [Contract] {
    &CONTRACTS
}
pub fn find(name: &str) -> Option<&'static Contract> {
    CONTRACTS.iter().find(|c| c.name == name)
}
