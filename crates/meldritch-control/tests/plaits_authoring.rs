#![allow(dead_code)]
#[path = "../examples/support/piano_roll.rs"]
mod support;
use meldritch_app::{
    AppCommand, InstrumentEdit, InstrumentParameter, InstrumentParameterId, InstrumentValue,
};
use meldritch_control::instrument::{encode_instrument, load_instrument, save_instrument};
use meldritch_core::FrameRange;
use meldritch_dsl::{PlaitsModel, load_song_directory};
use meldritch_render::song_host::CompiledSongPatch;
use std::path::PathBuf;

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../songs/examples/32-plaits-module")
}
fn edit(parameter: InstrumentParameter, value: InstrumentValue) -> InstrumentEdit {
    InstrumentEdit {
        target: InstrumentParameterId {
            synth: "edited-bass".into(),
            module: "osc".into(),
            parameter,
        },
        value,
    }
}
#[test]
fn every_plaits_parameter_changes_published_audio_and_roundtrips() {
    for edit in [
        edit(
            InstrumentParameter::Model,
            InstrumentValue::Model(PlaitsModel::Harmonic),
        ),
        edit(InstrumentParameter::Harmonics, InstrumentValue::Number(0.8)),
        edit(InstrumentParameter::Timbre, InstrumentValue::Number(0.8)),
        edit(InstrumentParameter::Morph, InstrumentValue::Number(0.8)),
    ] {
        let mut h = support::Harness::from_fixture(&fixture());
        let original = h.reader.snapshot();
        let result = h.app.dispatch(AppCommand::EditInstrument(edit)).unwrap();
        assert!(h.host.accepted(&h.app.view_model(), &result).unwrap());
        h.wait();
        let view = h.app.view_model().instrument_editor.unwrap();
        let text = encode_instrument(&view).unwrap();
        let saved = save_instrument(&view).unwrap();
        assert_eq!(std::fs::read_to_string(saved).unwrap(), text);
        let reload = load_instrument(&h.root, &text).unwrap();
        assert_eq!(reload.synths(), view.source.synths());
        assert_eq!(reload.fingerprint(), view.source.fingerprint());
        let range = FrameRange::new(0, 192000).unwrap();
        let render = |song| {
            CompiledSongPatch::compile_initial(song)
                .unwrap()
                .render_default(range)
                .unwrap()
        };
        let audio = render(&reload);
        assert_eq!(audio, render(&view.source));
        assert_ne!(audio.samples(), render(&h.base).samples());
        assert!(audio.samples().iter().all(|s| s.is_finite()));
        assert!(!std::sync::Arc::ptr_eq(&original, &h.reader.snapshot()));
        let mut callback = [0.0; 128];
        h.app.dispatch(AppCommand::Play).unwrap();
        h.renderer.render_f64(&mut callback);
        assert!(callback.iter().any(|x| *x != 0.0));
    }
}
#[test]
fn lua_edit_uses_the_shared_source() {
    let mut h = support::Harness::from_fixture(&fixture());
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
            .timbre,
        0.25
    );
}

#[test]
fn native_keyboard_inspects_and_changes_model_choice() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use meldritch_core::Step;
    let mut h = support::Harness::from_fixture(&fixture());
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
    let action = meldritch_tui::map_key_for_view(
        KeyEvent::new(KeyCode::Right, KeyModifiers::NONE),
        &Step::new(36),
        &h.app.view_model(),
    )
    .unwrap();
    let meldritch_tui::TuiAction::Input(input) = action else {
        panic!("typed keyboard input required")
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
        PlaitsModel::Harmonic
    );
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(80, 30)).unwrap();
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
    assert!(text.contains("24 models") && text.contains("harmonic") && text.contains("plaits"));
}

