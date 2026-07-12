//! Headless offline rendering foundations.
//!
//! This crate starts with a deliberately small deterministic event renderer. It
//! is not a sampler yet; it proves that scheduled core events can become finite
//! internal `f64` audio blocks without touching device I/O.

pub mod coordinator;
pub mod dsp;
pub mod dynamics;
pub mod effects;
pub mod futures;
pub mod live_edit;
pub mod mastering;
pub mod modulation;
pub mod phrases;
pub mod stereo_fx;
pub mod transforms;

use meldritch_audio::audio_publication::{
    AudioPublicationError, AudioSnapshotPublisher, AudioSnapshotReader, audio_publication,
};
use meldritch_audio::published_audio::{PublishedAudio, PublishedAudioError};
use meldritch_audio::{AudioBlock, SampleBuffer};
use meldritch_core::{
    Arrangement, AutomationInterpolation, AutomationLane, AutomationValue, Event, FrameRange,
    Pattern, PatternId, ProbabilitySeed, Sample, SampleRate, Tempo,
};
use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RenderSettings {
    channels: u16,
}

impl RenderSettings {
    pub fn new(channels: u16) -> Result<Self, RenderSettingsError> {
        if channels == 0 {
            return Err(RenderSettingsError::InvalidChannelCount(channels));
        }

        Ok(Self { channels })
    }

