use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use meldritch_app::{AppCommand, AppController, CompositionDefinition, NoteEdit};
use meldritch_control::{
    native_capture::{NativeCommandEnvelope, NativeProjectCapture, load_native_capture_baseline},
    native_project::{NativeProjectStore, load_native_project},
};
use meldritch_core::FrameRange;
use meldritch_render::song_host::CompiledSongPatch;
use std::{fs, path::PathBuf, sync::Arc};

const FIXTURE: &str =
    include_str!("../../../songs/examples/35-project-palette-routing/compositions.project.lua");

fn key(app: &mut AppController, code: KeyCode) {
    let event = KeyEvent::new(code, KeyModifiers::NONE);
    let source = app.authored_source().unwrap();
    let view = app.view_model();
    let command = meldritch_tui::project_level_command(event, &view, &source)
        .or_else(|| meldritch_tui::project_composition_command(event, &view, &source))
        .or_else(|| meldritch_tui::project_binding_command(event, &view, &source))
        .or_else(|| meldritch_tui::project_pattern_command(event, &view, &source))
        .expect("native key lowers to typed command");
    app.dispatch(command).unwrap();
}

#[test]
fn native_shared_instance_keys_membership_and_controls_reconstruct_without_retargeting() {
    let directory = Directory::new();
    let source = load_native_project(&directory.0, FIXTURE).unwrap();
    let mut app = directory.app();
    app.enable_native_project_source(source.clone()).unwrap();
    assert_eq!(
        app.view_model().selected_pattern_voice.as_deref(),
        Some("bass-copy")
    );
    let mut capture = NativeProjectCapture::new(app.authored_source().unwrap(), 96_000);
    // Same palette patch, independently routed instance, no implicit membership.
    for code in [
        KeyCode::Char('v'),
        KeyCode::Char('p'),
        KeyCode::PageDown,
        KeyCode::Char('g'),
    ] {
        key(&mut app, code);
        capture.checkpoint(&app).unwrap();
    }
    let created = app.authored_source().unwrap();
    let id = "voice-1-instance-2";
    assert_eq!(app.view_model().selected_pattern_voice.as_deref(), Some(id));
    assert!(!created.is_pattern_voice_active(id));
    assert_eq!(created.synths(), source.synths());
    assert_eq!(created.note_patterns(), source.note_patterns());
    assert_eq!(
        created.pattern_voices()[id].pitch_pattern.as_deref(),
        Some("melody")
    );
    assert_eq!(
        created.pattern_voices()[id].gate_pattern.as_deref(),
        Some("rhythm")
    );
    key(&mut app, KeyCode::Char('a'));
    capture.checkpoint(&app).unwrap();
    let included = app.authored_source().unwrap();
    key(&mut app, KeyCode::Char('a'));
    capture.checkpoint(&app).unwrap();
    assert!(Arc::ptr_eq(&included, &app.authored_source().unwrap()));
    // Selection changes neither the patch inspector nor authored revision.
    key(&mut app, KeyCode::Down);
    capture.checkpoint(&app).unwrap();
    assert_eq!(
        app.view_model().selected_pattern_voice.as_deref(),
        Some("bass-copy")
    );
    assert_eq!(app.view_model().instrument_editor.unwrap().synth, "voice-1");
    assert!(app.history().back().unwrap().dirty_ranges.is_empty());
    key(&mut app, KeyCode::Char('d'));
    capture.checkpoint(&app).unwrap();
    let removed = app.authored_source().unwrap();
    assert!(!removed.is_pattern_voice_active("bass-copy"));
    assert_eq!(
        removed.compositions()["pulse"],
        source.compositions()["pulse"]
    );
    app.dispatch(AppCommand::SetCockpitMode(
        meldritch_app::CockpitMode::Performance,
    ))
    .unwrap();
    capture.checkpoint(&app).unwrap();
    assert!(
        meldritch_tui::project_level_command(
            KeyEvent::new(KeyCode::Home, KeyModifiers::NONE),
            &app.view_model(),
            &removed
        )
        .is_none(),
        "inactive selection must not silently retarget"
    );
    key(&mut app, KeyCode::Down);
    capture.checkpoint(&app).unwrap();
    assert_eq!(
        app.view_model().selected_pattern_voice.as_deref(),
        Some("voice-1-instance")
    );
    key(&mut app, KeyCode::Home);
    capture.checkpoint(&app).unwrap();
    key(&mut app, KeyCode::Down);
    capture.checkpoint(&app).unwrap();
    key(&mut app, KeyCode::Left);
    capture.checkpoint(&app).unwrap();
    assert_eq!(
        app.output_levels()[&("voice-1-instance".into(), "level".into())],
        0.0
    );
    assert!((app.output_levels()[&(id.into(), "level".into())] - 0.7).abs() < 1e-12);
    assert!(Arc::ptr_eq(&removed, &app.authored_source().unwrap()));
    capture.finish(&app).unwrap();
    let text = fs::read_to_string(capture.session.as_ref().unwrap().path()).unwrap();
    let mut replay = directory.app();
    replay
        .enable_native_project_source(load_native_capture_baseline(&directory.0, &text).unwrap())
        .unwrap();
    let document: toml::Value = toml::from_str(&text).unwrap();
    for event in document["events"].as_array().unwrap() {
        let envelope =
            NativeCommandEnvelope::decode(&toml::to_string(&event["native"]).unwrap()).unwrap();
        replay.dispatch(envelope.command.into()).unwrap();
        assert_eq!(
            envelope.source_after,
            format!("{:016x}", replay.authored_source().unwrap().fingerprint())
        );
    }
    assert_eq!(
        replay.view_model().selected_pattern_voice,
        app.view_model().selected_pattern_voice
    );
    assert_eq!(replay.output_levels(), app.output_levels());
    let mut store = NativeProjectStore::open(&directory.project()).unwrap();
    store.save(&removed).unwrap();
    let reopened = NativeProjectStore::open(&directory.project()).unwrap();
    let mut fresh = directory.app();
    fresh
        .enable_native_project_source(reopened.source().unwrap().clone())
        .unwrap();
    assert_eq!(
        fresh.authored_source().unwrap().fingerprint(),
        removed.fingerprint()
    );
    assert_eq!(
        fresh.view_model().selected_pattern_voice.as_deref(),
        Some("voice-1-instance")
    );
    assert!(fresh.output_levels().is_empty());
}

