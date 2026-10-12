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
    store: meldritch_control::native_project::NativeProjectStore,
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
    fn require_prepared_audio(&self, source: &meldritch_app::ValidatedSong) -> Result<(), String> {
        if source.fingerprint() != self.submitted_fingerprint
            || self.ready_generation != Some(self.host.generation())
        {
            return Err("latest audio is still preparing; press Space again when ready".into());
        }
        Ok(())
    }
    #[cfg(test)]
    pub(crate) fn new(app: &AppController, engine: PlaybackEngine) -> Result<Self, String> {
        let source = app.authored_source().ok_or("no authored project source")?;
        let store = meldritch_control::native_project::NativeProjectStore::open(
            &meldritch_dsl::load_project_directory(source.root()).map_err(|e| e.to_string())?,
        )?;
        Self::with_store(app, engine, store)
    }
    pub(crate) fn with_store(
        app: &AppController,
        engine: PlaybackEngine,
        store: meldritch_control::native_project::NativeProjectStore,
    ) -> Result<Self, String> {
        let source = app.authored_source().ok_or("no authored project source")?;
        let frames = u32::try_from(source.authoring_frame(3840)?)
            .map_err(|_| "project loop exceeds PCM capacity")?;
        let initial = PublishedAudio::from_block(&AudioBlock::silent(2, frames), 6000)
            .map_err(|error| format!("invalid project publication: {error:?}"))?;
        let (publisher, reader) = audio_publication(initial);
        let patch = CompiledSongPatch::compile_initial(&source)?;
        let mut host = SongAudioHost::new(source.as_ref().clone(), patch, publisher, frames, 6000);
        if !source.pattern_voices().is_empty() || !source.compositions().is_empty() {
            for voice in source.pattern_voices().values() {
                if !voice.looped || source.authoring_frame(voice.length_ticks)? != u64::from(frames)
                {
                    return Err(
                        "native playback currently requires looping one-bar instances".into(),
                    );
                }
            }
            host.replace_source(source.as_ref().clone())?;
        }
        Ok(Self {
            store,
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
    fn persistence_status(&self, app: &AppController) -> String {
        if app
            .authored_source()
            .is_some_and(|source| Some(source.fingerprint()) == self.store.saved_fingerprint())
        {
            "Saved authored project · main.project.lua".into()
        } else {
            "Unsaved authored project · Ctrl-S/F6 to save".into()
        }
    }
    fn save_project(&mut self, app: &mut AppController) -> Result<String, String> {
        app.dispatch(AppCommand::SaveProject)
            .map_err(|e| format!("{e:?}"))?;
        self.capture.checkpoint(app)?;
        let path = self.store.save(
            app.authored_source()
                .ok_or("no authored project source")?
                .as_ref(),
        )?;
        Ok(format!(
            "Saved {} · runtime gain and capture remain separate",
            path.file_name().unwrap().to_string_lossy()
        ))
    }
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
                    .filter(|voice| source.is_pattern_voice_active(&voice.id))
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
        if source.pattern_voices().is_empty() && source.selected_composition().is_none() {
            return Err("create an explicit patch instance with v before playback".into());
        }
        self.require_prepared_audio(&source)?;
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
    fn native_composition_transition_requires_stopped_transport_and_prepared_audio() {
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        let project = project();
        let source = meldritch_control::native_project::load_native_project(
            project.root(),
            include_str!(
                "../../../songs/examples/35-project-palette-routing/compositions.project.lua"
            ),
        )
        .unwrap();
        let (mut app, engine) = AppController::new_empty_project(&project).unwrap();
        app.enable_native_project_source(source).unwrap();
        let mut host = NativeProjectAudio::new(&app, engine).unwrap();
        let ready = |app: &mut AppController, host: &mut NativeProjectAudio| {
            let deadline = Instant::now() + Duration::from_secs(3);
            loop {
                host.poll(app).unwrap();
                if host
                    .require_prepared_audio(&app.authored_source().unwrap())
                    .is_ok()
                {
                    break;
                }
                assert!(Instant::now() < deadline, "composition render timeout");
                std::thread::sleep(Duration::from_millis(1));
            }
        };
        app.dispatch(AppCommand::SetOutputLevel {
            voice: "bass-copy".into(),
            control: "level".into(),
            value: Some(0.2),
        })
        .unwrap();
        ready(&mut app, &mut host);
        let mut callback = PublishedAudioRenderer::from_engine(
            host.reader.clone(),
            2,
            u32::MAX,
            host.engine.take().unwrap(),
        )
        .unwrap();
        let source = app.authored_source().unwrap();
        let published = host.reader.snapshot();
        let generation = host.host.generation();
        let next = |app: &AppController, code| {
            meldritch_tui::project_composition_command(
                KeyEvent::new(code, KeyModifiers::NONE),
                &app.view_model(),
                &app.authored_source().unwrap(),
            )
            .unwrap()
        };
        app.dispatch(AppCommand::Play).unwrap();
        let mut samples = vec![0.0; 512];
        callback.render_f64(&mut samples);
        let history = app.history().len();
        assert!(app.dispatch(next(&app, KeyCode::F(8))).is_err());
        assert!(app.dispatch(next(&app, KeyCode::Char('d'))).is_err());
        assert!(Arc::ptr_eq(&source, &app.authored_source().unwrap()));
        assert!(Arc::ptr_eq(&published, &host.reader.snapshot()));
        assert_eq!(host.host.generation(), generation);
        assert_eq!(app.history().len(), history);
        app.dispatch(AppCommand::SetCockpitMode(
            meldritch_app::CockpitMode::Performance,
        ))
        .unwrap();
        let select = meldritch_tui::project_level_command(
            KeyEvent::new(KeyCode::Down, KeyModifiers::NONE),
            &app.view_model(),
            &source,
        )
        .unwrap();
        app.dispatch(select).unwrap();
        assert_eq!(
            app.view_model().selected_pattern_voice.as_deref(),
            Some("voice-1-instance")
        );
        host.poll(&mut app).unwrap();
        assert_eq!(host.host.generation(), generation);
        assert!(Arc::ptr_eq(&published, &host.reader.snapshot()));
        assert!(Arc::ptr_eq(&source, &app.authored_source().unwrap()));
        callback.render_f64(&mut samples);
        assert!(samples.iter().any(|sample| sample.abs() > 0.01));
        app.dispatch(AppCommand::SetCockpitMode(
            meldritch_app::CockpitMode::AllParameters,
        ))
        .unwrap();
        app.dispatch(AppCommand::Stop).unwrap();
        // Stop must be observed by the callback, not merely queued.
        assert!(app.dispatch(next(&app, KeyCode::F(8))).is_err());
        callback.render_f64(&mut samples);
        app.dispatch(next(&app, KeyCode::F(8))).unwrap();
        let selected = app.authored_source().unwrap();
        assert_eq!(selected.selected_composition(), Some("pulse"));
        assert_eq!(selected.synths(), source.synths());
        assert!(Arc::ptr_eq(&published, &host.reader.snapshot()));
        assert!(
            host.require_prepared_audio(&selected)
                .unwrap_err()
                .contains("still preparing")
        );
        ready(&mut app, &mut host);
        assert!(!Arc::ptr_eq(&published, &host.reader.snapshot()));
        assert_eq!(app.view_model().transport.state, TransportState::Stopped);
        assert_eq!(
            app.output_levels()
                .get(&("bass-copy".into(), "level".into())),
            Some(&0.2)
        );
        // Inactive held-composition gain is retained but cannot contaminate pulse.
        let expected = CompiledSongPatch::compile_initial(&selected)
            .unwrap()
            .render_default(meldritch_core::FrameRange::new(0, u64::from(host.frames)).unwrap())
            .unwrap();
        for (frame, sample) in expected.samples().iter().enumerate() {
            assert_eq!(
                host.reader.snapshot().frame(frame as u32).unwrap(),
                &[*sample, *sample]
            );
        }
        host.save_project(&mut app).unwrap();
        app.dispatch(AppCommand::Play).unwrap();
        callback.render_f64(&mut samples);
        assert!(samples.iter().any(|sample| sample.abs() > 0.01));
        app.dispatch(AppCommand::Stop).unwrap();
        callback.render_f64(&mut samples);
        app.dispatch(next(&app, KeyCode::F(7))).unwrap();
        ready(&mut app, &mut host);
        assert_eq!(
            app.authored_source().unwrap().selected_composition(),
            Some("held")
        );
        assert_eq!(
            app.output_levels()
                .get(&("bass-copy".into(), "level".into())),
            Some(&0.2)
        );
        // Construction selection is independent of the shared patch inspector.
        let select = meldritch_tui::project_binding_command(
            KeyEvent::new(KeyCode::Down, KeyModifiers::NONE),
            &app.view_model(),
            &app.authored_source().unwrap(),
        )
        .unwrap();
        app.dispatch(select).unwrap();
        assert_eq!(
            app.view_model().selected_pattern_voice.as_deref(),
            Some("bass-copy")
        );
        let published = host.reader.snapshot();
        let generation = host.host.generation();
        app.dispatch(next(&app, KeyCode::Char('d'))).unwrap();
        let removed = app.authored_source().unwrap();
        assert!(!removed.is_pattern_voice_active("bass-copy"));
        assert!(Arc::ptr_eq(&published, &host.reader.snapshot()));
        assert_eq!(host.host.generation(), generation);
        assert!(host.require_prepared_audio(&removed).is_err());
        ready(&mut app, &mut host);
        let expected = CompiledSongPatch::compile_initial(&removed)
            .unwrap()
            .render_default(meldritch_core::FrameRange::new(0, u64::from(host.frames)).unwrap())
            .unwrap();
        for (frame, sample) in expected.samples().iter().enumerate() {
            assert_eq!(
                host.reader.snapshot().frame(frame as u32).unwrap(),
                &[*sample, *sample]
            );
        }
        assert_eq!(
            app.output_levels()
                .get(&("bass-copy".into(), "level".into())),
            Some(&0.2)
        );
        app.dispatch(next(&app, KeyCode::Char('a'))).unwrap();
        ready(&mut app, &mut host);
        assert!(
            app.authored_source()
                .unwrap()
                .is_pattern_voice_active("bass-copy")
        );
        host.save_project(&mut app).unwrap();
        host.finish(&app).unwrap();
    }

    #[test]
    fn native_palette_selection_preserves_playing_audio_and_runtime_gain() {
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        let project = project();
        let source = meldritch_control::native_project::load_native_project(
            project.root(),
            include_str!("../../../songs/examples/35-project-palette-routing/main.project.lua"),
        )
        .unwrap();
        let (mut app, engine) = AppController::new_empty_project(&project).unwrap();
        app.enable_native_project_source(source).unwrap();
        let mut host = NativeProjectAudio::new(&app, engine).unwrap();
        app.dispatch(AppCommand::SetOutputLevel {
            voice: "bass-copy".into(),
            control: "level".into(),
            value: Some(0.2),
        })
        .unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        loop {
            host.poll(&mut app).unwrap();
            if host.ready_generation == Some(host.host.generation()) {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "worker publication timeout"
            );
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        let source = app.authored_source().unwrap();
        let published = host.reader.snapshot();
        let generation = host.host.generation();
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
        let position = app.view_model().transport.position;
        let command = meldritch_tui::project_pattern_command(
            KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE),
            &app.view_model(),
            &source,
        )
        .unwrap();
        app.dispatch(command).unwrap();
        host.poll(&mut app).unwrap();
        assert_eq!(
            app.view_model().palette_pattern_editor.unwrap().pattern.id,
            "rhythm"
        );
        assert!(Arc::ptr_eq(&source, &app.authored_source().unwrap()));
        assert!(Arc::ptr_eq(&published, &host.reader.snapshot()));
        assert_eq!(host.host.generation(), generation);
        assert_eq!(
            app.output_levels()
                .get(&("bass-copy".into(), "level".into())),
            Some(&0.2)
        );
        assert_eq!(app.view_model().transport.state, TransportState::Playing);
        callback.render_f64(&mut samples);
        assert!(samples.iter().any(|value| value.abs() > 0.01));
        assert!(app.view_model().transport.position > position);
        host.finish(&app).unwrap();
    }

    #[test]
    fn native_keyboard_workflow_saves_and_reopens_in_a_fresh_process() {
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        use meldritch_tui::{
            TuiAction, project_binding_command, project_construction_command,
            project_editor_action, project_level_command, project_pattern_command,
            project_save_key,
        };
        let project = project();
        let (mut app, engine) = AppController::new_empty_project(&project).unwrap();
        let mut host = NativeProjectAudio::new(&app, engine).unwrap();
        host.poll(&mut app).unwrap();
        assert_eq!(std::fs::read_dir(project.root()).unwrap().count(), 0);
        // Exercise the actual frontend's lowering helpers; only device output is replaced.
        let gesture = |app: &mut AppController, host: &mut NativeProjectAudio, key: KeyEvent| {
            host.prepare_action(app).unwrap();
            if project_save_key(key) {
                return host.save_project(app);
            }
            let model = app.view_model();
            if key.code == KeyCode::Tab && key.modifiers == KeyModifiers::CONTROL {
                app.handle_input(AppInput::ToggleCockpitMode).unwrap();
            } else {
                let source = app.authored_source().unwrap();
                let command = project_level_command(key, &model, &source)
                    .or_else(|| project_binding_command(key, &model, &source))
                    .or_else(|| project_pattern_command(key, &model, &source));
                if let Some(command) = command {
                    app.dispatch(command).unwrap();
                } else if let Some(command) = project_construction_command(key, &source) {
                    let result = app.dispatch(command).unwrap();
                    let meldritch_app::AppCommandResult::PatchEdited { synth, .. } = result else {
                        panic!("creation result")
                    };
                    app.dispatch(AppCommand::SelectInstrument(synth)).unwrap();
                } else if let Some(TuiAction::Input(input)) = project_editor_action(key, &model) {
                    app.handle_input(input).unwrap();
                } else {
                    panic!("unmapped native key {key:?}");
                }
            }
            host.poll(app).unwrap();
            Ok(String::new())
        };
        let key = |code| KeyEvent::new(code, KeyModifiers::NONE);
        let control = |code| KeyEvent::new(code, KeyModifiers::CONTROL);
        for event in [
            key(KeyCode::Char('n')),
            key(KeyCode::Char('c')),
            key(KeyCode::Enter),
            key(KeyCode::Right),
            key(KeyCode::Right),
        ] {
            gesture(&mut app, &mut host, event).unwrap();
        }
        for _ in 0..7 {
            gesture(&mut app, &mut host, key(KeyCode::Up)).unwrap();
        }
        for event in [
            key(KeyCode::Enter),
            key(KeyCode::Esc),
            key(KeyCode::Char('v')),
            key(KeyCode::Char('p')),
            key(KeyCode::Char('g')),
            key(KeyCode::Char('x')),
        ] {
            gesture(&mut app, &mut host, event).unwrap();
        }
        let source = app.authored_source().unwrap();
        let material = source.pattern_material("pattern-1").unwrap();
        assert_eq!(
            material
                .notes
                .iter()
                .map(|n| (n.note.as_str(), n.start_tick, n.duration_ticks, n.velocity))
                .collect::<Vec<_>>(),
            vec![("C3", 0, 480, 0.8), ("G3", 960, 480, 0.8)]
        );
        let ready = |app: &mut AppController, host: &mut NativeProjectAudio| {
            let deadline = Instant::now() + Duration::from_secs(5);
            while host.ready_generation != Some(host.host.generation()) {
                host.poll(app).unwrap();
                assert!(Instant::now() < deadline);
                std::thread::sleep(Duration::from_millis(2));
            }
        };
        ready(&mut app, &mut host);
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
        assert!(samples.iter().all(|v| v.is_finite()) && samples.iter().any(|v| v.abs() > 0.01));
        for _ in 0..2 {
            let published = host.reader.snapshot();
            gesture(&mut app, &mut host, control(KeyCode::Tab)).unwrap();
            assert!(Arc::ptr_eq(&published, &host.reader.snapshot()));
            assert!(Arc::ptr_eq(&source, &app.authored_source().unwrap()));
            assert_eq!(app.view_model().transport.state, TransportState::Playing);
        }
        gesture(&mut app, &mut host, key(KeyCode::Char('g'))).unwrap();
        ready(&mut app, &mut host);
        assert_eq!(
            app.authored_source().unwrap().pattern_voices()["voice-1-instance"]
                .pitch_pattern
                .as_deref(),
            Some("pattern-1")
        );
        callback.render_f64(&mut samples);
        assert!(samples.iter().all(|v| *v == 0.0));
        gesture(&mut app, &mut host, key(KeyCode::Char('g'))).unwrap();
        ready(&mut app, &mut host);
        callback.render_f64(&mut samples);
        assert!(samples.iter().any(|v| v.abs() > 0.01));
        let raw = |app: &AppController, range| {
            CompiledSongPatch::compile_initial(&app.authored_source().unwrap())
                .unwrap()
                .render_default(range)
                .unwrap()
                .samples()
                .iter()
                .flat_map(|v| v.to_le_bytes())
                .collect::<Vec<_>>()
        };
        let range = meldritch_core::FrameRange::new(0, 192_000).unwrap();
        let publication = |host: &NativeProjectAudio| {
            let snapshot = host.reader.snapshot();
            (0..snapshot.frames())
                .flat_map(|frame| {
                    snapshot
                        .frame(frame)
                        .unwrap()
                        .iter()
                        .flat_map(|v| v.to_le_bytes())
                })
                .collect::<Vec<_>>()
        };
        let published_before = publication(&host);
        let before = raw(&app, range);
        gesture(&mut app, &mut host, control(KeyCode::Char('n'))).unwrap();
        gesture(&mut app, &mut host, key(KeyCode::Char(']'))).unwrap();
        ready(&mut app, &mut host);
        assert_ne!(publication(&host), published_before);
        assert_ne!(raw(&app, range), before);
        gesture(&mut app, &mut host, key(KeyCode::Esc)).unwrap();
        let before = raw(&app, range);
        let published_before = publication(&host);
        for event in [
            control(KeyCode::Char('e')),
            key(KeyCode::Right),
            key(KeyCode::Esc),
        ] {
            gesture(&mut app, &mut host, event).unwrap();
        }
        assert_ne!(raw(&app, range), before);
        ready(&mut app, &mut host);
        assert_ne!(publication(&host), published_before);
        let rendered = CompiledSongPatch::compile_initial(&app.authored_source().unwrap())
            .unwrap()
            .render_default(meldritch_core::FrameRange::new(0, u64::from(host.frames)).unwrap())
            .unwrap();
        let stereo = rendered
            .samples()
            .iter()
            .flat_map(|v| std::iter::repeat_n(*v, 2).flat_map(|v| v.to_le_bytes()))
            .collect::<Vec<_>>();
        assert!(
            publication(&host) == stereo,
            "published stereo must duplicate authored mono PCM"
        );
        gesture(&mut app, &mut host, key(KeyCode::F(6))).unwrap();
        let saved = std::fs::read(project.root().join("main.project.lua")).unwrap();
        gesture(&mut app, &mut host, control(KeyCode::Tab)).unwrap();
        gesture(&mut app, &mut host, key(KeyCode::Home)).unwrap();
        ready(&mut app, &mut host);
        callback.render_f64(&mut samples);
        assert!(samples.iter().all(|v| *v == 0.0));
        assert!(app.view_model().transport.position > 0);
        gesture(&mut app, &mut host, key(KeyCode::F(6))).unwrap();
        assert_eq!(
            std::fs::read(project.root().join("main.project.lua")).unwrap(),
            saved
        );
        let accepted = app.authored_source().unwrap();
        let published = host.reader.snapshot();
        let generation = host.host.generation();
        let history = app.history().len();
        assert!(
            app.dispatch(AppCommand::EditNote {
                pattern: "pattern-1".into(),
                edit: NoteEdit::Duration {
                    id: "note-2".into(),
                    duration_ticks: 0
                }
            })
            .is_err()
        );
        assert!(Arc::ptr_eq(&accepted, &app.authored_source().unwrap()));
        assert_eq!(app.history().len(), history);
        assert!(Arc::ptr_eq(&published, &host.reader.snapshot()));
        assert_eq!(host.host.generation(), generation);
        std::fs::write(project.root().join(".main.project.lua.tmp"), "foreign").unwrap();
        assert!(gesture(&mut app, &mut host, key(KeyCode::F(6))).is_err());
        assert_eq!(
            std::fs::read(project.root().join("main.project.lua")).unwrap(),
            saved
        );
        assert!(Arc::ptr_eq(&accepted, &app.authored_source().unwrap()));
        assert!(Arc::ptr_eq(&published, &host.reader.snapshot()));
        assert_eq!(host.host.generation(), generation);
        std::fs::remove_file(project.root().join(".main.project.lua.tmp")).unwrap();
        gesture(&mut app, &mut host, key(KeyCode::F(6))).unwrap();
        std::fs::write(project.root().join("expected.pcm"), raw(&app, range)).unwrap();
        std::fs::write(
            project.root().join("expected.fingerprint"),
            format!("{:016x}", accepted.fingerprint()),
        )
        .unwrap();
        host.finish(&app).unwrap();
        drop(callback);
        drop(host);
        drop(app);
        let child = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "project_playback::tests::native_keyboard_reopen_child",
                "--ignored",
                "--nocapture",
            ])
            .env("MELDRITCH_KEYBOARD_TEST_ROOT", project.root())
            .output()
            .unwrap();
        assert!(
            child.status.success(),
            "{}{}",
            String::from_utf8_lossy(&child.stdout),
            String::from_utf8_lossy(&child.stderr)
        );
    }

    #[test]
    #[ignore = "invoked by isolated keyboard workflow parent"]
    fn native_keyboard_reopen_child() {
        let root = PathBuf::from(std::env::var_os("MELDRITCH_KEYBOARD_TEST_ROOT").unwrap());
        let before = std::fs::read_dir(&root).unwrap().count();
        let project = meldritch_dsl::load_project_directory(&root).unwrap();
        let store = meldritch_control::native_project::NativeProjectStore::open(&project).unwrap();
        let source = store.source().unwrap();
        let (mut app, engine) = AppController::new_empty_project(&project).unwrap();
        app.enable_native_project_source(source.clone()).unwrap();
        let mut host = NativeProjectAudio::with_store(&app, engine, store).unwrap();
        host.poll(&mut app).unwrap();
        assert!(app.history().is_empty() && app.output_levels().is_empty());
        assert_eq!(app.view_model().transport.state, TransportState::Stopped);
        assert_eq!(
            app.authored_source().unwrap().pattern_voices()["voice-1-instance"].level,
            0.8
        );
        assert!(
            app.view_model().palette_pattern_editor.is_some()
                && app.view_model().instrument_editor.is_some()
        );
        assert!(host.capture.session.is_none() && host.stream.is_none());
        let source = app.authored_source().unwrap();
        assert_eq!(
            format!("{:016x}", source.fingerprint()),
            std::fs::read_to_string(root.join("expected.fingerprint")).unwrap()
        );
        let patch = CompiledSongPatch::compile_initial(&source).unwrap();
        let raw = |range| {
            patch
                .render_default(range)
                .unwrap()
                .samples()
                .iter()
                .flat_map(|v| v.to_le_bytes())
                .collect::<Vec<_>>()
        };
        let expected = std::fs::read(root.join("expected.pcm")).unwrap();
        assert_eq!(
            raw(meldritch_core::FrameRange::new(0, 192_000).unwrap()),
            expected
        );
        let mut split = raw(meldritch_core::FrameRange::new(0, 17_123).unwrap());
        split.extend(raw(
            meldritch_core::FrameRange::new(17_123, 192_000).unwrap()
        ));
        assert_eq!(split, expected);
        host.finish(&app).unwrap();
        assert_eq!(std::fs::read_dir(&root).unwrap().count(), before);
    }

    #[test]
    fn native_saved_reopen_prepares_audio_and_resets_runtime_without_capture_or_device() {
        let project = project();
        let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let relative = "songs/examples/34-empty-project-patch/authored-project.lua";
        let example = if manifest.join(relative).is_file() {
            manifest.join(relative)
        } else {
            manifest.join("../..").join(relative)
        };
        let source = meldritch_control::native_project::load_native_project(
            project.root(),
            &std::fs::read_to_string(example).unwrap(),
        )
        .unwrap();
        let mut store =
            meldritch_control::native_project::NativeProjectStore::open(&project).unwrap();
        store.save(&source).unwrap();
        let (mut app, engine) = AppController::new_empty_project(&project).unwrap();
        app.enable_native_project_source(source).unwrap();
        assert!(app.history().is_empty());
        assert!(app.output_levels().is_empty());
        assert!(app.view_model().instrument_editor.is_some());
        assert!(app.view_model().palette_pattern_editor.is_some());
        let mut host = NativeProjectAudio::new(&app, engine).unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        while host.ready_generation != Some(host.host.generation()) {
            host.poll(&mut app).unwrap();
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(2));
        }
        assert!(host.persistence_status(&app).starts_with("Saved"));
        assert_eq!(std::fs::read_dir(project.root()).unwrap().count(), 1);
        assert!(host.capture.session.is_none());
        assert!(host.stream.is_none());
        assert_eq!(app.view_model().transport.state, TransportState::Stopped);
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
        assert!(samples.iter().any(|v| v.abs() > 0.01));
        app.dispatch(AppCommand::SetOutputLevel {
            voice: "voice-1-instance".into(),
            control: "level".into(),
            value: Some(0.0),
        })
        .unwrap();
        host.save_project(&mut app).unwrap();
        assert!(host.persistence_status(&app).starts_with("Saved"));
        assert_eq!(app.output_levels().len(), 1);
        app.dispatch(AppCommand::EditNote {
            pattern: "melody".into(),
            edit: NoteEdit::Duration {
                id: "note-2".into(),
                duration_ticks: 960,
            },
        })
        .unwrap();
        assert!(host.persistence_status(&app).starts_with("Unsaved"));
        let accepted = app.authored_source().unwrap();
        let published = host.reader.snapshot();
        let generation = host.host.generation();
        std::fs::write(project.root().join(".main.project.lua.tmp"), "obstructed").unwrap();
        assert!(host.save_project(&mut app).is_err());
        assert!(Arc::ptr_eq(&accepted, &app.authored_source().unwrap()));
        assert!(Arc::ptr_eq(&published, &host.reader.snapshot()));
        assert_eq!(host.host.generation(), generation);
        assert_eq!(app.output_levels().len(), 1);
        std::fs::remove_file(project.root().join(".main.project.lua.tmp")).unwrap();
        host.save_project(&mut app).unwrap();
        assert!(host.persistence_status(&app).starts_with("Saved"));
        let reloaded =
            meldritch_control::native_project::NativeProjectStore::open(&project).unwrap();
        let (mut fresh, _) = AppController::new_empty_project(&project).unwrap();
        fresh
            .enable_native_project_source(reloaded.source().unwrap().clone())
            .unwrap();
        assert!(fresh.output_levels().is_empty());
        assert_eq!(
            fresh.authored_source().unwrap().pattern_voices()["voice-1-instance"].level,
            0.8
        );
        host.finish(&app).unwrap();
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
