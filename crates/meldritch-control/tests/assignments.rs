#[path = "../examples/support/assignments.rs"]
mod support;
use meldritch_app::*;
use meldritch_control::{LuaProject, Phase, assignments::*};
use meldritch_core::FrameRange;
use meldritch_dsl::{DelayParameter, InstrumentParameter, MixerParameter};
use meldritch_render::song_host::CompiledSongPatch;
fn render(song: &ValidatedSong) -> meldritch_audio::AudioBlock {
    CompiledSongPatch::compile_initial(song)
        .unwrap()
        .render_default(FrameRange::new(0, 192000).unwrap())
        .unwrap()
}
fn parameter(h: &support::Harness, id: &str) -> f64 {
    let a = h.host.app.assignment_view().unwrap();
    let t = a.definition.targets.iter().find(|t| t.id == id).unwrap();
    a.source.editable_parameter(&t.target).unwrap().value
}
#[test]
fn physical_pickup_typed_source_render_publication_projections_and_saved_project_round_trip() {
    let mut h = support::Harness::new();
    let root = h.host.root.clone();
    let originals = [
        "main.lua",
        "main.mlperformance",
        "synths/bass.mlsynth",
        "dsp/echo.mldsp",
    ]
    .map(|p| (p, std::fs::read(root.join(p)).unwrap()));
    std::fs::write(
        root.join("performances/existing.mlperformance"),
        "preserve this recording",
    )
    .unwrap();
    h.host.app.dispatch(AppCommand::Play).unwrap();
    h.host.renderer.render_f64(&mut [0.0; 32]);
    let fingerprint = h.host.base.fingerprint();
    let history = h.host.app.history().len();
    assert!(!h.cc(48, 0));
    assert_eq!(h.host.app.history().len(), history);
    assert_eq!(h.host.host.generation(), 0);
    assert_eq!(
        h.host.app.assignment_view().unwrap().source.fingerprint(),
        fingerprint
    );
    h.script(include_str!(
        "../../../songs/examples/28-author-fader-mappings/scripts/compose.faders"
    ));
    assert_eq!(parameter(&h, "cutoff"), 6000.0);
    assert_eq!(parameter(&h, "resonance"), 0.5);
    assert_eq!(parameter(&h, "volume"), 0.5);
    assert_eq!(parameter(&h, "pan"), 1.0);
    assert_eq!(parameter(&h, "delay-mix"), 1.0);
    assert_eq!(h.host.host.generation(), 7);
    assert!(
        h.host
            .app
            .history()
            .iter()
            .filter(|r| matches!(r.command, AppCommand::EditParameter(_)) && r.changed)
            .all(|r| r.dirty_ranges.len() == 1)
    );
    let view = h.host.app.assignment_view().unwrap();
    let audio = render(&view.source);
    assert_eq!(audio.channels(), 2);
    assert!(audio.peak_abs() > 0.01);
    assert!(
        audio
            .samples()
            .chunks_exact(2)
            .all(|p| p[0] == 0.0 && p[1].is_finite())
    );
    assert_eq!(
        h.host.reader.snapshot().generation(),
        h.host.host.generation()
    );
    for (frame, pair) in audio.samples().chunks_exact(2).enumerate() {
        assert_eq!(h.host.reader.snapshot().frame(frame as u32).unwrap(), pair);
    }
    h.host.renderer.render_f64(&mut [0.0; 256]);
    assert_eq!(
        h.host.reader.playback_monitor().generation(),
        Some(h.host.host.generation())
    );
    assert_eq!(
        h.host.app.view_model().transport.state,
        meldritch_audio::transport::TransportState::Playing
    );
    let a = h.host.performance.plane.output("apc-mini").unwrap();
    let t = h.host.performance.plane.output("terminal").unwrap();
    assert!(a.cells[&3].label.contains("0.5"));
    assert!(a.cells[&3].label.contains("acquired"));
    assert_eq!(a.cells[&3].label, t.cells[&3].label);
    assert!(
        !h.host
            .performance
            .plane
            .trace()
            .iter()
            .any(|t| t.phase == Phase::Rejected)
    );
    let encoded = encode_controls(&view).unwrap();
    assert_eq!(
        encoded,
        std::fs::read_to_string(support::fixture().join("edited-controls.lua")).unwrap()
    );
    let (loaded, definition) = load_controls(&root, &encoded).unwrap();
    assert_eq!(loaded.fingerprint(), view.source.fingerprint());
    assert_eq!(render(&loaded), audio);
    assert_eq!(definition, view.definition);
    let path = save_controls(&view).unwrap();
    let other = save_controls(&view).unwrap();
    assert_ne!(path, other);
    assert_eq!(
        std::fs::read_to_string(path).unwrap(),
        std::fs::read_to_string(other).unwrap()
    );
    for (p, bytes) in originals {
        assert_eq!(std::fs::read(root.join(p)).unwrap(), bytes);
    }
    assert_eq!(
        std::fs::read_to_string(root.join("performances/existing.mlperformance")).unwrap(),
        "preserve this recording"
    );
    h.host
        .app
        .enable_assignment_authoring_from(h.host.base.clone(), loaded, definition)
        .unwrap();
    assert_eq!(
        encode_controls(&h.host.app.assignment_view().unwrap()).unwrap(),
        encoded
    );
    assert_eq!(
        h.host
            .capture
            .events
            .iter()
            .filter(|e| e.kind == "authored_parameter")
            .count(),
        11
    );
}
#[test]
fn remapping_page_navigation_other_surface_edits_and_unassignment_rearm_pickup() {
    let mut h = support::Harness::new();
    assert!(!h.cc(48, 0));
    assert!(h.cc(48, 127));
    let command = |h: &mut support::Harness, c: AppCommand| {
        let result = h.host.app.dispatch(c).unwrap();
        h.host
            .host
            .accepted(&h.host.app.view_model(), &result)
            .unwrap();
    };
    command(
        &mut h,
        AppCommand::AssignFader {
            id: "main-1".into(),
            target: Some("volume".into()),
        },
    );
    let generation = h.host.host.generation();
    assert!(!h.cc(48, 0));
    assert_eq!(parameter(&h, "volume"), 1.0);
    assert_eq!(h.host.host.generation(), generation);
    assert!(h.cc(48, 127));
    assert!(h.cc(48, 64));
    assert_eq!(parameter(&h, "volume"), 0.5);
    command(&mut h, AppCommand::AssignmentPage("tone".into()));
    command(&mut h, AppCommand::AssignmentPage("main".into()));
    assert!(!h.cc(48, 127));
    assert!(h.cc(48, 64));
    command(
        &mut h,
        AppCommand::EditParameter(ParameterEdit {
            target: EditableTarget::Track {
                id: "bass".into(),
                parameter: MixerParameter::Volume,
            },
            value: 0.2,
        }),
    );
    assert!(!h.cc(48, 127));
    assert!(h.cc(48, 0));
    command(
        &mut h,
        AppCommand::AssignFader {
            id: "main-1".into(),
            target: None,
        },
    );
    let fingerprint = h.host.app.assignment_view().unwrap().source.fingerprint();
    assert!(!h.cc(48, 127));
    assert_eq!(
        h.host.app.assignment_view().unwrap().source.fingerprint(),
        fingerprint
    );
    command(
        &mut h,
        AppCommand::AssignFader {
            id: "main-1".into(),
            target: Some("cutoff".into()),
        },
    );
    assert!(!h.cc(48, 0));
    assert!(h.cc(48, 127));
    command(
        &mut h,
        AppCommand::EditInstrument(InstrumentEdit {
            target: InstrumentParameterId {
                synth: "bass".into(),
                module: "filter".into(),
                parameter: InstrumentParameter::CutoffHz,
            },
            value: InstrumentValue::Number(2000.0),
        }),
    );
    assert!(!h.cc(48, 127));
    assert!(h.cc(48, 0));
    assert_eq!(parameter(&h, "cutoff"), 80.0);
    command(
        &mut h,
        AppCommand::AssignFader {
            id: "main-6".into(),
            target: Some("cutoff".into()),
        },
    );
    assert!(h.cc(53, 0));
    assert!(h.cc(53, 64));
    assert!(!h.cc(48, 0));
    assert!(h.cc(48, 64));
    h.pickup.disconnect("apc-mini");
    h.host.app.disconnect_assignments("apc-mini");
    assert!(
        h.host
            .app
            .assignment_view()
            .unwrap()
            .observations
            .values()
            .all(|o| !o.1)
    );
    assert!(!h.cc(48, 127));
    assert!(h.cc(48, 64));
}