#[test]
fn invalid_instance_selection_and_membership_preserve_accepted_state() {
    let directory = Directory::new();
    let mut app = directory.app();
    app.enable_native_project_source(load_native_project(&directory.0, FIXTURE).unwrap())
        .unwrap();
    let source = app.authored_source().unwrap();
    let selected = app.view_model().selected_pattern_voice;
    for command in [
        AppCommand::SelectPatternVoice("missing".into()),
        AppCommand::SetCompositionMember {
            composition: "missing".into(),
            voice: "bass-copy".into(),
            included: true,
        },
        AppCommand::SetCompositionMember {
            composition: "held".into(),
            voice: "missing".into(),
            included: false,
        },
    ] {
        assert!(app.dispatch(command).is_err());
        assert!(app.history().is_empty());
        assert!(Arc::ptr_eq(&source, &app.authored_source().unwrap()));
        assert_eq!(app.view_model().selected_pattern_voice, selected);
    }
    app.dispatch(AppCommand::Play).unwrap();
    app.dispatch(AppCommand::Stop).unwrap();
    let history = app.history().len();
    assert!(
        app.dispatch(AppCommand::SetCompositionMember {
            composition: "held".into(),
            voice: "bass-copy".into(),
            included: false
        })
        .is_err()
    );
    assert_eq!(history, app.history().len());
    assert!(Arc::ptr_eq(&source, &app.authored_source().unwrap()));
    assert_eq!(app.view_model().selected_pattern_voice, selected);
}
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "meldritch-compositions-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn project(&self) -> meldritch_dsl::ProjectDirectory {
        meldritch_dsl::load_project_directory(&self.0).unwrap()
    }
    fn app(&self) -> AppController {
        AppController::new_empty_project(&self.project()).unwrap().0
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn pcm(source: &meldritch_dsl::ValidatedSong, range: FrameRange) -> Vec<u8> {
    CompiledSongPatch::compile_initial(source)
        .unwrap()
        .render_default(range)
        .unwrap()
        .samples()
        .iter()
        .flat_map(|sample| sample.to_le_bytes())
        .collect()
}

#[test]
fn audited_invalid_candidates_reject_referenced_deletion_chords_combination_and_clocks() {
    let directory = Directory::new();
    let source = load_native_project(&directory.0, FIXTURE).unwrap();
    let mut app = directory.app();
    app.enable_native_project_source(source.clone()).unwrap();
    app.dispatch(AppCommand::SetOutputLevel {
        voice: "bass-copy".into(),
        control: "level".into(),
        value: Some(0.2),
    })
    .unwrap();
    let accepted = app.authored_source().unwrap();
    let selected = app.view_model().selected_pattern_voice;
    let runtime = app.output_levels().clone();
    let history = app.history().len();
    let cases: toml::Value = toml::from_str(include_str!(
        "../../../songs/examples/35-project-palette-routing/invalid/audit.toml"
    ))
    .unwrap();
    for case in cases["cases"].as_array().unwrap() {
        let kind = case["kind"].as_str().unwrap();
        let mut definition = source.native_project_definition().unwrap();
        let result = match kind {
            "multiple_selections" => load_native_project(
                &directory.0,
                &FIXTURE.replace(
                    "selected_composition = \"held\"",
                    "selected_composition = { \"held\", \"pulse\" }",
                ),
            ),
            "multiple_pitch_sources" => load_native_project(
                &directory.0,
                &FIXTURE.replace(
                    "pitch_pattern = \"melody\"",
                    "pitch_pattern = { \"melody\", \"rhythm\" }",
                ),
            ),
            "multiple_gate_sources" => load_native_project(
                &directory.0,
                &FIXTURE.replace(
                    "gate_pattern = \"rhythm\"",
                    "gate_pattern = { \"rhythm\", \"short-rhythm\" }",
                ),
            ),
            _ => {
                match kind {
                    "duplicate_instance" => definition.voices.push(definition.voices[0].clone()),
                    "remove_patch" => definition.patches.clear(),
                    "remove_pitch_pattern" => {
                        definition.patterns.retain(|pattern| pattern.id != "melody")
                    }
                    "remove_inactive_gate_pattern" => definition
                        .patterns
                        .retain(|pattern| pattern.id != "short-rhythm"),
                    "pitch_chord" => {
                        let melody = definition
                            .patterns
                            .iter_mut()
                            .find(|pattern| pattern.id == "melody")
                            .unwrap();
                        melody.notes[1].start_tick = 0;
                    }
                    "gate_overlap" => {
                        let rhythm = definition
                            .patterns
                            .iter_mut()
                            .find(|pattern| pattern.id == "short-rhythm")
                            .unwrap();
                        rhythm.notes[0].duration_ticks = 1440;
                    }
                    "clock_length" => definition.patterns[0].length_ticks = 7680,
                    "clock_loop" => definition.patterns[0].looped = false,
                    _ => panic!("unexecuted audit case {kind}"),
                }
                meldritch_dsl::ValidatedSong::restore_native_project(&directory.0, &definition)
            }
        };
        let error = result.unwrap_err();
        if let Some(expected) = case.get("expect") {
            assert!(
                error.contains(expected.as_str().unwrap()),
                "{kind}: {error}"
            );
        }
        assert!(Arc::ptr_eq(&accepted, &app.authored_source().unwrap()));
        assert_eq!(app.view_model().selected_pattern_voice, selected);
        assert_eq!(app.output_levels(), &runtime);
        assert_eq!(app.history().len(), history);
    }
    // The actual accepted-command path rejects duplicate IDs and bound chords too.
    for command in [
        AppCommand::CreatePatternVoice(source.pattern_voices()["bass-copy"].clone()),
        AppCommand::EditNote {
            pattern: "melody".into(),
            edit: NoteEdit::Insert {
                start_tick: 0,
                duration_ticks: 480,
                pitch: 60,
                velocity: 0.8,
            },
        },
    ] {
        assert!(app.dispatch(command).is_err());
        assert!(Arc::ptr_eq(&accepted, &app.authored_source().unwrap()));
        assert_eq!(app.view_model().selected_pattern_voice, selected);
        assert_eq!(app.output_levels(), &runtime);
        assert_eq!(app.history().len(), history);
    }
    assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 0);
}

