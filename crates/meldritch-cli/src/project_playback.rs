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
    keyboard: Option<crate::keyboard::NativeKeyboard>,
    sustain_cc: Option<u8>,
    live: Option<crate::live_keyboard::LiveKeyboardWorker>,
    live_producer: Option<meldritch_audio::streaming_audio::StreamingProducer>,
    live_consumer: Option<meldritch_audio::streaming_audio::StreamingConsumer>,
    live_control: meldritch_audio::streaming_audio::StreamingControl,
    #[cfg(test)]
    headless_output: bool,
}

impl NativeProjectAudio {
    fn ensure_stream(&mut self, app: &AppController) -> Result<(), String> {
        #[cfg(test)]
        if self.headless_output {
            return Ok(());
        }
        if self.stream.is_none() {
            let source = app.authored_source().ok_or("no project source")?;
            let engine = self.engine.take().ok_or(
                "device initialization failed; audio unavailable; authored edits retained",
            )?;
            self.stream = Some(RealtimePlaybackSession::open_default_streaming(
                self.reader.clone(),
                source.performance().sample_rate(),
                u32::MAX,
                app.playback_control(),
                engine,
                self.live_consumer.take(),
            )?);
        }
        Ok(())
    }
    fn bind_keyboard(
        &mut self,
        app: &mut AppController,
        voice: Option<String>,
    ) -> Result<(), String> {
        if self.live.is_none() {
            self.live = Some(crate::live_keyboard::LiveKeyboardWorker::new(
                self.live_producer
                    .take()
                    .ok_or("keyboard worker unavailable")?,
                self.live_control.clone(),
            )?);
        }
        self.live.as_ref().unwrap().bind(app, voice)?;
        self.capture.checkpoint(app)?;
        if let Some(keyboard) = &mut self.keyboard {
            keyboard.set_binding_epoch(app.keyboard().epoch);
        }
        Ok(())
    }
    pub(crate) fn enable_keyboard(
        &mut self,
        port: String,
        sustain_cc: Option<u8>,
    ) -> Result<(), String> {
        if sustain_cc.is_some_and(|cc| cc > 127) {
            return Err("sustain CC must be in 0..127".into());
        }
        self.keyboard = Some(crate::keyboard::NativeKeyboard::new(port)?);
        self.sustain_cc = sustain_cc;
        Ok(())
    }
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
        let (live_producer, live_consumer, live_control) =
            meldritch_audio::streaming_audio::streaming_audio(4096, 0)?;
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
            keyboard: None,
            sustain_cc: None,
            live: None,
            live_producer: Some(live_producer),
            live_consumer: Some(live_consumer),
            live_control,
            #[cfg(test)]
            headless_output: false,
        })
    }
}

