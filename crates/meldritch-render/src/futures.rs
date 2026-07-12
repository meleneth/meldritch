//! Deterministic planning for likely performance futures.

use crate::{ChunkPublicationError, Fingerprint, FingerprintBuilder, RealtimeChunkPublication};
use meldritch_audio::AudioBlock;
use meldritch_core::{Frame, FrameRange, PatternId, SampleRate, SceneId, Tempo, TrackId};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PerformanceGesture {
    QueueScene(SceneId),
    MuteTrack(TrackId),
    UnmuteTrack(TrackId),
    TriggerFill(PatternId),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FutureEvidence {
    pub gesture: PerformanceGesture,
    pub recency: u32,
    pub selected: bool,
    pub queued: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlannedFuture {
    pub gesture: PerformanceGesture,
    pub score: u32,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PerformanceFuturePlan {
    pub candidates: Vec<PlannedFuture>,
}

/// The complete semantic recipe needed to render one likely performance state.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum FutureRenderVariant {
    Scene {
        scene: SceneId,
        pattern: PatternId,
        muted_tracks: Vec<TrackId>,
    },
    TrackMix {
        pattern: PatternId,
        muted_tracks: Vec<TrackId>,
    },
    Fill {
        pattern: PatternId,
        fill: PatternId,
        muted_tracks: Vec<TrackId>,
    },
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FutureArtifactKey {
    range: FrameRange,
    sample_rate: SampleRate,
    fingerprint: Fingerprint,
}

impl FutureArtifactKey {
    #[must_use]
    pub const fn range(self) -> FrameRange {
        self.range
    }

    #[must_use]
    pub const fn sample_rate(self) -> SampleRate {
        self.sample_rate
    }

    #[must_use]
    pub const fn fingerprint(self) -> Fingerprint {
        self.fingerprint
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RenderableFuture {
    pub gesture: PerformanceGesture,
    pub score: u32,
    pub variant: FutureRenderVariant,
    pub key: FutureArtifactKey,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FuturePerformanceState {
    pub active_pattern: PatternId,
    pub scene_patterns: BTreeMap<SceneId, PatternId>,
    pub muted_tracks: BTreeSet<TrackId>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RenderableFuturePlan {
    pub candidates: Vec<RenderableFuture>,
    pub unresolved: Vec<PerformanceGesture>,
}

#[must_use]
pub fn plan_performance_futures(
    evidence: &[FutureEvidence],
    capacity: usize,
) -> PerformanceFuturePlan {
    let mut scores = BTreeMap::<PerformanceGesture, u32>::new();
    for item in evidence {
        let recency_score = 1_000u32.saturating_sub(item.recency.min(1_000));
        let score = recency_score
            .saturating_add(if item.selected { 2_000 } else { 0 })
            .saturating_add(if item.queued { 4_000 } else { 0 });
        scores
            .entry(item.gesture)
            .and_modify(|current| *current = (*current).max(score))
            .or_insert(score);
    }
    let mut candidates = scores
        .into_iter()
        .map(|(gesture, score)| PlannedFuture { gesture, score })
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| {
        right
            .score
            .cmp(&left.score)
            .then_with(|| left.gesture.cmp(&right.gesture))
    });
    candidates.truncate(capacity);
    PerformanceFuturePlan { candidates }
}

/// Resolves scored gestures into render recipes and content-addressed keys.
///
/// `pattern_fingerprints` must describe the current render inputs for every
/// pattern used by a recipe. Missing scene mappings or fingerprints are kept in
/// `unresolved` instead of silently producing an unusable artifact.
#[must_use]
pub fn resolve_renderable_futures(
    plan: &PerformanceFuturePlan,
    state: &FuturePerformanceState,
    range: FrameRange,
    sample_rate: SampleRate,
    pattern_fingerprints: &BTreeMap<PatternId, Fingerprint>,
) -> RenderableFuturePlan {
    let mut resolved = RenderableFuturePlan::default();
    for candidate in &plan.candidates {
        let Some(variant) = resolve_variant(candidate.gesture, state) else {
            resolved.unresolved.push(candidate.gesture);
            continue;
        };
        let Some(key) = future_artifact_key(&variant, range, sample_rate, pattern_fingerprints)
        else {
            resolved.unresolved.push(candidate.gesture);
            continue;
        };
        resolved.candidates.push(RenderableFuture {
            gesture: candidate.gesture,
            score: candidate.score,
            variant,
            key,
        });
    }
    resolved
}

fn resolve_variant(
    gesture: PerformanceGesture,
    state: &FuturePerformanceState,
) -> Option<FutureRenderVariant> {
    let mut muted_tracks = state.muted_tracks.clone();
    match gesture {
        PerformanceGesture::QueueScene(scene) => {
            let pattern = *state.scene_patterns.get(&scene)?;
            Some(FutureRenderVariant::Scene {
                scene,
                pattern,
                muted_tracks: muted_tracks.into_iter().collect(),
            })
        }
        PerformanceGesture::MuteTrack(track) => {
            muted_tracks.insert(track);
            Some(FutureRenderVariant::TrackMix {
                pattern: state.active_pattern,
                muted_tracks: muted_tracks.into_iter().collect(),
            })
        }
        PerformanceGesture::UnmuteTrack(track) => {
            muted_tracks.remove(&track);
            Some(FutureRenderVariant::TrackMix {
                pattern: state.active_pattern,
                muted_tracks: muted_tracks.into_iter().collect(),
            })
        }
        PerformanceGesture::TriggerFill(fill) => Some(FutureRenderVariant::Fill {
            pattern: state.active_pattern,
            fill,
            muted_tracks: muted_tracks.into_iter().collect(),
        }),
    }
}

fn future_artifact_key(
    variant: &FutureRenderVariant,
    range: FrameRange,
    sample_rate: SampleRate,
    pattern_fingerprints: &BTreeMap<PatternId, Fingerprint>,
) -> Option<FutureArtifactKey> {
    let mut fingerprint = FingerprintBuilder::new();
    fingerprint.write_u64(0x6675_7475_7265_7631);
    fingerprint.write_u64(range.start());
    fingerprint.write_u64(range.end());
    fingerprint.write_u64(u64::from(sample_rate));
    match variant {
        FutureRenderVariant::Scene {
            scene,
            pattern,
            muted_tracks,
        } => {
            fingerprint.write_u64(1);
            fingerprint.write_u64(scene.raw());
            write_pattern(&mut fingerprint, *pattern, pattern_fingerprints)?;
            write_muted_tracks(&mut fingerprint, muted_tracks);
        }
        FutureRenderVariant::TrackMix {
            pattern,
            muted_tracks,
        } => {
            fingerprint.write_u64(2);
            write_pattern(&mut fingerprint, *pattern, pattern_fingerprints)?;
            write_muted_tracks(&mut fingerprint, muted_tracks);
        }
        FutureRenderVariant::Fill {
            pattern,
            fill,
            muted_tracks,
        } => {
            fingerprint.write_u64(3);
            write_pattern(&mut fingerprint, *pattern, pattern_fingerprints)?;
            write_pattern(&mut fingerprint, *fill, pattern_fingerprints)?;
            write_muted_tracks(&mut fingerprint, muted_tracks);
        }
    }
    Some(FutureArtifactKey {
        range,
        sample_rate,
        fingerprint: fingerprint.finish(),
    })
}

fn write_pattern(
    fingerprint: &mut FingerprintBuilder,
    pattern: PatternId,
    pattern_fingerprints: &BTreeMap<PatternId, Fingerprint>,
) -> Option<()> {
    fingerprint.write_u64(pattern.raw());
    fingerprint.write_u64(pattern_fingerprints.get(&pattern)?.raw());
    Some(())
}

fn write_muted_tracks(fingerprint: &mut FingerprintBuilder, muted_tracks: &[TrackId]) {
    fingerprint.write_u64(muted_tracks.len() as u64);
    for track in muted_tracks {
        fingerprint.write_u64(track.raw());
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct FutureSubmission {
    pub desired: usize,
    pub cache_hits: usize,
    pub submitted: usize,
    pub unresolved: usize,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct FutureWorkerDiagnostics {
    pub desired_artifacts: usize,
    pub clean_artifacts: usize,
    pub queued_jobs: usize,
    pub active_jobs: usize,
    pub completed_jobs: usize,
    pub sleeping: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FutureCandidateStatus {
    Clean,
    Queued,
    Rendering,
    Missing,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FutureCandidateInspection {
    pub gesture: PerformanceGesture,
    pub score: u32,
    pub key: FutureArtifactKey,
    pub status: FutureCandidateStatus,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FutureWorkerPoolError {
    ZeroWorkers,
}

type FutureRenderTask = Box<dyn FnOnce() -> AudioBlock + Send + 'static>;

struct FutureRenderJob {
    key: FutureArtifactKey,
    score: u32,
    sequence: u64,
    task: FutureRenderTask,
}

#[derive(Default)]
struct FutureWorkerState {
    jobs: Vec<FutureRenderJob>,
    wanted: BTreeSet<FutureArtifactKey>,
    desired: BTreeSet<FutureArtifactKey>,
    next_sequence: u64,
    active: BTreeSet<FutureArtifactKey>,
    completed_jobs: usize,
    shutdown: bool,
}

#[derive(Default)]
struct FutureWorkerShared {
    state: Mutex<FutureWorkerState>,
    cache: Mutex<BTreeMap<FutureArtifactKey, AudioBlock>>,
    has_work: Condvar,
    idle: Condvar,
}

/// Background cache for speculative performance variants.
pub struct FutureWorkerPool {
    shared: Arc<FutureWorkerShared>,
    workers: Vec<JoinHandle<()>>,
}

impl FutureWorkerPool {
    pub fn new(worker_count: usize) -> Result<Self, FutureWorkerPoolError> {
        if worker_count == 0 {
            return Err(FutureWorkerPoolError::ZeroWorkers);
        }
        let shared = Arc::new(FutureWorkerShared::default());
        let workers = (0..worker_count)
            .map(|_| spawn_future_worker(Arc::clone(&shared)))
            .collect();
        Ok(Self { shared, workers })
    }

    /// Replaces the desired future set and queues only artifacts not already
    /// cached or in flight. Higher-scored candidates are consumed first.
    pub fn submit_plan<F>(&self, plan: &RenderableFuturePlan, render: F) -> FutureSubmission
    where
        F: Fn(&RenderableFuture) -> AudioBlock + Send + Sync + 'static,
    {
        let render = Arc::new(render);
        let mut state = self
            .shared
            .state
            .lock()
            .expect("future worker state lock poisoned");
        let cache = self
            .shared
            .cache
            .lock()
            .expect("future artifact cache lock poisoned");
        state.desired = plan
            .candidates
            .iter()
            .map(|candidate| candidate.key)
            .collect();
        let mut submission = FutureSubmission {
            desired: state.desired.len(),
            unresolved: plan.unresolved.len(),
            ..FutureSubmission::default()
        };
        for candidate in &plan.candidates {
            if cache.contains_key(&candidate.key) {
                submission.cache_hits += 1;
                continue;
            }
            if !state.wanted.insert(candidate.key) {
                continue;
            }
            let candidate = candidate.clone();
            let renderer = Arc::clone(&render);
            let sequence = state.next_sequence;
            state.next_sequence = state.next_sequence.wrapping_add(1);
            state.jobs.push(FutureRenderJob {
                key: candidate.key,
                score: candidate.score,
                sequence,
                task: Box::new(move || renderer(&candidate)),
            });
            submission.submitted += 1;
        }
        state.jobs.sort_by(|left, right| {
            right
                .score
                .cmp(&left.score)
                .then_with(|| left.sequence.cmp(&right.sequence))
        });
        drop(state);
        drop(cache);
        if submission.submitted != 0 {
            self.shared.has_work.notify_all();
        }
        submission
    }

    pub fn wait_until_idle(&self) {
        let mut state = self
            .shared
            .state
            .lock()
            .expect("future worker state lock poisoned");
        while !state.active.is_empty() || !state.jobs.is_empty() {
            state = self
                .shared
                .idle
                .wait(state)
                .expect("future worker state lock poisoned");
        }
    }

    #[must_use]
    pub fn cached_artifact(&self, key: FutureArtifactKey) -> Option<AudioBlock> {
        self.shared
            .cache
            .lock()
            .expect("future artifact cache lock poisoned")
            .get(&key)
            .cloned()
    }

    #[must_use]
    pub fn diagnostics(&self) -> FutureWorkerDiagnostics {
        let state = self
            .shared
            .state
            .lock()
            .expect("future worker state lock poisoned");
        let cache = self
            .shared
            .cache
            .lock()
            .expect("future artifact cache lock poisoned");
        let clean_artifacts = state
            .desired
            .iter()
            .filter(|key| cache.contains_key(key))
            .count();
        FutureWorkerDiagnostics {
            desired_artifacts: state.desired.len(),
            clean_artifacts,
            queued_jobs: state.jobs.len(),
            active_jobs: state.active.len(),
            completed_jobs: state.completed_jobs,
            sleeping: state.jobs.is_empty()
                && state.active.is_empty()
                && clean_artifacts == state.desired.len(),
        }
    }

    #[must_use]
    pub fn inspect_plan(&self, plan: &RenderableFuturePlan) -> Vec<FutureCandidateInspection> {
        let state = self
            .shared
            .state
            .lock()
            .expect("future worker state lock poisoned");
        let cache = self
            .shared
            .cache
            .lock()
            .expect("future artifact cache lock poisoned");
        plan.candidates
            .iter()
            .map(|candidate| {
                let status = if cache.contains_key(&candidate.key) {
                    FutureCandidateStatus::Clean
                } else if state.active.contains(&candidate.key) {
                    FutureCandidateStatus::Rendering
                } else if state.wanted.contains(&candidate.key) {
                    FutureCandidateStatus::Queued
                } else {
                    FutureCandidateStatus::Missing
                };
                FutureCandidateInspection {
                    gesture: candidate.gesture,
                    score: candidate.score,
                    key: candidate.key,
                    status,
                }
            })
            .collect()
    }
}

impl Drop for FutureWorkerPool {
    fn drop(&mut self) {
        {
            let mut state = self
                .shared
                .state
                .lock()
                .expect("future worker state lock poisoned");
            state.shutdown = true;
            self.shared.has_work.notify_all();
        }
        for worker in self.workers.drain(..) {
            worker.join().expect("future render worker panicked");
        }
    }
}

fn spawn_future_worker(shared: Arc<FutureWorkerShared>) -> JoinHandle<()> {
    thread::spawn(move || {
        loop {
            let job = {
                let mut state = shared
                    .state
                    .lock()
                    .expect("future worker state lock poisoned");
                loop {
                    if !state.jobs.is_empty() {
                        let job = state.jobs.remove(0);
                        state.active.insert(job.key);
                        break job;
                    }
                    if state.shutdown {
                        return;
                    }
                    state = shared
                        .has_work
                        .wait(state)
                        .expect("future worker state lock poisoned");
                }
            };
            let block = (job.task)();
            shared
                .cache
                .lock()
                .expect("future artifact cache lock poisoned")
                .insert(job.key, block);
            let mut state = shared
                .state
                .lock()
                .expect("future worker state lock poisoned");
            state.wanted.remove(&job.key);
            state.active.remove(&job.key);
            state.completed_jobs += 1;
            if state.active.is_empty() && state.jobs.is_empty() {
                shared.idle.notify_all();
            }
        }
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LaunchQuantization {
    Beat,
    Bar { beats: u32 },
}

impl LaunchQuantization {
    #[must_use]
    pub fn next_frame(self, playhead: Frame, tempo: Tempo) -> Frame {
        let beats = match self {
            Self::Beat => 1,
            Self::Bar { beats } => beats.max(1),
        };
        let boundary = (tempo.frames_per_beat() * f64::from(beats)).round() as Frame;
        playhead.div_ceil(boundary.max(1)) * boundary.max(1)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct QueuedPerformanceGesture {
    pub gesture: PerformanceGesture,
    pub launch_frame: Frame,
    pub fill_end_frame: Option<Frame>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ActivePerformanceState {
    pub scene: Option<SceneId>,
    pub muted_tracks: BTreeSet<TrackId>,
    pub fill: Option<PatternId>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PerformanceLaunchSource {
    Speculative(FutureArtifactKey),
    LiveFallback,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PerformanceLaunch {
    pub gesture: PerformanceGesture,
    pub frame: Frame,
    pub source: PerformanceLaunchSource,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PerformanceLauncherDiagnostics {
    pub queued_gestures: u64,
    pub cancelled_gestures: u64,
    pub speculative_launches: u64,
    pub fallback_launches: u64,
    pub fill_returns: u64,
    pub last_launch: Option<PerformanceLaunch>,
}

/// Quantized performance gesture executor. It owns no audio device state: the
/// caller atomically publishes the cached artifact identified by a successful
/// speculative launch, or continues through its live renderer on fallback.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PerformanceLauncher {
    quantization: LaunchQuantization,
    fill_beats: u32,
    queued: Option<QueuedPerformanceGesture>,
    active: ActivePerformanceState,
    fill_end_frame: Option<Frame>,
    diagnostics: PerformanceLauncherDiagnostics,
}

impl PerformanceLauncher {
    #[must_use]
    pub fn new(quantization: LaunchQuantization) -> Self {
        Self {
            quantization,
            fill_beats: 4,
            queued: None,
            active: ActivePerformanceState::default(),
            fill_end_frame: None,
            diagnostics: PerformanceLauncherDiagnostics::default(),
        }
    }

    #[must_use]
    pub fn with_fill_beats(mut self, fill_beats: u32) -> Self {
        self.fill_beats = fill_beats.max(1);
        self
    }

    pub fn queue(
        &mut self,
        gesture: PerformanceGesture,
        playhead: Frame,
        tempo: Tempo,
    ) -> QueuedPerformanceGesture {
        let launch_frame = self.quantization.next_frame(playhead, tempo);
        let fill_end_frame = matches!(gesture, PerformanceGesture::TriggerFill(_)).then(|| {
            let duration = (tempo.frames_per_beat() * f64::from(self.fill_beats)).round() as Frame;
            launch_frame.saturating_add(duration.max(1))
        });
        let queued = QueuedPerformanceGesture {
            gesture,
            launch_frame,
            fill_end_frame,
        };
        self.queued = Some(queued);
        self.diagnostics.queued_gestures += 1;
        queued
    }

    pub fn cancel(&mut self) -> Option<QueuedPerformanceGesture> {
        let queued = self.queued.take();
        if queued.is_some() {
            self.diagnostics.cancelled_gestures += 1;
        }
        queued
    }

    #[must_use]
    pub const fn queued(&self) -> Option<QueuedPerformanceGesture> {
        self.queued
    }

    #[must_use]
    pub const fn active(&self) -> &ActivePerformanceState {
        &self.active
    }

    pub fn clear_fill(&mut self) {
        self.active.fill = None;
        self.fill_end_frame = None;
    }

    #[must_use]
    pub const fn fill_end_frame(&self) -> Option<Frame> {
        self.fill_end_frame
    }

    #[must_use]
    pub const fn diagnostics(&self) -> PerformanceLauncherDiagnostics {
        self.diagnostics
    }

    pub fn advance(
        &mut self,
        playhead: Frame,
        plan: &RenderableFuturePlan,
        pool: &FutureWorkerPool,
    ) -> Option<PerformanceLaunch> {
        self.expire_fill(playhead);
        let queued = self
            .queued
            .filter(|queued| playhead >= queued.launch_frame)?;
        self.queued = None;
        apply_gesture(&mut self.active, queued.gesture);
        match queued.gesture {
            PerformanceGesture::TriggerFill(_) => self.fill_end_frame = queued.fill_end_frame,
            PerformanceGesture::QueueScene(_) => self.fill_end_frame = None,
            PerformanceGesture::MuteTrack(_) | PerformanceGesture::UnmuteTrack(_) => {}
        }
        let speculative = plan
            .candidates
            .iter()
            .find(|candidate| candidate.gesture == queued.gesture)
            .filter(|candidate| pool.cached_artifact(candidate.key).is_some())
            .map(|candidate| candidate.key);
        let launch = PerformanceLaunch {
            gesture: queued.gesture,
            frame: queued.launch_frame,
            source: speculative.map_or(
                PerformanceLaunchSource::LiveFallback,
                PerformanceLaunchSource::Speculative,
            ),
        };
        match launch.source {
            PerformanceLaunchSource::Speculative(_) => self.diagnostics.speculative_launches += 1,
            PerformanceLaunchSource::LiveFallback => self.diagnostics.fallback_launches += 1,
        }
        self.diagnostics.last_launch = Some(launch);
        Some(launch)
    }

    /// Executes a due launch and atomically selects its audio source.
    ///
    /// Prepared futures replace the published immutable snapshot in one swap.
    /// A live fallback restores the coordinator-owned chunk snapshot, which is
    /// also a single publication swap.
    pub fn advance_and_publish(
        &mut self,
        playhead: Frame,
        plan: &RenderableFuturePlan,
        pool: &FutureWorkerPool,
        publication: &RealtimeChunkPublication,
    ) -> Result<Option<PerformanceLaunch>, ChunkPublicationError> {
        let fill_expired = self.expire_fill(playhead);
        if fill_expired {
            publication.restore_live_snapshot()?;
        }
        let Some(launch) = self.advance(playhead, plan, pool) else {
            return Ok(None);
        };
        match launch.source {
            PerformanceLaunchSource::Speculative(key) => {
                let block = pool
                    .cached_artifact(key)
                    .expect("selected speculative artifact remains cached");
                publication.publish_override(&block)?;
            }
            PerformanceLaunchSource::LiveFallback => publication.restore_live_snapshot()?,
        }
        Ok(Some(launch))
    }

    fn expire_fill(&mut self, playhead: Frame) -> bool {
        if self
            .fill_end_frame
            .is_some_and(|end_frame| playhead >= end_frame)
        {
            self.clear_fill();
            self.diagnostics.fill_returns += 1;
            true
        } else {
            false
        }
    }
}

fn apply_gesture(state: &mut ActivePerformanceState, gesture: PerformanceGesture) {
    match gesture {
        PerformanceGesture::QueueScene(scene) => {
            state.scene = Some(scene);
            state.fill = None;
        }
        PerformanceGesture::MuteTrack(track) => {
            state.muted_tracks.insert(track);
        }
        PerformanceGesture::UnmuteTrack(track) => {
            state.muted_tracks.remove(&track);
        }
        PerformanceGesture::TriggerFill(pattern) => {
            state.fill = Some(pattern);
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct GestureHistory {
    sequence: u64,
    last_seen: BTreeMap<PerformanceGesture, u64>,
}

impl GestureHistory {
    pub fn record(&mut self, gesture: PerformanceGesture) {
        self.last_seen.insert(gesture, self.sequence);
        self.sequence = self.sequence.wrapping_add(1);
    }

    #[must_use]
    pub fn evidence(
        &self,
        selected: Option<PerformanceGesture>,
        queued: Option<PerformanceGesture>,
    ) -> Vec<FutureEvidence> {
        self.last_seen
            .iter()
            .map(|(gesture, sequence)| FutureEvidence {
                gesture: *gesture,
                recency: self
                    .sequence
                    .saturating_sub(*sequence)
                    .min(u64::from(u32::MAX)) as u32,
                selected: selected == Some(*gesture),
                queued: queued == Some(*gesture),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queued_then_selected_then_recent_candidates_are_prioritized() {
        let mute = PerformanceGesture::MuteTrack(TrackId::new(2));
        let fill = PerformanceGesture::TriggerFill(PatternId::new(9));
        let scene = PerformanceGesture::QueueScene(SceneId::new(3));
        let plan = plan_performance_futures(
            &[
                FutureEvidence {
                    gesture: mute,
                    recency: 0,
                    selected: false,
                    queued: false,
                },
                FutureEvidence {
                    gesture: fill,
                    recency: 20,
                    selected: true,
                    queued: false,
                },
                FutureEvidence {
                    gesture: scene,
                    recency: 100,
                    selected: false,
                    queued: true,
                },
            ],
            3,
        );
        assert_eq!(
            plan.candidates
                .iter()
                .map(|candidate| candidate.gesture)
                .collect::<Vec<_>>(),
            vec![scene, fill, mute]
        );
    }

    #[test]
    fn duplicate_gestures_are_deduplicated_at_the_best_score_and_capped() {
        let gesture = PerformanceGesture::MuteTrack(TrackId::new(1));
        let plan = plan_performance_futures(
            &[
                FutureEvidence {
                    gesture,
                    recency: 100,
                    selected: false,
                    queued: false,
                },
                FutureEvidence {
                    gesture,
                    recency: 0,
                    selected: true,
                    queued: false,
                },
            ],
            1,
        );
        assert_eq!(plan.candidates.len(), 1);
        assert_eq!(plan.candidates[0].score, 3_000);
        assert!(plan_performance_futures(&[], 0).candidates.is_empty());
    }

    #[test]
    fn gesture_history_produces_stable_recency_evidence() {
        let first = PerformanceGesture::QueueScene(SceneId::new(1));
        let second = PerformanceGesture::TriggerFill(PatternId::new(2));
        let mut history = GestureHistory::default();
        history.record(first);
        history.record(second);
        let evidence = history.evidence(Some(first), Some(second));
        assert_eq!(evidence[0].gesture, first);
        assert_eq!(evidence[0].recency, 2);
        assert!(evidence[0].selected);
        assert!(evidence[1].queued);
    }

    #[test]
    fn gestures_resolve_to_complete_scene_mute_and_fill_recipes() {
        let active = PatternId::new(10);
        let scene_pattern = PatternId::new(20);
        let fill = PatternId::new(30);
        let muted = TrackId::new(4);
        let state = FuturePerformanceState {
            active_pattern: active,
            scene_patterns: [(SceneId::new(2), scene_pattern)].into_iter().collect(),
            muted_tracks: [muted].into_iter().collect(),
        };
        let plan = PerformanceFuturePlan {
            candidates: vec![
                PlannedFuture {
                    gesture: PerformanceGesture::QueueScene(SceneId::new(2)),
                    score: 3,
                },
                PlannedFuture {
                    gesture: PerformanceGesture::UnmuteTrack(muted),
                    score: 2,
                },
                PlannedFuture {
                    gesture: PerformanceGesture::TriggerFill(fill),
                    score: 1,
                },
            ],
        };
        let fingerprints = [
            (active, Fingerprint::new(100)),
            (scene_pattern, Fingerprint::new(200)),
            (fill, Fingerprint::new(300)),
        ]
        .into_iter()
        .collect();
        let resolved = resolve_renderable_futures(
            &plan,
            &state,
            FrameRange::new(0, 48_000).unwrap(),
            48_000,
            &fingerprints,
        );

        assert!(resolved.unresolved.is_empty());
        assert_eq!(resolved.candidates.len(), 3);
        assert_eq!(
            resolved.candidates[0].variant,
            FutureRenderVariant::Scene {
                scene: SceneId::new(2),
                pattern: scene_pattern,
                muted_tracks: vec![muted],
            }
        );
        assert_eq!(
            resolved.candidates[1].variant,
            FutureRenderVariant::TrackMix {
                pattern: active,
                muted_tracks: Vec::new(),
            }
        );
        assert_eq!(
            resolved.candidates[2].variant,
            FutureRenderVariant::Fill {
                pattern: active,
                fill,
                muted_tracks: vec![muted],
            }
        );
    }

    #[test]
    fn future_keys_are_stable_and_track_every_render_input() {
        let pattern = PatternId::new(10);
        let fill = PatternId::new(30);
        let range = FrameRange::new(100, 200).unwrap();
        let state = FuturePerformanceState {
            active_pattern: pattern,
            scene_patterns: BTreeMap::new(),
            muted_tracks: [TrackId::new(2), TrackId::new(1)].into_iter().collect(),
        };
        let plan = PerformanceFuturePlan {
            candidates: vec![PlannedFuture {
                gesture: PerformanceGesture::TriggerFill(fill),
                score: 1,
            }],
        };
        let fingerprints = [
            (pattern, Fingerprint::new(11)),
            (fill, Fingerprint::new(22)),
        ]
        .into_iter()
        .collect::<BTreeMap<_, _>>();
        let key = resolve_renderable_futures(&plan, &state, range, 48_000, &fingerprints)
            .candidates[0]
            .key;
        let same = resolve_renderable_futures(&plan, &state, range, 48_000, &fingerprints)
            .candidates[0]
            .key;
        assert_eq!(key, same);
        assert_eq!(key.range(), range);
        assert_eq!(key.sample_rate(), 48_000);

        let mut changed = fingerprints;
        changed.insert(fill, Fingerprint::new(23));
        let changed_key =
            resolve_renderable_futures(&plan, &state, range, 48_000, &changed).candidates[0].key;
        assert_ne!(key.fingerprint(), changed_key.fingerprint());
    }

    #[test]
    fn unresolved_scene_mappings_and_pattern_content_are_reported() {
        let missing_scene = PerformanceGesture::QueueScene(SceneId::new(99));
        let missing_fill = PerformanceGesture::TriggerFill(PatternId::new(30));
        let plan = PerformanceFuturePlan {
            candidates: vec![
                PlannedFuture {
                    gesture: missing_scene,
                    score: 2,
                },
                PlannedFuture {
                    gesture: missing_fill,
                    score: 1,
                },
            ],
        };
        let state = FuturePerformanceState {
            active_pattern: PatternId::new(10),
            scene_patterns: BTreeMap::new(),
            muted_tracks: BTreeSet::new(),
        };
        let resolved = resolve_renderable_futures(
            &plan,
            &state,
            FrameRange::new(0, 1).unwrap(),
            48_000,
            &BTreeMap::new(),
        );
        assert!(resolved.candidates.is_empty());
        assert_eq!(resolved.unresolved, vec![missing_scene, missing_fill]);
    }

    #[test]
    fn future_workers_render_in_score_order_then_sleep_on_a_clean_plan() {
        let pattern = PatternId::new(1);
        let state = FuturePerformanceState {
            active_pattern: pattern,
            scene_patterns: BTreeMap::new(),
            muted_tracks: BTreeSet::new(),
        };
        let plan = PerformanceFuturePlan {
            candidates: vec![
                PlannedFuture {
                    gesture: PerformanceGesture::MuteTrack(TrackId::new(1)),
                    score: 10,
                },
                PlannedFuture {
                    gesture: PerformanceGesture::MuteTrack(TrackId::new(2)),
                    score: 30,
                },
                PlannedFuture {
                    gesture: PerformanceGesture::MuteTrack(TrackId::new(3)),
                    score: 20,
                },
            ],
        };
        let renderable = resolve_renderable_futures(
            &plan,
            &state,
            FrameRange::new(0, 8).unwrap(),
            48_000,
            &[(pattern, Fingerprint::new(9))].into_iter().collect(),
        );
        let order = Arc::new(Mutex::new(Vec::new()));
        let observed = Arc::clone(&order);
        let pool = FutureWorkerPool::new(1).unwrap();
        let submission = pool.submit_plan(&renderable, move |candidate| {
            observed
                .lock()
                .expect("render order lock poisoned")
                .push(candidate.score);
            AudioBlock::silent(1, 8)
        });
        assert_eq!(submission.submitted, 3);
        pool.wait_until_idle();
        assert_eq!(*order.lock().unwrap(), vec![30, 20, 10]);
        assert_eq!(
            pool.diagnostics(),
            FutureWorkerDiagnostics {
                desired_artifacts: 3,
                clean_artifacts: 3,
                queued_jobs: 0,
                active_jobs: 0,
                completed_jobs: 3,
                sleeping: true,
            }
        );
    }

    #[test]
    fn clean_future_plans_are_cache_hits_and_do_not_wake_workers() {
        let pattern = PatternId::new(1);
        let state = FuturePerformanceState {
            active_pattern: pattern,
            scene_patterns: BTreeMap::new(),
            muted_tracks: BTreeSet::new(),
        };
        let renderable = resolve_renderable_futures(
            &PerformanceFuturePlan {
                candidates: vec![PlannedFuture {
                    gesture: PerformanceGesture::MuteTrack(TrackId::new(1)),
                    score: 10,
                }],
            },
            &state,
            FrameRange::new(0, 8).unwrap(),
            48_000,
            &[(pattern, Fingerprint::new(9))].into_iter().collect(),
        );
        let pool = FutureWorkerPool::new(1).unwrap();
        assert_eq!(
            pool.submit_plan(&renderable, |_| AudioBlock::silent(1, 8)),
            FutureSubmission {
                desired: 1,
                cache_hits: 0,
                submitted: 1,
                unresolved: 0,
            }
        );
        pool.wait_until_idle();
        let key = renderable.candidates[0].key;
        assert_eq!(pool.cached_artifact(key).unwrap().frames(), 8);
        assert_eq!(
            pool.submit_plan(&renderable, |_| panic!("clean future rendered again")),
            FutureSubmission {
                desired: 1,
                cache_hits: 1,
                submitted: 0,
                unresolved: 0,
            }
        );
        assert!(pool.diagnostics().sleeping);
        assert_eq!(pool.diagnostics().completed_jobs, 1);
    }

    #[test]
    fn future_worker_pool_rejects_zero_workers() {
        assert_eq!(
            FutureWorkerPool::new(0).err(),
            Some(FutureWorkerPoolError::ZeroWorkers)
        );
    }

    #[test]
    fn beat_and_bar_quantization_choose_deterministic_boundaries() {
        let tempo = Tempo::new(120.0, 48_000).unwrap();
        assert_eq!(LaunchQuantization::Beat.next_frame(24_001, tempo), 48_000);
        assert_eq!(
            LaunchQuantization::Bar { beats: 4 }.next_frame(24_001, tempo),
            96_000
        );
        assert_eq!(
            LaunchQuantization::Bar { beats: 4 }.next_frame(96_000, tempo),
            96_000
        );
    }

    #[test]
    fn launcher_applies_due_gestures_and_falls_back_when_future_is_dirty() {
        let tempo = Tempo::new(120.0, 48_000).unwrap();
        let gesture = PerformanceGesture::MuteTrack(TrackId::new(2));
        let mut launcher = PerformanceLauncher::new(LaunchQuantization::Bar { beats: 4 });
        let queued = launcher.queue(gesture, 1, tempo);
        assert_eq!(queued.launch_frame, 96_000);
        let pool = FutureWorkerPool::new(1).unwrap();
        assert!(
            launcher
                .advance(95_999, &RenderableFuturePlan::default(), &pool)
                .is_none()
        );
        assert_eq!(
            launcher.advance(96_000, &RenderableFuturePlan::default(), &pool),
            Some(PerformanceLaunch {
                gesture,
                frame: 96_000,
                source: PerformanceLaunchSource::LiveFallback,
            })
        );
        assert!(launcher.active().muted_tracks.contains(&TrackId::new(2)));
        assert!(launcher.queued().is_none());
    }

    #[test]
    fn launcher_selects_a_clean_speculative_artifact_and_updates_scene_and_fill() {
        let tempo = Tempo::new(120.0, 48_000).unwrap();
        let pattern = PatternId::new(1);
        let scene = SceneId::new(4);
        let gesture = PerformanceGesture::QueueScene(scene);
        let state = FuturePerformanceState {
            active_pattern: pattern,
            scene_patterns: [(scene, pattern)].into_iter().collect(),
            muted_tracks: BTreeSet::new(),
        };
        let plan = resolve_renderable_futures(
            &PerformanceFuturePlan {
                candidates: vec![PlannedFuture { gesture, score: 9 }],
            },
            &state,
            FrameRange::new(0, 8).unwrap(),
            48_000,
            &[(pattern, Fingerprint::new(7))].into_iter().collect(),
        );
        let key = plan.candidates[0].key;
        let pool = FutureWorkerPool::new(1).unwrap();
        pool.submit_plan(&plan, |_| AudioBlock::silent(1, 8));
        pool.wait_until_idle();

        let mut launcher = PerformanceLauncher::new(LaunchQuantization::Beat);
        launcher.queue(PerformanceGesture::TriggerFill(PatternId::new(8)), 0, tempo);
        launcher.advance(0, &RenderableFuturePlan::default(), &pool);
        assert_eq!(launcher.active().fill, Some(PatternId::new(8)));
        launcher.queue(gesture, 0, tempo);
        assert_eq!(
            launcher.advance(0, &plan, &pool).unwrap().source,
            PerformanceLaunchSource::Speculative(key)
        );
        assert_eq!(launcher.active().scene, Some(scene));
        assert_eq!(launcher.active().fill, None);
    }

    #[test]
    fn due_cached_launch_atomically_publishes_the_prepared_audio() {
        let tempo = Tempo::new(120.0, 48_000).unwrap();
        let pattern = PatternId::new(1);
        let gesture = PerformanceGesture::MuteTrack(TrackId::new(2));
        let plan = resolve_renderable_futures(
            &PerformanceFuturePlan {
                candidates: vec![PlannedFuture { gesture, score: 9 }],
            },
            &FuturePerformanceState {
                active_pattern: pattern,
                scene_patterns: BTreeMap::new(),
                muted_tracks: BTreeSet::new(),
            },
            FrameRange::new(0, 8).unwrap(),
            48_000,
            &[(pattern, Fingerprint::new(7))].into_iter().collect(),
        );
        let pool = FutureWorkerPool::new(1).unwrap();
        pool.submit_plan(&plan, |_| {
            let mut block = AudioBlock::silent(1, 8);
            block.samples_mut().fill(0.625);
            block
        });
        pool.wait_until_idle();
        let publication = RealtimeChunkPublication::new(1, 8, 4).unwrap();
        let mut launcher = PerformanceLauncher::new(LaunchQuantization::Beat);
        launcher.queue(gesture, 0, tempo);

        let launch = launcher
            .advance_and_publish(0, &plan, &pool, &publication)
            .unwrap()
            .unwrap();
        assert_eq!(
            launch.source,
            PerformanceLaunchSource::Speculative(plan.candidates[0].key)
        );
        let snapshot = publication.reader().snapshot();
        assert_eq!(snapshot.frame(0), Ok([0.625].as_slice()));
        assert_eq!(snapshot.frame(7), Ok([0.625].as_slice()));
    }

    #[test]
    fn cache_miss_launch_restores_live_publication_without_waiting() {
        let tempo = Tempo::new(120.0, 48_000).unwrap();
        let gesture = PerformanceGesture::TriggerFill(PatternId::new(9));
        let pool = FutureWorkerPool::new(1).unwrap();
        let publication = RealtimeChunkPublication::new(1, 8, 4).unwrap();
        let mut launcher = PerformanceLauncher::new(LaunchQuantization::Beat);
        launcher.queue(gesture, 0, tempo);

        assert_eq!(
            launcher
                .advance_and_publish(0, &RenderableFuturePlan::default(), &pool, &publication,)
                .unwrap(),
            Some(PerformanceLaunch {
                gesture,
                frame: 0,
                source: PerformanceLaunchSource::LiveFallback,
            })
        );
        assert!(publication.reader().snapshot().frame(0).is_err());
    }

    #[test]
    fn fill_expires_after_its_configured_musical_lifetime() {
        let tempo = Tempo::new(120.0, 48_000).unwrap();
        let fill = PatternId::new(9);
        let gesture = PerformanceGesture::TriggerFill(fill);
        let pool = FutureWorkerPool::new(1).unwrap();
        let mut launcher = PerformanceLauncher::new(LaunchQuantization::Beat).with_fill_beats(2);
        let queued = launcher.queue(gesture, 1, tempo);
        assert_eq!(queued.launch_frame, 24_000);
        assert_eq!(queued.fill_end_frame, Some(72_000));
        launcher.advance(24_000, &RenderableFuturePlan::default(), &pool);
        assert_eq!(launcher.active().fill, Some(fill));
        assert_eq!(launcher.fill_end_frame(), Some(72_000));
        launcher.advance(71_999, &RenderableFuturePlan::default(), &pool);
        assert_eq!(launcher.active().fill, Some(fill));
        launcher.advance(72_000, &RenderableFuturePlan::default(), &pool);
        assert_eq!(launcher.active().fill, None);
        assert_eq!(launcher.fill_end_frame(), None);
        assert_eq!(launcher.diagnostics().queued_gestures, 1);
        assert_eq!(launcher.diagnostics().fallback_launches, 1);
        assert_eq!(launcher.diagnostics().fill_returns, 1);
        assert_eq!(launcher.diagnostics().last_launch.unwrap().gesture, gesture);
    }

    #[test]
    fn launcher_diagnostics_count_only_effective_cancellations() {
        let tempo = Tempo::new(120.0, 48_000).unwrap();
        let mut launcher = PerformanceLauncher::new(LaunchQuantization::Beat);
        assert!(launcher.cancel().is_none());
        launcher.queue(PerformanceGesture::MuteTrack(TrackId::new(1)), 1, tempo);
        assert!(launcher.cancel().is_some());
        let diagnostics = launcher.diagnostics();
        assert_eq!(diagnostics.queued_gestures, 1);
        assert_eq!(diagnostics.cancelled_gestures, 1);
        assert_eq!(diagnostics.speculative_launches, 0);
        assert_eq!(diagnostics.fallback_launches, 0);
    }

    #[test]
    fn expired_fill_atomically_returns_from_prepared_audio_to_live_audio() {
        let tempo = Tempo::new(120.0, 48_000).unwrap();
        let base = PatternId::new(1);
        let fill = PatternId::new(9);
        let gesture = PerformanceGesture::TriggerFill(fill);
        let plan = resolve_renderable_futures(
            &PerformanceFuturePlan {
                candidates: vec![PlannedFuture { gesture, score: 9 }],
            },
            &FuturePerformanceState {
                active_pattern: base,
                scene_patterns: BTreeMap::new(),
                muted_tracks: BTreeSet::new(),
            },
            FrameRange::new(0, 8).unwrap(),
            48_000,
            &[(base, Fingerprint::new(1)), (fill, Fingerprint::new(2))]
                .into_iter()
                .collect(),
        );
        let pool = FutureWorkerPool::new(1).unwrap();
        pool.submit_plan(&plan, |_| {
            let mut block = AudioBlock::silent(1, 8);
            block.samples_mut().fill(0.8);
            block
        });
        pool.wait_until_idle();
        let publication = RealtimeChunkPublication::new(1, 8, 4).unwrap();
        let mut launcher = PerformanceLauncher::new(LaunchQuantization::Beat).with_fill_beats(1);
        launcher.queue(gesture, 0, tempo);
        launcher
            .advance_and_publish(0, &plan, &pool, &publication)
            .unwrap();
        assert_eq!(
            publication.reader().snapshot().frame(0),
            Ok([0.8].as_slice())
        );

        assert_eq!(
            launcher
                .advance_and_publish(24_000, &plan, &pool, &publication)
                .unwrap(),
            None
        );
        assert_eq!(launcher.active().fill, None);
        assert!(publication.reader().snapshot().frame(0).is_err());
    }
}
