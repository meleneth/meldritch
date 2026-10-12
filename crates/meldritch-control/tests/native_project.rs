use meldritch_app::{AppCommand, AppController, NoteEdit, PatchCreation};
use meldritch_control::native_project::{encode_native_project, load_native_project};
use meldritch_core::FrameRange;
use meldritch_dsl::{NativeProjectDefinition, ValidatedSong};
use meldritch_render::song_host::{CompiledSongPatch, SongLiveOverrides};
use std::{fs, path::PathBuf, sync::Arc};

const EXAMPLE: &str =
    include_str!("../../../songs/examples/34-empty-project-patch/authored-project.lua");
const INVALID: &str =
    include_str!("../../../songs/examples/34-empty-project-patch/invalid/authored-project.lua");

struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "meldritch-native-project-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn pcm(source: &ValidatedSong, range: FrameRange) -> Vec<u8> {
    CompiledSongPatch::compile_initial(source)
        .unwrap()
        .render(range, &SongLiveOverrides::default())
        .unwrap()
        .samples()
        .iter()
        .flat_map(|v| v.to_le_bytes())
        .collect()
}

#[test]
fn readable_self_contained_example_round_trips_and_resets_runtime() {
    let directory = Directory::new();
    let source = load_native_project(&directory.0, EXAMPLE).unwrap();
    assert!(source.performance().tracks().is_empty());
    assert_eq!(source.pattern_material("melody").unwrap().next_note_id, 3);
    assert_eq!(source.pattern_voices()["voice-1-instance"].level, 0.8);
    let mut app = AppController::new_empty_project(
        &meldritch_dsl::load_project_directory(&directory.0).unwrap(),
    )
    .unwrap()
    .0;
    app.enable_project_source(source.clone()).unwrap();
    app.dispatch(AppCommand::SetOutputLevel {
        voice: "voice-1-instance".into(),
        control: "level".into(),
        value: Some(0.0),
    })
    .unwrap();
    let accepted = app.authored_source().unwrap();
    let encoded = encode_native_project(&accepted).unwrap();
    assert_eq!(load_native_project(&directory.0, &encoded).unwrap(), source);
    assert!(Arc::ptr_eq(&accepted, &app.authored_source().unwrap()));
    assert_eq!(app.output_levels().len(), 1);
    app.enable_project_source(load_native_project(&directory.0, &encoded).unwrap())
        .unwrap();
    assert!(app.output_levels().is_empty());
    app.dispatch(AppCommand::EditNote {
        pattern: "melody".into(),
        edit: NoteEdit::Insert {
            start_tick: 960,
            duration_ticks: 480,
            pitch: 55,
            velocity: 0.12345678901234567,
        },
    })
    .unwrap();
    let edited = app.authored_source().unwrap();
    assert_eq!(
        edited.pattern_material("melody").unwrap().notes[1].id,
        "note-3"
    );
    assert_eq!(
        load_native_project(&directory.0, &encode_native_project(&edited).unwrap()).unwrap(),
        *edited
    );
    assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 0);
}

#[test]
fn fresh_process_needs_only_the_snapshot_and_preserves_raw_pcm() {
    let origin = Directory::new();
    let destination = Directory::new();
    let source = load_native_project(&origin.0, EXAMPLE).unwrap();
    fs::write(
        destination.0.join("project.lua"),
        encode_native_project(&source).unwrap(),
    )
    .unwrap();
    fs::write(
        destination.0.join("expected.pcm"),
        pcm(&source, FrameRange::new(0, 192_000).unwrap()),
    )
    .unwrap();
    fs::write(
        destination.0.join("fingerprint"),
        format!("{:016x}", source.fingerprint()),
    )
    .unwrap();
    drop(origin);
    let child = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "fresh_process_child", "--ignored", "--nocapture"])
        .env("MELDRITCH_NATIVE_PROJECT_TEST_ROOT", &destination.0)
        .output()
        .unwrap();
    assert!(
        child.status.success(),
        "{}{}",
        String::from_utf8_lossy(&child.stdout),
        String::from_utf8_lossy(&child.stderr)
    );
}

#[test]
#[ignore = "executed by the isolated fresh-process parent"]
fn fresh_process_child() {
    let root = PathBuf::from(std::env::var_os("MELDRITCH_NATIVE_PROJECT_TEST_ROOT").unwrap());
    let source = load_native_project(
        &root,
        &fs::read_to_string(root.join("project.lua")).unwrap(),
    )
    .unwrap();
    assert_eq!(
        format!("{:016x}", source.fingerprint()),
        fs::read_to_string(root.join("fingerprint")).unwrap()
    );
    let expected = fs::read(root.join("expected.pcm")).unwrap();
    assert_eq!(pcm(&source, FrameRange::new(0, 192_000).unwrap()), expected);
    let mut split = pcm(&source, FrameRange::new(0, 17_123).unwrap());
    split.extend(pcm(&source, FrameRange::new(17_123, 192_000).unwrap()));
    assert_eq!(split, expected);
}

