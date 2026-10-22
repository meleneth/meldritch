use std::{
    fs,
    path::{Path, PathBuf},
};

use meldritch2::{compiler::compile_at_rate, lua, model::Graph};

fn example(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}

fn load(relative: &str) -> Graph {
    let path = example(relative);
    lua::read_with_root(&fs::read_to_string(&path).unwrap(), path.parent().unwrap()).unwrap()
}

fn render(graph: &Graph, rate: f64, seconds: f64) -> (Vec<f64>, Vec<f64>) {
    let mut machine = compile_at_rate(graph, rate).unwrap();
    let frames = (rate * seconds) as usize;
    let mut left = vec![0.0; frames];
    let mut right = vec![0.0; frames];
    machine.process_stereo(&mut left, &mut right);
    assert!(left.iter().chain(&right).all(|sample| sample.is_finite()));
    assert!(left.iter().chain(&right).any(|sample| sample.abs() > 1e-7));
    (left, right)
}

#[test]
fn arpeggiator_playground_is_polymetric_and_reproducible() {
    let first = load("examples/arpeggiator/arpeggiator.lua");
    let sequence = &first.performance.as_ref().unwrap().sequencer;
    assert_eq!(sequence.track_names(), ["Crystal", "Undertow", "Firefly"]);
    assert_eq!(
        sequence.pattern_names(),
        ["Glass", "Tides", "Sparks", "Home"]
    );
    for index in 0..4 {
        assert_eq!(
            sequence.pattern_at(index).unwrap().lengths[..3],
            [15, 20, 28]
        );
    }
    let second = load("examples/arpeggiator/arpeggiator.lua");
    assert_eq!(render(&first, 2_000.0, 8.0), render(&second, 2_000.0, 8.0));
}

#[test]
fn delay_laboratory_uses_stereo_tempo_routing_and_produces_distinct_channels() {
    let graph = load("examples/delay_lab/delay_lab.lua");
    let sequence = &graph.performance.as_ref().unwrap().sequencer;
    assert_eq!(sequence.track_names(), ["Pulse", "Chime"]);
    for index in 0..4 {
        assert_eq!(sequence.pattern_at(index).unwrap().lengths[..2], [32, 48]);
    }
    let machine = compile_at_rate(&graph, 4_000.0).unwrap();
    for control in [
        "left_time",
        "right_time",
        "feedback",
        "crossfeed",
        "delay_mix",
        "feedback_filter",
        "feedback_drive",
    ] {
        assert!(
            machine.control_handle(control).is_some(),
            "missing {control}"
        );
    }
    let (left, right) = render(&graph, 4_000.0, 8.0);
    assert!(
        left.iter()
            .zip(&right)
            .any(|(left, right)| (left - right).abs() > 1e-6)
    );
}

fn write_acceptance_wav(relative: &str, name: &str) {
    const RATE: u32 = 1_000;
    const SECONDS: u32 = 121;
    let graph = load(relative);
    let (left, right) = render(&graph, f64::from(RATE), f64::from(SECONDS));
    let path = std::env::temp_dir().join(format!("meldritch2-{name}-{}.wav", std::process::id()));
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: RATE,
        bits_per_sample: 24,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(&path, spec).unwrap();
    for (left, right) in left.into_iter().zip(right) {
        writer
            .write_sample((left.clamp(-1.0, 1.0) * 8_388_607.0).round() as i32)
            .unwrap();
        writer
            .write_sample((right.clamp(-1.0, 1.0) * 8_388_607.0).round() as i32)
            .unwrap();
    }
    writer.finalize().unwrap();
    let reader = hound::WavReader::open(&path).unwrap();
    assert_eq!(reader.spec().channels, 2);
    assert_eq!(reader.spec().bits_per_sample, 24);
    assert_eq!(reader.duration(), RATE * SECONDS);
    drop(reader);
    fs::remove_file(path).unwrap();
}

#[test]
fn both_compositions_render_finite_stereo_wavs_longer_than_two_minutes() {
    write_acceptance_wav("examples/arpeggiator/arpeggiator.lua", "arp");
    write_acceptance_wav("examples/delay_lab/delay_lab.lua", "delay-lab");
}