    #[must_use]
    pub const fn channels(self) -> u16 {
        self.channels
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RenderSettingsError {
    InvalidChannelCount(u16),
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Fingerprint(u64);

impl Fingerprint {
    #[must_use]
    pub const fn new(raw: u64) -> Self {
        Self(raw)
    }

    #[must_use]
    pub const fn raw(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ArtifactKey {
    pattern: PatternId,
    range: FrameRange,
    sample_rate: SampleRate,
    fingerprint: Fingerprint,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ArrangementArtifactKey {
    range: FrameRange,
    sample_rate: SampleRate,
    fingerprint: Fingerprint,
}

impl ArrangementArtifactKey {
    #[must_use]
    pub const fn range(self) -> FrameRange {
        self.range
    }

    #[must_use]
    pub const fn fingerprint(self) -> Fingerprint {
        self.fingerprint
    }

    #[must_use]
    pub const fn sample_rate(self) -> SampleRate {
        self.sample_rate
    }
}

type CompletionHook = Arc<dyn Fn(ArtifactKey, &AudioBlock) + Send + Sync + 'static>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CacheStatus {
    Hit,
    Miss,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CachedArtifact {
    key: ArtifactKey,
    block: AudioBlock,
    status: CacheStatus,
}

impl CachedArtifact {
    #[must_use]
    pub const fn key(&self) -> ArtifactKey {
        self.key
    }

    #[must_use]
    pub fn block(&self) -> &AudioBlock {
        &self.block
    }

    #[must_use]
    pub const fn status(&self) -> CacheStatus {
        self.status
    }

    #[must_use]
    pub fn into_block(self) -> AudioBlock {
        self.block
    }
}

#[derive(Clone, Debug, Default)]
pub struct ArtifactCache {
    artifacts: BTreeMap<ArtifactKey, AudioBlock>,
}

impl ArtifactCache {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.artifacts.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.artifacts.is_empty()
    }

    #[must_use]
    pub fn get(&self, key: ArtifactKey) -> Option<&AudioBlock> {
        self.artifacts.get(&key)
    }

    pub fn insert(&mut self, key: ArtifactKey, block: AudioBlock) -> Option<AudioBlock> {
        self.artifacts.insert(key, block)
    }

    pub fn invalidate_range(&mut self, pattern: PatternId, range: FrameRange) -> usize {
        let before = self.artifacts.len();
        self.artifacts.retain(|key, _| {
            key.pattern() != pattern
                || key.range().end() <= range.start()
                || key.range().start() >= range.end()
        });
        before - self.artifacts.len()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RenderPriority {
    Hot,
    Warm,
    Cold,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RenderHorizonError {
    EmptyTimeline,
    ZeroChunkFrames,
    PlayheadOutsideTimeline,
    InvalidRange,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RenderHorizon {
    playhead: u32,
    timeline_frames: u32,
    chunk_frames: u32,
    warm_chunks: usize,
}

impl RenderHorizon {
    pub fn new(
        playhead: u32,
        timeline_frames: u32,
        chunk_frames: u32,
        warm_chunks: usize,
    ) -> Result<Self, RenderHorizonError> {
        if timeline_frames == 0 {
            return Err(RenderHorizonError::EmptyTimeline);
        }
        if chunk_frames == 0 {
            return Err(RenderHorizonError::ZeroChunkFrames);
        }
        if playhead >= timeline_frames {
            return Err(RenderHorizonError::PlayheadOutsideTimeline);
        }
        Ok(Self {
            playhead,
            timeline_frames,
            chunk_frames,
            warm_chunks,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlannedRenderChunk {
    pub key: ArtifactKey,
    pub range: FrameRange,
    pub priority: RenderPriority,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RenderHorizonSubmission {
    pub planned_chunks: usize,
    pub cache_hits: usize,
    pub submitted_jobs: usize,
}

pub fn plan_sample_render_horizon(
    pattern: &Pattern,
    tempo: Tempo,
    probability_seed: ProbabilitySeed,
    settings: RenderSettings,
    samples_by_note: &BTreeMap<u8, SampleBuffer>,
    horizon: RenderHorizon,
) -> Result<Vec<PlannedRenderChunk>, RenderHorizonError> {
    let chunk_count = horizon.timeline_frames.div_ceil(horizon.chunk_frames) as usize;
    let current = (horizon.playhead / horizon.chunk_frames) as usize;
    let mut plan = Vec::with_capacity(chunk_count);
    for distance in 0..chunk_count {
        let index = (current + distance) % chunk_count;
        let start = index as u32 * horizon.chunk_frames;
        let end = horizon.timeline_frames.min(start + horizon.chunk_frames);
        let range = FrameRange::new(u64::from(start), u64::from(end))
            .map_err(|_| RenderHorizonError::InvalidRange)?;
        let priority = if distance == 0 {
            RenderPriority::Hot
        } else if distance <= horizon.warm_chunks {
            RenderPriority::Warm
        } else {
            RenderPriority::Cold
        };
        let key = pattern_sample_artifact_key(
            pattern,
            tempo,
            range,
            probability_seed,
            settings,
            samples_by_note,
        );
        plan.push(PlannedRenderChunk {
            key,
            range,
            priority,
        });
    }
    Ok(plan)
}

#[allow(clippy::too_many_arguments)]
pub fn submit_sample_render_horizon(
    pool: &RenderWorkerPool,
    publication: &RealtimeChunkPublication,
    pattern: &Pattern,
    tempo: Tempo,
    probability_seed: ProbabilitySeed,
    settings: RenderSettings,
    samples_by_note: Arc<BTreeMap<u8, SampleBuffer>>,
    horizon: RenderHorizon,
) -> Result<RenderHorizonSubmission, RenderHorizonError> {
    let plan = plan_sample_render_horizon(
        pattern,
        tempo,
        probability_seed,
        settings,
        &samples_by_note,
        horizon,
    )?;
    let mut submission = RenderHorizonSubmission {
        planned_chunks: plan.len(),
        ..RenderHorizonSubmission::default()
    };
    for chunk in plan {
        let _ = publication.expect_artifact(chunk.key);
        if let Some(block) = pool.cached_artifact(chunk.key) {
            let _ = publication.publish_artifact(chunk.key, &block);
            submission.cache_hits += 1;
            continue;
        }

        let pattern = pattern.clone();
        let samples_by_note = Arc::clone(&samples_by_note);
        if pool.submit_if_needed(chunk.key, chunk.priority, move || {
            render_pattern_samples_chunk(
                &pattern,
                tempo,
                chunk.range,
                probability_seed,
                settings,
                &samples_by_note,
            )
        }) {
            submission.submitted_jobs += 1;
        }
    }
    Ok(submission)
}

impl RenderPriority {
    const fn rank(self) -> u8 {
        match self {
            Self::Hot => 3,
            Self::Warm => 2,
            Self::Cold => 1,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct WorkerDiagnostics {
    pub queued_jobs: usize,
    pub active_jobs: usize,
    pub completed_jobs: usize,
}

type RenderTask = Box<dyn FnOnce() -> AudioBlock + Send + 'static>;

struct RenderJob {
    key: ArtifactKey,
    priority: RenderPriority,
    sequence: u64,
    task: RenderTask,
}

impl RenderJob {
    fn new(key: ArtifactKey, priority: RenderPriority, sequence: u64, task: RenderTask) -> Self {
        Self {
            key,
            priority,
            sequence,
            task,
        }
    }
}

impl Eq for RenderJob {}

impl PartialEq for RenderJob {
    fn eq(&self, other: &Self) -> bool {
        self.priority == other.priority && self.sequence == other.sequence && self.key == other.key
    }
}

impl Ord for RenderJob {
    fn cmp(&self, other: &Self) -> Ordering {
        self.priority
            .rank()
            .cmp(&other.priority.rank())
            .then_with(|| other.sequence.cmp(&self.sequence))
            .then_with(|| self.key.cmp(&other.key))
    }
}

impl PartialOrd for RenderJob {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Default)]
struct RenderJobQueue {
    jobs: BinaryHeap<RenderJob>,
    next_sequence: u64,
}

impl RenderJobQueue {
    fn push(&mut self, key: ArtifactKey, priority: RenderPriority, task: RenderTask) {
        let sequence = self.next_sequence;
        self.next_sequence = self.next_sequence.wrapping_add(1);
        self.jobs
            .push(RenderJob::new(key, priority, sequence, task));
    }

    fn pop(&mut self) -> Option<RenderJob> {
        self.jobs.pop()
    }

    fn len(&self) -> usize {
        self.jobs.len()
    }
}

#[derive(Default)]
struct WorkerState {
    queue: RenderJobQueue,
    wanted_keys: BTreeSet<ArtifactKey>,
    active_jobs: usize,
    completed_jobs: usize,
    shutdown: bool,
}

impl WorkerState {
    fn diagnostics(&self) -> WorkerDiagnostics {
        WorkerDiagnostics {
            queued_jobs: self.queue.len(),
            active_jobs: self.active_jobs,
            completed_jobs: self.completed_jobs,
        }
    }
}

#[derive(Default)]
struct WorkerShared {
    state: Mutex<WorkerState>,
    cache: Mutex<ArtifactCache>,
    has_work: Condvar,
    idle: Condvar,
    completion_hook: Option<CompletionHook>,
}

pub struct RenderWorkerPool {
    shared: Arc<WorkerShared>,
    workers: Vec<JoinHandle<()>>,
}

impl RenderWorkerPool {
    pub fn new(worker_count: usize) -> Result<Self, RenderWorkerPoolError> {
        Self::new_inner(worker_count, None)
    }

    pub fn with_completion_hook<F>(
        worker_count: usize,
        hook: F,
    ) -> Result<Self, RenderWorkerPoolError>
    where
        F: Fn(ArtifactKey, &AudioBlock) + Send + Sync + 'static,
    {
        Self::new_inner(worker_count, Some(Arc::new(hook)))
    }

    fn new_inner(
        worker_count: usize,
        completion_hook: Option<CompletionHook>,
    ) -> Result<Self, RenderWorkerPoolError> {
        if worker_count == 0 {
            return Err(RenderWorkerPoolError::ZeroWorkers);
        }

        let shared = Arc::new(WorkerShared {
            completion_hook,
            ..WorkerShared::default()
        });
        let workers = (0..worker_count)
            .map(|_| spawn_render_worker(Arc::clone(&shared)))
            .collect();

        Ok(Self { shared, workers })
    }

    pub fn submit<F>(&self, key: ArtifactKey, priority: RenderPriority, task: F)
    where
        F: FnOnce() -> AudioBlock + Send + 'static,
    {
        let _ = self.submit_if_needed(key, priority, task);
    }

    pub fn submit_if_needed<F>(&self, key: ArtifactKey, priority: RenderPriority, task: F) -> bool
    where
        F: FnOnce() -> AudioBlock + Send + 'static,
    {
        let cache = self
            .shared
            .cache
            .lock()
            .expect("artifact cache lock poisoned");
        let mut state = self
            .shared
            .state
            .lock()
            .expect("worker state lock poisoned");
        if cache.get(key).is_some() || !state.wanted_keys.insert(key) {
            return false;
        }
        state.queue.push(key, priority, Box::new(task));
        self.shared.has_work.notify_one();
        true
    }

    pub fn wait_until_idle(&self) {
        let mut state = self
            .shared
            .state
            .lock()
            .expect("worker state lock poisoned");
        while state.active_jobs != 0 || state.queue.len() != 0 {
            state = self
                .shared
                .idle
                .wait(state)
                .expect("worker state lock poisoned");
        }
    }

    #[must_use]
    pub fn diagnostics(&self) -> WorkerDiagnostics {
        self.shared
            .state
            .lock()
            .expect("worker state lock poisoned")
            .diagnostics()
    }

    #[must_use]
    pub fn cache_len(&self) -> usize {
        self.shared
            .cache
            .lock()
            .expect("artifact cache lock poisoned")
            .len()
    }

    #[must_use]
    pub fn cached_artifact(&self, key: ArtifactKey) -> Option<AudioBlock> {
        self.shared
            .cache
            .lock()
            .expect("artifact cache lock poisoned")
            .get(key)
            .cloned()
    }

    pub fn invalidate_range(&self, pattern: PatternId, range: FrameRange) -> usize {
        self.shared
            .cache
            .lock()
            .expect("artifact cache lock poisoned")
            .invalidate_range(pattern, range)
    }

    fn shutdown(&mut self) {
        {
            let mut state = self
                .shared
                .state
                .lock()
                .expect("worker state lock poisoned");
            state.shutdown = true;
            self.shared.has_work.notify_all();
        }

        for worker in self.workers.drain(..) {
            worker.join().expect("render worker panicked");
        }
    }
}

impl Drop for RenderWorkerPool {
    fn drop(&mut self) {
        self.shutdown();
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RenderWorkerPoolError {
    ZeroWorkers,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChunkPublicationError {
    InvalidSnapshot(PublishedAudioError),
    IncompatiblePublication(AudioPublicationError),
    MisalignedRange,
    RangeOutsideTimeline,
    ChannelMismatch { expected: u16, actual: u16 },
    FrameCountMismatch { expected: u32, actual: u32 },
    UnexpectedArtifact,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ChunkPublicationDiagnostics {
    pub ready_chunks: usize,
    pub published_artifacts: usize,
    pub rejected_artifacts: usize,
    pub invalidated_chunks: usize,
    pub stale_artifacts: usize,
}

struct ChunkPublicationState {
    chunks: Vec<Option<Arc<[Sample]>>>,
    expected_keys: Vec<Option<ArtifactKey>>,
    ready_chunks: usize,
}

struct ChunkPublicationInner {
    channels: u16,
    frames: u32,
    chunk_frames: u32,
    publisher: AudioSnapshotPublisher,
    reader: AudioSnapshotReader,
    state: Mutex<ChunkPublicationState>,
    published_artifacts: AtomicUsize,
    rejected_artifacts: AtomicUsize,
    invalidated_chunks: AtomicUsize,
    stale_artifacts: AtomicUsize,
}

/// Worker-side assembler that republishes an immutable realtime snapshot after
/// each compatible chunk artifact completes.
#[derive(Clone)]
pub struct RealtimeChunkPublication {
    inner: Arc<ChunkPublicationInner>,
}

impl RealtimeChunkPublication {
    pub fn new(
        channels: u16,
        frames: u32,
        chunk_frames: u32,
    ) -> Result<Self, ChunkPublicationError> {
        let chunk_count = if chunk_frames == 0 {
            0
        } else {
            frames.div_ceil(chunk_frames) as usize
        };
        let chunks = vec![None; chunk_count];
        let initial = PublishedAudio::from_chunks(channels, frames, chunk_frames, chunks.clone())
            .map_err(ChunkPublicationError::InvalidSnapshot)?;
        let (publisher, reader) = audio_publication(initial);
        Ok(Self {
            inner: Arc::new(ChunkPublicationInner {
                channels,
                frames,
                chunk_frames,
                publisher,
                reader,
                state: Mutex::new(ChunkPublicationState {
                    expected_keys: vec![None; chunk_count],
                    chunks,
                    ready_chunks: 0,
                }),
                published_artifacts: AtomicUsize::new(0),
                rejected_artifacts: AtomicUsize::new(0),
                invalidated_chunks: AtomicUsize::new(0),
                stale_artifacts: AtomicUsize::new(0),
            }),
        })
    }

    #[must_use]
    pub fn reader(&self) -> AudioSnapshotReader {
        self.inner.reader.clone()
    }

    pub fn publish_artifact(
        &self,
        key: ArtifactKey,
        block: &AudioBlock,
    ) -> Result<(), ChunkPublicationError> {
        let result = self.publish_artifact_inner(key, block);
        match result {
            Ok(()) => {
                self.inner
                    .published_artifacts
                    .fetch_add(1, AtomicOrdering::Relaxed);
                Ok(())
            }
            Err(error) => {
                self.inner
                    .rejected_artifacts
                    .fetch_add(1, AtomicOrdering::Relaxed);
                if error == ChunkPublicationError::UnexpectedArtifact {
                    self.inner
                        .stale_artifacts
                        .fetch_add(1, AtomicOrdering::Relaxed);
                }
                Err(error)
            }
        }
    }

    fn publish_artifact_inner(
        &self,
        key: ArtifactKey,
        block: &AudioBlock,
    ) -> Result<(), ChunkPublicationError> {
        let (index, expected_frames) = self.chunk_for_key(key)?;
        if block.channels() != self.inner.channels {
            return Err(ChunkPublicationError::ChannelMismatch {
                expected: self.inner.channels,
                actual: block.channels(),
            });
        }
        if block.frames() != expected_frames {
            return Err(ChunkPublicationError::FrameCountMismatch {
                expected: expected_frames,
                actual: block.frames(),
            });
        }

        let mut state = self
            .inner
            .state
            .lock()
            .expect("chunk publication state lock poisoned");
        if state.expected_keys[index] != Some(key) {
            return Err(ChunkPublicationError::UnexpectedArtifact);
        }
        if state.chunks[index].is_none() {
            state.ready_chunks += 1;
        }
        state.chunks[index] = Some(Arc::from(block.samples()));
        let snapshot = PublishedAudio::from_chunks(
            self.inner.channels,
            self.inner.frames,
            self.inner.chunk_frames,
            state.chunks.clone(),
        )
        .map_err(ChunkPublicationError::InvalidSnapshot)?;
        self.inner
            .publisher
            .publish(snapshot)
            .map_err(ChunkPublicationError::IncompatiblePublication)
    }

    pub fn expect_artifact(&self, key: ArtifactKey) -> Result<(), ChunkPublicationError> {
        let (index, _) = self.chunk_for_key(key)?;
        let mut state = self
            .inner
            .state
            .lock()
            .expect("chunk publication state lock poisoned");
        if state.expected_keys[index] == Some(key) {
            return Ok(());
        }
        state.expected_keys[index] = Some(key);
        if state.chunks[index].take().is_some() {
            state.ready_chunks -= 1;
            self.inner
                .invalidated_chunks
                .fetch_add(1, AtomicOrdering::Relaxed);
            self.publish_state(&state)?;
        }
        Ok(())
    }

    pub fn invalidate_range(&self, range: FrameRange) -> Result<usize, ChunkPublicationError> {
        if range.start() >= u64::from(self.inner.frames) || range.start() >= range.end() {
            return Ok(0);
        }
        let dirty_end = range.end().min(u64::from(self.inner.frames));
        let first = (range.start() / u64::from(self.inner.chunk_frames)) as usize;
        let last = ((dirty_end - 1) / u64::from(self.inner.chunk_frames)) as usize;
        let mut state = self
            .inner
            .state
            .lock()
            .expect("chunk publication state lock poisoned");
        let mut invalidated = 0;
        for index in first..=last {
            state.expected_keys[index] = None;
            if state.chunks[index].take().is_some() {
                state.ready_chunks -= 1;
                invalidated += 1;
            }
        }
        if invalidated != 0 {
            self.inner
                .invalidated_chunks
                .fetch_add(invalidated, AtomicOrdering::Relaxed);
            self.publish_state(&state)?;
        }
        Ok(invalidated)
    }

    fn chunk_for_key(&self, key: ArtifactKey) -> Result<(usize, u32), ChunkPublicationError> {
        let range = key.range();
        if range.end() > u64::from(self.inner.frames) {
            return Err(ChunkPublicationError::RangeOutsideTimeline);
        }
        let start = u32::try_from(range.start())
            .map_err(|_| ChunkPublicationError::RangeOutsideTimeline)?;
        let end =
            u32::try_from(range.end()).map_err(|_| ChunkPublicationError::RangeOutsideTimeline)?;
        if start % self.inner.chunk_frames != 0 || start == end {
            return Err(ChunkPublicationError::MisalignedRange);
        }
        let expected_frames = self.inner.chunk_frames.min(self.inner.frames - start);
        if end - start != expected_frames {
            return Err(ChunkPublicationError::MisalignedRange);
        }
        Ok(((start / self.inner.chunk_frames) as usize, expected_frames))
    }

    fn publish_state(&self, state: &ChunkPublicationState) -> Result<(), ChunkPublicationError> {
        let snapshot = PublishedAudio::from_chunks(
            self.inner.channels,
            self.inner.frames,
            self.inner.chunk_frames,
            state.chunks.clone(),
        )
        .map_err(ChunkPublicationError::InvalidSnapshot)?;
        self.inner
            .publisher
            .publish(snapshot)
            .map_err(ChunkPublicationError::IncompatiblePublication)
    }

    #[must_use]
    pub fn diagnostics(&self) -> ChunkPublicationDiagnostics {
        let state = self
            .inner
            .state
            .lock()
            .expect("chunk publication state lock poisoned");
        ChunkPublicationDiagnostics {
            ready_chunks: state.ready_chunks,
            published_artifacts: self.inner.published_artifacts.load(AtomicOrdering::Relaxed),
            rejected_artifacts: self.inner.rejected_artifacts.load(AtomicOrdering::Relaxed),
            invalidated_chunks: self.inner.invalidated_chunks.load(AtomicOrdering::Relaxed),
            stale_artifacts: self.inner.stale_artifacts.load(AtomicOrdering::Relaxed),
        }
    }

    pub fn completion_hook(&self) -> impl Fn(ArtifactKey, &AudioBlock) + Send + Sync + 'static {
        let publication = self.clone();
        move |key, block| {
            let _ = publication.publish_artifact(key, block);
        }
    }

    pub fn publish_override(&self, block: &AudioBlock) -> Result<(), ChunkPublicationError> {
        if block.channels() != self.inner.channels {
            return Err(ChunkPublicationError::ChannelMismatch {
                expected: self.inner.channels,
                actual: block.channels(),
            });
        }
        if block.frames() != self.inner.frames {
            return Err(ChunkPublicationError::FrameCountMismatch {
                expected: self.inner.frames,
                actual: block.frames(),
            });
        }
        let snapshot = PublishedAudio::from_block(block, self.inner.chunk_frames)
            .map_err(ChunkPublicationError::InvalidSnapshot)?;
        self.inner
            .publisher
            .publish(snapshot)
            .map_err(ChunkPublicationError::IncompatiblePublication)
    }

    pub fn restore_live_snapshot(&self) -> Result<(), ChunkPublicationError> {
        let state = self
            .inner
            .state
            .lock()
            .expect("chunk publication state lock poisoned");
        self.publish_state(&state)
    }
}

fn spawn_render_worker(shared: Arc<WorkerShared>) -> JoinHandle<()> {
    thread::spawn(move || {
        loop {
            let job = {
                let mut state = shared.state.lock().expect("worker state lock poisoned");
                loop {
                    if let Some(job) = state.queue.pop() {
                        state.active_jobs += 1;
                        break job;
                    }
                    if state.shutdown {
                        return;
                    }
                    state = shared
                        .has_work
                        .wait(state)
                        .expect("worker state lock poisoned");
                }
            };

            let block = (job.task)();
            if let Some(hook) = &shared.completion_hook {
                hook(job.key, &block);
            }
            shared
                .cache
                .lock()
                .expect("artifact cache lock poisoned")
                .insert(job.key, block);

            let mut state = shared.state.lock().expect("worker state lock poisoned");
            state.wanted_keys.remove(&job.key);
            state.active_jobs -= 1;
            state.completed_jobs += 1;
            if state.active_jobs == 0 && state.queue.len() == 0 {
                shared.idle.notify_all();
            }
        }
    })
}
impl ArtifactKey {
    #[must_use]
    pub const fn pattern(self) -> PatternId {
        self.pattern
    }

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

    #[must_use]
    pub fn with_automation(mut self, lanes: &[AutomationLane]) -> Self {
        let mut state = FingerprintBuilder::new();
        state.write_u64(self.fingerprint.raw());
        state.write_u64(automation_fingerprint(lanes, self.range).raw());
        self.fingerprint = state.finish();
        self
    }

    #[must_use]
    pub fn with_dependency_fingerprint(mut self, fingerprint: Fingerprint) -> Self {
        let mut state = FingerprintBuilder::new();
        state.write_u64(self.fingerprint.raw());
        state.write_u64(fingerprint.raw());
        self.fingerprint = state.finish();
        self
    }
}

#[must_use]
pub fn pattern_click_artifact_key(
    pattern: &Pattern,
    tempo: Tempo,
    range: FrameRange,
    probability_seed: ProbabilitySeed,
    settings: RenderSettings,
) -> ArtifactKey {
    let mut state = FingerprintBuilder::new();
    state.write_u64(pattern.id().raw());
    state.write_u64(u64::from(pattern.length_steps()));
    state.write_u64(u64::from(pattern.steps_per_beat()));
    state.write_u64(range.start());
    state.write_u64(range.end());
    state.write_u64(u64::from(tempo.sample_rate()));
    state.write_u64(tempo.bpm().to_bits());
    state.write_u64(probability_seed.raw());
    state.write_u64(u64::from(settings.channels()));
    state.write_u64(0x7061_7474_6572_6e63);
    let mut events = Vec::new();
    pattern.events_between(tempo, range, probability_seed, &mut events);
    events.retain(|event| {
        range.start() <= event.range().start() && event.range().start() < range.end()
    });
    write_event_fingerprint(&mut state, &events);

    ArtifactKey {
        pattern: pattern.id(),
        range,
        sample_rate: tempo.sample_rate(),
        fingerprint: state.finish(),
    }
}

#[must_use]
pub fn pattern_sample_artifact_key(
    pattern: &Pattern,
    tempo: Tempo,
    range: FrameRange,
    probability_seed: ProbabilitySeed,
    settings: RenderSettings,
    samples_by_note: &BTreeMap<u8, SampleBuffer>,
) -> ArtifactKey {
    let mut state = FingerprintBuilder::new();
    state.write_u64(pattern.id().raw());
    state.write_u64(u64::from(pattern.length_steps()));
    state.write_u64(u64::from(pattern.steps_per_beat()));
    state.write_u64(range.start());
    state.write_u64(range.end());
    state.write_u64(u64::from(tempo.sample_rate()));
    state.write_u64(tempo.bpm().to_bits());
    state.write_u64(probability_seed.raw());
    state.write_u64(u64::from(settings.channels()));
    state.write_u64(0x7361_6d70_6c65_7265);

    let lookbehind = samples_by_note
        .values()
        .map(SampleBuffer::frames)
        .max()
        .unwrap_or(0);
    let expanded = FrameRange::new(
        range.start().saturating_sub(u64::from(lookbehind)),
        range.end(),
    )
    .expect("artifact lookbehind preserves a valid range");
    let mut events = Vec::new();
    pattern.events_between(tempo, expanded, probability_seed, &mut events);
    events.retain(|event| {
        expanded.start() <= event.range().start() && event.range().start() < expanded.end()
    });
    write_event_fingerprint(&mut state, &events);
    let used_notes = events.iter().map(Event::note).collect::<BTreeSet<_>>();
    for note in used_notes {
        if let Some(sample) = samples_by_note.get(&note) {
            state.write_u64(u64::from(note));
            state.write_u64(u64::from(sample.channels()));
            state.write_u64(u64::from(sample.sample_rate()));
            state.write_u64(u64::from(sample.frames()));
            state.write_u64(sample_signature(sample));
        }
    }

    ArtifactKey {
        pattern: pattern.id(),
        range,
        sample_rate: tempo.sample_rate(),
        fingerprint: state.finish(),
    }
}

#[must_use]
pub fn arrangement_sample_artifact_key(
    arrangement: &Arrangement,
    tempo: Tempo,
    range: FrameRange,
    probability_seed: ProbabilitySeed,
    settings: RenderSettings,
    samples_by_note: &BTreeMap<u8, SampleBuffer>,
) -> ArrangementArtifactKey {
    let mut state = FingerprintBuilder::new();
    state.write_u64(range.start());
    state.write_u64(range.end());
    state.write_u64(u64::from(tempo.sample_rate()));
    state.write_u64(tempo.bpm().to_bits());
    state.write_u64(probability_seed.raw());
    state.write_u64(u64::from(settings.channels()));
    state.write_u64(arrangement.sections().len() as u64);
    let lookbehind = samples_by_note
        .values()
        .map(SampleBuffer::frames)
        .max()
        .unwrap_or(0);
    let expanded = FrameRange::new(
        range.start().saturating_sub(u64::from(lookbehind)),
        range.end(),
    )
    .expect("artifact lookbehind preserves a valid range");
    let mut events = Vec::new();
    arrangement.events_between(tempo, expanded, probability_seed, &mut events);
    write_event_fingerprint(&mut state, &events);
    for note in events.iter().map(Event::note).collect::<BTreeSet<_>>() {
        if let Some(sample) = samples_by_note.get(&note) {
            state.write_u64(u64::from(note));
            state.write_u64(u64::from(sample.channels()));
            state.write_u64(u64::from(sample.sample_rate()));
            state.write_u64(u64::from(sample.frames()));
            state.write_u64(sample_signature(sample));
        }
    }
    ArrangementArtifactKey {
        range,
        sample_rate: tempo.sample_rate(),
        fingerprint: state.finish(),
    }
}

#[must_use]
pub fn automation_fingerprint(lanes: &[AutomationLane], range: FrameRange) -> Fingerprint {
    let mut state = FingerprintBuilder::new();
    state.write_u64(range.start());
    state.write_u64(range.end());
    state.write_u64(lanes.len() as u64);
    for lane in lanes {
        state.write_u64(lane.target() as u64);
        state.write_u64(match lane.interpolation() {
            AutomationInterpolation::Linear => 0,
            AutomationInterpolation::Step => 1,
        });
        let points = lane.points_affecting(range);
        state.write_u64(points.len() as u64);
        for point in points {
            state.write_u64(point.frame);
            match point.value {
                AutomationValue::Continuous(value) => {
                    state.write_u64(0);
                    state.write_u64(value.to_bits());
                }
                AutomationValue::Discrete(value) => {
                    state.write_u64(1);
                    state.write_u64(value as u64);
                }
            }
        }
    }
    state.finish()
}

pub fn render_pattern_clicks_cached(
    cache: &mut ArtifactCache,
    pattern: &Pattern,
    tempo: Tempo,
    range: FrameRange,
    probability_seed: ProbabilitySeed,
    settings: RenderSettings,
) -> CachedArtifact {
    let key = pattern_click_artifact_key(pattern, tempo, range, probability_seed, settings);
    if let Some(block) = cache.get(key) {
        return CachedArtifact {
            key,
            block: block.clone(),
            status: CacheStatus::Hit,
        };
    }

    let block = render_pattern_clicks(pattern, tempo, range, probability_seed, settings);
    cache.insert(key, block.clone());
    CachedArtifact {
        key,
        block,
        status: CacheStatus::Miss,
    }
}

pub fn render_pattern_clicks(
    pattern: &Pattern,
    tempo: Tempo,
    range: FrameRange,
    probability_seed: ProbabilitySeed,
    settings: RenderSettings,
) -> AudioBlock {
    let frames = range
        .end()
        .saturating_sub(range.start())
        .min(u64::from(u32::MAX)) as u32;
    let mut block = AudioBlock::silent(settings.channels(), frames);
    let mut events = Vec::new();
    pattern.events_between(tempo, range, probability_seed, &mut events);

    for event in events {
        let Some(relative_frame) = event.range().start().checked_sub(range.start()) else {
            continue;
        };
        if relative_frame >= u64::from(frames) {
            continue;
        }

        let sample = event.velocity().clamp(0.0, 1.0) as Sample;
        write_frame(&mut block, relative_frame as u32, sample);
    }

    block
}

pub fn render_pattern_samples(
    pattern: &Pattern,
    tempo: Tempo,
    range: FrameRange,
    probability_seed: ProbabilitySeed,
    settings: RenderSettings,
    samples_by_note: &BTreeMap<u8, SampleBuffer>,
) -> AudioBlock {
    let frames = range
        .end()
        .saturating_sub(range.start())
        .min(u64::from(u32::MAX)) as u32;
    let mut block = AudioBlock::silent(settings.channels(), frames);
    let mut events = Vec::new();
    pattern.events_between(tempo, range, probability_seed, &mut events);

    for event in events {
        let Some(sample) = samples_by_note.get(&event.note()) else {
            continue;
        };
        let Some(relative_frame) = event.range().start().checked_sub(range.start()) else {
            continue;
        };
        if relative_frame >= u64::from(frames) {
            continue;
        }

        let gate_frames = event.range().end().saturating_sub(event.range().start());
        mix_sample(
            &mut block,
            relative_frame as u32,
            sample,
            event.velocity(),
            gate_frames,
        );
    }

    block
}

/// Render sample-backed arrangement audio, including tails from events that
/// started before the requested range.
pub fn render_arrangement_samples(
    arrangement: &Arrangement,
    tempo: Tempo,
    range: FrameRange,
    probability_seed: ProbabilitySeed,
    settings: RenderSettings,
    samples_by_note: &BTreeMap<u8, SampleBuffer>,
) -> AudioBlock {
    let frames = range
        .end()
        .saturating_sub(range.start())
        .min(u64::from(u32::MAX)) as u32;
    let mut block = AudioBlock::silent(settings.channels(), frames);
    let lookbehind = samples_by_note
        .values()
        .map(SampleBuffer::frames)
        .max()
        .unwrap_or(0);
    let expanded = FrameRange::new(
        range.start().saturating_sub(u64::from(lookbehind)),
        range.end(),
    )
    .expect("arrangement render lookbehind preserves a valid range");
    let mut events = Vec::new();
    arrangement.events_between(tempo, expanded, probability_seed, &mut events);

    for event in events {
        let Some(sample) = samples_by_note.get(&event.note()) else {
            continue;
        };
        let (start_frame, source_frame) = if event.range().start() < range.start() {
            (0, range.start() - event.range().start())
        } else {
            (event.range().start() - range.start(), 0)
        };
        if start_frame >= u64::from(frames) || source_frame >= u64::from(sample.frames()) {
            continue;
        }
        let gate_frames = event.range().end().saturating_sub(event.range().start());
        mix_sample_offset(
            &mut block,
            start_frame as u32,
            sample,
            source_frame as u32,
            event.velocity(),
            gate_frames.saturating_sub(source_frame),
        );
    }

    block
}

/// Render an independently cacheable chunk while preserving sample tails from
/// events that began before the chunk boundary.
pub fn render_pattern_samples_chunk(
    pattern: &Pattern,
    tempo: Tempo,
    range: FrameRange,
    probability_seed: ProbabilitySeed,
    settings: RenderSettings,
    samples_by_note: &BTreeMap<u8, SampleBuffer>,
) -> AudioBlock {
    let lookbehind = samples_by_note
        .values()
        .map(SampleBuffer::frames)
        .max()
        .unwrap_or(0);
    let expanded_start = range.start().saturating_sub(u64::from(lookbehind));
    let expanded_range = FrameRange::new(expanded_start, range.end())
        .expect("chunk expansion preserves a valid frame range");
    let expanded = render_pattern_samples(
        pattern,
        tempo,
        expanded_range,
        probability_seed,
        settings,
        samples_by_note,
    );
    let frames = range
        .end()
        .saturating_sub(range.start())
        .min(u64::from(u32::MAX)) as u32;
    let offset_frames = range.start().saturating_sub(expanded_start) as usize;
    let channels = usize::from(settings.channels());
    let start = offset_frames * channels;
    let end = start + frames as usize * channels;
    let mut chunk = AudioBlock::silent(settings.channels(), frames);
    chunk
        .samples_mut()
        .copy_from_slice(&expanded.samples()[start..end]);
    chunk
}

pub fn render_pattern_samples_with_event_gain<F>(
    pattern: &Pattern,
    tempo: Tempo,
    range: FrameRange,
    probability_seed: ProbabilitySeed,
    settings: RenderSettings,
    samples_by_note: &BTreeMap<u8, SampleBuffer>,
    mut event_gain: F,
) -> AudioBlock
where
    F: FnMut(&Event) -> Sample,
{
    let frames = range
        .end()
        .saturating_sub(range.start())
        .min(u64::from(u32::MAX)) as u32;
    let mut block = AudioBlock::silent(settings.channels(), frames);
    let mut events = Vec::new();
    pattern.events_between(tempo, range, probability_seed, &mut events);

    for event in events {
        let Some(sample) = samples_by_note.get(&event.note()) else {
            continue;
        };
        let Some(relative_frame) = event.range().start().checked_sub(range.start()) else {
            continue;
        };
        if relative_frame >= u64::from(frames) {
            continue;
        }

        mix_sample(
            &mut block,
            relative_frame as u32,
            sample,
            event.velocity() * event_gain(&event),
            event.range().end().saturating_sub(event.range().start()),
        );
    }

    block
}

pub fn render_pattern_samples_cached(
    cache: &mut ArtifactCache,
    pattern: &Pattern,
    tempo: Tempo,
    range: FrameRange,
    probability_seed: ProbabilitySeed,
    settings: RenderSettings,
    samples_by_note: &BTreeMap<u8, SampleBuffer>,
) -> CachedArtifact {
    let key = pattern_sample_artifact_key(
        pattern,
        tempo,
        range,
        probability_seed,
        settings,
        samples_by_note,
    );
    if let Some(block) = cache.get(key) {
        return CachedArtifact {
            key,
            block: block.clone(),
            status: CacheStatus::Hit,
        };
    }

    let block = render_pattern_samples(
        pattern,
        tempo,
        range,
        probability_seed,
        settings,
        samples_by_note,
    );
    cache.insert(key, block.clone());
    CachedArtifact {
        key,
        block,
        status: CacheStatus::Miss,
    }
}

fn write_frame(block: &mut AudioBlock, frame: u32, sample: Sample) {
    let channels = usize::from(block.channels());
    let start = frame as usize * channels;

    for channel_offset in 0..channels {
        block.samples_mut()[start + channel_offset] += sample;
    }
}

fn mix_sample(
    block: &mut AudioBlock,
    start_frame: u32,
    sample: &SampleBuffer,
    gain: Sample,
    gate_frames: u64,
) {
    mix_sample_offset(block, start_frame, sample, 0, gain, gate_frames);
}

fn mix_sample_offset(
    block: &mut AudioBlock,
    start_frame: u32,
    sample: &SampleBuffer,
    source_frame: u32,
    gain: Sample,
    gate_frames: u64,
) {
    let out_channels = usize::from(block.channels());
    let sample_channels = usize::from(sample.channels());
    let available_frames = block.frames().saturating_sub(start_frame);
    let frames_to_mix = available_frames
        .min(sample.frames().saturating_sub(source_frame))
        .min(gate_frames.min(u64::from(u32::MAX)) as u32);

    for frame in 0..frames_to_mix {
        for out_channel in 0..out_channels {
            let source_channel = out_channel.min(sample_channels.saturating_sub(1));
            let source_index = (source_frame + frame) as usize * sample_channels + source_channel;
            let target_index = (start_frame + frame) as usize * out_channels + out_channel;
            block.samples_mut()[target_index] += sample.samples()[source_index] * gain;
        }
    }
}

fn sample_signature(sample: &SampleBuffer) -> u64 {
    let mut state = FingerprintBuilder::new();
    for sample in sample.samples() {
        state.write_u64(sample.to_bits());
    }

    state.finish().raw()
}

fn write_event_fingerprint(state: &mut FingerprintBuilder, events: &[Event]) {
    state.write_u64(events.len() as u64);
    for event in events {
        state.write_u64(event.pattern().raw());
        state.write_u64(event.track().raw());
        state.write_u64(u64::from(event.step().raw()));
        state.write_u64(event.range().start());
        state.write_u64(event.range().end());
        state.write_u64(u64::from(event.note()));
        state.write_u64(event.velocity().to_bits());
        state.write_u64(event.tags().len() as u64);
        for tag in event.tags() {
            state.write_u64(*tag as u64);
        }
    }
}

struct FingerprintBuilder {
    state: u64,
}

impl FingerprintBuilder {
    fn new() -> Self {
        Self {
            state: 0xcbf2_9ce4_8422_2325,
        }
    }

    fn write_u64(&mut self, value: u64) {
        self.state ^= mix_u64(value);
        self.state = self.state.wrapping_mul(0x0000_0100_0000_01b3);
    }

    fn finish(self) -> Fingerprint {
        Fingerprint::new(mix_u64(self.state))
    }
}

fn mix_u64(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9e37_79b9_7f4a_7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;
    use meldritch_core::{
        ArrangementSection, AutomationPoint, AutomationTarget, PatternId, SceneId, Step, StepIndex,
        TrackId,
    };
    use std::sync::Barrier;

    fn tempo() -> Tempo {
        Tempo::new(120.0, 48_000).unwrap()
    }

    #[test]
    fn renders_pattern_events_into_audio_block() {
        let mut pattern = Pattern::new(PatternId::new(1), 16, 4).unwrap();
        pattern
            .set_step(
                TrackId::new(1),
                StepIndex::new(0),
                Step::new(36).with_velocity(0.75),
            )
            .unwrap();
        pattern
            .set_step(
                TrackId::new(1),
                StepIndex::new(4),
                Step::new(38).with_velocity(0.5),
            )
            .unwrap();

        let block = render_pattern_clicks(
            &pattern,
            tempo(),
            FrameRange::new(0, 30_000).unwrap(),
            ProbabilitySeed::new(0),
            RenderSettings::new(2).unwrap(),
        );

        assert_eq!(block.channels(), 2);
        assert_eq!(block.frames(), 30_000);
        assert_eq!(block.samples()[0], 0.75);
        assert_eq!(block.samples()[1], 0.75);
        assert_eq!(block.samples()[48_000], 0.5);
        assert_eq!(block.samples()[48_001], 0.5);
        assert!(block.samples().iter().all(|sample| sample.is_finite()));
    }

    #[test]
    fn renders_pattern_events_with_sample_lookup() {
        let mut pattern = Pattern::new(PatternId::new(1), 16, 4).unwrap();
        pattern
            .set_step(
                TrackId::new(1),
                StepIndex::new(0),
                Step::new(36).with_velocity(0.5),
            )
            .unwrap();
        let mut samples_by_note = BTreeMap::new();
        samples_by_note.insert(36, SampleBuffer::new(1, 48_000, vec![1.0, 0.5]));

        let block = render_pattern_samples(
            &pattern,
            tempo(),
            FrameRange::new(0, 4).unwrap(),
            ProbabilitySeed::new(0),
            RenderSettings::new(2).unwrap(),
            &samples_by_note,
        );

        assert_eq!(block.samples(), &[0.5, 0.5, 0.25, 0.25, 0.0, 0.0, 0.0, 0.0]);
    }

    #[test]
    fn sample_chunk_preserves_tail_from_an_earlier_event() {
        let mut pattern = Pattern::new(PatternId::new(1), 4, 4).unwrap();
        pattern
            .set_step(TrackId::new(1), StepIndex::new(0), Step::new(36))
            .unwrap();
        let mut samples = BTreeMap::new();
        samples.insert(36, SampleBuffer::new(1, 48_000, vec![1.0; 7_000]));

        let block = render_pattern_samples_chunk(
            &pattern,
            tempo(),
            FrameRange::new(3_000, 5_000).unwrap(),
            ProbabilitySeed::new(1),
            RenderSettings::new(1).unwrap(),
            &samples,
        );

        assert!(block.samples().iter().all(|sample| *sample == 1.0));
    }

    #[test]
    fn arrangement_sample_render_crosses_section_boundaries() {
        let mut first = Pattern::new(PatternId::new(1), 4, 4).unwrap();
        first
            .set_step(TrackId::new(1), StepIndex::new(0), Step::new(36))
            .unwrap();
        let mut second = Pattern::new(PatternId::new(2), 4, 4).unwrap();
        second
            .set_step(TrackId::new(1), StepIndex::new(0), Step::new(38))
            .unwrap();
        let arrangement = Arrangement::new(vec![
            ArrangementSection::new(first, 1, SceneId::new(1)).unwrap(),
            ArrangementSection::new(second, 1, SceneId::new(2)).unwrap(),
        ])
        .unwrap();
        let mut samples = BTreeMap::new();
        samples.insert(36, SampleBuffer::new(1, 48_000, vec![0.25]));
        samples.insert(38, SampleBuffer::new(1, 48_000, vec![0.75]));
        let boundary = 24_000;

        let block = render_arrangement_samples(
            &arrangement,
            tempo(),
            FrameRange::new(0, boundary + 1).unwrap(),
            ProbabilitySeed::new(1),
            RenderSettings::new(1).unwrap(),
            &samples,
        );

        assert_eq!(block.samples()[0], 0.25);
        assert_eq!(block.samples()[boundary as usize], 0.75);
    }

    #[test]
    fn arrangement_artifact_key_tracks_used_audio_only() {
        let mut pattern = Pattern::new(PatternId::new(1), 4, 4).unwrap();
        pattern
            .set_step(TrackId::new(1), StepIndex::new(0), Step::new(36))
            .unwrap();
        let arrangement = Arrangement::new(vec![
            ArrangementSection::new(pattern, 1, SceneId::new(1)).unwrap(),
        ])
        .unwrap();
        let range = FrameRange::new(0, 24_000).unwrap();
        let settings = RenderSettings::new(1).unwrap();
        let mut samples = BTreeMap::new();
        samples.insert(36, SampleBuffer::new(1, 48_000, vec![0.25]));
        samples.insert(99, SampleBuffer::new(1, 48_000, vec![0.5]));
        let base = arrangement_sample_artifact_key(
            &arrangement,
            tempo(),
            range,
            ProbabilitySeed::new(1),
            settings,
            &samples,
        );
        samples.insert(99, SampleBuffer::new(1, 48_000, vec![0.9]));
        let unused_changed = arrangement_sample_artifact_key(
            &arrangement,
            tempo(),
            range,
            ProbabilitySeed::new(1),
            settings,
            &samples,
        );
        samples.insert(36, SampleBuffer::new(1, 48_000, vec![0.9]));
        let used_changed = arrangement_sample_artifact_key(
            &arrangement,
            tempo(),
            range,
            ProbabilitySeed::new(1),
            settings,
            &samples,
        );

        assert_eq!(base, unused_changed);
        assert_ne!(base, used_changed);
        assert_eq!(base.sample_rate(), 48_000);
    }

    #[test]
    fn automation_fingerprint_uses_only_points_affecting_the_chunk() {
        let lane = |last_value| {
            AutomationLane::new(
                AutomationTarget::Cutoff,
                AutomationInterpolation::Linear,
                vec![
                    AutomationPoint {
                        frame: 0,
                        value: AutomationValue::Continuous(200.0),
                    },
                    AutomationPoint {
                        frame: 100,
                        value: AutomationValue::Continuous(1_000.0),
                    },
                    AutomationPoint {
                        frame: 200,
                        value: AutomationValue::Continuous(last_value),
                    },
                ],
            )
            .unwrap()
        };
        let early = FrameRange::new(0, 50).unwrap();
        let late = FrameRange::new(120, 150).unwrap();

        assert_eq!(
            automation_fingerprint(&[lane(2_000.0)], early),
            automation_fingerprint(&[lane(9_000.0)], early)
        );
        assert_ne!(
            automation_fingerprint(&[lane(2_000.0)], late),
            automation_fingerprint(&[lane(9_000.0)], late)
        );
    }

    #[test]
    fn sample_render_respects_step_gate() {
        let mut pattern = Pattern::new(PatternId::new(1), 16, 4).unwrap();
        pattern
            .set_step(
                TrackId::new(1),
                StepIndex::new(0),
                Step::new(36).with_gate(0.0005),
            )
            .unwrap();
        let mut samples = BTreeMap::new();
        samples.insert(36, SampleBuffer::new(1, 48_000, vec![1.0; 8]));

        let block = render_pattern_samples(
            &pattern,
            tempo(),
            FrameRange::new(0, 8).unwrap(),
            ProbabilitySeed::new(1),
            RenderSettings::new(1).unwrap(),
            &samples,
        );

        assert_eq!(block.samples(), &[1.0, 1.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0]);
    }

    #[test]
    fn sample_render_respects_never_probability() {
        let mut pattern = Pattern::new(PatternId::new(1), 16, 4).unwrap();
        pattern
            .set_step(
                TrackId::new(1),
                StepIndex::new(0),
                Step::new(36).with_probability(meldritch_core::Probability::NEVER),
            )
            .unwrap();
        let mut samples = BTreeMap::new();
        samples.insert(36, SampleBuffer::new(1, 48_000, vec![1.0; 8]));

        let block = render_pattern_samples(
            &pattern,
            tempo(),
            FrameRange::new(0, 8).unwrap(),
            ProbabilitySeed::new(1),
            RenderSettings::new(1).unwrap(),
            &samples,
        );

        assert!(block.samples().iter().all(|sample| *sample == 0.0));
    }

    #[test]
    fn renders_pattern_events_with_event_gain() {
        let mut pattern = Pattern::new(PatternId::new(1), 16, 4).unwrap();
        pattern
            .set_step(
                TrackId::new(1),
                StepIndex::new(0),
                Step::new(36).with_velocity(0.5),
            )
            .unwrap();
        let mut samples_by_note = BTreeMap::new();
        samples_by_note.insert(36, SampleBuffer::new(1, 48_000, vec![1.0, 0.5]));

        let block = render_pattern_samples_with_event_gain(
            &pattern,
            tempo(),
            FrameRange::new(0, 4).unwrap(),
            ProbabilitySeed::new(0),
            RenderSettings::new(2).unwrap(),
            &samples_by_note,
            |_| 0.25,
        );

        assert_eq!(
            block.samples(),
            &[0.125, 0.125, 0.0625, 0.0625, 0.0, 0.0, 0.0, 0.0]
        );
    }

    #[test]
    fn render_is_deterministic_for_same_seed() {
        let mut pattern = Pattern::new(PatternId::new(1), 4, 4).unwrap();
        pattern
            .set_step(TrackId::new(1), StepIndex::new(0), Step::new(36))
            .unwrap();

        let first = render_pattern_clicks(
            &pattern,
            tempo(),
            FrameRange::new(0, 24_000).unwrap(),
            ProbabilitySeed::new(9),
            RenderSettings::new(1).unwrap(),
        );
        let second = render_pattern_clicks(
            &pattern,
            tempo(),
            FrameRange::new(0, 24_000).unwrap(),
            ProbabilitySeed::new(9),
            RenderSettings::new(1).unwrap(),
        );

        assert_eq!(first, second);
    }

    #[test]
    fn rejects_zero_channels() {
        assert_eq!(
            RenderSettings::new(0),
            Err(RenderSettingsError::InvalidChannelCount(0))
        );
    }

    #[test]
    fn artifact_key_is_stable_for_same_render_inputs() {
        let pattern = Pattern::new(PatternId::new(1), 16, 4).unwrap();
        let key = pattern_click_artifact_key(
            &pattern,
            tempo(),
            FrameRange::new(0, 96_000).unwrap(),
            ProbabilitySeed::new(11),
            RenderSettings::new(2).unwrap(),
        );
        let same = pattern_click_artifact_key(
            &pattern,
            tempo(),
            FrameRange::new(0, 96_000).unwrap(),
            ProbabilitySeed::new(11),
            RenderSettings::new(2).unwrap(),
        );

        assert_eq!(key, same);
    }

    #[test]
    fn artifact_key_changes_for_semantic_render_inputs() {
        let pattern = Pattern::new(PatternId::new(1), 16, 4).unwrap();
        let base = pattern_click_artifact_key(
            &pattern,
            tempo(),
            FrameRange::new(0, 96_000).unwrap(),
            ProbabilitySeed::new(11),
            RenderSettings::new(2).unwrap(),
        );
        let changed_seed = pattern_click_artifact_key(
            &pattern,
            tempo(),
            FrameRange::new(0, 96_000).unwrap(),
            ProbabilitySeed::new(12),
            RenderSettings::new(2).unwrap(),
        );
        let changed_range = pattern_click_artifact_key(
            &pattern,
            tempo(),
            FrameRange::new(0, 48_000).unwrap(),
            ProbabilitySeed::new(11),
            RenderSettings::new(2).unwrap(),
        );

        assert_ne!(base.fingerprint(), changed_seed.fingerprint());
        assert_ne!(base.fingerprint(), changed_range.fingerprint());
    }

    #[test]
    fn artifact_key_changes_when_event_content_changes() {
        let empty = Pattern::new(PatternId::new(1), 16, 4).unwrap();
        let mut changed = empty.clone();
        changed
            .set_step(TrackId::new(1), StepIndex::new(0), Step::new(36))
            .unwrap();
        let range = FrameRange::new(0, 6_000).unwrap();

        let empty_key = pattern_sample_artifact_key(
            &empty,
            tempo(),
            range,
            ProbabilitySeed::new(1),
            RenderSettings::new(1).unwrap(),
            &BTreeMap::new(),
        );
        let changed_key = pattern_sample_artifact_key(
            &changed,
            tempo(),
            range,
            ProbabilitySeed::new(1),
            RenderSettings::new(1).unwrap(),
            &BTreeMap::new(),
        );

        assert_ne!(empty_key, changed_key);
    }

    #[test]
    fn cached_click_render_reports_miss_then_hit() {
        let pattern = Pattern::new(PatternId::new(1), 16, 4).unwrap();
        let mut cache = ArtifactCache::new();

        let first = render_pattern_clicks_cached(
            &mut cache,
            &pattern,
            tempo(),
            FrameRange::new(0, 96_000).unwrap(),
            ProbabilitySeed::new(11),
            RenderSettings::new(2).unwrap(),
        );
        let second = render_pattern_clicks_cached(
            &mut cache,
            &pattern,
            tempo(),
            FrameRange::new(0, 96_000).unwrap(),
            ProbabilitySeed::new(11),
            RenderSettings::new(2).unwrap(),
        );

        assert_eq!(first.status(), CacheStatus::Miss);
        assert_eq!(second.status(), CacheStatus::Hit);
        assert_eq!(cache.len(), 1);
        assert_eq!(first.block(), second.block());
    }

    #[test]
    fn sample_artifact_key_changes_when_sample_content_changes() {
        let mut pattern = Pattern::new(PatternId::new(1), 16, 4).unwrap();
        pattern
            .set_step(TrackId::new(1), StepIndex::new(0), Step::new(36))
            .unwrap();
        let mut first_samples = BTreeMap::new();
        first_samples.insert(36, SampleBuffer::new(1, 48_000, vec![1.0, 0.5]));
        let mut changed_samples = BTreeMap::new();
        changed_samples.insert(36, SampleBuffer::new(1, 48_000, vec![1.0, 0.25]));

        let first = pattern_sample_artifact_key(
            &pattern,
            tempo(),
            FrameRange::new(0, 96_000).unwrap(),
            ProbabilitySeed::new(11),
            RenderSettings::new(2).unwrap(),
            &first_samples,
        );
        let changed = pattern_sample_artifact_key(
            &pattern,
            tempo(),
            FrameRange::new(0, 96_000).unwrap(),
            ProbabilitySeed::new(11),
            RenderSettings::new(2).unwrap(),
            &changed_samples,
        );

        assert_ne!(first.fingerprint(), changed.fingerprint());
    }
    fn artifact_key(raw: u64) -> ArtifactKey {
        ArtifactKey {
            pattern: PatternId::new(raw),
            range: FrameRange::new(raw, raw + 1).unwrap(),
            sample_rate: 48_000,
            fingerprint: Fingerprint::new(raw),
        }
    }

    fn ranged_artifact_key(start: u64, end: u64) -> ArtifactKey {
        ArtifactKey {
            pattern: PatternId::new(1),
            range: FrameRange::new(start, end).unwrap(),
            sample_rate: 48_000,
            fingerprint: Fingerprint::new(start),
        }
    }

    fn ranged_artifact_key_with_fingerprint(start: u64, end: u64, fingerprint: u64) -> ArtifactKey {
        ArtifactKey {
            fingerprint: Fingerprint::new(fingerprint),
            ..ranged_artifact_key(start, end)
        }
    }

    #[test]
    fn render_job_queue_runs_hot_jobs_before_cold_jobs() {
        let mut queue = RenderJobQueue::default();
        queue.push(
            artifact_key(1),
            RenderPriority::Cold,
            Box::new(|| AudioBlock::silent(1, 1)),
        );
        queue.push(
            artifact_key(2),
            RenderPriority::Hot,
            Box::new(|| AudioBlock::silent(1, 1)),
        );
        queue.push(
            artifact_key(3),
            RenderPriority::Warm,
            Box::new(|| AudioBlock::silent(1, 1)),
        );
        queue.push(
            artifact_key(4),
            RenderPriority::Hot,
            Box::new(|| AudioBlock::silent(1, 1)),
        );

        assert_eq!(queue.pop().unwrap().key, artifact_key(2));
        assert_eq!(queue.pop().unwrap().key, artifact_key(4));
        assert_eq!(queue.pop().unwrap().key, artifact_key(3));
        assert_eq!(queue.pop().unwrap().key, artifact_key(1));
        assert!(queue.pop().is_none());
    }

    #[test]
    fn render_worker_pool_completes_jobs_into_cache() {
        let pool = RenderWorkerPool::new(1).unwrap();
        let cold_key = artifact_key(11);
        let hot_key = artifact_key(12);

        pool.submit(cold_key, RenderPriority::Cold, || AudioBlock::silent(1, 2));
        pool.submit(hot_key, RenderPriority::Hot, || AudioBlock::silent(2, 3));
        pool.wait_until_idle();

        assert_eq!(pool.cache_len(), 2);
        assert_eq!(pool.cached_artifact(cold_key).unwrap().frames(), 2);
        assert_eq!(pool.cached_artifact(hot_key).unwrap().channels(), 2);
        assert_eq!(
            pool.diagnostics(),
            WorkerDiagnostics {
                queued_jobs: 0,
                active_jobs: 0,
                completed_jobs: 2,
            }
        );
    }

    #[test]
    fn render_worker_pool_rejects_zero_workers() {
        assert!(matches!(
            RenderWorkerPool::new(0),
            Err(RenderWorkerPoolError::ZeroWorkers)
        ));
    }

    #[test]
    fn worker_pool_deduplicates_wanted_artifacts() {
        let pool = RenderWorkerPool::new(1).unwrap();
        let key = artifact_key(20);
        let barrier = Arc::new(Barrier::new(2));
        let worker_barrier = Arc::clone(&barrier);
        assert!(pool.submit_if_needed(key, RenderPriority::Hot, move || {
            worker_barrier.wait();
            AudioBlock::silent(1, 1)
        }));
        assert!(!pool.submit_if_needed(key, RenderPriority::Cold, || { AudioBlock::silent(1, 1) }));
        barrier.wait();
        pool.wait_until_idle();

        assert_eq!(pool.diagnostics().completed_jobs, 1);
        assert!(pool.cached_artifact(key).is_some());
    }

    #[test]
    fn worker_pool_invalidates_overlapping_cached_artifacts() {
        let pool = RenderWorkerPool::new(1).unwrap();
        let first = ranged_artifact_key(0, 2);
        let second = ranged_artifact_key(2, 4);
        pool.submit(first, RenderPriority::Hot, || AudioBlock::silent(1, 2));
        pool.submit(second, RenderPriority::Warm, || AudioBlock::silent(1, 2));
        pool.wait_until_idle();

        assert_eq!(
            pool.invalidate_range(PatternId::new(1), FrameRange::new(1, 2).unwrap()),
            1
        );
        assert!(pool.cached_artifact(first).is_none());
        assert!(pool.cached_artifact(second).is_some());
    }

    #[test]
    fn worker_completions_publish_ready_realtime_chunks() {
        let publication = RealtimeChunkPublication::new(1, 4, 2).unwrap();
        let reader = publication.reader();
        let pool =
            RenderWorkerPool::with_completion_hook(2, publication.completion_hook()).unwrap();
        let first_key = ranged_artifact_key(0, 2);
        let second_key = ranged_artifact_key(2, 4);
        publication.expect_artifact(first_key).unwrap();
        publication.expect_artifact(second_key).unwrap();
        pool.submit(first_key, RenderPriority::Hot, || {
            let mut block = AudioBlock::silent(1, 2);
            block.samples_mut().fill(0.25);
            block
        });
        pool.submit(second_key, RenderPriority::Hot, || {
            let mut block = AudioBlock::silent(1, 2);
            block.samples_mut().fill(0.75);
            block
        });
        pool.wait_until_idle();

        let snapshot = reader.snapshot();
        assert_eq!(snapshot.frame(0), Ok([0.25].as_slice()));
        assert_eq!(snapshot.frame(3), Ok([0.75].as_slice()));
        assert_eq!(
            publication.diagnostics(),
            ChunkPublicationDiagnostics {
                ready_chunks: 2,
                published_artifacts: 2,
                rejected_artifacts: 0,
                invalidated_chunks: 0,
                stale_artifacts: 0,
            }
        );
    }

    #[test]
    fn chunk_publication_rejects_misaligned_worker_results() {
        let publication = RealtimeChunkPublication::new(1, 4, 2).unwrap();
        let block = AudioBlock::silent(1, 1);

        assert_eq!(
            publication.publish_artifact(ranged_artifact_key(1, 2), &block),
            Err(ChunkPublicationError::MisalignedRange)
        );
        assert_eq!(publication.diagnostics().rejected_artifacts, 1);
        assert!(publication.reader().snapshot().frame(0).is_err());
    }

    #[test]
    fn invalidation_removes_dirty_chunk_and_rejects_superseded_completion() {
        let publication = RealtimeChunkPublication::new(1, 4, 2).unwrap();
        let old_key = ranged_artifact_key_with_fingerprint(0, 2, 1);
        let new_key = ranged_artifact_key_with_fingerprint(0, 2, 2);
        let block = AudioBlock::silent(1, 2);
        publication.expect_artifact(old_key).unwrap();
        publication.publish_artifact(old_key, &block).unwrap();

        assert_eq!(
            publication
                .invalidate_range(FrameRange::new(0, 1).unwrap())
                .unwrap(),
            1
        );
        assert!(publication.reader().snapshot().frame(0).is_err());
        publication.expect_artifact(new_key).unwrap();
        assert_eq!(
            publication.publish_artifact(old_key, &block),
            Err(ChunkPublicationError::UnexpectedArtifact)
        );
        publication.publish_artifact(new_key, &block).unwrap();

        let diagnostics = publication.diagnostics();
        assert_eq!(diagnostics.ready_chunks, 1);
        assert_eq!(diagnostics.invalidated_chunks, 1);
        assert_eq!(diagnostics.stale_artifacts, 1);
        assert_eq!(diagnostics.rejected_artifacts, 1);
    }

    #[test]
    fn transformed_override_swaps_atomically_and_restores_live_chunks() {
        let publication = RealtimeChunkPublication::new(1, 4, 2).unwrap();
        let first = ranged_artifact_key(0, 2);
        let second = ranged_artifact_key(2, 4);
        publication.expect_artifact(first).unwrap();
        publication.expect_artifact(second).unwrap();
        let mut live_first = AudioBlock::silent(1, 2);
        live_first.samples_mut().fill(0.25);
        let mut live_second = AudioBlock::silent(1, 2);
        live_second.samples_mut().fill(0.75);
        publication.publish_artifact(first, &live_first).unwrap();
        publication.publish_artifact(second, &live_second).unwrap();
        let mut transformed = AudioBlock::silent(1, 4);
        transformed.samples_mut().fill(0.9);

        publication.publish_override(&transformed).unwrap();
        assert_eq!(
            publication.reader().snapshot().frame(0),
            Ok([0.9].as_slice())
        );
        publication.restore_live_snapshot().unwrap();
        assert_eq!(
            publication.reader().snapshot().frame(0),
            Ok([0.25].as_slice())
        );
        assert_eq!(
            publication.reader().snapshot().frame(3),
            Ok([0.75].as_slice())
        );
    }

    #[test]
    fn horizon_plan_wraps_forward_from_the_playhead() {
        let pattern = Pattern::new(PatternId::new(1), 4, 4).unwrap();
        let samples = BTreeMap::new();
        let horizon = RenderHorizon::new(5, 8, 2, 1).unwrap();

        let plan = plan_sample_render_horizon(
            &pattern,
            tempo(),
            ProbabilitySeed::new(1),
            RenderSettings::new(1).unwrap(),
            &samples,
            horizon,
        )
        .unwrap();

        assert_eq!(
            plan.iter()
                .map(|chunk| (chunk.range.start(), chunk.range.end(), chunk.priority))
                .collect::<Vec<_>>(),
            vec![
                (4, 6, RenderPriority::Hot),
                (6, 8, RenderPriority::Warm),
                (0, 2, RenderPriority::Cold),
                (2, 4, RenderPriority::Cold),
            ]
        );
    }

    #[test]
    fn horizon_submission_renders_misses_and_publishes_cache_hits() {
        let pattern = Pattern::new(PatternId::new(1), 4, 4).unwrap();
        let samples = Arc::new(BTreeMap::new());
        let publication = RealtimeChunkPublication::new(1, 4, 2).unwrap();
        let pool =
            RenderWorkerPool::with_completion_hook(2, publication.completion_hook()).unwrap();
        let horizon = RenderHorizon::new(0, 4, 2, 1).unwrap();
        let settings = RenderSettings::new(1).unwrap();

        let first = submit_sample_render_horizon(
            &pool,
            &publication,
            &pattern,
            tempo(),
            ProbabilitySeed::new(1),
            settings,
            Arc::clone(&samples),
            horizon,
        )
        .unwrap();
        assert_eq!(first.planned_chunks, 2);
        assert_eq!(first.cache_hits, 0);
        assert_eq!(first.submitted_jobs, 2);
        pool.wait_until_idle();

        let second = submit_sample_render_horizon(
            &pool,
            &publication,
            &pattern,
            tempo(),
            ProbabilitySeed::new(1),
            settings,
            samples,
            horizon,
        )
        .unwrap();
        assert_eq!(second.cache_hits, 2);
        assert_eq!(second.submitted_jobs, 0);
        assert_eq!(publication.diagnostics().ready_chunks, 2);
        assert!(publication.reader().snapshot().frame(3).is_ok());
    }
}
