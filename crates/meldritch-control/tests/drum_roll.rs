#[path = "../examples/support/piano_roll.rs"]
mod support;
use meldritch_app::*;
use meldritch_control::pattern::*;
use meldritch_core::FrameRange;
use meldritch_render::song_host::CompiledSongPatch;
use support::Harness;

fn fixture() -> std::path::PathBuf {
    support::fixture()
        .parent()
        .unwrap()
        .join("31-apc-drum-editor")
}
fn render(song: &ValidatedSong) -> meldritch_audio::AudioBlock {
    CompiledSongPatch::compile_initial(song)
        .unwrap()
        .render_default(FrameRange::new(0, 192000).unwrap())
        .unwrap()
}
#[test]
fn drum_script_toggles_layers_publishes_and_round_trips_explicit_slots() {
    let mut host = Harness::from_fixture(&fixture());
    host.app.dispatch(AppCommand::Play).unwrap();
    host.renderer.render_f64(&mut [0.0; 32]);
    let originals = [
        "main.mlperformance",
        "main.lua",
        "patterns/seed.mlpattern",
        "samples/kit.mlsamples",
    ]
    .map(|p| (p, std::fs::read(host.root.join(p)).unwrap()));
    host.run_script(include_str!(
        "../../../songs/examples/31-apc-drum-editor/scripts/compose.pads"
    ));
    assert!(
        !host
            .performance
            .plane
            .trace()
            .iter()
            .any(|t| t.phase == meldritch_control::Phase::Rejected)
    );
    let view = host.app.view_model().pattern_editor.unwrap();
    assert_eq!(view.pattern.notes.len(), 9);
    assert_eq!(view.pattern.next_note_id, 11);
    let snare = view
        .pattern
        .notes
        .iter()
        .find(|n| n.slot.as_deref() == Some("snare"))
        .unwrap();
    assert!((snare.velocity - 0.9).abs() < 1e-12);
    assert!(
        view.pattern
            .notes
            .iter()
            .any(|n| n.slot.as_deref() == Some("kick") && n.start_tick == 1920)
    );
    let expected = render(&view.source);
    assert!(expected.peak_abs() > 0.01);
    assert!(expected.samples().iter().all(|s| s.is_finite()));
    assert_eq!(host.reader.snapshot().generation(), host.host.generation());
    host.renderer.render_f64(&mut [0.0; 256]);
    assert_eq!(
        host.reader.playback_monitor().generation(),
        Some(host.host.generation())
    );
    for (frame, sample) in expected.samples().iter().enumerate() {
        assert_eq!(
            host.reader.snapshot().frame(frame as u32).unwrap()[0],
            *sample
        );
    }
    assert_eq!(
        host.app.view_model().transport.state,
        meldritch_audio::transport::TransportState::Playing
    );
    let exported = std::fs::read_dir(host.root.join("authored"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let text = std::fs::read_to_string(&exported).unwrap();
    assert!(text.contains("slot = \"snare\""));
    assert_eq!(text, encode_pattern(&view).unwrap());
    assert_eq!(
        text,
        std::fs::read_to_string(fixture().join("edited-pattern.lua")).unwrap()
    );
    assert!(
        load_pattern(
            &host.root,
            &text.replace("slot = \"kick\"", "slot = \"unknown\"")
        )
        .is_err()
    );
    assert!(
        load_pattern(
            &host.root,
            &text.replace("slot = \"snare\"", "slot = \"snare\", slice = \"unknown\"")
        )
        .is_err()
    );
    let (loaded, pattern) =
        load_pattern_file(&host.root, exported.strip_prefix(&host.root).unwrap()).unwrap();
    assert_eq!(pattern, "roll");
    assert_eq!(loaded.fingerprint(), view.source.fingerprint());
    assert_eq!(render(&loaded), expected);
    assert_eq!(loaded.authored_pattern("roll").unwrap(), view.pattern);
    for (p, bytes) in originals {
        assert_eq!(std::fs::read(host.root.join(p)).unwrap(), bytes);
    }
    assert!(
        std::fs::read_dir(host.root.join("performances"))
            .unwrap()
            .next()
            .is_some()
    );
    let apc = host.performance.plane.output("apc-mini").unwrap();
    let tui = host.performance.plane.output("terminal").unwrap();
    for (cell, present) in &apc.cells {
        assert_eq!(present.label, tui.cells[cell].label);
    }
    assert!(apc.cells[&0].label.contains("kick t1920"));
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(100, 24)).unwrap();
    terminal
        .draw(|f| meldritch_tui::draw(f, &host.app.view_model()))
        .unwrap();
    let screen: String = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|c| c.symbol())
        .collect();
    assert!(screen.contains("closed-hat"));
    assert!(screen.contains("Pattern: roll"));
    // Reload uses the same authoring owner and preserves slot metadata on further edits.
    host.app
        .enable_pattern_authoring_from(host.base.clone(), loaded, "roll")
        .unwrap();
    host.run_script("release 0\npress 106\nrelease 106\npress 0\nrelease 0\nexpect_count 8\npress 0\nrelease 0\nexpect_count 9");
    assert_eq!(
        host.app
            .view_model()
            .pattern_editor
            .unwrap()
            .pattern
            .next_note_id,
        12
    );
}

