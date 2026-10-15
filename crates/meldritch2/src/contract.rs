#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Domain {
    MonoAudio,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FanIn {
    One,
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
    pub process: fn(&[f64], f64) -> f64,
    pub inputs: &'static [Port],
    pub outputs: &'static [Port],
    pub params: &'static [Param],
}

fn constant(_: &[f64], value: f64) -> f64 {
    value
}
fn gain(input: &[f64], gain: f64) -> f64 {
    input[0] * gain
}
fn output(input: &[f64], _: f64) -> f64 {
    input[0]
}
fn mix(input: &[f64], _: f64) -> f64 {
    input[0] + input[1]
}

const INPUT: Port = Port {
    name: "input",
    domain: Domain::MonoAudio,
    fan_in: FanIn::One,
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
const CONTRACTS: [Contract; 4] = [
    Contract {
        name: "constant",
        process: constant,
        inputs: &[],
        outputs: &[OUT],
        params: &[VALUE],
    },
    Contract {
        name: "gain",
        process: gain,
        inputs: &[INPUT],
        outputs: &[OUT],
        params: &[GAIN],
    },
    Contract {
        name: "output",
        process: output,
        inputs: &[INPUT],
        outputs: &[],
        params: &[],
    },
    Contract {
        name: "mix",
        process: mix,
        inputs: &[LEFT, RIGHT],
        outputs: &[OUT],
        params: &[],
    },
];

pub fn all() -> &'static [Contract] {
    &CONTRACTS
}
pub fn find(name: &str) -> Option<&'static Contract> {
    CONTRACTS.iter().find(|c| c.name == name)
}