#[test]
fn one_shared_patch_edit_changes_each_composition_and_preserves_instance_overlays() {
    use meldritch_app::{
        InstrumentEdit, InstrumentParameter, InstrumentParameterId, InstrumentValue,
        InstrumentWaveform,
    };
    let directory = Directory::new();
    let source = load_native_project(&directory.0, FIXTURE).unwrap();
    let range = FrameRange::new(0, 192_000).unwrap();
    let mut app = directory.app();
    app.enable_native_project_source(source.clone()).unwrap();
    app.dispatch(AppCommand::SetOutputLevel {
        voice: "bass-copy".into(),
        control: "level".into(),
        value: Some(0.2),
    })
    .unwrap();
    let runtime = app.output_levels().clone();
    app.dispatch(AppCommand::EditInstrument(InstrumentEdit {
        target: InstrumentParameterId {
            synth: "voice-1".into(),
            module: "osc".into(),
            parameter: InstrumentParameter::Waveform,
        },
        value: InstrumentValue::Waveform(InstrumentWaveform::Triangle),
    }))
    .unwrap();
    let edited = app.authored_source().unwrap();
    assert_eq!(edited.synths().len(), 1);
    assert_eq!(edited.pattern_voices(), source.pattern_voices());
    assert_eq!(edited.note_patterns(), source.note_patterns());
    assert_eq!(edited.compositions(), source.compositions());
    assert_eq!(app.output_levels(), &runtime);
    for id in ["held", "pulse"] {
        let selected = edited.select_composition(id).unwrap();
        assert!(pcm(&selected, range) != pcm(&source.select_composition(id).unwrap(), range));
        // Every referencing instance receives the edited oscillator, while its
        // independent authored level and event bindings stay byte-for-byte stable.
        let patch = CompiledSongPatch::compile_initial(&selected).unwrap();
        let full = patch.render_default(range).unwrap();
        let mut sum = vec![0.0; full.samples().len()];
        let normalization = 1.0 / (selected.active_pattern_voices().count() as f64).sqrt();
        for voice in selected.active_pattern_voices() {
            let mut definition = selected.native_project_definition().unwrap();
            definition.version = 2;
            definition.compositions.clear();
            definition.selected_composition = None;
            definition
                .voices
                .retain(|candidate| candidate.id == voice.id);
            let solo =
                meldritch_dsl::ValidatedSong::restore_native_project(&directory.0, &definition)
                    .unwrap();
            let solo = CompiledSongPatch::compile_initial(&solo)
                .unwrap()
                .render_default(range)
                .unwrap();
            for (sum, value) in sum.iter_mut().zip(solo.samples()) {
                *sum += value * normalization;
            }
        }
        for (actual, expected) in full.samples().iter().zip(sum) {
            assert!((actual - expected).abs() < 1e-12);
        }
    }
}

