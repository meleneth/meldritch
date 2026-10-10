//! Executable example: real app commands, simulated surfaces, no host hardware.
use meldritch_app::{AppController, Selection};
use meldritch_audio::device_output::playback_session_parts;
use meldritch_control::{ControlPlane, Input, LuaProject};
use meldritch_core::{FrameRange, Pattern, PatternId, ProbabilitySeed, StepIndex, Tempo, TrackId};
use meldritch_render::RenderSettings;
use meldritch_render::coordinator::{
    RenderCoordinator, RenderCoordinatorConfig, SampleRenderState,
};
use meldritch_render::live_edit::LivePatternEditor;
use meldritch_render::song_render::compile_drone_song;
use std::{collections::BTreeMap, path::PathBuf, sync::Arc, time::Duration};

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let root = args.next().map(PathBuf::from).unwrap_or_else(|| {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../songs/examples/18-lua-control-plane")
    });
    let source = std::fs::read_to_string(root.join("main.lua")).map_err(|e| e.to_string())?;
    let project = LuaProject::load(&root, &source)?;
    let patch = compile_drone_song(&project.song).map_err(|e| format!("{e:?}"))?;
    let audio = patch
        .render(FrameRange::new(0, 4096).unwrap())
        .map_err(|e| format!("{e:?}"))?;
    println!(
        "validated {} fingerprint {:016x}; rendered {} frames, peak {:.3}",
        project.id,
        project.song.fingerprint(),
        audio.frames(),
        audio.peak_abs()
    );
    let (playback, _engine) = playback_session_parts(8)?;
    let pattern = Pattern::new(PatternId::new(1), 4, 4).unwrap();
    let state = SampleRenderState::new(
        pattern,
        Tempo::new(120.0, 48000).unwrap(),
        ProbabilitySeed::new(1),
        RenderSettings::new(1).unwrap(),
        Arc::new(BTreeMap::new()),
    );
    let coordinator = RenderCoordinator::new(
        RenderCoordinatorConfig::new(1, 24000, 6000, 1, Duration::from_millis(2)).unwrap(),
        state.pattern().clone(),
        state.tempo(),
        state.probability_seed(),
        state.settings(),
        Arc::clone(state.samples_by_note()),
        playback.status_monitor(),
    )
    .map_err(|e| format!("{e:?}"))?;
    let mut app = AppController::new(
        playback,
        coordinator,
        LivePatternEditor::new(state, 24000),
        Selection {
            track: TrackId::new(1),
            step: StepIndex::new(0),
        },
        256,
    );
    let mut plane = ControlPlane::new(project, &app)?;
    let mut inputs = args.collect::<Vec<_>>();
    if inputs.is_empty() {
        inputs = vec!["apc:0".into(), "deck:31".into(), "deck:30".into()];
    }
    if inputs.len() > 64 {
        return Err("demo accepts at most 64 inputs".into());
    }
    for (index, input) in inputs.iter().enumerate() {
        let (surface, cell) = input.split_once(':').ok_or("input must be surface:cell")?;
        let input = match cell {
            "disconnect" => Input::Connection(false),
            "connect" => Input::Connection(true),
            other => Input::Press(other.parse().map_err(|_| "cell must be a number")?),
        };
        plane.enqueue(surface, index as u64 * 480, input)?;
        plane.pump(&mut app);
    }
    for record in plane.trace() {
        println!(
            "{} @{} {} {:?}: {}",
            record.sequence, record.frame, record.source, record.phase, record.detail
        );
    }
    for surface in ["apc", "deck", "terminal"] {
        println!("{surface}: {:?}", plane.output(surface));
    }
    let path = plane
        .take_journal()
        .write_session("/tmp/meldritch-lua-headless")?;
    println!("accepted-command session: {}", path.display());
    Ok(())
}