#[test]
fn invalid_authored_models_ports_and_fields_reject_at_existing_loader() {
    let h = support::Harness::from_fixture(&fixture());
    let path = h.root.join("synths/edited-bass.mlsynth");
    let source = std::fs::read_to_string(&path).unwrap();
    for invalid in [
        source.replace("model = \"fm\"", "model = \"unknown\""),
        source.replace("timbre = 0.35", "timbre = 1.1"),
        source.replace("timbre = 0.35", "timbre = \"bright\""),
        source.replace("model = \"fm\"", "model = \"fm\"\nwaveform = \"saw\""),
        source.replace("osc.pitch", "osc.missing"),
    ] {
        std::fs::write(&path, invalid).unwrap();
        assert!(load_song_directory(&h.root).is_err());
    }
}
#[test]
fn invalid_values_and_ids_retain_source_and_publication() {
    let mut h = support::Harness::from_fixture(&fixture());
    let before = h.reader.snapshot();
    for value in [
        InstrumentValue::Number(-0.1),
        InstrumentValue::Number(1.1),
        InstrumentValue::Number(f64::NAN),
        InstrumentValue::Model(PlaitsModel::Fm),
    ] {
        assert!(
            h.app
                .dispatch(AppCommand::EditInstrument(edit(
                    InstrumentParameter::Timbre,
                    value
                )))
                .is_err()
        );
    }
    assert!(
        h.app
            .dispatch(AppCommand::EditInstrument(edit(
                InstrumentParameter::Model,
                InstrumentValue::Number(0.0)
            )))
            .is_err()
    );
    let mut bad = edit(InstrumentParameter::Timbre, InstrumentValue::Number(0.2));
    bad.target.module = "missing".into();
    assert!(h.app.dispatch(AppCommand::EditInstrument(bad)).is_err());
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
#[test]
fn split_ranges_models_and_extreme_parameters_are_finite_and_repeatable() {
    let base = load_song_directory(fixture()).unwrap();
    for model in [PlaitsModel::Fm, PlaitsModel::Harmonic] {
        for value in [0.0, 0.5, 1.0] {
            let mut song = base
                .edit_instrument(&edit(
                    InstrumentParameter::Model,
                    InstrumentValue::Model(model),
                ))
                .unwrap();
            for p in [
                InstrumentParameter::Harmonics,
                InstrumentParameter::Timbre,
                InstrumentParameter::Morph,
            ] {
                song = song
                    .edit_instrument(&edit(p, InstrumentValue::Number(value)))
                    .unwrap();
            }
            let patch = CompiledSongPatch::compile_initial(&song).unwrap();
            let full = patch
                .render_default(FrameRange::new(0, 96000).unwrap())
                .unwrap();
            let a = patch
                .render_default(FrameRange::new(0, 137).unwrap())
                .unwrap();
            let b = patch
                .render_default(FrameRange::new(137, 96000).unwrap())
                .unwrap();
            assert!(full.samples().iter().all(|x| x.is_finite()));
            assert_eq!(full.samples(), [a.samples(), b.samples()].concat());
        }
    }
}
#[test]
fn omitted_sample_rate_defaults_to_48k_and_explicit_rates_remain_authored() {
    let h = support::Harness::from_fixture(&fixture());
    let path = h.root.join("main.mlperformance");
    let source = std::fs::read_to_string(&path).unwrap();
    std::fs::write(&path, source.replace("sample_rate = 48000", "")).unwrap();
    let defaulted = load_song_directory(&h.root).unwrap();
    assert_eq!(
        defaulted.performance().sample_rate(),
        meldritch_core::DEFAULT_SAMPLE_RATE
    );
    assert_eq!(defaulted.fingerprint(), h.base.fingerprint());
    std::fs::write(path, source.replace("48000", "44100")).unwrap();
    let other = load_song_directory(&h.root).unwrap();
    assert_eq!(other.performance().sample_rate(), 44100);
    let error = CompiledSongPatch::compile_initial(&other)
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("48000") && error.contains("44100"),
        "{error}"
    );
}
