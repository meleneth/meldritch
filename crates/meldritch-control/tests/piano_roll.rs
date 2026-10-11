#[path = "../examples/support/piano_roll.rs"]
mod support;
use meldritch_app::*;
use meldritch_control::{ControlPlane, Input, LuaProject, Phase, pattern::*};
use meldritch_core::FrameRange;
use meldritch_render::song_host::CompiledSongPatch;
use std::path::Path;
use support::Harness;

fn render(song: &ValidatedSong) -> meldritch_audio::AudioBlock {
    CompiledSongPatch::compile_initial(song)
        .unwrap()
        .render_default(FrameRange::new(0, 192000).unwrap())
        .unwrap()
}
fn edit(host: &mut Harness, edit: NoteEdit) -> AppCommandResult {
    let result = host
        .app
        .dispatch(AppCommand::EditNote {
            pattern: "roll".into(),
            edit,
        })
        .unwrap();
    host.host.accepted(&host.app.view_model(), &result).unwrap();
    result
}
fn new_pattern(host: &mut Harness) {
    host.run_script("press 116\nrelease 116");
}

#[test]
fn occupied_tap_toggles_off_but_endpoint_gestures_keep_polyphonic_notes() {
    let mut host = Harness::new();
    new_pattern(&mut host);
    host.run_script(
        "press 0\nrelease 0\npress 16\nrelease 16\npress 32\nrelease 32\nexpect_count 3",
    );
    let chord = host.app.view_model().pattern_editor.unwrap().source;
    assert_eq!(chord.synths()["edited-bass"].polyphony(), 8);
    let audio = render(&chord);
    let mut sum = vec![0.0; audio.samples().len()];
    for keep in ["note-1", "note-2", "note-3"] {
        let mut solo = chord.as_ref().clone();
        for delete in ["note-1", "note-2", "note-3"] {
            if delete != keep {
                solo = solo
                    .edit_note("roll", &NoteEdit::Delete { id: delete.into() })
                    .unwrap()
                    .0;
            }
        }
        for (out, sample) in sum.iter_mut().zip(render(&solo).samples()) {
            *out += sample;
        }
    }
    for (actual, expected) in audio.samples().iter().zip(sum) {
        assert!((actual - expected).abs() < 1e-12);
    }
    host.run_script("press 0\npress 3\nrelease 0\nrelease 3\nexpect_count 3\nexpect_note note-1 C3 0 1920 0.8\npress 0\nrelease 0\nexpect_count 2\npress 0\nzero 0\nexpect_count 3\nexpect_note note-4 C3 0 480 0.8\nwait");
    assert_eq!(host.reader.snapshot().generation(), host.host.generation());
    assert!(render(&host.app.view_model().pattern_editor.unwrap().source).peak_abs() > 0.0);
}

#[test]
fn later_insert_preserves_historical_simultaneous_note_order_and_audio_prefix() {
    let host = Harness::new();
    let path = host.root.join("patterns/seed.mlpattern");
    let mut text = std::fs::read_to_string(&path).unwrap();
    for index in 1..=12 {
        text.push_str(&format!(
            "\n[[events]]\nat = \"1:1:0\"\nnote = \"C3\"\nduration = \"1/8\"\nvelocity = {}\n",
            f64::from(index) / 20.0
        ));
    }
    std::fs::write(path, text).unwrap();
    let original = meldritch_dsl::load_song_directory(&host.root).unwrap();
    let before = original.authored_pattern("seed").unwrap();
    let (edited, _, earliest) = original
        .edit_note(
            "seed",
            &NoteEdit::Insert {
                start_tick: 1920,
                duration_ticks: 480,
                pitch: 50,
                velocity: 0.8,
            },
        )
        .unwrap();
    assert_eq!(earliest, 48000);
    assert_eq!(
        edited.authored_pattern("seed").unwrap().notes[..12],
        before.notes
    );
    assert_eq!(
        render(&original).samples()[..48000],
        render(&edited).samples()[..48000]
    );
}