#[test]
fn eight_distinct_one_shots_sum_and_invalid_slot_edits_are_atomic() {
    let base = meldritch_dsl::load_song_directory(fixture()).unwrap();
    let mut together = base.clone();
    let mut sum = vec![0.0; 192000];
    let mut previous = None;
    for (row, slot) in [
        "kick",
        "snare",
        "clap",
        "closed-hat",
        "open-hat",
        "low-tom",
        "high-tom",
        "ride",
    ]
    .iter()
    .enumerate()
    {
        let edit = NoteEdit::InsertSample {
            start_tick: 0,
            duration_ticks: 240,
            pitch: 36 + row as u8,
            velocity: 0.8,
            slot: (*slot).into(),
        };
        let solo = base.edit_note("seed", &edit).unwrap().0;
        let audio = render(&solo);
        assert!(audio.peak_abs() > 0.01);
        assert!(audio.samples().iter().all(|s| s.is_finite()));
        if let Some(previous) = previous {
            assert_ne!(audio, previous);
        }
        for (out, sample) in sum.iter_mut().zip(audio.samples()) {
            *out += sample;
        }
        previous = Some(audio);
        together = together.edit_note("seed", &edit).unwrap().0;
    }
    for (actual, expected) in render(&together).samples().iter().zip(sum) {
        assert!((actual - expected).abs() < 1e-12);
    }
    let mut host = Harness::from_fixture(&fixture());
    let fingerprint = host.base.fingerprint();
    for edit in [
        NoteEdit::InsertSample {
            start_tick: 0,
            duration_ticks: 240,
            pitch: 36,
            velocity: 0.8,
            slot: "missing".into(),
        },
        NoteEdit::Insert {
            start_tick: 0,
            duration_ticks: 240,
            pitch: 36,
            velocity: 0.8,
        },
        NoteEdit::InsertSample {
            start_tick: 3800,
            duration_ticks: 240,
            pitch: 36,
            velocity: 0.8,
            slot: "kick".into(),
        },
    ] {
        assert!(
            host.app
                .dispatch(AppCommand::EditNote {
                    pattern: "seed".into(),
                    edit
                })
                .is_err()
        );
        assert_eq!(
            host.app
                .view_model()
                .pattern_editor
                .unwrap()
                .source
                .fingerprint(),
            fingerprint
        );
        assert_eq!(host.host.generation(), 0);
        assert!(host.app.history().is_empty());
    }
    assert!(
        Harness::new()
            .base
            .edit_note(
                "seed",
                &NoteEdit::InsertSample {
                    start_tick: 0,
                    duration_ticks: 240,
                    pitch: 36,
                    velocity: 0.8,
                    slot: "kick".into()
                }
            )
            .unwrap_err()
            .contains("sampler")
    );
    let view = host.app.view_model().pattern_editor.unwrap();
    let encoded = encode_pattern(&view).unwrap();
    assert!(
        load_pattern(
            &host.root,
            &encoded.replace("next_note_id = 1", "next_note_id = 0")
        )
        .is_err()
    );
    let bank = host.root.join("samples/kit.mlsamples");
    std::fs::write(
        &bank,
        std::fs::read_to_string(&bank)
            .unwrap()
            .replace("one_shot", "loop"),
    )
    .unwrap();
    let looping = meldritch_dsl::load_song_directory(&host.root).unwrap();
    assert!(
        looping
            .edit_note(
                "seed",
                &NoteEdit::InsertSample {
                    start_tick: 0,
                    duration_ticks: 240,
                    pitch: 36,
                    velocity: 0.8,
                    slot: "kick".into()
                }
            )
            .unwrap_err()
            .contains("one-shot")
    );
}

#[test]
fn modifiers_duplicate_notes_and_disconnect_release_do_not_make_extra_hits() {
    let mut host = Harness::from_fixture(&fixture());
    host.run_script("press 0\npress 8\nrelease 0\nrelease 8\nexpect_count 2\npress 114\npress 2\nrelease 2\nrelease 114\nexpect_count 2\npress 0\npress 0\nrelease 0\nexpect_count 1\npress 112\ndisconnect\nconnect\nrelease 112\npress 16\nzero 16\nexpect_count 2\nwait");
    assert_eq!(
        host.app.view_model().pattern_editor.unwrap().pattern.notes[1]
            .slot
            .as_deref(),
        Some("clap")
    );
}

#[test]
fn invalid_lua_slot_requests_are_rejected_without_accepted_edits() {
    use meldritch_control::{ControlPlane, Input, LuaProject, Phase};
    let mut host = Harness::from_fixture(&fixture());
    let source = std::fs::read_to_string(host.root.join("main.lua")).unwrap();
    for file in std::fs::read_dir(fixture().join("invalid")).unwrap() {
        let request = std::fs::read_to_string(file.unwrap().path()).unwrap();
        let callback = format!(
            "local function interact(view, event)\n{request}\nend\nlocal function unused(view,event)"
        );
        let script = source.replace("local function interact(view, event)", &callback);
        let mut plane =
            ControlPlane::new(LuaProject::load(&host.root, &script).unwrap(), &host.app).unwrap();
        plane
            .enqueue(
                "apc-mini",
                0,
                Input::Pad {
                    cell: 0,
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
        assert!(host.app.history().is_empty());
        assert_eq!(
            host.app
                .view_model()
                .pattern_editor
                .unwrap()
                .source
                .fingerprint(),
            host.base.fingerprint()
        );
        assert_eq!(host.host.generation(), 0);
    }
}