#[test]
fn keyboard_assignment_workflow_and_native_tui_observe_authoritative_state() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use meldritch_tui::{TuiAction, map_key_for_view};
    let mut h = support::Harness::new();
    let key = |h: &mut support::Harness, code, modifiers| {
        let action = map_key_for_view(
            KeyEvent::new(code, modifiers),
            &meldritch_core::Step::new(36),
            &h.host.app.view_model(),
        )
        .unwrap();
        let TuiAction::Input(input) = action else {
            panic!("expected typed input")
        };
        h.host.app.handle_input(input).unwrap()
    };
    key(&mut h, KeyCode::Char('m'), KeyModifiers::CONTROL);
    assert!(h.host.app.assignment_view().unwrap().visible);
    key(&mut h, KeyCode::Right, KeyModifiers::NONE);
    key(&mut h, KeyCode::Enter, KeyModifiers::NONE);
    assert_eq!(
        h.host.app.assignment_view().unwrap().definition.faders[0]
            .target
            .as_deref(),
        Some("resonance")
    );
    assert!(!h.cc(48, 0));
    assert!(h.cc(48, 127));
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(150, 30)).unwrap();
    h.host.run_script("wait 100");
    terminal
        .draw(|frame| meldritch_tui::draw(frame, &h.host.app.view_model()))
        .unwrap();
    let screen: String = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|c| c.symbol())
        .collect();
    for word in [
        "synth:bass/filter.resonance",
        "raw Some(127)",
        "acquired",
        "applied to published audio",
    ] {
        assert!(screen.contains(word), "missing {word}: {screen}");
    }
    key(&mut h, KeyCode::Delete, KeyModifiers::NONE);
    assert!(
        h.host.app.assignment_view().unwrap().definition.faders[0]
            .target
            .is_none()
    );
    key(&mut h, KeyCode::PageDown, KeyModifiers::NONE);
    assert_eq!(h.host.app.assignment_view().unwrap().page, "tone");
    key(&mut h, KeyCode::PageUp, KeyModifiers::NONE);
    assert_eq!(h.host.app.assignment_view().unwrap().page, "main");
    assert!(matches!(
        key(&mut h, KeyCode::Char('s'), KeyModifiers::CONTROL),
        AppCommandResult::AssignmentSaveRequested
    ));
    key(&mut h, KeyCode::Esc, KeyModifiers::NONE);
    assert!(!h.host.app.assignment_view().unwrap().visible);
}

