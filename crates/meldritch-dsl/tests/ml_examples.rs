use std::fs;
use std::path::{Path, PathBuf};

use meldritch_dsl::{
    ModuleKind, ParameterInterpolation, ParameterOwner, SignalType, load_song_directory,
};

fn examples_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("songs/examples")
}

fn collect_ml_files(directory: &Path, output: &mut Vec<PathBuf>) {
    let mut entries = fs::read_dir(directory)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", directory.display()))
        .map(|entry| {
            entry
                .expect("example directory entry should be readable")
                .path()
        })
        .collect::<Vec<_>>();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            collect_ml_files(&path, output);
        } else if matches!(
            path.extension().and_then(|extension| extension.to_str()),
            Some("mlsynth" | "mldsp" | "mlpattern" | "mlperformance")
        ) {
            output.push(path);
        }
    }
}

fn parse(path: &Path) -> toml::Value {
    let input = fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
    toml::from_str(&input)
        .unwrap_or_else(|error| panic!("failed to parse {}: {error}", path.display()))
}

fn required_string<'a>(value: &'a toml::Value, path: &[&str]) -> &'a str {
    let mut current = value;
    for segment in path {
        current = current
            .get(*segment)
            .unwrap_or_else(|| panic!("missing TOML field {}", path.join(".")));
    }
    current
        .as_str()
        .unwrap_or_else(|| panic!("TOML field {} must be a string", path.join(".")))
}

#[test]
fn every_example_file_is_versioned_toml_with_matching_kind() {
    let root = examples_root();
    let mut files = Vec::new();
    collect_ml_files(&root, &mut files);
    assert!(!files.is_empty(), "the example corpus must not be empty");

    for path in files {
        let value = parse(&path);
        let expected_kind = match path.extension().and_then(|extension| extension.to_str()) {
            Some("mlsynth") => "synth",
            Some("mldsp") => "dsp",
            Some("mlpattern") => "pattern",
            Some("mlperformance") => "performance",
            _ => unreachable!(),
        };
        assert_eq!(
            required_string(&value, &["meldritch", "kind"]),
            expected_kind,
            "{} has the wrong document kind",
            path.display()
        );
        assert_eq!(
            value
                .get("meldritch")
                .and_then(|header| header.get("version"))
                .and_then(toml::Value::as_integer),
            Some(1),
            "{} must use format version 1",
            path.display()
        );
    }
}

#[test]
fn performance_references_resolve_inside_their_song_root() {
    let examples = examples_root();
    let mut entries = fs::read_dir(&examples)
        .expect("example root should be readable")
        .map(|entry| entry.expect("example entry should be readable").path())
        .filter(|path| path.is_dir() && path.join("main.mlperformance").is_file())
        .collect::<Vec<_>>();
    entries.sort();

    for song_root in entries {
        let canonical_root = song_root
            .canonicalize()
            .expect("song root should canonicalize");
        let entry = song_root.join("main.mlperformance");
        let performance = parse(&entry);
        let tracks = performance
            .get("tracks")
            .and_then(toml::Value::as_array)
            .unwrap_or_else(|| panic!("{} must declare tracks", entry.display()));

        for track in tracks {
            let mut references = vec![required_string(track, &["synth"])];
            for field in ["patterns", "dsp"] {
                if let Some(values) = track.get(field).and_then(toml::Value::as_array) {
                    references.extend(values.iter().map(|value| {
                        value.as_str().unwrap_or_else(|| {
                            panic!("{}.{} entries must be paths", entry.display(), field)
                        })
                    }));
                }
            }

            for reference in references {
                let resolved = entry
                    .parent()
                    .expect("entry has a parent")
                    .join(reference)
                    .canonicalize()
                    .unwrap_or_else(|error| {
                        panic!(
                            "{} references missing file {reference}: {error}",
                            entry.display()
                        )
                    });
                assert!(
                    resolved.starts_with(&canonical_root),
                    "{} reference {reference} escapes the song root",
                    entry.display()
                );
            }
        }
    }
}