impl ProjectPlaybackHost for NativeProjectAudio {
    fn toggle_keyboard(&mut self, app: &mut AppController) -> Result<(), String> {
        if app.keyboard().voice.is_some() {
            return self.bind_keyboard(app, None);
        }
        if self.keyboard.is_none() {
            return Err("open-project --midi-keyboard PORT is required".into());
        }
        let id = app
            .view_model()
            .selected_pattern_voice
            .ok_or("select an active mono instance first")?;
        let source = app.authored_source().ok_or("no project source")?;
        if !source.is_pattern_voice_active(&id) {
            return Err("selected instance is inactive".into());
        }
        if !app.keyboard_transport_stopped() {
            return Err("stop transport before keyboard audition".into());
        }
        self.ensure_stream(app)?;
        self.bind_keyboard(app, Some(id))
    }
    fn persistence_status(&self, app: &AppController) -> String {
        let saved: String = if app
            .authored_source()
            .is_some_and(|source| Some(source.fingerprint()) == self.store.saved_fingerprint())
        {
            "Saved authored project · main.project.lua".into()
        } else {
            "Unsaved authored project · Ctrl-S/F6 to save".into()
        };
        let audition = app
            .keyboard()
            .voice
            .as_deref()
            .map_or(String::new(), |voice| {
                format!(
                    " · Live keys → {voice}: held {} · pedal {} · velocity {} · bend {} · stream gaps {} · F9 release",
                    app.keyboard().held_keys,
                    if app.keyboard().sustain {"down"} else {"up"},
                    app.keyboard()
                        .velocity
                        .map_or_else(|| "—".into(), |v| format!("{v}/127")),
                    app.keyboard().bend.map_or_else(|| "center".into(), |v| format!("{v}/16383")),
                    self.live_control.underruns()
                )
            });
        let saved = format!("{saved}{audition} · F9 MIDI audition (stopped)");
        self.keyboard
            .as_ref()
            .map_or(saved.clone(), |k| format!("{saved} · {}", k.status))
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
        self.capture.finish(app)?;
        self.live_control.deactivate();
        Ok(())
    }
    fn poll(&mut self, app: &mut AppController) -> Result<(), String> {
        if let Err(error) = self.capture.checkpoint(app) {
            // Do not leave a live gate sounding indefinitely while persistence
            // prevents normal input/loss processing. Keep typed cleanup in the
            // bounded app history for the same capture's later retry.
            self.live_control.deactivate();
            if app.keyboard().voice.is_some() {
                if let Some(live) = &self.live {
                    live.bind(app, None)?;
                }
                if let Some(keyboard) = &mut self.keyboard {
                    keyboard.set_binding_epoch(app.keyboard().epoch);
                }
            }
            return Err(format!(
                "capture checkpoint: {error}; keyboard audition released"
            ));
        }
        if let Some(live) = &self.live {
            if let Err(error) = live.synchronize(app) {
                self.bind_keyboard(app, None)?;
                return Err(format!("keyboard worker: {error}"));
            }
        }
        if let Some(keyboard) = &mut self.keyboard {
            if keyboard.binding_epoch() != app.keyboard().epoch {
                keyboard.set_binding_epoch(app.keyboard().epoch);
            }
            if let Some(reason) = keyboard.poll() {
                if app.keyboard().voice.is_some() {
                    self.live
                        .as_ref()
                        .ok_or("no live worker")?
                        .reset(app, reason)?;
                    self.capture.checkpoint(app)?;
                }
                keyboard.set_binding_epoch(app.keyboard().epoch);
            }
            for _ in 0..meldritch_control::QUEUE_CAPACITY {
                let Some(event) = keyboard.next() else { break };
                if !event.is_current() || event.binding_epoch != app.keyboard().epoch {
                    continue;
                }
                if app.keyboard().voice.is_some() {
                    if let Err(error) = self.live.as_ref().ok_or("no live worker")?.key(
                        app,
                        &event.bytes,
                        event.binding_epoch,
                        self.sustain_cc,
                    ) {
                        self.live
                            .as_ref()
                            .unwrap()
                            .reset(app, meldritch_app::KeyboardResetReason::Overflow)?;
                        self.capture.checkpoint(app)?;
                        keyboard.set_binding_epoch(app.keyboard().epoch);
                        return Err(format!("MIDI live input reset: {error}"));
                    }
                    self.capture.checkpoint(app)?;
                } else if event.is_current()
                    && let Some(command) = meldritch_control::devices::keyboard_step_command(
                        &event.bytes,
                        &app.view_model(),
                    )
                {
                    app.dispatch(command)
                        .map_err(|e| format!("MIDI note entry rejected: {e:?}"))?;
                    self.capture.checkpoint(app)?;
                }
            }
        }
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
        if app.keyboard().voice.is_some() {
            self.bind_keyboard(app, None)?;
        }
        self.ensure_stream(app)?;
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

    #[test]
    fn native_keyboard_audition_streams_shared_dsp_and_cleans_lost_input() {
        use meldritch_app::{AppCommandResult, CockpitMode};
        use meldritch_core::FrameRange;
        use meldritch_render::song_render::{LiveVoiceEvent, compile_pattern_voice};
        let project = project();
        let source = meldritch_control::native_project::load_native_project(
            project.root(),
            include_str!("../../../songs/examples/36-patch-lfo/sustain.project.lua"),
        )
        .unwrap();
        use meldritch_dsl::{ModuleKind, PatchEdit, PatchOperation, PitchModulationSettings};
        let source = source
            .edit_patch(&PatchEdit {
                synth: "bass".into(),
                operations: vec![
                    PatchOperation::Instantiate {
                        id: "touch".into(),
                        kind: ModuleKind::ControlMultiply,
                    },
                    PatchOperation::Disconnect {
                        from: "shape.control".into(),
                        to: "amp.level".into(),
                    },
                    PatchOperation::Connect {
                        from: "shape.control".into(),
                        to: "touch.left".into(),
                    },
                    PatchOperation::Connect {
                        from: "input.velocity".into(),
                        to: "touch.right".into(),
                    },
                    PatchOperation::Connect {
                        from: "touch.control".into(),
                        to: "amp.level".into(),
                    },
                    PatchOperation::Instantiate {
                        id: "wheel".into(),
                        kind: ModuleKind::PitchModulate,
                    },
                    PatchOperation::ConfigurePitchModulation {
                        module: "wheel".into(),
                        settings: PitchModulationSettings {
                            depth_semitones: 2.0,
                        },
                    },
                    PatchOperation::Disconnect {
                        from: "input.pitch".into(),
                        to: "osc.pitch".into(),
                    },
                    PatchOperation::Connect {
                        from: "input.pitch".into(),
                        to: "wheel.pitch".into(),
                    },
                    PatchOperation::Connect {
                        from: "input.bend".into(),
                        to: "wheel.control".into(),
                    },
                    PatchOperation::Connect {
                        from: "wheel.pitch".into(),
                        to: "osc.pitch".into(),
                    },
                    PatchOperation::Instantiate {
                        id: "hold".into(),
                        kind: ModuleKind::GateHold,
                    },
                    PatchOperation::Disconnect {
                        from: "input.gate".into(),
                        to: "env.gate".into(),
                    },
                    PatchOperation::Connect {
                        from: "input.gate".into(),
                        to: "hold.gate".into(),
                    },
                    PatchOperation::Connect {
                        from: "input.sustain".into(),
                        to: "hold.hold".into(),
                    },
                    PatchOperation::Connect {
                        from: "hold.gate".into(),
                        to: "env.gate".into(),
                    },
                ],
            })
            .unwrap();
        let fingerprint = source.fingerprint();
        let patch =
            compile_pattern_voice(&source, &source.pattern_voices()["bass-instance"]).unwrap();
        let (mut app, engine) = AppController::new_empty_project(&project).unwrap();
        app.enable_native_project_source(source).unwrap();
        let mut host = NativeProjectAudio::new(&app, engine).unwrap();
        assert!(host.toggle_keyboard(&mut app).is_err()); // No keyboard.
        let (keyboard, route) = crate::keyboard::NativeKeyboard::simulated();
        host.keyboard = Some(keyboard);
        host.sustain_cc = Some(64);
        let mut callback = PublishedAudioRenderer::from_engine(
            host.reader.clone(),
            2,
            u32::MAX,
            host.engine.take().unwrap(),
        )
        .unwrap()
        .with_streaming(host.live_consumer.take().unwrap());
        host.headless_output = true;
        host.poll(&mut app).unwrap();
        let source_arc = app.authored_source().unwrap();
        route.observe(&[0x90, 70, 127]); // Old ingress cannot be retargeted by F9.
        host.toggle_keyboard(&mut app).unwrap();
        assert_eq!(app.keyboard().voice.as_deref(), Some("bass-instance"));
        route.observe(&[0x90, 48, 32]);
        host.poll(&mut app).unwrap();
        app.dispatch(AppCommand::SetCockpitMode(CockpitMode::Performance))
            .unwrap();
        host.poll(&mut app).unwrap();
        assert_eq!(app.keyboard().held_keys, 1);
        assert!(Arc::ptr_eq(&source_arc, &app.authored_source().unwrap()));
        let mut reference = patch.live_renderer(48, 0).unwrap();
        let pump =
            |host: &mut NativeProjectAudio,
             app: &mut AppController,
             callback: &mut PublishedAudioRenderer,
             reference: &mut meldritch_render::song_render::LiveMonoRenderer| {
                let start = host.live_control.clock();
                let end = start + 256;
                let deadline = Instant::now() + Duration::from_secs(5);
                while host.live.as_ref().unwrap().frontier() < end {
                    assert!(Instant::now() < deadline, "worker stalled");
                    std::thread::sleep(Duration::from_millis(1));
                }
                let events: Vec<_> = app
                    .history()
                    .iter()
                    .filter_map(|record| match (&record.command, &record.result) {
                        (
                            AppCommand::KeyboardKey { frame, .. }
                            | AppCommand::KeyboardBend { frame, .. }
                            | AppCommand::KeyboardSustain { frame, .. },
                            AppCommandResult::KeyboardUpdated { outputs, .. },
                        ) if *frame >= start && *frame < end => Some(
                            outputs
                                .iter()
                                .copied()
                                .map(|input| LiveVoiceEvent {
                                    frame: *frame,
                                    input,
                                })
                                .collect::<Vec<_>>(),
                        ),
                        _ => None,
                    })
                    .flatten()
                    .collect();
                let expected = reference
                    .render(FrameRange::new(start, end).unwrap(), &events)
                    .unwrap();
                let mut output = [0.0; 512];
                callback.render_f64(&mut output);
                for (actual, expected) in output.chunks_exact(2).zip(
                    expected
                        .samples()
                        .chunks_exact(usize::from(expected.channels())),
                ) {
                    assert_eq!(actual, &[expected[0] * 0.8, expected[0] * 0.8]);
                }
                host.poll(app).unwrap();
                output.iter().any(|value| value.abs() > 0.01)
            };
        let mut heard = false;
        for _ in 0..400 {
            heard |= pump(&mut host, &mut app, &mut callback, &mut reference);
        }
        assert!(heard);
        assert!(app.keyboard().gate); // Held beyond the authored bar and pattern releases.
        route.observe(&[0xb0, 64, 127]);
        host.poll(&mut app).unwrap();
        assert!(app.keyboard().sustain);
        route.observe(&[0xe0, 127, 127]);
        host.poll(&mut app).unwrap();
        assert_eq!(app.keyboard().bend, Some(16383));
        for _ in 0..6 {
            pump(&mut host, &mut app, &mut callback, &mut reference);
        }
        route.observe(&[0x91, 55, 96]);
        host.poll(&mut app).unwrap();
        for _ in 0..6 {
            pump(&mut host, &mut app, &mut callback, &mut reference);
        }
        route.observe(&[0x91, 55, 0]);
        host.poll(&mut app).unwrap();
        assert_eq!(app.keyboard().pitch, Some(48));
        assert_eq!(app.keyboard().velocity, Some(32));
        assert_eq!(app.keyboard().bend, Some(16383)); // No implicit key centering.
        for _ in 0..6 {
            pump(&mut host, &mut app, &mut callback, &mut reference);
        }
        route.observe(&[0x80, 48, 0]);
        host.poll(&mut app).unwrap();
        for _ in 0..6 {
            assert!(pump(&mut host, &mut app, &mut callback, &mut reference));
        }
        assert!(!app.keyboard().gate);
        assert!(app.keyboard().sustain);
        route.observe(&[0xb0, 64, 0]);
        host.poll(&mut app).unwrap();
        assert!(!app.keyboard().sustain);
        for _ in 0..6 {
            pump(&mut host, &mut app, &mut callback, &mut reference);
        }
        assert!(!pump(&mut host, &mut app, &mut callback, &mut reference));
        assert_eq!(app.authored_source().unwrap().fingerprint(), fingerprint);
        // Overflow: pre-loss queued messages must not revive any key.
        route.observe(&[0x90, 60, 127]);
        host.poll(&mut app).unwrap();
        let epoch = app.keyboard().epoch;
        for _ in 0..65 {
            route.observe(&[0x90, 60, 127]);
        }
        host.poll(&mut app).unwrap();
        assert!(app.keyboard().epoch > epoch);
        assert_eq!(app.keyboard().bend, Some(8192));
        assert_eq!(app.keyboard().held_keys, 0);
        assert!(!app.keyboard().gate);
        assert!(!app.keyboard().sustain);
        route.observe(&[0x90, 60, 127]);
        host.poll(&mut app).unwrap();
        assert_eq!(app.keyboard().held_keys, 1);
        route.observe(&[0xe0, 0, 0]);
        host.poll(&mut app).unwrap();
        route.observe(&[0xb0, 64, 127]);
        host.poll(&mut app).unwrap();
        host.keyboard.as_mut().unwrap().simulate_disconnect();
        host.poll(&mut app).unwrap();
        assert_eq!(app.keyboard().held_keys, 0);
        assert_eq!(app.keyboard().bend, Some(8192));
        assert!(!app.keyboard().sustain);
        route.observe(&[0x90, 60, 127]); // Disconnected route is stale.
        host.poll(&mut app).unwrap();
        assert_eq!(app.keyboard().held_keys, 0);
        // An authored edit requires fresh binding and immediately disables old PCM on poll.
        app.dispatch(AppCommand::CreatePalettePattern {
            id: "new".into(),
            length_ticks: 3840,
            looped: true,
        })
        .unwrap();
        host.poll(&mut app).unwrap();
        assert!(app.keyboard().voice.is_none());
        assert!(!host.live_control.enabled());
        let mut silence = [9.0; 512];
        callback.render_f64(&mut silence);
        assert!(silence.iter().all(|s| *s == 0.0));
        // F9 rejects pending/playing transport; Space/typed Play release audition.
        host.toggle_keyboard(&mut app).unwrap();
        app.dispatch(AppCommand::Play).unwrap();
        assert!(app.keyboard().voice.is_none());
        assert!(host.toggle_keyboard(&mut app).is_err());
        callback.render_f64(&mut silence);
        assert!(!host.live_control.enabled());
        host.poll(&mut app).unwrap();
        assert!(host.toggle_keyboard(&mut app).is_err());
        app.dispatch(AppCommand::Stop).unwrap();
        callback.render_f64(&mut silence);
        host.poll(&mut app).unwrap();
        host.toggle_keyboard(&mut app).unwrap();
        host.toggle_keyboard(&mut app).unwrap();
        assert!(app.keyboard().voice.is_none());
        host.finish(&app).unwrap();
        let text = std::fs::read_to_string(host.capture.session.as_ref().unwrap().path()).unwrap();
        assert!(text.contains("runtime_keyboard"));
        assert!(text.contains("Disconnect"));
        assert!(text.contains("Overflow"));
        // Reconstruct accepted source/runtime facts through the typed vocabulary.
        // Drain transport commands headlessly; no timed hardware replay is implied.
        let (mut replay, replay_engine) = AppController::new_empty_project(&project).unwrap();
        replay
            .enable_native_project_source(
                meldritch_control::native_capture::load_native_capture_baseline(
                    project.root(),
                    &text,
                )
                .unwrap(),
            )
            .unwrap();
        let (_, replay_reader) = meldritch_audio::audio_publication::audio_publication(
            PublishedAudio::from_block(&AudioBlock::silent(2, 1), 1).unwrap(),
        );
        let mut replay_callback =
            PublishedAudioRenderer::from_engine(replay_reader, 2, u32::MAX, replay_engine).unwrap();
        let document: toml::Value = toml::from_str(&text).unwrap();
        for event in document["events"].as_array().unwrap() {
            let envelope = meldritch_control::native_capture::NativeCommandEnvelope::decode(
                &toml::to_string(&event["native"]).unwrap(),
            )
            .unwrap();
            assert_eq!(
                envelope.source_before,
                format!("{:016x}", replay.authored_source().unwrap().fingerprint())
            );
            replay.dispatch(envelope.command.into()).unwrap();
            replay_callback.render_f64(&mut [0.0; 2]);
            assert_eq!(
                envelope.source_after,
                format!("{:016x}", replay.authored_source().unwrap().fingerprint())
            );
        }
        assert_eq!(replay.keyboard(), app.keyboard());
        std::fs::remove_dir_all(project.root()).unwrap();
    }

    #[test]
    fn native_keyboard_audition_without_selected_material_is_atomic_and_device_free() {
        let project = project();
        let (mut app, engine) = AppController::new_empty_project(&project).unwrap();
        let mut host = NativeProjectAudio::new(&app, engine).unwrap();
        host.keyboard = Some(crate::keyboard::NativeKeyboard::simulated().0);
        let source = app.authored_source().unwrap();
        let history = app.history().len();
        assert!(
            host.toggle_keyboard(&mut app)
                .unwrap_err()
                .contains("select")
        );
        assert!(host.engine.is_some());
        assert!(host.stream.is_none());
        assert!(host.live.is_none());
        assert!(Arc::ptr_eq(&source, &app.authored_source().unwrap()));
        assert_eq!(app.history().len(), history);
        assert!(meldritch_tui::project_keyboard_key(
            crossterm::event::KeyEvent::new(
                crossterm::event::KeyCode::F(9),
                crossterm::event::KeyModifiers::NONE
            )
        ));
        let mut key = crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::F(9),
            crossterm::event::KeyModifiers::CONTROL,
        );
        assert!(!meldritch_tui::project_keyboard_key(key));
        key.modifiers = crossterm::event::KeyModifiers::NONE;
        key.kind = crossterm::event::KeyEventKind::Repeat;
        assert!(!meldritch_tui::project_keyboard_key(key));
        key.kind = crossterm::event::KeyEventKind::Release;
        assert!(!meldritch_tui::project_keyboard_key(key));
        app.enable_native_project_source(
            meldritch_control::native_project::load_native_project(
                project.root(),
                include_str!("../../../songs/examples/36-patch-lfo/main.project.lua"),
            )
            .unwrap(),
        )
        .unwrap();
        host.engine = None; // A failed output device cannot accept a binding.
        let history = app.history().len();
        assert!(host.toggle_keyboard(&mut app).is_err());
        assert_eq!(app.history().len(), history);
        assert!(app.keyboard().voice.is_none());
        std::fs::remove_dir_all(project.root()).unwrap();
    }