#[test]
fn every_catalog_parameter_changes_audio_and_round_trips_bounds_and_midpoints() {
    let h = support::Harness::new();
    let original = h.host.app.assignment_view().unwrap();
    let before = render(&original.source);
    for t in &original.definition.targets {
        for value in [t.minimum, (t.minimum + t.maximum) / 2.0, t.maximum] {
            let mut view = original.clone();
            view.source = std::sync::Arc::new(
                view.source
                    .edit_parameter(&ParameterEdit {
                        target: t.target.clone(),
                        value,
                    })
                    .unwrap()
                    .canonicalize_instrument_defaults(&view.base)
                    .unwrap(),
            );
            let audio = render(&view.source);
            if value != original.source.editable_parameter(&t.target).unwrap().value {
                assert_ne!(
                    audio, before,
                    "{}={value} must affect rendered samples",
                    t.id
                );
            }
            let text = encode_controls(&view).unwrap();
            let (song, definition) = load_controls(&h.host.root, &text).unwrap();
            assert_eq!(song.fingerprint(), view.source.fingerprint());
            assert_eq!(song.editable_parameter(&t.target).unwrap().value, value);
            assert_eq!(render(&song), audio);
            assert_eq!(definition, view.definition);
        }
    }
}

#[test]
fn failed_save_and_readonly_lua_leave_authored_edits_intact() {
    let mut h = support::Harness::new();
    assert!(!h.cc(48, 0));
    assert!(h.cc(48, 127));
    h.host.run_script("wait 100");
    let view = h.host.app.assignment_view().unwrap();
    std::fs::write(h.host.root.join("authored"), "occupied path").unwrap();
    let result = h.host.app.dispatch(AppCommand::SaveAssignments).unwrap();
    assert!(
        h.host
            .host
            .accepted(&h.host.app.view_model(), &result)
            .is_err()
    );
    h.host.host.refresh_view(&mut h.host.app);
    assert!(
        h.host
            .app
            .assignment_view()
            .unwrap()
            .save_status
            .unwrap()
            .contains("edits remain unsaved")
    );
    assert_eq!(
        h.host.app.assignment_view().unwrap().source.fingerprint(),
        view.source.fingerprint()
    );
    assert_eq!(
        std::fs::read_to_string(h.host.root.join("authored")).unwrap(),
        "occupied path"
    );
    let text=std::fs::read_to_string(h.host.root.join("main.lua")).unwrap().replace("local f=current(view,index);local targets=", "view.assignments.definition.faders[1].target='volume';local f=current(view,index);local targets=");
    let project = LuaProject::load(&h.host.root, &text).unwrap();
    let mut plane = meldritch_control::ControlPlane::new(project, &h.host.app).unwrap();
    plane
        .enqueue("apc-mini", 0, meldritch_control::Input::Press(0))
        .unwrap();
    plane.pump(&mut h.host.app);
    assert!(
        plane
            .trace()
            .iter()
            .any(|t| t.phase == Phase::Rejected && t.detail.contains("read-only"))
    );
    assert_eq!(
        h.host.app.assignment_view().unwrap().definition,
        view.definition
    );
    assert_eq!(
        h.host.app.assignment_view().unwrap().source.fingerprint(),
        view.source.fingerprint()
    );
}