#[test]
fn minimal_synth_loads_as_a_typed_patch_graph() {
    let song_root = examples_root().join("00-minimal-synth");
    let song = load_song_directory(&song_root).expect("minimal song should validate");
    let repeated = load_song_directory(&song_root).expect("repeated load should validate");

    assert_eq!(song.performance().id(), "minimal-synth");
    assert_eq!(song.performance().tracks().len(), 1);
    assert_eq!(song.performance().tracks()[0].synth_id(), "drone");
    assert_eq!(song.synths().len(), 1);
    assert_eq!(song.fingerprint(), repeated.fingerprint());

    let synth = &song.synths()["drone"];
    assert_eq!(synth.polyphony(), 1);
    assert_eq!(synth.modules().len(), 2);
    assert_eq!(synth.modules()[0].kind(), ModuleKind::Oscillator);
    assert_eq!(synth.modules()[1].kind(), ModuleKind::AudioOutput);
    assert_eq!(synth.cables().len(), 1);
    assert_eq!(synth.cables()[0].signal(), SignalType::Audio);
}

#[test]
fn note_pattern_drives_a_modular_synth_on_the_u64_timeline() {
    let song = load_song_directory(examples_root().join("01-synth-note-pattern"))
        .expect("note-pattern song should validate");

    let synth = &song.synths()["bass"];
    assert_eq!(synth.inputs().len(), 2);
    assert_eq!(synth.inputs()[0].signal(), SignalType::Pitch);
    assert_eq!(synth.inputs()[1].signal(), SignalType::Gate);
    assert_eq!(synth.modules()[1].kind(), ModuleKind::Adsr);
    assert_eq!(synth.modules()[2].kind(), ModuleKind::Vca);

    let pattern = &song.note_patterns()["bass-line"];
    assert_eq!(pattern.length_frames(), 96_000);
    assert!(pattern.is_looped());
    assert_eq!(pattern.events().len(), 4);
    assert_eq!(pattern.events()[0].start_frame(), 0);
    assert_eq!(pattern.events()[0].duration_frames(), 24_000);
    assert_eq!(pattern.events()[0].note(), 48);
    assert_eq!(pattern.events()[1].start_frame(), 24_000);
    assert_eq!(pattern.events()[1].note(), 52);
    assert_eq!(pattern.events()[2].note(), 55);
    assert_eq!(pattern.events()[3].note(), 58);
    assert_eq!(
        song.performance().tracks()[0].initial_pattern(),
        Some("bass-line")
    );
}

#[test]
fn synth_parameter_pattern_resolves_to_a_real_module_parameter() {
    let song = load_song_directory(examples_root().join("02-synth-parameter-pattern"))
        .expect("synth parameter-pattern song should validate");

    let synth = &song.synths()["swept-bass"];
    let filter = synth
        .modules()
        .iter()
        .find(|module| module.id() == "filter")
        .expect("filter module should exist");
    assert_eq!(filter.kind(), ModuleKind::LowPass);
    assert_eq!(filter.cutoff_hz(), Some(180.0));

    let pattern = &song.parameter_patterns()["filter-sweep"];
    assert_eq!(pattern.length_frames(), 96_000);
    assert!(pattern.is_looped());
    assert_eq!(pattern.lanes().len(), 1);
    let lane = &pattern.lanes()[0];
    assert_eq!(lane.target().owner(), &ParameterOwner::Synth);
    assert_eq!(lane.target().definition_id(), "swept-bass");
    assert_eq!(lane.target().module_id(), "filter");
    assert_eq!(lane.target().parameter(), "cutoff_hz");
    assert_eq!(lane.interpolation(), ParameterInterpolation::Linear);
    assert_eq!(lane.points()[0].frame(), 0);
    assert_eq!(lane.points()[0].value(), 180.0);
    assert_eq!(lane.points()[1].frame(), 96_000);
    assert_eq!(lane.points()[1].value(), 2_400.0);
    assert_eq!(
        song.performance().tracks()[0].parameter_pattern_ids(),
        &["filter-sweep"]
    );
}

