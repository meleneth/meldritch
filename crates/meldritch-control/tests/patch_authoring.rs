#![allow(dead_code)]
#[path = "../examples/support/piano_roll.rs"]
mod support;
use meldritch_app::AppCommand;
use meldritch_control::instrument::{encode_instrument, load_instrument, save_instrument};
use meldritch_core::FrameRange;
use meldritch_dsl::{
    DependentCables, InstrumentParameter, InstrumentValue, ModuleKind, PatchEdit, PatchOperation,
};
use meldritch_render::song_host::CompiledSongPatch;
use std::path::PathBuf;
fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../songs/examples/30-compose-typed-patch")
}
fn command(operations: Vec<PatchOperation>) -> AppCommand {
    AppCommand::EditPatch(PatchEdit {
        synth: "edited-bass".into(),
        operations,
    })
}
fn connect(from: &str, to: &str) -> PatchOperation {
    PatchOperation::Connect {
        from: from.into(),
        to: to.into(),
    }
}
fn disconnect(from: &str, to: &str) -> PatchOperation {
    PatchOperation::Disconnect {
        from: from.into(),
        to: to.into(),
    }
}
fn keyboard(
    h: &mut support::Harness,
    code: crossterm::event::KeyCode,
    modifiers: crossterm::event::KeyModifiers,
) -> Result<meldritch_app::AppCommandResult, meldritch_app::AppCommandError> {
    let action = meldritch_tui::map_key_for_view(
        crossterm::event::KeyEvent::new(code, modifiers),
        &meldritch_core::Step::new(36),
        &h.app.view_model(),
    )
    .unwrap();
    let meldritch_tui::TuiAction::Input(input) = action else {
        panic!("typed key input")
    };
    h.app.handle_input(input)
}
fn line(
    h: &mut support::Harness,
    text: &str,
) -> Result<meldritch_app::AppCommandResult, meldritch_app::AppCommandError> {
    use crossterm::event::{KeyCode, KeyModifiers};
    for c in text.chars() {
        keyboard(h, KeyCode::Char(c), KeyModifiers::NONE).unwrap();
    }
    keyboard(h, KeyCode::Enter, KeyModifiers::NONE)
}
#[test]
fn keyboard_composes_full_draft_then_publishes_saves_and_reloads() {
    use crossterm::event::{KeyCode, KeyModifiers};
    let mut h = support::Harness::from_fixture(&fixture());
    let before = h.reader.snapshot();
    let base_fp = h.base.fingerprint();
    h.app.dispatch(AppCommand::Play).unwrap();
    h.renderer.render_f64(&mut [0.0; 16]);
    keyboard(&mut h, KeyCode::Char('e'), KeyModifiers::CONTROL).unwrap();
    keyboard(&mut h, KeyCode::Char('g'), KeyModifiers::CONTROL).unwrap();
    let script = std::fs::read_to_string(fixture().join("keyboard-script.txt")).unwrap();
    for text in script
        .lines()
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
    {
        let result = line(&mut h, text).unwrap();
        if text == "commit" {
            assert!(h.host.accepted(&h.app.view_model(), &result).unwrap());
        } else {
            assert_eq!(
                h.app
                    .view_model()
                    .instrument_editor
                    .unwrap()
                    .source
                    .fingerprint(),
                base_fp
            );
            assert!(std::sync::Arc::ptr_eq(&before, &h.reader.snapshot()));
        }
    }
    h.wait();
    let mut played = [0.0; 128];
    h.renderer.render_f64(&mut played);
    assert!(played.iter().any(|s| *s != 0.0));
    let view = h.app.view_model().instrument_editor.unwrap();
    assert_eq!(view.source.synths()["edited-bass"].modules().len(), 5);
    assert!(
        view.source.synths()["edited-bass"]
            .modules()
            .iter()
            .any(|m| m.id() == "source")
    );
    let encoded = encode_instrument(&view).unwrap();
    assert!(encoded.contains("version = 2") && encoded.contains("source.audio"));
    let file = save_instrument(&view).unwrap();
    assert_eq!(std::fs::read_to_string(&file).unwrap(), encoded);
    let restored = load_instrument(&h.root, &encoded).unwrap();
    let example = load_instrument(
        &h.root,
        &std::fs::read_to_string(fixture().join("edited-instrument.lua")).unwrap(),
    )
    .unwrap();
    assert_eq!(example.synths(), restored.synths());
    assert_eq!(restored.synths(), view.source.synths());
    assert_eq!(restored.fingerprint(), view.source.fingerprint());
    let render = |source| {
        CompiledSongPatch::compile_initial(source)
            .unwrap()
            .render_default(FrameRange::new(0, 96000).unwrap())
            .unwrap()
    };
    let audio = render(&restored);
    assert_eq!(audio, render(&view.source));
    assert!(audio.peak_abs() > 0.01 && audio.samples().iter().all(|s| s.is_finite()));
    let patch = CompiledSongPatch::compile_initial(&restored).unwrap();
    let a = patch
        .render_default(FrameRange::new(0, 333).unwrap())
        .unwrap();
    let b = patch
        .render_default(FrameRange::new(333, 96000).unwrap())
        .unwrap();
    assert_eq!(audio.samples(), [a.samples(), b.samples()].concat());
    h.app
        .enable_instrument_editor_from(h.base.clone(), restored)
        .unwrap();
    // Parameter edits after topology authoring must retain the new graph.
    h.app
        .dispatch(command(vec![PatchOperation::SetParameter {
            module: "tone".into(),
            parameter: InstrumentParameter::CutoffHz,
            value: InstrumentValue::Number(700.0),
        }]))
        .unwrap();
    assert_eq!(
        h.app
            .view_model()
            .instrument_editor
            .unwrap()
            .source
            .synths()["edited-bass"]
            .modules()[0]
            .id(),
        "source"
    );
}
#[test]
fn lua_composition_and_native_inspection_use_the_same_graph() {
    let mut h = support::Harness::from_fixture(&fixture());
    h.pad("terminal", 0, true);
    h.pad("terminal", 0, false);
    h.wait();
    let view = h.app.view_model().instrument_editor.unwrap();
    assert!(
        view.source.synths()["edited-bass"]
            .modules()
            .iter()
            .any(|m| m.id() == "tone")
    );
    h.app.dispatch(AppCommand::TogglePatchEditor).unwrap();
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(100, 40)).unwrap();
    terminal
        .draw(|f| meldritch_tui::draw(f, &h.app.view_model()))
        .unwrap();
    let text: String = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|c| c.symbol())
        .collect();
    assert!(text.contains("source (oscillator)") && text.contains("contour.control -> gain.level"));
}
#[test]
fn disconnect_is_draft_only_and_failed_commit_retains_last_revision() {
    let mut h = support::Harness::from_fixture(&fixture());
    h.app.dispatch(AppCommand::TogglePatchEditor).unwrap();
    line(&mut h, "disconnect filter.audio out.audio").unwrap();
    assert!(line(&mut h, "commit").is_err());
    assert_eq!(
        h.app
            .view_model()
            .instrument_editor
            .unwrap()
            .source
            .fingerprint(),
        h.base.fingerprint()
    );
    // Failed commit leaves text intact; clear command input before reconnecting.
    for _ in 0..6 {
        h.app.dispatch(AppCommand::PatchBackspace).unwrap();
    }
    line(&mut h, "connect filter.audio out.audio").unwrap();
    let result = line(&mut h, "commit").unwrap();
    assert!(!h.host.accepted(&h.app.view_model(), &result).unwrap());
}
#[test]
fn invalid_batches_reject_atomically_with_actionable_diagnostics() {
    let mut h = support::Harness::from_fixture(&fixture());
    for (operations, diagnostic) in [
        (
            vec![PatchOperation::Instantiate {
                id: "osc".into(),
                kind: ModuleKind::Oscillator,
            }],
            "duplicate",
        ),
        (
            vec![PatchOperation::Remove {
                id: "filter".into(),
                cables: DependentCables::Reject,
            }],
            "dependent",
        ),
        (
            vec![PatchOperation::Remove {
                id: "filter".into(),
                cables: DependentCables::Disconnect,
            }],
            "MissingCable",
        ),
        (vec![connect("osc.missing", "env.missing")], "port"),
        (
            vec![
                disconnect("filter.audio", "out.audio"),
                connect("input.pitch", "out.audio"),
            ],
            "incompatible",
        ),
        (
            vec![PatchOperation::Instantiate {
                id: "second".into(),
                kind: ModuleKind::Oscillator,
            }],
            "MultipleModules",
        ),
        (
            vec![
                disconnect("osc.audio", "amp.audio"),
                connect("filter.audio", "amp.audio"),
            ],
            "cycle",
        ),
        (vec![connect("osc.audio", "amp.audio")], "already"),
        (vec![disconnect("missing.audio", "out.audio")], "missing"),
        (
            vec![PatchOperation::Instantiate {
                id: "delay".into(),
                kind: ModuleKind::TempoDelay,
            }],
            "DSP",
        ),
    ] {
        let error = format!("{:?}", h.app.dispatch(command(operations)).unwrap_err());
        assert!(error.contains(diagnostic), "{diagnostic}: {error}");
        assert_eq!(
            h.app
                .view_model()
                .instrument_editor
                .unwrap()
                .source
                .fingerprint(),
            h.base.fingerprint()
        );
        assert!(h.host.poll_publish().unwrap().is_none());
    }
}
#[test]
fn typed_plaits_replacement_is_inspectable_editable_and_persistent() {
    let mut h = support::Harness::from_fixture(&fixture());
    h.app
        .dispatch(command(vec![
            PatchOperation::Remove {
                id: "osc".into(),
                cables: DependentCables::Disconnect,
            },
            PatchOperation::Instantiate {
                id: "osc".into(),
                kind: ModuleKind::Plaits,
            },
            connect("input.pitch", "osc.pitch"),
            connect("osc.aux", "amp.audio"),
        ]))
        .unwrap();
    let view = h.app.view_model().instrument_editor.unwrap();
    assert_eq!(
        view.source.synths()["edited-bass"]
            .modules()
            .last()
            .unwrap()
            .kind(),
        ModuleKind::Plaits
    );
    let restored = load_instrument(&h.root, &encode_instrument(&view).unwrap()).unwrap();
    assert_eq!(restored.fingerprint(), view.source.fingerprint());
}

