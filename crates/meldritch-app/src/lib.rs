//! Headless application state and command dispatch for UI frontends.

use meldritch_audio::device_output::PlaybackControl;
use meldritch_audio::realtime_queue::{QueueDiagnostics, QueueFull};
use meldritch_audio::realtime_status::RealtimeStatusSnapshot;
use meldritch_audio::transport::TransportState;
use meldritch_core::{
    Arrangement, AutomationInterpolation, AutomationLane, AutomationTarget, AutomationValue,
    DirtyRange, Frame, Pattern, PatternId, Probability, SceneId, Step, StepIndex, Tempo, TrackId,
};
use meldritch_render::coordinator::{RenderCoordinator, RenderCoordinatorDiagnostics};
use meldritch_render::dsp::{BassVoiceSettings, Waveform, synthesize_bass_sample};
use meldritch_render::dynamics::{DuckBands, SidechainRelation};
use meldritch_render::effects::{ActiveSendExplanation, EffectSendRule, explain_effect_sends};
pub use meldritch_render::futures::PerformanceGesture;
use meldritch_render::futures::{FutureCandidateInspection, FutureWorkerDiagnostics};
use meldritch_render::futures::{
    FutureWorkerPool, LaunchQuantization, PerformanceLaunch, PerformanceLauncher,
    PerformanceLauncherDiagnostics, QueuedPerformanceGesture, RenderableFuturePlan,
};
use meldritch_render::live_edit::{
    LiveEditCommand, LiveEditError, LiveEditResult, LivePatternEditor,
};
use meldritch_render::performance_fx::{PerformanceFxSettings, apply_performance_fx};
use meldritch_render::transforms::{
    ChunkTransform, ChunkTransformError, TransformArtifactCache, TransformArtifactKey,
    TransformCacheStatus,
};
use std::collections::VecDeque;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Condvar, Mutex, mpsc};
use std::thread::JoinHandle;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Selection {
    pub track: TrackId,
    pub step: StepIndex,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CockpitMode {
    Performance,
    AllParameters,
}

impl CockpitMode {
    #[must_use]
    pub const fn toggled(self) -> Self {
        match self {
            Self::Performance => Self::AllParameters,
            Self::AllParameters => Self::Performance,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum AppCommand {
    Play,
    Stop,
    Rewind,
    Select(Selection),
    Edit(LiveEditCommand),
    SetBassVoice(BassVoiceSettings),
    SetPerformanceFx(PerformanceFxSettings),
    Transform(ChunkTransform),
    AuditionTransform,
    ReturnToLive,
    QueueNextScene,
    QueueScene(SceneId),
    QueueSceneVariation(SceneId, usize),
    ToggleTrackMute(TrackId),
    TriggerFill(PatternId),
    CancelPerformance,
    SetCockpitMode(CockpitMode),
    AdjustCuratedControl { id: String, steps: i32 },
    SetCuratedControlNormalized { id: String, value: f64 },
}

#[derive(Clone, Debug, PartialEq)]
pub struct CommandRecord {
    pub sequence: u64,
    pub command: AppCommand,
    pub changed: bool,
    pub dirty_ranges: Vec<DirtyRange>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum AppCommandResult {
    TransportQueued,
    SelectionChanged {
        previous: Selection,
        current: Selection,
    },
    Edit(LiveEditResult),
    SynthUpdated {
        invalidated_chunks: usize,
    },
    PerformanceFxUpdated(PerformanceFxSettings),
    TransformCreated {
        key: TransformArtifactKey,
        status: TransformCacheStatus,
    },
    AudioSourceSwitched {
        transformed: bool,
    },
    PerformanceQueued(QueuedPerformanceGesture),
    PerformanceCancelled(Option<QueuedPerformanceGesture>),
    CockpitModeChanged {
        previous: CockpitMode,
        current: CockpitMode,
    },
    CuratedControlAdjusted {
        id: String,
        previous: f64,
        current: f64,
    },
}

#[derive(Debug)]
pub enum AppCommandError {
    QueueFull(QueueFull),
    LiveEdit(LiveEditError),
    NoStepSelected,
    NoBassSynth,
    NoChordLayer,
    NoChordSelected,
    Transform(ChunkTransformError),
    TransformSourceUnavailable,
    Publication(meldritch_render::ChunkPublicationError),
    NoPerformanceScenes,
    NoFillPattern,
    UnknownCuratedControl(String),
}

#[derive(Clone, Debug)]
pub struct AppDiagnostics {
    pub selection: Selection,
    pub history_len: usize,
    pub playback: RealtimeStatusSnapshot,
    pub transport_commands: QueueDiagnostics,
    pub render: RenderCoordinatorDiagnostics,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TransportView {
    pub state: TransportState,
    pub position: u32,
    pub callbacks: u64,
    pub underruns: u64,
    pub missed_artifacts: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ArrangementSectionView {
    pub index: usize,
    pub scene: SceneId,
    pub repeats: u32,
    pub active: bool,
    pub in_loop: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ArrangementView {
    pub sections: Vec<ArrangementSectionView>,
    pub active_repeat: Option<u32>,
    pub loop_sections: (usize, usize),
}

#[derive(Clone, Debug, PartialEq)]
pub struct AutomationLaneView {
    pub target: AutomationTarget,
    pub interpolation: AutomationInterpolation,
    pub current: AutomationValue,
    pub next_point: Option<(Frame, AutomationValue)>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AutomationView {
    pub lanes: Vec<AutomationLaneView>,
    pub scene: Option<i64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EffectSendView {
    pub recent: Vec<ActiveSendExplanation>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SidechainView {
    pub source_role: meldritch_core::SourceRole,
    pub target_role: meldritch_core::SourceRole,
    pub bands: DuckBands,
    pub attenuation: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TransformView {
    pub transform: ChunkTransform,
    pub key: TransformArtifactKey,
    pub status: TransformCacheStatus,
    pub channels: u16,
    pub frames: u32,
    pub auditioning: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FutureCacheView {
    pub diagnostics: FutureWorkerDiagnostics,
    pub candidates: Vec<FutureCandidateInspection>,
    pub unresolved: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PerformanceView {
    pub queued: Option<QueuedPerformanceGesture>,
    pub active_scene: Option<SceneId>,
    pub muted_tracks: Vec<TrackId>,
    pub active_fill: Option<PatternId>,
    pub fill_end_frame: Option<Frame>,
    pub diagnostics: PerformanceLauncherDiagnostics,
    pub learned_phrase_cues: Vec<LearnedPhraseCueView>,
    pub pages: Vec<PerformancePageView>,
    pub active_page: Option<usize>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LearnedPhraseCueView {
    pub scene: SceneId,
    pub frame: Frame,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PerformancePageView {
    pub id: String,
    pub label: String,
    pub strips: Vec<PerformanceStripView>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PerformanceStripView {
    pub strip: u8,
    pub lane_id: String,
    pub lane_label: String,
    pub lane_role: String,
    pub track_id: Option<String>,
    pub variation_ids: Vec<String>,
}

#[derive(Clone, Debug)]
struct ArrangementPresentation {
    arrangement: Arrangement,
    tempo: Tempo,
    loop_sections: (usize, usize),
}

#[derive(Clone, Debug, PartialEq)]
pub struct StepCellView {
    pub step: StepIndex,
    pub selected: bool,
    pub value: Option<Step>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TrackRowView {
    pub track: TrackId,
    pub selected: bool,
    pub steps: Vec<StepCellView>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PatternGridView {
    pub pattern: PatternId,
    pub length_steps: u32,
    pub tracks: Vec<TrackRowView>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StepInspectorView {
    pub selection: Selection,
    pub value: Option<Step>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CuratedControlView {
    pub id: String,
    pub label: String,
    pub target: String,
    pub minimum: f64,
    pub maximum: f64,
    pub step: f64,
    pub binding: String,
    pub value: Option<f64>,
}

#[derive(Clone, Debug)]
pub struct AppViewModel {
    pub cockpit_mode: CockpitMode,
    pub curated_controls: Vec<CuratedControlView>,
    pub transport: TransportView,
    pub arrangement: Option<ArrangementView>,
    pub automation: Option<AutomationView>,
    pub effect_sends: Option<EffectSendView>,
    pub sidechain: Option<SidechainView>,
    pub transform: Option<TransformView>,
    pub futures: Option<FutureCacheView>,
    pub performance: PerformanceView,
    pub pattern_grid: PatternGridView,
    pub inspector: StepInspectorView,
    pub diagnostics: AppDiagnostics,
    pub history: Vec<CommandRecord>,
    pub bass_voice: Option<BassVoiceSettings>,
    pub performance_fx: Option<PerformanceFxSettings>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum AppInput {
    ToggleCockpitMode,
    AdjustCuratedControl { id: String, steps: i32 },
    SetCuratedControlNormalized { id: String, value: f64 },
    MoveLeft,
    MoveRight,
    MoveUp,
    MoveDown,
    ToggleSelected(Step),
    TogglePlayback,
    Stop,
    Rewind,
    IncreaseVelocity,
    DecreaseVelocity,
    IncreaseGate,
    DecreaseGate,
    IncreaseProbability,
    DecreaseProbability,
    IncreaseCutoff,
    DecreaseCutoff,
    IncreaseResonance,
    DecreaseResonance,
    CycleWaveform,
    IncreaseFilterEnvelope,
    DecreaseFilterEnvelope,
    IncreaseDrive,
    DecreaseDrive,
    IncreaseSynthLevel,
    DecreaseSynthLevel,
    IncreaseAttack,
    DecreaseAttack,
    IncreaseDecay,
    DecreaseDecay,
    IncreaseSustain,
    DecreaseSustain,
    IncreaseRelease,
    DecreaseRelease,
    IncreaseSubLevel,
    DecreaseSubLevel,
    IncreaseGlide,
    DecreaseGlide,
    IncreaseDucking,
    DecreaseDucking,
    IncreaseDuckingRelease,
    DecreaseDuckingRelease,
    IncreaseHatFilter,
    DecreaseHatFilter,
    IncreaseHatFilterRelease,
    DecreaseHatFilterRelease,
    IncreaseNote,
    DecreaseNote,
    TransposeChordUp,
    TransposeChordDown,
    InvertChordUp,
    InvertChordDown,
    CreateReverse,
    CreateReslice,
    CreateFreeze,
    CreateSmear,
    AuditionTransform,
    ReturnToLive,
    QueueNextScene,
    QueuePhrase(SceneId),
    QueuePhraseVariation(SceneId, usize),
    ToggleTrackMute,
    TriggerFill,
    CancelPerformance,
    IncreaseDelayFeedback,
    DecreaseDelayFeedback,
    IncreasePhaserMix,
    DecreasePhaserMix,
    ToggleReverbFreeze,
    IncreaseModulationDepth,
    DecreaseModulationDepth,
    IncreaseMasterDrive,
    DecreaseMasterDrive,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MidiControlAction {
    Absolute,
    Centered { center: f64 },
    Overdrive { normal: f64, normal_midi: u8 },
    Decrement,
    Increment,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MidiControlBinding {
    pub control_id: String,
    pub device: String,
    pub channel: Option<u8>,
    pub cc: u8,
    pub minimum: f64,
    pub maximum: f64,
    pub action: MidiControlAction,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MidiControlInput {
    pub device: String,
    pub channel: u8,
    pub cc: u8,
    pub value: u8,
}

pub fn map_midi_control_input(
    bindings: &[MidiControlBinding],
    input: MidiControlInput,
) -> Option<AppInput> {
    bindings
        .iter()
        .find(|binding| {
            binding.device == input.device
                && binding.cc == input.cc
                && binding
                    .channel
                    .is_none_or(|channel| channel == input.channel)
        })
        .and_then(|binding| match binding.action {
            MidiControlAction::Absolute => Some(AppInput::SetCuratedControlNormalized {
                id: binding.control_id.clone(),
                value: f64::from(input.value.min(127)) / 127.0,
            }),
            MidiControlAction::Centered { center } => Some(AppInput::SetCuratedControlNormalized {
                id: binding.control_id.clone(),
                value: normalized_from_centered_midi(
                    input.value,
                    binding.minimum,
                    binding.maximum,
                    center,
                ),
            }),
            MidiControlAction::Overdrive {
                normal,
                normal_midi,
            } => Some(AppInput::SetCuratedControlNormalized {
                id: binding.control_id.clone(),
                value: normalized_from_overdrive_midi(
                    input.value,
                    binding.minimum,
                    binding.maximum,
                    normal,
                    normal_midi,
                ),
            }),
            MidiControlAction::Decrement if input.value != 0 => {
                Some(AppInput::AdjustCuratedControl {
                    id: binding.control_id.clone(),
                    steps: -1,
                })
            }
            MidiControlAction::Increment if input.value != 0 => {
                Some(AppInput::AdjustCuratedControl {
                    id: binding.control_id.clone(),
                    steps: 1,
                })
            }
            MidiControlAction::Decrement | MidiControlAction::Increment => None,
        })
}

fn normalized_from_centered_midi(value: u8, minimum: f64, maximum: f64, center: f64) -> f64 {
    let value = value.min(127);
    let center = center.clamp(minimum, maximum);
    let actual = if value <= 64 {
        let phase = f64::from(value) / 64.0;
        minimum + phase * (center - minimum)
    } else {
        let phase = f64::from(value - 64) / 63.0;
        center + phase * (maximum - center)
    };
    normalized_from_actual(actual, minimum, maximum)
}

fn normalized_from_overdrive_midi(
    value: u8,
    minimum: f64,
    maximum: f64,
    normal: f64,
    normal_midi: u8,
) -> f64 {
    let value = value.min(127);
    let normal = normal.clamp(minimum, maximum);
    let normal_midi = normal_midi.clamp(1, 126);
    let actual = if value <= normal_midi {
        let phase = f64::from(value) / f64::from(normal_midi);
        minimum + phase * (normal - minimum)
    } else {
        let phase = f64::from(value - normal_midi) / f64::from(127 - normal_midi);
        normal + phase * (maximum - normal)
    };
    normalized_from_actual(actual, minimum, maximum)
}

fn normalized_from_actual(actual: f64, minimum: f64, maximum: f64) -> f64 {
    if maximum <= minimum {
        return 0.0;
    }
    ((actual.clamp(minimum, maximum) - minimum) / (maximum - minimum)).clamp(0.0, 1.0)
}

struct BassSynthControl {
    settings: BassVoiceSettings,
    track: TrackId,
    notes: BTreeSet<u8>,
    sample_frames: u32,
}

struct FxWorkerState {
    pending: Option<(
        u64,
        Arc<meldritch_audio::AudioBlock>,
        Tempo,
        PerformanceFxSettings,
    )>,
    shutdown: bool,
}

struct PerformanceFxWorker {
    shared: Arc<(Mutex<FxWorkerState>, Condvar)>,
    completed: mpsc::Receiver<(u64, meldritch_audio::AudioBlock, PerformanceFxSettings)>,
    thread: Option<JoinHandle<()>>,
    generation: u64,
}

impl PerformanceFxWorker {
    fn new() -> Self {
        let shared = Arc::new((
            Mutex::new(FxWorkerState {
                pending: None,
                shutdown: false,
            }),
            Condvar::new(),
        ));
        let worker_shared = Arc::clone(&shared);
        let (sender, completed) = mpsc::channel();
        let thread = std::thread::spawn(move || {
            loop {
                let job = {
                    let (lock, changed) = &*worker_shared;
                    let mut state = lock.lock().expect("FX worker lock poisoned");
                    while state.pending.is_none() && !state.shutdown {
                        state = changed.wait(state).expect("FX worker wait poisoned");
                    }
                    if state.shutdown {
                        break;
                    }
                    state.pending.take()
                };
                if let Some((generation, source, tempo, settings)) = job {
                    let block = apply_performance_fx(&source, tempo, settings);
                    if sender.send((generation, block, settings)).is_err() {
                        break;
                    }
                }
            }
        });
        Self {
            shared,
            completed,
            thread: Some(thread),
            generation: 0,
        }
    }

    fn submit(
        &mut self,
        source: Arc<meldritch_audio::AudioBlock>,
        tempo: Tempo,
        settings: PerformanceFxSettings,
    ) -> u64 {
        self.generation = self.generation.wrapping_add(1);
        let generation = self.generation;
        let (lock, changed) = &*self.shared;
        lock.lock().expect("FX worker lock poisoned").pending =
            Some((generation, source, tempo, settings));
        changed.notify_one();
        generation
    }

    fn latest_completed(
        &self,
    ) -> Option<(u64, meldritch_audio::AudioBlock, PerformanceFxSettings)> {
        self.completed
            .try_iter()
            .max_by_key(|(generation, _, _)| *generation)
    }
}

impl Drop for PerformanceFxWorker {
    fn drop(&mut self) {
        let (lock, changed) = &*self.shared;
        lock.lock().expect("FX worker lock poisoned").shutdown = true;
        changed.notify_one();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

pub struct AppController {
    cockpit_mode: CockpitMode,
    curated_controls: Vec<CuratedControlView>,
    playback: PlaybackControl,
    coordinator: RenderCoordinator,
    editor: LivePatternEditor,
    selection: Selection,
    history: VecDeque<CommandRecord>,
    history_capacity: usize,
    next_sequence: u64,
    bass_synth: Option<BassSynthControl>,
    arrangement: Option<ArrangementPresentation>,
    automation: Vec<AutomationLane>,
    effect_rules: Vec<EffectSendRule>,
    sidechain: Option<SidechainRelation>,
    transform_cache: TransformArtifactCache,
    transform_view: Option<TransformView>,
    future_cache_view: Option<FutureCacheView>,
    performance_launcher: PerformanceLauncher,
    performance_pages: Vec<PerformancePageView>,
    active_performance_page: Option<usize>,
    performance_scenes: Vec<SceneId>,
    phrase_patterns: BTreeMap<SceneId, Vec<Pattern>>,
    queued_phrase_variation: Option<(SceneId, usize)>,
    learned_phrase_cues: Vec<LearnedPhraseCueView>,
    fill_pattern: Option<PatternId>,
    transformed_audio: Option<meldritch_audio::AudioBlock>,
    performance_fx: Option<PerformanceFxSettings>,
    performance_fx_source: Option<Arc<meldritch_audio::AudioBlock>>,
    performance_fx_worker: PerformanceFxWorker,
    performance_fx_generation: u64,
}

impl AppController {
    #[must_use]
    pub fn new(
        playback: PlaybackControl,
        coordinator: RenderCoordinator,
        editor: LivePatternEditor,
        selection: Selection,
        history_capacity: usize,
    ) -> Self {
        Self {
            cockpit_mode: CockpitMode::Performance,
            curated_controls: Vec::new(),
            playback,
            coordinator,
            editor,
            selection,
            history: VecDeque::with_capacity(history_capacity),
            history_capacity,
            next_sequence: 0,
            bass_synth: None,
            arrangement: None,
            automation: Vec::new(),
            effect_rules: Vec::new(),
            sidechain: None,
            transform_cache: TransformArtifactCache::default(),
            transform_view: None,
            future_cache_view: None,
            performance_launcher: PerformanceLauncher::new(LaunchQuantization::Bar { beats: 4 }),
            performance_pages: Vec::new(),
            active_performance_page: None,
            performance_scenes: Vec::new(),
            phrase_patterns: BTreeMap::new(),
            queued_phrase_variation: None,
            learned_phrase_cues: Vec::new(),
            fill_pattern: None,
            transformed_audio: None,
            performance_fx: None,
            performance_fx_source: None,
            performance_fx_worker: PerformanceFxWorker::new(),
            performance_fx_generation: 0,
        }
    }

    pub fn show_arrangement(
        &mut self,
        arrangement: Arrangement,
        tempo: Tempo,
        loop_sections: (usize, usize),
    ) -> Result<(), &'static str> {
        if arrangement
            .section_range(tempo, loop_sections.0, loop_sections.1)
            .is_none()
        {
            return Err("arrangement loop section range is invalid");
        }
        self.performance_scenes = arrangement
            .sections()
            .iter()
            .map(|section| section.scene())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        self.arrangement = Some(ArrangementPresentation {
            arrangement,
            tempo,
            loop_sections,
        });
        Ok(())
    }

    pub fn show_automation(&mut self, lanes: Vec<AutomationLane>) {
        self.automation = lanes;
    }

    pub fn set_curated_controls(&mut self, controls: Vec<CuratedControlView>) {
        self.curated_controls = controls;
    }

    pub fn set_performance_pages(&mut self, pages: Vec<PerformancePageView>) {
        self.active_performance_page = if pages.is_empty() { None } else { Some(0) };
        self.performance_pages = pages;
    }

    pub fn show_effect_sends(&mut self, rules: Vec<EffectSendRule>) {
        self.effect_rules = rules;
    }

    pub fn show_sidechain(&mut self, relation: SidechainRelation) {
        self.sidechain = Some(relation);
    }

    pub fn show_future_cache(&mut self, view: FutureCacheView) {
        self.future_cache_view = Some(view);
    }

    pub fn set_fill_pattern(&mut self, pattern: PatternId) {
        self.fill_pattern = Some(pattern);
    }

    pub fn configure_phrase_scenes(
        &mut self,
        phrases: impl IntoIterator<Item = (SceneId, Pattern)>,
    ) -> Result<(), &'static str> {
        let phrases = phrases.into_iter().collect::<Vec<_>>();
        if phrases.is_empty() {
            return Err("phrase scene bank must not be empty");
        }
        let current = self.editor.state().pattern();
        if phrases.iter().any(|(_, pattern)| {
            pattern.length_steps() != current.length_steps()
                || pattern.steps_per_beat() != current.steps_per_beat()
        }) {
            return Err("phrase scene layouts must match the live pattern");
        }
        let patterns = phrases
            .iter()
            .cloned()
            .map(|(scene, pattern)| (scene, vec![pattern]))
            .collect::<BTreeMap<_, _>>();
        if patterns.len() != phrases.len() {
            return Err("phrase scene IDs must be unique");
        }
        self.performance_scenes = phrases.iter().map(|(scene, _)| *scene).collect();
        self.phrase_patterns = patterns;
        Ok(())
    }

    pub fn configure_phrase_variations(
        &mut self,
        phrases: impl IntoIterator<Item = (SceneId, Vec<Pattern>)>,
    ) -> Result<(), &'static str> {
        let phrases = phrases.into_iter().collect::<Vec<_>>();
        if phrases.is_empty() || phrases.iter().any(|(_, patterns)| patterns.is_empty()) {
            return Err("phrase variation banks must not be empty");
        }
        let current = self.editor.state().pattern();
        if phrases
            .iter()
            .flat_map(|(_, patterns)| patterns)
            .any(|pattern| {
                pattern.length_steps() != current.length_steps()
                    || pattern.steps_per_beat() != current.steps_per_beat()
            })
        {
            return Err("phrase variation layouts must match the live pattern");
        }
        let patterns = phrases.iter().cloned().collect::<BTreeMap<_, _>>();
        if patterns.len() != phrases.len() {
            return Err("phrase scene IDs must be unique");
        }
        self.performance_scenes = phrases.iter().map(|(scene, _)| *scene).collect();
        self.phrase_patterns = patterns;
        Ok(())
    }

    pub fn queue_phrase_scene(
        &mut self,
        scene: SceneId,
    ) -> Result<QueuedPerformanceGesture, AppCommandError> {
        if !self.performance_scenes.contains(&scene) || !self.phrase_patterns.contains_key(&scene) {
            return Err(AppCommandError::NoPerformanceScenes);
        }
        self.queued_phrase_variation = None;
        Ok(self.performance_launcher.queue(
            PerformanceGesture::QueueScene(scene),
            u64::from(self.playback.status_monitor().snapshot().position),
            self.editor.state().tempo(),
        ))
    }

    pub fn queue_phrase_variation(
        &mut self,
        scene: SceneId,
        variation: usize,
    ) -> Result<QueuedPerformanceGesture, AppCommandError> {
        if !self.performance_scenes.contains(&scene)
            || self
                .phrase_patterns
                .get(&scene)
                .is_none_or(|patterns| variation >= patterns.len())
        {
            return Err(AppCommandError::NoPerformanceScenes);
        }
        self.queued_phrase_variation = Some((scene, variation));
        Ok(self.performance_launcher.queue(
            PerformanceGesture::QueueScene(scene),
            u64::from(self.playback.status_monitor().snapshot().position),
            self.editor.state().tempo(),
        ))
    }

    pub fn show_learned_phrase_cues(&mut self, cues: Vec<LearnedPhraseCueView>) {
        self.learned_phrase_cues = cues;
    }

    /// Launches a layout-compatible phrase pattern through the normal render
    /// coordinator, returning the number of invalidated horizon chunks.
    pub fn launch_pattern(&mut self, pattern: Pattern) -> Result<usize, AppCommandError> {
        self.editor
            .replace_pattern(&self.coordinator, pattern)
            .map_err(AppCommandError::LiveEdit)
    }

    #[must_use]
    pub const fn playback_control(&self) -> &PlaybackControl {
        &self.playback
    }

    #[must_use]
    pub const fn coordinator(&self) -> &RenderCoordinator {
        &self.coordinator
    }

    #[must_use]
    pub const fn selection(&self) -> Selection {
        self.selection
    }

    #[must_use]
    pub fn history(&self) -> &VecDeque<CommandRecord> {
        &self.history
    }

    pub fn dispatch(&mut self, command: AppCommand) -> Result<AppCommandResult, AppCommandError> {
        let result = match &command {
            AppCommand::Play => {
                self.playback.play().map_err(AppCommandError::QueueFull)?;
                AppCommandResult::TransportQueued
            }
            AppCommand::Stop => {
                self.playback.stop().map_err(AppCommandError::QueueFull)?;
                AppCommandResult::TransportQueued
            }
            AppCommand::Rewind => {
                self.playback.rewind().map_err(AppCommandError::QueueFull)?;
                AppCommandResult::TransportQueued
            }
            AppCommand::Select(selection) => {
                let previous = self.selection;
                self.selection = *selection;
                AppCommandResult::SelectionChanged {
                    previous,
                    current: *selection,
                }
            }
            AppCommand::Edit(edit) => AppCommandResult::Edit(
                self.editor
                    .apply(&self.coordinator, edit.clone())
                    .map_err(AppCommandError::LiveEdit)?,
            ),
            AppCommand::SetBassVoice(settings) => {
                let Some(control) = &self.bass_synth else {
                    return Err(AppCommandError::NoBassSynth);
                };
                let changed = control.settings != *settings;
                if !changed {
                    AppCommandResult::SynthUpdated {
                        invalidated_chunks: 0,
                    }
                } else {
                    let mut samples = self.editor.state().samples_by_note().as_ref().clone();
                    for note in &control.notes {
                        samples.insert(
                            *note,
                            synthesize_bass_sample(
                                *note,
                                self.editor.state().tempo().sample_rate(),
                                control.sample_frames,
                                *settings,
                            ),
                        );
                    }
                    let invalidated_chunks = self
                        .editor
                        .replace_bass_synth(
                            &self.coordinator,
                            control.track,
                            *settings,
                            std::sync::Arc::new(samples),
                        )
                        .map_err(AppCommandError::LiveEdit)?;
                    self.bass_synth
                        .as_mut()
                        .expect("bass synth exists")
                        .settings = *settings;
                    AppCommandResult::SynthUpdated { invalidated_chunks }
                }
            }
            AppCommand::SetPerformanceFx(settings) => {
                let settings = settings.normalized();
                let source = if let Some(source) = &self.performance_fx_source {
                    Arc::clone(source)
                } else {
                    let source = Arc::new(self.capture_published_audio()?);
                    self.performance_fx_source = Some(Arc::clone(&source));
                    source
                };
                self.performance_fx_generation = self.performance_fx_worker.submit(
                    source,
                    self.editor.state().tempo(),
                    settings,
                );
                self.performance_fx = Some(settings);
                AppCommandResult::PerformanceFxUpdated(settings)
            }
            AppCommand::Transform(transform) => {
                let snapshot = self.coordinator.audio_reader().snapshot();
                let mut source =
                    meldritch_audio::AudioBlock::silent(snapshot.channels(), snapshot.frames());
                for frame in 0..snapshot.frames() {
                    let values = snapshot
                        .frame(frame)
                        .map_err(|_| AppCommandError::TransformSourceUnavailable)?;
                    let channels = usize::from(snapshot.channels());
                    let start = frame as usize * channels;
                    source.samples_mut()[start..start + channels].copy_from_slice(values);
                }
                let cached = self
                    .transform_cache
                    .render(&source, transform)
                    .map_err(AppCommandError::Transform)?;
                self.transform_view = Some(TransformView {
                    transform: transform.clone(),
                    key: cached.key,
                    status: cached.status,
                    channels: cached.block.channels(),
                    frames: cached.block.frames(),
                    auditioning: false,
                });
                self.transformed_audio = Some(cached.block);
                AppCommandResult::TransformCreated {
                    key: cached.key,
                    status: cached.status,
                }
            }
            AppCommand::AuditionTransform => {
                let block = self
                    .transformed_audio
                    .as_ref()
                    .ok_or(AppCommandError::TransformSourceUnavailable)?;
                self.coordinator
                    .audition_block(block)
                    .map_err(AppCommandError::Publication)?;
                if let Some(view) = &mut self.transform_view {
                    view.auditioning = true;
                }
                AppCommandResult::AudioSourceSwitched { transformed: true }
            }
            AppCommand::ReturnToLive => {
                self.coordinator
                    .restore_live_audio()
                    .map_err(AppCommandError::Publication)?;
                if let Some(view) = &mut self.transform_view {
                    view.auditioning = false;
                }
                AppCommandResult::AudioSourceSwitched { transformed: false }
            }
            AppCommand::QueueNextScene => {
                if self.performance_scenes.is_empty() {
                    return Err(AppCommandError::NoPerformanceScenes);
                }
                let current = self
                    .performance_launcher
                    .queued()
                    .and_then(|queued| match queued.gesture {
                        PerformanceGesture::QueueScene(scene) => Some(scene),
                        _ => None,
                    })
                    .or(self.performance_launcher.active().scene);
                let index = current
                    .and_then(|scene| {
                        self.performance_scenes
                            .iter()
                            .position(|candidate| *candidate == scene)
                    })
                    .map_or(0, |index| (index + 1) % self.performance_scenes.len());
                let scene = *self
                    .performance_scenes
                    .get(index)
                    .ok_or(AppCommandError::NoPerformanceScenes)?;
                self.queued_phrase_variation = None;
                let queued = self.performance_launcher.queue(
                    PerformanceGesture::QueueScene(scene),
                    u64::from(self.playback.status_monitor().snapshot().position),
                    self.editor.state().tempo(),
                );
                AppCommandResult::PerformanceQueued(queued)
            }
            AppCommand::QueueScene(scene) => {
                if !self.performance_scenes.contains(scene)
                    || !self.phrase_patterns.contains_key(scene)
                {
                    return Err(AppCommandError::NoPerformanceScenes);
                }
                self.queued_phrase_variation = None;
                let queued = self.performance_launcher.queue(
                    PerformanceGesture::QueueScene(*scene),
                    u64::from(self.playback.status_monitor().snapshot().position),
                    self.editor.state().tempo(),
                );
                AppCommandResult::PerformanceQueued(queued)
            }
            AppCommand::QueueSceneVariation(scene, variation) => {
                return self
                    .queue_phrase_variation(*scene, *variation)
                    .map(AppCommandResult::PerformanceQueued);
            }
            AppCommand::ToggleTrackMute(track) => {
                let gesture = if self
                    .performance_launcher
                    .active()
                    .muted_tracks
                    .contains(track)
                {
                    PerformanceGesture::UnmuteTrack(*track)
                } else {
                    PerformanceGesture::MuteTrack(*track)
                };
                let queued = self.performance_launcher.queue(
                    gesture,
                    u64::from(self.playback.status_monitor().snapshot().position),
                    self.editor.state().tempo(),
                );
                AppCommandResult::PerformanceQueued(queued)
            }
            AppCommand::TriggerFill(pattern) => {
                let queued = self.performance_launcher.queue(
                    PerformanceGesture::TriggerFill(*pattern),
                    u64::from(self.playback.status_monitor().snapshot().position),
                    self.editor.state().tempo(),
                );
                AppCommandResult::PerformanceQueued(queued)
            }
            AppCommand::CancelPerformance => {
                AppCommandResult::PerformanceCancelled(self.performance_launcher.cancel())
            }
            AppCommand::SetCockpitMode(mode) => {
                let previous = self.cockpit_mode;
                self.cockpit_mode = *mode;
                AppCommandResult::CockpitModeChanged {
                    previous,
                    current: *mode,
                }
            }
            AppCommand::AdjustCuratedControl { id, steps } => {
                let control = self
                    .curated_controls
                    .iter_mut()
                    .find(|control| control.id == *id)
                    .ok_or_else(|| AppCommandError::UnknownCuratedControl(id.clone()))?;
                let previous = control.value.unwrap_or(control.minimum);
                let previous_index = ((previous - control.minimum) / control.step).round();
                let current_index = previous_index + f64::from(*steps);
                let current = (control.minimum + control.step * current_index)
                    .clamp(control.minimum, control.maximum);
                control.value = Some(current);
                AppCommandResult::CuratedControlAdjusted {
                    id: id.clone(),
                    previous,
                    current,
                }
            }
            AppCommand::SetCuratedControlNormalized { id, value } => {
                let control = self
                    .curated_controls
                    .iter_mut()
                    .find(|control| control.id == *id)
                    .ok_or_else(|| AppCommandError::UnknownCuratedControl(id.clone()))?;
                let previous = control.value.unwrap_or(control.minimum);
                let normalized = value.clamp(0.0, 1.0);
                let raw = control.minimum + normalized * (control.maximum - control.minimum);
                let current_index = ((raw - control.minimum) / control.step).round();
                let current = (control.minimum + control.step * current_index)
                    .clamp(control.minimum, control.maximum);
                control.value = Some(current);
                AppCommandResult::CuratedControlAdjusted {
                    id: id.clone(),
                    previous,
                    current,
                }
            }
        };
        let (changed, dirty_ranges) = match &result {
            AppCommandResult::TransportQueued => (true, Vec::new()),
            AppCommandResult::SelectionChanged { previous, current } => {
                (previous != current, Vec::new())
            }
            AppCommandResult::Edit(edit) => (edit.changed, edit.dirty_ranges.clone()),
            AppCommandResult::SynthUpdated { invalidated_chunks } => {
                (*invalidated_chunks != 0, Vec::new())
            }
            AppCommandResult::PerformanceFxUpdated(_) => (true, Vec::new()),
            AppCommandResult::TransformCreated { .. } => (true, Vec::new()),
            AppCommandResult::AudioSourceSwitched { .. } => (true, Vec::new()),
            AppCommandResult::PerformanceQueued(_) => (true, Vec::new()),
            AppCommandResult::PerformanceCancelled(queued) => (queued.is_some(), Vec::new()),
            AppCommandResult::CockpitModeChanged { previous, current } => {
                (previous != current, Vec::new())
            }
            AppCommandResult::CuratedControlAdjusted {
                previous, current, ..
            } => (previous != current, Vec::new()),
        };
        self.record(command, changed, dirty_ranges);
        Ok(result)
    }

    #[must_use]
    pub fn diagnostics(&self) -> AppDiagnostics {
        AppDiagnostics {
            selection: self.selection,
            history_len: self.history.len(),
            playback: self.playback.status_monitor().snapshot(),
            transport_commands: self.playback.command_diagnostics(),
            render: self.coordinator.diagnostics(),
        }
    }

    #[must_use]
    pub fn view_model(&self) -> AppViewModel {
        let diagnostics = self.diagnostics();
        let pattern = self.editor.state().pattern();
        let mut tracks = pattern
            .active_step_counts_by_track()
            .into_keys()
            .collect::<BTreeSet<_>>();
        tracks.insert(self.selection.track);
        let track_rows = tracks
            .into_iter()
            .map(|track| TrackRowView {
                track,
                selected: track == self.selection.track,
                steps: (0..pattern.length_steps())
                    .map(|raw| {
                        let step = StepIndex::new(raw);
                        StepCellView {
                            step,
                            selected: track == self.selection.track && step == self.selection.step,
                            value: pattern.get_step(track, step).cloned(),
                        }
                    })
                    .collect(),
            })
            .collect();
        AppViewModel {
            cockpit_mode: self.cockpit_mode,
            curated_controls: self.curated_controls.clone(),
            transport: TransportView {
                state: diagnostics.playback.state,
                position: diagnostics.playback.position,
                callbacks: diagnostics.playback.callbacks,
                underruns: diagnostics.playback.underruns,
                missed_artifacts: diagnostics.playback.missed_artifacts,
            },
            arrangement: self.arrangement.as_ref().map(|presentation| {
                let position = presentation.arrangement.position_at_frame(
                    presentation.tempo,
                    u64::from(diagnostics.playback.position),
                );
                ArrangementView {
                    sections: presentation
                        .arrangement
                        .sections()
                        .iter()
                        .enumerate()
                        .map(|(index, section)| ArrangementSectionView {
                            index,
                            scene: section.scene(),
                            repeats: section.repeats(),
                            active: position
                                .is_some_and(|position| position.section_index == index),
                            in_loop: presentation.loop_sections.0 <= index
                                && index < presentation.loop_sections.1,
                        })
                        .collect(),
                    active_repeat: position.map(|position| position.repeat_index),
                    loop_sections: presentation.loop_sections,
                }
            }),
            automation: if self.automation.is_empty() {
                None
            } else {
                let frame = u64::from(diagnostics.playback.position);
                let lanes = self
                    .automation
                    .iter()
                    .map(|lane| AutomationLaneView {
                        target: lane.target(),
                        interpolation: lane.interpolation(),
                        current: lane.value_at(frame),
                        next_point: lane
                            .points()
                            .iter()
                            .find(|point| point.frame > frame)
                            .map(|point| (point.frame, point.value)),
                    })
                    .collect::<Vec<_>>();
                let scene = lanes
                    .iter()
                    .find(|lane| lane.target == AutomationTarget::Scene)
                    .and_then(|lane| match lane.current {
                        AutomationValue::Discrete(scene) => Some(scene),
                        AutomationValue::Continuous(_) => None,
                    });
                Some(AutomationView { lanes, scene })
            },
            effect_sends: if self.effect_rules.is_empty() {
                None
            } else {
                let end = u64::from(diagnostics.playback.position).saturating_add(1);
                let range = meldritch_core::FrameRange::new(0, end)
                    .expect("effect explanation view range is ordered");
                let mut recent = explain_effect_sends(
                    pattern,
                    self.editor.state().tempo(),
                    range,
                    self.editor.state().probability_seed(),
                    &self.effect_rules,
                );
                if recent.len() > 6 {
                    recent.drain(..recent.len() - 6);
                }
                Some(EffectSendView { recent })
            },
            sidechain: self.sidechain.map(|relation| {
                let frame = u64::from(diagnostics.playback.position);
                let range = meldritch_core::FrameRange::new(0, frame.saturating_add(1))
                    .expect("sidechain view range is ordered");
                let mut events = Vec::new();
                pattern.events_between(
                    self.editor.state().tempo(),
                    range,
                    self.editor.state().probability_seed(),
                    &mut events,
                );
                let latest = events
                    .iter()
                    .filter(|event| event.track() == relation.control_track)
                    .map(|event| event.range().start())
                    .max();
                let release_frames = (relation.settings.release_seconds.max(0.000_001)
                    * f64::from(self.editor.state().tempo().sample_rate()))
                .max(1.0);
                let attenuation = latest.map_or(0.0, |trigger| {
                    relation.settings.amount.clamp(0.0, 1.0)
                        * (-((frame - trigger) as f64) / release_frames).exp()
                });
                SidechainView {
                    source_role: relation.source_role,
                    target_role: relation.target_role,
                    bands: relation.settings.bands,
                    attenuation,
                }
            }),
            transform: self.transform_view.clone(),
            futures: self.future_cache_view.clone(),
            performance: PerformanceView {
                queued: self.performance_launcher.queued(),
                active_scene: self.performance_launcher.active().scene,
                muted_tracks: self
                    .performance_launcher
                    .active()
                    .muted_tracks
                    .iter()
                    .copied()
                    .collect(),
                active_fill: self.performance_launcher.active().fill,
                fill_end_frame: self.performance_launcher.fill_end_frame(),
                diagnostics: self.performance_launcher.diagnostics(),
                learned_phrase_cues: self.learned_phrase_cues.clone(),
                pages: self.performance_pages.clone(),
                active_page: self.active_performance_page,
            },
            pattern_grid: PatternGridView {
                pattern: pattern.id(),
                length_steps: pattern.length_steps(),
                tracks: track_rows,
            },
            inspector: StepInspectorView {
                selection: self.selection,
                value: pattern
                    .get_step(self.selection.track, self.selection.step)
                    .cloned(),
            },
            diagnostics,
            history: self.history.iter().cloned().collect(),
            bass_voice: self.bass_synth.as_ref().map(|synth| synth.settings),
            performance_fx: self.performance_fx,
        }
    }

    pub fn handle_input(&mut self, input: AppInput) -> Result<AppCommandResult, AppCommandError> {
        let pattern_length = self.editor.state().pattern().length_steps();
        let command = match input {
            AppInput::ToggleCockpitMode => AppCommand::SetCockpitMode(self.cockpit_mode.toggled()),
            AppInput::AdjustCuratedControl { id, steps } => {
                AppCommand::AdjustCuratedControl { id, steps }
            }
            AppInput::SetCuratedControlNormalized { id, value } => {
                AppCommand::SetCuratedControlNormalized { id, value }
            }
            AppInput::MoveLeft => AppCommand::Select(Selection {
                track: self.selection.track,
                step: StepIndex::new(self.selection.step.raw().saturating_sub(1)),
            }),
            AppInput::MoveRight => AppCommand::Select(Selection {
                track: self.selection.track,
                step: StepIndex::new(
                    self.selection
                        .step
                        .raw()
                        .saturating_add(1)
                        .min(pattern_length.saturating_sub(1)),
                ),
            }),
            AppInput::MoveUp => AppCommand::Select(Selection {
                track: TrackId::new(self.selection.track.raw().saturating_sub(1)),
                step: self.selection.step,
            }),
            AppInput::MoveDown => AppCommand::Select(Selection {
                track: TrackId::new(self.selection.track.raw().saturating_add(1)),
                step: self.selection.step,
            }),
            AppInput::ToggleSelected(value) => AppCommand::Edit(LiveEditCommand::ToggleStep {
                track: self.selection.track,
                step: self.selection.step,
                value,
            }),
            AppInput::TogglePlayback => {
                if self.playback.status_monitor().snapshot().state == TransportState::Playing {
                    AppCommand::Stop
                } else {
                    AppCommand::Play
                }
            }
            AppInput::Stop => AppCommand::Stop,
            AppInput::Rewind => AppCommand::Rewind,
            AppInput::IncreaseVelocity => return self.adjust_selected(0.05, 0.0, 0.0),
            AppInput::DecreaseVelocity => return self.adjust_selected(-0.05, 0.0, 0.0),
            AppInput::IncreaseGate => return self.adjust_selected(0.0, 0.05, 0.0),
            AppInput::DecreaseGate => return self.adjust_selected(0.0, -0.05, 0.0),
            AppInput::IncreaseProbability => return self.adjust_selected(0.0, 0.0, 0.05),
            AppInput::DecreaseProbability => return self.adjust_selected(0.0, 0.0, -0.05),
            AppInput::IncreaseCutoff => {
                return self.adjust_bass_voice(|voice| voice.cutoff_hz *= 1.1);
            }
            AppInput::DecreaseCutoff => {
                return self.adjust_bass_voice(|voice| voice.cutoff_hz /= 1.1);
            }
            AppInput::IncreaseResonance => {
                return self.adjust_bass_voice(|voice| {
                    voice.resonance = (voice.resonance + 0.05).min(0.99)
                });
            }
            AppInput::DecreaseResonance => {
                return self.adjust_bass_voice(|voice| {
                    voice.resonance = (voice.resonance - 0.05).max(0.0)
                });
            }
            AppInput::CycleWaveform => {
                return self.adjust_bass_voice(|voice| {
                    voice.waveform = match voice.waveform {
                        Waveform::Sine => Waveform::Triangle,
                        Waveform::Triangle => Waveform::Saw,
                        Waveform::Saw => Waveform::Pulse,
                        Waveform::Pulse => Waveform::SyncFold,
                        Waveform::SyncFold => Waveform::Sine,
                    };
                });
            }
            AppInput::IncreaseFilterEnvelope => {
                return self.adjust_bass_voice(|voice| {
                    voice.filter_envelope_octaves = (voice.filter_envelope_octaves + 0.25).min(8.0);
                });
            }
            AppInput::DecreaseFilterEnvelope => {
                return self.adjust_bass_voice(|voice| {
                    voice.filter_envelope_octaves = (voice.filter_envelope_octaves - 0.25).max(0.0);
                });
            }
            AppInput::IncreaseDrive => {
                return self.adjust_bass_voice(|voice| {
                    voice.drive = (voice.drive + 0.1).min(20.0);
                });
            }
            AppInput::DecreaseDrive => {
                return self.adjust_bass_voice(|voice| {
                    voice.drive = (voice.drive - 0.1).max(0.0);
                });
            }
            AppInput::IncreaseSynthLevel => {
                return self.adjust_bass_voice(|voice| {
                    voice.level = (voice.level + 0.05).min(1.0);
                });
            }
            AppInput::DecreaseSynthLevel => {
                return self.adjust_bass_voice(|voice| {
                    voice.level = (voice.level - 0.05).max(0.0);
                });
            }
            AppInput::IncreaseAttack => {
                return self.adjust_bass_voice(|voice| {
                    voice.attack_seconds = (voice.attack_seconds + 0.005).min(0.2);
                });
            }
            AppInput::DecreaseAttack => {
                return self.adjust_bass_voice(|voice| {
                    voice.attack_seconds = (voice.attack_seconds - 0.005).max(0.0);
                });
            }
            AppInput::IncreaseDecay => {
                return self.adjust_bass_voice(|voice| {
                    voice.decay_seconds = (voice.decay_seconds + 0.01).min(0.2);
                });
            }
            AppInput::DecreaseDecay => {
                return self.adjust_bass_voice(|voice| {
                    voice.decay_seconds = (voice.decay_seconds - 0.01).max(0.0);
                });
            }
            AppInput::IncreaseSustain => {
                return self.adjust_bass_voice(|voice| {
                    voice.sustain_level = (voice.sustain_level + 0.05).min(1.0);
                });
            }
            AppInput::DecreaseSustain => {
                return self.adjust_bass_voice(|voice| {
                    voice.sustain_level = (voice.sustain_level - 0.05).max(0.0);
                });
            }
            AppInput::IncreaseRelease => {
                return self.adjust_bass_voice(|voice| {
                    voice.release_seconds = (voice.release_seconds + 0.01).min(0.2);
                });
            }
            AppInput::DecreaseRelease => {
                return self.adjust_bass_voice(|voice| {
                    voice.release_seconds = (voice.release_seconds - 0.01).max(0.0);
                });
            }
            AppInput::IncreaseSubLevel => {
                return self.adjust_bass_voice(|voice| {
                    voice.sub_level = (voice.sub_level + 0.05).min(1.0);
                });
            }
            AppInput::DecreaseSubLevel => {
                return self.adjust_bass_voice(|voice| {
                    voice.sub_level = (voice.sub_level - 0.05).max(0.0);
                });
            }
            AppInput::IncreaseGlide => {
                return self.adjust_bass_voice(|voice| {
                    voice.glide_seconds = (voice.glide_seconds + 0.01).min(0.5);
                });
            }
            AppInput::DecreaseGlide => {
                return self.adjust_bass_voice(|voice| {
                    voice.glide_seconds = (voice.glide_seconds - 0.01).max(0.0);
                });
            }
            AppInput::IncreaseDucking => {
                return self.adjust_bass_voice(|voice| {
                    voice.ducking_amount = (voice.ducking_amount + 0.05).min(1.0);
                });
            }
            AppInput::DecreaseDucking => {
                return self.adjust_bass_voice(|voice| {
                    voice.ducking_amount = (voice.ducking_amount - 0.05).max(0.0);
                });
            }
            AppInput::IncreaseDuckingRelease => {
                return self.adjust_bass_voice(|voice| {
                    voice.ducking_release_seconds = (voice.ducking_release_seconds + 0.01).min(1.0);
                });
            }
            AppInput::DecreaseDuckingRelease => {
                return self.adjust_bass_voice(|voice| {
                    voice.ducking_release_seconds =
                        (voice.ducking_release_seconds - 0.01).max(0.01);
                });
            }
            AppInput::IncreaseHatFilter => {
                return self.adjust_bass_voice(|voice| {
                    voice.hat_filter_octaves = (voice.hat_filter_octaves + 0.25).min(8.0);
                });
            }
            AppInput::DecreaseHatFilter => {
                return self.adjust_bass_voice(|voice| {
                    voice.hat_filter_octaves = (voice.hat_filter_octaves - 0.25).max(0.0);
                });
            }
            AppInput::IncreaseHatFilterRelease => {
                return self.adjust_bass_voice(|voice| {
                    voice.hat_filter_release_seconds =
                        (voice.hat_filter_release_seconds + 0.01).min(1.0);
                });
            }
            AppInput::DecreaseHatFilterRelease => {
                return self.adjust_bass_voice(|voice| {
                    voice.hat_filter_release_seconds =
                        (voice.hat_filter_release_seconds - 0.01).max(0.01);
                });
            }
            AppInput::IncreaseNote => return self.transpose_selected(1),
            AppInput::DecreaseNote => return self.transpose_selected(-1),
            AppInput::TransposeChordUp => return self.transpose_chord(1),
            AppInput::TransposeChordDown => return self.transpose_chord(-1),
            AppInput::InvertChordUp => return self.invert_chord(true),
            AppInput::InvertChordDown => return self.invert_chord(false),
            AppInput::IncreaseDelayFeedback => {
                return self.adjust_performance_fx(|settings| settings.delay_feedback += 0.08);
            }
            AppInput::DecreaseDelayFeedback => {
                return self.adjust_performance_fx(|settings| settings.delay_feedback -= 0.08);
            }
            AppInput::IncreasePhaserMix => {
                return self.adjust_performance_fx(|settings| settings.phaser_mix += 0.1);
            }
            AppInput::DecreasePhaserMix => {
                return self.adjust_performance_fx(|settings| settings.phaser_mix -= 0.1);
            }
            AppInput::ToggleReverbFreeze => {
                return self.adjust_performance_fx(|settings| {
                    settings.reverb_freeze = !settings.reverb_freeze;
                });
            }
            AppInput::IncreaseModulationDepth => {
                return self.adjust_performance_fx(|settings| settings.modulation_depth += 0.1);
            }
            AppInput::DecreaseModulationDepth => {
                return self.adjust_performance_fx(|settings| settings.modulation_depth -= 0.1);
            }
            AppInput::IncreaseMasterDrive => {
                return self.adjust_performance_fx(|settings| settings.master_drive += 0.35);
            }
            AppInput::DecreaseMasterDrive => {
                return self.adjust_performance_fx(|settings| settings.master_drive -= 0.35);
            }
            AppInput::CreateReverse => AppCommand::Transform(ChunkTransform::Reverse),
            AppInput::CreateReslice => AppCommand::Transform(ChunkTransform::Reslice {
                order: vec![3, 2, 1, 0],
            }),
            AppInput::CreateFreeze => AppCommand::Transform(ChunkTransform::Freeze {
                frame: self.playback.status_monitor().snapshot().position.min(
                    self.coordinator
                        .audio_reader()
                        .snapshot()
                        .frames()
                        .saturating_sub(1),
                ),
            }),
            AppInput::CreateSmear => {
                AppCommand::Transform(ChunkTransform::Smear { radius_frames: 256 })
            }
            AppInput::AuditionTransform => AppCommand::AuditionTransform,
            AppInput::ReturnToLive => AppCommand::ReturnToLive,
            AppInput::QueueNextScene => AppCommand::QueueNextScene,
            AppInput::QueuePhrase(scene) => AppCommand::QueueScene(scene),
            AppInput::QueuePhraseVariation(scene, variation) => {
                AppCommand::QueueSceneVariation(scene, variation)
            }
            AppInput::ToggleTrackMute => AppCommand::ToggleTrackMute(self.selection.track),
            AppInput::TriggerFill => {
                AppCommand::TriggerFill(self.fill_pattern.ok_or(AppCommandError::NoFillPattern)?)
            }
            AppInput::CancelPerformance => AppCommand::CancelPerformance,
        };
        self.dispatch(command)
    }

    pub fn tick_performance(
        &mut self,
        plan: &RenderableFuturePlan,
        pool: &FutureWorkerPool,
    ) -> Result<Option<PerformanceLaunch>, AppCommandError> {
        self.performance_launcher
            .advance_and_publish(
                u64::from(self.playback.status_monitor().snapshot().position),
                plan,
                pool,
                &self.coordinator.realtime_publication(),
            )
            .map_err(AppCommandError::Publication)
    }

    pub fn tick_phrase_launch(&mut self) -> Result<Option<PerformanceLaunch>, AppCommandError> {
        let position = u64::from(self.playback.status_monitor().snapshot().position);
        let Some(launch) = self.performance_launcher.advance_live(position) else {
            return Ok(None);
        };
        if let PerformanceGesture::QueueScene(scene) = launch.gesture {
            let variation = self
                .queued_phrase_variation
                .take()
                .filter(|(queued_scene, _)| *queued_scene == scene)
                .map_or(0, |(_, variation)| variation);
            let pattern = self
                .phrase_patterns
                .get(&scene)
                .and_then(|patterns| patterns.get(variation))
                .cloned()
                .ok_or(AppCommandError::NoPerformanceScenes)?;
            let held_audio = self.capture_published_audio()?;
            self.coordinator
                .audition_block(&held_audio)
                .map_err(AppCommandError::Publication)?;
            self.editor
                .replace_pattern(&self.coordinator, pattern)
                .map_err(AppCommandError::LiveEdit)?;
            if !self.coordinator.wait_for_ready_chunks(
                self.coordinator.prepared_chunk_target(),
                std::time::Duration::from_secs(2),
            ) {
                return Ok(Some(launch));
            }
            self.coordinator
                .restore_live_audio()
                .map_err(AppCommandError::Publication)?;
            self.performance_fx_source = None;
            if let Some(settings) = self.performance_fx {
                let _ = self.dispatch(AppCommand::SetPerformanceFx(settings))?;
            }
        }
        Ok(Some(launch))
    }

    pub fn enable_bass_synth(
        &mut self,
        settings: BassVoiceSettings,
        notes: BTreeSet<u8>,
        sample_frames: u32,
    ) {
        self.bass_synth = Some(BassSynthControl {
            settings,
            track: self.selection.track,
            notes,
            sample_frames,
        });
    }

    pub fn enable_performance_fx(&mut self, settings: PerformanceFxSettings) {
        self.performance_fx = Some(settings.normalized());
        self.performance_fx_source = None;
    }

    pub fn tick_performance_fx(&mut self) -> Result<bool, AppCommandError> {
        let Some((generation, block, settings)) = self.performance_fx_worker.latest_completed()
        else {
            return Ok(false);
        };
        if generation != self.performance_fx_generation || Some(settings) != self.performance_fx {
            return Ok(false);
        }
        self.coordinator
            .audition_block(&block)
            .map_err(AppCommandError::Publication)?;
        Ok(true)
    }

    fn adjust_performance_fx(
        &mut self,
        adjust: impl FnOnce(&mut PerformanceFxSettings),
    ) -> Result<AppCommandResult, AppCommandError> {
        let mut settings = self.performance_fx.unwrap_or_default();
        adjust(&mut settings);
        self.dispatch(AppCommand::SetPerformanceFx(settings))
    }

    fn capture_published_audio(&self) -> Result<meldritch_audio::AudioBlock, AppCommandError> {
        let snapshot = self.coordinator.audio_reader().snapshot();
        let mut source =
            meldritch_audio::AudioBlock::silent(snapshot.channels(), snapshot.frames());
        for frame in 0..snapshot.frames() {
            let Ok(values) = snapshot.frame(frame) else {
                continue;
            };
            let channels = usize::from(snapshot.channels());
            let start = frame as usize * channels;
            source.samples_mut()[start..start + channels].copy_from_slice(values);
        }
        Ok(source)
    }

    fn adjust_bass_voice(
        &mut self,
        adjust: impl FnOnce(&mut BassVoiceSettings),
    ) -> Result<AppCommandResult, AppCommandError> {
        let Some(control) = &self.bass_synth else {
            return Err(AppCommandError::NoBassSynth);
        };
        let mut settings = control.settings;
        adjust(&mut settings);
        self.dispatch(AppCommand::SetBassVoice(settings))
    }

    fn adjust_selected(
        &mut self,
        velocity_delta: f64,
        gate_delta: f64,
        probability_delta: f64,
    ) -> Result<AppCommandResult, AppCommandError> {
        let Some(step) = self
            .editor
            .state()
            .pattern()
            .get_step(self.selection.track, self.selection.step)
            .cloned()
        else {
            return Err(AppCommandError::NoStepSelected);
        };
        let velocity = (step.velocity() + velocity_delta).clamp(0.0, 1.0);
        let gate = (step.gate() + gate_delta).clamp(0.0, 1.0);
        let probability =
            Probability::new((step.probability().chance() + probability_delta).clamp(0.0, 1.0))
                .expect("clamped probability is valid");
        self.dispatch(AppCommand::Edit(LiveEditCommand::SetStep {
            track: self.selection.track,
            step: self.selection.step,
            value: step
                .with_velocity(velocity)
                .with_gate(gate)
                .with_probability(probability),
        }))
    }

    fn transpose_selected(&mut self, semitones: i16) -> Result<AppCommandResult, AppCommandError> {
        let Some(step) = self
            .editor
            .state()
            .pattern()
            .get_step(self.selection.track, self.selection.step)
            .cloned()
        else {
            return Err(AppCommandError::NoStepSelected);
        };
        let note = (i16::from(step.note()) + semitones).clamp(0, 127) as u8;
        self.dispatch(AppCommand::Edit(LiveEditCommand::SetStep {
            track: self.selection.track,
            step: self.selection.step,
            value: step.with_note(note),
        }))
    }

    fn chord_steps(&self) -> Result<Vec<(TrackId, Step)>, AppCommandError> {
        let Some(layer) = self.editor.state().chord_layer() else {
            return Err(AppCommandError::NoChordLayer);
        };
        let steps = (layer.first_track.raw()..=layer.last_track.raw())
            .filter_map(|raw_track| {
                let track = TrackId::new(raw_track);
                self.editor
                    .state()
                    .pattern()
                    .get_step(track, self.selection.step)
                    .cloned()
                    .map(|step| (track, step))
            })
            .collect::<Vec<_>>();
        if steps.is_empty() {
            return Err(AppCommandError::NoChordSelected);
        }
        Ok(steps)
    }

    fn transpose_chord(&mut self, semitones: i16) -> Result<AppCommandResult, AppCommandError> {
        let steps = self.chord_steps()?;
        let mut result = None;
        for (track, step) in steps {
            let note = (i16::from(step.note()) + semitones).clamp(0, 127) as u8;
            result = Some(self.dispatch(AppCommand::Edit(LiveEditCommand::SetStep {
                track,
                step: self.selection.step,
                value: step.with_note(note),
            }))?);
        }
        result.ok_or(AppCommandError::NoChordSelected)
    }

    fn invert_chord(&mut self, upward: bool) -> Result<AppCommandResult, AppCommandError> {
        let steps = self.chord_steps()?;
        let (track, step) = if upward {
            steps.into_iter().min_by_key(|(_, step)| step.note())
        } else {
            steps.into_iter().max_by_key(|(_, step)| step.note())
        }
        .ok_or(AppCommandError::NoChordSelected)?;
        let note = if upward {
            step.note().saturating_add(12).min(127)
        } else {
            step.note().saturating_sub(12)
        };
        self.dispatch(AppCommand::Edit(LiveEditCommand::SetStep {
            track,
            step: self.selection.step,
            value: step.with_note(note),
        }))
    }

    fn record(&mut self, command: AppCommand, changed: bool, dirty_ranges: Vec<DirtyRange>) {
        if self.history_capacity == 0 {
            return;
        }
        if self.history.len() == self.history_capacity {
            self.history.pop_front();
        }
        self.history.push_back(CommandRecord {
            sequence: self.next_sequence,
            command,
            changed,
            dirty_ranges,
        });
        self.next_sequence = self.next_sequence.wrapping_add(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use meldritch_audio::SampleBuffer;
    use meldritch_audio::device_output::playback_session_parts;
    use meldritch_core::{
        ArrangementSection, AutomationPoint, Pattern, PatternId, ProbabilitySeed, SceneId, Step,
        Tempo,
    };
    use meldritch_render::RenderSettings;
    use meldritch_render::coordinator::{ChordLayer, RenderCoordinatorConfig, SampleRenderState};
    use meldritch_render::dsp::BassVoiceSettings;
    use std::collections::BTreeMap;
    use std::sync::Arc;
    use std::time::Duration;

    fn controller(
        history_capacity: usize,
    ) -> (
        AppController,
        meldritch_audio::device_output::PlaybackEngine,
    ) {
        let (playback, engine) = playback_session_parts(8).unwrap();
        let pattern = Pattern::new(PatternId::new(1), 4, 4).unwrap();
        let mut samples = BTreeMap::new();
        samples.insert(36, SampleBuffer::new(1, 48_000, vec![0.7; 2]));
        let state = SampleRenderState::new(
            pattern,
            Tempo::new(120.0, 48_000).unwrap(),
            ProbabilitySeed::new(1),
            RenderSettings::new(1).unwrap(),
            Arc::new(samples),
        );
        let config =
            RenderCoordinatorConfig::new(1, 24_000, 6_000, 1, Duration::from_millis(2)).unwrap();
        let coordinator = RenderCoordinator::new(
            config,
            state.pattern().clone(),
            state.tempo(),
            state.probability_seed(),
            state.settings(),
            Arc::clone(state.samples_by_note()),
            playback.status_monitor(),
        )
        .unwrap();
        let editor = LivePatternEditor::new(state, 24_000);
        (
            AppController::new(
                playback,
                coordinator,
                editor,
                Selection {
                    track: TrackId::new(1),
                    step: StepIndex::new(0),
                },
                history_capacity,
            ),
            engine,
        )
    }

    #[test]
    fn controller_dispatches_selection_transport_and_live_edits() {
        let (mut controller, _engine) = controller(8);
        assert!(
            controller
                .coordinator()
                .wait_for_ready_chunks(4, Duration::from_secs(1))
        );

        controller.dispatch(AppCommand::Play).unwrap();
        controller
            .dispatch(AppCommand::Select(Selection {
                track: TrackId::new(2),
                step: StepIndex::new(3),
            }))
            .unwrap();
        let edit = controller
            .dispatch(AppCommand::Edit(LiveEditCommand::SetStep {
                track: TrackId::new(1),
                step: StepIndex::new(0),
                value: Step::new(36),
            }))
            .unwrap();

        assert!(matches!(edit, AppCommandResult::Edit(result) if result.changed));
        assert!(
            controller
                .coordinator()
                .wait_for_ready_chunks(4, Duration::from_secs(1))
        );
        assert_eq!(
            controller.coordinator().audio_reader().snapshot().frame(0),
            Ok([0.7].as_slice())
        );
        let diagnostics = controller.diagnostics();
        assert_eq!(diagnostics.selection.track, TrackId::new(2));
        assert_eq!(diagnostics.history_len, 3);
        assert_eq!(diagnostics.transport_commands.applied, 0);
    }

    #[test]
    fn transform_inputs_capture_cached_published_audio_and_update_provenance_view() {
        let (mut controller, _engine) = controller(8);
        assert!(
            controller
                .coordinator()
                .wait_for_ready_chunks(4, Duration::from_secs(1))
        );
        let first = controller.handle_input(AppInput::CreateReverse).unwrap();
        assert!(matches!(
            first,
            AppCommandResult::TransformCreated {
                status: TransformCacheStatus::Miss,
                ..
            }
        ));
        let second = controller.handle_input(AppInput::CreateReverse).unwrap();
        assert!(matches!(
            second,
            AppCommandResult::TransformCreated {
                status: TransformCacheStatus::Hit,
                ..
            }
        ));
        let view = controller.view_model().transform.unwrap();
        assert_eq!(view.transform, ChunkTransform::Reverse);
        assert_eq!(view.frames, 24_000);
        controller
            .handle_input(AppInput::AuditionTransform)
            .unwrap();
        assert!(controller.view_model().transform.unwrap().auditioning);
        controller.handle_input(AppInput::ReturnToLive).unwrap();
        assert!(!controller.view_model().transform.unwrap().auditioning);
    }

    #[test]
    fn command_history_is_bounded_and_sequence_remains_monotonic() {
        let (mut controller, _engine) = controller(2);
        controller.dispatch(AppCommand::Play).unwrap();
        controller.dispatch(AppCommand::Stop).unwrap();
        controller.dispatch(AppCommand::Rewind).unwrap();

        assert_eq!(controller.history().len(), 2);
        assert_eq!(controller.history()[0].sequence, 1);
        assert_eq!(controller.history()[1].sequence, 2);
    }

    #[test]
    fn performance_mode_is_default_and_mode_switch_preserves_runtime_state() {
        let (mut controller, _engine) = controller(8);
        let before = controller.view_model();
        assert_eq!(before.cockpit_mode, CockpitMode::Performance);

        let result = controller
            .handle_input(AppInput::ToggleCockpitMode)
            .unwrap();
        assert_eq!(
            result,
            AppCommandResult::CockpitModeChanged {
                previous: CockpitMode::Performance,
                current: CockpitMode::AllParameters,
            }
        );
        let after = controller.view_model();
        assert_eq!(after.cockpit_mode, CockpitMode::AllParameters);
        assert_eq!(after.transport.position, before.transport.position);
        assert_eq!(after.transport.state, before.transport.state);
        assert_eq!(after.inspector.selection, before.inspector.selection);
        assert_eq!(
            controller.history().back().unwrap().command,
            AppCommand::SetCockpitMode(CockpitMode::AllParameters)
        );

        controller
            .handle_input(AppInput::ToggleCockpitMode)
            .unwrap();
        assert_eq!(
            controller.view_model().cockpit_mode,
            CockpitMode::Performance
        );
    }

    #[test]
    fn curated_control_commands_step_clamp_and_record_without_touching_transport() {
        let (mut controller, _engine) = controller(8);
        controller.set_curated_controls(vec![CuratedControlView {
            id: "echo-feedback".to_owned(),
            label: "Echo Feedback".to_owned(),
            target: "dsp:echo/delay.feedback".to_owned(),
            minimum: 0.0,
            maximum: 0.4,
            step: 0.05,
            binding: "f".to_owned(),
            value: Some(0.35),
        }]);
        let transport = controller.view_model().transport;

        assert_eq!(
            controller
                .handle_input(AppInput::AdjustCuratedControl {
                    id: "echo-feedback".to_owned(),
                    steps: 1,
                })
                .unwrap(),
            AppCommandResult::CuratedControlAdjusted {
                id: "echo-feedback".to_owned(),
                previous: 0.35,
                current: 0.4,
            }
        );
        controller
            .handle_input(AppInput::AdjustCuratedControl {
                id: "echo-feedback".to_owned(),
                steps: 1,
            })
            .unwrap();
        controller
            .handle_input(AppInput::AdjustCuratedControl {
                id: "echo-feedback".to_owned(),
                steps: -2,
            })
            .unwrap();
        let view = controller.view_model();
        assert!((view.curated_controls[0].value.unwrap() - 0.3).abs() < f64::EPSILON);
        assert_eq!(view.transport.position, transport.position);
        assert_eq!(view.transport.state, transport.state);
        assert_eq!(controller.history().len(), 3);
        assert!(!controller.history()[1].changed);
    }

    #[test]
    fn performance_pages_are_view_state_not_hard_coded_policy() {
        let (mut controller, _engine) = controller(8);
        controller.set_performance_pages(vec![
            PerformancePageView {
                id: "main".to_owned(),
                label: "Main".to_owned(),
                strips: vec![PerformanceStripView {
                    strip: 1,
                    lane_id: "pad".to_owned(),
                    lane_label: "Pad".to_owned(),
                    lane_role: "polyphonic_synth".to_owned(),
                    track_id: Some("pad-track".to_owned()),
                    variation_ids: vec![
                        "pad-a".to_owned(),
                        "pad-b".to_owned(),
                        "pad-c".to_owned(),
                        "pad-d".to_owned(),
                    ],
                }],
            },
            PerformancePageView {
                id: "drums".to_owned(),
                label: "Drums".to_owned(),
                strips: vec![PerformanceStripView {
                    strip: 8,
                    lane_id: "kick".to_owned(),
                    lane_label: "Kick".to_owned(),
                    lane_role: "drum".to_owned(),
                    track_id: Some("kick-track".to_owned()),
                    variation_ids: vec!["kick-a".to_owned()],
                }],
            },
        ]);

        let view = controller.view_model();
        assert_eq!(view.performance.active_page, Some(0));
        assert_eq!(view.performance.pages.len(), 2);
        assert_eq!(view.performance.pages[0].id, "main");
        assert_eq!(view.performance.pages[0].strips[0].lane_id, "pad");
        assert_eq!(view.performance.pages[1].strips[0].strip, 8);
    }

    #[test]
    fn curated_control_normalized_inputs_set_absolute_values_for_faders() {
        let (mut controller, _engine) = controller(8);
        controller.set_curated_controls(vec![CuratedControlView {
            id: "echo-feedback".to_owned(),
            label: "Echo Feedback".to_owned(),
            target: "dsp:echo/delay.feedback".to_owned(),
            minimum: 0.0,
            maximum: 0.4,
            step: 0.05,
            binding: "f".to_owned(),
            value: Some(0.1),
        }]);

        assert_eq!(
            controller
                .handle_input(AppInput::SetCuratedControlNormalized {
                    id: "echo-feedback".to_owned(),
                    value: 64.0 / 127.0,
                })
                .unwrap(),
            AppCommandResult::CuratedControlAdjusted {
                id: "echo-feedback".to_owned(),
                previous: 0.1,
                current: 0.2,
            }
        );
        assert_eq!(
            controller
                .handle_input(AppInput::SetCuratedControlNormalized {
                    id: "echo-feedback".to_owned(),
                    value: 2.0,
                })
                .unwrap(),
            AppCommandResult::CuratedControlAdjusted {
                id: "echo-feedback".to_owned(),
                previous: 0.2,
                current: 0.4,
            }
        );
        assert_eq!(
            controller
                .handle_input(AppInput::SetCuratedControlNormalized {
                    id: "echo-feedback".to_owned(),
                    value: -1.0,
                })
                .unwrap(),
            AppCommandResult::CuratedControlAdjusted {
                id: "echo-feedback".to_owned(),
                previous: 0.4,
                current: 0.0,
            }
        );
    }

    #[test]
    fn midi_control_bindings_map_scripted_ccs_to_curated_control_inputs() {
        let bindings = vec![
            MidiControlBinding {
                control_id: "echo-feedback".to_owned(),
                device: "launch-control-xl".to_owned(),
                channel: Some(1),
                cc: 77,
                minimum: 0.0,
                maximum: 1.0,
                action: MidiControlAction::Absolute,
            },
            MidiControlBinding {
                control_id: "echo-feedback".to_owned(),
                device: "launch-control-xl".to_owned(),
                channel: Some(1),
                cc: 41,
                minimum: 0.0,
                maximum: 1.0,
                action: MidiControlAction::Decrement,
            },
            MidiControlBinding {
                control_id: "echo-feedback".to_owned(),
                device: "launch-control-xl".to_owned(),
                channel: Some(1),
                cc: 57,
                minimum: 0.0,
                maximum: 1.0,
                action: MidiControlAction::Increment,
            },
        ];

        assert_eq!(
            map_midi_control_input(
                &bindings,
                MidiControlInput {
                    device: "launch-control-xl".to_owned(),
                    channel: 1,
                    cc: 77,
                    value: 64,
                },
            ),
            Some(AppInput::SetCuratedControlNormalized {
                id: "echo-feedback".to_owned(),
                value: 64.0 / 127.0,
            })
        );
        assert_eq!(
            map_midi_control_input(
                &bindings,
                MidiControlInput {
                    device: "launch-control-xl".to_owned(),
                    channel: 1,
                    cc: 41,
                    value: 127,
                },
            ),
            Some(AppInput::AdjustCuratedControl {
                id: "echo-feedback".to_owned(),
                steps: -1,
            })
        );
        assert_eq!(
            map_midi_control_input(
                &bindings,
                MidiControlInput {
                    device: "launch-control-xl".to_owned(),
                    channel: 1,
                    cc: 57,
                    value: 127,
                },
            ),
            Some(AppInput::AdjustCuratedControl {
                id: "echo-feedback".to_owned(),
                steps: 1,
            })
        );
        assert_eq!(
            map_midi_control_input(
                &bindings,
                MidiControlInput {
                    device: "launch-control-xl".to_owned(),
                    channel: 1,
                    cc: 57,
                    value: 0,
                },
            ),
            None
        );
        assert_eq!(
            map_midi_control_input(
                &[MidiControlBinding {
                    control_id: "echo-feedback".to_owned(),
                    device: "launch-control-xl".to_owned(),
                    channel: None,
                    cc: 57,
                    minimum: 0.0,
                    maximum: 1.0,
                    action: MidiControlAction::Increment,
                }],
                MidiControlInput {
                    device: "launch-control-xl".to_owned(),
                    channel: 16,
                    cc: 57,
                    value: 127,
                },
            ),
            Some(AppInput::AdjustCuratedControl {
                id: "echo-feedback".to_owned(),
                steps: 1,
            })
        );
        assert_eq!(
            map_midi_control_input(
                &[MidiControlBinding {
                    control_id: "cutoff".to_owned(),
                    device: "launch-control-xl".to_owned(),
                    channel: Some(9),
                    cc: 13,
                    minimum: 100.0,
                    maximum: 5000.0,
                    action: MidiControlAction::Centered { center: 4350.0 },
                }],
                MidiControlInput {
                    device: "launch-control-xl".to_owned(),
                    channel: 9,
                    cc: 13,
                    value: 64,
                },
            ),
            Some(AppInput::SetCuratedControlNormalized {
                id: "cutoff".to_owned(),
                value: (4350.0 - 100.0) / (5000.0 - 100.0),
            })
        );
        assert_eq!(
            map_midi_control_input(
                &[MidiControlBinding {
                    control_id: "cutoff".to_owned(),
                    device: "launch-control-xl".to_owned(),
                    channel: Some(9),
                    cc: 77,
                    minimum: 100.0,
                    maximum: 5000.0,
                    action: MidiControlAction::Overdrive {
                        normal: 4350.0,
                        normal_midi: 108,
                    },
                }],
                MidiControlInput {
                    device: "launch-control-xl".to_owned(),
                    channel: 9,
                    cc: 77,
                    value: 108,
                },
            ),
            Some(AppInput::SetCuratedControlNormalized {
                id: "cutoff".to_owned(),
                value: (4350.0 - 100.0) / (5000.0 - 100.0),
            })
        );
    }

    #[test]
    fn view_model_is_read_only_and_inputs_dispatch_commands() {
        let (mut controller, _engine) = controller(8);
        controller.handle_input(AppInput::MoveRight).unwrap();
        controller.handle_input(AppInput::MoveDown).unwrap();
        controller
            .handle_input(AppInput::ToggleSelected(Step::new(36)))
            .unwrap();
        assert!(
            controller
                .coordinator()
                .wait_for_ready_chunks(4, Duration::from_secs(1))
        );

        let view = controller.view_model();
        assert_eq!(
            view.inspector.selection,
            Selection {
                track: TrackId::new(2),
                step: StepIndex::new(1),
            }
        );
        assert_eq!(view.inspector.value.as_ref().map(Step::note), Some(36));
        let selected_row = view
            .pattern_grid
            .tracks
            .iter()
            .find(|row| row.selected)
            .unwrap();
        assert_eq!(selected_row.track, TrackId::new(2));
        assert!(selected_row.steps[1].selected);
        assert_eq!(view.history.len(), 3);

        controller.handle_input(AppInput::MoveLeft).unwrap();
        controller.handle_input(AppInput::MoveLeft).unwrap();
        assert_eq!(controller.selection().step, StepIndex::new(0));
    }

    #[test]
    fn parameter_inputs_edit_the_selected_step_and_clamp_values() {
        let (mut controller, _engine) = controller(32);
        controller
            .handle_input(AppInput::ToggleSelected(
                Step::new(36).with_velocity(0.95).with_gate(0.05),
            ))
            .unwrap();

        controller.handle_input(AppInput::IncreaseVelocity).unwrap();
        controller.handle_input(AppInput::IncreaseVelocity).unwrap();
        controller.handle_input(AppInput::DecreaseGate).unwrap();
        controller.handle_input(AppInput::DecreaseGate).unwrap();
        let value = controller.view_model().inspector.value.unwrap();
        assert_eq!(value.velocity(), 1.0);
        assert_eq!(value.gate(), 0.0);

        controller.handle_input(AppInput::DecreaseVelocity).unwrap();
        controller.handle_input(AppInput::IncreaseGate).unwrap();
        controller
            .handle_input(AppInput::DecreaseProbability)
            .unwrap();
        controller.handle_input(AppInput::IncreaseNote).unwrap();
        let value = controller.view_model().inspector.value.unwrap();
        assert_eq!(value.note(), 37);
        assert!((value.velocity() - 0.95).abs() < f64::EPSILON);
        assert!((value.gate() - 0.05).abs() < f64::EPSILON);
        assert!((value.probability().chance() - 0.95).abs() < f64::EPSILON);
        assert!(matches!(
            controller.history().back().map(|record| &record.command),
            Some(AppCommand::Edit(LiveEditCommand::SetStep { .. }))
        ));
    }

    #[test]
    fn parameter_inputs_require_an_active_step() {
        let (mut controller, _engine) = controller(8);

        assert!(matches!(
            controller.handle_input(AppInput::IncreaseVelocity),
            Err(AppCommandError::NoStepSelected)
        ));
        assert!(controller.history().is_empty());
    }

    #[test]
    fn bass_controls_regenerate_samples_and_publish_new_settings() {
        let (mut controller, _engine) = controller(8);
        let initial = BassVoiceSettings::default();
        controller.enable_bass_synth(initial, [36].into_iter().collect(), 8);

        let result = controller.handle_input(AppInput::IncreaseCutoff).unwrap();
        assert!(matches!(result, AppCommandResult::SynthUpdated { .. }));
        controller.handle_input(AppInput::CycleWaveform).unwrap();
        controller
            .handle_input(AppInput::IncreaseFilterEnvelope)
            .unwrap();
        controller.handle_input(AppInput::IncreaseDrive).unwrap();
        controller
            .handle_input(AppInput::IncreaseSynthLevel)
            .unwrap();
        controller.handle_input(AppInput::IncreaseAttack).unwrap();
        controller.handle_input(AppInput::IncreaseDecay).unwrap();
        controller.handle_input(AppInput::IncreaseSustain).unwrap();
        controller.handle_input(AppInput::IncreaseRelease).unwrap();
        controller.handle_input(AppInput::IncreaseSubLevel).unwrap();
        controller.handle_input(AppInput::IncreaseGlide).unwrap();
        controller.handle_input(AppInput::IncreaseDucking).unwrap();
        controller
            .handle_input(AppInput::IncreaseDuckingRelease)
            .unwrap();
        controller
            .handle_input(AppInput::IncreaseHatFilter)
            .unwrap();
        controller
            .handle_input(AppInput::IncreaseHatFilterRelease)
            .unwrap();
        let updated = controller.view_model().bass_voice.unwrap();
        assert!(updated.cutoff_hz > initial.cutoff_hz);
        assert_ne!(updated.waveform, initial.waveform);
        assert!(updated.filter_envelope_octaves > initial.filter_envelope_octaves);
        assert!(updated.drive > initial.drive);
        assert!(updated.level > initial.level);
        assert!(updated.attack_seconds > initial.attack_seconds);
        assert!(updated.decay_seconds > initial.decay_seconds);
        assert!(updated.sustain_level > initial.sustain_level);
        assert!(updated.release_seconds > initial.release_seconds);
        assert!(updated.sub_level > initial.sub_level);
        assert!(updated.glide_seconds > initial.glide_seconds);
        assert!(updated.ducking_amount > initial.ducking_amount);
        assert!(updated.ducking_release_seconds > initial.ducking_release_seconds);
        assert!(updated.hat_filter_octaves > initial.hat_filter_octaves);
        assert!(updated.hat_filter_release_seconds > initial.hat_filter_release_seconds);
        assert!(matches!(
            controller.history().back().map(|record| &record.command),
            Some(AppCommand::SetBassVoice(_))
        ));
    }

    #[test]
    fn performance_fx_controls_publish_bounded_audio_and_update_view() {
        let (mut controller, _engine) = controller(16);
        assert!(
            controller
                .coordinator()
                .wait_for_ready_chunks(2, Duration::from_secs(1))
        );
        controller.enable_performance_fx(PerformanceFxSettings::default());
        for input in [
            AppInput::IncreaseDelayFeedback,
            AppInput::IncreasePhaserMix,
            AppInput::ToggleReverbFreeze,
            AppInput::IncreaseModulationDepth,
            AppInput::IncreaseMasterDrive,
        ] {
            assert!(matches!(
                controller.handle_input(input).unwrap(),
                AppCommandResult::PerformanceFxUpdated(_)
            ));
        }
        let settings = controller.view_model().performance_fx.unwrap();
        assert!(settings.delay_feedback > PerformanceFxSettings::default().delay_feedback);
        assert!(settings.phaser_mix > PerformanceFxSettings::default().phaser_mix);
        assert!(settings.reverb_freeze);
        assert!(settings.modulation_depth > PerformanceFxSettings::default().modulation_depth);
        assert!(settings.master_drive > PerformanceFxSettings::default().master_drive);
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        let mut published = false;
        while std::time::Instant::now() < deadline {
            if controller.tick_performance_fx().unwrap() {
                published = true;
                break;
            }
            std::thread::yield_now();
        }
        assert!(published);
        let snapshot = controller.coordinator().audio_reader().snapshot();
        let peak = (0..snapshot.frames())
            .flat_map(|frame| snapshot.frame(frame).unwrap().iter().copied())
            .fold(0.0_f64, |peak, sample| peak.max(sample.abs()));
        assert!(peak <= 0.95);
    }

    #[test]
    fn arrangement_overview_follows_the_realtime_playhead() {
        let (mut controller, _engine) = controller(8);
        let pattern = controller.editor.state().pattern().clone();
        let tempo = controller.editor.state().tempo();
        let arrangement = Arrangement::new(vec![
            ArrangementSection::new(pattern.clone(), 2, SceneId::new(1)).unwrap(),
            ArrangementSection::new(pattern, 1, SceneId::new(2)).unwrap(),
        ])
        .unwrap();
        controller
            .show_arrangement(arrangement, tempo, (0, 2))
            .unwrap();

        let view = controller.view_model().arrangement.unwrap();
        assert!(view.sections[0].active);
        assert!(!view.sections[1].active);
        assert!(view.sections.iter().all(|section| section.in_loop));
        assert_eq!(view.active_repeat, Some(0));
    }

    #[test]
    fn performance_inputs_queue_scene_mute_fill_and_cancel_at_the_playhead() {
        let (mut controller, _engine) = controller(16);
        let pattern = controller.editor.state().pattern().clone();
        let tempo = controller.editor.state().tempo();
        let arrangement = Arrangement::new(vec![
            ArrangementSection::new(pattern.clone(), 1, SceneId::new(1)).unwrap(),
            ArrangementSection::new(pattern, 1, SceneId::new(2)).unwrap(),
        ])
        .unwrap();
        controller
            .show_arrangement(arrangement, tempo, (0, 2))
            .unwrap();
        controller.set_fill_pattern(PatternId::new(9));

        let AppCommandResult::PerformanceQueued(scene) =
            controller.handle_input(AppInput::QueueNextScene).unwrap()
        else {
            panic!("scene was not queued");
        };
        assert_eq!(
            scene.gesture,
            PerformanceGesture::QueueScene(SceneId::new(1))
        );
        assert_eq!(scene.launch_frame, 0);
        let AppCommandResult::PerformanceQueued(mute) =
            controller.handle_input(AppInput::ToggleTrackMute).unwrap()
        else {
            panic!("mute was not queued");
        };
        assert_eq!(mute.gesture, PerformanceGesture::MuteTrack(TrackId::new(1)));
        assert!(matches!(
            controller
                .handle_input(AppInput::CancelPerformance)
                .unwrap(),
            AppCommandResult::PerformanceCancelled(Some(_))
        ));
        let AppCommandResult::PerformanceQueued(fill) =
            controller.handle_input(AppInput::TriggerFill).unwrap()
        else {
            panic!("fill was not queued");
        };
        assert_eq!(
            fill.gesture,
            PerformanceGesture::TriggerFill(PatternId::new(9))
        );
        assert_eq!(controller.view_model().performance.queued, Some(fill));
    }

    #[test]
    fn queued_phrase_scene_launches_into_the_live_render_state() {
        let (mut controller, _engine) = controller(16);
        let mut first = Pattern::new(PatternId::new(201), 4, 4).unwrap();
        first
            .set_step(TrackId::new(1), StepIndex::new(0), Step::new(36))
            .unwrap();
        let mut second = Pattern::new(PatternId::new(202), 4, 4).unwrap();
        second
            .set_step(TrackId::new(1), StepIndex::new(1), Step::new(36))
            .unwrap();
        let mut first_variation = first.clone();
        first_variation
            .set_step(TrackId::new(1), StepIndex::new(2), Step::new(36))
            .unwrap();
        controller
            .configure_phrase_variations([
                (SceneId::new(1), vec![first, first_variation]),
                (SceneId::new(2), vec![second]),
            ])
            .unwrap();

        controller.handle_input(AppInput::QueueNextScene).unwrap();
        let launch = controller.tick_phrase_launch().unwrap().unwrap();
        assert_eq!(
            launch.gesture,
            PerformanceGesture::QueueScene(SceneId::new(1))
        );
        assert_eq!(
            launch.source,
            meldritch_render::futures::PerformanceLaunchSource::LiveFallback
        );
        assert_eq!(controller.editor.state().pattern().id(), PatternId::new(1));
        assert!(
            controller
                .editor
                .state()
                .pattern()
                .get_step(TrackId::new(1), StepIndex::new(0))
                .is_some()
        );
        assert!(controller.view_model().performance.queued.is_none());
        assert_eq!(
            controller.view_model().performance.active_scene,
            Some(SceneId::new(1))
        );

        let AppCommandResult::PerformanceQueued(direct) = controller
            .handle_input(AppInput::QueuePhrase(SceneId::new(2)))
            .unwrap()
        else {
            panic!("direct phrase pad was not queued");
        };
        assert_eq!(
            direct.gesture,
            PerformanceGesture::QueueScene(SceneId::new(2))
        );
        assert!(matches!(
            controller.handle_input(AppInput::QueuePhrase(SceneId::new(9))),
            Err(AppCommandError::NoPerformanceScenes)
        ));
        controller
            .handle_input(AppInput::QueuePhraseVariation(SceneId::new(1), 1))
            .unwrap();
        controller.tick_phrase_launch().unwrap().unwrap();
        assert!(
            controller
                .editor
                .state()
                .pattern()
                .get_step(TrackId::new(1), StepIndex::new(2))
                .is_some()
        );
        assert!(matches!(
            controller.handle_input(AppInput::QueuePhraseVariation(SceneId::new(2), 1)),
            Err(AppCommandError::NoPerformanceScenes)
        ));
    }

    #[test]
    fn automation_view_reports_current_value_and_next_point() {
        let (mut controller, _engine) = controller(8);
        controller.show_automation(vec![
            AutomationLane::new(
                AutomationTarget::Scene,
                AutomationInterpolation::Step,
                vec![
                    AutomationPoint {
                        frame: 0,
                        value: AutomationValue::Discrete(1),
                    },
                    AutomationPoint {
                        frame: 48_000,
                        value: AutomationValue::Discrete(2),
                    },
                ],
            )
            .unwrap(),
        ]);

        let automation = controller.view_model().automation.unwrap();
        assert_eq!(automation.scene, Some(1));
        assert_eq!(automation.lanes[0].current, AutomationValue::Discrete(1));
        assert_eq!(
            automation.lanes[0].next_point,
            Some((48_000, AutomationValue::Discrete(2)))
        );
    }

    #[test]
    fn chord_controls_transpose_and_invert_all_selected_tones() {
        let (playback, _engine) = playback_session_parts(8).unwrap();
        let mut pattern = Pattern::new(PatternId::new(1), 4, 4).unwrap();
        for (track, note) in [(10, 60), (11, 63), (12, 67)] {
            pattern
                .set_step(TrackId::new(track), StepIndex::new(0), Step::new(note))
                .unwrap();
        }
        let state = SampleRenderState::new(
            pattern,
            Tempo::new(120.0, 48_000).unwrap(),
            ProbabilitySeed::new(1),
            RenderSettings::new(1).unwrap(),
            Arc::new(BTreeMap::new()),
        )
        .with_chord_layer(ChordLayer {
            first_track: TrackId::new(10),
            last_track: TrackId::new(12),
            settings: BassVoiceSettings::default(),
            voice_count: 8,
        });
        let config =
            RenderCoordinatorConfig::new(1, 12_000, 2_000, 1, Duration::from_millis(2)).unwrap();
        let coordinator =
            RenderCoordinator::new_from_state(config, state.clone(), playback.status_monitor())
                .unwrap();
        let editor = LivePatternEditor::new(state, 12_000);
        let mut controller = AppController::new(
            playback,
            coordinator,
            editor,
            Selection {
                track: TrackId::new(10),
                step: StepIndex::new(0),
            },
            16,
        );

        controller.handle_input(AppInput::TransposeChordUp).unwrap();
        assert_eq!(
            [10, 11, 12].map(|track| {
                controller
                    .editor
                    .state()
                    .pattern()
                    .get_step(TrackId::new(track), StepIndex::new(0))
                    .unwrap()
                    .note()
            }),
            [61, 64, 68]
        );
        controller.handle_input(AppInput::InvertChordUp).unwrap();
        assert_eq!(
            controller
                .editor
                .state()
                .pattern()
                .get_step(TrackId::new(10), StepIndex::new(0))
                .unwrap()
                .note(),
            73
        );
    }
}