#[test]
fn composition_script_authors_publishes_projects_and_persists_real_midi() {
    let mut host = Harness::new();
    let original_files = [
        "main.mlperformance",
        "patterns/seed.mlpattern",
        "synths/bass.mlsynth",
        "main.lua",
    ]
    .map(|p| (p, std::fs::read(host.root.join(p)).unwrap()));
    host.app.dispatch(AppCommand::Play).unwrap();
    host.renderer.render_f64(&mut [0.0; 32]);
    host.run_script(include_str!(
        "../../../songs/examples/26-apc-note-pattern/scripts/compose.pads"
    ));
    let view = host.app.view_model().pattern_editor.unwrap();
    assert_eq!(view.pattern.id, "roll");
    assert_eq!(view.pattern.notes.len(), 2);
    assert_eq!(
        view.source.performance().tracks()[0].initial_pattern(),
        Some("roll")
    );
    assert!(render(&view.source).peak_abs() > 0.01);
    assert_eq!(host.reader.snapshot().generation(), host.host.generation());
    let published = host.reader.snapshot();
    let expected_audio = render(&view.source);
    for (frame, sample) in expected_audio.samples().iter().enumerate() {
        assert_eq!(published.frame(frame as u32).unwrap()[0], *sample);
    }
    assert_eq!(
        host.app.view_model().transport.state,
        meldritch_audio::transport::TransportState::Playing
    );
    host.renderer.render_f64(&mut [0.0; 256]);
    assert_eq!(
        host.reader.playback_monitor().generation(),
        Some(host.host.generation())
    );
    let a = host.performance.plane.output("apc-mini").unwrap();
    let t = host.performance.plane.output("terminal").unwrap();
    for (cell, presentation) in &a.cells {
        assert_eq!(presentation.label, t.cells[cell].label);
        assert_eq!(presentation.color, t.cells[cell].color);
    }
    assert!(a.cells[&1].label.contains("body C3")); // cross-page held note
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(80, 24)).unwrap();
    terminal
        .draw(|f| meldritch_tui::draw(f, &host.app.view_model()))
        .unwrap();
    let text: String = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|c| c.symbol())
        .collect();
    assert!(text.contains("Pattern: roll"));
    assert!(text.contains("note-1"));
    assert!(text.contains("4800"));
    for (path, bytes) in original_files {
        assert_eq!(std::fs::read(host.root.join(path)).unwrap(), bytes);
    }
    let exported = std::fs::read_dir(host.root.join("authored"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    assert!(
        exported
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("pattern-")
    );
    let (loaded, id) =
        load_pattern_file(&host.root, exported.strip_prefix(&host.root).unwrap()).unwrap();
    assert_eq!(id, "roll");
    assert_eq!(loaded.fingerprint(), view.source.fingerprint());
    assert_eq!(render(&loaded), render(&view.source));
    let journal_before = std::fs::read(host.capture.path()).unwrap();
    let saved = save_pattern(&view).unwrap();
    assert_eq!(std::fs::read(host.capture.path()).unwrap(), journal_before);
    assert_eq!(
        std::fs::read(&saved).unwrap(),
        std::fs::read(exported).unwrap()
    );
    let mut fresh = Harness::new();
    fresh
        .app
        .enable_pattern_authoring_from(fresh.base.clone(), loaded, &id)
        .unwrap();
    assert_eq!(
        encode_pattern(&fresh.app.view_model().pattern_editor.unwrap()).unwrap(),
        encode_pattern(&view).unwrap()
    );
    let patch = CompiledSongPatch::compile_initial(&view.source).unwrap();
    let first = patch
        .render_default(FrameRange::new(0, 17003).unwrap())
        .unwrap();
    let second = patch
        .render_default(FrameRange::new(17003, 192000).unwrap())
        .unwrap();
    assert_eq!(
        [first.samples(), second.samples()].concat(),
        render(&view.source).samples()
    );
    assert!(host.capture.events.iter().any(|e| e.kind == "note_pattern"));
    assert!(
        !host
            .performance
            .plane
            .trace()
            .iter()
            .any(|t| t.phase == Phase::Rejected)
    );
}

#[test]
fn release_order_modifiers_interruptions_and_rewind_never_create_accidental_notes() {
    let mut host = Harness::new();
    host.run_script(include_str!(
        "../../../songs/examples/27-held-pad-duration/interrupted.pads"
    ));
    let history = host.app.history().len();
    host.run_script("release 0\nzero 4\nrelease 114");
    assert_eq!(host.app.history().len(), history);
    // Earlier endpoint is consumed, never inserted; same for another pitch row.
    host.run_script("press 5\nrelease 5\npress 5\npress 2\nrelease 2\npress 14\nrelease 14\nrelease 5\nexpect_count 2");
    assert_eq!(
        host.app.view_model().pattern_editor.unwrap().pattern.notes[1].duration_ticks,
        480
    );
    // Pitch-range change interrupts a hold until its original start is released.
    host.run_script("press 5\npress 105\nrelease 105\npress 105\nrelease 105\npress 104\nrelease 104\npress 1\nrelease 1\nrelease 5\nexpect_count 2\npress 1\nrelease 1\nexpect_count 3\nexpect_note note-3 G#3 480 480 0.8");
    // Explicit modifier on an empty pad makes no note, in either release order.
    host.run_script("press 112\npress 6\nrelease 112\nrelease 6\npress 113\npress 7\nrelease 7\nrelease 113\nexpect_count 3");
    assert!(!host.midi(&[0x91, 0, 127]));
    assert!(!host.midi(&[0x90, 0]));
    assert!(!host.midi(&[0x90, 128, 127]));
    // Queued endpoints must be discarded under backpressure cancellation.
    host.run_script("press 104\nrelease 104\npress 0");
    host.performance
        .plane
        .enqueue(
            "apc-mini",
            0,
            Input::Pad {
                cell: 7,
                pressed: true,
            },
        )
        .unwrap();
    host.performance
        .cancel("apc-mini", &mut host.app, &mut host.host, &mut host.capture)
        .unwrap();
    host.run_script("release 7\nrelease 0\nexpect_note note-1 C3 0 2400 0.8");
    // Actual observation frame can move backwards after rewind/wrap. Ordered
    // physical inputs remain valid; scheduled Named/legacy Press stays strict.
    host.performance
        .pad(
            "apc-mini",
            0,
            true,
            50000,
            &mut host.app,
            &mut host.host,
            &mut host.capture,
        )
        .unwrap();
    host.app.dispatch(AppCommand::Rewind).unwrap();
    host.renderer.render_f64(&mut [0.0; 1]);
    host.performance
        .pad(
            "apc-mini",
            3,
            true,
            0,
            &mut host.app,
            &mut host.host,
            &mut host.capture,
        )
        .unwrap();
    host.performance
        .pad(
            "apc-mini",
            0,
            false,
            0,
            &mut host.app,
            &mut host.host,
            &mut host.capture,
        )
        .unwrap();
    host.performance
        .pad(
            "apc-mini",
            3,
            false,
            0,
            &mut host.app,
            &mut host.host,
            &mut host.capture,
        )
        .unwrap();
    assert_eq!(
        host.app
            .view_model()
            .pattern_editor
            .unwrap()
            .pattern
            .notes
            .iter()
            .find(|n| n.id == "note-1")
            .unwrap()
            .duration_ticks,
        1920
    );
}

#[test]
fn deleted_selection_cancels_held_gesture_and_note_ids_are_not_reused() {
    let mut host = Harness::new();
    new_pattern(&mut host);
    host.run_script("press 0\nrelease 0\npress 0");
    edit(
        &mut host,
        NoteEdit::Delete {
            id: "note-1".into(),
        },
    );
    host.performance.plane.refresh(&host.app).unwrap();
    host.run_script("press 3\nrelease 0\nrelease 3\nexpect_count 0\npress 0\nrelease 0\nexpect_note note-2 C3 0 480 0.8");
    let view = host.app.view_model().pattern_editor.unwrap();
    let source = encode_pattern(&view).unwrap();
    let (loaded, _) = load_pattern(&host.root, &source).unwrap();
    let (next, id, _) = loaded
        .edit_note(
            "roll",
            &NoteEdit::Insert {
                start_tick: 960,
                duration_ticks: 480,
                pitch: 50,
                velocity: 0.5,
            },
        )
        .unwrap();
    assert_eq!(id.as_deref(), Some("note-3"));
    assert_eq!(next.authored_pattern("roll").unwrap().notes.len(), 2);
    host.run_script("press 0");
    host.app
        .dispatch(AppCommand::CreateNotePattern {
            track: "bass".into(),
            id: "other".into(),
            length_ticks: 7680,
            looped: true,
        })
        .unwrap();
    host.performance.plane.refresh(&host.app).unwrap();
    host.run_script("press 3\nrelease 3\nrelease 0\nexpect_count 0");
}

#[test]
fn typed_ranges_rejections_overlap_and_roundtrip_preserve_authority() {
    let mut host = Harness::new();
    new_pattern(&mut host);
    host.wait();
    let first = edit(
        &mut host,
        NoteEdit::Insert {
            start_tick: 1920,
            duration_ticks: 480,
            pitch: 48,
            velocity: 0.8,
        },
    );
    let AppCommandResult::PatternEdited { dirty_ranges, .. } = first else {
        panic!()
    };
    assert_eq!(dirty_ranges, vec![FrameRange::new(48000, 192000).unwrap()]);
    assert_eq!(
        host.app.history().back().unwrap().dirty_ranges[0].range(),
        dirty_ranges[0]
    );
    host.wait();
    let view = host.app.view_model().pattern_editor.unwrap();
    let original = render(&view.source);
    assert!(original.samples()[..48000].iter().all(|v| *v == 0.0));
    for invalid in [
        NoteEdit::Insert {
            start_tick: 7680,
            duration_ticks: 480,
            pitch: 48,
            velocity: 0.8,
        },
        NoteEdit::Insert {
            start_tick: 0,
            duration_ticks: 0,
            pitch: 48,
            velocity: 0.8,
        },
        NoteEdit::Insert {
            start_tick: 0,
            duration_ticks: u64::MAX,
            pitch: 48,
            velocity: 0.8,
        },
        NoteEdit::Insert {
            start_tick: 0,
            duration_ticks: 480,
            pitch: 128,
            velocity: 0.8,
        },
        NoteEdit::Duration {
            id: "missing".into(),
            duration_ticks: 480,
        },
        NoteEdit::Duration {
            id: "note-1".into(),
            duration_ticks: 7680,
        },
        NoteEdit::Velocity {
            id: "note-1".into(),
            velocity: f64::NAN,
        },
        NoteEdit::Velocity {
            id: "note-1".into(),
            velocity: 1.1,
        },
        NoteEdit::Delete {
            id: "missing".into(),
        },
    ] {
        let history = host.app.history().len();
        let generation = host.host.generation();
        assert!(
            host.app
                .dispatch(AppCommand::EditNote {
                    pattern: "roll".into(),
                    edit: invalid
                })
                .is_err()
        );
        assert_eq!(host.app.history().len(), history);
        assert_eq!(host.host.generation(), generation);
        assert_eq!(
            host.app
                .view_model()
                .pattern_editor
                .unwrap()
                .source
                .fingerprint(),
            view.source.fingerprint()
        );
    }
    assert!(
        host.app
            .dispatch(AppCommand::CreateNotePattern {
                track: "bass".into(),
                id: "seed".into(),
                length_ticks: 7680,
                looped: true
            })
            .is_err()
    );
    assert!(
        host.app
            .dispatch(AppCommand::EditNote {
                pattern: "missing".into(),
                edit: NoteEdit::Delete {
                    id: "note-1".into()
                }
            })
            .is_err()
    );
    let noop = edit(
        &mut host,
        NoteEdit::Velocity {
            id: "note-1".into(),
            velocity: 0.8,
        },
    );
    assert!(matches!(
        noop,
        AppCommandResult::PatternEdited { changed: false, .. }
    ));
    // Adjacent starts have independent identity; extending creates an overlap.
    edit(
        &mut host,
        NoteEdit::Insert {
            start_tick: 2400,
            duration_ticks: 480,
            pitch: 48,
            velocity: 0.6,
        },
    );
    edit(
        &mut host,
        NoteEdit::Duration {
            id: "note-1".into(),
            duration_ticks: 960,
        },
    );
    host.wait();
    let view = host.app.view_model().pattern_editor.unwrap();
    assert_eq!(view.pattern.notes.len(), 2);
    assert_eq!(
        &original.samples()[..48000],
        &render(&view.source).samples()[..48000]
    );
    let (restored, _) = load_pattern(&host.root, &encode_pattern(&view).unwrap()).unwrap();
    assert_eq!(render(&restored), render(&view.source));
    host.performance.plane.refresh(&host.app).unwrap();
    assert!(
        host.performance.plane.output("apc-mini").unwrap().cells[&5]
            .label
            .contains("overlap")
    );
}

#[test]
fn persistence_validation_failure_readonly_and_instrument_source_ownership() {
    let mut host = Harness::new();
    new_pattern(&mut host);
    host.run_script("press 0\nrelease 0");
    let view = host.app.view_model().pattern_editor.unwrap();
    let text = encode_pattern(&view).unwrap();
    for source in [
        text.replace("version = 1", "version = 9"),
        text.replace("\"bass\"", "\"unknown\""),
        text.replace("duration_ticks = 480", "duration_ticks = 0"),
        text.replace("velocity = 0.8", "velocity = true"),
        text.replace("note = \"C3\"", "note = \"unknown\""),
        text.replace("next_note_id = 2", "next_note_id = 1"),
        text.replace("version = 1,", "version = 1, extra = true,"),
    ] {
        assert!(load_pattern(&host.root, &source).is_err());
    }
    assert!(load_pattern_file(&host.root, Path::new("../escape.lua")).is_err());
    std::fs::write(host.root.join("authored"), "filesystem obstruction").unwrap();
    let result = host.app.dispatch(AppCommand::SavePattern).unwrap();
    assert!(host.host.accepted(&host.app.view_model(), &result).is_err());
    host.host.refresh_view(&mut host.app);
    assert!(
        host.app
            .view_model()
            .pattern_editor
            .unwrap()
            .save_status
            .unwrap()
            .contains("unsaved")
    );
    assert_eq!(
        host.app
            .view_model()
            .pattern_editor
            .unwrap()
            .source
            .fingerprint(),
        view.source.fingerprint()
    );
    let source = std::fs::read_to_string(host.root.join("main.lua"))
        .unwrap()
        .replace(
            "local function interact(view, event)",
            "local function interact(view, event)\n view.pattern.notes[1].velocity = 0.1",
        );
    let mut plane =
        ControlPlane::new(LuaProject::load(&host.root, &source).unwrap(), &host.app).unwrap();
    plane
        .enqueue(
            "apc-mini",
            0,
            Input::Pad {
                cell: 1,
                pressed: true,
            },
        )
        .unwrap();
    plane.pump(&mut host.app);
    assert!(
        plane
            .trace()
            .iter()
            .any(|t| t.phase == Phase::Rejected && t.detail.contains("read-only"))
    );
    assert_eq!(
        host.app
            .view_model()
            .pattern_editor
            .unwrap()
            .source
            .fingerprint(),
        view.source.fingerprint()
    );
    let instrument = host.app.view_model().instrument_editor.unwrap();
    let parameter = instrument
        .parameters
        .iter()
        .find(|p| p.id.parameter == InstrumentParameter::Release)
        .unwrap();
    host.app
        .dispatch(AppCommand::EditInstrument(InstrumentEdit {
            target: parameter.id.clone(),
            value: InstrumentValue::Number(0.3),
        }))
        .unwrap();
    assert_eq!(
        host.app.view_model().pattern_editor.unwrap().pattern,
        view.pattern
    ); // no separate source model
    assert!(
        encode_pattern(&host.app.view_model().pattern_editor.unwrap())
            .unwrap_err()
            .contains("unrelated")
    );
}

#[test]
fn logical_tui_events_and_keyboard_inspection_share_notes_and_golden_export() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    let script = include_str!("../../../songs/examples/26-apc-note-pattern/scripts/compose.pads");
    let mut apc = Harness::new();
    apc.run_script(script);
    let mut tui = Harness::new();
    for line in script
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
    {
        let fields: Vec<_> = line.split_whitespace().collect();
        if matches!(fields[0], "press" | "release" | "zero") {
            tui.pad("terminal", fields[1].parse().unwrap(), fields[0] == "press");
        }
    }
    let a = apc.app.view_model().pattern_editor.unwrap();
    let t = tui.app.view_model().pattern_editor.unwrap();
    assert_eq!(a.pattern, t.pattern);
    assert_eq!(a.source.fingerprint(), t.source.fingerprint());
    let encoded = encode_pattern(&a).unwrap();
    assert_eq!(
        encoded,
        std::fs::read_to_string(support::fixture().join("edited-pattern.lua")).unwrap()
    );
    let (loaded, pattern) =
        load_pattern_file(support::fixture(), Path::new("edited-pattern.lua")).unwrap();
    assert_eq!(pattern, "roll");
    assert_eq!(render(&loaded), render(&a.source));
    let action = meldritch_tui::map_key_for_view(
        KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL),
        &meldritch_core::Step::new(48),
        &apc.app.view_model(),
    )
    .unwrap();
    assert_eq!(
        action,
        meldritch_tui::TuiAction::Input(AppInput::SavePattern)
    );
    assert!(
        meldritch_tui::map_key_for_view(
            KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE),
            &meldritch_core::Step::new(48),
            &apc.app.view_model()
        )
        .is_none()
    );
    assert_eq!(
        apc.app.view_model().pattern_editor.unwrap().selected_note,
        a.selected_note
    );
}