    #[test]
    fn native_keyboard_capture_failure_releases_audition_and_retries_typed_cleanup() {
        let project = project();
        let (mut app, engine) = AppController::new_empty_project(&project).unwrap();
        app.enable_native_project_source(
            meldritch_control::native_project::load_native_project(
                project.root(),
                include_str!("../../../songs/examples/36-patch-lfo/main.project.lua"),
            )
            .unwrap(),
        )
        .unwrap();
        let mut host = NativeProjectAudio::new(&app, engine).unwrap();
        let (keyboard, route) = crate::keyboard::NativeKeyboard::simulated();
        host.keyboard = Some(keyboard);
        host.headless_output = true;
        host.toggle_keyboard(&mut app).unwrap();
        let path = host.capture.session.as_ref().unwrap().path().to_path_buf();
        let backup = path.with_extension("saved");
        std::fs::rename(&path, &backup).unwrap();
        std::fs::create_dir(&path).unwrap();
        route.observe(&[0x90, 48, 127]);
        assert!(host.poll(&mut app).is_err());
        assert!(app.keyboard().gate); // Accepted gesture awaiting failed checkpoint.
        assert!(host.poll(&mut app).is_err());
        assert!(app.keyboard().voice.is_none());
        assert!(!host.live_control.enabled());
        std::fs::remove_dir(&path).unwrap();
        std::fs::rename(&backup, &path).unwrap();
        host.poll(&mut app).unwrap();
        host.finish(&app).unwrap();
        let document: toml::Value =
            toml::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        let commands: Vec<_> = document["events"]
            .as_array()
            .unwrap()
            .iter()
            .map(|event| {
                meldritch_control::native_capture::NativeCommandEnvelope::decode(
                    &toml::to_string(&event["native"]).unwrap(),
                )
                .unwrap()
                .command
            })
            .collect();
        assert_eq!(
            commands
                .iter()
                .filter(|c| matches!(
                    c,
                    meldritch_control::native_capture::NativeProjectCommand::KeyboardKey { .. }
                ))
                .count(),
            1
        );
        assert_eq!(
            commands
                .iter()
                .filter(|c| matches!(
                    c,
                    meldritch_control::native_capture::NativeProjectCommand::BindKeyboard { .. }
                ))
                .count(),
            2
        );
        std::fs::remove_dir_all(project.root()).unwrap();
    }