#[test]
fn both_compositions_reopen_in_fresh_processes_without_copying_the_palette() {
    let directory = Directory::new();
    let source = load_native_project(&directory.0, FIXTURE).unwrap();
    assert_eq!(source.synths().len(), 1);
    assert_eq!(source.pattern_voices().len(), 3);
    assert_eq!(source.compositions().len(), 2);
    let range = FrameRange::new(0, 192_000).unwrap();
    let mut store = NativeProjectStore::open(&directory.project()).unwrap();
    let mut prior = None;
    let edited = source
        .set_composition_member("held", "bass-copy", false)
        .unwrap()
        .set_composition_member("held", "pulse-instance", true)
        .unwrap();
    for (base, id) in [(&source, "held"), (&source, "pulse"), (&edited, "held")] {
        let selected = base.select_composition(id).unwrap();
        assert_eq!(source.synths(), selected.synths());
        assert_eq!(source.note_patterns(), selected.note_patterns());
        assert_eq!(source.pattern_voices(), selected.pattern_voices());
        let actual = pcm(&selected, range);
        if let Some(prior) = &prior {
            assert!(actual != *prior, "compositions must sound different");
        }
        prior = Some(actual.clone());
        // Directly compile only declared members as an independent v2 reference.
        let mut reference = selected.native_project_definition().unwrap();
        reference.version = 2;
        reference
            .voices
            .retain(|voice| selected.is_pattern_voice_active(&voice.id));
        reference.compositions.clear();
        reference.selected_composition = None;
        let direct =
            meldritch_dsl::ValidatedSong::restore_native_project(&directory.0, &reference).unwrap();
        assert!(
            actual == pcm(&direct, range),
            "membership must determine the rendered mix"
        );
        store.save(&selected).unwrap();
        let encoded = fs::read_to_string(directory.0.join("main.project.lua")).unwrap();
        assert!(encoded.contains("version = 3"));
        fs::write(directory.0.join("expected.pcm"), actual).unwrap();
        fs::write(
            directory.0.join("expected.fingerprint"),
            format!("{:016x}", selected.fingerprint()),
        )
        .unwrap();
        let child = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "composition_reopen_child",
                "--ignored",
                "--nocapture",
            ])
            .env("MELDRITCH_COMPOSITION_ROOT", &directory.0)
            .env("MELDRITCH_COMPOSITION_ID", id)
            .output()
            .unwrap();
        assert!(
            child.status.success(),
            "{}{}",
            String::from_utf8_lossy(&child.stdout),
            String::from_utf8_lossy(&child.stderr)
        );
    }
}

