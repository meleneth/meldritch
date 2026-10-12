//! Native project adapter to the same song worker/publication/device boundary.
use meldritch_app::{AppCommand, AppController};
use meldritch_audio::{
    AudioBlock,
    audio_publication::{AudioSnapshotReader, audio_publication},
    device_output::{PlaybackEngine, RealtimePlaybackSession},
    published_audio::PublishedAudio,
    transport::TransportState,
};
use meldritch_control::song_host::SongAudioHost;
use meldritch_render::song_host::CompiledSongPatch;
use meldritch_tui::ProjectPlaybackHost;

pub(crate) struct NativeProjectAudio {
    capture: meldritch_control::native_capture::NativeProjectCapture,
    host: SongAudioHost,
    reader: AudioSnapshotReader,
    engine: Option<PlaybackEngine>,
    stream: Option<RealtimePlaybackSession>,
    submitted_fingerprint: u64,
    ready_generation: Option<u64>,
    frames: u32,
}

impl NativeProjectAudio {
    pub(crate) fn new(app: &AppController, engine: PlaybackEngine) -> Result<Self, String> {
        let source = app.authored_source().ok_or("no authored project source")?;
        let frames = u32::try_from(source.authoring_frame(3840)?)
            .map_err(|_| "project loop exceeds PCM capacity")?;
        let initial = PublishedAudio::from_block(&AudioBlock::silent(2, frames), 6000)
            .map_err(|error| format!("invalid project publication: {error:?}"))?;
        let (publisher, reader) = audio_publication(initial);
        let patch = CompiledSongPatch::compile_initial(&source)?;
        let host = SongAudioHost::new(source.as_ref().clone(), patch, publisher, frames, 6000);
        Ok(Self {
            capture: meldritch_control::native_capture::NativeProjectCapture::new(
                source.clone(),
                frames,
            ),
            host,
            reader,
            engine: Some(engine),
            stream: None,
            submitted_fingerprint: source.fingerprint(),
            ready_generation: None,
            frames,
        })
    }
}

impl ProjectPlaybackHost for NativeProjectAudio {
    fn prepare_action(&mut self, app: &AppController) -> Result<(), String> {
        self.capture.checkpoint(app)
    }
    fn finish(&mut self, app: &AppController) -> Result<(), String> {
        self.capture.finish(app)
    }
    fn poll(&mut self, app: &mut AppController) -> Result<(), String> {
        self.capture
            .checkpoint(app)
            .map_err(|error| format!("capture checkpoint: {error}"))?;
        let source = app.authored_source().ok_or("no authored project source")?;
        if source.fingerprint() != self.submitted_fingerprint {
            // This first native playback slice has one fixed one-bar PCM layout.
            // Other clocks must not silently be truncated or repeat incorrectly.
            for voice in source.pattern_voices().values() {
                if !voice.looped
                    || source.authoring_frame(voice.length_ticks)? != u64::from(self.frames)
                {
                    return Err(
                        "native playback currently requires looping one-bar instances".into(),
                    );
                }
            }
            self.host.replace_source(source.as_ref().clone())?;
            self.submitted_fingerprint = source.fingerprint();
        }
        let levels = app
            .output_levels()
            .iter()
            .filter_map(|((voice, control), value)| {
                source
                    .validate_output_level_control(voice, control)
                    .ok()
                    .map(|_| (voice.clone(), *value))
            })
            .collect();
        self.host.set_voice_levels(levels)?;
        if let Some(generation) = self.host.poll_publish()? {
            self.ready_generation = Some(generation);
        }
        self.host.refresh_view(app);
        Ok(())
    }

