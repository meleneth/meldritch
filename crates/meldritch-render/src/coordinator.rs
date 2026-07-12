//! Long-lived playhead-driven render coordination.

use crate::dsp::BassVoiceSettings;
use crate::dynamics::SidechainRelation;
use crate::effects::EffectSendRule;
use crate::{
    ChunkPublicationDiagnostics, ChunkPublicationError, RealtimeChunkPublication, RenderHorizon,
    RenderHorizonSubmission, RenderSettings, RenderWorkerPool, RenderWorkerPoolError,
    WorkerDiagnostics, plan_sample_render_horizon, render_pattern_samples_chunk,
    submit_sample_render_horizon,
};
use arc_swap::ArcSwap;
use meldritch_audio::SampleBuffer;
use meldritch_audio::audio_publication::AudioSnapshotReader;
use meldritch_audio::realtime_status::RealtimeStatusMonitor;
use meldritch_core::{AutomationLane, FrameRange, Pattern, ProbabilitySeed, Tempo};
use std::collections::BTreeMap;
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RenderCoordinatorError {
    ZeroWorkers,
    EmptyTimeline,
    ZeroChunkFrames,
    ZeroPollInterval,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RenderStateUpdateError {
    PatternChanged,
    ChannelCountChanged,
    SampleRateChanged,
    InvalidDirtyRange,
    Invalidation(ChunkPublicationError),
}

#[derive(Clone, Debug)]
pub struct SampleRenderState {
    pattern: Pattern,
    tempo: Tempo,
    probability_seed: ProbabilitySeed,
    settings: RenderSettings,
    samples_by_note: Arc<BTreeMap<u8, SampleBuffer>>,
    bass_layer: Option<(meldritch_core::TrackId, BassVoiceSettings)>,
    chord_layer: Option<ChordLayer>,
    automation: Arc<Vec<AutomationLane>>,
    effect_rules: Arc<Vec<EffectSendRule>>,
    sidechain: Option<SidechainRelation>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChordLayer {
    pub first_track: meldritch_core::TrackId,
    pub last_track: meldritch_core::TrackId,
    pub settings: BassVoiceSettings,
    pub voice_count: usize,
}

impl SampleRenderState {
    #[must_use]
    pub fn new(
        pattern: Pattern,
        tempo: Tempo,
        probability_seed: ProbabilitySeed,
        settings: RenderSettings,
        samples_by_note: Arc<BTreeMap<u8, SampleBuffer>>,
    ) -> Self {
        Self {
            pattern,
            tempo,
            probability_seed,
            settings,
            samples_by_note,
            bass_layer: None,
            chord_layer: None,
            automation: Arc::new(Vec::new()),
            effect_rules: Arc::new(Vec::new()),
            sidechain: None,
        }
    }

    #[must_use]
    pub const fn pattern(&self) -> &Pattern {
        &self.pattern
    }

    #[must_use]
    pub const fn tempo(&self) -> Tempo {
        self.tempo
    }

    #[must_use]
    pub const fn probability_seed(&self) -> ProbabilitySeed {
        self.probability_seed
    }

    #[must_use]
    pub const fn settings(&self) -> RenderSettings {
        self.settings
    }

    #[must_use]
    pub fn samples_by_note(&self) -> &Arc<BTreeMap<u8, SampleBuffer>> {
        &self.samples_by_note
    }

    #[must_use]
    pub const fn bass_layer(&self) -> Option<(meldritch_core::TrackId, BassVoiceSettings)> {
        self.bass_layer
    }

    #[must_use]
    pub fn with_bass_layer(
        &self,
        track: meldritch_core::TrackId,
        settings: BassVoiceSettings,
    ) -> Self {
        let mut state = self.clone();
        state.bass_layer = Some((track, settings));
        state
    }

    #[must_use]
    pub const fn chord_layer(&self) -> Option<ChordLayer> {
        self.chord_layer
    }

    #[must_use]
    pub fn with_chord_layer(&self, chord_layer: ChordLayer) -> Self {
        let mut state = self.clone();
        state.chord_layer = Some(chord_layer);
        state
    }

    #[must_use]
    pub fn with_automation(&self, lanes: Vec<AutomationLane>) -> Self {
        let mut state = self.clone();
        state.automation = Arc::new(lanes);
        state
    }

    #[must_use]
    pub fn automation(&self) -> &[AutomationLane] {
        &self.automation
    }

    #[must_use]
    pub fn with_effect_rules(&self, rules: Vec<EffectSendRule>) -> Self {
        let mut state = self.clone();
        state.effect_rules = Arc::new(rules);
        state
    }

    #[must_use]
    pub fn effect_rules(&self) -> &[EffectSendRule] {
        &self.effect_rules
    }

    #[must_use]
    pub fn with_sidechain(&self, relation: SidechainRelation) -> Self {
        let mut state = self.clone();
        state.sidechain = Some(relation);
        state
    }

    #[must_use]
    pub const fn sidechain(&self) -> Option<SidechainRelation> {
        self.sidechain
    }

    #[must_use]
    pub fn with_pattern(&self, pattern: Pattern) -> Self {
        Self {
            pattern,
            tempo: self.tempo,
            probability_seed: self.probability_seed,
            settings: self.settings,
            samples_by_note: Arc::clone(&self.samples_by_note),
            bass_layer: self.bass_layer,
            chord_layer: self.chord_layer,
            automation: Arc::clone(&self.automation),
            effect_rules: Arc::clone(&self.effect_rules),
            sidechain: self.sidechain,
        }
    }

    #[must_use]
    pub fn with_samples(&self, samples_by_note: Arc<BTreeMap<u8, SampleBuffer>>) -> Self {
        Self {
            pattern: self.pattern.clone(),
            tempo: self.tempo,
            probability_seed: self.probability_seed,
            settings: self.settings,
            samples_by_note,
            bass_layer: self.bass_layer,
            chord_layer: self.chord_layer,
            automation: Arc::clone(&self.automation),
            effect_rules: Arc::clone(&self.effect_rules),
            sidechain: self.sidechain,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RenderCoordinatorConfig {
    worker_count: usize,
    timeline_frames: u32,
    chunk_frames: u32,
    warm_chunks: usize,
    poll_interval: Duration,
}

impl RenderCoordinatorConfig {
    pub fn new(
        worker_count: usize,
        timeline_frames: u32,
        chunk_frames: u32,
        warm_chunks: usize,
        poll_interval: Duration,
    ) -> Result<Self, RenderCoordinatorError> {
        if worker_count == 0 {
            return Err(RenderCoordinatorError::ZeroWorkers);
        }
        if timeline_frames == 0 {
            return Err(RenderCoordinatorError::EmptyTimeline);
        }
        if chunk_frames == 0 {
            return Err(RenderCoordinatorError::ZeroChunkFrames);
        }
        if poll_interval.is_zero() {
            return Err(RenderCoordinatorError::ZeroPollInterval);
        }
        Ok(Self {
            worker_count,
            timeline_frames,
            chunk_frames,
            warm_chunks,
            poll_interval,
        })
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RenderCoordinatorDiagnostics {
    pub refreshes: u64,
    pub playhead: u32,
    pub last_submission: RenderHorizonSubmission,
    pub workers: WorkerDiagnostics,
    pub publication: ChunkPublicationDiagnostics,
    pub chord_active_voices: usize,
    pub chord_peak_voices: usize,
    pub chord_voice_steals: u64,
}

#[derive(Default)]
struct CoordinatorControl {
    shutdown: bool,
    diagnostics: RenderCoordinatorDiagnostics,
    pending_invalidations: Vec<FrameRange>,
}

struct CoordinatorShared {
    control: Mutex<CoordinatorControl>,
    changed: Condvar,
}

pub struct RenderCoordinator {
    shared: Arc<CoordinatorShared>,
    publication: RealtimeChunkPublication,
    render_state: Arc<ArcSwap<SampleRenderState>>,
    pattern_id: meldritch_core::PatternId,
    channels: u16,
    sample_rate: u32,
    thread: Option<JoinHandle<()>>,
}

impl RenderCoordinator {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        config: RenderCoordinatorConfig,
        pattern: Pattern,
        tempo: Tempo,
        probability_seed: ProbabilitySeed,
        settings: RenderSettings,
        samples_by_note: Arc<BTreeMap<u8, SampleBuffer>>,
        status: RealtimeStatusMonitor,
    ) -> Result<Self, RenderCoordinatorError> {
        Self::new_from_state(
            config,
            SampleRenderState::new(pattern, tempo, probability_seed, settings, samples_by_note),
            status,
        )
    }

    pub fn new_from_state(
        config: RenderCoordinatorConfig,
        initial_state: SampleRenderState,
        status: RealtimeStatusMonitor,
    ) -> Result<Self, RenderCoordinatorError> {
        let pattern_id = initial_state.pattern.id();
        let channels = initial_state.settings.channels();
        let sample_rate = initial_state.tempo.sample_rate();
        let render_state = Arc::new(ArcSwap::from_pointee(initial_state));
        let publication =
            RealtimeChunkPublication::new(channels, config.timeline_frames, config.chunk_frames)
                .map_err(|_| RenderCoordinatorError::EmptyTimeline)?;
        let pool = RenderWorkerPool::with_completion_hook(
            config.worker_count,
            publication.completion_hook(),
        )
        .map_err(|error| match error {
            RenderWorkerPoolError::ZeroWorkers => RenderCoordinatorError::ZeroWorkers,
        })?;
        let shared = Arc::new(CoordinatorShared {
            control: Mutex::new(CoordinatorControl::default()),
            changed: Condvar::new(),
        });
        let thread_shared = Arc::clone(&shared);
        let thread_publication = publication.clone();
        let thread_render_state = Arc::clone(&render_state);
        let thread = thread::spawn(move || {
            coordinator_loop(
                config,
                thread_render_state,
                status,
                pool,
                thread_publication,
                thread_shared,
            );
        });
        Ok(Self {
            shared,
            publication,
            render_state,
            pattern_id,
            channels,
            sample_rate,
            thread: Some(thread),
        })
    }

    #[must_use]
    pub fn audio_reader(&self) -> AudioSnapshotReader {
        self.publication.reader()
    }

    #[must_use]
    pub fn realtime_publication(&self) -> RealtimeChunkPublication {
        self.publication.clone()
    }

    pub fn audition_block(
        &self,
        block: &meldritch_audio::AudioBlock,
    ) -> Result<(), ChunkPublicationError> {
        self.publication.publish_override(block)
    }

    pub fn restore_live_audio(&self) -> Result<(), ChunkPublicationError> {
        self.publication.restore_live_snapshot()
    }

    #[must_use]
    pub fn diagnostics(&self) -> RenderCoordinatorDiagnostics {
        self.shared
            .control
            .lock()
            .expect("render coordinator state lock poisoned")
            .diagnostics
    }

    pub fn wake(&self) {
        self.shared.changed.notify_one();
    }

    pub fn invalidate_range(&self, range: FrameRange) -> Result<usize, ChunkPublicationError> {
        let invalidated = self.publication.invalidate_range(range)?;
        let mut control = self
            .shared
            .control
            .lock()
            .expect("render coordinator state lock poisoned");
        control.pending_invalidations.push(range);
        control.diagnostics.publication = self.publication.diagnostics();
        drop(control);
        self.wake();
        Ok(invalidated)
    }

    pub fn update_render_state(
        &self,
        state: SampleRenderState,
        dirty_range: FrameRange,
    ) -> Result<usize, RenderStateUpdateError> {
        self.update_render_state_ranges(state, &[dirty_range])
    }

    pub fn update_render_state_ranges(
        &self,
        state: SampleRenderState,
        dirty_ranges: &[FrameRange],
    ) -> Result<usize, RenderStateUpdateError> {
        if state.pattern.id() != self.pattern_id {
            return Err(RenderStateUpdateError::PatternChanged);
        }
        if state.settings.channels() != self.channels {
            return Err(RenderStateUpdateError::ChannelCountChanged);
        }
        if state.tempo.sample_rate() != self.sample_rate {
            return Err(RenderStateUpdateError::SampleRateChanged);
        }
        if dirty_ranges.is_empty()
            || dirty_ranges
                .iter()
                .any(|range| range.start() >= range.end())
        {
            return Err(RenderStateUpdateError::InvalidDirtyRange);
        }
        self.render_state.store(Arc::new(state));
        let mut invalidated = 0;
        for range in dirty_ranges {
            invalidated += self
                .publication
                .invalidate_range(*range)
                .map_err(RenderStateUpdateError::Invalidation)?;
        }
        let mut control = self
            .shared
            .control
            .lock()
            .expect("render coordinator state lock poisoned");
        control
            .pending_invalidations
            .extend_from_slice(dirty_ranges);
        control.diagnostics.publication = self.publication.diagnostics();
        drop(control);
        self.wake();
        Ok(invalidated)
    }

    pub fn wait_for_ready_chunks(&self, minimum: usize, timeout: Duration) -> bool {
        let deadline = Instant::now() + timeout;
        let mut control = self
            .shared
            .control
            .lock()
            .expect("render coordinator state lock poisoned");
        while control.diagnostics.publication.ready_chunks < minimum {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return false;
            }
            let (next, result) = self
                .shared
                .changed
                .wait_timeout(control, remaining)
                .expect("render coordinator state lock poisoned");
            control = next;
            if result.timed_out() && control.diagnostics.publication.ready_chunks < minimum {
                return false;
            }
        }
        true
    }

    pub fn wait_for_refreshes(&self, minimum: u64, timeout: Duration) -> bool {
        let deadline = Instant::now() + timeout;
        let mut control = self
            .shared
            .control
            .lock()
            .expect("render coordinator state lock poisoned");
        while control.diagnostics.refreshes < minimum {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return false;
            }
            let (next, result) = self
                .shared
                .changed
                .wait_timeout(control, remaining)
                .expect("render coordinator state lock poisoned");
            control = next;
            if result.timed_out() && control.diagnostics.refreshes < minimum {
                return false;
            }
        }
        true
    }

    pub fn shutdown(&mut self) {
        if let Some(thread) = self.thread.take() {
            {
                let mut control = self
                    .shared
                    .control
                    .lock()
                    .expect("render coordinator state lock poisoned");
                control.shutdown = true;
            }
            self.shared.changed.notify_all();
            thread.join().expect("render coordinator panicked");
        }
    }
}

impl Drop for RenderCoordinator {
    fn drop(&mut self) {
        self.shutdown();
    }
}

#[allow(clippy::too_many_arguments)]
fn coordinator_loop(
    config: RenderCoordinatorConfig,
    render_state: Arc<ArcSwap<SampleRenderState>>,
    status: RealtimeStatusMonitor,
    pool: RenderWorkerPool,
    publication: RealtimeChunkPublication,
    shared: Arc<CoordinatorShared>,
) {
    loop {
        let pending_invalidations = {
            let mut control = shared
                .control
                .lock()
                .expect("render coordinator state lock poisoned");
            std::mem::take(&mut control.pending_invalidations)
        };
        let state = render_state.load_full();
        for range in pending_invalidations {
            pool.invalidate_range(state.pattern.id(), range);
        }
        let playhead = status.snapshot().position % config.timeline_frames;
        let horizon = RenderHorizon::new(
            playhead,
            config.timeline_frames,
            config.chunk_frames,
            config.warm_chunks,
        )
        .expect("validated render coordinator horizon");
        let submission = if let Some((track, bass_settings)) = state.bass_layer {
            submit_mixed_horizon(&pool, &publication, &state, horizon, track, bass_settings)
        } else if !state.effect_rules.is_empty() {
            submit_effect_horizon(&pool, &publication, &state, horizon)
        } else {
            submit_sample_render_horizon(
                &pool,
                &publication,
                &state.pattern,
                state.tempo,
                state.probability_seed,
                state.settings,
                Arc::clone(&state.samples_by_note),
                horizon,
            )
            .expect("validated render coordinator submission")
        };
        let chord_diagnostics = state.chord_layer.map(|chord| {
            let mut chord_pattern = state.pattern.clone();
            for track in chord_pattern.active_step_counts_by_track().into_keys() {
                if track < chord.first_track || track > chord.last_track {
                    for step in 0..chord_pattern.length_steps() {
                        let _ =
                            chord_pattern.clear_step(track, meldritch_core::StepIndex::new(step));
                    }
                }
            }
            crate::dsp::polyphonic_schedule_diagnostics(
                &chord_pattern,
                state.tempo,
                FrameRange::new(0, u64::from(config.timeline_frames))
                    .expect("coordinator timeline is ordered"),
                u64::from(playhead),
                state.probability_seed,
                chord.voice_count,
            )
            .expect("validated chord voice count")
        });

        let mut control = shared
            .control
            .lock()
            .expect("render coordinator state lock poisoned");
        control.diagnostics.refreshes += 1;
        control.diagnostics.playhead = playhead;
        control.diagnostics.last_submission = submission;
        control.diagnostics.workers = pool.diagnostics();
        control.diagnostics.publication = publication.diagnostics();
        if let Some(chord) = chord_diagnostics {
            control.diagnostics.chord_active_voices = chord.active_voices;
            control.diagnostics.chord_peak_voices = chord.peak_voices;
            control.diagnostics.chord_voice_steals = chord.stolen_voices;
        }
        shared.changed.notify_all();
        if control.shutdown {
            return;
        }
        let (next, _) = shared
            .changed
            .wait_timeout(control, config.poll_interval)
            .expect("render coordinator state lock poisoned");
        if next.shutdown {
            return;
        }
    }
}

fn submit_effect_horizon(
    pool: &RenderWorkerPool,
    publication: &RealtimeChunkPublication,
    state: &SampleRenderState,
    horizon: RenderHorizon,
) -> RenderHorizonSubmission {
    let plan = plan_sample_render_horizon(
        &state.pattern,
        state.tempo,
        state.probability_seed,
        state.settings,
        &state.samples_by_note,
        horizon,
    )
    .expect("validated effect horizon");
    let mut submission = RenderHorizonSubmission {
        planned_chunks: plan.len(),
        ..RenderHorizonSubmission::default()
    };
    for chunk in plan {
        let effects = crate::effects::effect_artifact_key(
            &state.pattern,
            state.tempo,
            chunk.range,
            state.probability_seed,
            state.settings,
            &state.samples_by_note,
            &state.effect_rules,
        );
        let key = chunk.key.with_dependency_fingerprint(effects.fingerprint);
        let _ = publication.expect_artifact(key);
        if let Some(block) = pool.cached_artifact(key) {
            let _ = publication.publish_artifact(key, &block);
            submission.cache_hits += 1;
            continue;
        }
        let pattern = state.pattern.clone();
        let samples = Arc::clone(&state.samples_by_note);
        let rules = Arc::clone(&state.effect_rules);
        let tempo = state.tempo;
        let seed = state.probability_seed;
        let settings = state.settings;
        if pool.submit_if_needed(key, chunk.priority, move || {
            crate::effects::render_event_aware_effects_chunk(
                &pattern,
                tempo,
                chunk.range,
                seed,
                settings,
                &samples,
                &rules,
            )
            .mix
        }) {
            submission.submitted_jobs += 1;
        }
    }
    submission
}

fn submit_mixed_horizon(
    pool: &RenderWorkerPool,
    publication: &RealtimeChunkPublication,
    state: &SampleRenderState,
    horizon: RenderHorizon,
    bass_track: meldritch_core::TrackId,
    bass_settings: BassVoiceSettings,
) -> RenderHorizonSubmission {
    let plan = plan_sample_render_horizon(
        &state.pattern,
        state.tempo,
        state.probability_seed,
        state.settings,
        &state.samples_by_note,
        horizon,
    )
    .expect("validated mixed horizon");
    let mut submission = RenderHorizonSubmission {
        planned_chunks: plan.len(),
        ..RenderHorizonSubmission::default()
    };
    for chunk in plan {
        let mut key = chunk.key.with_automation(&state.automation);
        if !state.effect_rules.is_empty() {
            let effects = crate::effects::effect_artifact_key(
                &state.pattern,
                state.tempo,
                chunk.range,
                state.probability_seed,
                state.settings,
                &state.samples_by_note,
                &state.effect_rules,
            );
            key = key.with_dependency_fingerprint(effects.fingerprint);
        }
        if let Some(relation) = state.sidechain {
            key = key.with_dependency_fingerprint(crate::dynamics::sidechain_relation_fingerprint(
                relation,
            ));
        }
        let _ = publication.expect_artifact(key);
        if let Some(block) = pool.cached_artifact(key) {
            let _ = publication.publish_artifact(key, &block);
            submission.cache_hits += 1;
            continue;
        }
        let mut drums = state.pattern.clone();
        for step in 0..drums.length_steps() {
            let _ = drums.clear_step(bass_track, meldritch_core::StepIndex::new(step));
        }
        if let Some(chord) = state.chord_layer {
            for raw_track in chord.first_track.raw()..=chord.last_track.raw() {
                for step in 0..drums.length_steps() {
                    let _ = drums.clear_step(
                        meldritch_core::TrackId::new(raw_track),
                        meldritch_core::StepIndex::new(step),
                    );
                }
            }
        }
        let mut bass = state.pattern.clone();
        for track in bass.active_step_counts_by_track().into_keys() {
            if track != bass_track {
                for step in 0..bass.length_steps() {
                    let _ = bass.clear_step(track, meldritch_core::StepIndex::new(step));
                }
            }
        }
        let samples = Arc::clone(&state.samples_by_note);
        let chord_layer = state.chord_layer;
        let chord_pattern = chord_layer.map(|chord| {
            let mut pattern = state.pattern.clone();
            for track in pattern.active_step_counts_by_track().into_keys() {
                if track < chord.first_track || track > chord.last_track {
                    for step in 0..pattern.length_steps() {
                        let _ = pattern.clear_step(track, meldritch_core::StepIndex::new(step));
                    }
                }
            }
            pattern
        });
        let tempo = state.tempo;
        let seed = state.probability_seed;
        let settings = state.settings;
        let automation = Arc::clone(&state.automation);
        let effect_rules = Arc::clone(&state.effect_rules);
        let sidechain = state.sidechain;
        if pool.submit_if_needed(key, chunk.priority, move || {
            let mut mixed = if effect_rules.is_empty() {
                render_pattern_samples_chunk(&drums, tempo, chunk.range, seed, settings, &samples)
            } else {
                crate::effects::render_event_aware_effects_chunk(
                    &drums,
                    tempo,
                    chunk.range,
                    seed,
                    settings,
                    &samples,
                    &effect_rules,
                )
                .mix
            };
            let mut bass_audio = if automation.is_empty() {
                let mut audio =
                    crate::dsp::render_monophonic_pattern_bass_chunk_with_filter_control(
                        &bass,
                        &drums,
                        meldritch_core::TrackId::new(3),
                        tempo,
                        chunk.range,
                        seed,
                        settings,
                        bass_settings,
                    );
                if sidechain.is_none() {
                    crate::dsp::apply_pattern_ducking(
                        &mut audio,
                        &drums,
                        meldritch_core::TrackId::new(1),
                        tempo,
                        chunk.range,
                        seed,
                        bass_settings.ducking_amount,
                        bass_settings.ducking_release_seconds,
                    );
                }
                audio
            } else {
                crate::dsp::render_monophonic_pattern_bass_with_automation(
                    &bass,
                    sidechain
                        .is_none()
                        .then_some((&drums, meldritch_core::TrackId::new(1))),
                    tempo,
                    chunk.range,
                    seed,
                    settings,
                    bass_settings,
                    &automation,
                )
            };
            if let Some(relation) = sidechain {
                let mut control = drums.clone();
                for track in control.active_step_counts_by_track().into_keys() {
                    if track != relation.control_track {
                        for step in 0..control.length_steps() {
                            let _ = control.clear_step(track, meldritch_core::StepIndex::new(step));
                        }
                    }
                }
                let control_audio = render_pattern_samples_chunk(
                    &control,
                    tempo,
                    chunk.range,
                    seed,
                    settings,
                    &samples,
                );
                bass_audio = crate::dynamics::apply_role_sidechain(
                    &bass_audio,
                    &control_audio,
                    tempo.sample_rate(),
                    relation.source_role,
                    relation.target_role,
                    &meldritch_core::RolePriorityTable::default(),
                    relation.settings,
                )
                .0;
            }
            for (output, bass_sample) in mixed.samples_mut().iter_mut().zip(bass_audio.samples()) {
                *output += bass_sample;
            }
            if let (Some(chord), Some(chord_pattern)) = (chord_layer, chord_pattern) {
                let chord_audio = if automation.is_empty() {
                    crate::dsp::render_polyphonic_pattern_chunk(
                        &chord_pattern,
                        tempo,
                        chunk.range,
                        seed,
                        settings,
                        chord.settings,
                        chord.voice_count,
                    )
                } else {
                    crate::dsp::render_polyphonic_pattern_with_automation(
                        &chord_pattern,
                        tempo,
                        chunk.range,
                        seed,
                        settings,
                        chord.settings,
                        chord.voice_count,
                        &automation,
                    )
                }
                .expect("validated chord voice count");
                for (output, chord_sample) in
                    mixed.samples_mut().iter_mut().zip(chord_audio.samples())
                {
                    *output += chord_sample;
                }
            }
            mixed
        }) {
            submission.submitted_jobs += 1;
        }
    }
    submission
}

#[cfg(test)]
mod tests {
    use super::*;
    use meldritch_audio::realtime_status::realtime_status;
    use meldritch_core::{EventTag, PatternId, Step, StepIndex, TrackId};

    #[test]
    fn coordinator_renders_horizon_and_reports_combined_progress() {
        let config = RenderCoordinatorConfig::new(2, 4, 2, 1, Duration::from_millis(2)).unwrap();
        let pattern = Pattern::new(PatternId::new(1), 4, 4).unwrap();
        let (mut status_publisher, status_monitor, _) = realtime_status();
        let mut transport = meldritch_audio::transport::PlaybackTransport::new(0, 4, 2).unwrap();
        transport.play();
        assert_eq!(transport.next_frame(), Some(0));
        status_publisher.publish_transport(&transport);
        let mut coordinator = RenderCoordinator::new(
            config,
            pattern,
            Tempo::new(120.0, 48_000).unwrap(),
            ProbabilitySeed::new(1),
            RenderSettings::new(1).unwrap(),
            Arc::new(BTreeMap::new()),
            status_monitor,
        )
        .unwrap();

        assert!(coordinator.wait_for_ready_chunks(2, Duration::from_secs(1)));
        let refreshes = coordinator.diagnostics().refreshes;
        assert_eq!(
            coordinator
                .invalidate_range(FrameRange::new(0, 1).unwrap())
                .unwrap(),
            1
        );
        assert_eq!(transport.next_frame(), Some(1));
        status_publisher.publish_transport(&transport);
        coordinator.wake();
        assert!(coordinator.wait_for_refreshes(refreshes + 1, Duration::from_secs(1)));
        assert!(coordinator.wait_for_ready_chunks(2, Duration::from_secs(1)));
        assert!(coordinator.audio_reader().snapshot().frame(0).is_ok());
        assert!(coordinator.audio_reader().snapshot().frame(3).is_ok());
        let refreshes_after_invalidation = coordinator.diagnostics().refreshes;
        let mut updated_pattern = Pattern::new(PatternId::new(1), 4, 4).unwrap();
        updated_pattern
            .set_step(TrackId::new(1), StepIndex::new(0), Step::new(36))
            .unwrap();
        let mut updated_samples = BTreeMap::new();
        updated_samples.insert(36, SampleBuffer::new(1, 48_000, vec![0.8]));
        let updated_state = SampleRenderState::new(
            updated_pattern,
            Tempo::new(120.0, 48_000).unwrap(),
            ProbabilitySeed::new(1),
            RenderSettings::new(1).unwrap(),
            Arc::new(updated_samples),
        );
        assert_eq!(
            coordinator
                .update_render_state(updated_state, FrameRange::new(0, 1).unwrap())
                .unwrap(),
            1
        );
        assert!(
            coordinator
                .wait_for_refreshes(refreshes_after_invalidation + 1, Duration::from_secs(1))
        );
        assert!(coordinator.wait_for_ready_chunks(2, Duration::from_secs(1)));
        assert_eq!(
            coordinator.audio_reader().snapshot().frame(0),
            Ok([0.8].as_slice())
        );
        let diagnostics = coordinator.diagnostics();
        assert!(diagnostics.refreshes > refreshes);
        assert_eq!(diagnostics.playhead, 2);
        assert_eq!(diagnostics.publication.ready_chunks, 2);
        coordinator.shutdown();
    }

    #[test]
    fn coordinator_configuration_rejects_zero_values() {
        assert_eq!(
            RenderCoordinatorConfig::new(0, 4, 2, 1, Duration::from_millis(1)),
            Err(RenderCoordinatorError::ZeroWorkers)
        );
        assert_eq!(
            RenderCoordinatorConfig::new(1, 4, 2, 1, Duration::ZERO),
            Err(RenderCoordinatorError::ZeroPollInterval)
        );
    }

    #[test]
    fn coordinator_publishes_mixed_drum_and_monophonic_bass_chunks() {
        let config =
            RenderCoordinatorConfig::new(2, 12_000, 6_000, 1, Duration::from_millis(2)).unwrap();
        let mut pattern = Pattern::new(PatternId::new(1), 4, 4).unwrap();
        pattern
            .set_step(TrackId::new(1), StepIndex::new(0), Step::new(36))
            .unwrap();
        pattern
            .set_step(TrackId::new(4), StepIndex::new(0), Step::new(24))
            .unwrap();
        let mut samples = BTreeMap::new();
        samples.insert(36, SampleBuffer::new(1, 48_000, vec![0.2; 12_000]));
        samples.insert(24, SampleBuffer::new(1, 48_000, vec![0.9; 12_000]));
        let state = SampleRenderState::new(
            pattern,
            Tempo::new(120.0, 48_000).unwrap(),
            ProbabilitySeed::new(1),
            RenderSettings::new(1).unwrap(),
            Arc::new(samples),
        )
        .with_bass_layer(TrackId::new(4), BassVoiceSettings::default());
        let (_, status, _) = realtime_status();
        let mut coordinator = RenderCoordinator::new_from_state(config, state, status).unwrap();

        assert!(coordinator.wait_for_ready_chunks(2, Duration::from_secs(1)));
        let snapshot = coordinator.audio_reader().snapshot();
        assert_eq!(snapshot.frame(0), Ok([0.2].as_slice()));
        assert_ne!(snapshot.frame(500), Ok([0.2].as_slice()));
        assert_eq!(coordinator.diagnostics().publication.ready_chunks, 2);
        coordinator.shutdown();
    }

    #[test]
    fn coordinator_publishes_tagged_effect_tails_without_a_synth_layer() {
        let config =
            RenderCoordinatorConfig::new(2, 24_000, 6_000, 3, Duration::from_millis(2)).unwrap();
        let mut pattern = Pattern::new(PatternId::new(1), 4, 4).unwrap();
        pattern
            .set_step(
                TrackId::new(1),
                StepIndex::new(0),
                Step::new(36).with_tag(EventTag::Accent),
            )
            .unwrap();
        let mut samples = BTreeMap::new();
        samples.insert(36, SampleBuffer::new(1, 48_000, vec![1.0; 4]));
        let state = SampleRenderState::new(
            pattern,
            Tempo::new(120.0, 48_000).unwrap(),
            ProbabilitySeed::new(1),
            RenderSettings::new(1).unwrap(),
            Arc::new(samples),
        )
        .with_effect_rules(vec![crate::effects::EffectSendRule {
            bus: crate::effects::EffectBus::Delay,
            required_tag: EventTag::Accent,
            send_gain: 0.5,
        }]);
        let (_, status, _) = realtime_status();
        let mut coordinator = RenderCoordinator::new_from_state(config, state, status).unwrap();

        assert!(coordinator.wait_for_ready_chunks(4, Duration::from_secs(1)));
        assert!(coordinator.audio_reader().snapshot().frame(18_000).unwrap()[0] > 0.0);
        coordinator.shutdown();
    }
}