#[test]
fn mixer_source_edits_preserve_mono_and_render_deterministic_stereo_ranges() {
    use meldritch_dsl::load_song_directory;
    let mono = load_song_directory(
        support::fixture()
            .parent()
            .unwrap()
            .join("09-curated-performance-controls"),
    )
    .unwrap();
    let baseline = render(&mono);
    assert_eq!(baseline.channels(), 1);
    let edit = |song: &ValidatedSong, parameter, value| {
        song.edit_parameter(&ParameterEdit {
            target: EditableTarget::Track {
                id: "bass".into(),
                parameter,
            },
            value,
        })
        .unwrap()
    };
    assert_eq!(render(&edit(&mono, MixerParameter::Volume, 1.0)), baseline);
    let half = render(&edit(&mono, MixerParameter::Volume, 0.5));
    assert_eq!(half.channels(), 1);
    assert!(
        half.samples()
            .iter()
            .zip(baseline.samples())
            .all(|(a, b)| *a == b * 0.5)
    );
    assert_eq!(
        render(&edit(&mono, MixerParameter::Volume, 0.0)).peak_abs(),
        0.0
    );
    for pan in [-1.0, -0.5, 0.0, 0.5, 1.0] {
        let song = edit(&mono, MixerParameter::Pan, pan);
        let audio = render(&song);
        assert_eq!(audio.channels(), 2);
        assert_ne!(song.fingerprint(), mono.fingerprint());
        for (p, v) in audio.samples().chunks_exact(2).zip(baseline.samples()) {
            assert_eq!(p[0], v * (1.0 - pan.max(0.0)));
            assert_eq!(p[1], v * (1.0 + pan.min(0.0)));
        }
        let patch = CompiledSongPatch::compile_initial(&song).unwrap();
        let a = patch
            .render_default(FrameRange::new(0, 7331).unwrap())
            .unwrap();
        let b = patch
            .render_default(FrameRange::new(7331, 192000).unwrap())
            .unwrap();
        assert_eq!(
            a.samples()
                .iter()
                .chain(b.samples())
                .copied()
                .collect::<Vec<_>>(),
            audio.samples()
        );
    }
    let ensemble = load_song_directory(
        support::fixture()
            .parent()
            .unwrap()
            .join("19-apc-mini-mk2-playground"),
    )
    .unwrap();
    let before = render(&ensemble);
    let muted = ensemble
        .edit_parameter(&ParameterEdit {
            target: EditableTarget::Track {
                id: "bass-a".into(),
                parameter: MixerParameter::Volume,
            },
            value: 0.0,
        })
        .unwrap();
    let after = render(&muted);
    assert!(after.peak_abs() > 0.0);
    assert_ne!(after, before);
    let stereo = muted
        .edit_parameter(&ParameterEdit {
            target: EditableTarget::Track {
                id: "pad".into(),
                parameter: MixerParameter::Pan,
            },
            value: 1.0,
        })
        .unwrap();
    let audio = render(&stereo);
    assert_eq!(audio.channels(), 2);
    assert!(audio.samples().chunks_exact(2).any(|p| p[0] != p[1]));
}