#[test]
fn stale_keyboard_drafts_require_restage_and_prompt_remains_visible_at_80x24() {
    let mut h = support::Harness::from_fixture(&fixture());
    h.app.dispatch(AppCommand::TogglePatchEditor).unwrap();
    line(&mut h, "disconnect filter.audio out.audio").unwrap();
    h.app
        .dispatch(command(vec![PatchOperation::SetParameter {
            module: "filter".into(),
            parameter: InstrumentParameter::CutoffHz,
            value: InstrumentValue::Number(900.0),
        }]))
        .unwrap();
    let error = format!("{:?}", line(&mut h, "commit").unwrap_err());
    assert!(error.contains("base changed"));
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(80, 24)).unwrap();
    terminal
        .draw(|f| meldritch_tui::draw(f, &h.app.view_model()))
        .unwrap();
    let text: String = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|c| c.symbol())
        .collect();
    assert!(text.contains("> commit"));
}

#[test]
fn graph_reload_rejects_forged_ports_versions_ranges_and_unrelated_state() {
    let mut h = support::Harness::from_fixture(&fixture());
    h.pad("terminal", 0, true);
    h.pad("terminal", 0, false);
    h.wait();
    let source = encode_instrument(&h.app.view_model().instrument_editor.unwrap()).unwrap();
    for invalid in [
        source.replace("version = 2", "version = 99"),
        source.replace("signal = \"audio\"", "signal = \"pitch\""),
        source.replace("from = \"source.audio\"", "from = \"missing.audio\""),
        source.replace("cutoff_hz = 800.0", "cutoff_hz = -1.0"),
        source.replace("polyphony = 1", "polyphony = 8"),
        source.replace("kind = \"oscillator\"", "kind = \"unknown\""),
        source.replace("kind = \"vca\"", "kind = \"vca\", attack = 0.1"),
        source.replace("channels = 1", "channels = 2"),
    ] {
        assert!(load_instrument(&h.root, &invalid).is_err(), "{invalid}");
    }
    let one = save_instrument(&h.app.view_model().instrument_editor.unwrap()).unwrap();
    let two = save_instrument(&h.app.view_model().instrument_editor.unwrap()).unwrap();
    assert_ne!(one, two);
    assert_eq!(
        std::fs::read_to_string(h.root.join("main.mlperformance")).unwrap(),
        std::fs::read_to_string(fixture().join("main.mlperformance")).unwrap()
    );
}

