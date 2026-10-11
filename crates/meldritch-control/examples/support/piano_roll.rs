//! Shared headless fixture host: production Lua/MIDI/command/capture/render paths.
#![allow(dead_code)] // The executable and integration tests inspect different host fields.
use meldritch_app::*;
use meldritch_audio::device_output::{PublishedAudioRenderer, playback_session_parts};
use meldritch_audio::{
    audio_publication::{AudioSnapshotPublisher, AudioSnapshotReader, audio_publication},
    published_audio::PublishedAudio,
};
use meldritch_control::{
    LuaProject,
    devices::{ApcInput, decode_apc},
    performance::LuaPerformance,
    session::PerformanceSessionCapture,
    song_host::SongAudioHost,
};
use meldritch_core::{FrameRange, Pattern, PatternId, ProbabilitySeed, StepIndex, Tempo, TrackId};
use meldritch_render::{
    RenderSettings,
    coordinator::{RenderCoordinator, RenderCoordinatorConfig, SampleRenderState},
    live_edit::LivePatternEditor,
    song_host::CompiledSongPatch,
};
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

pub struct Harness {
    pub app: AppController,
    pub host: SongAudioHost,
    pub performance: LuaPerformance,
    pub capture: PerformanceSessionCapture,
    pub reader: AudioSnapshotReader,
    pub renderer: PublishedAudioRenderer,
    pub root: PathBuf,
    pub base: ValidatedSong,
    frame: u64,
}
pub fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../songs/examples/26-apc-note-pattern")
}
impl Harness {
    pub fn new() -> Self {
        Self::from_fixture(&fixture())
    }
    pub fn from_fixture(fixture: &std::path::Path) -> Self {
        Self::from_fixture_with_project(fixture, None)
    }
    pub fn from_fixture_with_project(
        fixture: &std::path::Path,
        authored: Option<&std::path::Path>,
    ) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "meldritch-piano-roll-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fn copy(from: &std::path::Path, to: &std::path::Path) {
            std::fs::create_dir_all(to).unwrap();
            for entry in std::fs::read_dir(from).unwrap() {
                let entry = entry.unwrap();
                if entry.file_name() == "performances" {
                    continue;
                }
                if entry.file_type().unwrap().is_dir() {
                    copy(&entry.path(), &to.join(entry.file_name()));
                } else {
                    std::fs::copy(entry.path(), to.join(entry.file_name())).unwrap();
                }
            }
        }
        copy(fixture, &root);
        let base = meldritch_dsl::load_song_directory(&root).unwrap();
        let source = std::fs::read_to_string(root.join("main.lua")).unwrap();
        let project = LuaProject::load(&root, &source).unwrap();
        let restored = authored
            .map(|file| meldritch_control::project::load_project_file(&root, file).unwrap());
        let song = restored.as_ref().map_or(&base, |p| &p.song);
        let patch = CompiledSongPatch::compile_initial(song).unwrap();
        let mut audio = patch
            .render_default(FrameRange::new(0, 192000).unwrap())
            .unwrap();
        if audio.channels() == 1
            && project.assignments.as_ref().is_some_and(|a| {
                a.targets.iter().any(|t| {
                    matches!(
                        t.target,
                        EditableTarget::Track {
                            parameter: meldritch_dsl::MixerParameter::Pan,
                            ..
                        }
                    )
                })
            })
        {
            let mut stereo = meldritch_audio::AudioBlock::silent(2, audio.frames());
            for (pair, sample) in stereo
                .samples_mut()
                .chunks_exact_mut(2)
                .zip(audio.samples())
            {
                pair.fill(*sample);
            }
            audio = stereo;
        }
        let (publisher, reader): (AudioSnapshotPublisher, AudioSnapshotReader) =
            audio_publication(PublishedAudio::from_block(&audio, 1024).unwrap());
        let (playback, engine) = playback_session_parts(64).unwrap();
        let renderer =
            PublishedAudioRenderer::from_engine(reader.clone(), audio.channels(), u32::MAX, engine)
                .unwrap();
        let state = SampleRenderState::new(
            Pattern::new(PatternId::new(1), 1, 16).unwrap(),
            Tempo::new(120.0, 48000).unwrap(),
            ProbabilitySeed::new(1),
            RenderSettings::new(1).unwrap(),
            Arc::new(BTreeMap::new()),
        );
        let coordinator = RenderCoordinator::new_from_state(
            RenderCoordinatorConfig::new(1, 192000, 6000, 1, Duration::from_millis(2)).unwrap(),
            state.clone(),
            playback.status_monitor(),
        )
        .unwrap();
        let mut app = AppController::new(
            playback,
            coordinator,
            LivePatternEditor::new(state, 192000),
            Selection {
                track: TrackId::new(1),
                step: StepIndex::new(0),
            },
            256,
        );
        if let Some(p) = &restored {
            app.enable_project_authoring_from(
                base.clone(),
                p.song.clone(),
                &p.instrument,
                &p.pattern,
                p.assignments.clone(),
            )
            .unwrap();
        } else {
            app.enable_instrument_editor(base.clone()).unwrap();
            app.enable_pattern_editor(base.performance().tracks()[0].initial_pattern().unwrap())
                .unwrap();
            if let Some(a) = project.assignments.clone() {
                app.enable_assignments(a).unwrap();
            }
        }
        let performance =
            LuaPerformance::new(project, &app, Tempo::new(120.0, 48000).unwrap()).unwrap();
        let capture = PerformanceSessionCapture::create(song, 192000).unwrap();
        let host = SongAudioHost::new(song.clone(), patch, publisher, 192000, 1024);
        Self {
            app,
            host,
            performance,
            capture,
            reader,
            renderer,
            root,
            base,
            frame: 0,
        }
    }
    pub fn midi(&mut self, bytes: &[u8]) -> bool {
        let decoded = match decode_apc(0, bytes) {
            Some(ApcInput::Grid { cell, pressed }) => Some((cell, pressed)),
            Some(ApcInput::Button { note, pressed }) => Some((note, pressed)),
            _ => None,
        };
        let Some((cell, pressed)) = decoded else {
            return false;
        };
        self.pad("apc-mini", cell, pressed);
        true
    }
    pub fn pad(&mut self, source: &str, cell: u8, pressed: bool) {
        self.performance
            .pad(
                source,
                cell,
                pressed,
                self.frame,
                &mut self.app,
                &mut self.host,
                &mut self.capture,
            )
            .unwrap();
        self.frame += 16;
    }
    pub fn wait(&mut self) {
        let deadline = Instant::now() + Duration::from_secs(3);
        while self.host.poll_publish().unwrap().is_none() {
            assert!(Instant::now() < deadline, "publication timed out");
            std::thread::sleep(Duration::from_millis(2));
        }
        self.host.refresh_view(&mut self.app);
        self.performance.plane.refresh(&self.app).unwrap();
    }
    pub fn run_script(&mut self, source: &str) {
        for line in source
            .lines()
            .filter(|l| !l.trim().is_empty() && !l.trim().starts_with('#'))
        {
            let fields: Vec<_> = line.split_whitespace().collect();
            match fields[0] {
                "press" | "release" | "zero" => {
                    let cell = fields[1].parse::<u8>().unwrap();
                    let bytes = match fields[0] {
                        "press" => [0x90, cell, 127],
                        "zero" => [0x90, cell, 0],
                        _ => [0x80, cell, 0],
                    };
                    assert!(self.midi(&bytes));
                }
                "cancel" => self
                    .performance
                    .cancel("apc-mini", &mut self.app, &mut self.host, &mut self.capture)
                    .unwrap(),
                "disconnect" | "connect" => self
                    .performance
                    .connection(
                        "apc-mini",
                        fields[0] == "connect",
                        &mut self.app,
                        &mut self.host,
                        &mut self.capture,
                    )
                    .unwrap(),
                "wait" => self.wait(),
                "rewind" => {
                    self.app.dispatch(AppCommand::Rewind).unwrap();
                    self.renderer.render_f64(&mut [0.0; 1]);
                }
                "expect_count" => assert_eq!(
                    self.app
                        .view_model()
                        .pattern_editor
                        .unwrap()
                        .pattern
                        .notes
                        .len(),
                    fields[1].parse::<usize>().unwrap(),
                    "{line}"
                ),
                "expect_note" => {
                    let p = self.app.view_model().pattern_editor.unwrap().pattern;
                    let n = p.notes.iter().find(|n| n.id == fields[1]).unwrap();
                    assert_eq!(n.note, fields[2], "{line}");
                    assert_eq!(n.start_tick, fields[3].parse::<u64>().unwrap(), "{line}");
                    assert_eq!(
                        n.duration_ticks,
                        fields[4].parse::<u64>().unwrap(),
                        "{line}"
                    );
                    assert!(
                        (n.velocity - fields[5].parse::<f64>().unwrap()).abs() < 1e-12,
                        "{line}"
                    );
                }
                _ => panic!("unknown script directive {line}"),
            }
        }
    }
}
impl Drop for Harness {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}