#[test]
fn mono_import_with_pan_catalog_publishes_before_and_after_first_pan_edit() {
    let seed = support::Harness::new();
    let performance = seed.host.root.join("main.mlperformance");
    let text = std::fs::read_to_string(&performance)
        .unwrap()
        .replace("pan = 0.0\n", "")
        .replace("pan = 0\n", "");
    std::fs::write(performance, text).unwrap();
    let mut h = support::Harness::from_fixture(&seed.host.root);
    assert_eq!(render(&h.host.base).channels(), 1);
    assert_eq!(h.host.reader.snapshot().channels(), 2);
    assert!(h.cc(49, 127));
    assert!(h.cc(49, 64));
    h.host.run_script("wait 100");
    assert_eq!(
        h.host
            .app
            .assignment_view()
            .unwrap()
            .source
            .performance()
            .tracks()[0]
            .pan(),
        None
    );
    assert_eq!(h.host.reader.snapshot().channels(), 2);
    assert!(!h.cc(50, 0));
    assert!(h.cc(50, 127));
    h.host.run_script("wait 100");
    let view = h.host.app.assignment_view().unwrap();
    let block = render(&view.source);
    assert_eq!(
        h.host.reader.snapshot().generation(),
        h.host.host.generation()
    );
    assert_eq!(h.host.reader.snapshot().channels(), 2);
    for (i, pair) in block.samples().chunks_exact(2).enumerate() {
        assert_eq!(h.host.reader.snapshot().frame(i as u32).unwrap(), pair);
    }
    let (source, _) = load_controls(&h.host.root, &encode_controls(&view).unwrap()).unwrap();
    assert_eq!(render(&source), block);
}