#[test]
fn rejects_invalid_or_lossy_snapshots_without_changing_accepted_source() {
    let directory = Directory::new();
    let source = load_native_project(&directory.0, EXAMPLE).unwrap();
    let mut app = AppController::new_empty_project(
        &meldritch_dsl::load_project_directory(&directory.0).unwrap(),
    )
    .unwrap()
    .0;
    app.enable_project_source(source.clone()).unwrap();
    let before = app.authored_source().unwrap();
    for invalid in [
        INVALID.to_owned(),
        EXAMPLE.replace("version = 2", "version = 3"),
        EXAMPLE.replace("kind = \"project\"", "kind = \"session\""),
        EXAMPLE.replace("pitch_pattern = \"melody\"", "pitch_pattern = \"missing\""),
        EXAMPLE.replace("to = \"osc.pitch\"", "to = \"env.gate\""),
        EXAMPLE.replace("to = \"out.audio\"", "to = \"missing.audio\""),
        EXAMPLE.replace("id = \"note-2\"", "id = \"note-3\""),
        EXAMPLE.replace("next_note_id = 3", "next_note_id = 0"),
        EXAMPLE.replace("duration_ticks = 480", "duration_ticks = 0"),
        EXAMPLE.replace("velocity = 0.8", "velocity = 2"),
        EXAMPLE.replace("bpm = 120", "bpm = 0/0"),
        EXAMPLE.replace("sample_rate = 48000", "sample_rate = 0"),
        "while true do end".into(),
        "return os.execute('true')".into(),
        " ".repeat(256 * 1024 + 1),
    ] {
        assert!(
            load_native_project(&directory.0, &invalid).is_err(),
            "{invalid:.160}"
        );
        assert!(Arc::ptr_eq(&before, &app.authored_source().unwrap()));
        assert!(app.history().is_empty());
    }
    let definition = source.native_project_definition().unwrap();
    let mut duplicate = definition.clone();
    duplicate.patches.push(duplicate.patches[0].clone());
    assert!(ValidatedSong::restore_native_project(&directory.0, &duplicate).is_err());
    duplicate = definition.clone();
    duplicate.patterns.push(duplicate.patterns[0].clone());
    assert!(ValidatedSong::restore_native_project(&directory.0, &duplicate).is_err());
    duplicate = definition.clone();
    duplicate.voices.push(duplicate.voices[0].clone());
    assert!(ValidatedSong::restore_native_project(&directory.0, &duplicate).is_err());
    let legacy = meldritch_dsl::load_song_directory(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../songs/examples/09-curated-performance-controls"),
    )
    .unwrap();
    assert!(encode_native_project(&legacy).is_err());
    assert_eq!(legacy.fingerprint(), 0x41472b38b65b60ed);
}

#[test]
fn rejects_oversized_material_and_preserves_empty_or_unbound_palette() {
    let directory = Directory::new();
    let empty = meldritch_dsl::load_project_directory(&directory.0)
        .unwrap()
        .authoring_source();
    assert_eq!(
        load_native_project(&directory.0, &encode_native_project(&empty).unwrap()).unwrap(),
        empty
    );
    let palette = empty
        .create_patch(&PatchCreation::mono_voice("unused"))
        .unwrap()
        .create_palette_pattern("unused-notes", 3840, true)
        .unwrap();
    assert_eq!(
        load_native_project(&directory.0, &encode_native_project(&palette).unwrap()).unwrap(),
        palette
    );
    let definition: NativeProjectDefinition = load_native_project(&directory.0, EXAMPLE)
        .unwrap()
        .native_project_definition()
        .unwrap();
    let mut too_large = definition.clone();
    too_large.patterns = vec![definition.patterns[0].clone(); 33];
    assert!(
        ValidatedSong::restore_native_project(&directory.0, &too_large)
            .unwrap_err()
            .contains("limits")
    );
    too_large = definition.clone();
    too_large.patterns[0].notes = vec![definition.patterns[0].notes[0].clone(); 4097];
    assert!(
        ValidatedSong::restore_native_project(&directory.0, &too_large)
            .unwrap_err()
            .contains("limits")
    );
}

#[test]
fn complete_edited_graph_defaults_and_independent_bindings_round_trip() {
    use meldritch_dsl::{
        InstrumentParameter, InstrumentValue, ModuleKind, PatchEdit, PatchOperation,
    };
    let directory = Directory::new();
    let source = load_native_project(&directory.0, EXAMPLE).unwrap();
    let source = source
        .preview_patch(&PatchEdit {
            synth: "voice-1".into(),
            operations: vec![
                PatchOperation::Disconnect {
                    from: "amp.audio".into(),
                    to: "out.audio".into(),
                },
                PatchOperation::Instantiate {
                    id: "filter".into(),
                    kind: ModuleKind::LowPass,
                },
                PatchOperation::Connect {
                    from: "amp.audio".into(),
                    to: "filter.audio".into(),
                },
                PatchOperation::Connect {
                    from: "filter.audio".into(),
                    to: "out.audio".into(),
                },
                PatchOperation::SetParameter {
                    module: "filter".into(),
                    parameter: InstrumentParameter::CutoffHz,
                    value: InstrumentValue::Number(1234.5678901234567),
                },
            ],
        })
        .unwrap();
    let source = source
        .bind_pattern_voice(
            "voice-1-instance",
            meldritch_dsl::PatternSignal::Pitch,
            None,
        )
        .unwrap();
    let restored =
        load_native_project(&directory.0, &encode_native_project(&source).unwrap()).unwrap();
    assert_eq!(restored, source);
    assert_eq!(
        pcm(&restored, FrameRange::new(0, 192_000).unwrap()),
        pcm(&source, FrameRange::new(0, 192_000).unwrap())
    );
    // Serialized module settings use the existing Plaits model enum and validator.
    // Equality tests authored state only; upstream RNG prevents an exact PCM claim.
    let plaits = EXAMPLE.replace("kind = \"oscillator\", waveform = \"sine\"",
        "kind = \"plaits\", plaits = { model = \"fm\", harmonics = 0.5, timbre = 0.5, morph = 0.5 }");
    let source = load_native_project(&directory.0, &plaits).unwrap();
    assert_eq!(
        load_native_project(&directory.0, &encode_native_project(&source).unwrap()).unwrap(),
        source
    );
}
