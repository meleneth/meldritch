//! A musician workflow through existing Lua/MIDI, key mapping and publication.
#[path = "piano_roll.rs"]
pub mod piano;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use meldritch_app::*;
use meldritch_control::{
    devices::{MidiInputQueue, MidiPickup},
    project::*,
};
use meldritch_core::{FrameRange, Step, Tempo};
use meldritch_render::song_host::CompiledSongPatch;
use std::path::{Path, PathBuf};

pub fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../songs/examples/29-lua-authored-roundtrip")
}
pub fn key(
    h: &mut piano::Harness,
    code: KeyCode,
    modifiers: KeyModifiers,
) -> Result<AppCommandResult, String> {
    let action = meldritch_tui::map_key_for_view(
        KeyEvent::new(code, modifiers),
        &Step::new(36),
        &h.app.view_model(),
    )
    .ok_or("unmapped key")?;
    let meldritch_tui::TuiAction::Input(input) = action else {
        return Err("quit action".into());
    };
    let result = h
        .app
        .handle_input(input.clone())
        .map_err(|e| format!("{e:?}"))?;
    h.capture.record(
        &h.app,
        &input,
        &result,
        192000,
        Tempo::new(120.0, 48000).unwrap(),
    )?;
    let scheduled = h.host.accepted(&h.app.view_model(), &result);
    if matches!(scheduled, Ok(true)) {
        h.wait();
    }
    h.host.refresh_view(&mut h.app);
    h.performance.plane.refresh(&h.app)?;
    scheduled?;
    Ok(result)
}
pub fn line(h: &mut piano::Harness, text: &str) -> Result<AppCommandResult, String> {
    for c in text.chars() {
        key(h, KeyCode::Char(c), KeyModifiers::NONE)?;
    }
    key(h, KeyCode::Enter, KeyModifiers::NONE)
}
fn parameter(h: &mut piano::Harness, wanted: InstrumentParameter) {
    let view = h.app.view_model().instrument_editor.unwrap();
    let index = view
        .parameters
        .iter()
        .position(|p| p.id.parameter == wanted)
        .unwrap();
    for _ in view.selected_parameter..index {
        key(h, KeyCode::Down, KeyModifiers::NONE).unwrap();
    }
    for _ in index..view.selected_parameter {
        key(h, KeyCode::Up, KeyModifiers::NONE).unwrap();
    }
}
pub fn screen(h: &piano::Harness) -> String {
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(80, 30)).unwrap();
    terminal
        .draw(|f| meldritch_tui::draw(f, &h.app.view_model()))
        .unwrap();
    terminal
        .backend()
        .buffer()
        .content
        .chunks(80)
        .map(|row| row.iter().map(|c| c.symbol()).collect::<String>() + "\n")
        .collect()
}
pub fn cc(h: &mut piano::Harness, pickup: &mut MidiPickup, cc: u8, value: u8) -> bool {
    let queue = MidiInputQueue::default();
    assert!(queue.route("apc-mini").observe(&[0xb0, cc, value]));
    let observed = queue.next().unwrap();
    assert!(observed.is_current());
    h.performance
        .fader(
            &observed.device,
            &observed.bytes,
            48000,
            &mut h.app,
            pickup,
            &mut h.host,
            &mut h.capture,
        )
        .unwrap()
        .unwrap()
}
pub fn state(h: &piano::Harness) -> String {
    let view = h.app.view_model();
    let source = &view.instrument_editor.as_ref().unwrap().source;
    serde_json::to_string_pretty(&serde_json::json!({
        "fingerprint": format!("{:016x}", source.fingerprint()),
        "synths": source.synths(),
        "pattern": view.pattern_editor.unwrap().pattern,
        "assignments": view.assignments.unwrap().definition,
        "tracks": source.performance().tracks().iter().map(|t| serde_json::json!({"id":t.id(),"volume":t.volume(),"pan":t.pan()})).collect::<Vec<_>>()
    })).unwrap()
}
pub fn pcm(h: &piano::Harness) -> Vec<u8> {
    let source = h.app.view_model().instrument_editor.unwrap().source;
    let patch = CompiledSongPatch::compile_initial(&source).unwrap();
    let audio = patch
        .render_default(FrameRange::new(0, 192000).unwrap())
        .unwrap();
    let a = patch
        .render_default(FrameRange::new(0, 57123).unwrap())
        .unwrap();
    let b = patch
        .render_default(FrameRange::new(57123, 192000).unwrap())
        .unwrap();
    assert!(audio.samples().iter().all(|s| s.is_finite()));
    assert!(audio.samples().iter().any(|s| s.abs() > 1e-6));
    if source.performance().tracks()[0].pan() == Some(1.0) {
        assert!(audio.samples().chunks_exact(2).all(|pair| pair[0] == 0.0));
        assert!(
            audio
                .samples()
                .chunks_exact(2)
                .any(|pair| pair[1].abs() > 1e-6)
        );
    }
    assert_eq!(
        audio.samples(),
        a.samples()
            .iter()
            .chain(b.samples())
            .copied()
            .collect::<Vec<_>>()
    );
    for (frame, pair) in audio.samples().chunks_exact(2).enumerate() {
        assert_eq!(h.reader.snapshot().frame(frame as u32).unwrap(), pair);
    }
    audio
        .samples()
        .iter()
        .flat_map(|s| s.to_le_bytes())
        .collect()
}
fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &to.join(entry.file_name()));
        } else {
            std::fs::copy(entry.path(), to.join(entry.file_name())).unwrap();
        }
    }
}
pub fn finish(h: &mut piano::Harness) {
    let action = meldritch_tui::map_key_for_view(
        KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE),
        &Step::new(36),
        &h.app.view_model(),
    );
    assert!(matches!(action, Some(meldritch_tui::TuiAction::Quit)));
    h.performance
        .cancel("apc-mini", &mut h.app, &mut h.host, &mut h.capture)
        .unwrap();
    h.capture.finish_clean(&h.app).unwrap();
    assert!(
        std::fs::read_to_string(h.capture.path())
            .unwrap()
            .contains("clean_termination = true")
    );
}
/// Return the persisted song directory, ready to pass to a separate process.
pub fn run(output: &Path) -> PathBuf {
    std::fs::create_dir(output).expect("acceptance output must be a new directory");
    let mut h = piano::Harness::from_fixture(&fixture());
    let base_bytes = std::fs::read(h.root.join("main.mlperformance")).unwrap();
    let mut screens = String::new();
    h.pad("deck", 0, true);
    h.pad("deck", 0, false);
    h.renderer.render_f64(&mut [0.0; 64]);
    assert_eq!(
        h.app.view_model().transport.state,
        meldritch_audio::transport::TransportState::Playing
    );
    screens.push_str(&screen(&h));
    key(&mut h, KeyCode::Char('e'), KeyModifiers::CONTROL).unwrap();
    parameter(&mut h, InstrumentParameter::Waveform);
    let before_sound = pcm(&h);
    key(&mut h, KeyCode::Right, KeyModifiers::NONE).unwrap();
    assert_ne!(before_sound, pcm(&h), "waveform edit must change sound");
    parameter(&mut h, InstrumentParameter::Release);
    key(&mut h, KeyCode::Right, KeyModifiers::NONE).unwrap();
    screens.push_str(&screen(&h));
    key(&mut h, KeyCode::Char('g'), KeyModifiers::CONTROL).unwrap();
    for command in
        include_str!("../../../../songs/examples/29-lua-authored-roundtrip/compose.patch")
            .lines()
            .filter(|s| !s.starts_with('#'))
    {
        line(&mut h, command).unwrap();
    }
    screens.push_str(&screen(&h));
    key(&mut h, KeyCode::Esc, KeyModifiers::NONE).unwrap();
    key(&mut h, KeyCode::Char('n'), KeyModifiers::CONTROL).unwrap();
    h.run_script(include_str!(
        "../../../../songs/examples/29-lua-authored-roundtrip/compose.pads"
    ));
    screens.push_str(&screen(&h));
    let notes = h.app.view_model().pattern_editor.unwrap().pattern;
    assert_eq!(notes.notes.len(), 4);
    key(&mut h, KeyCode::Char('m'), KeyModifiers::CONTROL).unwrap();
    for index in 0..3 {
        if index > 0 {
            key(&mut h, KeyCode::Down, KeyModifiers::NONE).unwrap();
            key(&mut h, KeyCode::Right, KeyModifiers::NONE).unwrap();
        }
        key(&mut h, KeyCode::Enter, KeyModifiers::NONE).unwrap();
    }
    let definition = h.app.assignment_view().unwrap().definition;
    assert_eq!(
        definition
            .faders
            .iter()
            .map(|f| f.target.as_deref().unwrap())
            .collect::<Vec<_>>(),
        ["cutoff", "volume", "pan"]
    );
    let mut pickup = MidiPickup::default();
    for (number, acquire, change) in [(48, 27, 127), (49, 64, 32), (50, 64, 127)] {
        let before = h
            .app
            .view_model()
            .instrument_editor
            .unwrap()
            .source
            .fingerprint();
        assert!(!cc(&mut h, &mut pickup, number, 0));
        assert_eq!(
            before,
            h.app
                .view_model()
                .instrument_editor
                .unwrap()
                .source
                .fingerprint()
        );
        assert!(cc(&mut h, &mut pickup, number, acquire));
        h.wait();
        let before = h.reader.snapshot();
        let before_sound = pcm(&h);
        assert!(cc(&mut h, &mut pickup, number, change));
        h.wait();
        assert!(!std::sync::Arc::ptr_eq(&before, &h.reader.snapshot()));
        assert_ne!(
            before_sound,
            pcm(&h),
            "fader {number} must change rendered sound"
        );
    }
    screens.push_str(&screen(&h));
    assert!(
        h.app
            .assignment_view()
            .unwrap()
            .observations
            .values()
            .all(|(_, picked)| *picked)
    );
    for code in ['n', 'e', 'm'] {
        key(&mut h, KeyCode::Char(code), KeyModifiers::CONTROL).unwrap();
        screens.push_str(&screen(&h));
        h.renderer.render_f64(&mut [0.0; 16]);
        assert_eq!(
            h.app.view_model().transport.state,
            meldritch_audio::transport::TransportState::Playing
        );
    }
    key(&mut h, KeyCode::Esc, KeyModifiers::NONE).unwrap();
    key(&mut h, KeyCode::Tab, KeyModifiers::CONTROL).unwrap();
    assert_eq!(h.app.view_model().cockpit_mode, CockpitMode::AllParameters);
    key(&mut h, KeyCode::Tab, KeyModifiers::CONTROL).unwrap();
    assert_eq!(h.app.view_model().cockpit_mode, CockpitMode::Performance);
    screens.push_str(&screen(&h));
    h.performance.plane.refresh(&h.app).unwrap();
    let apc = h.performance.plane.output("apc-mini").unwrap();
    let terminal = h.performance.plane.output("terminal").unwrap();
    for cell in 0..64 {
        assert_eq!(apc.cells[&cell].label, terminal.cells[&cell].label);
        assert_eq!(apc.cells[&cell].color, terminal.cells[&cell].color);
    }
    assert_eq!(
        h.performance.plane.output("deck").unwrap().cells[&0].style,
        "lcd"
    );
    let telemetry = &h.performance.plane.output("deck").unwrap().cells[&4].label;
    for expected in ["Notes 4", "cutoff 6000", "volume 0.5", "pan 1"] {
        assert!(telemetry.contains(expected), "{telemetry}");
    }
    let unchanged = state(&h);
    for cell in [1, 2, 0] {
        h.pad("deck", cell, true);
        h.pad("deck", cell, false);
        h.renderer.render_f64(&mut [0.0; 16]);
        h.host.refresh_view(&mut h.app);
        h.performance.plane.refresh(&h.app).unwrap();
        assert_eq!(
            h.app.view_model().transport.state,
            if cell == 0 {
                meldritch_audio::transport::TransportState::Playing
            } else {
                meldritch_audio::transport::TransportState::Stopped
            }
        );
        assert_eq!(state(&h), unchanged);
    }
    let view = h.app.view_model();
    let source = encode_project(&view).unwrap();
    assert_eq!(source, encode_project(&view).unwrap());
    // Actual performer save command; host reports its unique path to every editor.
    key(&mut h, KeyCode::F(6), KeyModifiers::NONE).unwrap();
    let message = h
        .app
        .view_model()
        .instrument_editor
        .unwrap()
        .save_status
        .unwrap();
    assert!(message.starts_with("Project saved:"), "{message}");
    // The simulated Stream Deck invokes the same Lua save request and preserves the first export.
    h.pad("deck", 3, true);
    h.pad("deck", 3, false);
    let exports = std::fs::read_dir(h.root.join("authored"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect::<Vec<_>>();
    assert_eq!(exports.len(), 2);
    for path in &exports {
        assert_eq!(source, std::fs::read_to_string(path).unwrap());
    }
    let saved = std::fs::read_dir(h.root.join("authored"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    assert_eq!(source, std::fs::read_to_string(saved).unwrap());
    std::fs::write(h.root.join("edited-project.lua"), &source).unwrap();
    let bytes = pcm(&h);
    let saved_state = state(&h);
    for expected in [
        "Parameter: Release",
        "env.Release = 0.10000",
        "End (exclusive)",
        "note-4       C3          4800             6240",
        "track:bass/volume value Some(0.5)",
        "track:bass/pan value Some(1.0)",
        "Selected fader-3: raw Some(127), acquired",
        "F6 saves composed Lua",
    ] {
        assert!(
            screens.contains(expected),
            "missing native TUI projection: {expected}"
        );
    }
    let mut callback = vec![0.0; 4096];
    h.renderer.render_f64(&mut callback);
    assert!(callback.iter().any(|s| s.abs() > 1e-6));
    finish(&mut h);
    assert_eq!(
        base_bytes,
        std::fs::read(h.root.join("main.mlperformance")).unwrap()
    );
    let song = output.join("song");
    copy_tree(&h.root, &song);
    std::fs::write(output.join("saved-state.json"), saved_state).unwrap();
    std::fs::write(output.join("saved.f64le"), bytes).unwrap();
    std::fs::write(output.join("views.txt"), screens).unwrap();
    song
}
pub fn reload(song: &Path, output: &Path) {
    let mut h =
        piano::Harness::from_fixture_with_project(song, Some(Path::new("edited-project.lua")));
    assert_eq!(
        h.app.view_model().transport.state,
        meldritch_audio::transport::TransportState::Stopped
    );
    assert!(h.app.assignment_view().unwrap().observations.is_empty());
    assert!(
        h.app
            .view_model()
            .pattern_editor
            .unwrap()
            .selected_note
            .is_none()
    );
    let view = h.app.view_model();
    assert_eq!(
        std::fs::read_to_string(song.join("edited-project.lua")).unwrap(),
        encode_project(&view).unwrap()
    );
    std::fs::write(output.join("reloaded-state.json"), state(&h)).unwrap();
    std::fs::write(output.join("reloaded.f64le"), pcm(&h)).unwrap();
    finish(&mut h);
    std::fs::copy(
        h.capture.path(),
        output.join("reloaded-session.mlperformance"),
    )
    .unwrap();
}
