//! Control-thread acceptance and one worker over the existing mono DSP kernel.
use meldritch_app::{AppCommand, AppCommandResult, AppController, KeyboardResetReason};
use meldritch_audio::streaming_audio::{StreamingControl, StreamingProducer};
use meldritch_core::FrameRange;
use meldritch_render::song_render::{LiveMonoRenderer, LiveVoiceEvent, compile_pattern_voice};
use std::{
    sync::{Arc, Mutex},
    thread::{self, JoinHandle},
    time::Duration,
};

const BLOCK: u64 = 256;
const LOOKAHEAD: u64 = 1024;
struct State {
    renderer: Option<LiveMonoRenderer>,
    pending: Option<(Arc<meldritch_dsl::ValidatedSong>, String, u64)>,
    events: Vec<LiveVoiceEvent>,
    frontier: u64,
    app_epoch: u64,
    stream_epoch: u64,
    level: f64,
    error: Option<String>,
    shutdown: bool,
}
pub(crate) struct LiveKeyboardWorker {
    state: Arc<Mutex<State>>,
    pub(crate) control: StreamingControl,
    thread: Option<JoinHandle<()>>,
}
impl LiveKeyboardWorker {
    #[cfg(test)]
    pub(crate) fn frontier(&self) -> u64 {
        self.state.lock().unwrap().frontier
    }
    pub(crate) fn new(
        mut producer: StreamingProducer,
        control: StreamingControl,
    ) -> Result<Self, String> {
        let state = Arc::new(Mutex::new(State {
            renderer: None,
            pending: None,
            events: Vec::with_capacity(128),
            frontier: control.clock(),
            app_epoch: 0,
            stream_epoch: 0,
            level: 1.0,
            error: None,
            shutdown: false,
        }));
        let shared = state.clone();
        let clock = control.clone();
        let thread = thread::Builder::new()
            .name("keyboard-mono-render".into())
            .spawn(move || {
                loop {
                    let Ok(mut state) = shared.lock() else {
                        clock.deactivate();
                        break;
                    };
                    if state.shutdown {
                        break;
                    }
                    let result = advance(&mut state, &mut producer, &clock);
                    if let Err(error) = result {
                        state.error = Some(error);
                        state.renderer = None;
                        state.pending = None;
                        clock.deactivate();
                    }
                    drop(state);
                    thread::sleep(Duration::from_millis(1));
                }
            })
            .map_err(|e| format!("keyboard worker startup: {e}"))?;
        Ok(Self {
            state,
            control,
            thread: Some(thread),
        })
    }
    pub(crate) fn bind(
        &self,
        app: &mut AppController,
        voice: Option<String>,
    ) -> Result<(), String> {
        let mut state = self.state.lock().map_err(|_| "keyboard worker failed")?;
        let frame = self.control.clock().max(app.keyboard().frame);
        app.dispatch(AppCommand::BindKeyboard {
            voice: voice.clone(),
            frame,
        })
        .map_err(|e| format!("{e:?}"))?;
        self.control.deactivate();
        state.renderer = None;
        state.pending = None;
        state.events.clear();
        state.error = None;
        state.frontier = frame;
        state.app_epoch = app.keyboard().epoch;
        if let Some(voice) = voice {
            state.stream_epoch = self.control.activate()?;
            state.pending = Some((
                app.authored_source().ok_or("no project source")?,
                voice,
                frame,
            ));
        }
        Ok(())
    }
    /// Lock only on control/worker threads. Acceptance and scheduling share one
    /// frontier so a renderer cannot race past a just-accepted event frame.
    pub(crate) fn key(
        &self,
        app: &mut AppController,
        bytes: &[u8],
        epoch: u64,
    ) -> Result<(), String> {
        let mut state = self.state.lock().map_err(|_| "keyboard worker failed")?;
        let frame = state
            .frontier
            .max(self.control.clock())
            .max(app.keyboard().frame);
        let Some(command) = meldritch_control::devices::keyboard_key_command(bytes, epoch, frame)
        else {
            return Ok(());
        };
        if state.events.len() > 125 {
            return Err("keyboard pending event budget exceeded".into());
        }
        let result = app.dispatch(command).map_err(|e| format!("{e:?}"))?;
        append(&mut state, frame, result);
        Ok(())
    }
    pub(crate) fn reset(
        &self,
        app: &mut AppController,
        reason: KeyboardResetReason,
    ) -> Result<(), String> {
        let mut state = self.state.lock().map_err(|_| "keyboard worker failed")?;
        let frame = state
            .frontier
            .max(self.control.clock())
            .max(app.keyboard().frame);
        let result = app
            .dispatch(AppCommand::ResetKeyboard {
                epoch: app.keyboard().epoch,
                frame,
                reason,
            })
            .map_err(|e| format!("{e:?}"))?;
        // Preserve queued accepted events/DSP history, reserve room for release.
        append(&mut state, frame, result);
        state.app_epoch = app.keyboard().epoch;
        state.stream_epoch = self.control.activate()?;
        Ok(())
    }
    pub(crate) fn synchronize(&self, app: &AppController) -> Result<(), String> {
        let mut state = self.state.lock().map_err(|_| "keyboard worker failed")?;
        if let Some(error) = state.error.take() {
            return Err(error);
        }
        if app.keyboard().voice.is_none() || state.app_epoch != app.keyboard().epoch {
            self.control.deactivate();
            state.renderer = None;
            state.pending = None;
            state.events.clear();
        }
        if let Some(id) = &app.keyboard().voice {
            let source = app.authored_source().ok_or("no project source")?;
            let voice = &source.pattern_voices()[id];
            state.level = source.synths()[&voice.synth]
                .output_level_control()
                .and_then(|control| {
                    app.output_levels()
                        .get(&(id.clone(), control.into()))
                        .copied()
                })
                .unwrap_or(voice.level);
        }
        Ok(())
    }
}
fn append(state: &mut State, frame: u64, result: AppCommandResult) {
    if let AppCommandResult::KeyboardUpdated { outputs, .. } = result {
        state.events.extend(
            outputs
                .into_iter()
                .map(|input| LiveVoiceEvent { frame, input }),
        );
    }
}
fn advance(
    state: &mut State,
    producer: &mut StreamingProducer,
    control: &StreamingControl,
) -> Result<(), String> {
    if let Some((source, id, origin)) = state.pending.take() {
        let voice = &source.pattern_voices()[&id];
        state.renderer = Some(
            compile_pattern_voice(&source, voice)
                .map_err(|e| e.to_string())?
                .live_renderer(voice.initial_pitch, origin)
                .map_err(|e| e.to_string())?,
        );
        state.frontier = origin;
    }
    let Some(renderer) = &mut state.renderer else {
        return Ok(());
    };
    if !control.enabled()
        || state.frontier.saturating_add(BLOCK) > control.clock().saturating_add(LOOKAHEAD)
        || producer.available() < BLOCK as usize
    {
        return Ok(());
    }
    let start = renderer.cursor();
    let end = start.checked_add(BLOCK).ok_or("keyboard frame overflow")?;
    let count = state.events.partition_point(|event| event.frame < end);
    let audio = renderer
        .render(
            FrameRange::new(start, end).map_err(|e| e.to_string())?,
            &state.events[..count],
        )
        .map_err(|e| e.to_string())?;
    state.events.drain(..count);
    state.frontier = end;
    let clock = control.clock();
    if end <= clock {
        return Ok(());
    } // Advance DSP through missed device frames.
    let offset = clock.saturating_sub(start) as usize;
    let channels = usize::from(audio.channels());
    let samples: Vec<_> = audio
        .samples()
        .chunks_exact(channels)
        .skip(offset)
        .map(|frame| frame[0] * state.level)
        .collect();
    producer.publish(start + offset as u64, state.stream_epoch, &samples)
}
impl Drop for LiveKeyboardWorker {
    fn drop(&mut self) {
        self.control.deactivate();
        if let Ok(mut state) = self.state.lock() {
            state.shutdown = true;
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pending_keyboard_budget_reserves_release_and_rejects_before_acceptance() {
        let root =
            std::env::temp_dir().join(format!("meldritch-keyboard-budget-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let (mut app, _) = AppController::new_empty_project(
            &meldritch_dsl::load_project_directory(&root).unwrap(),
        )
        .unwrap();
        app.enable_native_project_source(
            meldritch_control::native_project::load_native_project(
                &root,
                include_str!("../../../songs/examples/36-patch-lfo/main.project.lua"),
            )
            .unwrap(),
        )
        .unwrap();
        let (mut producer, mut consumer, control) =
            meldritch_audio::streaming_audio::streaming_audio(4096, 5_000_000_000).unwrap();
        // Same worker/control methods with a paused worker: deterministic backpressure.
        let worker = LiveKeyboardWorker {
            state: Arc::new(Mutex::new(State {
                renderer: None,
                pending: None,
                events: Vec::with_capacity(128),
                frontier: control.clock(),
                app_epoch: 0,
                stream_epoch: 0,
                level: 1.0,
                error: None,
                shutdown: false,
            })),
            control,
            thread: None,
        };
        worker.bind(&mut app, Some("bass-instance".into())).unwrap();
        let epoch = app.keyboard().epoch;
        for note in 0..63 {
            worker.key(&mut app, &[0x90, note, 127], epoch).unwrap();
        }
        let before = app.keyboard().clone();
        let history = app.history().len();
        assert!(
            worker
                .key(&mut app, &[0x90, 64, 127], before.epoch)
                .is_err()
        );
        assert_eq!(app.keyboard(), &before);
        assert_eq!(app.history().len(), history);
        assert_eq!(worker.state.lock().unwrap().events.len(), 126);
        worker
            .key(&mut app, &[0xb0, 64, 127], before.epoch)
            .unwrap(); // Unmapped sustain does not reset.
        worker
            .reset(&mut app, KeyboardResetReason::Overflow)
            .unwrap();
        assert_eq!(worker.state.lock().unwrap().events.len(), 127);
        assert_eq!(app.keyboard().held_keys, 0);
        assert!(
            worker
                .key(&mut app, &[0x90, 64, 127], before.epoch)
                .is_err()
        );
        advance(
            &mut worker.state.lock().unwrap(),
            &mut producer,
            &worker.control,
        )
        .unwrap();
        consumer.begin_callback();
        for _ in 0..256 {
            assert_eq!(consumer.next_sample(), 0.0);
        }
        assert_eq!(worker.control.clock(), 5_000_000_256);
        for _ in 0..4 {
            advance(
                &mut worker.state.lock().unwrap(),
                &mut producer,
                &worker.control,
            )
            .unwrap();
        }
        let frontier = worker.frontier();
        assert_eq!(frontier, worker.control.clock() + LOOKAHEAD);
        consumer.begin_callback();
        for _ in 0..50 {
            consumer.next_sample();
        }
        advance(
            &mut worker.state.lock().unwrap(),
            &mut producer,
            &worker.control,
        )
        .unwrap();
        // Irregular device buffers must not permit a block to exceed lookahead.
        assert_eq!(worker.frontier(), frontier);
        assert!(worker.frontier() <= worker.control.clock() + LOOKAHEAD);
        std::fs::remove_dir_all(root).unwrap();
    }
}
