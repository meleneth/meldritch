//! Headless offline rendering foundations.
//!
//! This crate starts with a deliberately small deterministic event renderer. It
//! is not a sampler yet; it proves that scheduled core events can become finite
//! internal `f64` audio blocks without touching device I/O.

use meldritch_audio::{AudioBlock, SampleBuffer};
use meldritch_core::{
    Event, FrameRange, Pattern, PatternId, ProbabilitySeed, Sample, SampleRate, Tempo,
};
use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::collections::BinaryHeap;
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
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RenderPriority {
    Hot,
    Warm,
    Cold,
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
}

pub struct RenderWorkerPool {
    shared: Arc<WorkerShared>,
    workers: Vec<JoinHandle<()>>,
}

impl RenderWorkerPool {
    pub fn new(worker_count: usize) -> Result<Self, RenderWorkerPoolError> {
        if worker_count == 0 {
            return Err(RenderWorkerPoolError::ZeroWorkers);
        }

        let shared = Arc::new(WorkerShared::default());
        let workers = (0..worker_count)
            .map(|_| spawn_render_worker(Arc::clone(&shared)))
            .collect();

        Ok(Self { shared, workers })
    }

    pub fn submit<F>(&self, key: ArtifactKey, priority: RenderPriority, task: F)
    where
        F: FnOnce() -> AudioBlock + Send + 'static,
    {
        let mut state = self
            .shared
            .state
            .lock()
            .expect("worker state lock poisoned");
        state.queue.push(key, priority, Box::new(task));
        self.shared.has_work.notify_one();
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
            shared
                .cache
                .lock()
                .expect("artifact cache lock poisoned")
                .insert(job.key, block);

            let mut state = shared.state.lock().expect("worker state lock poisoned");
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

    for (note, sample) in samples_by_note {
        state.write_u64(u64::from(*note));
        state.write_u64(u64::from(sample.channels()));
        state.write_u64(u64::from(sample.sample_rate()));
        state.write_u64(u64::from(sample.frames()));
        state.write_u64(sample_signature(sample));
    }

    ArtifactKey {
        pattern: pattern.id(),
        range,
        sample_rate: tempo.sample_rate(),
        fingerprint: state.finish(),
    }
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

        mix_sample(&mut block, relative_frame as u32, sample, event.velocity());
    }

    block
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

fn mix_sample(block: &mut AudioBlock, start_frame: u32, sample: &SampleBuffer, gain: Sample) {
    let out_channels = usize::from(block.channels());
    let sample_channels = usize::from(sample.channels());
    let available_frames = block.frames().saturating_sub(start_frame);
    let frames_to_mix = available_frames.min(sample.frames());

    for frame in 0..frames_to_mix {
        for out_channel in 0..out_channels {
            let source_channel = out_channel.min(sample_channels.saturating_sub(1));
            let source_index = frame as usize * sample_channels + source_channel;
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
    use meldritch_core::{PatternId, Step, StepIndex, TrackId};

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
        let pattern = Pattern::new(PatternId::new(1), 16, 4).unwrap();
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
}