#[test]
fn track_resolves_an_ordered_tempo_aware_dsp_patch() {
    let song = load_song_directory(examples_root().join("03-dsp-chain"))
        .expect("DSP-chain song should validate");

    assert_eq!(song.performance().tracks()[0].dsp_ids(), &["echo"]);
    let dsp = &song.dsps()["echo"];
    assert_eq!(dsp.inputs().len(), 1);
    assert_eq!(dsp.inputs()[0].signal(), SignalType::Audio);
    assert_eq!(dsp.modules().len(), 2);
    let delay = &dsp.modules()[0];
    assert_eq!(delay.kind(), ModuleKind::TempoDelay);
    assert_eq!(delay.time(), Some("1/8"));
    assert_eq!(delay.feedback(), Some(0.35));
    assert_eq!(delay.mix(), Some(0.25));
    assert_eq!(dsp.cables().len(), 2);
    assert!(
        dsp.cables()
            .iter()
            .all(|cable| cable.signal() == SignalType::Audio)
    );
}

#[test]
fn dsp_parameter_pattern_and_curated_control_share_one_typed_target() {
    let song = load_song_directory(examples_root().join("04-dsp-parameter-pattern"))
        .expect("DSP parameter-pattern song should validate");

    let pattern = &song.parameter_patterns()["echo-motion"];
    let lane = &pattern.lanes()[0];
    assert_eq!(lane.target().owner(), &ParameterOwner::Dsp);
    assert_eq!(lane.target().definition_id(), "echo");
    assert_eq!(lane.target().module_id(), "delay");
    assert_eq!(lane.target().parameter(), "feedback");
    assert_eq!(lane.interpolation(), ParameterInterpolation::Step);
    assert_eq!(lane.points().len(), 4);
    assert_eq!(lane.points()[3].frame(), 72_000);

    let controls = song.performance().controls();
    assert_eq!(controls.len(), 1);
    assert_eq!(controls[0].id(), "echo-feedback");
    assert_eq!(controls[0].target(), lane.target());
    assert_eq!(controls[0].range(), (0.0, 0.85));
    assert_eq!(controls[0].step(), 0.05);
    assert_eq!(controls[0].binding(), "f");
}

#[test]
fn curated_control_example_resolves_its_small_performance_surface() {
    let song = load_song_directory(examples_root().join("09-curated-performance-controls"))
        .expect("curated-control song should validate");
    let [control] = song.performance().controls() else {
        panic!("example should expose exactly one control");
    };
    assert_eq!(control.id(), "echo-feedback");
    assert_eq!(control.label(), "Echo Feedback");
    assert_eq!(control.target().owner(), &ParameterOwner::Dsp);
    assert_eq!(control.target().definition_id(), "echo");
    assert_eq!(control.target().module_id(), "delay");
    assert_eq!(control.target().parameter(), "feedback");
}

#[test]
fn launch_control_xl_playground_declares_full_midi_surface_in_scripts() {
    let song = load_song_directory(examples_root().join("16-launch-control-xl-playground"))
        .expect("LaunchControl XL playground should validate");
    let devices = song.performance().midi_devices();
    assert_eq!(devices.len(), 1);
    assert_eq!(devices[0].id(), "launch-control-xl");
    assert_eq!(devices[0].name_contains(), "Launch Control XL");
    let controls = song.performance().controls();
    assert_eq!(controls.len(), 32);
    let actions = song.performance().actions();
    assert_eq!(actions.len(), 24);
    let midi_cc_bindings = controls
        .iter()
        .flat_map(|control| control.bindings())
        .chain(actions.iter().flat_map(|action| action.bindings()))
        .filter(|binding| {
            matches!(
                binding,
                meldritch_dsl::ControlBindingDefinition::MidiCc { .. }
            )
        })
        .count();
    assert_eq!(midi_cc_bindings, 36);
    let midi_note_bindings = actions
        .iter()
        .flat_map(|action| action.bindings())
        .filter(|binding| {
            matches!(
                binding,
                meldritch_dsl::ControlBindingDefinition::MidiNote { .. }
            )
        })
        .count();
    assert_eq!(midi_note_bindings, 20);
}
