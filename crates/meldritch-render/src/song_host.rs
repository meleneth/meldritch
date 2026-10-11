//! Shared non-realtime compiled-song, rerender and atomic-publication boundary.
use meldritch_audio::{audio_publication::AudioSnapshotPublisher, published_audio::PublishedAudio};
use meldritch_core::FrameRange;
use std::{
    collections::BTreeMap,
    sync::{Arc, Condvar, Mutex},
    thread::{self, JoinHandle},
};

#[derive(Clone, Debug, PartialEq)]
pub enum CompiledSongPatch {
    Delayed(crate::song_render::CompiledDelayedNotePatch),
    Mixed(crate::song_render::CompiledMixedNoteSong),
}

impl CompiledSongPatch {
    pub fn compile_initial(song: &meldritch_dsl::ValidatedSong) -> Result<Self, String> {
        match crate::song_render::compile_delayed_note_song(song) {
            Ok(patch) => Ok(Self::Delayed(patch)),
            Err(delayed_error) => {
                let mixed = crate::song_render::compile_mixed_note_song(song)
                    .map_err(|mixed_error| {
                        format!(
                            "failed to compile song for TUI playback: delayed={delayed_error:?}; mixed={mixed_error:?}"
                        )
                    })?;
                Ok(Self::Mixed(mixed))
            }
        }
    }

    pub fn render(
        &self,
        range: FrameRange,
        overrides: &SongLiveOverrides,
    ) -> Result<meldritch_audio::AudioBlock, crate::song_render::SongRenderError> {
        match self {
            Self::Delayed(patch) => patch.render_with_extended_overrides(
                range,
                overrides.feedback,
                overrides.cutoff_hz,
                overrides.resonance,
                overrides.mix,
            ),
            Self::Mixed(patch) => {
                let synth_filter_overrides = overrides.synth_filter_overrides();
                let lane_level_overrides = overrides.lane_level_overrides();
                patch.render_with_live_overrides(
                    range,
                    &synth_filter_overrides,
                    &lane_level_overrides,
                )
            }
        }
    }

    pub fn render_default(
        &self,
        range: FrameRange,
    ) -> Result<meldritch_audio::AudioBlock, crate::song_render::SongRenderError> {
        self.render(range, &SongLiveOverrides::default())
    }

    pub fn feedback(&self) -> Option<f64> {
        match self {
            Self::Delayed(patch) => Some(patch.feedback()),
            Self::Mixed(_) => None,
        }
    }

    pub fn mix(&self) -> Option<f64> {
        match self {
            Self::Delayed(patch) => Some(patch.mix()),
            Self::Mixed(_) => None,
        }
    }

    pub fn cutoff_hz(&self) -> Option<f64> {
        match self {
            Self::Delayed(patch) => patch.cutoff_hz(),
            Self::Mixed(_) => None,
        }
    }

    pub fn resonance(&self) -> Option<f64> {
        match self {
            Self::Delayed(patch) => patch.resonance(),
            Self::Mixed(_) => None,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SongLiveOverrides {
    pub feedback: Option<f64>,
    pub cutoff_hz: Option<f64>,
    pub resonance: Option<f64>,
    pub mix: Option<f64>,
    pub synth_filters: BTreeMap<SynthFilterTarget, SongFilterLiveOverrides>,
    pub lane_levels: BTreeMap<String, f64>,
    pub lane_transpose_semitones: BTreeMap<String, i8>,
}

impl SongLiveOverrides {
    pub fn synth_filter_overrides(&self) -> Vec<crate::song_render::CompiledSynthFilterOverride> {
        self.synth_filters
            .iter()
            .map(|(target, overrides)| {
                crate::song_render::CompiledSynthFilterOverride::new(
                    target.synth_id.clone(),
                    target.module_id.clone(),
                    overrides.cutoff_hz,
                    overrides.resonance,
                )
            })
            .collect()
    }

    pub fn lane_level_overrides(&self) -> Vec<crate::song_render::CompiledLaneLevelOverride> {
        self.lane_levels
            .iter()
            .map(|(lane_id, level)| {
                crate::song_render::CompiledLaneLevelOverride::new(lane_id.clone(), *level)
            })
            .collect()
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SynthFilterTarget {
    pub synth_id: String,
    pub module_id: String,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SongFilterLiveOverrides {
    pub cutoff_hz: Option<f64>,
    pub resonance: Option<f64>,
}

type Completion = (
    u64,
    Result<meldritch_audio::AudioBlock, crate::song_render::SongRenderError>,
    SongLiveOverrides,
);
struct SongRenderRequest {
    generation: u64,
    patch: CompiledSongPatch,
    overrides: SongLiveOverrides,
}
#[derive(Default)]
struct WorkerState {
    pending: Option<SongRenderRequest>,
    completed: Option<Completion>,
    shutdown: bool,
}

/// One pending request and one refreshable completion slot; no audio-thread use.
pub struct SongRerenderWorker {
    shared: Arc<(Mutex<WorkerState>, Condvar)>,
    thread: Option<JoinHandle<()>>,
}
impl SongRerenderWorker {
    pub fn new(frames: u32) -> Self {
        let shared = Arc::new((Mutex::new(WorkerState::default()), Condvar::new()));
        let owned = Arc::clone(&shared);
        let thread = thread::spawn(move || {
            loop {
                let request = {
                    let (lock, changed) = &*owned;
                    let mut state = lock.lock().expect("song worker lock poisoned");
                    while state.pending.is_none() && !state.shutdown {
                        state = changed.wait(state).expect("song worker condvar poisoned");
                    }
                    if state.shutdown {
                        break;
                    }
                    state.pending.take().expect("pending request after wait")
                };
                let range = FrameRange::new(0, u64::from(frames)).expect("ordered song range");
                let result = request.patch.render(range, &request.overrides);
                let mut state = owned.0.lock().expect("song worker lock poisoned");
                // A newer pending request supersedes obsolete audio, never command facts.
                if state
                    .pending
                    .as_ref()
                    .is_none_or(|next| next.generation <= request.generation)
                {
                    state.completed = Some((request.generation, result, request.overrides));
                }
            }
        });
        Self {
            shared,
            thread: Some(thread),
        }
    }
    pub fn submit(
        &mut self,
        generation: u64,
        patch: CompiledSongPatch,
        overrides: SongLiveOverrides,
    ) {
        let mut state = self.shared.0.lock().expect("song worker lock poisoned");
        state.pending = Some(SongRenderRequest {
            generation,
            patch,
            overrides,
        });
        state.completed = None;
        self.shared.1.notify_one();
    }
    pub fn latest_completed(&self) -> Option<Completion> {
        self.shared
            .0
            .lock()
            .expect("song worker lock poisoned")
            .completed
            .take()
    }
}
impl Drop for SongRerenderWorker {
    fn drop(&mut self) {
        self.shared
            .0
            .lock()
            .expect("song worker lock poisoned")
            .shutdown = true;
        self.shared.1.notify_one();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

pub fn publish_song_audio(
    publisher: &AudioSnapshotPublisher,
    block: &meldritch_audio::AudioBlock,
    chunk_frames: u32,
) -> Result<(), String> {
    let snapshot = PublishedAudio::from_block(block, chunk_frames)
        .map_err(|error| format!("invalid song audio snapshot: {error:?}"))?;
    publisher
        .publish(snapshot)
        .map_err(|error| format!("incompatible song audio publication: {error:?}"))
}