#[test]
#[ignore = "invoked by fresh-process composition parent"]
fn composition_reopen_child() {
    let root = PathBuf::from(std::env::var_os("MELDRITCH_COMPOSITION_ROOT").unwrap());
    let project = meldritch_dsl::load_project_directory(&root).unwrap();
    let before = fs::read_dir(&root).unwrap().count();
    let store = NativeProjectStore::open(&project).unwrap();
    let source = store.source().unwrap();
    assert_eq!(
        source.selected_composition(),
        Some(std::env::var("MELDRITCH_COMPOSITION_ID").unwrap().as_str())
    );
    assert_eq!(source.compositions().len(), 2);
    assert_eq!(source.synths().len(), 1);
    let (mut app, _) = AppController::new_empty_project(&project).unwrap();
    app.enable_native_project_source(source.clone()).unwrap();
    assert!(app.history().is_empty() && app.output_levels().is_empty());
    assert_eq!(
        app.view_model().transport.state,
        meldritch_audio::transport::TransportState::Stopped
    );
    assert_eq!(
        format!("{:016x}", source.fingerprint()),
        fs::read_to_string(root.join("expected.fingerprint")).unwrap()
    );
    let expected = fs::read(root.join("expected.pcm")).unwrap();
    assert!(pcm(source, FrameRange::new(0, 192_000).unwrap()) == expected);
    let mut split = pcm(source, FrameRange::new(0, 17_123).unwrap());
    split.extend(pcm(source, FrameRange::new(17_123, 192_000).unwrap()));
    assert!(split == expected);
    assert_eq!(fs::read_dir(&root).unwrap().count(), before);
}

#[test]
fn composition_authoring_and_selection_use_existing_source_and_typed_capture() {
    let directory = Directory::new();
    let source = load_native_project(
        &directory.0,
        include_str!("../../../songs/examples/35-project-palette-routing/main.project.lua"),
    )
    .unwrap();
    let mut app = directory.app();
    app.enable_native_project_source(source.clone()).unwrap();
    let mut capture = NativeProjectCapture::new(app.authored_source().unwrap(), 96_000);
    let create = meldritch_tui::project_composition_command(
        KeyEvent::new(KeyCode::Char('b'), KeyModifiers::NONE),
        &app.view_model(),
        &source,
    )
    .unwrap();
    app.dispatch(create).unwrap();
    capture.checkpoint(&app).unwrap();
    assert_eq!(
        app.authored_source().unwrap().selected_composition(),
        Some("composition-1")
    );
    let range = FrameRange::new(0, 96_000).unwrap();
    assert!(pcm(&source, range) == pcm(&app.authored_source().unwrap(), range));
    app.dispatch(AppCommand::CreateComposition(CompositionDefinition {
        id: "composition-2".into(),
        voices: vec![],
    }))
    .unwrap();
    capture.checkpoint(&app).unwrap();
    let selection = meldritch_tui::project_composition_command(
        KeyEvent::new(KeyCode::F(8), KeyModifiers::NONE),
        &app.view_model(),
        &app.authored_source().unwrap(),
    )
    .unwrap();
    app.dispatch(selection).unwrap();
    capture.checkpoint(&app).unwrap();
    assert_eq!(
        app.authored_source().unwrap().selected_composition(),
        Some("composition-2")
    );
    assert!(
        pcm(&app.authored_source().unwrap(), range)
            .iter()
            .all(|byte| *byte == 0)
    );
    assert_eq!(app.authored_source().unwrap().synths(), source.synths());
    capture.finish(&app).unwrap();
    let text = fs::read_to_string(capture.session.as_ref().unwrap().path()).unwrap();
    let mut restored = directory.app();
    restored
        .enable_native_project_source(load_native_capture_baseline(&directory.0, &text).unwrap())
        .unwrap();
    let document: toml::Value = toml::from_str(&text).unwrap();
    for event in document["events"].as_array().unwrap() {
        let envelope =
            NativeCommandEnvelope::decode(&toml::to_string(&event["native"]).unwrap()).unwrap();
        restored.dispatch(envelope.command.into()).unwrap();
        assert_eq!(
            envelope.source_after,
            format!("{:016x}", restored.authored_source().unwrap().fingerprint())
        );
    }
    assert_eq!(
        restored.authored_source().unwrap().fingerprint(),
        app.authored_source().unwrap().fingerprint()
    );
}

