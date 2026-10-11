#![allow(dead_code)]
#[path = "../examples/support/piano_roll.rs"]
mod support;
use meldritch_app::{
    AppCommand, InstrumentEdit, InstrumentParameter, InstrumentParameterId, InstrumentValue,
};
use meldritch_control::instrument::{encode_instrument, load_instrument, save_instrument};
use meldritch_core::FrameRange;
use meldritch_dsl::{PatchEdit, PatchOperation, PlaitsModel};
use meldritch_render::song_host::CompiledSongPatch;
use std::path::PathBuf;
fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../songs/examples/33-plaits-models")
}
#[test]
fn invalid_catalog_examples_diagnose_unknown_models_and_gate_types() {
    for (file, message) in [
        ("unknown-model", "unknown variant"),
        ("wrong-gate", "connects Audio to Gate"),
    ] {
        let h = support::Harness::from_fixture(&fixture());
        let before = h.reader.snapshot();
        let source =
            std::fs::read_to_string(fixture().join(format!("invalid/{file}.mlsynth"))).unwrap();
        std::fs::write(h.root.join("synths/edited-bass.mlsynth"), source).unwrap();
        let error = meldritch_dsl::load_song_directory(&h.root)
            .unwrap_err()
            .to_string();
        assert!(error.contains(message), "{error}");
        assert_eq!(
            h.app
                .view_model()
                .instrument_editor
                .unwrap()
                .source
                .fingerprint(),
            h.base.fingerprint()
        );
        assert!(std::sync::Arc::ptr_eq(&before, &h.reader.snapshot()));
    }
}
fn select(model: PlaitsModel) -> AppCommand {
    AppCommand::EditInstrument(InstrumentEdit {
        target: InstrumentParameterId {
            synth: "edited-bass".into(),
            module: "osc".into(),
            parameter: InstrumentParameter::Model,
        },
        value: InstrumentValue::Model(model),
    })
}
#[test]
fn every_models_parameter_endpoints_roundtrip_without_loss() {
    for model in PlaitsModel::ALL {
        let mut h = support::Harness::from_fixture(&fixture());
        h.app.dispatch(select(model)).unwrap();
        for value in [0.0, 0.5, 1.0] {
            for parameter in [
                InstrumentParameter::Harmonics,
                InstrumentParameter::Timbre,
                InstrumentParameter::Morph,
            ] {
                h.app
                    .dispatch(AppCommand::EditInstrument(InstrumentEdit {
                        target: InstrumentParameterId {
                            synth: "edited-bass".into(),
                            module: "osc".into(),
                            parameter,
                        },
                        value: InstrumentValue::Number(value),
                    }))
                    .unwrap();
            }
            let view = h.app.view_model().instrument_editor.unwrap();
            let restored = load_instrument(&h.root, &encode_instrument(&view).unwrap()).unwrap();
            assert_eq!(restored.synths(), view.source.synths());
            assert_eq!(restored.fingerprint(), view.source.fingerprint());
            let settings = restored.synths()["edited-bass"].modules()[0]
                .plaits()
                .unwrap();
            assert_eq!(
                (
                    settings.model,
                    settings.harmonics,
                    settings.timbre,
                    settings.morph
                ),
                (model, value, value, value)
            );
        }
    }
}
#[test]
fn lua_controller_cycles_the_same_typed_catalog_and_publishes() {
    let mut h = support::Harness::from_fixture(&fixture());
    h.app.dispatch(select(PlaitsModel::Fm)).unwrap();
    // Refresh the read-only projection before the first Lua gesture.
    h.performance.plane.refresh(&h.app).unwrap();
    for expected in PlaitsModel::ALL
        .into_iter()
        .skip(1)
        .chain([PlaitsModel::Fm])
    {
        h.pad("terminal", 0, true);
        h.pad("terminal", 0, false);
        h.wait();
        assert_eq!(
            h.app
                .view_model()
                .instrument_editor
                .unwrap()
                .source
                .synths()["edited-bass"]
                .modules()[0]
                .plaits()
                .unwrap()
                .model,
            expected
        );
    }
}
#[test]
fn every_model_publishes_and_independently_reloads_main_and_aux() {
    for model in PlaitsModel::ALL {
        let mut h = support::Harness::from_fixture(&fixture());
        let original = h.reader.snapshot();
        let result = h.app.dispatch(select(model)).unwrap();
        if h.host.accepted(&h.app.view_model(), &result).unwrap() {
            h.wait();
        }
        let view = h.app.view_model().instrument_editor.unwrap();
        assert_eq!(
            view.source.synths()["edited-bass"].modules()[0]
                .plaits()
                .unwrap()
                .model,
            model
        );
        let encoded = encode_instrument(&view).unwrap();
        let saved = save_instrument(&view).unwrap();
        assert_eq!(std::fs::read_to_string(saved).unwrap(), encoded);
        let restored = load_instrument(&h.root, &encoded).unwrap();
        assert_eq!(
            restored.fingerprint(),
            view.source.fingerprint(),
            "{model:?}"
        );
        assert_eq!(restored.synths(), view.source.synths());
        let render = |s| {
            CompiledSongPatch::compile_initial(s)
                .unwrap()
                .render_default(FrameRange::new(0, 48000).unwrap())
                .unwrap()
        };
        let audio = render(&restored);
        assert!(audio.samples().iter().all(|s| s.is_finite()), "{model:?}");
        if [
            PlaitsModel::Fm,
            PlaitsModel::Harmonic,
            PlaitsModel::VirtualAnalog,
            PlaitsModel::Waveshaping,
            PlaitsModel::Grain,
            PlaitsModel::Wavetable,
            PlaitsModel::Chord,
            PlaitsModel::VirtualAnalogVcf,
            PlaitsModel::PhaseDistortion,
            PlaitsModel::WaveTerrain,
        ]
        .contains(&model)
        {
            assert_eq!(audio, render(&view.source), "{model:?} reload");
            let compiled = CompiledSongPatch::compile_initial(&restored).unwrap();
            let first = compiled
                .render_default(FrameRange::new(0, 24013).unwrap())
                .unwrap();
            let second = compiled
                .render_default(FrameRange::new(24013, 48000).unwrap())
                .unwrap();
            let split: Vec<f64> = first
                .samples()
                .iter()
                .chain(second.samples())
                .copied()
                .collect();
            assert_eq!(audio.samples(), split, "{model:?} split");
        }
        assert!(
            audio.samples().iter().any(|s| s.abs() > 1e-6),
            "{model:?} silent main"
        );
        if model != PlaitsModel::VirtualAnalog {
            assert!(
                !std::sync::Arc::ptr_eq(&original, &h.reader.snapshot()),
                "{model:?}"
            );
        }
        h.app.dispatch(AppCommand::Play).unwrap();
        let mut played = [0.0; 2048];
        h.renderer.render_f64(&mut played);
        assert!(
            played.iter().any(|s| s.abs() > 1e-6),
            "{model:?} unpublished audio"
        );
        let result = h
            .app
            .dispatch(AppCommand::EditPatch(PatchEdit {
                synth: "edited-bass".into(),
                operations: vec![
                    PatchOperation::Disconnect {
                        from: "osc.audio".into(),
                        to: "amp.audio".into(),
                    },
                    PatchOperation::Connect {
                        from: "osc.aux".into(),
                        to: "amp.audio".into(),
                    },
                ],
            }))
            .unwrap();
        if h.host.accepted(&h.app.view_model(), &result).unwrap() {
            h.wait();
        }
        let aux_view = h.app.view_model().instrument_editor.unwrap();
        let aux_encoded = encode_instrument(&aux_view).unwrap();
        let aux_restored = load_instrument(&h.root, &aux_encoded).unwrap();
        assert_eq!(aux_restored.synths(), aux_view.source.synths());
        let aux = render(&aux_restored);
        assert!(aux.samples().iter().all(|s| s.is_finite()), "{model:?}");
        assert!(
            aux.samples().iter().any(|s| s.abs() > 1e-6),
            "{model:?} silent aux"
        );
    }
}
#[test]
fn keyboard_cycles_every_model_and_wraps() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    let mut h = support::Harness::from_fixture(&fixture());
    h.app.dispatch(select(PlaitsModel::Fm)).unwrap();
    h.app.dispatch(AppCommand::ToggleInstrumentEditor).unwrap();
    let view = h.app.view_model().instrument_editor.unwrap();
    let index = view
        .parameters
        .iter()
        .position(|p| p.id.parameter == InstrumentParameter::Model)
        .unwrap();
    h.app
        .dispatch(AppCommand::NavigateInstrument {
            synth_delta: 0,
            parameter_delta: index as i32,
        })
        .unwrap();
    for expected in PlaitsModel::ALL
        .into_iter()
        .skip(1)
        .chain([PlaitsModel::Fm])
    {
        let action = meldritch_tui::map_key_for_view(
            KeyEvent::new(KeyCode::Right, KeyModifiers::NONE),
            &meldritch_core::Step::new(36),
            &h.app.view_model(),
        )
        .unwrap();
        let meldritch_tui::TuiAction::Input(input) = action else {
            panic!("input")
        };
        h.app.handle_input(input).unwrap();
        assert_eq!(
            h.app
                .view_model()
                .instrument_editor
                .unwrap()
                .source
                .synths()["edited-bass"]
                .modules()[0]
                .plaits()
                .unwrap()
                .model,
            expected
        );
    }
    assert!(
        PlaitsModel::parse("909")
            .unwrap_err()
            .contains("supported:")
    );
}