#[test]
fn each_note_edit_changes_audio_and_only_latest_generation_is_published() {
    let mut host = Harness::new();
    new_pattern(&mut host);
    let mut before = render(&host.app.view_model().pattern_editor.unwrap().source);
    for change in [
        NoteEdit::Insert {
            start_tick: 0,
            duration_ticks: 480,
            pitch: 48,
            velocity: 0.8,
        },
        NoteEdit::Duration {
            id: "note-1".into(),
            duration_ticks: 1920,
        },
        NoteEdit::Velocity {
            id: "note-1".into(),
            velocity: 0.3,
        },
        NoteEdit::Delete {
            id: "note-1".into(),
        },
    ] {
        edit(&mut host, change);
        let after = render(&host.app.view_model().pattern_editor.unwrap().source);
        assert_ne!(after.samples(), before.samples());
        assert!(after.samples().iter().all(|s| s.is_finite()));
        before = after;
    }
    host.wait();
    assert_eq!(host.reader.snapshot().generation(), 5);
    assert_eq!(before.peak_abs(), 0.0);
}

#[test]
fn invalid_lua_note_requests_leave_state_publication_and_accepted_facts_unchanged() {
    let mut host = Harness::new();
    new_pattern(&mut host);
    host.run_script("press 0\nrelease 0");
    host.wait();
    let source = std::fs::read_to_string(host.root.join("main.lua")).unwrap();
    let fingerprint = host
        .app
        .view_model()
        .pattern_editor
        .unwrap()
        .source
        .fingerprint();
    let history = host.app.history().len();
    let generation = host.host.generation();
    let publication = host.reader.snapshot();
    for entry in std::fs::read_dir(support::fixture().join("invalid")).unwrap() {
        let request = std::fs::read_to_string(entry.unwrap().path()).unwrap();
        let callback = format!(
            "local function interact(view, event)\n {}\nend\nlocal function unused(view,event)",
            request
        );
        let script = source.replace("local function interact(view, event)", &callback);
        let mut plane =
            ControlPlane::new(LuaProject::load(&host.root, &script).unwrap(), &host.app).unwrap();
        plane
            .enqueue(
                "apc-mini",
                0,
                Input::Pad {
                    cell: 1,
                    pressed: true,
                },
            )
            .unwrap();
        plane.pump_with_observer(&mut host.app, |app, _, result, _, _| {
            host.host.accepted(&app.view_model(), result)?;
            Ok(None)
        });
        assert!(plane.trace().iter().any(|t| t.phase == Phase::Rejected));
        assert!(plane.take_journal().commands.is_empty());
        assert_eq!(host.app.history().len(), history);
        assert_eq!(host.host.generation(), generation);
        assert_eq!(
            host.app
                .view_model()
                .pattern_editor
                .unwrap()
                .source
                .fingerprint(),
            fingerprint
        );
        assert!(std::sync::Arc::ptr_eq(
            &publication,
            &host.reader.snapshot()
        ));
    }
}