#[test]
fn malformed_or_inactive_composition_material_and_pending_play_reject() {
    let directory = Directory::new();
    for invalid in [
        include_str!(
            "../../../songs/examples/35-project-palette-routing/invalid/compositions.project.lua"
        )
        .to_owned(),
        FIXTURE.replace("version = 3", "version = 2"),
        FIXTURE.replace("version = 3", "version = 4"),
        FIXTURE.replace("selected_composition = \"held\",", ""),
        FIXTURE.replace(
            "selected_composition = \"held\"",
            "selected_composition = \"missing\"",
        ),
        FIXTURE.replace("{ id = \"pulse\",", "{ id = \"held\","),
        FIXTURE.replace(
            "voices = { \"pulse-instance\" }",
            "voices = { \"missing\" }",
        ),
        FIXTURE.replace(
            "\"voice-1-instance\", \"bass-copy\"",
            "\"bass-copy\", \"bass-copy\"",
        ),
        FIXTURE.replace(
            "gate_pattern = \"short-rhythm\"",
            "gate_pattern = \"missing\"",
        ),
    ] {
        assert!(load_native_project(&directory.0, &invalid).is_err());
    }
    let source = load_native_project(&directory.0, FIXTURE).unwrap();
    let mut definition = source.native_project_definition().unwrap();
    definition.compositions = (0..33)
        .map(|index| CompositionDefinition {
            id: format!("composition-{index}"),
            voices: vec![],
        })
        .collect();
    assert!(
        meldritch_dsl::ValidatedSong::restore_native_project(&directory.0, &definition).is_err()
    );
    let mut app = directory.app();
    app.enable_native_project_source(source).unwrap();
    let accepted = app.authored_source().unwrap();
    for command in [
        AppCommand::SelectComposition("missing".into()),
        AppCommand::CreateComposition(CompositionDefinition {
            id: "held".into(),
            voices: vec![],
        }),
        AppCommand::CreateComposition(CompositionDefinition {
            id: "bad/id".into(),
            voices: vec![],
        }),
        AppCommand::EditNote {
            pattern: "short-rhythm".into(),
            edit: NoteEdit::Duration {
                id: "note-1".into(),
                duration_ticks: 1440,
            },
        },
    ] {
        assert!(app.dispatch(command).is_err());
        assert!(Arc::ptr_eq(&accepted, &app.authored_source().unwrap()));
        assert!(app.history().is_empty());
    }
    app.dispatch(AppCommand::Play).unwrap();
    assert!(
        app.dispatch(AppCommand::SelectComposition("pulse".into()))
            .is_err()
    );
    assert_eq!(app.history().len(), 1);
    assert!(Arc::ptr_eq(&accepted, &app.authored_source().unwrap()));
    app.dispatch(AppCommand::Stop).unwrap();
    assert!(
        app.dispatch(AppCommand::SelectComposition("pulse".into()))
            .is_err()
    );
    assert_eq!(app.history().len(), 2);
    assert!(Arc::ptr_eq(&accepted, &app.authored_source().unwrap()));
    assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 0);
}