#[test]
fn checked_in_invalid_lua_requests_reject_without_source_changes() {
    use mlua::LuaSerdeExt;
    let mut h = support::Harness::from_fixture(&fixture());
    let lua = mlua::Lua::new();
    for name in [
        "duplicate-module",
        "incompatible-cable",
        "cycle",
        "unsupported-source",
    ] {
        let source =
            std::fs::read_to_string(fixture().join(format!("invalid/{name}.lua"))).unwrap();
        let value = lua.load(source).eval::<mlua::Value>().unwrap();
        let request: meldritch_control::CommandRequest = lua.from_value(value).unwrap();
        assert!(h.app.dispatch(request.command().unwrap()).is_err());
        assert_eq!(
            h.app
                .view_model()
                .instrument_editor
                .unwrap()
                .source
                .fingerprint(),
            h.base.fingerprint()
        );
        assert!(h.host.poll_publish().unwrap().is_none());
    }
}

#[test]
fn patch_replacement_cannot_bypass_authored_automation_ownership() {
    let base =
        meldritch_dsl::load_song_directory(fixture().with_file_name("02-synth-parameter-pattern"))
            .unwrap();
    let next = base
        .preview_patch(&PatchEdit {
            synth: "swept-bass".into(),
            operations: vec![
                PatchOperation::Remove {
                    id: "filter".into(),
                    cables: DependentCables::Disconnect,
                },
                PatchOperation::Instantiate {
                    id: "filter".into(),
                    kind: ModuleKind::LowPass,
                },
                connect("osc.audio", "filter.audio"),
                connect("filter.audio", "amp.audio"),
            ],
        })
        .unwrap();
    let error = base
        .replace_patch(next.synths()["swept-bass"].clone())
        .unwrap_err();
    assert!(error.contains("owned by authored automation"), "{error}");
}