    #[test]
    fn native_midi_keyboard_entry_publishes_captures_and_reopens_without_live_voice_claims() {
        use meldritch_app::CockpitMode;
        use meldritch_control::native_capture::{
            NativeCommandEnvelope, load_native_capture_baseline,
        };
        let project = project();
        let source = meldritch_control::native_project::load_native_project(
            project.root(),
            include_str!("../../../songs/examples/36-patch-lfo/main.project.lua"),
        )
        .unwrap();
        let (mut app, engine) = AppController::new_empty_project(&project).unwrap();
        app.enable_native_project_source(source).unwrap();
        let mut host = NativeProjectAudio::new(&app, engine).unwrap();
        let (keyboard, route) = crate::keyboard::NativeKeyboard::simulated();
        host.keyboard = Some(keyboard);
        let ready = |app: &mut AppController, host: &mut NativeProjectAudio| {
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
        ready(&mut app, &mut host);
        let initial_audio = host.reader.snapshot();
        app.dispatch(AppCommand::SelectPalettePattern("melody".into()))
            .unwrap();
        app.dispatch(AppCommand::SetCockpitMode(CockpitMode::AllParameters))
            .unwrap();
        // Closed editor must not accidentally record incoming keys.
        let before = app.authored_source().unwrap();
        route.observe(&[0x90, 60, 99]);
        host.poll(&mut app).unwrap();
        assert!(Arc::ptr_eq(&before, &app.authored_source().unwrap()));
        app.dispatch(AppCommand::TogglePatternEditor).unwrap();
        app.dispatch(AppCommand::EditNote {
            pattern: "melody".into(),
            edit: NoteEdit::Delete {
                id: "note-1".into(),
            },
        })
        .unwrap();
        ready(&mut app, &mut host);
        app.dispatch(AppCommand::Play).unwrap();
        let generation = host.host.generation();
        assert!(route.observe(&[0x93, 60, 99])); // All MIDI channels accepted for entry.
        host.poll(&mut app).unwrap();
        ready(&mut app, &mut host);
        assert!(host.host.generation() > generation);
        let source = app.authored_source().unwrap();
        let melody = source.pattern_material("melody").unwrap();
        assert_eq!(melody.notes.len(), 1);
        assert_eq!(melody.notes[0].note, "C4");
        assert_eq!(melody.notes[0].velocity, 99.0 / 127.0);
        assert_eq!(melody.notes[0].duration_ticks, 480);
        assert_eq!(
            source.pattern_voices()["bass-instance"]
                .gate_pattern
                .as_deref(),
            Some("rhythm")
        );
        assert!(
            (0..host.frames).any(|f| initial_audio.frame(f) != host.reader.snapshot().frame(f))
        );
        // Repeating a key selects, preserving authored velocity and identity.
        route.observe(&[0x90, 60, 1]);
        host.poll(&mut app).unwrap();
        assert!(Arc::ptr_eq(&source, &app.authored_source().unwrap()));
        assert_eq!(
            app.view_model()
                .palette_pattern_editor
                .unwrap()
                .selected_note
                .as_deref(),
            Some(melody.notes[0].id.as_str())
        );
        let history = app.history().len();
        let audio = host.reader.snapshot();
        route.observe(&[0x90, 67, 127]); // Overlapping mono pitch-source notes reject.
        assert!(host.poll(&mut app).is_err());
        assert!(Arc::ptr_eq(&source, &app.authored_source().unwrap()));
        assert!(Arc::ptr_eq(&audio, &host.reader.snapshot()));
        assert_eq!(history, app.history().len());
        app.dispatch(AppCommand::ToggleInstrumentEditor).unwrap();
        route.observe(&[0x90, 69, 127]); // An obscured pattern must not receive notes.
        host.poll(&mut app).unwrap();
        assert!(Arc::ptr_eq(&source, &app.authored_source().unwrap()));
        app.dispatch(AppCommand::ToggleInstrumentEditor).unwrap();
        let negatives: toml::Value = toml::from_str(include_str!(
            "../../../songs/examples/36-patch-lfo/invalid/keyboard.toml"
        ))
        .unwrap();
        app.dispatch(AppCommand::TogglePatternEditor).unwrap();
        for case in negatives["cases"].as_array().unwrap() {
            let bytes: Vec<_> = case["bytes"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_integer().unwrap() as u8)
                .collect();
            let history = app.history().len();
            let published = host.reader.snapshot();
            route.observe(&bytes);
            if case.get("reject").and_then(toml::Value::as_bool) == Some(true) {
                assert!(host.poll(&mut app).is_err());
            } else {
                host.poll(&mut app).unwrap();
            }
            assert!(
                Arc::ptr_eq(&source, &app.authored_source().unwrap()),
                "{}",
                case["kind"]
            );
            assert_eq!(app.history().len(), history);
            assert!(Arc::ptr_eq(&published, &host.reader.snapshot()));
        }
        app.dispatch(AppCommand::SetCockpitMode(CockpitMode::Performance))
            .unwrap();
        route.observe(&[0x90, 67, 127]);
        host.poll(&mut app).unwrap();
        assert!(Arc::ptr_eq(&source, &app.authored_source().unwrap()));
        app.dispatch(AppCommand::SetCockpitMode(CockpitMode::AllParameters))
            .unwrap();
        app.dispatch(AppCommand::MovePatternCursor {
            tick_delta: 3600,
            pitch_delta: 0,
        })
        .unwrap();
        route.observe(&[0x90, 64, 127]);
        ready(&mut app, &mut host);
        let source = app.authored_source().unwrap();
        let notes = source.pattern_material("melody").unwrap().notes;
        assert_eq!(notes.last().unwrap().duration_ticks, 240);
        let audio = host.reader.snapshot();
        let history = app.history().len();
        route.observe(&[0x90, 69, 127]);
        route.invalidate(); // A queued old-connection event must never author a note.
        host.poll(&mut app).unwrap();
        assert!(Arc::ptr_eq(&source, &app.authored_source().unwrap()));
        assert!(Arc::ptr_eq(&audio, &host.reader.snapshot()));
        assert_eq!(app.history().len(), history);
        let mut callback = PublishedAudioRenderer::from_engine(
            host.reader.clone(),
            2,
            u32::MAX,
            host.engine.take().unwrap(),
        )
        .unwrap();
        let mut samples = [0.0; 512];
        callback.render_f64(&mut samples);
        assert!(samples.iter().any(|s| s.abs() > 0.01));
        assert_eq!(app.view_model().transport.state, TransportState::Playing);
        host.save_project(&mut app).unwrap();
        host.finish(&app).unwrap();
        let reopened =
            meldritch_control::native_project::NativeProjectStore::open(&project).unwrap();
        assert_eq!(
            reopened.source().unwrap().fingerprint(),
            source.fingerprint()
        );
        let expected = CompiledSongPatch::compile_initial(&source)
            .unwrap()
            .render_default(meldritch_core::FrameRange::new(0, u64::from(host.frames)).unwrap())
            .unwrap();
        for (frame, sample) in expected.samples().iter().enumerate() {
            assert_eq!(audio.frame(frame as u32).unwrap(), &[*sample, *sample]);
        }
        let session =
            std::fs::read_to_string(host.capture.session.as_ref().unwrap().path()).unwrap();
        let baseline = load_native_capture_baseline(project.root(), &session).unwrap();
        let (mut replay, _) = AppController::new_empty_project(&project).unwrap();
        replay.enable_native_project_source(baseline).unwrap();
        let document: toml::Value = toml::from_str(&session).unwrap();
        for event in document["events"].as_array().unwrap() {
            let envelope =
                NativeCommandEnvelope::decode(&toml::to_string(&event["native"]).unwrap()).unwrap();
            replay.dispatch(envelope.command.into()).unwrap();
            assert_eq!(
                envelope.source_after,
                format!("{:016x}", replay.authored_source().unwrap().fingerprint())
            );
        }
        assert_eq!(
            replay.authored_source().unwrap().fingerprint(),
            source.fingerprint()
        );
        assert!(host.stream.is_none());
    }

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

    // Actual frontend lowering, with only the device output replaced in tests.
    fn native_gesture(
        app: &mut AppController,
        host: &mut NativeProjectAudio,
        key: crossterm::event::KeyEvent,
    ) -> Result<String, String> {
        use crossterm::event::{KeyCode, KeyModifiers};
        use meldritch_tui::{
            TuiAction, project_binding_command, project_composition_command,
            project_construction_command, project_editor_action, project_level_command,
            project_pattern_command, project_save_key,
        };
        host.prepare_action(app)?;
        if project_save_key(key) {
            return host.save_project(app);
        }
        let model = app.view_model();
        if key.code == KeyCode::Tab && key.modifiers == KeyModifiers::CONTROL {
            app.handle_input(AppInput::ToggleCockpitMode)
                .map_err(|e| format!("{e:?}"))?;
        } else {
            let source = app.authored_source().unwrap();
            let command = project_level_command(key, &model, &source)
                .or_else(|| project_composition_command(key, &model, &source))
                .or_else(|| project_binding_command(key, &model, &source))
                .or_else(|| project_pattern_command(key, &model, &source));
            if let Some(command) = command {
                app.dispatch(command).map_err(|e| format!("{e:?}"))?;
            } else if let Some(command) = project_construction_command(key, &source) {
                let result = app.dispatch(command).map_err(|e| format!("{e:?}"))?;
                let meldritch_app::AppCommandResult::PatchEdited { synth, .. } = result else {
                    panic!("creation result")
                };
                app.dispatch(AppCommand::SelectInstrument(synth))
                    .map_err(|e| format!("{e:?}"))?;
            } else if let Some(TuiAction::Input(input)) = project_editor_action(key, &model) {
                app.handle_input(input).map_err(|e| format!("{e:?}"))?;
            } else {
                panic!("unmapped native key {key:?}");
            }
        }
        host.poll(app)?;
        Ok(String::new())
    }

    #[test]
    fn native_two_composition_keyboard_workflow_publishes_and_reopens_each_selection() {
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        let project = project();
        let (mut app, engine) = AppController::new_empty_project(&project).unwrap();
        let mut host = NativeProjectAudio::new(&app, engine).unwrap();
        host.poll(&mut app).unwrap();
        assert_eq!(std::fs::read_dir(project.root()).unwrap().count(), 0);
        let script: toml::Value = toml::from_str(include_str!(
            "../../../songs/examples/35-project-palette-routing/keyboard-workflow.toml"
        ))
        .unwrap();
        for key in script["construction"].as_array().unwrap() {
            let code = match key.as_str().unwrap() {
                "Enter" => KeyCode::Enter,
                "Esc" => KeyCode::Esc,
                "Right" => KeyCode::Right,
                "Up" => KeyCode::Up,
                "PageUp" => KeyCode::PageUp,
                "PageDown" => KeyCode::PageDown,
                "F6" => KeyCode::F(6),
                "F7" => KeyCode::F(7),
                "F8" => KeyCode::F(8),
                key if key.len() == 1 => KeyCode::Char(key.chars().next().unwrap()),
                key => panic!("unexecuted workflow key {key}"),
            };
            native_gesture(&mut app, &mut host, KeyEvent::new(code, KeyModifiers::NONE)).unwrap();
        }
        let source = app.authored_source().unwrap();
        assert_eq!(
            (
                source.synths().len(),
                source.note_patterns().len(),
                source.pattern_voices().len(),
                source.compositions().len()
            ),
            (1, 3, 2, 2)
        );
        assert_eq!(
            source.compositions()["composition-1"].voices,
            ["voice-1-instance"]
        );
        assert_eq!(
            source.compositions()["composition-2"].voices,
            ["voice-1-instance-2"]
        );
        assert_eq!(
            source.pattern_voices()["voice-1-instance"]
                .pitch_pattern
                .as_deref(),
            Some("pattern-1")
        );
        assert_eq!(
            source.pattern_voices()["voice-1-instance-2"]
                .pitch_pattern
                .as_deref(),
            Some("pattern-1")
        );
        let mut callback = PublishedAudioRenderer::from_engine(
            host.reader.clone(),
            2,
            u32::MAX,
            host.engine.take().unwrap(),
        )
        .unwrap();
        let ready = |app: &mut AppController, host: &mut NativeProjectAudio| {
            let deadline = Instant::now() + Duration::from_secs(5);
            loop {
                host.poll(app).unwrap();
                if host
                    .require_prepared_audio(&app.authored_source().unwrap())
                    .is_ok()
                {
                    break;
                }
                assert!(
                    Instant::now() < deadline,
                    "composed workflow render timeout"
                );
                std::thread::sleep(Duration::from_millis(2));
            }
        };
        let mut previous = None;
        for id in ["composition-1", "composition-2"] {
            if app.authored_source().unwrap().selected_composition() != Some(id) {
                native_gesture(
                    &mut app,
                    &mut host,
                    KeyEvent::new(KeyCode::F(8), KeyModifiers::NONE),
                )
                .unwrap();
            }
            ready(&mut app, &mut host);
            let accepted = app.authored_source().unwrap();
            let patch = CompiledSongPatch::compile_initial(&accepted).unwrap();
            let rendered = patch
                .render_default(meldritch_core::FrameRange::new(0, u64::from(host.frames)).unwrap())
                .unwrap();
            for (frame, sample) in rendered.samples().iter().enumerate() {
                assert_eq!(
                    host.reader.snapshot().frame(frame as u32).unwrap(),
                    &[*sample, *sample]
                );
            }
            if let Some(previous) = previous {
                assert_ne!(rendered.samples(), previous);
            }
            app.dispatch(AppCommand::Play).unwrap();
            let mut samples = vec![0.0; 512];
            callback.render_f64(&mut samples);
            assert!(samples.iter().any(|sample| sample.abs() > 0.01));
            let published = host.reader.snapshot();
            let generation = host.host.generation();
            let history = app.history().len();
            for command in [
                AppCommand::BindPatternVoice {
                    voice: "voice-1-instance".into(),
                    signal: PatternSignal::Gate,
                    pattern: Some("missing".into()),
                },
                AppCommand::EditNote {
                    pattern: "pattern-1".into(),
                    edit: NoteEdit::Insert {
                        start_tick: 0,
                        duration_ticks: 480,
                        pitch: 60,
                        velocity: 0.8,
                    },
                },
            ] {
                assert!(app.dispatch(command).is_err());
                host.poll(&mut app).unwrap();
                assert!(Arc::ptr_eq(&accepted, &app.authored_source().unwrap()));
                assert!(Arc::ptr_eq(&published, &host.reader.snapshot()));
                assert_eq!(host.host.generation(), generation);
                assert_eq!(app.history().len(), history);
                assert_eq!(app.view_model().transport.state, TransportState::Playing);
            }
            if id == "composition-1" {
                for event in [
                    KeyEvent::new(KeyCode::Char('e'), KeyModifiers::CONTROL),
                    KeyEvent::new(KeyCode::Right, KeyModifiers::NONE),
                    KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
                ] {
                    native_gesture(&mut app, &mut host, event).unwrap();
                }
                ready(&mut app, &mut host);
                assert_ne!(
                    accepted.fingerprint(),
                    app.authored_source().unwrap().fingerprint()
                );
                assert_eq!(app.view_model().transport.state, TransportState::Playing);
                callback.render_f64(&mut samples);
                assert!(samples.iter().any(|sample| sample.abs() > 0.01));
            }
            app.dispatch(AppCommand::Stop).unwrap();
            callback.render_f64(&mut samples);
            let accepted = app.authored_source().unwrap();
            let patch = CompiledSongPatch::compile_initial(&accepted).unwrap();
            let rendered = patch
                .render_default(meldritch_core::FrameRange::new(0, u64::from(host.frames)).unwrap())
                .unwrap();
            for (frame, sample) in rendered.samples().iter().enumerate() {
                assert_eq!(
                    host.reader.snapshot().frame(frame as u32).unwrap(),
                    &[*sample, *sample]
                );
            }
            previous = Some(rendered.samples().to_vec());
            native_gesture(
                &mut app,
                &mut host,
                KeyEvent::new(KeyCode::F(6), KeyModifiers::NONE),
            )
            .unwrap();
            std::fs::write(
                project.root().join("expected.fingerprint"),
                format!("{:016x}", accepted.fingerprint()),
            )
            .unwrap();
            let pcm: Vec<_> = patch
                .render_default(meldritch_core::FrameRange::new(0, 192_000).unwrap())
                .unwrap()
                .samples()
                .iter()
                .flat_map(|sample| sample.to_le_bytes())
                .collect();
            std::fs::write(project.root().join("expected.pcm"), pcm).unwrap();
            let child = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "project_playback::tests::native_keyboard_reopen_child",
                    "--ignored",
                    "--nocapture",
                ])
                .env("MELDRITCH_KEYBOARD_TEST_ROOT", project.root())
                .env("MELDRITCH_TWO_COMPOSITION_ID", id)
                .output()
                .unwrap();
            assert!(
                child.status.success(),
                "{}{}",
                String::from_utf8_lossy(&child.stdout),
                String::from_utf8_lossy(&child.stderr)
            );
        }
        host.finish(&app).unwrap();
    }

    #[test]
    fn native_graph_lfo_rate_edit_republishes_and_invalid_units_keep_playing_audio() {
        use meldritch_dsl::{GraphLfoRate, GraphLfoSettings, PatchEdit, PatchOperation};
        let project = project();
        let source = meldritch_control::native_project::load_native_project(
            project.root(),
            include_str!("../../../songs/examples/36-patch-lfo/main.project.lua"),
        )
        .unwrap();
        let (mut app, engine) = AppController::new_empty_project(&project).unwrap();
        app.enable_native_project_source(source).unwrap();
        let mut host = NativeProjectAudio::new(&app, engine).unwrap();
        let ready = |app: &mut AppController, host: &mut NativeProjectAudio| {
            let deadline = Instant::now() + Duration::from_secs(5);
            loop {
                host.poll(app).unwrap();
                if host
                    .require_prepared_audio(&app.authored_source().unwrap())
                    .is_ok()
                {
                    break;
                }
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
        assert!(samples.iter().any(|sample| sample.abs() > 0.01));
        let source = app.authored_source().unwrap();
        let published = host.reader.snapshot();
        let generation = host.host.generation();
        let history = app.history().len();
        assert!(
            app.dispatch(AppCommand::EditPatch(PatchEdit {
                synth: "bass".into(),
                operations: vec![
                    PatchOperation::Disconnect {
                        from: "shape.control".into(),
                        to: "amp.level".into()
                    },
                    PatchOperation::Connect {
                        from: "slow.control".into(),
                        to: "amp.level".into()
                    },
                ]
            }))
            .is_err()
        );
        host.poll(&mut app).unwrap();
        assert!(Arc::ptr_eq(&source, &app.authored_source().unwrap()));
        assert!(Arc::ptr_eq(&published, &host.reader.snapshot()));
        assert_eq!(host.host.generation(), generation);
        assert_eq!(history, app.history().len());
        assert_eq!(app.view_model().transport.state, TransportState::Playing);
        app.dispatch(AppCommand::EditPatch(PatchEdit {
            synth: "bass".into(),
            operations: vec![PatchOperation::ConfigureLfo {
                module: "slow".into(),
                settings: GraphLfoSettings {
                    rate: GraphLfoRate::Beats(0.5),
                    ..GraphLfoSettings::default()
                },
            }],
        }))
        .unwrap();
        assert!(Arc::ptr_eq(&published, &host.reader.snapshot()));
        ready(&mut app, &mut host);
        assert!(!Arc::ptr_eq(&published, &host.reader.snapshot()));
        let expected = CompiledSongPatch::compile_initial(&app.authored_source().unwrap())
            .unwrap()
            .render_default(meldritch_core::FrameRange::new(0, u64::from(host.frames)).unwrap())
            .unwrap();
        assert!(
            (0..host.frames)
                .any(|frame| published.frame(frame) != host.reader.snapshot().frame(frame))
        );
        for (frame, sample) in expected.samples().iter().enumerate() {
            assert_eq!(
                host.reader.snapshot().frame(frame as u32).unwrap(),
                &[*sample, *sample]
            );
        }
        callback.render_f64(&mut samples);
        assert!(samples.iter().any(|sample| sample.abs() > 0.01));
        assert_eq!(app.view_model().transport.state, TransportState::Playing);
        let published = host.reader.snapshot();
        app.dispatch(AppCommand::EditPatch(PatchEdit {
            synth: "bass".into(),
            operations: vec![
                PatchOperation::Disconnect {
                    from: "input.pitch".into(),
                    to: "osc.pitch".into(),
                },
                PatchOperation::Instantiate {
                    id: "vibrato".into(),
                    kind: meldritch_dsl::ModuleKind::PitchModulate,
                },
                PatchOperation::Connect {
                    from: "input.pitch".into(),
                    to: "vibrato.pitch".into(),
                },
                PatchOperation::Connect {
                    from: "slow.control".into(),
                    to: "vibrato.control".into(),
                },
                PatchOperation::Connect {
                    from: "vibrato.pitch".into(),
                    to: "osc.pitch".into(),
                },
            ],
        }))
        .unwrap();
        assert!(Arc::ptr_eq(&published, &host.reader.snapshot()));
        ready(&mut app, &mut host);
        let expected = CompiledSongPatch::compile_initial(&app.authored_source().unwrap())
            .unwrap()
            .render_default(meldritch_core::FrameRange::new(0, u64::from(host.frames)).unwrap())
            .unwrap();
        assert!(
            (0..host.frames)
                .any(|frame| published.frame(frame) != host.reader.snapshot().frame(frame))
        );
        for (frame, sample) in expected.samples().iter().enumerate() {
            assert_eq!(
                host.reader.snapshot().frame(frame as u32).unwrap(),
                &[*sample, *sample]
            );
        }
        callback.render_f64(&mut samples);
        assert!(samples.iter().any(|sample| sample.abs() > 0.01));
        assert_eq!(app.view_model().transport.state, TransportState::Playing);
        let published = host.reader.snapshot();
        app.dispatch(AppCommand::EditPatch(PatchEdit {
            synth: "bass".into(),
            operations: vec![
                PatchOperation::Disconnect {
                    from: "amp.audio".into(),
                    to: "out.audio".into(),
                },
                PatchOperation::Instantiate {
                    id: "tone".into(),
                    kind: meldritch_dsl::ModuleKind::LowPass,
                },
                PatchOperation::Instantiate {
                    id: "cutoff".into(),
                    kind: meldritch_dsl::ModuleKind::ControlHertz,
                },
                PatchOperation::Connect {
                    from: "amp.audio".into(),
                    to: "tone.audio".into(),
                },
                PatchOperation::Connect {
                    from: "tone.audio".into(),
                    to: "out.audio".into(),
                },
                PatchOperation::Connect {
                    from: "unipolar.control".into(),
                    to: "cutoff.control".into(),
                },
                PatchOperation::Connect {
                    from: "cutoff.control".into(),
                    to: "tone.cutoff".into(),
                },
            ],
        }))
        .unwrap();
        assert!(Arc::ptr_eq(&published, &host.reader.snapshot()));
        ready(&mut app, &mut host);
        let expected = CompiledSongPatch::compile_initial(&app.authored_source().unwrap())
            .unwrap()
            .render_default(meldritch_core::FrameRange::new(0, u64::from(host.frames)).unwrap())
            .unwrap();
        assert!(
            (0..host.frames)
                .any(|frame| published.frame(frame) != host.reader.snapshot().frame(frame))
        );
        for (frame, sample) in expected.samples().iter().enumerate() {
            assert_eq!(
                host.reader.snapshot().frame(frame as u32).unwrap(),
                &[*sample, *sample]
            );
        }
        let source = app.authored_source().unwrap();
        let publication = host.reader.snapshot();
        let generation = host.host.generation();
        let history = app.history().len();
        assert!(
            app.dispatch(AppCommand::EditPatch(PatchEdit {
                synth: "bass".into(),
                operations: vec![
                    PatchOperation::Disconnect {
                        from: "cutoff.control".into(),
                        to: "tone.cutoff".into()
                    },
                    PatchOperation::Connect {
                        from: "unipolar.control".into(),
                        to: "tone.cutoff".into()
                    },
                ]
            }))
            .is_err()
        );
        host.poll(&mut app).unwrap();
        assert!(Arc::ptr_eq(&source, &app.authored_source().unwrap()));
        assert!(Arc::ptr_eq(&publication, &host.reader.snapshot()));
        assert_eq!(generation, host.host.generation());
        assert_eq!(history, app.history().len());
        callback.render_f64(&mut samples);
        assert!(samples.iter().any(|sample| sample.abs() > 0.01));
        assert_eq!(app.view_model().transport.state, TransportState::Playing);
        let published = host.reader.snapshot();
        app.dispatch(AppCommand::EditPatch(PatchEdit {
            synth: "bass".into(),
            operations: vec![
                PatchOperation::Instantiate {
                    id: "depth".into(),
                    kind: meldritch_dsl::ModuleKind::ControlAttenuate,
                },
                PatchOperation::SetParameter {
                    module: "env".into(),
                    parameter: meldritch_dsl::InstrumentParameter::Attack,
                    value: meldritch_dsl::InstrumentValue::Number(0.25),
                },
                PatchOperation::Disconnect {
                    from: "slow.control".into(),
                    to: "unipolar.control".into(),
                },
                PatchOperation::Disconnect {
                    from: "slow.control".into(),
                    to: "vibrato.control".into(),
                },
                PatchOperation::Connect {
                    from: "slow.control".into(),
                    to: "depth.control".into(),
                },
                PatchOperation::Connect {
                    from: "env.control".into(),
                    to: "depth.amount".into(),
                },
                PatchOperation::Connect {
                    from: "depth.control".into(),
                    to: "unipolar.control".into(),
                },
                PatchOperation::Connect {
                    from: "depth.control".into(),
                    to: "vibrato.control".into(),
                },
            ],
        }))
        .unwrap();
        assert!(Arc::ptr_eq(&published, &host.reader.snapshot()));
        ready(&mut app, &mut host);
        let expected = CompiledSongPatch::compile_initial(&app.authored_source().unwrap())
            .unwrap()
            .render_default(meldritch_core::FrameRange::new(0, u64::from(host.frames)).unwrap())
            .unwrap();
        assert!(
            (0..host.frames)
                .any(|frame| published.frame(frame) != host.reader.snapshot().frame(frame))
        );
        for (frame, sample) in expected.samples().iter().enumerate() {
            assert_eq!(
                host.reader.snapshot().frame(frame as u32).unwrap(),
                &[*sample, *sample]
            );
        }
        let source = app.authored_source().unwrap();
        let published = host.reader.snapshot();
        let generation = host.host.generation();
        let history = app.history().len();
        assert!(
            app.dispatch(AppCommand::EditPatch(PatchEdit {
                synth: "bass".into(),
                operations: vec![
                    PatchOperation::Disconnect {
                        from: "env.control".into(),
                        to: "depth.amount".into()
                    },
                    PatchOperation::Connect {
                        from: "slow.control".into(),
                        to: "depth.amount".into()
                    },
                ]
            }))
            .is_err()
        );
        host.poll(&mut app).unwrap();
        assert!(Arc::ptr_eq(&source, &app.authored_source().unwrap()));
        assert!(Arc::ptr_eq(&published, &host.reader.snapshot()));
        assert_eq!(generation, host.host.generation());
        assert_eq!(history, app.history().len());
        callback.render_f64(&mut samples);
        assert!(samples.iter().any(|sample| sample.abs() > 0.01));
        assert_eq!(app.view_model().transport.state, TransportState::Playing);
        let published = host.reader.snapshot();
        app.dispatch(AppCommand::EditPatch(PatchEdit {
            synth: "bass".into(),
            operations: vec![
                PatchOperation::Instantiate {
                    id: "clock".into(),
                    kind: meldritch_dsl::ModuleKind::Lfo,
                },
                PatchOperation::Instantiate {
                    id: "clock-scale".into(),
                    kind: meldritch_dsl::ModuleKind::ControlScale,
                },
                PatchOperation::Instantiate {
                    id: "speed".into(),
                    kind: meldritch_dsl::ModuleKind::ControlHertz,
                },
                PatchOperation::ConfigureControlHertz {
                    module: "speed".into(),
                    settings: meldritch_dsl::ControlHertzSettings {
                        gain_hz: 3.0,
                        offset_hz: 1.0,
                        clamp_min_hz: 1.0,
                        clamp_max_hz: 4.0,
                    },
                },
                PatchOperation::Connect {
                    from: "clock.control".into(),
                    to: "clock-scale.control".into(),
                },
                PatchOperation::Connect {
                    from: "clock-scale.control".into(),
                    to: "speed.control".into(),
                },
                PatchOperation::Connect {
                    from: "speed.control".into(),
                    to: "slow.rate".into(),
                },
            ],
        }))
        .unwrap();
        assert!(Arc::ptr_eq(&published, &host.reader.snapshot()));
        ready(&mut app, &mut host);
        let expected = CompiledSongPatch::compile_initial(&app.authored_source().unwrap())
            .unwrap()
            .render_default(meldritch_core::FrameRange::new(0, u64::from(host.frames)).unwrap())
            .unwrap();
        assert!(
            (0..host.frames)
                .any(|frame| published.frame(frame) != host.reader.snapshot().frame(frame))
        );
        for (frame, sample) in expected.samples().iter().enumerate() {
            assert_eq!(
                host.reader.snapshot().frame(frame as u32).unwrap(),
                &[*sample, *sample]
            );
        }
        let source = app.authored_source().unwrap();
        let published = host.reader.snapshot();
        let generation = host.host.generation();
        let history = app.history().len();
        assert!(
            app.dispatch(AppCommand::EditPatch(PatchEdit {
                synth: "bass".into(),
                operations: vec![
                    PatchOperation::Disconnect {
                        from: "clock.control".into(),
                        to: "clock-scale.control".into()
                    },
                    PatchOperation::Connect {
                        from: "slow.control".into(),
                        to: "clock-scale.control".into()
                    },
                ]
            }))
            .is_err()
        );
        host.poll(&mut app).unwrap();
        assert!(Arc::ptr_eq(&source, &app.authored_source().unwrap()));
        assert!(Arc::ptr_eq(&published, &host.reader.snapshot()));
        assert_eq!(generation, host.host.generation());
        assert_eq!(history, app.history().len());
        callback.render_f64(&mut samples);
        assert!(samples.iter().any(|sample| sample.abs() > 0.01));
        assert_eq!(app.view_model().transport.state, TransportState::Playing);
        host.save_project(&mut app).unwrap();
        host.finish(&app).unwrap();
        assert!(host.stream.is_none());
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
        let project = project();
        let (mut app, engine) = AppController::new_empty_project(&project).unwrap();
        let mut host = NativeProjectAudio::new(&app, engine).unwrap();
        host.poll(&mut app).unwrap();
        assert_eq!(std::fs::read_dir(project.root()).unwrap().count(), 0);
        let gesture = native_gesture;
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
        if let Ok(id) = std::env::var("MELDRITCH_TWO_COMPOSITION_ID") {
            assert_eq!(source.selected_composition(), Some(id.as_str()));
            assert_eq!(
                (
                    source.synths().len(),
                    source.note_patterns().len(),
                    source.pattern_voices().len(),
                    source.compositions().len()
                ),
                (1, 3, 2, 2)
            );
        }
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
