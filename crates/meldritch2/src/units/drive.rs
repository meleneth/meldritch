use crate::contract::{Contract, Domain, FanIn, InputPort, OutputPort, Param, Role};

fn process(
    _state: &mut [f64],
    input: &[f64],
    params: &[f64],
    _sample_rate: f64,
    outputs: &mut [f64],
) {
    let signal = if input[0].is_finite() { input[0] } else { 0.0 };
    let amount = params[0];
    // Normalization preserves full-scale peak level while tanh adds progressively
    // stronger odd-harmonic saturation. Oversampling is deliberately not hidden here.
    outputs[0] = (signal * amount).tanh() / amount.tanh() * params[1];
}

pub const CONTRACT: Contract = Contract {
    name: "drive",
    role: Role::Processor,
    process,
    state_len: 0,
    inputs: &[InputPort {
        name: "input",
        domain: Domain::MonoAudio,
        fan_in: FanIn::One,
        unconnected: None,
    }],
    outputs: &[OutputPort {
        name: "out",
        domain: Domain::MonoAudio,
    }],
    params: &[
        Param {
            name: "amount",
            default: 1.0,
            range: Some((0.01, 20.0)),
        },
        Param {
            name: "output_gain",
            default: 1.0,
            range: Some((0.0, 4.0)),
        },
    ],
};

#[cfg(test)]
mod tests {
    use super::process;
    #[test]
    fn normalized_tanh_is_bounded_and_drive_changes_shape_not_peak() {
        let mut low = [0.0];
        let mut high = [0.0];
        process(&mut [], &[0.25], &[1.0, 1.0], 48_000.0, &mut low);
        process(&mut [], &[0.25], &[10.0, 1.0], 48_000.0, &mut high);
        assert!(high[0] > low[0]);
        for input in [-10.0, f64::NAN, 10.0] {
            process(&mut [], &[input], &[20.0, 0.5], 48_000.0, &mut high);
            assert!(high[0].is_finite() && high[0].abs() <= 0.5);
        }
    }
}