    fn toggle_playback(&mut self, app: &mut AppController) -> Result<(), String> {
        if app.playback_control().status_monitor().snapshot().state == TransportState::Playing {
            return app
                .dispatch(AppCommand::Stop)
                .map(|_| ())
                .map_err(|error| format!("{error:?}"));
        }
        self.poll(app)?;
        let source = app.authored_source().ok_or("no authored project source")?;
        if source.pattern_voices().is_empty() {
            return Err("create an explicit patch instance with v before playback".into());
        }
        if self.ready_generation != Some(self.host.generation()) {
            return Err("latest audio is still preparing; press Space again when ready".into());
        }
        if self.stream.is_none() {
            let engine = self.engine.take().ok_or("device initialization failed; playback is unavailable in this cockpit; authored edits are retained")?;
            self.stream = Some(RealtimePlaybackSession::open_default(
                self.reader.clone(),
                source.performance().sample_rate(),
                u32::MAX,
                app.playback_control(),
                engine,
            )?);
        }
        app.dispatch(AppCommand::Play)
            .map(|_| ())
            .map_err(|error| format!("{error:?}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use meldritch_app::{AppInput, NoteEdit, PatchCreation, PatternSignal, PatternVoiceDefinition};
    use meldritch_audio::device_output::PublishedAudioRenderer;
    use std::{
        path::PathBuf,
        sync::Arc,
        time::{Duration, Instant},
    };

    struct ProjectFixture {
        path: PathBuf,
        project: meldritch_dsl::ProjectDirectory,
    }
    impl std::ops::Deref for ProjectFixture {
        type Target = meldritch_dsl::ProjectDirectory;
        fn deref(&self) -> &Self::Target {
            &self.project
        }
    }
    impl Drop for ProjectFixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }
    fn project() -> ProjectFixture {
        let path = std::env::temp_dir().join(format!(
            "meldritch-native-host-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&path).unwrap();
        let project = meldritch_dsl::load_project_directory(&path).unwrap();
        ProjectFixture { path, project }
    }

    #[test]
    fn native_host_reuses_worker_and_callback_without_opening_devices_or_resetting_mode_transport()
    {
        let project = project();
        let (mut app, engine) = AppController::new_empty_project(&project).unwrap();
        let mut host = NativeProjectAudio::new(&app, engine).unwrap();
        host.poll(&mut app).unwrap();
        assert_eq!(std::fs::read_dir(project.root()).unwrap().count(), 0);
        assert!(host.stream.is_none());
        assert!(
            host.toggle_playback(&mut app)
                .unwrap_err()
                .contains("instance")
        );
        assert!(host.engine.is_some());
        app.dispatch(AppCommand::CreatePatch(PatchCreation::mono_voice(
            "voice-1",
        )))
        .unwrap();
        app.dispatch(AppCommand::CreatePalettePattern {
            id: "pattern-1".into(),
            length_ticks: 3840,
            looped: true,
        })
        .unwrap();
        app.dispatch(AppCommand::EditNote {
            pattern: "pattern-1".into(),
            edit: NoteEdit::Insert {
                start_tick: 0,
                duration_ticks: 480,
                pitch: 48,
                velocity: 0.8,
            },
        })
        .unwrap();
        app.dispatch(AppCommand::CreatePatternVoice(PatternVoiceDefinition {
            id: "voice-1-instance".into(),
            synth: "voice-1".into(),
            length_ticks: 3840,
            looped: true,
            initial_pitch: 48,
            level: 0.8,
            pitch_pattern: Some("pattern-1".into()),
            gate_pattern: Some("pattern-1".into()),
        }))
        .unwrap();
        let wait_ready = |host: &mut NativeProjectAudio, app: &mut AppController| {
            let deadline = Instant::now() + Duration::from_secs(5);
            loop {
                host.poll(app).unwrap();
                if host.ready_generation == Some(host.host.generation()) {
                    break;
                }
                assert!(Instant::now() < deadline);
                std::thread::sleep(Duration::from_millis(2));
            }
        };
        app.dispatch(AppCommand::ExposeOutputLevel {
            synth: "voice-1".into(),
            control: "level".into(),
        })
        .unwrap();
        wait_ready(&mut host, &mut app);
        let source = app.authored_source().unwrap();
        let mut callback = PublishedAudioRenderer::from_engine(
            host.reader.clone(),
            2,
            u32::MAX,
            host.engine.take().unwrap(),
        )
        .unwrap();
        app.dispatch(AppCommand::Play).unwrap();
        let mut samples = vec![0.0; 512];
        callback.render_f64(&mut samples);
        assert!(samples.iter().any(|sample| sample.abs() > 0.01));
        let control = |value| AppCommand::SetOutputLevel {
            voice: "voice-1-instance".into(),
            control: "level".into(),
            value,
        };
        let history = app.history().len();
        for value in [f64::NAN, f64::INFINITY, -0.1, 1.1] {
            assert!(app.dispatch(control(Some(value))).is_err());
        }
        assert_eq!(app.history().len(), history);
        assert!(Arc::ptr_eq(&source, &app.authored_source().unwrap()));
        app.dispatch(control(Some(0.0))).unwrap();
        assert!(app.history().back().unwrap().changed);
        assert_eq!(app.history().back().unwrap().dirty_ranges.len(), 1);
        wait_ready(&mut host, &mut app);
        assert!(Arc::ptr_eq(&source, &app.authored_source().unwrap()));
        assert_eq!(source.pattern_voices()["voice-1-instance"].level, 0.8);
        callback.render_f64(&mut samples);
        assert!(samples.iter().all(|sample| *sample == 0.0));
        assert_eq!(
            app.playback_control().status_monitor().snapshot().position,
            512
        );
        let generation = host.host.generation();
        let muted = host.reader.snapshot();
        app.dispatch(control(Some(0.0))).unwrap();
        host.poll(&mut app).unwrap();
        assert!(!app.history().back().unwrap().changed);
        assert_eq!(host.host.generation(), generation);
        assert!(Arc::ptr_eq(&muted, &host.reader.snapshot()));
        app.dispatch(control(None)).unwrap();
        wait_ready(&mut host, &mut app);
        callback.render_f64(&mut samples);
        assert!(samples.iter().any(|sample| sample.abs() > 0.01));
        assert!(app.view_model().output_levels.is_empty());
        let snapshot = host.reader.snapshot();
        for _ in 0..2 {
            app.handle_input(AppInput::ToggleCockpitMode).unwrap();
            host.poll(&mut app).unwrap();
            assert!(Arc::ptr_eq(&source, &app.authored_source().unwrap()));
            assert!(Arc::ptr_eq(&snapshot, &host.reader.snapshot()));
            assert_eq!(
                app.playback_control().status_monitor().snapshot().position,
                768
            );
        }
        app.dispatch(AppCommand::BindPatternVoice {
            voice: "voice-1-instance".into(),
            signal: PatternSignal::Gate,
            pattern: None,
        })
        .unwrap();
        wait_ready(&mut host, &mut app);
        callback.render_f64(&mut samples);
        assert!(samples.iter().all(|sample| *sample == 0.0));
        assert_eq!(
            app.playback_control().status_monitor().snapshot().position,
            1024
        );
        assert!(host.stream.is_none());
        host.finish(&app).unwrap();
        let captured: toml::Table = toml::from_str(
            &std::fs::read_to_string(host.capture.session.as_ref().unwrap().path()).unwrap(),
        )
        .unwrap();
        assert!(captured["session"]["clean_termination"].as_bool().unwrap());
        assert_eq!(
            captured["events"].as_array().unwrap().len(),
            app.history().len()
        );
        for (event, record) in captured["events"]
            .as_array()
            .unwrap()
            .iter()
            .zip(app.history())
        {
            assert_eq!(
                event["absolute_frame"].as_integer(),
                Some(record.observed_frame as i64)
            );
        }
    }

    #[test]
    fn unsupported_native_clock_leaves_the_last_publication_and_worker_generation_unchanged() {
        let project = project();
        let (mut app, engine) = AppController::new_empty_project(&project).unwrap();
        let mut host = NativeProjectAudio::new(&app, engine).unwrap();
        app.dispatch(AppCommand::CreatePatch(PatchCreation::mono_voice(
            "voice-1",
        )))
        .unwrap();
        app.dispatch(AppCommand::CreatePatternVoice(PatternVoiceDefinition {
            id: "short".into(),
            synth: "voice-1".into(),
            length_ticks: 1920,
            looped: true,
            initial_pitch: 48,
            level: 0.8,
            pitch_pattern: None,
            gate_pattern: None,
        }))
        .unwrap();
        let audio = host.reader.snapshot();
        let source = app.authored_source().unwrap();
        let generation = host.host.generation();
        assert!(host.poll(&mut app).unwrap_err().contains("one-bar"));
        assert_eq!(host.host.generation(), generation);
        assert!(Arc::ptr_eq(&audio, &host.reader.snapshot()));
        assert!(Arc::ptr_eq(&source, &app.authored_source().unwrap()));
        assert!(host.stream.is_none());
    }
}