#[test]
fn patch_parameter_edits_rearm_observing_faders_and_protect_catalog_references() {
    let mut h =
        support::Harness::from_fixture(&fixture().with_file_name("28-author-fader-mappings"));
    h.app.observe_assignment("main-1", 64, true);
    h.app
        .dispatch(AppCommand::EditPatch(PatchEdit {
            synth: "bass".into(),
            operations: vec![PatchOperation::SetParameter {
                module: "filter".into(),
                parameter: InstrumentParameter::CutoffHz,
                value: InstrumentValue::Number(900.0),
            }],
        }))
        .unwrap();
    assert_eq!(
        h.app.assignment_view().unwrap().observations["main-1"],
        (64, false)
    );
    let error = h
        .app
        .dispatch(AppCommand::EditPatch(PatchEdit {
            synth: "bass".into(),
            operations: vec![
                PatchOperation::Remove {
                    id: "filter".into(),
                    cables: DependentCables::Disconnect,
                },
                connect("amp.audio", "out.audio"),
            ],
        }))
        .unwrap_err();
    assert!(format!("{error:?}").contains("parameter"));
}

#[test]
fn structural_defaults_are_not_lost_when_module_ids_and_cables_stay_equal() {
    let mut h = support::Harness::from_fixture(&fixture());
    let path = h.root.join("synths/edited-bass.mlsynth");
    let source = std::fs::read_to_string(&path).unwrap();
    std::fs::write(
        &path,
        source.replace(
            "waveform = \"saw\"",
            "waveform = \"saw\"\nfrequency_hz = 123.0",
        ),
    )
    .unwrap();
    let base = meldritch_dsl::load_song_directory(&h.root).unwrap();
    let mut graph = base
        .edit_patch(&PatchEdit {
            synth: "edited-bass".into(),
            operations: vec![
                PatchOperation::Remove {
                    id: "osc".into(),
                    cables: DependentCables::Disconnect,
                },
                PatchOperation::Instantiate {
                    id: "osc".into(),
                    kind: ModuleKind::Oscillator,
                },
                connect("input.pitch", "osc.pitch"),
                connect("osc.audio", "amp.audio"),
            ],
        })
        .unwrap();
    graph = graph.canonicalize_instrument_defaults(&base).unwrap();
    assert_eq!(
        graph.synths()["edited-bass"]
            .modules()
            .last()
            .unwrap()
            .frequency_hz(),
        None
    );
    // Reordering back to original modules/cables still requires v2 for frequency_hz.
    let mut raw = serde_json::to_value(&graph.synths()["edited-bass"]).unwrap();
    raw["modules"].as_array_mut().unwrap().sort_by_key(|m| {
        base.synths()["edited-bass"]
            .modules()
            .iter()
            .position(|b| Some(b.id()) == m["id"].as_str())
            .unwrap()
    });
    raw["cables"].as_array_mut().unwrap().sort_by_key(|c| {
        base.synths()["edited-bass"]
            .cables()
            .iter()
            .position(|b| Some(b.from()) == c["from"].as_str() && Some(b.to()) == c["to"].as_str())
            .unwrap()
    });
    graph = base
        .replace_patch(serde_json::from_value(raw).unwrap())
        .unwrap()
        .canonicalize_instrument_defaults(&base)
        .unwrap();
    h.app
        .enable_instrument_editor_from(base.clone(), graph.clone())
        .unwrap();
    let view = h.app.view_model().instrument_editor.unwrap();
    let encoded = encode_instrument(&view).unwrap();
    let reload = load_instrument(&h.root, &encoded).unwrap();
    assert_eq!(reload.synths(), graph.synths());
    assert!(encoded.contains("version = 2"));
}