#[test]
fn existing_curated_widget_and_fader_edit_one_authoritative_parameter() {
    let mut h = support::Harness::new();
    h.host.app.set_curated_controls(vec![CuratedControlView {
        id: "keyboard-tone".into(),
        label: "Tone".into(),
        target: "synth:bass/filter.cutoff_hz".into(),
        minimum: 80.0,
        maximum: 6000.0,
        step: 10.0,
        binding: "c".into(),
        value: Some(parameter(&h, "cutoff")),
    }]);
    assert!(!h.cc(48, 0));
    assert!(h.cc(48, 127));
    assert_eq!(
        h.host
            .app
            .view_model()
            .curated_controls
            .iter()
            .find(|c| c.id == "keyboard-tone")
            .unwrap()
            .value,
        Some(6000.0)
    );
    let result = h
        .host
        .app
        .dispatch(AppCommand::SetCuratedControlNormalized {
            id: "keyboard-tone".into(),
            value: 0.0,
        })
        .unwrap();
    assert!(matches!(
        result,
        AppCommandResult::ParameterEdited { changed: true, .. }
    ));
    h.host
        .host
        .accepted(&h.host.app.view_model(), &result)
        .unwrap();
    assert_eq!(parameter(&h, "cutoff"), 80.0);
    assert!(!h.cc(48, 127));
    assert!(h.cc(48, 0));
    let result = h
        .host
        .app
        .dispatch(AppCommand::AdjustCuratedControl {
            id: "keyboard-tone".into(),
            steps: 1,
        })
        .unwrap();
    h.host
        .host
        .accepted(&h.host.app.view_model(), &result)
        .unwrap();
    assert_eq!(parameter(&h, "cutoff"), 90.0);
    assert_eq!(
        h.host
            .app
            .view_model()
            .instrument_editor
            .unwrap()
            .parameters
            .iter()
            .find(|p| p.id.parameter == InstrumentParameter::CutoffHz)
            .unwrap()
            .value,
        InstrumentValue::Number(90.0)
    );
    let view = h.host.app.assignment_view().unwrap();
    assert!(view.observations.values().all(|o| !o.1));
    let (song, _) = load_controls(&h.host.root, &encode_controls(&view).unwrap()).unwrap();
    assert_eq!(song.fingerprint(), view.source.fingerprint());
    h.host
        .app
        .dispatch(AppCommand::AssignFader {
            id: "main-6".into(),
            target: Some("cutoff".into()),
        })
        .unwrap();
    assert!(h.cc(53, 0));
    assert!(h.cc(53, 127));
    for value in [0.0, 1.0] {
        let command = meldritch_control::CommandRequest::SetControl {
            id: "assignment:main-1".into(),
            value,
        }
        .command()
        .unwrap();
        let result = h.host.app.dispatch(command).unwrap();
        assert!(matches!(
            h.host.app.history().back().unwrap().command,
            AppCommand::EditParameter(_)
        ));
        h.host
            .host
            .accepted(&h.host.app.view_model(), &result)
            .unwrap();
    }
    // Even away-and-back edits through Lua's normalized facade must invalidate
    // another observing fader's acquired ownership, not only its own identity.
    assert!(!h.cc(53, 64));
}
#[test]
fn invalid_targets_definitions_and_saved_projects_reject_atomically() {
    let mut h = support::Harness::new();
    let view = h.host.app.assignment_view().unwrap();
    use mlua::LuaSerdeExt;
    for file in [
        "missing-fader",
        "missing-target",
        "missing-parameter",
        "incompatible-parameter",
        "pan-range",
        "value-type",
    ] {
        let lua = mlua::Lua::new();
        let text = std::fs::read_to_string(support::fixture().join(format!("invalid/{file}.lua")))
            .unwrap();
        let value = lua.load(&text).eval().unwrap();
        let request: Result<meldritch_control::CommandRequest, _> = lua.from_value(value);
        if file == "value-type" {
            assert!(request.is_err());
        } else {
            let command = request.unwrap().command().unwrap();
            assert!(h.host.app.dispatch(command).is_err(), "{file}");
        }
        assert_eq!(
            h.host.app.assignment_view().unwrap().source.fingerprint(),
            view.source.fingerprint()
        );
        assert!(h.host.app.history().is_empty());
    }
    for c in [
        AppCommand::AssignFader {
            id: "missing".into(),
            target: None,
        },
        AppCommand::AssignFader {
            id: "main-1".into(),
            target: Some("missing".into()),
        },
        AppCommand::AssignmentPage("missing".into()),
        AppCommand::EditParameter(ParameterEdit {
            target: EditableTarget::Track {
                id: "bass".into(),
                parameter: MixerParameter::Pan,
            },
            value: 2.0,
        }),
        AppCommand::EditParameter(ParameterEdit {
            target: EditableTarget::Dsp {
                id: "echo".into(),
                module: "delay".into(),
                parameter: DelayParameter::Mix,
            },
            value: f64::NAN,
        }),
    ] {
        assert!(h.host.app.dispatch(c).is_err());
        assert_eq!(
            h.host.app.assignment_view().unwrap().definition,
            view.definition
        );
        assert_eq!(
            h.host.app.assignment_view().unwrap().source.fingerprint(),
            view.source.fingerprint()
        );
        assert!(h.host.app.history().is_empty());
    }
    let mut invalid = Vec::new();
    let mut d = view.definition.clone();
    d.faders[0].page = "missing".into();
    invalid.push(d);
    let mut d = view.definition.clone();
    d.faders[0].channel = 0;
    invalid.push(d);
    let mut d = view.definition.clone();
    d.targets[1].target = d.targets[0].target.clone();
    invalid.push(d);
    let mut d = view.definition.clone();
    d.faders[1].cc = d.faders[0].cc;
    invalid.push(d);
    let mut d = view.definition.clone();
    d.faders[0].device = "missing".into();
    invalid.push(d);
    let mut d = view.definition.clone();
    d.targets[0].minimum = 7000.0;
    invalid.push(d);
    let mut d = view.definition.clone();
    d.targets[0].step = f64::NAN;
    invalid.push(d);
    let mut d = view.definition.clone();
    d.targets[0].target = EditableTarget::Synth {
        id: "bass".into(),
        module: "osc".into(),
        parameter: InstrumentParameter::Waveform,
    };
    invalid.push(d);
    for d in invalid {
        assert!(h.host.app.enable_assignments(d).is_err());
        assert_eq!(
            h.host.app.assignment_view().unwrap().definition,
            view.definition
        );
    }
    let source = encode_controls(&view).unwrap();
    for bad in [
        source.replace("version = 1", "version = 2"),
        source.replace("cc = 48", "cc = 128"),
        source.replace("id = \"bass\", module", "id = \"missing\", module"),
        source.replace("minimum = 80.0", "minimum = 7000.0"),
    ] {
        assert!(load_controls(&h.host.root, &bad).is_err());
    }
    let outside = std::env::temp_dir().join(format!(
        "meldritch-controls-outside-{}.lua",
        std::process::id()
    ));
    std::fs::write(&outside, &source).unwrap();
    assert!(load_controls_file(&h.host.root, &outside).is_err());
    std::fs::remove_file(outside).unwrap();
    let source_script = std::fs::read_to_string(h.host.root.join("main.lua")).unwrap();
    assert!(LuaProject::load(&h.host.root, &source_script.replace("cc=47+i", "cc=48")).is_err());
    let performance = h.host.root.join("main.mlperformance");
    let original = std::fs::read_to_string(&performance).unwrap();
    for bad in [
        original.replace("volume = 1.0", "volume = -1.0"),
        original.replace("pan = 0.0", "pan = 2.0"),
        original.replace("volume = 1.0", "volume = nan"),
    ] {
        std::fs::write(&performance, bad).unwrap();
        let error = meldritch_dsl::load_song_directory(&h.host.root)
            .unwrap_err()
            .to_string();
        assert!(error.contains("track 'bass'") && error.contains("finite"));
    }
    std::fs::write(&performance, original).unwrap();
}
