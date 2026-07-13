use clap::{Parser, Subcommand, ValueEnum};
use meldritch_core::{
    Arrangement, ArrangementSection, AutomationInterpolation, AutomationLane, AutomationPoint,
    AutomationTarget, AutomationValue, DirtyRange, EntityId, Event, EventTag, FrameRange, Pattern,
    PatternId, ProbabilitySeed, SceneId, SourceId, Step, StepIndex, Tempo, TrackId,
};
use meldritch_dsl::{ParameterOwner, ParameterTargetDefinition};
use meldritch_render::coordinator::{RenderCoordinator, RenderCoordinatorConfig};
use meldritch_render::{ArtifactCache, CacheStatus, RenderSettings};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::OpenOptions;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex, mpsc};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
enum LearnedAction {
    IncreaseCutoff,
    DecreaseCutoff,
    IncreaseResonance,
    DecreaseResonance,
    CycleWaveform,
    IncreaseFilterEnvelope,
    DecreaseFilterEnvelope,
    IncreaseDrive,
    DecreaseDrive,
    TransposeChordUp,
    TransposeChordDown,
    InvertChordUp,
    InvertChordDown,
    QueueNextScene,
    QueuePhrase(u64),
    QueuePhraseVariation(u64, usize),
    ToggleTrackMute,
    TriggerFill,
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

impl LearnedAction {
    fn from_input(input: &meldritch_app::AppInput) -> Option<Self> {
        use meldritch_app::AppInput;
        Some(match input {
            AppInput::IncreaseCutoff => Self::IncreaseCutoff,
            AppInput::DecreaseCutoff => Self::DecreaseCutoff,
            AppInput::IncreaseResonance => Self::IncreaseResonance,
            AppInput::DecreaseResonance => Self::DecreaseResonance,
            AppInput::CycleWaveform => Self::CycleWaveform,
            AppInput::IncreaseFilterEnvelope => Self::IncreaseFilterEnvelope,
            AppInput::DecreaseFilterEnvelope => Self::DecreaseFilterEnvelope,
            AppInput::IncreaseDrive => Self::IncreaseDrive,
            AppInput::DecreaseDrive => Self::DecreaseDrive,
            AppInput::TransposeChordUp => Self::TransposeChordUp,
            AppInput::TransposeChordDown => Self::TransposeChordDown,
            AppInput::InvertChordUp => Self::InvertChordUp,
            AppInput::InvertChordDown => Self::InvertChordDown,
            AppInput::QueueNextScene => Self::QueueNextScene,
            AppInput::QueuePhrase(scene) => Self::QueuePhrase(scene.raw()),
            AppInput::QueuePhraseVariation(scene, variation) => {
                Self::QueuePhraseVariation(scene.raw(), *variation)
            }
            AppInput::ToggleTrackMute => Self::ToggleTrackMute,
            AppInput::TriggerFill => Self::TriggerFill,
            AppInput::IncreaseDelayFeedback => Self::IncreaseDelayFeedback,
            AppInput::DecreaseDelayFeedback => Self::DecreaseDelayFeedback,
            AppInput::IncreasePhaserMix => Self::IncreasePhaserMix,
            AppInput::DecreasePhaserMix => Self::DecreasePhaserMix,
            AppInput::ToggleReverbFreeze => Self::ToggleReverbFreeze,
            AppInput::IncreaseModulationDepth => Self::IncreaseModulationDepth,
            AppInput::DecreaseModulationDepth => Self::DecreaseModulationDepth,
            AppInput::IncreaseMasterDrive => Self::IncreaseMasterDrive,
            AppInput::DecreaseMasterDrive => Self::DecreaseMasterDrive,
            _ => return None,
        })
    }

    const fn input(self) -> Option<meldritch_app::AppInput> {
        use meldritch_app::AppInput;
        Some(match self {
            Self::IncreaseCutoff => AppInput::IncreaseCutoff,
            Self::DecreaseCutoff => AppInput::DecreaseCutoff,
            Self::IncreaseResonance => AppInput::IncreaseResonance,
            Self::DecreaseResonance => AppInput::DecreaseResonance,
            Self::CycleWaveform => AppInput::CycleWaveform,
            Self::IncreaseFilterEnvelope => AppInput::IncreaseFilterEnvelope,
            Self::DecreaseFilterEnvelope => AppInput::DecreaseFilterEnvelope,
            Self::IncreaseDrive => AppInput::IncreaseDrive,
            Self::DecreaseDrive => AppInput::DecreaseDrive,
            Self::TransposeChordUp => AppInput::TransposeChordUp,
            Self::TransposeChordDown => AppInput::TransposeChordDown,
            Self::InvertChordUp => AppInput::InvertChordUp,
            Self::InvertChordDown => AppInput::InvertChordDown,
            Self::QueueNextScene => AppInput::QueueNextScene,
            Self::QueuePhrase(scene) => AppInput::QueuePhrase(SceneId::new(scene)),
            Self::QueuePhraseVariation(scene, variation) => {
                AppInput::QueuePhraseVariation(SceneId::new(scene), variation)
            }
            Self::ToggleTrackMute => AppInput::ToggleTrackMute,
            Self::TriggerFill => AppInput::TriggerFill,
            Self::IncreaseDelayFeedback => AppInput::IncreaseDelayFeedback,
            Self::DecreaseDelayFeedback => AppInput::DecreaseDelayFeedback,
            Self::IncreasePhaserMix => AppInput::IncreasePhaserMix,
            Self::DecreasePhaserMix => AppInput::DecreasePhaserMix,
            Self::ToggleReverbFreeze => AppInput::ToggleReverbFreeze,
            Self::IncreaseModulationDepth => AppInput::IncreaseModulationDepth,
            Self::DecreaseModulationDepth => AppInput::DecreaseModulationDepth,
            Self::IncreaseMasterDrive => AppInput::IncreaseMasterDrive,
            Self::DecreaseMasterDrive => AppInput::DecreaseMasterDrive,
        })
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct LearnedFuture {
    action: LearnedAction,
    occurrences: u64,
    last_session: u64,
    mean_phase: f64,
    score: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct CapturedFuture {
    origin: String,
    action: LearnedAction,
    frame: u32,
    phase: f64,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
struct FutureLibrary {
    schema_version: u32,
    sessions: u64,
    learned: Vec<LearnedFuture>,
    last_session: Vec<CapturedFuture>,
}

fn merge_performer_futures(
    library: &mut FutureLibrary,
    captured: Vec<CapturedFuture>,
    session: u64,
) {
    let mut learned = std::mem::take(&mut library.learned)
        .into_iter()
        .map(|future| (future.action, future))
        .collect::<BTreeMap<_, _>>();
    for event in captured.iter().filter(|event| event.origin == "performer") {
        let future = learned.entry(event.action).or_insert(LearnedFuture {
            action: event.action,
            occurrences: 0,
            last_session: session,
            mean_phase: event.phase,
            score: 0,
        });
        let old_count = future.occurrences;
        future.occurrences = future.occurrences.saturating_add(1);
        future.mean_phase =
            (future.mean_phase * old_count as f64 + event.phase) / future.occurrences as f64;
        future.last_session = session;
        future.score = future
            .occurrences
            .saturating_mul(1_000)
            .saturating_add(session);
    }
    library.learned = learned.into_values().collect();
    library.learned.sort_by(|left, right| {
        right
            .score
            .cmp(&left.score)
            .then_with(|| left.action.cmp(&right.action))
    });
    library.last_session = captured;
}

fn learned_phrase_schedule(
    library: &FutureLibrary,
    frame_count: u32,
    limit: usize,
) -> Vec<(u32, SceneId)> {
    if frame_count == 0 || limit == 0 {
        return Vec::new();
    }
    let last_frame = frame_count.saturating_sub(1);
    let mut schedule = library
        .learned
        .iter()
        .filter_map(|future| match future.action {
            LearnedAction::QueuePhrase(scene) => Some((
                (future.mean_phase.clamp(0.0, 1.0) * f64::from(last_frame)).round() as u32,
                SceneId::new(scene),
                future.score,
            )),
            LearnedAction::QueuePhraseVariation(scene, _) => Some((
                (future.mean_phase.clamp(0.0, 1.0) * f64::from(last_frame)).round() as u32,
                SceneId::new(scene),
                future.score,
            )),
            _ => None,
        })
        .collect::<Vec<_>>();
    schedule.sort_by(|left, right| {
        left.0
            .cmp(&right.0)
            .then_with(|| right.2.cmp(&left.2))
            .then_with(|| left.1.cmp(&right.1))
    });
    schedule
        .into_iter()
        .take(limit)
        .map(|(frame, scene, _)| (frame, scene))
        .collect()
}

fn is_dsp_action(action: LearnedAction) -> bool {
    matches!(
        action,
        LearnedAction::IncreaseDelayFeedback
            | LearnedAction::DecreaseDelayFeedback
            | LearnedAction::IncreasePhaserMix
            | LearnedAction::DecreasePhaserMix
            | LearnedAction::ToggleReverbFreeze
            | LearnedAction::IncreaseModulationDepth
            | LearnedAction::DecreaseModulationDepth
            | LearnedAction::IncreaseMasterDrive
            | LearnedAction::DecreaseMasterDrive
    )
}

fn learned_dsp_schedule(
    library: &FutureLibrary,
    frame_count: u32,
    limit: usize,
) -> Vec<(u32, LearnedAction)> {
    let last = frame_count.saturating_sub(1);
    let mut schedule = library
        .learned
        .iter()
        .filter(|future| is_dsp_action(future.action))
        .map(|future| {
            (
                (future.mean_phase.clamp(0.0, 1.0) * f64::from(last)).round() as u32,
                future.action,
                future.score,
            )
        })
        .collect::<Vec<_>>();
    schedule.sort_by(|left, right| left.0.cmp(&right.0).then_with(|| right.2.cmp(&left.2)));
    schedule
        .into_iter()
        .take(limit)
        .map(|(frame, action, _)| (frame, action))
        .collect()
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct PerformerOverrideGrace {
    until_frame: Option<u32>,
}

impl PerformerOverrideGrace {
    fn record(&mut self, frame: u32, grace_frames: u32, timeline_frames: u32) {
        self.until_frame = Some(
            frame
                .saturating_add(grace_frames)
                .min(timeline_frames.saturating_sub(1)),
        );
    }

    #[must_use]
    fn suppresses(self, frame: u32) -> bool {
        self.until_frame.is_some_and(|until| frame <= until)
    }

    fn reset(&mut self) {
        self.until_frame = None;
    }
}

#[derive(Debug, Parser)]
#[command(name = "meldritch")]
#[command(about = "Headless Meldritch project tooling")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum TransformKindArg {
    Reverse,
    Reslice,
    Freeze,
    Smear,
}

#[derive(Debug, Subcommand)]
enum Command {
    Validate {
        #[arg(value_name = "PROJECT")]
        project: PathBuf,
    },
    /// Validate a directory-based `.ml*` song and print its resolved identity.
    ValidateSong {
        #[arg(value_name = "SONG_DIRECTORY")]
        song: PathBuf,
    },
    /// List MIDI ports and monitor script-defined MIDI control bindings.
    MidiControlsCheck {
        #[arg(value_name = "SONG_DIRECTORY")]
        song: PathBuf,
        /// Seconds to listen for incoming control messages.
        #[arg(long, default_value_t = 10)]
        seconds: u64,
    },
    /// Capture and transform a range from a WAV into a derived artifact.
    TransformChunk {
        #[arg(value_name = "WAV")]
        input: PathBuf,
        #[arg(long, value_enum)]
        kind: TransformKindArg,
        #[arg(long, default_value_t = 0)]
        start: u32,
        /// Frames to capture; defaults to the remainder of the input.
        #[arg(long)]
        frames: Option<u32>,
        /// Comma-separated reslice permutation, for example `3,2,1,0`.
        #[arg(long, default_value = "1,0")]
        order: String,
        #[arg(long, default_value_t = 0)]
        freeze_frame: u32,
        #[arg(long, default_value_t = 256)]
        smear_radius: u32,
        #[arg(long, default_value = "artifacts/transformed.wav", value_name = "WAV")]
        output: PathBuf,
        #[arg(
            long,
            default_value = "artifacts/transformed.manifest.json",
            value_name = "JSON"
        )]
        manifest: PathBuf,
        #[arg(long)]
        play: bool,
    },
    Inspect {
        #[arg(value_name = "PROJECT")]
        project: PathBuf,
    },
    SummaryJson {
        #[arg(value_name = "PROJECT")]
        project: PathBuf,
    },
    GraphJson {
        #[arg(value_name = "PROJECT")]
        project: PathBuf,
    },
    RelationsJson {
        #[arg(value_name = "PROJECT")]
        project: PathBuf,
    },
    SamplesJson {
        #[arg(value_name = "PROJECT")]
        project: PathBuf,
    },
    EventsJson {
        #[arg(value_name = "PROJECT")]
        project: PathBuf,
        #[arg(long)]
        pattern_id: Option<u64>,
        #[arg(long, default_value_t = 96_000)]
        frames: u64,
    },
    ControlEventsJson {
        #[arg(value_name = "PROJECT")]
        project: PathBuf,
        #[arg(long)]
        pattern_id: u64,
        #[arg(long, default_value_t = 96_000)]
        frames: u64,
    },
    ControlEventsCheck {
        #[arg(value_name = "PROJECT")]
        project: PathBuf,
        #[arg(long)]
        pattern_id: u64,
        #[arg(long, default_value_t = 96_000)]
        frames: u64,
        #[arg(long)]
        events: Option<usize>,
        #[arg(long)]
        controller_patterns: Option<usize>,
        #[arg(long)]
        active_events: Option<usize>,
        #[arg(long)]
        min_active_controllers: Option<usize>,
    },
    DirtyJson {
        #[arg(value_name = "PROJECT")]
        project: PathBuf,
        #[arg(long)]
        source_id: u64,
        #[arg(long)]
        start: u64,
        #[arg(long)]
        end: u64,
    },
    DirtyNoteJson {
        #[arg(value_name = "PROJECT")]
        project: PathBuf,
        #[arg(long)]
        note: u8,
        #[arg(long)]
        start: u64,
        #[arg(long)]
        end: u64,
    },
    DirtyPatternJson {
        #[arg(value_name = "PROJECT")]
        project: PathBuf,
        #[arg(long)]
        pattern_id: u64,
        #[arg(long)]
        start: u64,
        #[arg(long)]
        end: u64,
    },
    RenderClicks {
        #[arg(value_name = "PROJECT")]
        project: PathBuf,
        #[arg(long)]
        pattern_id: Option<u64>,
        #[arg(long, default_value_t = 96_000)]
        frames: u64,
        #[arg(long, default_value_t = 2)]
        channels: u16,
        #[arg(long, value_name = "WAV")]
        output: Option<PathBuf>,
        #[arg(long)]
        normalize: bool,
        #[arg(long)]
        cache_probe: bool,
    },
    RenderSamples {
        #[arg(value_name = "PROJECT")]
        project: PathBuf,
        #[arg(long)]
        pattern_id: Option<u64>,
        #[arg(long, default_value_t = 96_000)]
        frames: u64,
        #[arg(long, default_value_t = 2)]
        channels: u16,
        #[arg(long, value_name = "WAV")]
        output: Option<PathBuf>,
        #[arg(long, value_name = "JSON")]
        manifest: Option<PathBuf>,
        #[arg(long)]
        normalize: bool,
        #[arg(long)]
        cache_probe: bool,
    },
    /// Layer a native synthesized bassline onto a sample-backed drum pattern.
    RenderBassline {
        #[arg(value_name = "PROJECT")]
        project: PathBuf,
        #[arg(long)]
        pattern_id: Option<u64>,
        #[arg(long)]
        frames: Option<u64>,
        #[arg(long, default_value_t = 2)]
        channels: u16,
        #[arg(long, default_value = "artifacts/bassline.wav", value_name = "WAV")]
        output: PathBuf,
        #[arg(long)]
        normalize: bool,
    },
    /// Render drums, relational bass, and a polyphonic chord progression.
    RenderPolyDemo {
        #[arg(value_name = "PROJECT")]
        project: PathBuf,
        #[arg(long, default_value = "artifacts/poly_demo.wav", value_name = "WAV")]
        output: PathBuf,
        #[arg(long, default_value_t = 768_000)]
        frames: u64,
        #[arg(long)]
        normalize: bool,
    },
    /// Render a long intro, groove, breakdown, and full-return arrangement.
    RenderArrangement {
        #[arg(value_name = "PROJECT")]
        project: PathBuf,
        #[arg(long, default_value = "artifacts/arrangement.wav", value_name = "WAV")]
        output: PathBuf,
        #[arg(long)]
        normalize: bool,
    },
    /// Render the deterministic 32-bar arrangement, synth layers, and manifest.
    RenderShowcase {
        #[arg(value_name = "PROJECT", default_value = "fixtures/showcase.toml")]
        project: PathBuf,
        #[arg(long, default_value = "artifacts/showcase.wav", value_name = "WAV")]
        output: PathBuf,
        #[arg(
            long,
            default_value = "artifacts/showcase.manifest.json",
            value_name = "JSON"
        )]
        manifest: PathBuf,
        #[arg(long)]
        normalize: bool,
    },
    /// Render the original 142 BPM warehouse phrase set and sync-fold synths.
    RenderWarehouse {
        #[arg(value_name = "PROJECT", default_value = "fixtures/warehouse.toml")]
        project: PathBuf,
        #[arg(long, default_value = "artifacts/warehouse.wav", value_name = "WAV")]
        output: PathBuf,
        #[arg(
            long,
            default_value = "artifacts/warehouse.manifest.json",
            value_name = "JSON"
        )]
        manifest: PathBuf,
        #[arg(long)]
        normalize: bool,
    },
    /// Render and automatically play the complete 142 BPM warehouse set.
    WarehouseShowcase {
        #[arg(value_name = "PROJECT", default_value = "fixtures/warehouse.toml")]
        project: PathBuf,
        #[arg(long, default_value = "artifacts/warehouse.wav", value_name = "WAV")]
        output: PathBuf,
        #[arg(
            long,
            default_value = "artifacts/warehouse.manifest.json",
            value_name = "JSON"
        )]
        manifest: PathBuf,
        #[arg(long, default_value_t = 1)]
        loops: u32,
        /// Reuse an existing WAV instead of rendering it again.
        #[arg(long)]
        reuse: bool,
        /// Fail if device playback reports an underrun or missed artifact.
        #[arg(long)]
        require_clean: bool,
        /// Limit playback frames for smoke testing.
        #[arg(long)]
        frames: Option<u32>,
    },
    /// Play a rendered showcase through the default host audio device.
    PlayShowcase {
        #[arg(value_name = "WAV", default_value = "artifacts/showcase.wav")]
        audio: PathBuf,
        /// Limit playback for smoke checks; defaults to the complete WAV.
        #[arg(long)]
        frames: Option<u32>,
        #[arg(long, default_value_t = 1)]
        loops: u32,
        /// Fail on any render underrun or missed audio artifact.
        #[arg(long)]
        require_clean: bool,
    },
    /// Play a contiguous range of the demo arrangement on the default device.
    PlayArrangement {
        #[arg(value_name = "PROJECT")]
        project: PathBuf,
        #[arg(long, default_value_t = 0)]
        from_section: usize,
        #[arg(long, default_value_t = 4)]
        to_section: usize,
        #[arg(long, default_value_t = 1)]
        loops: u32,
        #[arg(long)]
        normalize: bool,
    },
    /// Render a sample pattern ahead of time and play it on the default device.
    PlaySamples {
        #[arg(value_name = "PROJECT")]
        project: PathBuf,
        #[arg(long)]
        pattern_id: Option<u64>,
        /// Frames per loop; defaults to one complete pattern cycle.
        #[arg(long)]
        frames: Option<u64>,
        #[arg(long, default_value_t = 2)]
        channels: u16,
        #[arg(long, default_value_t = 4)]
        loops: u32,
        #[arg(long)]
        normalize: bool,
    },
    /// Render future chunks on workers while playing the default device.
    PlayRealtimeSamples {
        #[arg(value_name = "PROJECT")]
        project: PathBuf,
        #[arg(long)]
        pattern_id: Option<u64>,
        /// Frames per loop; defaults to one complete pattern cycle.
        #[arg(long)]
        frames: Option<u64>,
        #[arg(long, default_value_t = 2)]
        channels: u16,
        #[arg(long, default_value_t = 4)]
        loops: u32,
        #[arg(long, default_value_t = 4096)]
        chunk_frames: u32,
        #[arg(long, default_value_t = 4)]
        warm_chunks: usize,
        /// Render workers; zero selects available parallelism.
        #[arg(long, default_value_t = 0)]
        workers: usize,
    },
    /// Open the interactive realtime sample-pattern cockpit.
    TuiSamples {
        #[arg(value_name = "PROJECT")]
        project: PathBuf,
        #[arg(long)]
        pattern_id: Option<u64>,
        #[arg(long)]
        frames: Option<u64>,
        #[arg(long, default_value_t = 2)]
        channels: u16,
        #[arg(long, default_value_t = 4096)]
        chunk_frames: u32,
        #[arg(long, default_value_t = 4)]
        warm_chunks: usize,
        #[arg(long, default_value_t = 0)]
        workers: usize,
        /// Note used when toggling an empty step.
        #[arg(long, default_value_t = 36)]
        note: u8,
    },
    /// Open the interactive cockpit for a directory-based `.ml*` song.
    TuiSong {
        #[arg(value_name = "SONG_DIRECTORY")]
        song: PathBuf,
        #[arg(long)]
        frames: Option<u64>,
        #[arg(long, default_value_t = 4096)]
        chunk_frames: u32,
        /// Render workers for the backing coordinator; zero selects available parallelism.
        #[arg(long, default_value_t = 0)]
        workers: usize,
        /// Disable script-defined MIDI control input.
        #[arg(long = "no-midi-controls", action = clap::ArgAction::SetFalse, default_value_t = true)]
        midi_controls: bool,
    },
    /// Open the realtime cockpit with a native synthesized bassline track.
    TuiBassline {
        #[arg(value_name = "PROJECT")]
        project: PathBuf,
        #[arg(long)]
        pattern_id: Option<u64>,
        #[arg(long)]
        frames: Option<u64>,
        #[arg(long, default_value_t = 2)]
        channels: u16,
        #[arg(long, default_value_t = 4096)]
        chunk_frames: u32,
        #[arg(long, default_value_t = 4)]
        warm_chunks: usize,
        #[arg(long, default_value_t = 0)]
        workers: usize,
        /// Bass note used when toggling an empty step.
        #[arg(long, default_value_t = 24)]
        note: u8,
    },
    /// Open the realtime cockpit with drums, bass, and polyphonic chords.
    TuiPolyDemo {
        #[arg(value_name = "PROJECT")]
        project: PathBuf,
        #[arg(long, default_value_t = 4096)]
        chunk_frames: u32,
        #[arg(long, default_value_t = 4)]
        warm_chunks: usize,
        #[arg(long, default_value_t = 0)]
        workers: usize,
    },
    /// Start an evolving polyphonic set; manual tweaks are saved as future evidence.
    LiveShowcase {
        #[arg(value_name = "PROJECT", default_value = "fixtures/basic_drums.toml")]
        project: PathBuf,
        #[arg(
            long,
            default_value = "artifacts/live_showcase.futures.json",
            value_name = "JSON"
        )]
        futures: PathBuf,
        #[arg(long, default_value_t = 16_384)]
        chunk_frames: u32,
        #[arg(long, default_value_t = 16)]
        warm_chunks: usize,
        #[arg(long, default_value_t = 2)]
        workers: usize,
    },
    /// Open the 142 BPM warehouse cockpit with quantized phrase launching.
    WarehouseCockpit {
        #[arg(value_name = "PROJECT", default_value = "fixtures/warehouse.toml")]
        project: PathBuf,
        #[arg(
            long,
            default_value = "artifacts/warehouse.futures.json",
            value_name = "JSON"
        )]
        futures: PathBuf,
        #[arg(long, default_value_t = 16_384)]
        chunk_frames: u32,
        #[arg(long, default_value_t = 16)]
        warm_chunks: usize,
        #[arg(long, default_value_t = 2)]
        workers: usize,
    },
    /// Soak the host audio device while stressing warehouse DSP rendering.
    WarehouseSoak {
        #[arg(value_name = "WAV", default_value = "artifacts/warehouse.wav")]
        audio: PathBuf,
        #[arg(long, default_value_t = 120)]
        seconds: u32,
        #[arg(long)]
        require_clean: bool,
    },
    RenderControlledSamples {
        #[arg(value_name = "PROJECT")]
        project: PathBuf,
        #[arg(long)]
        pattern_id: u64,
        #[arg(long, default_value_t = 96_000)]
        frames: u64,
        #[arg(long, default_value_t = 2)]
        channels: u16,
        #[arg(long, value_name = "WAV")]
        output: Option<PathBuf>,
        #[arg(long, value_name = "JSON")]
        manifest: Option<PathBuf>,
        #[arg(long, default_value_t = 0.5)]
        active_scale: f64,
        #[arg(long)]
        normalize: bool,
    },
    ManifestSummaryJson {
        #[arg(value_name = "MANIFEST")]
        manifest: PathBuf,
    },
    ManifestCheck {
        #[arg(value_name = "MANIFEST")]
        manifest: PathBuf,
        #[arg(long)]
        pattern_id: Option<u64>,
        #[arg(long)]
        sample_sources: Option<usize>,
        #[arg(long)]
        relations: Option<usize>,
        #[arg(long = "relation-kind", value_name = "KIND=COUNT")]
        relation_kinds: Vec<String>,
        #[arg(long)]
        finite: bool,
        #[arg(long)]
        nonzero: bool,
        #[arg(long)]
        active_scale: Option<f64>,
        #[arg(long)]
        active_events: Option<usize>,
        #[arg(long)]
        max_active_controllers: Option<usize>,
    },
    DirtyStep {
        #[arg(value_name = "PROJECT")]
        project: PathBuf,
        #[arg(long)]
        pattern_id: Option<u64>,
        #[arg(long)]
        step: u32,
        #[arg(long, default_value_t = 0)]
        cycle: u64,
    },
}

fn main() {
    if let Err(err) = run(Cli::parse()) {
        eprintln!("{err}");
        std::process::exit(1);
    }
}

fn run(cli: Cli) -> Result<(), String> {
    match cli.command {
        Command::Validate { project } => validate_project(project),
        Command::ValidateSong { song } => validate_song(song),
        Command::MidiControlsCheck { song, seconds } => midi_controls_check(song, seconds),
        Command::TransformChunk {
            input,
            kind,
            start,
            frames,
            order,
            freeze_frame,
            smear_radius,
            output,
            manifest,
            play,
        } => transform_chunk_command(
            input,
            kind,
            start,
            frames,
            order,
            freeze_frame,
            smear_radius,
            output,
            manifest,
            play,
        ),
        Command::Inspect { project } => inspect_project(project),
        Command::SummaryJson { project } => summarize_project_json(project),
        Command::GraphJson { project } => graph_json(project),
        Command::RelationsJson { project } => relations_json(project),
        Command::SamplesJson { project } => samples_json(project),
        Command::EventsJson {
            project,
            pattern_id,
            frames,
        } => events_json(project, pattern_id, frames),
        Command::ControlEventsJson {
            project,
            pattern_id,
            frames,
        } => control_events_json(project, pattern_id, frames),
        Command::ControlEventsCheck {
            project,
            pattern_id,
            frames,
            events,
            controller_patterns,
            active_events,
            min_active_controllers,
        } => control_events_check(
            project,
            ControlEventsCheckOptions {
                pattern_id,
                frames,
                events,
                controller_patterns,
                active_events,
                min_active_controllers,
            },
        ),
        Command::DirtyJson {
            project,
            source_id,
            start,
            end,
        } => dirty_json(project, source_id, start, end),
        Command::DirtyNoteJson {
            project,
            note,
            start,
            end,
        } => dirty_note_json(project, note, start, end),
        Command::DirtyPatternJson {
            project,
            pattern_id,
            start,
            end,
        } => dirty_pattern_json(project, pattern_id, start, end),
        Command::RenderClicks {
            project,
            pattern_id,
            frames,
            channels,
            output,
            normalize,
            cache_probe,
        } => render_clicks(
            project,
            pattern_id,
            frames,
            channels,
            output,
            normalize,
            cache_probe,
        ),
        Command::RenderSamples {
            project,
            pattern_id,
            frames,
            channels,
            output,
            manifest,
            normalize,
            cache_probe,
        } => render_samples(
            project,
            RenderSamplesOptions {
                pattern_id,
                frames,
                channels,
                output,
                manifest,
                normalize,
                cache_probe,
            },
        ),
        Command::PlaySamples {
            project,
            pattern_id,
            frames,
            channels,
            loops,
            normalize,
        } => play_samples(project, pattern_id, frames, channels, loops, normalize),
        Command::RenderBassline {
            project,
            pattern_id,
            frames,
            channels,
            output,
            normalize,
        } => render_bassline(project, pattern_id, frames, channels, output, normalize),
        Command::RenderPolyDemo {
            project,
            output,
            frames,
            normalize,
        } => render_poly_demo(project, output, frames, normalize),
        Command::RenderArrangement {
            project,
            output,
            normalize,
        } => render_arrangement(project, output, normalize),
        Command::RenderShowcase {
            project,
            output,
            manifest,
            normalize,
        } => render_showcase(project, output, manifest, normalize, false),
        Command::RenderWarehouse {
            project,
            output,
            manifest,
            normalize,
        } => render_showcase(project, output, manifest, normalize, true),
        Command::WarehouseShowcase {
            project,
            output,
            manifest,
            loops,
            reuse,
            require_clean,
            frames,
        } => warehouse_showcase(
            project,
            output,
            manifest,
            loops,
            reuse,
            require_clean,
            frames,
        ),
        Command::PlayShowcase {
            audio,
            frames,
            loops,
            require_clean,
        } => play_showcase(audio, frames, loops, require_clean),
        Command::PlayArrangement {
            project,
            from_section,
            to_section,
            loops,
            normalize,
        } => play_arrangement(project, from_section, to_section, loops, normalize),
        Command::PlayRealtimeSamples {
            project,
            pattern_id,
            frames,
            channels,
            loops,
            chunk_frames,
            warm_chunks,
            workers,
        } => play_realtime_samples(
            project,
            pattern_id,
            frames,
            channels,
            loops,
            chunk_frames,
            warm_chunks,
            workers,
        ),
        Command::TuiSamples {
            project,
            pattern_id,
            frames,
            channels,
            chunk_frames,
            warm_chunks,
            workers,
            note,
        } => tui_samples(
            project,
            pattern_id,
            frames,
            channels,
            chunk_frames,
            warm_chunks,
            workers,
            note,
            false,
            false,
            None,
            false,
        ),
        Command::TuiSong {
            song,
            frames,
            chunk_frames,
            workers,
            midi_controls,
        } => tui_song(song, frames, chunk_frames, workers, midi_controls),
        Command::TuiBassline {
            project,
            pattern_id,
            frames,
            channels,
            chunk_frames,
            warm_chunks,
            workers,
            note,
        } => tui_samples(
            project,
            pattern_id,
            frames,
            channels,
            chunk_frames,
            warm_chunks,
            workers,
            note,
            true,
            false,
            None,
            false,
        ),
        Command::TuiPolyDemo {
            project,
            chunk_frames,
            warm_chunks,
            workers,
        } => tui_samples(
            project,
            None,
            None,
            2,
            chunk_frames,
            warm_chunks,
            workers,
            24,
            true,
            true,
            None,
            false,
        ),
        Command::LiveShowcase {
            project,
            futures,
            chunk_frames,
            warm_chunks,
            workers,
        } => tui_samples(
            project,
            None,
            Some(768_000),
            2,
            chunk_frames,
            warm_chunks,
            workers,
            24,
            true,
            true,
            Some(futures),
            false,
        ),
        Command::WarehouseCockpit {
            project,
            futures,
            chunk_frames,
            warm_chunks,
            workers,
        } => tui_samples(
            project,
            None,
            None,
            2,
            chunk_frames,
            warm_chunks,
            workers,
            24,
            true,
            true,
            Some(futures),
            true,
        ),
        Command::WarehouseSoak {
            audio,
            seconds,
            require_clean,
        } => warehouse_soak(audio, seconds, require_clean),
        Command::RenderControlledSamples {
            project,
            pattern_id,
            frames,
            channels,
            output,
            manifest,
            active_scale,
            normalize,
        } => render_controlled_samples(
            project,
            RenderControlledSamplesOptions {
                pattern_id,
                frames,
                channels,
                output,
                manifest,
                active_scale,
                normalize,
            },
        ),
        Command::ManifestSummaryJson { manifest } => manifest_summary_json(manifest),
        Command::ManifestCheck {
            manifest,
            pattern_id,
            sample_sources,
            relations,
            relation_kinds,
            finite,
            nonzero,
            active_scale,
            active_events,
            max_active_controllers,
        } => manifest_check(
            manifest,
            ManifestCheckOptions {
                pattern_id,
                sample_sources,
                relations,
                relation_kinds,
                finite,
                nonzero,
                active_scale,
                active_events,
                max_active_controllers,
            },
        ),
        Command::DirtyStep {
            project,
            pattern_id,
            step,
            cycle,
        } => dirty_step(project, pattern_id, step, cycle),
    }
}

fn inspect_project(path: PathBuf) -> Result<(), String> {
    let input = std::fs::read_to_string(&path)
        .map_err(|err| format!("failed to read {}: {err}", path.display()))?;
    let project = meldritch_dsl::parse_project(&input).map_err(|err| {
        err.diagnostics()
            .into_iter()
            .map(|diagnostic| diagnostic.message().to_owned())
            .collect::<Vec<_>>()
            .join("\n")
    })?;

    println!("project: {}", project.name());
    println!(
        "tempo: bpm={}, sample_rate={}, seed={}",
        project.tempo().bpm(),
        project.tempo().sample_rate(),
        project.probability_seed().raw()
    );
    println!("samples: {}", project.samples().len());
    for sample in project.samples() {
        println!("  note {} -> {}", sample.note(), sample.path());
    }
    println!("patterns: {}", project.patterns().len());
    for pattern in project.patterns() {
        println!(
            "  pattern {}: length_steps={}, steps_per_beat={}, active_steps={}",
            pattern.id().raw(),
            pattern.length_steps(),
            pattern.steps_per_beat(),
            pattern.active_step_count()
        );
        for (track, count) in pattern.active_step_counts_by_track() {
            println!("    track {}: active_steps={count}", track.raw());
        }
    }

    Ok(())
}

fn graph_json(path: PathBuf) -> Result<(), String> {
    let input = std::fs::read_to_string(&path)
        .map_err(|err| format!("failed to read {}: {err}", path.display()))?;
    let project = meldritch_dsl::parse_project(&input).map_err(|err| {
        err.diagnostics()
            .into_iter()
            .map(|diagnostic| diagnostic.message().to_owned())
            .collect::<Vec<_>>()
            .join("\n")
    })?;
    let compiled = meldritch_dsl::compile_project(&project).map_err(|err| {
        err.diagnostics()
            .iter()
            .map(|diagnostic| diagnostic.message().to_owned())
            .collect::<Vec<_>>()
            .join("\n")
    })?;

    let output = CompiledGraphSummary::from_compiled(&compiled);
    let json = serde_json::to_string_pretty(&output)
        .map_err(|err| format!("failed to encode compiled graph: {err}"))?;
    println!("{json}");

    Ok(())
}

fn relations_json(path: PathBuf) -> Result<(), String> {
    let input = std::fs::read_to_string(&path)
        .map_err(|err| format!("failed to read {}: {err}", path.display()))?;
    let project = meldritch_dsl::parse_project(&input).map_err(|err| {
        err.diagnostics()
            .into_iter()
            .map(|diagnostic| diagnostic.message().to_owned())
            .collect::<Vec<_>>()
            .join("\n")
    })?;
    let compiled = meldritch_dsl::compile_project(&project).map_err(|err| {
        err.diagnostics()
            .iter()
            .map(|diagnostic| diagnostic.message().to_owned())
            .collect::<Vec<_>>()
            .join("\n")
    })?;

    let output = RelationDiagnostics::from_project_and_compiled(&project, &compiled);
    let json = serde_json::to_string_pretty(&output)
        .map_err(|err| format!("failed to encode relation diagnostics: {err}"))?;
    println!("{json}");

    Ok(())
}

fn samples_json(path: PathBuf) -> Result<(), String> {
    let input = std::fs::read_to_string(&path)
        .map_err(|err| format!("failed to read {}: {err}", path.display()))?;
    let project = meldritch_dsl::parse_project(&input).map_err(|err| {
        err.diagnostics()
            .into_iter()
            .map(|diagnostic| diagnostic.message().to_owned())
            .collect::<Vec<_>>()
            .join("\n")
    })?;

    let mut samples = Vec::new();
    for sample_ref in project.samples() {
        let sample_path = resolve_project_path(&path, sample_ref.path());
        let sample = meldritch_audio::read_wav(&sample_path)
            .map_err(|err| format!("failed to read sample {}: {err}", sample_path.display()))?;
        samples.push(SampleDiagnostic::from_sample(
            sample_ref.note(),
            sample_ref.path(),
            &sample_path,
            &sample,
        ));
    }

    let output = SampleDiagnostics {
        schema_version: 1,
        samples,
    };
    let json = serde_json::to_string_pretty(&output)
        .map_err(|err| format!("failed to encode sample diagnostics: {err}"))?;
    println!("{json}");

    Ok(())
}

fn dirty_json(path: PathBuf, source_id: u64, start: u64, end: u64) -> Result<(), String> {
    let compiled = compile_project_file(&path)?;
    emit_dirty_json(&compiled, source_id, start, end)
}

fn dirty_note_json(path: PathBuf, note: u8, start: u64, end: u64) -> Result<(), String> {
    let compiled = compile_project_file(&path)?;
    let source_id = compiled
        .source_bindings()
        .iter()
        .find_map(|binding| match binding.kind() {
            meldritch_dsl::SourceBindingKind::Sample {
                note: sample_note, ..
            } if *sample_note == note => Some(binding.source().raw()),
            _ => None,
        })
        .ok_or_else(|| format!("compiled graph has no sample source for note {note}"))?;

    emit_dirty_json(&compiled, source_id, start, end)
}

fn dirty_pattern_json(path: PathBuf, pattern_id: u64, start: u64, end: u64) -> Result<(), String> {
    let compiled = compile_project_file(&path)?;
    let pattern = meldritch_core::PatternId::new(pattern_id);
    let source_id = compiled
        .source_bindings()
        .iter()
        .find_map(|binding| match binding.kind() {
            meldritch_dsl::SourceBindingKind::Pattern {
                pattern: binding_pattern,
            } if *binding_pattern == pattern => Some(binding.source().raw()),
            _ => None,
        })
        .ok_or_else(|| format!("compiled graph has no pattern source for pattern {pattern_id}"))?;

    emit_dirty_json(&compiled, source_id, start, end)
}

fn manifest_summary_json(path: PathBuf) -> Result<(), String> {
    let input = std::fs::read_to_string(&path)
        .map_err(|err| format!("failed to read {}: {err}", path.display()))?;
    let manifest = serde_json::from_str::<serde_json::Value>(&input)
        .map_err(|err| format!("failed to parse {}: {err}", path.display()))?;
    let summary = RenderManifestDiagnostics::from_manifest(&manifest)?;
    let json = serde_json::to_string_pretty(&summary)
        .map_err(|err| format!("failed to encode manifest summary: {err}"))?;
    println!("{json}");

    Ok(())
}

struct ManifestCheckOptions {
    pattern_id: Option<u64>,
    sample_sources: Option<usize>,
    relations: Option<usize>,
    relation_kinds: Vec<String>,
    finite: bool,
    nonzero: bool,
    active_scale: Option<f64>,
    active_events: Option<usize>,
    max_active_controllers: Option<usize>,
}

fn manifest_check(path: PathBuf, options: ManifestCheckOptions) -> Result<(), String> {
    let input = std::fs::read_to_string(&path)
        .map_err(|err| format!("failed to read {}: {err}", path.display()))?;
    let manifest = serde_json::from_str::<serde_json::Value>(&input)
        .map_err(|err| format!("failed to parse {}: {err}", path.display()))?;
    let summary = RenderManifestDiagnostics::from_manifest(&manifest)?;

    if let Some(expected) = options.pattern_id {
        ensure_equal("pattern_id", summary.pattern_id, expected)?;
    }
    if let Some(expected) = options.sample_sources {
        ensure_equal("sample_source_count", summary.sample_source_count, expected)?;
    }
    if let Some(expected) = options.relations {
        ensure_equal("relation_count", summary.relation_count, expected)?;
    }
    for expected in options.relation_kinds {
        let (kind, count) = parse_expected_relation_kind(&expected)?;
        let actual = summary.relation_kinds.get(kind).copied().unwrap_or(0);
        ensure_equal(&format!("relation kind {kind}"), actual, count)?;
    }
    if options.finite && !summary.result.finite {
        return Err("manifest result is not finite".to_owned());
    }
    if options.nonzero && summary.result.nonzero_samples == 0 {
        return Err("manifest result has no nonzero samples".to_owned());
    }
    if let Some(expected) = options.active_scale {
        let Some(control) = &summary.control else {
            return Err("manifest has no control summary".to_owned());
        };
        ensure_float_close("control active_scale", control.active_scale, expected)?;
    }
    if let Some(expected) = options.active_events {
        let Some(control) = &summary.control else {
            return Err("manifest has no control summary".to_owned());
        };
        ensure_equal(
            "control active_event_count",
            control.active_event_count,
            expected,
        )?;
    }
    if let Some(expected) = options.max_active_controllers {
        let Some(control) = &summary.control else {
            return Err("manifest has no control summary".to_owned());
        };
        ensure_equal(
            "control max_active_controller_count",
            control.max_active_controller_count,
            expected,
        )?;
    }

    println!("manifest ok: {}", path.display());
    Ok(())
}

fn parse_expected_relation_kind(input: &str) -> Result<(&str, usize), String> {
    let (kind, count) = input
        .split_once('=')
        .ok_or_else(|| format!("relation kind expectation must be KIND=COUNT, got '{input}'"))?;
    if kind.is_empty() {
        return Err(format!(
            "relation kind expectation must include a kind, got '{input}'"
        ));
    }
    let count = count
        .parse::<usize>()
        .map_err(|err| format!("relation kind expectation '{input}' has invalid count: {err}"))?;
    Ok((kind, count))
}

fn ensure_equal<T>(name: &str, actual: T, expected: T) -> Result<(), String>
where
    T: std::fmt::Display + PartialEq,
{
    if actual == expected {
        Ok(())
    } else {
        Err(format!(
            "manifest {name} mismatch: expected {expected}, got {actual}"
        ))
    }
}

fn ensure_at_least<T>(name: &str, actual: T, minimum: T) -> Result<(), String>
where
    T: std::fmt::Display + PartialOrd,
{
    if actual >= minimum {
        Ok(())
    } else {
        Err(format!(
            "manifest {name} mismatch: expected at least {minimum}, got {actual}"
        ))
    }
}

fn ensure_float_close(name: &str, actual: f64, expected: f64) -> Result<(), String> {
    if (actual - expected).abs() <= 1.0e-9 {
        Ok(())
    } else {
        Err(format!(
            "manifest {name} mismatch: expected {expected}, got {actual}"
        ))
    }
}

fn compile_project_file(path: &Path) -> Result<meldritch_dsl::CompiledProject, String> {
    let input = std::fs::read_to_string(path)
        .map_err(|err| format!("failed to read {}: {err}", path.display()))?;
    let project = meldritch_dsl::parse_project(&input).map_err(|err| {
        err.diagnostics()
            .into_iter()
            .map(|diagnostic| diagnostic.message().to_owned())
            .collect::<Vec<_>>()
            .join("\n")
    })?;
    meldritch_dsl::compile_project(&project).map_err(|err| {
        err.diagnostics()
            .iter()
            .map(|diagnostic| diagnostic.message().to_owned())
            .collect::<Vec<_>>()
            .join("\n")
    })
}

fn emit_dirty_json(
    compiled: &meldritch_dsl::CompiledProject,
    source_id: u64,
    start: u64,
    end: u64,
) -> Result<(), String> {
    let range = FrameRange::new(start, end).map_err(|err| err.to_string())?;
    let source = SourceId::new(source_id);
    let node = compiled
        .sources()
        .get(source)
        .ok_or_else(|| format!("compiled graph has no source {source_id}"))?
        .node();
    let dirty = compiled.relations().invalidate_from(node, range);
    let output = DirtyGraphSummary::from_dirty(source_id, node.raw(), range, &dirty);
    let json = serde_json::to_string_pretty(&output)
        .map_err(|err| format!("failed to encode dirty graph summary: {err}"))?;
    println!("{json}");

    Ok(())
}

fn events_json(path: PathBuf, pattern_id: Option<u64>, frames: u64) -> Result<(), String> {
    let input = std::fs::read_to_string(&path)
        .map_err(|err| format!("failed to read {}: {err}", path.display()))?;
    let project = meldritch_dsl::parse_project(&input).map_err(|err| {
        err.diagnostics()
            .into_iter()
            .map(|diagnostic| diagnostic.message().to_owned())
            .collect::<Vec<_>>()
            .join("\n")
    })?;
    let pattern = select_pattern(&project, pattern_id, "schedule")?;
    let range = FrameRange::new(0, frames).map_err(|err| err.to_string())?;

    let mut events = Vec::new();
    pattern.events_between(
        project.tempo(),
        range,
        project.probability_seed(),
        &mut events,
    );

    let output = EventSchedule::from_events(pattern.id().raw(), range, &events);
    let json = serde_json::to_string_pretty(&output)
        .map_err(|err| format!("failed to encode event schedule: {err}"))?;
    println!("{json}");

    Ok(())
}

fn control_events_json(path: PathBuf, pattern_id: u64, frames: u64) -> Result<(), String> {
    let output = build_control_event_schedule(&path, pattern_id, frames)?;
    let json = serde_json::to_string_pretty(&output)
        .map_err(|err| format!("failed to encode control event schedule: {err}"))?;
    println!("{json}");

    Ok(())
}

struct ControlEventsCheckOptions {
    pattern_id: u64,
    frames: u64,
    events: Option<usize>,
    controller_patterns: Option<usize>,
    active_events: Option<usize>,
    min_active_controllers: Option<usize>,
}

fn control_events_check(path: PathBuf, options: ControlEventsCheckOptions) -> Result<(), String> {
    let output = build_control_event_schedule(&path, options.pattern_id, options.frames)?;

    if let Some(expected) = options.events {
        ensure_equal("control event count", output.events.len(), expected)?;
    }
    if let Some(expected) = options.controller_patterns {
        ensure_equal(
            "controller pattern count",
            output.controller_patterns.len(),
            expected,
        )?;
    }
    if let Some(expected) = options.active_events {
        ensure_equal(
            "active control event count",
            output.active_event_count,
            expected,
        )?;
    }
    if let Some(expected) = options.min_active_controllers {
        ensure_at_least(
            "max active controllers",
            output.max_active_controller_count,
            expected,
        )?;
    }

    println!("control events ok: {}", path.display());
    Ok(())
}

fn build_control_event_schedule(
    path: &Path,
    pattern_id: u64,
    frames: u64,
) -> Result<ControlEventSchedule, String> {
    let input = std::fs::read_to_string(path)
        .map_err(|err| format!("failed to read {}: {err}", path.display()))?;
    let project = meldritch_dsl::parse_project(&input).map_err(|err| {
        err.diagnostics()
            .into_iter()
            .map(|diagnostic| diagnostic.message().to_owned())
            .collect::<Vec<_>>()
            .join("\n")
    })?;
    let compiled = meldritch_dsl::compile_project(&project).map_err(|err| {
        err.diagnostics()
            .iter()
            .map(|diagnostic| diagnostic.message().to_owned())
            .collect::<Vec<_>>()
            .join("\n")
    })?;
    let pattern = select_pattern(&project, Some(pattern_id), "inspect control events")?;
    let range = FrameRange::new(0, frames).map_err(|err| err.to_string())?;

    let mut target_events = Vec::new();
    pattern.events_between(
        project.tempo(),
        range,
        project.probability_seed(),
        &mut target_events,
    );

    let controller_patterns = incoming_controller_patterns(&compiled, pattern.id());
    let mut controller_events = Vec::new();
    for controller_pattern in &controller_patterns {
        let controller = select_pattern(
            &project,
            Some(controller_pattern.raw()),
            "inspect control events",
        )?;
        let mut events = Vec::new();
        controller.events_between(
            project.tempo(),
            range,
            project.probability_seed(),
            &mut events,
        );
        controller_events.push(ControllerEvents {
            pattern: *controller_pattern,
            events,
        });
    }

    Ok(ControlEventSchedule::from_events(
        pattern.id().raw(),
        range,
        &target_events,
        &controller_events,
    ))
}

fn incoming_controller_patterns(
    compiled: &meldritch_dsl::CompiledProject,
    target: meldritch_core::PatternId,
) -> Vec<meldritch_core::PatternId> {
    compiled
        .relation_bindings()
        .iter()
        .filter_map(|binding| match binding.kind() {
            meldritch_dsl::RelationBindingKind::PatternControlsPattern {
                from_pattern,
                to_pattern,
            } if *to_pattern == target => Some(*from_pattern),
            _ => None,
        })
        .collect()
}

fn summarize_project_json(path: PathBuf) -> Result<(), String> {
    let input = std::fs::read_to_string(&path)
        .map_err(|err| format!("failed to read {}: {err}", path.display()))?;
    let project = meldritch_dsl::parse_project(&input).map_err(|err| {
        err.diagnostics()
            .into_iter()
            .map(|diagnostic| diagnostic.message().to_owned())
            .collect::<Vec<_>>()
            .join("\n")
    })?;

    let summary = ProjectSummary::from_project(&project);
    let output = serde_json::to_string_pretty(&summary)
        .map_err(|err| format!("failed to encode project summary: {err}"))?;
    println!("{output}");

    Ok(())
}

fn validate_song(path: PathBuf) -> Result<(), String> {
    let song = meldritch_dsl::load_song_directory(&path).map_err(|error| error.to_string())?;
    println!(
        "valid song '{}' · {} track(s) · {} synth(s) · {} DSP graph(s) · {} note pattern(s) · {} parameter pattern(s) · fingerprint {:016x}",
        song.performance().title(),
        song.performance().tracks().len(),
        song.synths().len(),
        song.dsps().len(),
        song.note_patterns().len(),
        song.parameter_patterns().len(),
        song.fingerprint()
    );
    Ok(())
}

struct SongRenderRequest {
    generation: u64,
    overrides: SongLiveOverrides,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct SongLiveOverrides {
    feedback: Option<f64>,
    cutoff_hz: Option<f64>,
}

struct SongRerenderWorker {
    shared: Arc<(Mutex<SongRerenderState>, Condvar)>,
    completed: mpsc::Receiver<(u64, meldritch_audio::AudioBlock, SongLiveOverrides)>,
    thread: Option<JoinHandle<()>>,
}

#[derive(Default)]
struct SongRerenderState {
    latest: Option<SongRenderRequest>,
    shutdown: bool,
}

impl SongRerenderWorker {
    fn new(patch: meldritch_render::song_render::CompiledDelayedNotePatch, frames: u32) -> Self {
        let shared = Arc::new((Mutex::new(SongRerenderState::default()), Condvar::new()));
        let thread_shared = Arc::clone(&shared);
        let (completed_tx, completed) = mpsc::channel();
        let thread = thread::spawn(move || {
            let range = FrameRange::new(0, u64::from(frames)).expect("song range is ordered");
            loop {
                let request = {
                    let (lock, changed) = &*thread_shared;
                    let mut state = lock.lock().expect("song render worker lock poisoned");
                    while state.latest.is_none() && !state.shutdown {
                        state = changed
                            .wait(state)
                            .expect("song render worker condvar poisoned");
                    }
                    if state.shutdown {
                        break;
                    }
                    state
                        .latest
                        .take()
                        .expect("latest request exists after wait")
                };
                let Ok(block) = patch.render_with_overrides(
                    range,
                    request.overrides.feedback,
                    request.overrides.cutoff_hz,
                ) else {
                    continue;
                };
                if completed_tx
                    .send((request.generation, block, request.overrides))
                    .is_err()
                {
                    break;
                }
            }
        });
        Self {
            shared,
            completed,
            thread: Some(thread),
        }
    }

    fn submit(&mut self, generation: u64, overrides: SongLiveOverrides) {
        let (lock, changed) = &*self.shared;
        lock.lock()
            .expect("song render worker lock poisoned")
            .latest = Some(SongRenderRequest {
            generation,
            overrides,
        });
        changed.notify_one();
    }

    fn latest_completed(&self) -> Option<(u64, meldritch_audio::AudioBlock, SongLiveOverrides)> {
        self.completed
            .try_iter()
            .max_by_key(|(generation, _, _)| *generation)
    }
}

impl Drop for SongRerenderWorker {
    fn drop(&mut self) {
        let (lock, changed) = &*self.shared;
        lock.lock()
            .expect("song render worker lock poisoned")
            .shutdown = true;
        changed.notify_one();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct CapturedSessionEvent {
    sequence: u64,
    wall_offset_ms: u128,
    absolute_frame: u32,
    musical_beat: f64,
    requested_quantization: String,
    actual_frame: u32,
    provenance: String,
    input: String,
    command: String,
    result: String,
    changed: bool,
    kind: String,
    target_id: Option<String>,
    previous: Option<String>,
    current: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
struct FinalSessionState {
    cockpit_mode: String,
    transport_state: String,
    transport_position: u32,
    selection_track: u64,
    selection_step: u32,
    history_len: usize,
    curated_controls: Vec<FinalSessionControl>,
}

#[derive(Clone, Debug, PartialEq)]
struct FinalSessionControl {
    id: String,
    target: String,
    value: Option<f64>,
}

#[derive(Debug)]
struct PerformanceSessionCapture {
    path: PathBuf,
    temp_path: PathBuf,
    session_id: String,
    created_at_utc: String,
    source_performance: String,
    source_title: String,
    song_root: String,
    source_fingerprint: u64,
    timeline_frames: u32,
    started: Instant,
    events: Vec<CapturedSessionEvent>,
    uncheckpointed_events: usize,
    event_buffer_limit: usize,
    final_state: Option<FinalSessionState>,
    clean_termination: bool,
    termination: String,
}

const SESSION_EVENT_BUFFER_LIMIT: usize = 8;

impl PerformanceSessionCapture {
    fn create(song: &meldritch_dsl::ValidatedSong, timeline_frames: u32) -> Result<Self, String> {
        let timestamp = utc_session_timestamp()?;
        Self::create_at(song.root(), song, timeline_frames, &timestamp)
    }

    fn create_at(
        session_root: &Path,
        song: &meldritch_dsl::ValidatedSong,
        timeline_frames: u32,
        timestamp: &str,
    ) -> Result<Self, String> {
        Self::create_at_with_buffer_limit(
            session_root,
            song,
            timeline_frames,
            timestamp,
            SESSION_EVENT_BUFFER_LIMIT,
        )
    }

    fn create_at_with_buffer_limit(
        session_root: &Path,
        song: &meldritch_dsl::ValidatedSong,
        timeline_frames: u32,
        timestamp: &str,
        event_buffer_limit: usize,
    ) -> Result<Self, String> {
        if event_buffer_limit == 0 {
            return Err("session event buffer limit must be greater than zero".to_owned());
        }
        let directory = session_root.join("performances");
        std::fs::create_dir_all(&directory)
            .map_err(|error| format!("failed to create {}: {error}", directory.display()))?;
        let (path, session_id) = reserve_session_path(&directory, timestamp)?;
        let temp_path = path.with_extension("mlperformance.tmp");
        let mut capture = Self {
            path,
            temp_path,
            session_id,
            created_at_utc: timestamp.to_owned(),
            source_performance: song.performance().id().to_owned(),
            source_title: song.performance().title().to_owned(),
            song_root: song.root().display().to_string(),
            source_fingerprint: song.fingerprint(),
            timeline_frames,
            started: Instant::now(),
            events: Vec::new(),
            uncheckpointed_events: 0,
            event_buffer_limit,
            final_state: None,
            clean_termination: false,
            termination: "running".to_owned(),
        };
        capture.checkpoint()?;
        Ok(capture)
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn record(
        &mut self,
        controller: &meldritch_app::AppController,
        input: &meldritch_app::AppInput,
        result: &meldritch_app::AppCommandResult,
        frame_count: u32,
        tempo: Tempo,
    ) -> Result<(), String> {
        let view = controller.view_model();
        let latest = controller.history().back();
        let sequence = latest.map_or(self.events.len() as u64, |record| record.sequence);
        let command = latest.map_or_else(
            || "<not-recorded>".to_owned(),
            |record| format!("{:?}", record.command),
        );
        let changed = latest.is_none_or(|record| record.changed);
        let absolute_frame = view.transport.position;
        let musical_beat = f64::from(absolute_frame) / tempo.frames_per_beat().max(f64::EPSILON);
        let (kind, target_id, previous, current) = session_result_fields(input, result);
        self.push_event(CapturedSessionEvent {
            sequence,
            wall_offset_ms: self.started.elapsed().as_millis(),
            absolute_frame,
            musical_beat,
            requested_quantization: "immediate".to_owned(),
            actual_frame: absolute_frame.min(frame_count.saturating_sub(1)),
            provenance: "performer".to_owned(),
            input: format!("{input:?}"),
            command,
            result: format!("{result:?}"),
            changed,
            kind,
            target_id,
            previous,
            current,
        })
    }

    fn push_event(&mut self, event: CapturedSessionEvent) -> Result<(), String> {
        self.events.push(event);
        self.uncheckpointed_events += 1;
        if self.uncheckpointed_events >= self.event_buffer_limit {
            self.checkpoint()?;
        }
        Ok(())
    }

    fn capture_final_state(&mut self, controller: &meldritch_app::AppController) {
        self.final_state = Some(FinalSessionState::from_view(&controller.view_model()));
    }

    fn finish_clean(&mut self, controller: &meldritch_app::AppController) -> Result<(), String> {
        self.capture_final_state(controller);
        self.finish_clean_without_controller()
    }

    fn finish_clean_without_controller(&mut self) -> Result<(), String> {
        self.clean_termination = true;
        self.termination = "clean".to_owned();
        self.checkpoint()
    }

    fn checkpoint(&mut self) -> Result<(), String> {
        let encoded = self.to_toml();
        std::fs::write(&self.temp_path, encoded)
            .map_err(|error| format!("failed to write {}: {error}", self.temp_path.display()))?;
        std::fs::rename(&self.temp_path, &self.path).map_err(|error| {
            format!("failed to publish session {}: {error}", self.path.display())
        })?;
        self.uncheckpointed_events = 0;
        Ok(())
    }

    fn to_toml(&self) -> String {
        let mut out = String::new();
        out.push_str("[meldritch]\n");
        out.push_str("kind = \"performance_session\"\n");
        out.push_str("version = 1\n\n");
        out.push_str("[session]\n");
        push_string(&mut out, "id", &self.session_id);
        push_string(&mut out, "created_at_utc", &self.created_at_utc);
        push_string(&mut out, "source_performance", &self.source_performance);
        push_string(&mut out, "source_title", &self.source_title);
        push_string(&mut out, "song_root", &self.song_root);
        push_string(
            &mut out,
            "source_fingerprint",
            &format!("{:016x}", self.source_fingerprint),
        );
        out.push_str(&format!("timeline_frames = {}\n", self.timeline_frames));
        out.push_str(&format!(
            "event_buffer_limit = {}\n",
            self.event_buffer_limit
        ));
        out.push_str(&format!("clean_termination = {}\n", self.clean_termination));
        push_string(&mut out, "termination", &self.termination);
        out.push('\n');
        if let Some(final_state) = &self.final_state {
            out.push_str("[final_state]\n");
            push_string(&mut out, "cockpit_mode", &final_state.cockpit_mode);
            push_string(&mut out, "transport_state", &final_state.transport_state);
            out.push_str(&format!(
                "transport_position = {}\n",
                final_state.transport_position
            ));
            out.push_str(&format!(
                "selection_track = {}\n",
                final_state.selection_track
            ));
            out.push_str(&format!(
                "selection_step = {}\n",
                final_state.selection_step
            ));
            out.push_str(&format!("history_len = {}\n\n", final_state.history_len));
            for control in &final_state.curated_controls {
                out.push_str("[[final_controls]]\n");
                push_string(&mut out, "id", &control.id);
                push_string(&mut out, "target", &control.target);
                if let Some(value) = control.value {
                    out.push_str(&format!("value = {:.12}\n", value));
                }
                out.push('\n');
            }
        }
        for event in &self.events {
            out.push_str("[[events]]\n");
            out.push_str(&format!("sequence = {}\n", event.sequence));
            out.push_str(&format!("wall_offset_ms = {}\n", event.wall_offset_ms));
            out.push_str(&format!("absolute_frame = {}\n", event.absolute_frame));
            out.push_str(&format!("musical_beat = {:.9}\n", event.musical_beat));
            push_string(
                &mut out,
                "requested_quantization",
                &event.requested_quantization,
            );
            out.push_str(&format!("actual_frame = {}\n", event.actual_frame));
            push_string(&mut out, "provenance", &event.provenance);
            push_string(&mut out, "input", &event.input);
            push_string(&mut out, "command", &event.command);
            push_string(&mut out, "result", &event.result);
            out.push_str(&format!("changed = {}\n", event.changed));
            push_string(&mut out, "kind", &event.kind);
            if let Some(target_id) = &event.target_id {
                push_string(&mut out, "target_id", target_id);
            }
            if let Some(previous) = &event.previous {
                push_string(&mut out, "previous", previous);
            }
            if let Some(current) = &event.current {
                push_string(&mut out, "current", current);
            }
            out.push('\n');
        }
        out
    }
}

impl FinalSessionState {
    fn from_view(view: &meldritch_app::AppViewModel) -> Self {
        Self {
            cockpit_mode: format!("{:?}", view.cockpit_mode),
            transport_state: format!("{:?}", view.transport.state),
            transport_position: view.transport.position,
            selection_track: view.diagnostics.selection.track.raw(),
            selection_step: view.diagnostics.selection.step.raw(),
            history_len: view.diagnostics.history_len,
            curated_controls: view
                .curated_controls
                .iter()
                .map(|control| FinalSessionControl {
                    id: control.id.clone(),
                    target: control.target.clone(),
                    value: control.value,
                })
                .collect(),
        }
    }
}

fn session_result_fields(
    input: &meldritch_app::AppInput,
    result: &meldritch_app::AppCommandResult,
) -> (String, Option<String>, Option<String>, Option<String>) {
    match result {
        meldritch_app::AppCommandResult::CuratedControlAdjusted {
            id,
            previous,
            current,
        } => (
            "curated_control".to_owned(),
            Some(id.clone()),
            Some(format!("{previous:.12}")),
            Some(format!("{current:.12}")),
        ),
        meldritch_app::AppCommandResult::CockpitModeChanged { previous, current } => (
            "cockpit_mode".to_owned(),
            None,
            Some(format!("{previous:?}")),
            Some(format!("{current:?}")),
        ),
        meldritch_app::AppCommandResult::SelectionChanged { previous, current } => (
            "selection".to_owned(),
            None,
            Some(format!("{previous:?}")),
            Some(format!("{current:?}")),
        ),
        meldritch_app::AppCommandResult::TransportQueued => (
            "transport".to_owned(),
            Some(
                match input {
                    meldritch_app::AppInput::TogglePlayback => "toggle_playback",
                    meldritch_app::AppInput::Stop => "stop",
                    meldritch_app::AppInput::Rewind => "rewind",
                    _ => "transport",
                }
                .to_owned(),
            ),
            None,
            None,
        ),
        meldritch_app::AppCommandResult::Edit(_) => (
            "parameter_edit".to_owned(),
            Some(format!("{input:?}")),
            None,
            Some(format!("{result:?}")),
        ),
        meldritch_app::AppCommandResult::SynthUpdated { .. } => (
            "synth_control".to_owned(),
            Some(format!("{input:?}")),
            None,
            Some(format!("{result:?}")),
        ),
        meldritch_app::AppCommandResult::PerformanceFxUpdated(_) => (
            "performance_fx".to_owned(),
            Some(format!("{input:?}")),
            None,
            Some(format!("{result:?}")),
        ),
        meldritch_app::AppCommandResult::TransformCreated { .. } => (
            "transform".to_owned(),
            Some(format!("{input:?}")),
            None,
            Some(format!("{result:?}")),
        ),
        meldritch_app::AppCommandResult::AudioSourceSwitched { transformed } => (
            "audio_source".to_owned(),
            None,
            None,
            Some(if *transformed { "transformed" } else { "live" }.to_owned()),
        ),
        meldritch_app::AppCommandResult::PerformanceQueued(_) => (
            "performance_queue".to_owned(),
            Some(format!("{input:?}")),
            None,
            Some(format!("{result:?}")),
        ),
        meldritch_app::AppCommandResult::PerformanceCancelled(_) => (
            "performance_cancel".to_owned(),
            Some(format!("{input:?}")),
            None,
            Some(format!("{result:?}")),
        ),
    }
}

fn reserve_session_path(directory: &Path, timestamp: &str) -> Result<(PathBuf, String), String> {
    for suffix in 0..1_000_u16 {
        let session_id = if suffix == 0 {
            format!("session-{timestamp}")
        } else {
            format!("session-{timestamp}-{suffix:03}")
        };
        let path = directory.join(format!("{session_id}.mlperformance"));
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(_) => return Ok((path, session_id)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => {
                return Err(format!("failed to reserve {}: {error}", path.display()));
            }
        }
    }
    Err(format!(
        "could not reserve a unique session filename in {} for {timestamp}",
        directory.display()
    ))
}

fn utc_session_timestamp() -> Result<String, String> {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("system clock is before the Unix epoch: {error}"))?;
    Ok(format_unix_timestamp_utc(elapsed.as_secs()))
}

fn format_unix_timestamp_utc(seconds: u64) -> String {
    let days = (seconds / 86_400) as i64;
    let seconds_of_day = seconds % 86_400;
    let (year, month, day) = civil_from_days(days);
    let hour = seconds_of_day / 3_600;
    let minute = (seconds_of_day % 3_600) / 60;
    let second = seconds_of_day % 60;
    format!("{year:04}{month:02}{day:02}T{hour:02}{minute:02}{second:02}Z")
}

fn civil_from_days(days_since_unix_epoch: i64) -> (i64, u64, u64) {
    let days = days_since_unix_epoch + 719_468;
    let era = if days >= 0 { days } else { days - 146_096 } / 146_097;
    let day_of_era = days - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    let year = year + i64::from(month <= 2);
    (year, month as u64, day as u64)
}

fn push_string(out: &mut String, key: &str, value: &str) {
    out.push_str(key);
    out.push_str(" = ");
    out.push_str(&toml_string(value));
    out.push('\n');
}

fn toml_string(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len() + 2);
    escaped.push('"');
    for character in value.chars() {
        match character {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            character if character.is_control() => {
                escaped.push_str(&format!("\\u{:04x}", character as u32));
            }
            character => escaped.push(character),
        }
    }
    escaped.push('"');
    escaped
}

struct MidiControlInputConnection {
    _connection: midir::MidiInputConnection<()>,
}

#[derive(Clone, Debug, PartialEq)]
struct MidiActionBinding {
    device: String,
    message: MidiActionMessage,
    input: meldritch_app::AppInput,
    label: String,
}

#[derive(Clone, Debug, PartialEq)]
enum MidiActionMessage {
    ControlChange { channel: Option<u8>, cc: u8 },
    Note { channel: Option<u8>, note: u8 },
}

#[derive(Clone, Debug, PartialEq)]
struct MidiControlDiagnosticEvent {
    device: String,
    port: String,
    message: MidiDiagnosticMessage,
    mapped: Option<MappedMidiInput>,
}

#[derive(Clone, Debug, PartialEq)]
struct MappedMidiInput {
    input: meldritch_app::AppInput,
    label: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
enum MidiDiagnosticMessage {
    ControlChange {
        channel: u8,
        cc: u8,
        value: u8,
    },
    Note {
        channel: u8,
        note: u8,
        velocity: u8,
        on: bool,
    },
    Raw(Vec<u8>),
}

impl std::fmt::Display for MidiDiagnosticMessage {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ControlChange { channel, cc, value } => {
                write!(formatter, "cc ch {channel} cc {cc} value {value}")
            }
            Self::Note {
                channel,
                note,
                velocity,
                on,
            } => write!(
                formatter,
                "note {} ch {channel} note {note} velocity {velocity}",
                if *on { "on" } else { "off" }
            ),
            Self::Raw(bytes) => write!(formatter, "raw {bytes:?}"),
        }
    }
}

impl MappedMidiInput {
    fn display_text(&self) -> String {
        match &self.label {
            Some(label) => format!("{label} ({:?})", self.input),
            None => format!("{:?}", self.input),
        }
    }
}

fn midi_controls_check(path: PathBuf, seconds: u64) -> Result<(), String> {
    let song = meldritch_dsl::load_song_directory(&path).map_err(|error| error.to_string())?;
    let midi_input = midir::MidiInput::new("meldritch-midi-port-list")
        .map_err(|error| midi_input_client_error("failed to create MIDI input client", &error))?;
    let ports = midi_input.ports();
    println!("visible MIDI input ports:");
    if ports.is_empty() {
        println!("  <none>");
    } else {
        for (index, port) in ports.iter().enumerate() {
            let name = midi_input
                .port_name(port)
                .unwrap_or_else(|_| "<unreadable port name>".to_owned());
            println!("  {index}: {name}");
        }
    }
    drop(midi_input);

    let bindings = midi_control_bindings_for_song(&song);
    let action_bindings = midi_action_bindings_for_song(&song);
    let (sender, receiver) = mpsc::channel();
    let mut connections = Vec::new();
    for device in song.performance().midi_devices() {
        let device_bindings = midi_control_bindings_for_device(&bindings, device.id());
        let device_action_bindings = midi_action_bindings_for_device(&action_bindings, device.id());
        println!(
            "script device '{}' matches ports containing '{}' with {} control binding(s), {} action binding(s)",
            device.id(),
            device.name_contains(),
            device_bindings.len(),
            device_action_bindings.len()
        );
        if device_bindings.is_empty() && device_action_bindings.is_empty() {
            continue;
        }
        match connect_midi_control_diagnostic(
            device,
            sender.clone(),
            device_bindings,
            device_action_bindings,
        )? {
            Some(connection) => connections.push(connection),
            None => println!("  no matching port opened for '{}'", device.id()),
        }
    }
    if connections.is_empty() {
        println!("no script-declared MIDI devices were opened");
        return Ok(());
    }

    println!("listening for MIDI controls for {seconds}s; move knobs/faders or press buttons");
    let deadline = Instant::now() + Duration::from_secs(seconds);
    let mut events = 0_u64;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            break;
        }
        let timeout = remaining.min(Duration::from_millis(250));
        match receiver.recv_timeout(timeout) {
            Ok(event) => {
                events = events.saturating_add(1);
                let mapped = event
                    .mapped
                    .as_ref()
                    .map_or_else(|| "unmapped".to_owned(), MappedMidiInput::display_text);
                println!(
                    "{} · {} · {} -> {}",
                    event.device, event.port, event.message, mapped
                );
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
    println!("observed {events} MIDI control event(s)");
    Ok(())
}

fn connect_midi_control_input(
    device: &meldritch_dsl::MidiDeviceDefinition,
    sender: mpsc::Sender<meldritch_app::AppInput>,
    bindings: Vec<meldritch_app::MidiControlBinding>,
    action_bindings: Vec<MidiActionBinding>,
) -> Result<Option<MidiControlInputConnection>, String> {
    let mut midi_input = midir::MidiInput::new(&format!("meldritch-midi-{}", device.id()))
        .map_err(|error| midi_input_client_error("failed to create MIDI input client", &error))?;
    midi_input.ignore(midir::Ignore::None);
    let Some((port, name)) = midi_input
        .ports()
        .into_iter()
        .filter_map(|port| {
            let name = midi_input.port_name(&port).ok()?;
            midi_port_name_matches(&name, device.name_contains()).then_some((port, name))
        })
        .next()
    else {
        return Ok(None);
    };
    let device_id = device.id().to_owned();
    let connection = midi_input
        .connect(
            &port,
            &format!("meldritch-midi-{}-input", device.id()),
            move |_timestamp, message, _state| {
                let input =
                    decode_script_midi_input(message, &device_id, &bindings, &action_bindings);
                let Some(input) = input else {
                    return;
                };
                let _ = sender.send(input.input);
            },
            (),
        )
        .map_err(|error| format!("failed to open MIDI input '{name}': {error}"))?;
    eprintln!("MIDI control input '{}' connected: {name}", device.id());
    Ok(Some(MidiControlInputConnection {
        _connection: connection,
    }))
}

fn connect_midi_control_diagnostic(
    device: &meldritch_dsl::MidiDeviceDefinition,
    sender: mpsc::Sender<MidiControlDiagnosticEvent>,
    bindings: Vec<meldritch_app::MidiControlBinding>,
    action_bindings: Vec<MidiActionBinding>,
) -> Result<Option<MidiControlInputConnection>, String> {
    let mut midi_input = midir::MidiInput::new(&format!("meldritch-midi-check-{}", device.id()))
        .map_err(|error| midi_input_client_error("failed to create MIDI input client", &error))?;
    midi_input.ignore(midir::Ignore::None);
    let Some((port, port_name)) = midi_input
        .ports()
        .into_iter()
        .filter_map(|port| {
            let name = midi_input.port_name(&port).ok()?;
            midi_port_name_matches(&name, device.name_contains()).then_some((port, name))
        })
        .next()
    else {
        return Ok(None);
    };
    let device_id = device.id().to_owned();
    let callback_device_id = device_id.clone();
    let callback_port_name = port_name.clone();
    let connection = midi_input
        .connect(
            &port,
            &format!("meldritch-midi-check-{}-input", device.id()),
            move |_timestamp, message, _state| {
                let (diagnostic, input) =
                    if let Some((channel, cc, value)) = decode_midi_cc_message(message) {
                        let input = map_script_midi_control_change(
                            &bindings,
                            &action_bindings,
                            meldritch_app::MidiControlInput {
                                device: callback_device_id.clone(),
                                channel,
                                cc,
                                value,
                            },
                        );
                        (
                            MidiDiagnosticMessage::ControlChange { channel, cc, value },
                            input,
                        )
                    } else if let Some((channel, note, velocity, on)) =
                        decode_midi_note_message(message)
                    {
                        let input = map_script_midi_note(
                            &action_bindings,
                            &callback_device_id,
                            channel,
                            note,
                            on,
                        );
                        (
                            MidiDiagnosticMessage::Note {
                                channel,
                                note,
                                velocity,
                                on,
                            },
                            input,
                        )
                    } else {
                        (MidiDiagnosticMessage::Raw(message.to_vec()), None)
                    };
                let _ = sender.send(MidiControlDiagnosticEvent {
                    device: callback_device_id.clone(),
                    port: callback_port_name.clone(),
                    message: diagnostic,
                    mapped: input,
                });
            },
            (),
        )
        .map_err(|error| format!("failed to open MIDI input '{port_name}': {error}"))?;
    println!("  opened '{}' on port '{port_name}'", device_id);
    Ok(Some(MidiControlInputConnection {
        _connection: connection,
    }))
}

fn midi_input_client_error(context: &str, error: &midir::InitError) -> String {
    format!(
        "{context}: {error}. On Linux, ensure the ALSA sequencer device is available (usually /dev/snd/seq) and the user can access it. On Windows, ensure the controller is visible as a MIDI input device."
    )
}

fn midi_port_name_matches(name: &str, contains: &str) -> bool {
    normalize_midi_name(name).contains(&normalize_midi_name(contains))
}

fn normalize_midi_name(value: &str) -> String {
    value.to_ascii_lowercase().replace([' ', '-', '_'], "")
}

fn midi_control_bindings_for_song(
    song: &meldritch_dsl::ValidatedSong,
) -> Vec<meldritch_app::MidiControlBinding> {
    song.performance()
        .controls()
        .iter()
        .flat_map(|control| {
            control.bindings().iter().filter_map(|binding| {
                let meldritch_dsl::ControlBindingDefinition::MidiCc {
                    device,
                    channel,
                    cc,
                    action,
                } = binding
                else {
                    return None;
                };
                let action = match action {
                    meldritch_dsl::ControlBindingAction::Absolute => {
                        meldritch_app::MidiControlAction::Absolute
                    }
                    meldritch_dsl::ControlBindingAction::Decrement => {
                        meldritch_app::MidiControlAction::Decrement
                    }
                    meldritch_dsl::ControlBindingAction::Increment => {
                        meldritch_app::MidiControlAction::Increment
                    }
                };
                Some(meldritch_app::MidiControlBinding {
                    control_id: control.id().to_owned(),
                    device: device.clone(),
                    channel: *channel,
                    cc: *cc,
                    action,
                })
            })
        })
        .collect()
}

fn midi_control_bindings_for_device(
    bindings: &[meldritch_app::MidiControlBinding],
    device: &str,
) -> Vec<meldritch_app::MidiControlBinding> {
    bindings
        .iter()
        .filter(|binding| binding.device == device)
        .cloned()
        .collect()
}

fn midi_action_bindings_for_song(song: &meldritch_dsl::ValidatedSong) -> Vec<MidiActionBinding> {
    song.performance()
        .actions()
        .iter()
        .flat_map(|action| {
            let Some(input) = app_input_for_performance_action(action.action()) else {
                return Vec::new();
            };
            action
                .bindings()
                .iter()
                .filter_map(move |binding| match binding {
                    meldritch_dsl::ControlBindingDefinition::MidiCc {
                        device,
                        channel,
                        cc,
                        ..
                    } => Some(MidiActionBinding {
                        device: device.clone(),
                        message: MidiActionMessage::ControlChange {
                            channel: *channel,
                            cc: *cc,
                        },
                        input: input.clone(),
                        label: action.label().to_owned(),
                    }),
                    meldritch_dsl::ControlBindingDefinition::MidiNote {
                        device,
                        channel,
                        note,
                    } => Some(MidiActionBinding {
                        device: device.clone(),
                        message: MidiActionMessage::Note {
                            channel: *channel,
                            note: *note,
                        },
                        input: input.clone(),
                        label: action.label().to_owned(),
                    }),
                    meldritch_dsl::ControlBindingDefinition::Key { .. } => None,
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

fn midi_action_bindings_for_device(
    bindings: &[MidiActionBinding],
    device: &str,
) -> Vec<MidiActionBinding> {
    bindings
        .iter()
        .filter(|binding| binding.device == device)
        .cloned()
        .collect()
}

fn app_input_for_performance_action(
    action: &meldritch_dsl::PerformanceActionDefinition,
) -> Option<meldritch_app::AppInput> {
    Some(match action {
        meldritch_dsl::PerformanceActionDefinition::TogglePlayback => {
            meldritch_app::AppInput::TogglePlayback
        }
        meldritch_dsl::PerformanceActionDefinition::Stop => meldritch_app::AppInput::Stop,
        meldritch_dsl::PerformanceActionDefinition::Rewind => meldritch_app::AppInput::Rewind,
        meldritch_dsl::PerformanceActionDefinition::ToggleCockpitMode => {
            meldritch_app::AppInput::ToggleCockpitMode
        }
        meldritch_dsl::PerformanceActionDefinition::QueueNextScene => {
            meldritch_app::AppInput::QueueNextScene
        }
        meldritch_dsl::PerformanceActionDefinition::QueuePhrase { scene } => {
            meldritch_app::AppInput::QueuePhrase(SceneId::new(*scene))
        }
        meldritch_dsl::PerformanceActionDefinition::QueuePhraseVariation { scene, variation } => {
            meldritch_app::AppInput::QueuePhraseVariation(SceneId::new(*scene), *variation)
        }
        meldritch_dsl::PerformanceActionDefinition::ToggleTrackMute => {
            meldritch_app::AppInput::ToggleTrackMute
        }
        meldritch_dsl::PerformanceActionDefinition::TriggerFill => {
            meldritch_app::AppInput::TriggerFill
        }
        meldritch_dsl::PerformanceActionDefinition::CancelPerformance => {
            meldritch_app::AppInput::CancelPerformance
        }
    })
}

fn decode_script_midi_input(
    message: &[u8],
    device: &str,
    control_bindings: &[meldritch_app::MidiControlBinding],
    action_bindings: &[MidiActionBinding],
) -> Option<MappedMidiInput> {
    if let Some((channel, cc, value)) = decode_midi_cc_message(message) {
        return map_script_midi_control_change(
            control_bindings,
            action_bindings,
            meldritch_app::MidiControlInput {
                device: device.to_owned(),
                channel,
                cc,
                value,
            },
        );
    }
    let (channel, note, _velocity, on) = decode_midi_note_message(message)?;
    map_script_midi_note(action_bindings, device, channel, note, on)
}

fn map_script_midi_control_change(
    control_bindings: &[meldritch_app::MidiControlBinding],
    action_bindings: &[MidiActionBinding],
    input: meldritch_app::MidiControlInput,
) -> Option<MappedMidiInput> {
    meldritch_app::map_midi_control_input(control_bindings, input.clone())
        .map(|input| MappedMidiInput { input, label: None })
        .or_else(|| {
            if input.value == 0 {
                return None;
            }
            action_bindings
                .iter()
                .find(|binding| {
                    binding.device == input.device
                        && matches!(
                            binding.message,
                            MidiActionMessage::ControlChange { channel, cc }
                                if cc == input.cc
                                    && channel.is_none_or(|channel| channel == input.channel)
                        )
                })
                .map(|binding| MappedMidiInput {
                    input: binding.input.clone(),
                    label: Some(binding.label.clone()),
                })
        })
}

fn map_script_midi_note(
    action_bindings: &[MidiActionBinding],
    device: &str,
    channel: u8,
    note: u8,
    on: bool,
) -> Option<MappedMidiInput> {
    if !on {
        return None;
    }
    action_bindings
        .iter()
        .find(|binding| {
            binding.device == device
                && matches!(
                    binding.message,
                    MidiActionMessage::Note {
                        channel: binding_channel,
                        note: binding_note,
                    } if binding_note == note
                        && binding_channel.is_none_or(|binding_channel| binding_channel == channel)
                )
        })
        .map(|binding| MappedMidiInput {
            input: binding.input.clone(),
            label: Some(binding.label.clone()),
        })
}

fn decode_midi_cc_message(message: &[u8]) -> Option<(u8, u8, u8)> {
    let [status, cc, value, ..] = *message else {
        return None;
    };
    if status & 0xF0 != 0xB0 {
        return None;
    }
    Some(((status & 0x0F) + 1, cc, value.min(127)))
}

fn decode_midi_note_message(message: &[u8]) -> Option<(u8, u8, u8, bool)> {
    let [status, note, velocity, ..] = *message else {
        return None;
    };
    match status & 0xF0 {
        0x80 => Some(((status & 0x0F) + 1, note.min(127), velocity.min(127), false)),
        0x90 => Some((
            (status & 0x0F) + 1,
            note.min(127),
            velocity.min(127),
            velocity != 0,
        )),
        _ => None,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LiveSongControlTarget {
    DelayFeedback,
    FilterCutoff,
}

fn live_song_control_targets(
    song: &meldritch_dsl::ValidatedSong,
) -> BTreeMap<String, LiveSongControlTarget> {
    song.performance()
        .controls()
        .iter()
        .filter_map(|control| {
            let target = parameter_target_label(control.target());
            let target = match target.as_str() {
                "dsp:echo/delay.feedback" => LiveSongControlTarget::DelayFeedback,
                value if value.ends_with("/filter.cutoff_hz") => {
                    LiveSongControlTarget::FilterCutoff
                }
                _ => return None,
            };
            Some((control.id().to_owned(), target))
        })
        .collect()
}

fn tui_song(
    path: PathBuf,
    frames: Option<u64>,
    chunk_frames: u32,
    workers: usize,
    midi_controls: bool,
) -> Result<(), String> {
    let song = meldritch_dsl::load_song_directory(&path).map_err(|error| error.to_string())?;
    let patch = meldritch_render::song_render::compile_delayed_note_song(&song)
        .map_err(|error| format!("failed to compile song for TUI playback: {error:?}"))?;
    let frame_count = frames.unwrap_or_else(|| {
        song.note_patterns()
            .values()
            .map(|pattern| pattern.length_frames())
            .max()
            .unwrap_or(96_000)
    });
    let frame_count = u32::try_from(frame_count)
        .map_err(|_| "TUI song frame count exceeds u32::MAX".to_owned())?;
    let initial = patch
        .render(FrameRange::new(0, u64::from(frame_count)).expect("song range is ordered"))
        .map_err(|error| format!("failed to render initial song audio: {error:?}"))?;
    let worker_count = if workers == 0 {
        std::thread::available_parallelism().map_or(1, std::num::NonZero::get)
    } else {
        workers
    };
    let (playback, engine) = meldritch_audio::device_output::playback_session_parts(32)?;
    let settings = RenderSettings::new(initial.channels())
        .map_err(|error| format!("invalid render settings: {error:?}"))?;
    let tempo = Tempo::new(song.performance().bpm(), song.performance().sample_rate())
        .map_err(|error| format!("invalid song tempo: {error}"))?;
    let pattern = Pattern::new(PatternId::new(1), 16, 4)
        .map_err(|error| format!("failed to create TUI backing pattern: {error:?}"))?;
    let timeline_chunks = frame_count.div_ceil(chunk_frames).max(1) as usize;
    let config = RenderCoordinatorConfig::new(
        worker_count,
        frame_count,
        chunk_frames,
        timeline_chunks,
        Duration::from_millis(10),
    )
    .map_err(|error| format!("invalid TUI song render configuration: {error:?}"))?;
    let samples_by_note = Arc::new(BTreeMap::new());
    let state = meldritch_render::coordinator::SampleRenderState::new(
        pattern.clone(),
        tempo,
        ProbabilitySeed::new(song.fingerprint()),
        settings,
        samples_by_note,
    );
    let coordinator =
        RenderCoordinator::new_from_state(config, state.clone(), playback.status_monitor())
            .map_err(|error| format!("failed to start song render coordinator: {error:?}"))?;
    let _ = coordinator.wait_for_ready_chunks(timeline_chunks, Duration::from_secs(2));
    coordinator
        .audition_block(&initial)
        .map_err(|error| format!("failed to publish initial song audio: {error:?}"))?;
    let editor = meldritch_render::live_edit::LivePatternEditor::new(state, frame_count);
    let mut controller = meldritch_app::AppController::new(
        playback,
        coordinator,
        editor,
        meldritch_app::Selection {
            track: TrackId::new(1),
            step: StepIndex::new(0),
        },
        256,
    );
    let controls = song_controls_for_view(&song, patch.feedback(), patch.cutoff_hz());
    let live_control_targets = live_song_control_targets(&song);
    controller.set_curated_controls(controls);
    let (external_input_sender, external_input_receiver) = mpsc::channel();
    let midi_control_bindings = midi_control_bindings_for_song(&song);
    let midi_action_bindings = midi_action_bindings_for_song(&song);
    let _midi_inputs = if midi_controls {
        let mut connections = Vec::new();
        for device in song.performance().midi_devices() {
            let bindings = midi_control_bindings_for_device(&midi_control_bindings, device.id());
            let action_bindings =
                midi_action_bindings_for_device(&midi_action_bindings, device.id());
            if bindings.is_empty() && action_bindings.is_empty() {
                continue;
            }
            match connect_midi_control_input(
                device,
                external_input_sender.clone(),
                bindings,
                action_bindings,
            )? {
                Some(connection) => connections.push(connection),
                None => {
                    eprintln!(
                        "MIDI control input '{}' not found; continuing without that device",
                        device.id()
                    );
                }
            }
        }
        connections
    } else {
        Vec::new()
    };
    let _session = meldritch_audio::device_output::RealtimePlaybackSession::open_default(
        controller.coordinator().audio_reader(),
        song.performance().sample_rate(),
        u32::MAX,
        controller.playback_control(),
        engine,
    )?;
    let worker = Arc::new(Mutex::new(SongRerenderWorker::new(patch, frame_count)));
    let input_worker = Arc::clone(&worker);
    let submitted_generation = Arc::new(Mutex::new(0_u64));
    let input_generation = Arc::clone(&submitted_generation);
    let live_overrides = Arc::new(Mutex::new(SongLiveOverrides::default()));
    let input_live_overrides = Arc::clone(&live_overrides);
    let published_generation = Arc::new(Mutex::new(0_u64));
    let tick_published_generation = Arc::clone(&published_generation);
    let tick_submitted_generation = Arc::clone(&submitted_generation);
    let capture = Arc::new(Mutex::new(PerformanceSessionCapture::create(
        &song,
        frame_count,
    )?));
    let input_capture = Arc::clone(&capture);
    let run_result = meldritch_tui::run_with_hooks_and_external_inputs(
        &mut controller,
        Step::new(36),
        move |controller| {
            let completed = worker
                .lock()
                .expect("song render worker lock poisoned")
                .latest_completed();
            if let Some((generation, block, overrides)) = completed {
                let submitted = *tick_submitted_generation
                    .lock()
                    .expect("song render generation lock poisoned");
                if generation < submitted {
                    return Ok(None);
                }
                let mut published = tick_published_generation
                    .lock()
                    .expect("song render generation lock poisoned");
                if generation <= *published {
                    return Ok(None);
                }
                controller
                    .coordinator()
                    .audition_block(&block)
                    .map_err(|error| format!("song publication failed: {error:?}"))?;
                *published = generation;
                return Ok(Some(format!("Song rerender published: {overrides:?}")));
            }
            Ok(None)
        },
        move || external_input_receiver.try_recv().ok(),
        move |controller, input, result| {
            if let Err(error) = input_capture
                .lock()
                .expect("performance session capture lock poisoned")
                .record(controller, input, result, frame_count, tempo)
            {
                eprintln!("session capture failed: {error}");
            }
            let control_id = match input {
                meldritch_app::AppInput::AdjustCuratedControl { id, .. }
                | meldritch_app::AppInput::SetCuratedControlNormalized { id, .. } => id,
                _ => return,
            };
            let Some(target) = live_control_targets.get(control_id) else {
                return;
            };
            let Some(value) = controller
                .view_model()
                .curated_controls
                .into_iter()
                .find(|control| control.id == *control_id)
                .and_then(|control| control.value)
            else {
                return;
            };
            let overrides = {
                let mut overrides = input_live_overrides
                    .lock()
                    .expect("song live override lock poisoned");
                match target {
                    LiveSongControlTarget::DelayFeedback => overrides.feedback = Some(value),
                    LiveSongControlTarget::FilterCutoff => overrides.cutoff_hz = Some(value),
                }
                *overrides
            };
            let mut generation = input_generation
                .lock()
                .expect("song render generation lock poisoned");
            *generation = generation.saturating_add(1);
            input_worker
                .lock()
                .expect("song render worker lock poisoned")
                .submit(*generation, overrides);
        },
    );
    if let Err(error) = run_result {
        return Err(format!("TUI song failed: {error}"));
    }
    let session_path = {
        let mut capture = capture
            .lock()
            .expect("performance session capture lock poisoned");
        capture.finish_clean(&controller)?;
        capture.path().to_owned()
    };
    println!("saved performance session: {}", session_path.display());
    Ok(())
}

fn song_controls_for_view(
    song: &meldritch_dsl::ValidatedSong,
    default_feedback: f64,
    default_cutoff_hz: Option<f64>,
) -> Vec<meldritch_app::CuratedControlView> {
    song.performance()
        .controls()
        .iter()
        .map(|control| {
            let (minimum, maximum) = control.range();
            let target = parameter_target_label(control.target());
            let value = match target.as_str() {
                "dsp:echo/delay.feedback" => Some(default_feedback.clamp(minimum, maximum)),
                value if value.ends_with("/filter.cutoff_hz") => {
                    default_cutoff_hz.map(|cutoff| cutoff.clamp(minimum, maximum))
                }
                _ => None,
            };
            meldritch_app::CuratedControlView {
                id: control.id().to_owned(),
                label: control.label().to_owned(),
                target,
                minimum,
                maximum,
                step: control.step(),
                binding: control.binding().to_owned(),
                value,
            }
        })
        .collect()
}

fn parameter_target_label(target: &ParameterTargetDefinition) -> String {
    let owner = match target.owner() {
        ParameterOwner::Synth => "synth",
        ParameterOwner::Dsp => "dsp",
    };
    format!(
        "{}:{}/{}.{}",
        owner,
        target.definition_id(),
        target.module_id(),
        target.parameter()
    )
}

fn validate_project(path: PathBuf) -> Result<(), String> {
    let input = std::fs::read_to_string(&path)
        .map_err(|err| format!("failed to read {}: {err}", path.display()))?;
    let project = meldritch_dsl::parse_project(&input).map_err(|err| {
        err.diagnostics()
            .into_iter()
            .map(|diagnostic| diagnostic.message().to_owned())
            .collect::<Vec<_>>()
            .join("\n")
    })?;

    println!(
        "valid: {} (patterns: {}, bpm: {}, sample_rate: {})",
        project.name(),
        project.patterns().len(),
        project.tempo().bpm(),
        project.tempo().sample_rate()
    );

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn transform_chunk_command(
    input: PathBuf,
    kind: TransformKindArg,
    start: u32,
    requested_frames: Option<u32>,
    order: String,
    freeze_frame: u32,
    smear_radius: u32,
    output: PathBuf,
    manifest: PathBuf,
    play: bool,
) -> Result<(), String> {
    let source = meldritch_audio::read_wav(&input)
        .map_err(|err| format!("failed to read {}: {err}", input.display()))?;
    if start >= source.frames() {
        return Err(format!(
            "capture start {start} is outside input with {} frames",
            source.frames()
        ));
    }
    let frames = requested_frames
        .unwrap_or_else(|| source.frames() - start)
        .min(source.frames() - start);
    if frames == 0 {
        return Err("transform capture must contain at least one frame".to_owned());
    }
    let channels = usize::from(source.channels());
    let source_start = start as usize * channels;
    let source_end = source_start + frames as usize * channels;
    let mut captured = meldritch_audio::AudioBlock::silent(source.channels(), frames);
    captured
        .samples_mut()
        .copy_from_slice(&source.samples()[source_start..source_end]);
    let transform = match kind {
        TransformKindArg::Reverse => meldritch_render::transforms::ChunkTransform::Reverse,
        TransformKindArg::Reslice => {
            let order = order
                .split(',')
                .map(|value| {
                    value
                        .trim()
                        .parse::<usize>()
                        .map_err(|err| format!("invalid reslice index '{}': {err}", value.trim()))
                })
                .collect::<Result<Vec<_>, _>>()?;
            meldritch_render::transforms::ChunkTransform::Reslice { order }
        }
        TransformKindArg::Freeze => meldritch_render::transforms::ChunkTransform::Freeze {
            frame: freeze_frame,
        },
        TransformKindArg::Smear => meldritch_render::transforms::ChunkTransform::Smear {
            radius_frames: smear_radius,
        },
    };
    let mut cache = meldritch_render::transforms::TransformArtifactCache::default();
    let transformed = cache
        .render(&captured, &transform)
        .map_err(|err| format!("invalid transform: {err:?}"))?;
    for target in [&output, &manifest] {
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|err| format!("failed to create {}: {err}", parent.display()))?;
        }
    }
    meldritch_audio::write_wav_f32(&output, &transformed.block, source.sample_rate())
        .map_err(|err| format!("failed to write {}: {err}", output.display()))?;
    let manifest_value = serde_json::json!({
        "schema_version": 1,
        "source": input,
        "capture": { "start": start, "frames": frames },
        "transform": format!("{transform:?}"),
        "artifact_fingerprint": transformed.key.fingerprint.raw(),
        "channels": transformed.block.channels(),
        "sample_rate": source.sample_rate(),
        "peak": transformed.block.peak_abs(),
        "finite": transformed.block.samples().iter().all(|sample| sample.is_finite()),
    });
    std::fs::write(
        &manifest,
        serde_json::to_string_pretty(&manifest_value)
            .map_err(|err| format!("failed to encode transform manifest: {err}"))?,
    )
    .map_err(|err| format!("failed to write {}: {err}", manifest.display()))?;
    println!(
        "transformed chunk: kind={transform:?}, frames={frames}, peak={:.3}, output={}, manifest={}",
        transformed.block.peak_abs(),
        output.display(),
        manifest.display()
    );
    if play {
        let report = meldritch_audio::device_output::play_blocking(
            &transformed.block,
            source.sample_rate(),
            1,
        )?;
        println!(
            "played transformed chunk: device={}, callbacks={}, underruns={}, misses={}",
            report.device_name, report.callbacks, report.underruns, report.missed_artifacts
        );
    }
    Ok(())
}

#[derive(Debug, Serialize)]
struct ProjectSummary {
    schema_version: u32,
    name: String,
    tempo: TempoSummary,
    samples: Vec<SampleSummary>,
    patterns: Vec<PatternSummary>,
    relations: Vec<ProjectRelationSummary>,
}

impl ProjectSummary {
    fn from_project(project: &meldritch_dsl::ValidatedProject) -> Self {
        Self {
            schema_version: 1,
            name: project.name().to_owned(),
            tempo: TempoSummary {
                bpm: project.tempo().bpm(),
                sample_rate: project.tempo().sample_rate(),
                probability_seed: project.probability_seed().raw(),
            },
            samples: project
                .samples()
                .iter()
                .map(|sample| SampleSummary {
                    note: sample.note(),
                    path: sample.path().to_owned(),
                })
                .collect(),
            patterns: project
                .patterns()
                .iter()
                .map(|pattern| PatternSummary {
                    id: pattern.id().raw(),
                    length_steps: pattern.length_steps(),
                    steps_per_beat: pattern.steps_per_beat(),
                    active_steps: pattern.active_step_count(),
                    tracks: pattern
                        .active_step_counts_by_track()
                        .into_iter()
                        .map(|(track, active_steps)| TrackSummary {
                            id: track.raw(),
                            active_steps,
                        })
                        .collect(),
                })
                .collect(),
            relations: project
                .relations()
                .iter()
                .map(ProjectRelationSummary::from_relation)
                .collect(),
        }
    }
}

#[derive(Debug, Serialize)]
struct TempoSummary {
    bpm: f64,
    sample_rate: u32,
    probability_seed: u64,
}

#[derive(Debug, Serialize)]
struct SampleSummary {
    note: u8,
    path: String,
}

#[derive(Debug, Serialize)]
struct PatternSummary {
    id: u64,
    length_steps: u32,
    steps_per_beat: u32,
    active_steps: usize,
    tracks: Vec<TrackSummary>,
}

#[derive(Debug, Serialize)]
struct TrackSummary {
    id: u64,
    active_steps: usize,
}

#[derive(Debug, Serialize)]
struct ProjectRelationSummary {
    from: ProjectRelationEndpointSummary,
    to: ProjectRelationEndpointSummary,
    kind: &'static str,
}

impl ProjectRelationSummary {
    fn from_relation(relation: &meldritch_dsl::RelationRef) -> Self {
        Self {
            from: ProjectRelationEndpointSummary::from_endpoint(relation.from()),
            to: ProjectRelationEndpointSummary::from_endpoint(relation.to()),
            kind: match relation.kind() {
                meldritch_dsl::RelationKind::Audio => "audio",
                meldritch_dsl::RelationKind::Control => "control",
                meldritch_dsl::RelationKind::Sidechain => "sidechain",
            },
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(tag = "type")]
enum ProjectRelationEndpointSummary {
    SampleNote { note: u8 },
    Pattern { pattern_id: u64 },
}

impl ProjectRelationEndpointSummary {
    const fn from_endpoint(endpoint: meldritch_dsl::RelationEndpoint) -> Self {
        match endpoint {
            meldritch_dsl::RelationEndpoint::SampleNote(note) => Self::SampleNote { note },
            meldritch_dsl::RelationEndpoint::Pattern(pattern) => Self::Pattern {
                pattern_id: pattern.raw(),
            },
        }
    }
}

#[derive(Debug, Serialize)]
struct RelationDiagnostics {
    schema_version: u32,
    declared_count: usize,
    compiled_count: usize,
    declared: Vec<ProjectRelationSummary>,
    compiled: Vec<CompiledRelationSummary>,
}

impl RelationDiagnostics {
    fn from_project_and_compiled(
        project: &meldritch_dsl::ValidatedProject,
        compiled: &meldritch_dsl::CompiledProject,
    ) -> Self {
        Self {
            schema_version: 1,
            declared_count: project.relations().len(),
            compiled_count: compiled.relation_bindings().len(),
            declared: project
                .relations()
                .iter()
                .map(ProjectRelationSummary::from_relation)
                .collect(),
            compiled: compiled
                .relation_bindings()
                .iter()
                .map(CompiledRelationSummary::from_binding)
                .collect(),
        }
    }
}

#[derive(Debug, Serialize)]
struct CompiledGraphSummary {
    schema_version: u32,
    source_count: usize,
    node_count: usize,
    edge_count: usize,
    sources: Vec<CompiledSourceSummary>,
    relations: Vec<CompiledRelationSummary>,
}

impl CompiledGraphSummary {
    fn from_compiled(compiled: &meldritch_dsl::CompiledProject) -> Self {
        Self {
            schema_version: 1,
            source_count: compiled.sources().len(),
            node_count: compiled.relations().len_nodes(),
            edge_count: compiled.relations().len_edges(),
            sources: compiled
                .source_bindings()
                .iter()
                .map(CompiledSourceSummary::from_binding)
                .collect(),
            relations: compiled
                .relation_bindings()
                .iter()
                .map(CompiledRelationSummary::from_binding)
                .collect(),
        }
    }
}

#[derive(Debug, Serialize)]
struct CompiledSourceSummary {
    source_id: u64,
    node_id: u64,
    kind: CompiledSourceKindSummary,
}

impl CompiledSourceSummary {
    fn from_binding(binding: &meldritch_dsl::SourceBinding) -> Self {
        Self {
            source_id: binding.source().raw(),
            node_id: binding.node().raw(),
            kind: CompiledSourceKindSummary::from_kind(binding.kind()),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(tag = "type")]
enum CompiledSourceKindSummary {
    Sample { note: u8, path: String },
    Pattern { pattern_id: u64 },
}

impl CompiledSourceKindSummary {
    fn from_kind(kind: &meldritch_dsl::SourceBindingKind) -> Self {
        match kind {
            meldritch_dsl::SourceBindingKind::Sample { note, path } => Self::Sample {
                note: *note,
                path: path.clone(),
            },
            meldritch_dsl::SourceBindingKind::Pattern { pattern } => Self::Pattern {
                pattern_id: pattern.raw(),
            },
        }
    }
}

#[derive(Debug, Serialize)]
struct CompiledRelationSummary {
    relation_id: u64,
    from_node_id: u64,
    to_node_id: u64,
    kind: CompiledRelationKindSummary,
}

impl CompiledRelationSummary {
    fn from_binding(binding: &meldritch_dsl::RelationBinding) -> Self {
        Self {
            relation_id: binding.relation().raw(),
            from_node_id: binding.from().raw(),
            to_node_id: binding.to().raw(),
            kind: CompiledRelationKindSummary::from_kind(binding.kind()),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(tag = "type")]
#[allow(clippy::enum_variant_names)]
enum CompiledRelationKindSummary {
    SampleToPattern {
        note: u8,
        pattern_id: u64,
    },
    PatternControlsPattern {
        from_pattern_id: u64,
        to_pattern_id: u64,
    },
    PatternSidechainsPattern {
        from_pattern_id: u64,
        to_pattern_id: u64,
    },
}

impl CompiledRelationKindSummary {
    fn from_kind(kind: &meldritch_dsl::RelationBindingKind) -> Self {
        match kind {
            meldritch_dsl::RelationBindingKind::SampleToPattern { note, pattern } => {
                Self::SampleToPattern {
                    note: *note,
                    pattern_id: pattern.raw(),
                }
            }
            meldritch_dsl::RelationBindingKind::PatternControlsPattern {
                from_pattern,
                to_pattern,
            } => Self::PatternControlsPattern {
                from_pattern_id: from_pattern.raw(),
                to_pattern_id: to_pattern.raw(),
            },
            meldritch_dsl::RelationBindingKind::PatternSidechainsPattern {
                from_pattern,
                to_pattern,
            } => Self::PatternSidechainsPattern {
                from_pattern_id: from_pattern.raw(),
                to_pattern_id: to_pattern.raw(),
            },
        }
    }
}

#[derive(Debug, Serialize)]
struct SampleDiagnostics {
    schema_version: u32,
    samples: Vec<SampleDiagnostic>,
}

#[derive(Debug, Serialize)]
struct SampleDiagnostic {
    note: u8,
    path: String,
    resolved_path: String,
    sample_rate: u32,
    channels: u16,
    frames: meldritch_audio::Frames,
    sample_count: usize,
    peak: f64,
    finite: bool,
}

impl SampleDiagnostic {
    fn from_sample(
        note: u8,
        path: &str,
        resolved_path: &Path,
        sample: &meldritch_audio::SampleBuffer,
    ) -> Self {
        Self {
            note,
            path: path.to_owned(),
            resolved_path: resolved_path.display().to_string(),
            sample_rate: sample.sample_rate(),
            channels: sample.channels(),
            frames: sample.frames(),
            sample_count: sample.samples().len(),
            peak: sample
                .samples()
                .iter()
                .fold(0.0, |peak, sample| peak.max(sample.abs())),
            finite: sample.samples().iter().all(|sample| sample.is_finite()),
        }
    }
}

#[derive(Debug, Serialize)]
struct RenderManifest {
    schema_version: u32,
    project_path: String,
    output_path: Option<String>,
    pattern_id: u64,
    range: FrameRangeSummary,
    channels: u16,
    normalize: bool,
    cache_probe: bool,
    cache_probe_result: Option<CacheProbeSummary>,
    artifact: ArtifactSummary,
    graph: RenderGraphSummary,
    samples: Vec<RenderSampleSummary>,
    control: Option<RenderControlSummary>,
    result: RenderResultSummary,
}

struct RenderManifestInput<'a> {
    project_path: &'a Path,
    output_path: Option<&'a Path>,
    pattern: &'a Pattern,
    range: FrameRange,
    channels: u16,
    normalize: bool,
    cache_probe: bool,
    cache_probe_summary: Option<CacheProbeSummary>,
    artifact_key: meldritch_render::ArtifactKey,
    compiled: &'a meldritch_dsl::CompiledProject,
    samples_by_note: &'a BTreeMap<u8, meldritch_audio::SampleBuffer>,
    control: Option<RenderControlSummary>,
    peak: f64,
    nonzero_samples: usize,
    finite: bool,
}

impl RenderManifest {
    fn from_render(input: RenderManifestInput<'_>) -> Result<Self, String> {
        let graph = RenderGraphSummary::from_compiled(input.compiled, input.pattern.id())?;
        Ok(Self {
            schema_version: 1,
            project_path: input.project_path.display().to_string(),
            output_path: input.output_path.map(|path| path.display().to_string()),
            pattern_id: input.pattern.id().raw(),
            range: FrameRangeSummary::from_range(input.range),
            channels: input.channels,
            normalize: input.normalize,
            cache_probe: input.cache_probe,
            cache_probe_result: input.cache_probe_summary,
            artifact: ArtifactSummary::from_key(input.artifact_key),
            graph,
            samples: input
                .samples_by_note
                .iter()
                .map(|(note, sample)| RenderSampleSummary::from_sample(*note, sample))
                .collect(),
            control: input.control,
            result: RenderResultSummary {
                peak: input.peak,
                nonzero_samples: input.nonzero_samples,
                finite: input.finite,
            },
        })
    }
}

#[derive(Debug, Serialize)]
struct RenderManifestDiagnostics {
    schema_version: u32,
    manifest_schema_version: u64,
    pattern_id: u64,
    sample_source_count: usize,
    relation_count: usize,
    relation_kinds: BTreeMap<String, usize>,
    control: Option<RenderControlSummary>,
    result: RenderResultSummary,
}

impl RenderManifestDiagnostics {
    fn from_manifest(manifest: &serde_json::Value) -> Result<Self, String> {
        let manifest_schema_version = required_u64(manifest, &["schema_version"])?;
        let pattern_id = required_u64(manifest, &["pattern_id"])?;
        let sample_source_count = required_array(manifest, &["graph", "sample_sources"])?.len();
        let relations = required_array(manifest, &["graph", "relations"])?;
        let mut relation_kinds = BTreeMap::new();
        for relation in relations {
            let kind = required_str(relation, &["kind", "type"])?;
            *relation_kinds.entry(kind.to_owned()).or_insert(0) += 1;
        }

        Ok(Self {
            schema_version: 1,
            manifest_schema_version,
            pattern_id,
            sample_source_count,
            relation_count: relations.len(),
            relation_kinds,
            control: RenderControlSummary::from_manifest(manifest)?,
            result: RenderResultSummary {
                peak: required_f64(manifest, &["result", "peak"])?,
                nonzero_samples: required_u64(manifest, &["result", "nonzero_samples"])?
                    .try_into()
                    .map_err(|_| "manifest result nonzero_samples is too large".to_owned())?,
                finite: required_bool(manifest, &["result", "finite"])?,
            },
        })
    }
}

#[derive(Clone, Debug, Serialize)]
struct RenderControlSummary {
    active_scale: f64,
    active_event_count: usize,
    max_active_controller_count: usize,
    controller_patterns: Vec<u64>,
}

impl RenderControlSummary {
    fn from_schedule(active_scale: f64, schedule: &ControlEventSchedule) -> Self {
        Self {
            active_scale,
            active_event_count: schedule.active_event_count,
            max_active_controller_count: schedule.max_active_controller_count,
            controller_patterns: schedule.controller_patterns.clone(),
        }
    }

    fn from_manifest(manifest: &serde_json::Value) -> Result<Option<Self>, String> {
        let Some(control) = manifest.get("control") else {
            return Ok(None);
        };
        if control.is_null() {
            return Ok(None);
        }

        Ok(Some(Self {
            active_scale: required_f64(control, &["active_scale"])?,
            active_event_count: required_u64(control, &["active_event_count"])?
                .try_into()
                .map_err(|_| "manifest control active_event_count is too large".to_owned())?,
            max_active_controller_count: required_u64(control, &["max_active_controller_count"])?
                .try_into()
                .map_err(|_| {
                    "manifest control max_active_controller_count is too large".to_owned()
                })?,
            controller_patterns: required_array(control, &["controller_patterns"])?
                .iter()
                .map(|value| {
                    value.as_u64().ok_or_else(|| {
                        "manifest control controller_patterns must be unsigned integers".to_owned()
                    })
                })
                .collect::<Result<Vec<_>, _>>()?,
        }))
    }
}

#[derive(Debug, Serialize)]
struct RenderGraphSummary {
    pattern_source_id: u64,
    pattern_node_id: u64,
    sample_sources: Vec<RenderGraphSampleSourceSummary>,
    relations: Vec<RenderGraphRelationSummary>,
}

impl RenderGraphSummary {
    fn from_compiled(
        compiled: &meldritch_dsl::CompiledProject,
        pattern: meldritch_core::PatternId,
    ) -> Result<Self, String> {
        let pattern_binding = compiled
            .source_bindings()
            .iter()
            .find(|binding| {
                matches!(
                    binding.kind(),
                    meldritch_dsl::SourceBindingKind::Pattern {
                        pattern: binding_pattern
                    } if *binding_pattern == pattern
                )
            })
            .ok_or_else(|| format!("compiled graph has no pattern {}", pattern.raw()))?;

        let mut audio_notes = BTreeSet::new();
        let relations = compiled
            .relation_bindings()
            .iter()
            .filter_map(|binding| match binding.kind() {
                meldritch_dsl::RelationBindingKind::SampleToPattern {
                    note,
                    pattern: relation_pattern,
                } if *relation_pattern == pattern => {
                    audio_notes.insert(*note);
                    Some(RenderGraphRelationSummary::from_binding(binding))
                }
                meldritch_dsl::RelationBindingKind::PatternControlsPattern {
                    from_pattern,
                    to_pattern,
                } if *from_pattern == pattern || *to_pattern == pattern => {
                    Some(RenderGraphRelationSummary::from_binding(binding))
                }
                _ => None,
            })
            .collect::<Vec<_>>();

        Ok(Self {
            pattern_source_id: pattern_binding.source().raw(),
            pattern_node_id: pattern_binding.node().raw(),
            sample_sources: compiled
                .source_bindings()
                .iter()
                .filter_map(|binding| match binding.kind() {
                    meldritch_dsl::SourceBindingKind::Sample { note, .. }
                        if audio_notes.contains(note) =>
                    {
                        Some(RenderGraphSampleSourceSummary {
                            note: *note,
                            source_id: binding.source().raw(),
                            node_id: binding.node().raw(),
                        })
                    }
                    meldritch_dsl::SourceBindingKind::Pattern { .. } => None,
                    meldritch_dsl::SourceBindingKind::Sample { .. } => None,
                })
                .collect(),
            relations,
        })
    }
}

#[derive(Debug, Serialize)]
struct RenderGraphSampleSourceSummary {
    note: u8,
    source_id: u64,
    node_id: u64,
}

#[derive(Debug, Serialize)]
struct RenderGraphRelationSummary {
    relation_id: u64,
    from_node_id: u64,
    to_node_id: u64,
    kind: RenderGraphRelationKindSummary,
}

impl RenderGraphRelationSummary {
    fn from_binding(binding: &meldritch_dsl::RelationBinding) -> Self {
        Self {
            relation_id: binding.relation().raw(),
            from_node_id: binding.from().raw(),
            to_node_id: binding.to().raw(),
            kind: RenderGraphRelationKindSummary::from_kind(binding.kind()),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(tag = "type")]
#[allow(clippy::enum_variant_names)]
enum RenderGraphRelationKindSummary {
    SampleToPattern {
        note: u8,
        pattern_id: u64,
    },
    PatternControlsPattern {
        from_pattern_id: u64,
        to_pattern_id: u64,
    },
    PatternSidechainsPattern {
        from_pattern_id: u64,
        to_pattern_id: u64,
    },
}

impl RenderGraphRelationKindSummary {
    fn from_kind(kind: &meldritch_dsl::RelationBindingKind) -> Self {
        match kind {
            meldritch_dsl::RelationBindingKind::SampleToPattern { note, pattern } => {
                Self::SampleToPattern {
                    note: *note,
                    pattern_id: pattern.raw(),
                }
            }
            meldritch_dsl::RelationBindingKind::PatternControlsPattern {
                from_pattern,
                to_pattern,
            } => Self::PatternControlsPattern {
                from_pattern_id: from_pattern.raw(),
                to_pattern_id: to_pattern.raw(),
            },
            meldritch_dsl::RelationBindingKind::PatternSidechainsPattern {
                from_pattern,
                to_pattern,
            } => Self::PatternSidechainsPattern {
                from_pattern_id: from_pattern.raw(),
                to_pattern_id: to_pattern.raw(),
            },
        }
    }
}

fn required_value<'a>(
    value: &'a serde_json::Value,
    path: &[&str],
) -> Result<&'a serde_json::Value, String> {
    let mut current = value;
    for segment in path {
        current = current
            .get(*segment)
            .ok_or_else(|| format!("manifest is missing field {}", path.to_vec().join(".")))?;
    }
    Ok(current)
}

fn required_u64(value: &serde_json::Value, path: &[&str]) -> Result<u64, String> {
    required_value(value, path)?.as_u64().ok_or_else(|| {
        format!(
            "manifest field {} must be an unsigned integer",
            path.join(".")
        )
    })
}

fn required_f64(value: &serde_json::Value, path: &[&str]) -> Result<f64, String> {
    required_value(value, path)?
        .as_f64()
        .ok_or_else(|| format!("manifest field {} must be a number", path.join(".")))
}

fn required_bool(value: &serde_json::Value, path: &[&str]) -> Result<bool, String> {
    required_value(value, path)?
        .as_bool()
        .ok_or_else(|| format!("manifest field {} must be a boolean", path.join(".")))
}

fn required_str<'a>(value: &'a serde_json::Value, path: &[&str]) -> Result<&'a str, String> {
    required_value(value, path)?
        .as_str()
        .ok_or_else(|| format!("manifest field {} must be a string", path.join(".")))
}

fn required_array<'a>(
    value: &'a serde_json::Value,
    path: &[&str],
) -> Result<&'a Vec<serde_json::Value>, String> {
    required_value(value, path)?
        .as_array()
        .ok_or_else(|| format!("manifest field {} must be an array", path.join(".")))
}

#[derive(Debug, Serialize)]
struct CacheProbeSummary {
    first: String,
    second: String,
    artifacts: usize,
}

#[derive(Debug, Serialize)]
struct ArtifactSummary {
    pattern_id: u64,
    range: FrameRangeSummary,
    sample_rate: u32,
    fingerprint: u64,
}

impl ArtifactSummary {
    fn from_key(key: meldritch_render::ArtifactKey) -> Self {
        Self {
            pattern_id: key.pattern().raw(),
            range: FrameRangeSummary::from_range(key.range()),
            sample_rate: key.sample_rate(),
            fingerprint: key.fingerprint().raw(),
        }
    }
}

#[derive(Debug, Serialize)]
struct RenderSampleSummary {
    note: u8,
    sample_rate: u32,
    channels: u16,
    frames: meldritch_audio::Frames,
    peak: f64,
    finite: bool,
}

impl RenderSampleSummary {
    fn from_sample(note: u8, sample: &meldritch_audio::SampleBuffer) -> Self {
        Self {
            note,
            sample_rate: sample.sample_rate(),
            channels: sample.channels(),
            frames: sample.frames(),
            peak: sample
                .samples()
                .iter()
                .fold(0.0, |peak, sample| peak.max(sample.abs())),
            finite: sample.samples().iter().all(|sample| sample.is_finite()),
        }
    }
}

#[derive(Debug, Serialize)]
struct RenderResultSummary {
    peak: f64,
    nonzero_samples: usize,
    finite: bool,
}

#[derive(Debug, Serialize)]
struct DirtyGraphSummary {
    schema_version: u32,
    source_id: u64,
    node_id: u64,
    range: FrameRangeSummary,
    dirty: Vec<DirtySummary>,
}

impl DirtyGraphSummary {
    fn from_dirty(source_id: u64, node_id: u64, range: FrameRange, dirty: &[DirtyRange]) -> Self {
        Self {
            schema_version: 1,
            source_id,
            node_id,
            range: FrameRangeSummary::from_range(range),
            dirty: dirty.iter().map(DirtySummary::from_dirty).collect(),
        }
    }
}

#[derive(Debug, Serialize)]
struct DirtySummary {
    entity: EntitySummary,
    range: FrameRangeSummary,
}

impl DirtySummary {
    fn from_dirty(dirty: &DirtyRange) -> Self {
        Self {
            entity: EntitySummary::from_entity(dirty.entity()),
            range: FrameRangeSummary::from_range(dirty.range()),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(tag = "type")]
enum EntitySummary {
    Node { id: u64 },
    Pattern { id: u64 },
    Port { id: u64 },
    Relation { id: u64 },
    Source { id: u64 },
    Track { id: u64 },
}

impl EntitySummary {
    const fn from_entity(entity: EntityId) -> Self {
        match entity {
            EntityId::Node(id) => Self::Node { id: id.raw() },
            EntityId::Pattern(id) => Self::Pattern { id: id.raw() },
            EntityId::Port(id) => Self::Port { id: id.raw() },
            EntityId::Relation(id) => Self::Relation { id: id.raw() },
            EntityId::Source(id) => Self::Source { id: id.raw() },
            EntityId::Track(id) => Self::Track { id: id.raw() },
        }
    }
}

#[derive(Debug, Serialize)]
struct EventSchedule {
    schema_version: u32,
    pattern_id: u64,
    range: FrameRangeSummary,
    events: Vec<EventSummary>,
}

impl EventSchedule {
    fn from_events(pattern_id: u64, range: FrameRange, events: &[Event]) -> Self {
        Self {
            schema_version: 1,
            pattern_id,
            range: FrameRangeSummary::from_range(range),
            events: events.iter().map(EventSummary::from_event).collect(),
        }
    }
}

#[derive(Debug, Serialize)]
struct EventSummary {
    pattern_id: u64,
    track_id: u64,
    step: u32,
    range: FrameRangeSummary,
    note: u8,
    velocity: f64,
    tags: Vec<&'static str>,
}

impl EventSummary {
    fn from_event(event: &Event) -> Self {
        Self {
            pattern_id: event.pattern().raw(),
            track_id: event.track().raw(),
            step: event.step().raw(),
            range: FrameRangeSummary::from_range(event.range()),
            note: event.note(),
            velocity: event.velocity(),
            tags: event.tags().iter().map(event_tag_name).collect(),
        }
    }
}

struct ControllerEvents {
    pattern: meldritch_core::PatternId,
    events: Vec<Event>,
}

#[derive(Debug, Serialize)]
struct ControlEventSchedule {
    schema_version: u32,
    pattern_id: u64,
    range: FrameRangeSummary,
    controller_patterns: Vec<u64>,
    active_event_count: usize,
    max_active_controller_count: usize,
    events: Vec<ControlledEventSummary>,
}

impl ControlEventSchedule {
    fn from_events(
        pattern_id: u64,
        range: FrameRange,
        events: &[Event],
        controllers: &[ControllerEvents],
    ) -> Self {
        let events = events
            .iter()
            .map(|event| ControlledEventSummary::from_event(event, controllers))
            .collect::<Vec<_>>();
        let active_event_count = events
            .iter()
            .filter(|event| event.active_controller_count > 0)
            .count();
        let max_active_controller_count = events
            .iter()
            .map(|event| event.active_controller_count)
            .max()
            .unwrap_or(0);

        Self {
            schema_version: 1,
            pattern_id,
            range: FrameRangeSummary::from_range(range),
            controller_patterns: controllers
                .iter()
                .map(|controller| controller.pattern.raw())
                .collect(),
            active_event_count,
            max_active_controller_count,
            events,
        }
    }
}

#[derive(Debug, Serialize)]
struct ControlledEventSummary {
    event: EventSummary,
    control: Vec<EventControlSummary>,
    active_controller_count: usize,
}

impl ControlledEventSummary {
    fn from_event(event: &Event, controllers: &[ControllerEvents]) -> Self {
        let control = controllers
            .iter()
            .map(|controller| EventControlSummary::from_controller(event, controller))
            .collect::<Vec<_>>();
        let active_controller_count = control
            .iter()
            .filter(|control| control.active_event_count > 0)
            .count();

        Self {
            event: EventSummary::from_event(event),
            control,
            active_controller_count,
        }
    }
}

#[derive(Debug, Serialize)]
struct EventControlSummary {
    pattern_id: u64,
    active_event_count: usize,
}

impl EventControlSummary {
    fn from_controller(event: &Event, controller: &ControllerEvents) -> Self {
        let event_start = event.range().start();
        let active_event_count = controller
            .events
            .iter()
            .filter(|controller_event| controller_event.range().contains_frame(event_start))
            .count();

        Self {
            pattern_id: controller.pattern.raw(),
            active_event_count,
        }
    }
}

#[derive(Debug, Serialize)]
struct FrameRangeSummary {
    start: u64,
    end: u64,
}

impl FrameRangeSummary {
    const fn from_range(range: FrameRange) -> Self {
        Self {
            start: range.start(),
            end: range.end(),
        }
    }
}

fn event_tag_name(tag: &EventTag) -> &'static str {
    match tag {
        EventTag::Accent => "accent",
        EventTag::Ghost => "ghost",
        EventTag::Fill => "fill",
        EventTag::Ratchet => "ratchet",
        EventTag::Probabilistic => "probabilistic",
        EventTag::Humanized => "humanized",
        EventTag::SceneTransition => "scene_transition",
    }
}

fn render_clicks(
    path: PathBuf,
    pattern_id: Option<u64>,
    frames: u64,
    channels: u16,
    output: Option<PathBuf>,
    normalize: bool,
    cache_probe: bool,
) -> Result<(), String> {
    if let Some(output) = &output
        && output.exists()
    {
        return Err(format!(
            "output {} already exists; choose a new path",
            output.display()
        ));
    }

    let input = std::fs::read_to_string(&path)
        .map_err(|err| format!("failed to read {}: {err}", path.display()))?;
    let project = meldritch_dsl::parse_project(&input).map_err(|err| {
        err.diagnostics()
            .into_iter()
            .map(|diagnostic| diagnostic.message().to_owned())
            .collect::<Vec<_>>()
            .join("\n")
    })?;
    let pattern = select_pattern(&project, pattern_id, "render")?;
    let settings =
        RenderSettings::new(channels).map_err(|err| format!("invalid render settings: {err:?}"))?;
    let range = FrameRange::new(0, frames).map_err(|err| err.to_string())?;
    let mut block = if cache_probe {
        let mut cache = ArtifactCache::new();
        let first = meldritch_render::render_pattern_clicks_cached(
            &mut cache,
            pattern,
            project.tempo(),
            range,
            project.probability_seed(),
            settings,
        );
        let second = meldritch_render::render_pattern_clicks_cached(
            &mut cache,
            pattern,
            project.tempo(),
            range,
            project.probability_seed(),
            settings,
        );
        println!(
            "cache probe: first={}, second={}, artifacts={}",
            format_cache_status(first.status()),
            format_cache_status(second.status()),
            cache.len()
        );
        second.into_block()
    } else {
        meldritch_render::render_pattern_clicks(
            pattern,
            project.tempo(),
            range,
            project.probability_seed(),
            settings,
        )
    };
    if normalize {
        block = block.normalized_to_peak(1.0);
    }

    let peak = block.peak_abs();
    let nonzero_samples = block
        .samples()
        .iter()
        .filter(|sample| **sample != 0.0)
        .count();
    let finite = block.samples().iter().all(|sample| sample.is_finite());

    println!(
        "rendered: frames={}, channels={}, finite={}, nonzero_samples={}, peak={}",
        block.frames(),
        block.channels(),
        finite,
        nonzero_samples,
        peak
    );

    if let Some(output) = &output {
        meldritch_audio::write_wav_f32(output, &block, project.tempo().sample_rate())
            .map_err(|err| format!("failed to write {}: {err}", output.display()))?;
        println!("wrote: {}", output.display());
    }

    Ok(())
}

struct RenderSamplesOptions {
    pattern_id: Option<u64>,
    frames: u64,
    channels: u16,
    output: Option<PathBuf>,
    manifest: Option<PathBuf>,
    normalize: bool,
    cache_probe: bool,
}

struct RenderControlledSamplesOptions {
    pattern_id: u64,
    frames: u64,
    channels: u16,
    output: Option<PathBuf>,
    manifest: Option<PathBuf>,
    active_scale: f64,
    normalize: bool,
}

fn render_bassline(
    path: PathBuf,
    pattern_id: Option<u64>,
    frames: Option<u64>,
    channels: u16,
    output: PathBuf,
    normalize: bool,
) -> Result<(), String> {
    let input = std::fs::read_to_string(&path)
        .map_err(|err| format!("failed to read {}: {err}", path.display()))?;
    let project = meldritch_dsl::parse_project(&input).map_err(|err| {
        err.diagnostics()
            .into_iter()
            .map(|diagnostic| diagnostic.message().to_owned())
            .collect::<Vec<_>>()
            .join("\n")
    })?;
    let drums = select_pattern(&project, pattern_id, "bassline render")?;
    let frame_count = frames.unwrap_or_else(|| {
        project
            .tempo()
            .step_start_frame(u64::from(drums.length_steps()), drums.steps_per_beat())
    });
    let range = FrameRange::new(0, frame_count).map_err(|err| err.to_string())?;
    let settings =
        RenderSettings::new(channels).map_err(|err| format!("invalid render settings: {err:?}"))?;
    let samples = load_project_samples(&project, &path)?;
    let mut mix = meldritch_render::render_pattern_samples(
        drums,
        project.tempo(),
        range,
        project.probability_seed(),
        settings,
        &samples,
    );

    let mut bass = Pattern::new(
        meldritch_core::PatternId::new(9_001),
        drums.length_steps(),
        drums.steps_per_beat(),
    )
    .map_err(|err| format!("failed to create bass pattern: {err:?}"))?;
    let phrase = [
        (0, 36, 0.85),
        (3, 36, 0.65),
        (6, 39, 0.75),
        (10, 43, 0.8),
        (14, 34, 0.7),
    ];
    for (step, note, velocity) in phrase {
        if step >= drums.length_steps() {
            continue;
        }
        let mut value = Step::new(note).with_velocity(velocity).with_gate(0.8);
        if matches!(step, 0 | 10) {
            value = value.with_tag(EventTag::Accent);
        }
        bass.set_step(TrackId::new(1), StepIndex::new(step), value)
            .map_err(|err| format!("failed to program bass step {step}: {err:?}"))?;
    }
    let bass_settings = meldritch_render::dsp::BassVoiceSettings::default();
    let mut bass_audio = meldritch_render::dsp::render_monophonic_pattern_bass_with_filter_control(
        &bass,
        drums,
        TrackId::new(3),
        project.tempo(),
        range,
        project.probability_seed(),
        settings,
        bass_settings,
    );
    meldritch_render::dsp::apply_pattern_ducking(
        &mut bass_audio,
        drums,
        TrackId::new(1),
        project.tempo(),
        range,
        project.probability_seed(),
        bass_settings.ducking_amount,
        bass_settings.ducking_release_seconds,
    );
    for (output_sample, bass_sample) in mix.samples_mut().iter_mut().zip(bass_audio.samples()) {
        *output_sample += bass_sample;
    }
    if normalize {
        mix = mix.normalized_to_peak(0.9);
    }
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|err| format!("failed to create {}: {err}", parent.display()))?;
    }
    meldritch_audio::write_wav_f32(&output, &mix, project.tempo().sample_rate())
        .map_err(|err| format!("failed to write {}: {err}", output.display()))?;
    println!(
        "rendered bassline mix: pattern={}, frames={}, channels={}, peak={:.3}, output={}",
        drums.id().raw(),
        mix.frames(),
        mix.channels(),
        mix.peak_abs(),
        output.display()
    );
    Ok(())
}

fn render_poly_demo(
    path: PathBuf,
    output: PathBuf,
    frame_count: u64,
    normalize: bool,
) -> Result<(), String> {
    if frame_count < 4 {
        return Err("poly demo requires at least four frames".to_owned());
    }
    let input = std::fs::read_to_string(&path)
        .map_err(|err| format!("failed to read {}: {err}", path.display()))?;
    let project = meldritch_dsl::parse_project(&input).map_err(|err| {
        err.diagnostics()
            .into_iter()
            .map(|diagnostic| diagnostic.message().to_owned())
            .collect::<Vec<_>>()
            .join("\n")
    })?;
    let drums = select_pattern(&project, None, "polyphonic demo")?;
    let range = FrameRange::new(0, frame_count).map_err(|err| err.to_string())?;
    let render_settings = RenderSettings::new(2).expect("stereo settings are valid");
    let samples = load_project_samples(&project, &path)?;
    let mut mix = meldritch_render::render_pattern_samples(
        drums,
        project.tempo(),
        range,
        project.probability_seed(),
        render_settings,
        &samples,
    );

    let mut bass = Pattern::new(
        meldritch_core::PatternId::new(9_001),
        drums.length_steps(),
        drums.steps_per_beat(),
    )
    .map_err(|err| format!("failed to create bass pattern: {err:?}"))?;
    for (step, note, velocity) in [
        (0, 24, 0.85),
        (3, 24, 0.65),
        (6, 27, 0.75),
        (10, 31, 0.8),
        (14, 22, 0.7),
    ] {
        let mut value = Step::new(note).with_velocity(velocity).with_gate(0.8);
        if matches!(step, 0 | 10) {
            value = value.with_tag(EventTag::Accent);
        }
        bass.set_step(TrackId::new(4), StepIndex::new(step), value)
            .map_err(|err| format!("failed to program bass: {err:?}"))?;
    }
    let bass_settings = meldritch_render::dsp::BassVoiceSettings::default();
    let bass_lanes = demo_automation_lanes(frame_count, false)?;
    let bass_audio = meldritch_render::dsp::render_monophonic_pattern_bass_with_automation(
        &bass,
        Some((drums, TrackId::new(1))),
        project.tempo(),
        range,
        project.probability_seed(),
        render_settings,
        bass_settings,
        &bass_lanes,
    );

    let mut chords = Pattern::new(
        meldritch_core::PatternId::new(9_002),
        drums.length_steps(),
        drums.steps_per_beat(),
    )
    .map_err(|err| format!("failed to create chord pattern: {err:?}"))?;
    for (step, notes) in [
        (0, [60, 63, 67]),
        (4, [56, 60, 63]),
        (8, [63, 67, 70]),
        (12, [58, 62, 65]),
    ] {
        for (lane, note) in notes.into_iter().enumerate() {
            chords
                .set_step(
                    TrackId::new(10 + lane as u64),
                    StepIndex::new(step),
                    Step::new(note).with_velocity(0.55),
                )
                .map_err(|err| format!("failed to program chord: {err:?}"))?;
        }
    }
    let chord_settings = meldritch_render::dsp::BassVoiceSettings {
        level: 0.18,
        cutoff_hz: 1_200.0,
        resonance: 0.25,
        filter_envelope_octaves: 0.75,
        sub_level: 0.0,
        glide_seconds: 0.0,
        ..meldritch_render::dsp::BassVoiceSettings::default()
    };
    let chord_lanes = demo_automation_lanes(frame_count, true)?;
    let chord_audio = meldritch_render::dsp::render_polyphonic_pattern_with_automation(
        &chords,
        project.tempo(),
        range,
        project.probability_seed(),
        render_settings,
        chord_settings,
        8,
        &chord_lanes,
    )
    .map_err(|err| format!("failed to render chord synth: {err:?}"))?;
    for ((output_sample, bass_sample), chord_sample) in mix
        .samples_mut()
        .iter_mut()
        .zip(bass_audio.samples())
        .zip(chord_audio.samples())
    {
        *output_sample += bass_sample + chord_sample;
    }
    if normalize {
        mix = mix.normalized_to_peak(0.9);
    }
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|err| format!("failed to create {}: {err}", parent.display()))?;
    }
    meldritch_audio::write_wav_f32(&output, &mix, project.tempo().sample_rate())
        .map_err(|err| format!("failed to write {}: {err}", output.display()))?;
    println!(
        "rendered automated poly demo: frames={}, voices=8, progression=Cm-Ab-Eb-Bb, automation=continuous+waveform/voicing/mute/scenes, peak={:.3}, output={}",
        mix.frames(),
        mix.peak_abs(),
        output.display()
    );
    Ok(())
}

fn demo_automation_lanes(frame_count: u64, chords: bool) -> Result<Vec<AutomationLane>, String> {
    let midpoint = frame_count / 2;
    let lane = |target, values: [f64; 3]| {
        AutomationLane::new(
            target,
            AutomationInterpolation::Linear,
            [0, midpoint, frame_count]
                .into_iter()
                .zip(values)
                .map(|(frame, value)| AutomationPoint {
                    frame,
                    value: AutomationValue::Continuous(value),
                })
                .collect(),
        )
        .map_err(|err| format!("invalid demo automation lane: {err:?}"))
    };
    let discrete = |target, points: &[(u64, i64)]| {
        AutomationLane::new(
            target,
            AutomationInterpolation::Step,
            points
                .iter()
                .map(|(frame, value)| AutomationPoint {
                    frame: *frame,
                    value: AutomationValue::Discrete(*value),
                })
                .collect(),
        )
        .map_err(|err| format!("invalid discrete demo automation lane: {err:?}"))
    };
    let mut lanes = vec![
        lane(
            AutomationTarget::Cutoff,
            if chords {
                [350.0, 2_800.0, 700.0]
            } else {
                [100.0, 900.0, 180.0]
            },
        )?,
        lane(AutomationTarget::Drive, [0.8, 2.8, 1.4])?,
        lane(
            AutomationTarget::Level,
            if chords {
                [0.08, 0.24, 0.16]
            } else {
                [0.18, 0.42, 0.3]
            },
        )?,
        lane(AutomationTarget::Modulation, [0.0, 1.25, 0.2])?,
        lane(AutomationTarget::Ducking, [0.15, 0.55, 0.35])?,
        discrete(
            AutomationTarget::Waveform,
            &[(0, 2), (midpoint, if chords { 1 } else { 3 })],
        )?,
        discrete(
            AutomationTarget::Scene,
            &[
                (0, 1),
                (frame_count / 4, 2),
                (midpoint, 3),
                (frame_count * 3 / 4, 4),
            ],
        )?,
    ];
    if chords {
        lanes.push(discrete(
            AutomationTarget::Voicing,
            &[(0, 0), (midpoint, 5), (frame_count * 3 / 4, 0)],
        )?);
        lanes.push(discrete(
            AutomationTarget::Mute,
            &[(0, 0), (midpoint, 1), (frame_count * 3 / 4, 0)],
        )?);
    }
    Ok(lanes)
}

fn warehouse_automation_lanes(
    frame_count: u64,
    chords: bool,
) -> Result<Vec<AutomationLane>, String> {
    let mut lanes = demo_automation_lanes(frame_count, chords)?;
    lanes.retain(|lane| {
        !matches!(
            lane.target(),
            AutomationTarget::Cutoff
                | AutomationTarget::Drive
                | AutomationTarget::Modulation
                | AutomationTarget::Waveform
        )
    });
    let continuous = |target, values: [f64; 5]| {
        AutomationLane::new(
            target,
            AutomationInterpolation::Linear,
            [
                0,
                frame_count / 4,
                frame_count / 2,
                frame_count * 3 / 4,
                frame_count,
            ]
            .into_iter()
            .zip(values)
            .map(|(frame, value)| AutomationPoint {
                frame,
                value: AutomationValue::Continuous(value),
            })
            .collect(),
        )
        .map_err(|err| format!("invalid warehouse automation lane: {err:?}"))
    };
    lanes.push(continuous(
        AutomationTarget::Cutoff,
        if chords {
            [180.0, 4_800.0, 320.0, 7_200.0, 700.0]
        } else {
            [70.0, 3_600.0, 110.0, 6_500.0, 160.0]
        },
    )?);
    lanes.push(continuous(
        AutomationTarget::Drive,
        [1.4, 4.8, 2.0, 7.5, 2.8],
    )?);
    lanes.push(continuous(
        AutomationTarget::Modulation,
        [0.15, 1.8, 0.35, 2.6, 0.5],
    )?);
    lanes.push(
        AutomationLane::new(
            AutomationTarget::Waveform,
            AutomationInterpolation::Step,
            vec![
                AutomationPoint {
                    frame: 0,
                    value: AutomationValue::Discrete(4),
                },
                AutomationPoint {
                    frame: frame_count / 2,
                    value: AutomationValue::Discrete(2),
                },
                AutomationPoint {
                    frame: frame_count * 3 / 4,
                    value: AutomationValue::Discrete(4),
                },
            ],
        )
        .map_err(|err| format!("invalid warehouse waveform lane: {err:?}"))?,
    );
    Ok(lanes)
}

fn render_arrangement(path: PathBuf, output: PathBuf, normalize: bool) -> Result<(), String> {
    let input = std::fs::read_to_string(&path)
        .map_err(|err| format!("failed to read {}: {err}", path.display()))?;
    let project = meldritch_dsl::parse_project(&input).map_err(|err| {
        err.diagnostics()
            .into_iter()
            .map(|diagnostic| diagnostic.message().to_owned())
            .collect::<Vec<_>>()
            .join("\n")
    })?;
    let pattern = select_pattern(&project, None, "arrangement render")?;
    let arrangement = demo_arrangement(pattern)?;
    let frame_count = arrangement.total_frames(project.tempo());
    let range = FrameRange::new(0, frame_count).map_err(|err| err.to_string())?;
    let settings = RenderSettings::new(2).expect("stereo settings are valid");
    let samples = load_project_samples(&project, &path)?;
    let mut block = meldritch_render::render_arrangement_samples(
        &arrangement,
        project.tempo(),
        range,
        project.probability_seed(),
        settings,
        &samples,
    );
    if normalize {
        block = block.normalized_to_peak(0.9);
    }
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|err| format!("failed to create {}: {err}", parent.display()))?;
    }
    meldritch_audio::write_wav_f32(&output, &block, project.tempo().sample_rate())
        .map_err(|err| format!("failed to write {}: {err}", output.display()))?;
    let seconds = f64::from(block.frames()) / f64::from(project.tempo().sample_rate());
    println!(
        "rendered arrangement: sections=4, frames={}, duration={seconds:.1}s, peak={:.3}, output={}",
        block.frames(),
        block.peak_abs(),
        output.display()
    );
    Ok(())
}

fn demo_arrangement(pattern: &Pattern) -> Result<Arrangement, String> {
    let sections = vec![
        ArrangementSection::new(pattern.clone(), 2, SceneId::new(1))
            .map_err(|err| format!("invalid intro section: {err:?}"))?
            .with_muted_track(TrackId::new(2)),
        ArrangementSection::new(pattern.clone(), 2, SceneId::new(2))
            .map_err(|err| format!("invalid groove section: {err:?}"))?,
        ArrangementSection::new(pattern.clone(), 2, SceneId::new(3))
            .map_err(|err| format!("invalid breakdown section: {err:?}"))?
            .with_muted_track(TrackId::new(1)),
        ArrangementSection::new(pattern.clone(), 2, SceneId::new(4))
            .map_err(|err| format!("invalid return section: {err:?}"))?,
    ];
    Arrangement::new(sections).map_err(|err| format!("failed to build arrangement: {err:?}"))
}

fn render_showcase(
    path: PathBuf,
    output: PathBuf,
    manifest: PathBuf,
    normalize: bool,
    warehouse: bool,
) -> Result<(), String> {
    let input = std::fs::read_to_string(&path)
        .map_err(|err| format!("failed to read {}: {err}", path.display()))?;
    let project = meldritch_dsl::parse_project(&input).map_err(|err| {
        err.diagnostics()
            .into_iter()
            .map(|diagnostic| diagnostic.message().to_owned())
            .collect::<Vec<_>>()
            .join("\n")
    })?;
    if project.patterns().len() < 4 {
        return Err("showcase fixture requires four drum patterns".to_owned());
    }
    let order = [0, 1, 3, 1, 2, 1, 3, 0];
    let names = [
        "intro",
        "groove",
        "full",
        "variation",
        "breakdown",
        "build",
        "climax",
        "outro",
    ];
    let sections = order
        .into_iter()
        .enumerate()
        .map(|(index, pattern)| {
            ArrangementSection::new(
                project.patterns()[pattern].clone(),
                4,
                SceneId::new(index as u64 + 1),
            )
            .map_err(|err| format!("invalid showcase section: {err:?}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let arrangement =
        Arrangement::new(sections).map_err(|err| format!("failed to build showcase: {err:?}"))?;
    let frame_count = arrangement.total_frames(project.tempo());
    let range = FrameRange::new(0, frame_count).map_err(|err| err.to_string())?;
    let settings = RenderSettings::new(2).expect("stereo settings are valid");
    let samples = load_project_samples(&project, &path)?;
    let mut mix = meldritch_render::render_arrangement_samples(
        &arrangement,
        project.tempo(),
        range,
        project.probability_seed(),
        settings,
        &samples,
    );

    let mut bass = Pattern::new(meldritch_core::PatternId::new(9_101), 64, 4)
        .map_err(|err| format!("failed to create showcase bass: {err:?}"))?;
    let bass_phrase: &[(u32, u8)] = if warehouse {
        &[
            (0, 24),
            (3, 24),
            (7, 31),
            (10, 27),
            (14, 22),
            (16, 24),
            (19, 36),
            (23, 31),
            (27, 22),
            (30, 27),
            (32, 20),
            (38, 27),
            (43, 24),
            (48, 31),
            (51, 34),
            (55, 27),
            (59, 22),
            (62, 36),
        ]
    } else {
        &[(0, 24), (12, 24), (16, 20), (32, 27), (48, 22)]
    };
    for &(step, note) in bass_phrase {
        bass.set_step(
            TrackId::new(4),
            StepIndex::new(step),
            Step::new(note).with_velocity(0.78).with_gate(0.85),
        )
        .map_err(|err| format!("failed to program showcase bass: {err:?}"))?;
    }
    let automation = if warehouse {
        warehouse_automation_lanes(frame_count, false)?
    } else {
        demo_automation_lanes(frame_count, false)?
    };
    let bass_settings = if warehouse {
        meldritch_render::dsp::BassVoiceSettings {
            level: 0.42,
            waveform: meldritch_render::dsp::Waveform::SyncFold,
            cutoff_hz: 85.0,
            resonance: 0.86,
            filter_envelope_octaves: 4.5,
            pre_filter_drive: 4.5,
            drive: 3.2,
            sub_level: 0.16,
            glide_seconds: 0.055,
            ..meldritch_render::dsp::BassVoiceSettings::default()
        }
    } else {
        meldritch_render::dsp::BassVoiceSettings::default()
    };
    let bass_modulation = if warehouse {
        use meldritch_render::modulation::{
            Lfo, LfoRate, LfoShape, ModulationDestination, ModulationMatrix, ModulationPolarity,
            ModulationRoute,
        };
        ModulationMatrix::new(vec![
            ModulationRoute {
                source: Lfo {
                    shape: LfoShape::Triangle,
                    rate: LfoRate::Beats(8.0),
                    phase: 0.0,
                    seed: 808,
                },
                destination: ModulationDestination::FilterOctaves,
                depth: 1.4,
                polarity: ModulationPolarity::Bipolar,
            },
            ModulationRoute {
                source: Lfo {
                    shape: LfoShape::SampleAndHold,
                    rate: LfoRate::Beats(0.5),
                    phase: 0.0,
                    seed: 909,
                },
                destination: ModulationDestination::DriveOctaves,
                depth: 0.28,
                polarity: ModulationPolarity::Unipolar,
            },
        ])
    } else {
        meldritch_render::modulation::ModulationMatrix::default()
    };
    let bass_audio = meldritch_render::dsp::render_monophonic_pattern_bass_with_modulation(
        &bass,
        Some((&project.patterns()[3], TrackId::new(1))),
        project.tempo(),
        range,
        project.probability_seed(),
        settings,
        bass_settings,
        &automation,
        &bass_modulation,
    );

    let mut chords = Pattern::new(meldritch_core::PatternId::new(9_102), 64, 4)
        .map_err(|err| format!("failed to create showcase chords: {err:?}"))?;
    for (step, notes) in [
        (0, [60, 63, 67]),
        (16, [56, 60, 63]),
        (32, [63, 67, 70]),
        (48, [58, 62, 65]),
    ] {
        for (lane, note) in notes.into_iter().enumerate() {
            chords
                .set_step(
                    TrackId::new(10 + lane as u64),
                    StepIndex::new(step),
                    Step::new(note).with_velocity(0.55).with_gate(4.0),
                )
                .map_err(|err| format!("failed to program showcase chord: {err:?}"))?;
        }
    }
    let chord_settings = meldritch_render::dsp::BassVoiceSettings {
        level: 0.18,
        waveform: if warehouse {
            meldritch_render::dsp::Waveform::SyncFold
        } else {
            meldritch_render::dsp::Waveform::Saw
        },
        cutoff_hz: 900.0,
        resonance: 0.3,
        filter_envelope_octaves: 0.8,
        pre_filter_drive: if warehouse { 2.8 } else { 1.0 },
        sub_level: 0.0,
        glide_seconds: 0.0,
        ..meldritch_render::dsp::BassVoiceSettings::default()
    };
    let chord_automation = if warehouse {
        warehouse_automation_lanes(frame_count, true)?
    } else {
        demo_automation_lanes(frame_count, true)?
    };
    let chord_audio = meldritch_render::dsp::render_polyphonic_pattern_with_stereo_spread(
        &chords,
        project.tempo(),
        range,
        project.probability_seed(),
        settings,
        chord_settings,
        8,
        &chord_automation,
        if warehouse { 0.9 } else { 0.0 },
    )
    .map_err(|err| format!("failed to render showcase chords: {err:?}"))?;
    let chord_audio = if warehouse {
        let phased = meldritch_render::stereo_fx::apply_tempo_stereo_phaser(
            &chord_audio,
            project.tempo(),
            meldritch_render::stereo_fx::PhaserSettings::default(),
        );
        meldritch_render::effects::apply_modulated_reverb(
            &phased,
            project.tempo(),
            meldritch_render::effects::ModulatedReverbSettings {
                mix: 0.24,
                ..meldritch_render::effects::ModulatedReverbSettings::default()
            },
        )
    } else {
        chord_audio
    };
    for ((output, bass), chords) in mix
        .samples_mut()
        .iter_mut()
        .zip(bass_audio.samples())
        .zip(chord_audio.samples())
    {
        *output += bass + chords;
    }
    if warehouse {
        mix = meldritch_render::mastering::master_bus(
            &mix,
            project.tempo().sample_rate(),
            meldritch_render::mastering::MasteringSettings::default(),
        );
    }
    if normalize {
        mix = mix.normalized_to_peak(0.9);
    }
    for target in [&output, &manifest] {
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|err| format!("failed to create {}: {err}", parent.display()))?;
        }
    }
    meldritch_audio::write_wav_f32(&output, &mix, project.tempo().sample_rate())
        .map_err(|err| format!("failed to write {}: {err}", output.display()))?;
    let manifest_value = serde_json::json!({
        "schema_version": 1,
        "name": project.name(),
        "style": if warehouse { "warehouse" } else { "showcase" },
        "bars": 32,
        "frames": frame_count,
        "duration_seconds": frame_count as f64 / f64::from(project.tempo().sample_rate()),
        "voices": 8,
        "sections": names,
        "drum_pattern_ids": project.patterns().iter().map(|pattern| pattern.id().raw()).collect::<Vec<_>>(),
        "automation_targets": ["cutoff", "drive", "level", "ducking", "modulation", "waveform", "voicing", "mute", "scene"],
        "peak": mix.peak_abs(),
        "finite": mix.samples().iter().all(|sample| sample.is_finite()),
    });
    std::fs::write(
        &manifest,
        serde_json::to_string_pretty(&manifest_value)
            .map_err(|err| format!("failed to encode showcase manifest: {err}"))?,
    )
    .map_err(|err| format!("failed to write {}: {err}", manifest.display()))?;
    println!(
        "rendered 32-bar {}: sections=8, duration={:.1}s, peak={:.3}, output={}, manifest={}",
        if warehouse {
            "warehouse set"
        } else {
            "showcase"
        },
        frame_count as f64 / f64::from(project.tempo().sample_rate()),
        mix.peak_abs(),
        output.display(),
        manifest.display()
    );
    Ok(())
}

fn play_arrangement(
    path: PathBuf,
    from_section: usize,
    to_section: usize,
    loops: u32,
    normalize: bool,
) -> Result<(), String> {
    if loops == 0 {
        return Err("playback loop count must be at least one".to_owned());
    }
    let input = std::fs::read_to_string(&path)
        .map_err(|err| format!("failed to read {}: {err}", path.display()))?;
    let project = meldritch_dsl::parse_project(&input).map_err(|err| {
        err.diagnostics()
            .into_iter()
            .map(|diagnostic| diagnostic.message().to_owned())
            .collect::<Vec<_>>()
            .join("\n")
    })?;
    let pattern = select_pattern(&project, None, "arrangement playback")?;
    let arrangement = demo_arrangement(pattern)?;
    let range = arrangement
        .section_range(project.tempo(), from_section, to_section)
        .ok_or_else(|| {
            format!(
                "invalid section range {from_section}..{to_section}; arrangement has {} sections",
                arrangement.sections().len()
            )
        })?;
    let settings = RenderSettings::new(2).expect("stereo settings are valid");
    let samples = load_project_samples(&project, &path)?;
    let mut block = meldritch_render::render_arrangement_samples(
        &arrangement,
        project.tempo(),
        range,
        project.probability_seed(),
        settings,
        &samples,
    );
    if normalize {
        block = block.normalized_to_peak(0.9);
    }
    println!(
        "playing arrangement sections {from_section}..{to_section}: source_frames={}..{}, loops={loops}, peak={:.3}",
        range.start(),
        range.end(),
        block.peak_abs()
    );
    let report = meldritch_audio::device_output::play_blocking(
        &block,
        project.tempo().sample_rate(),
        loops,
    )?;
    println!(
        "played arrangement: callbacks={}, underruns={}, misses={}, final_position={}",
        report.callbacks, report.underruns, report.missed_artifacts, report.final_position
    );
    Ok(())
}

fn play_showcase(
    audio: PathBuf,
    frame_limit: Option<u32>,
    loops: u32,
    require_clean: bool,
) -> Result<(), String> {
    if loops == 0 {
        return Err("playback loop count must be at least one".to_owned());
    }
    let source = meldritch_audio::read_wav(&audio)
        .map_err(|err| format!("failed to read {}: {err}", audio.display()))?;
    let frames = frame_limit
        .unwrap_or_else(|| source.frames())
        .min(source.frames());
    if frames == 0 {
        return Err("showcase playback must contain at least one frame".to_owned());
    }
    let channels = usize::from(source.channels());
    let mut block = meldritch_audio::AudioBlock::silent(source.channels(), frames);
    block
        .samples_mut()
        .copy_from_slice(&source.samples()[..frames as usize * channels]);
    println!(
        "playing showcase: frames={}, duration={:.2}s, loops={}, clean_check={require_clean}",
        frames,
        f64::from(frames) / f64::from(source.sample_rate()),
        loops
    );
    let report =
        meldritch_audio::device_output::play_blocking(&block, source.sample_rate(), loops)?;
    println!(
        "showcase playback: device={}, callbacks={}, stream_errors={}, underruns={}, misses={}, final_position={}",
        report.device_name,
        report.callbacks,
        report.stream_errors,
        report.underruns,
        report.missed_artifacts,
        report.final_position
    );
    if require_clean && (report.underruns != 0 || report.missed_artifacts != 0) {
        return Err(format!(
            "showcase smoke failed: underruns={}, misses={}",
            report.underruns, report.missed_artifacts
        ));
    }
    if report.stream_errors != 0 {
        eprintln!(
            "warning: host reported {} backend stream notification(s)",
            report.stream_errors
        );
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn warehouse_showcase(
    project: PathBuf,
    output: PathBuf,
    manifest: PathBuf,
    loops: u32,
    reuse: bool,
    require_clean: bool,
    frames: Option<u32>,
) -> Result<(), String> {
    if loops == 0 {
        return Err("warehouse loop count must be at least one".to_owned());
    }
    if reuse {
        if !output.exists() {
            return Err(format!(
                "cannot reuse missing warehouse render {}",
                output.display()
            ));
        }
        println!("reusing warehouse render: {}", output.display());
    } else {
        println!(
            "preparing warehouse showcase: project={}, output={}",
            project.display(),
            output.display()
        );
        render_showcase(project, output.clone(), manifest, true, true)?;
    }
    play_showcase(output, frames, loops, require_clean)
}

fn warehouse_soak(audio: PathBuf, seconds: u32, require_clean: bool) -> Result<(), String> {
    if seconds == 0 {
        return Err("warehouse soak duration must be at least one second".to_owned());
    }
    let source = meldritch_audio::read_wav(&audio)
        .map_err(|err| format!("failed to read {}: {err}", audio.display()))?;
    let soak_frames = source
        .frames()
        .min(source.sample_rate().saturating_mul(4))
        .max(1);
    let channels = usize::from(source.channels());
    let mut playback_block = meldritch_audio::AudioBlock::silent(source.channels(), soak_frames);
    playback_block
        .samples_mut()
        .copy_from_slice(&source.samples()[..soak_frames as usize * channels]);
    let requested_frames = u64::from(seconds) * u64::from(source.sample_rate());
    let loops = requested_frames
        .div_ceil(u64::from(soak_frames))
        .min(u64::from(u32::MAX)) as u32;

    let stress_frames = source
        .frames()
        .min(source.sample_rate().saturating_mul(2))
        .max(1);
    let mut stress_source = meldritch_audio::AudioBlock::silent(source.channels(), stress_frames);
    stress_source
        .samples_mut()
        .copy_from_slice(&source.samples()[..stress_frames as usize * channels]);
    let running = Arc::new(std::sync::atomic::AtomicBool::new(true));
    let completed = Arc::new(std::sync::atomic::AtomicU64::new(0));
    let worst_micros = Arc::new(std::sync::atomic::AtomicU64::new(0));
    let worker_running = Arc::clone(&running);
    let worker_completed = Arc::clone(&completed);
    let worker_worst = Arc::clone(&worst_micros);
    let tempo = meldritch_core::Tempo::new(142.0, source.sample_rate())
        .map_err(|err| format!("invalid soak tempo: {err}"))?;
    let stress = std::thread::spawn(move || {
        let mut generation = 0_u64;
        while worker_running.load(std::sync::atomic::Ordering::Relaxed) {
            let phase = (generation % 8) as f64 / 7.0;
            let settings = meldritch_render::performance_fx::PerformanceFxSettings {
                delay_feedback: 0.2 + phase * 0.65,
                phaser_mix: 0.15 + (1.0 - phase) * 0.7,
                reverb_freeze: generation.is_multiple_of(7),
                modulation_depth: phase * 0.8,
                master_drive: 1.0 + phase * 3.0,
            };
            let started = Instant::now();
            let rendered = meldritch_render::performance_fx::apply_performance_fx(
                &stress_source,
                tempo,
                settings,
            );
            if rendered.samples().iter().all(|sample| sample.is_finite()) {
                worker_completed.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            }
            worker_worst.fetch_max(
                started.elapsed().as_micros().min(u128::from(u64::MAX)) as u64,
                std::sync::atomic::Ordering::Relaxed,
            );
            generation = generation.wrapping_add(1);
            std::thread::sleep(Duration::from_millis(10));
        }
    });

    println!(
        "warehouse soak: requested={}s, block_frames={}, loops={}, clean_check={require_clean}",
        seconds, soak_frames, loops
    );
    let report =
        meldritch_audio::device_output::play_blocking(&playback_block, source.sample_rate(), loops);
    running.store(false, std::sync::atomic::Ordering::Relaxed);
    stress
        .join()
        .map_err(|_| "warehouse soak DSP worker panicked".to_owned())?;
    let report = report?;
    let completed = completed.load(std::sync::atomic::Ordering::Relaxed);
    let worst_micros = worst_micros.load(std::sync::atomic::Ordering::Relaxed);
    println!(
        "warehouse soak result: device={}, callbacks={}, underruns={}, misses={}, stream_errors={}, dsp_renders={}, worst_dsp_ms={:.2}",
        report.device_name,
        report.callbacks,
        report.underruns,
        report.missed_artifacts,
        report.stream_errors,
        completed,
        worst_micros as f64 / 1_000.0,
    );
    if require_clean
        && (report.underruns != 0 || report.missed_artifacts != 0 || report.stream_errors != 0)
    {
        return Err(format!(
            "warehouse soak failed: underruns={}, misses={}, stream_errors={}",
            report.underruns, report.missed_artifacts, report.stream_errors
        ));
    }
    Ok(())
}

fn play_samples(
    path: PathBuf,
    pattern_id: Option<u64>,
    frames: Option<u64>,
    channels: u16,
    loops: u32,
    normalize: bool,
) -> Result<(), String> {
    let input = std::fs::read_to_string(&path)
        .map_err(|err| format!("failed to read {}: {err}", path.display()))?;
    let project = meldritch_dsl::parse_project(&input).map_err(|err| {
        err.diagnostics()
            .into_iter()
            .map(|diagnostic| diagnostic.message().to_owned())
            .collect::<Vec<_>>()
            .join("\n")
    })?;
    let pattern = select_pattern(&project, pattern_id, "play")?;
    let frame_count = frames.unwrap_or_else(|| {
        project
            .tempo()
            .step_start_frame(u64::from(pattern.length_steps()), pattern.steps_per_beat())
    });
    let range = FrameRange::new(0, frame_count).map_err(|err| err.to_string())?;
    let settings =
        RenderSettings::new(channels).map_err(|err| format!("invalid render settings: {err:?}"))?;
    let samples_by_note = load_project_samples(&project, &path)?;
    let mut block = meldritch_render::render_pattern_samples(
        pattern,
        project.tempo(),
        range,
        project.probability_seed(),
        settings,
        &samples_by_note,
    );
    if normalize {
        block = block.normalized_to_peak(0.9);
    }
    if block.peak_abs() == 0.0 {
        return Err("rendered pattern is silent; nothing to play".to_owned());
    }

    println!(
        "playing pattern {}: frames={}, loops={}, peak={:.3}",
        pattern.id().raw(),
        block.frames(),
        loops,
        block.peak_abs()
    );
    let report = meldritch_audio::device_output::play_blocking(
        &block,
        project.tempo().sample_rate(),
        loops,
    )?;
    println!(
        "played: device={}, rate={}, channels={}, format={}, callbacks={}, stream_errors={}, commands_applied={}, commands_dropped={}, underruns={}, missed_artifacts={}, final_position={}",
        report.device_name,
        report.sample_rate,
        report.channels,
        report.sample_format,
        report.callbacks,
        report.stream_errors,
        report.commands_applied,
        report.commands_dropped,
        report.underruns,
        report.missed_artifacts,
        report.final_position
    );
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn play_realtime_samples(
    path: PathBuf,
    pattern_id: Option<u64>,
    frames: Option<u64>,
    channels: u16,
    loops: u32,
    chunk_frames: u32,
    warm_chunks: usize,
    workers: usize,
) -> Result<(), String> {
    if loops == 0 {
        return Err("playback loop count must be at least one".to_owned());
    }
    let input = std::fs::read_to_string(&path)
        .map_err(|err| format!("failed to read {}: {err}", path.display()))?;
    let project = meldritch_dsl::parse_project(&input).map_err(|err| {
        err.diagnostics()
            .into_iter()
            .map(|diagnostic| diagnostic.message().to_owned())
            .collect::<Vec<_>>()
            .join("\n")
    })?;
    let pattern = select_pattern(&project, pattern_id, "realtime playback")?;
    let frame_count = frames.unwrap_or_else(|| {
        project
            .tempo()
            .step_start_frame(u64::from(pattern.length_steps()), pattern.steps_per_beat())
    });
    let frame_count = u32::try_from(frame_count)
        .map_err(|_| "realtime playback frame count exceeds u32::MAX".to_owned())?;
    let settings =
        RenderSettings::new(channels).map_err(|err| format!("invalid render settings: {err:?}"))?;
    let samples_by_note = Arc::new(load_project_samples(&project, &path)?);
    let worker_count = if workers == 0 {
        std::thread::available_parallelism().map_or(1, std::num::NonZero::get)
    } else {
        workers
    };
    let (control, engine) = meldritch_audio::device_output::playback_session_parts(16)?;
    let config = RenderCoordinatorConfig::new(
        worker_count,
        frame_count,
        chunk_frames,
        warm_chunks,
        Duration::from_millis(10),
    )
    .map_err(|err| format!("invalid realtime render configuration: {err:?}"))?;
    let mut coordinator = RenderCoordinator::new(
        config,
        pattern.clone(),
        project.tempo(),
        project.probability_seed(),
        settings,
        samples_by_note,
        control.status_monitor(),
    )
    .map_err(|err| format!("failed to start render coordinator: {err:?}"))?;
    let session = meldritch_audio::device_output::RealtimePlaybackSession::open_default(
        coordinator.audio_reader(),
        project.tempo().sample_rate(),
        loops,
        &control,
        engine,
    )?;

    println!(
        "realtime playing pattern {}: frames={}, chunks={}, workers={}, loops={}",
        pattern.id().raw(),
        frame_count,
        frame_count.div_ceil(chunk_frames),
        worker_count,
        loops
    );
    control
        .play()
        .map_err(|_| "failed to enqueue realtime play command".to_owned())?;
    let audio_duration = Duration::from_secs_f64(
        f64::from(frame_count) * f64::from(loops) / f64::from(project.tempo().sample_rate()),
    );
    let deadline = Instant::now() + audio_duration + Duration::from_secs(5);
    while !session.is_finished() {
        if Instant::now() >= deadline {
            return Err("realtime playback timed out before completion".to_owned());
        }
        std::thread::sleep(Duration::from_millis(5));
    }

    let playback = session.report();
    let refreshes = coordinator.diagnostics().refreshes;
    coordinator.wake();
    let _ = coordinator.wait_for_refreshes(refreshes + 1, Duration::from_secs(1));
    let render = coordinator.diagnostics();
    coordinator.shutdown();
    println!(
        "realtime played: device={}, callbacks={}, underruns={}, missed_artifacts={}, ready_chunks={}, published_artifacts={}, rejected_artifacts={}, refreshes={}, completed_jobs={}",
        playback.device_name,
        playback.callbacks,
        playback.underruns,
        playback.missed_artifacts,
        render.publication.ready_chunks,
        render.publication.published_artifacts,
        render.publication.rejected_artifacts,
        render.refreshes,
        render.workers.completed_jobs,
    );
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn tui_samples(
    path: PathBuf,
    pattern_id: Option<u64>,
    frames: Option<u64>,
    channels: u16,
    chunk_frames: u32,
    warm_chunks: usize,
    workers: usize,
    note: u8,
    bassline: bool,
    chords: bool,
    future_log: Option<PathBuf>,
    warehouse: bool,
) -> Result<(), String> {
    let input = std::fs::read_to_string(&path)
        .map_err(|err| format!("failed to read {}: {err}", path.display()))?;
    let project = meldritch_dsl::parse_project(&input).map_err(|err| {
        err.diagnostics()
            .into_iter()
            .map(|diagnostic| diagnostic.message().to_owned())
            .collect::<Vec<_>>()
            .join("\n")
    })?;
    let mut pattern = select_pattern(&project, pattern_id, "TUI playback")?.clone();
    let frame_count = frames.unwrap_or_else(|| {
        project
            .tempo()
            .step_start_frame(u64::from(pattern.length_steps()), pattern.steps_per_beat())
    });
    let frame_count = u32::try_from(frame_count)
        .map_err(|_| "TUI playback frame count exceeds u32::MAX".to_owned())?;
    let settings =
        RenderSettings::new(channels).map_err(|err| format!("invalid render settings: {err:?}"))?;
    let mut samples_by_note = load_project_samples(&project, &path)?;
    if bassline {
        let bass_track = TrackId::new(4);
        let phrase = [
            (0, 24, 0.85),
            (3, 24, 0.65),
            (6, 27, 0.75),
            (10, 31, 0.8),
            (14, 22, 0.7),
        ];
        for (step, bass_note, velocity) in phrase {
            if step < pattern.length_steps() {
                let mut value = Step::new(bass_note).with_velocity(velocity).with_gate(0.8);
                if matches!(step, 0 | 10) {
                    value = value.with_tag(EventTag::Accent);
                }
                pattern
                    .set_step(bass_track, StepIndex::new(step), value)
                    .map_err(|err| format!("failed to program bass step: {err:?}"))?;
            }
        }
        for bass_note in [22, 24, 27, 31, note] {
            samples_by_note.entry(bass_note).or_insert_with(|| {
                meldritch_render::dsp::synthesize_bass_sample(
                    bass_note,
                    project.tempo().sample_rate(),
                    12_000,
                    meldritch_render::dsp::BassVoiceSettings::default(),
                )
            });
        }
    }
    if chords {
        for (step, notes) in [
            (0, [60, 63, 67]),
            (4, [56, 60, 63]),
            (8, [63, 67, 70]),
            (12, [58, 62, 65]),
        ] {
            for (lane, chord_note) in notes.into_iter().enumerate() {
                pattern
                    .set_step(
                        TrackId::new(10 + lane as u64),
                        StepIndex::new(step),
                        Step::new(chord_note).with_velocity(0.55),
                    )
                    .map_err(|err| format!("failed to program chord: {err:?}"))?;
            }
        }
    }
    let samples_by_note = Arc::new(samples_by_note);
    if !samples_by_note.contains_key(&note) {
        return Err(format!("toggle note {note} has no loaded sample"));
    }
    let worker_count = if workers == 0 {
        std::thread::available_parallelism().map_or(1, std::num::NonZero::get)
    } else {
        workers
    };
    let (playback, engine) = meldritch_audio::device_output::playback_session_parts(32)?;
    let mut state = meldritch_render::coordinator::SampleRenderState::new(
        pattern.clone(),
        project.tempo(),
        project.probability_seed(),
        settings,
        Arc::clone(&samples_by_note),
    );
    let bass_settings = if warehouse {
        meldritch_render::dsp::BassVoiceSettings {
            level: 0.42,
            waveform: meldritch_render::dsp::Waveform::SyncFold,
            cutoff_hz: 85.0,
            resonance: 0.86,
            filter_envelope_octaves: 4.5,
            pre_filter_drive: 4.5,
            drive: 3.2,
            sub_level: 0.16,
            glide_seconds: 0.055,
            ..meldritch_render::dsp::BassVoiceSettings::default()
        }
    } else {
        meldritch_render::dsp::BassVoiceSettings::default()
    };
    let effect_rules = if chords {
        vec![
            meldritch_render::effects::EffectSendRule {
                bus: meldritch_render::effects::EffectBus::Delay,
                required_tag: EventTag::Accent,
                send_gain: 0.32,
            },
            meldritch_render::effects::EffectSendRule {
                bus: meldritch_render::effects::EffectBus::Reverb,
                required_tag: EventTag::Ghost,
                send_gain: 0.24,
            },
        ]
    } else {
        Vec::new()
    };
    if !effect_rules.is_empty() {
        state = state.with_effect_rules(effect_rules.clone());
    }
    if bassline {
        state = state.with_bass_layer(TrackId::new(4), bass_settings);
    }
    if chords {
        state = state
            .with_chord_layer(meldritch_render::coordinator::ChordLayer {
                first_track: TrackId::new(10),
                last_track: TrackId::new(12),
                settings: meldritch_render::dsp::BassVoiceSettings {
                    level: 0.18,
                    cutoff_hz: 1_200.0,
                    resonance: 0.25,
                    filter_envelope_octaves: 0.75,
                    sub_level: 0.0,
                    glide_seconds: 0.0,
                    ..meldritch_render::dsp::BassVoiceSettings::default()
                },
                voice_count: 8,
            })
            .with_automation(demo_automation_lanes(u64::from(frame_count), true)?)
            .with_sidechain(meldritch_render::dynamics::SidechainRelation {
                control_track: TrackId::new(1),
                source_role: meldritch_core::SourceRole::Kick,
                target_role: meldritch_core::SourceRole::Bass,
                settings: meldritch_render::dynamics::SidechainSettings::default(),
            });
    }
    let config = RenderCoordinatorConfig::new(
        worker_count,
        frame_count,
        chunk_frames,
        warm_chunks,
        Duration::from_millis(10),
    )
    .map_err(|err| format!("invalid TUI render configuration: {err:?}"))?;
    let coordinator =
        RenderCoordinator::new_from_state(config, state.clone(), playback.status_monitor())
            .map_err(|err| format!("failed to start render coordinator: {err:?}"))?;
    let automation_view = state.automation().to_vec();
    let sidechain_view = state.sidechain();
    let editor = meldritch_render::live_edit::LivePatternEditor::new(state, frame_count);
    let mut controller = meldritch_app::AppController::new(
        playback,
        coordinator,
        editor,
        meldritch_app::Selection {
            track: TrackId::new(if bassline { 4 } else { 1 }),
            step: StepIndex::new(0),
        },
        256,
    );
    if bassline {
        controller.enable_bass_synth(
            bass_settings,
            [22, 24, 27, 31, note].into_iter().collect(),
            12_000,
        );
    }
    if warehouse {
        controller.enable_performance_fx(
            meldritch_render::performance_fx::PerformanceFxSettings::default(),
        );
        controller
            .configure_phrase_variations(project.patterns().iter().enumerate().map(
                |(index, pattern)| {
                    let mut variation = pattern.clone();
                    variation
                        .set_step(
                            TrackId::new(3),
                            StepIndex::new(15),
                            Step::new(42)
                                .with_velocity(0.82)
                                .with_tag(EventTag::Fill)
                                .with_tag(EventTag::Accent),
                        )
                        .expect("warehouse variation step is in range");
                    (
                        SceneId::new(index as u64 + 1),
                        vec![pattern.clone(), variation],
                    )
                },
            ))
            .map_err(|err| format!("failed to configure warehouse phrases: {err}"))?;
    }
    if !automation_view.is_empty() {
        controller.show_automation(automation_view);
    }
    if !effect_rules.is_empty() {
        controller.show_effect_sends(effect_rules);
    }
    if let Some(relation) = sidechain_view {
        controller.show_sidechain(relation);
    }
    let _session = meldritch_audio::device_output::RealtimePlaybackSession::open_default(
        controller.coordinator().audio_reader(),
        project.tempo().sample_rate(),
        u32::MAX,
        controller.playback_control(),
        engine,
    )?;
    if future_log.is_none() {
        return meldritch_tui::run(&mut controller, Step::new(note))
            .map_err(|err| format!("TUI failed: {err}"));
    }

    let future_log = future_log.expect("live showcase has a future log path");
    let mut library = if future_log.exists() {
        serde_json::from_str::<FutureLibrary>(
            &std::fs::read_to_string(&future_log)
                .map_err(|err| format!("failed to read {}: {err}", future_log.display()))?,
        )
        .unwrap_or_default()
    } else {
        FutureLibrary::default()
    };
    library.schema_version = 2;
    library.sessions = library.sessions.saturating_add(1);
    let session = library.sessions;
    let cue_frames = [
        (frame_count / 8, "filter opening"),
        (frame_count / 4, "scene 2 / modulation rising"),
        (frame_count * 3 / 8, "drive build"),
        (frame_count / 2, "waveform and voicing change"),
        (frame_count * 5 / 8, "breakdown"),
        (frame_count * 3 / 4, "full-pattern return"),
        (frame_count * 7 / 8, "filter landing"),
    ];
    let mut ranked = library.learned.clone();
    ranked.sort_by(|left, right| {
        right
            .score
            .cmp(&left.score)
            .then_with(|| right.last_session.cmp(&left.last_session))
            .then_with(|| left.action.cmp(&right.action))
    });
    let learned_phrase_cues = learned_phrase_schedule(&library, frame_count, 8);
    let learned_dsp_cues = learned_dsp_schedule(&library, frame_count, 12);
    controller.show_learned_phrase_cues(
        learned_phrase_cues
            .iter()
            .map(|(frame, scene)| meldritch_app::LearnedPhraseCueView {
                scene: *scene,
                frame: u64::from(*frame),
            })
            .collect(),
    );
    let wanted_ready = warm_chunks.min(frame_count.div_ceil(chunk_frames) as usize);
    if !controller
        .coordinator()
        .wait_for_ready_chunks(wanted_ready.max(1), Duration::from_secs(10))
    {
        return Err("live showcase could not prepare its initial audio horizon".to_owned());
    }
    for learned in ranked.into_iter().take(4) {
        match learned.action {
            LearnedAction::QueuePhrase(_) | LearnedAction::QueuePhraseVariation(_, _) => continue,
            action if is_dsp_action(action) => continue,
            action => {
                controller
                    .handle_input(action.input().expect("non-phrase action has an input"))
                    .map_err(|err| format!("failed to prepare learned future: {err:?}"))?;
            }
        }
    }
    controller
        .dispatch(meldritch_app::AppCommand::Play)
        .map_err(|err| format!("failed to start showcase transport: {err:?}"))?;
    let captured = Arc::new(std::sync::Mutex::new(Vec::<CapturedFuture>::new()));
    let performer_capture = Arc::clone(&captured);
    let override_grace = Arc::new(std::sync::Mutex::new(PerformerOverrideGrace::default()));
    let tick_override_grace = Arc::clone(&override_grace);
    let input_override_grace = Arc::clone(&override_grace);
    let override_grace_frames = (project.tempo().frames_per_beat() * 4.0).round() as u32;
    let mut fired = vec![false; cue_frames.len()];
    let mut phrase_fired = vec![false; learned_phrase_cues.len()];
    let mut dsp_fired = vec![false; learned_dsp_cues.len()];
    let mut previous_position = 0;
    meldritch_tui::run_with_hooks(
        &mut controller,
        Step::new(note),
        move |controller| {
            if controller
                .tick_performance_fx()
                .map_err(|err| format!("FX publication failed: {err:?}"))?
            {
                return Ok(Some("Performance FX published".to_owned()));
            }
            if let Some(launch) = controller
                .tick_phrase_launch()
                .map_err(|err| format!("phrase launch failed: {err:?}"))?
                && warehouse
            {
                return Ok(Some(format!(
                    "Warehouse phrase launched: {:?}",
                    launch.gesture
                )));
            }
            let position = controller.view_model().transport.position;
            if position < previous_position {
                fired.fill(false);
                phrase_fired.fill(false);
                dsp_fired.fill(false);
                tick_override_grace
                    .lock()
                    .expect("performer override lock poisoned")
                    .reset();
            }
            previous_position = position;
            let performer_override = tick_override_grace
                .lock()
                .expect("performer override lock poisoned")
                .suppresses(position);
            if warehouse
                && !performer_override
                && controller.view_model().performance.queued.is_none()
            {
                for (index, (frame, scene)) in learned_phrase_cues.iter().enumerate() {
                    if !phrase_fired[index] && position >= *frame {
                        controller
                            .queue_phrase_scene(*scene)
                            .map_err(|err| format!("learned phrase cue failed: {err:?}"))?;
                        phrase_fired[index] = true;
                        return Ok(Some(format!(
                            "Learned warehouse phrase queued: scene {}",
                            scene.raw()
                        )));
                    }
                }
            }
            if warehouse && !performer_override {
                for (index, (frame, action)) in learned_dsp_cues.iter().enumerate() {
                    if !dsp_fired[index] && position >= *frame {
                        controller
                            .handle_input(action.input().expect("DSP action has an input"))
                            .map_err(|err| format!("learned DSP cue failed: {err:?}"))?;
                        dsp_fired[index] = true;
                        return Ok(Some(format!("Learned DSP gesture: {action:?}")));
                    }
                }
            }
            for (index, (frame, description)) in cue_frames.iter().enumerate() {
                if !fired[index] && position >= *frame {
                    fired[index] = true;
                    return Ok(Some(format!("Autopilot: {description}")));
                }
            }
            Ok(None)
        },
        move |controller, input, _result| {
            if warehouse
                && matches!(
                    input,
                    meldritch_app::AppInput::QueueNextScene
                        | meldritch_app::AppInput::QueuePhrase(_)
                        | meldritch_app::AppInput::QueuePhraseVariation(_, _)
                        | meldritch_app::AppInput::IncreaseDelayFeedback
                        | meldritch_app::AppInput::DecreaseDelayFeedback
                        | meldritch_app::AppInput::IncreasePhaserMix
                        | meldritch_app::AppInput::DecreasePhaserMix
                        | meldritch_app::AppInput::ToggleReverbFreeze
                        | meldritch_app::AppInput::IncreaseModulationDepth
                        | meldritch_app::AppInput::DecreaseModulationDepth
                        | meldritch_app::AppInput::IncreaseMasterDrive
                        | meldritch_app::AppInput::DecreaseMasterDrive
                )
            {
                input_override_grace
                    .lock()
                    .expect("performer override lock poisoned")
                    .record(
                        controller.view_model().transport.position,
                        override_grace_frames,
                        frame_count,
                    );
            }
            let action = if warehouse && matches!(input, meldritch_app::AppInput::QueueNextScene) {
                controller
                    .view_model()
                    .performance
                    .queued
                    .and_then(|queued| match queued.gesture {
                        meldritch_render::futures::PerformanceGesture::QueueScene(scene) => {
                            Some(LearnedAction::QueuePhrase(scene.raw()))
                        }
                        _ => None,
                    })
            } else {
                LearnedAction::from_input(input)
            };
            if let Some(action) = action {
                let position = controller.view_model().transport.position;
                performer_capture
                    .lock()
                    .expect("future capture lock poisoned")
                    .push(CapturedFuture {
                        origin: "performer".to_owned(),
                        action,
                        frame: position,
                        phase: f64::from(position) / f64::from(frame_count),
                    });
            }
        },
    )
    .map_err(|err| format!("TUI failed: {err}"))?;
    let captured = Arc::try_unwrap(captured)
        .expect("future capture has no remaining owners")
        .into_inner()
        .expect("future capture lock poisoned");
    merge_performer_futures(&mut library, captured, session);
    if let Some(parent) = future_log.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|err| format!("failed to create {}: {err}", parent.display()))?;
    }
    std::fs::write(
        &future_log,
        serde_json::to_string_pretty(&library)
            .map_err(|err| format!("failed to encode future log: {err}"))?,
    )
    .map_err(|err| format!("failed to write {}: {err}", future_log.display()))?;
    println!("saved possible futures: {}", future_log.display());
    Ok(())
}

fn render_samples(path: PathBuf, options: RenderSamplesOptions) -> Result<(), String> {
    if let Some(output) = &options.output
        && output.exists()
    {
        return Err(format!(
            "output {} already exists; choose a new path",
            output.display()
        ));
    }
    if let Some(manifest) = &options.manifest
        && manifest.exists()
    {
        return Err(format!(
            "manifest {} already exists; choose a new path",
            manifest.display()
        ));
    }

    let input = std::fs::read_to_string(&path)
        .map_err(|err| format!("failed to read {}: {err}", path.display()))?;
    let project = meldritch_dsl::parse_project(&input).map_err(|err| {
        err.diagnostics()
            .into_iter()
            .map(|diagnostic| diagnostic.message().to_owned())
            .collect::<Vec<_>>()
            .join("\n")
    })?;
    let pattern = select_pattern(&project, options.pattern_id, "render")?;
    let compiled = meldritch_dsl::compile_project(&project).map_err(|err| {
        err.diagnostics()
            .iter()
            .map(|diagnostic| diagnostic.message().to_owned())
            .collect::<Vec<_>>()
            .join("\n")
    })?;
    let settings = RenderSettings::new(options.channels)
        .map_err(|err| format!("invalid render settings: {err:?}"))?;
    let samples_by_note = load_project_samples(&project, &path)?;
    let range = FrameRange::new(0, options.frames).map_err(|err| err.to_string())?;
    let artifact_key = meldritch_render::pattern_sample_artifact_key(
        pattern,
        project.tempo(),
        range,
        project.probability_seed(),
        settings,
        &samples_by_note,
    );
    let mut cache_probe_summary = None;
    let mut block = if options.cache_probe {
        let mut cache = ArtifactCache::new();
        let first = meldritch_render::render_pattern_samples_cached(
            &mut cache,
            pattern,
            project.tempo(),
            range,
            project.probability_seed(),
            settings,
            &samples_by_note,
        );
        let second = meldritch_render::render_pattern_samples_cached(
            &mut cache,
            pattern,
            project.tempo(),
            range,
            project.probability_seed(),
            settings,
            &samples_by_note,
        );
        println!(
            "cache probe: first={}, second={}, artifacts={}",
            format_cache_status(first.status()),
            format_cache_status(second.status()),
            cache.len()
        );
        cache_probe_summary = Some(CacheProbeSummary {
            first: format_cache_status(first.status()).to_owned(),
            second: format_cache_status(second.status()).to_owned(),
            artifacts: cache.len(),
        });
        second.into_block()
    } else {
        meldritch_render::render_pattern_samples(
            pattern,
            project.tempo(),
            range,
            project.probability_seed(),
            settings,
            &samples_by_note,
        )
    };
    if options.normalize {
        block = block.normalized_to_peak(1.0);
    }

    let peak = block.peak_abs();
    let nonzero_samples = block
        .samples()
        .iter()
        .filter(|sample| **sample != 0.0)
        .count();
    let finite = block.samples().iter().all(|sample| sample.is_finite());

    println!(
        "rendered samples: frames={}, channels={}, finite={}, nonzero_samples={}, peak={}",
        block.frames(),
        block.channels(),
        finite,
        nonzero_samples,
        peak
    );

    if let Some(output) = &options.output {
        meldritch_audio::write_wav_f32(output, &block, project.tempo().sample_rate())
            .map_err(|err| format!("failed to write {}: {err}", output.display()))?;
        println!("wrote: {}", output.display());
    }

    if let Some(manifest) = options.manifest {
        let summary = RenderManifest::from_render(RenderManifestInput {
            project_path: &path,
            output_path: options.output.as_deref(),
            pattern,
            range,
            channels: options.channels,
            normalize: options.normalize,
            cache_probe: options.cache_probe,
            cache_probe_summary,
            artifact_key,
            compiled: &compiled,
            samples_by_note: &samples_by_note,
            control: None,
            peak,
            nonzero_samples,
            finite,
        })?;
        let json = serde_json::to_string_pretty(&summary)
            .map_err(|err| format!("failed to encode render manifest: {err}"))?;
        std::fs::write(&manifest, json)
            .map_err(|err| format!("failed to write {}: {err}", manifest.display()))?;
        println!("wrote manifest: {}", manifest.display());
    }

    Ok(())
}

fn render_controlled_samples(
    path: PathBuf,
    options: RenderControlledSamplesOptions,
) -> Result<(), String> {
    if let Some(output) = &options.output
        && output.exists()
    {
        return Err(format!(
            "output {} already exists; choose a new path",
            output.display()
        ));
    }
    if let Some(manifest) = &options.manifest
        && manifest.exists()
    {
        return Err(format!(
            "manifest {} already exists; choose a new path",
            manifest.display()
        ));
    }
    if !options.active_scale.is_finite() || options.active_scale < 0.0 {
        return Err(format!(
            "active scale must be finite and non-negative, got {}",
            options.active_scale
        ));
    }

    let input = std::fs::read_to_string(&path)
        .map_err(|err| format!("failed to read {}: {err}", path.display()))?;
    let project = meldritch_dsl::parse_project(&input).map_err(|err| {
        err.diagnostics()
            .into_iter()
            .map(|diagnostic| diagnostic.message().to_owned())
            .collect::<Vec<_>>()
            .join("\n")
    })?;
    let pattern = select_pattern(&project, Some(options.pattern_id), "render")?;
    let compiled = meldritch_dsl::compile_project(&project).map_err(|err| {
        err.diagnostics()
            .iter()
            .map(|diagnostic| diagnostic.message().to_owned())
            .collect::<Vec<_>>()
            .join("\n")
    })?;
    let settings = RenderSettings::new(options.channels)
        .map_err(|err| format!("invalid render settings: {err:?}"))?;
    let samples_by_note = load_project_samples(&project, &path)?;
    let range = FrameRange::new(0, options.frames).map_err(|err| err.to_string())?;
    let control = build_control_event_schedule(&path, options.pattern_id, options.frames)?;
    let artifact_key = meldritch_render::pattern_sample_artifact_key(
        pattern,
        project.tempo(),
        range,
        project.probability_seed(),
        settings,
        &samples_by_note,
    );
    let active_event_starts = control
        .events
        .iter()
        .filter(|event| event.active_controller_count > 0)
        .map(|event| event.event.range.start)
        .collect::<BTreeSet<_>>();

    let mut block = meldritch_render::render_pattern_samples_with_event_gain(
        pattern,
        project.tempo(),
        range,
        project.probability_seed(),
        settings,
        &samples_by_note,
        |event| {
            if active_event_starts.contains(&event.range().start()) {
                options.active_scale
            } else {
                1.0
            }
        },
    );
    if options.normalize {
        block = block.normalized_to_peak(1.0);
    }

    let peak = block.peak_abs();
    let nonzero_samples = block
        .samples()
        .iter()
        .filter(|sample| **sample != 0.0)
        .count();
    let finite = block.samples().iter().all(|sample| sample.is_finite());

    println!(
        "rendered controlled samples: frames={}, channels={}, finite={}, nonzero_samples={}, peak={}, active_scale={}, active_events={}",
        block.frames(),
        block.channels(),
        finite,
        nonzero_samples,
        peak,
        options.active_scale,
        control.active_event_count
    );

    if let Some(output) = &options.output {
        meldritch_audio::write_wav_f32(output, &block, project.tempo().sample_rate())
            .map_err(|err| format!("failed to write {}: {err}", output.display()))?;
        println!("wrote: {}", output.display());
    }

    if let Some(manifest) = options.manifest {
        let summary = RenderManifest::from_render(RenderManifestInput {
            project_path: &path,
            output_path: options.output.as_deref(),
            pattern,
            range,
            channels: options.channels,
            normalize: options.normalize,
            cache_probe: false,
            cache_probe_summary: None,
            artifact_key,
            compiled: &compiled,
            samples_by_note: &samples_by_note,
            control: Some(RenderControlSummary::from_schedule(
                options.active_scale,
                &control,
            )),
            peak,
            nonzero_samples,
            finite,
        })?;
        let json = serde_json::to_string_pretty(&summary)
            .map_err(|err| format!("failed to encode render manifest: {err}"))?;
        std::fs::write(&manifest, json)
            .map_err(|err| format!("failed to write {}: {err}", manifest.display()))?;
        println!("wrote manifest: {}", manifest.display());
    }

    Ok(())
}

fn format_cache_status(status: CacheStatus) -> &'static str {
    match status {
        CacheStatus::Hit => "hit",
        CacheStatus::Miss => "miss",
    }
}

fn dirty_step(path: PathBuf, pattern_id: Option<u64>, step: u32, cycle: u64) -> Result<(), String> {
    let input = std::fs::read_to_string(&path)
        .map_err(|err| format!("failed to read {}: {err}", path.display()))?;
    let project = meldritch_dsl::parse_project(&input).map_err(|err| {
        err.diagnostics()
            .into_iter()
            .map(|diagnostic| diagnostic.message().to_owned())
            .collect::<Vec<_>>()
            .join("\n")
    })?;
    let pattern = select_pattern(&project, pattern_id, "inspect")?;
    let dirty = pattern
        .step_dirty_range(project.tempo(), StepIndex::new(step), cycle)
        .map_err(|err| err.to_string())?;

    println!(
        "dirty: entity={:?}, start={}, end={}",
        dirty.entity(),
        dirty.range().start(),
        dirty.range().end()
    );

    Ok(())
}

fn select_pattern<'a>(
    project: &'a meldritch_dsl::ValidatedProject,
    pattern_id: Option<u64>,
    action: &str,
) -> Result<&'a Pattern, String> {
    match pattern_id {
        Some(pattern_id) => project
            .patterns()
            .iter()
            .find(|pattern| pattern.id().raw() == pattern_id)
            .ok_or_else(|| format!("project has no pattern {pattern_id} to {action}")),
        None => project
            .patterns()
            .first()
            .ok_or_else(|| format!("project has no patterns to {action}")),
    }
}

fn load_project_samples(
    project: &meldritch_dsl::ValidatedProject,
    project_path: &Path,
) -> Result<BTreeMap<u8, meldritch_audio::SampleBuffer>, String> {
    let mut samples = BTreeMap::new();
    for sample_ref in project.samples() {
        let path = resolve_project_path(project_path, sample_ref.path());
        let sample = meldritch_audio::read_wav(&path)
            .map_err(|err| format!("failed to read sample {}: {err}", path.display()))?;
        samples.insert(sample_ref.note(), sample);
    }

    Ok(samples)
}

fn resolve_project_path(project_path: &Path, relative_or_absolute: &str) -> PathBuf {
    let path = PathBuf::from(relative_or_absolute);
    if path.is_absolute() {
        path
    } else {
        project_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn performer_actions_are_ranked_across_sessions_and_autopilot_is_not_learned() {
        let mut library = FutureLibrary::default();
        merge_performer_futures(
            &mut library,
            vec![
                CapturedFuture {
                    origin: "autopilot".to_owned(),
                    action: LearnedAction::IncreaseDrive,
                    frame: 10,
                    phase: 0.1,
                },
                CapturedFuture {
                    origin: "performer".to_owned(),
                    action: LearnedAction::IncreaseCutoff,
                    frame: 20,
                    phase: 0.2,
                },
            ],
            1,
        );
        merge_performer_futures(
            &mut library,
            vec![CapturedFuture {
                origin: "performer".to_owned(),
                action: LearnedAction::IncreaseCutoff,
                frame: 60,
                phase: 0.6,
            }],
            2,
        );

        assert_eq!(library.learned.len(), 1);
        let learned = &library.learned[0];
        assert_eq!(learned.action, LearnedAction::IncreaseCutoff);
        assert_eq!(learned.occurrences, 2);
        assert!((learned.mean_phase - 0.4).abs() < f64::EPSILON);
        assert_eq!(learned.score, 2_002);
        assert_eq!(learned.last_session, 2);
    }

    #[test]
    fn typed_future_library_round_trips_as_json() {
        let library = FutureLibrary {
            schema_version: 2,
            sessions: 3,
            learned: vec![LearnedFuture {
                action: LearnedAction::InvertChordUp,
                occurrences: 2,
                last_session: 3,
                mean_phase: 0.5,
                score: 2_003,
            }],
            last_session: Vec::new(),
        };
        let json = serde_json::to_string(&library).unwrap();
        let decoded: FutureLibrary = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.learned[0].action, LearnedAction::InvertChordUp);
        assert_eq!(decoded.learned[0].score, 2_003);
    }

    #[test]
    fn warehouse_futures_preserve_exact_phrase_identity() {
        let mut library = FutureLibrary::default();
        merge_performer_futures(
            &mut library,
            vec![
                CapturedFuture {
                    origin: "performer".to_owned(),
                    action: LearnedAction::QueuePhrase(3),
                    frame: 100,
                    phase: 0.7,
                },
                CapturedFuture {
                    origin: "performer".to_owned(),
                    action: LearnedAction::QueuePhrase(4),
                    frame: 120,
                    phase: 0.8,
                },
            ],
            1,
        );
        let json = serde_json::to_string(&library).unwrap();
        let decoded: FutureLibrary = serde_json::from_str(&json).unwrap();
        assert!(
            decoded
                .learned
                .iter()
                .any(|future| future.action == LearnedAction::QueuePhrase(3))
        );
        assert!(
            decoded
                .learned
                .iter()
                .any(|future| future.action == LearnedAction::QueuePhrase(4))
        );
        assert_eq!(
            LearnedAction::QueuePhrase(3).input(),
            Some(meldritch_app::AppInput::QueuePhrase(SceneId::new(3)))
        );
    }

    #[test]
    fn learned_phrase_schedule_orders_cues_by_musical_phase() {
        let library = FutureLibrary {
            schema_version: 2,
            sessions: 4,
            learned: vec![
                LearnedFuture {
                    action: LearnedAction::QueuePhrase(4),
                    occurrences: 3,
                    last_session: 4,
                    mean_phase: 0.75,
                    score: 3_004,
                },
                LearnedFuture {
                    action: LearnedAction::IncreaseDrive,
                    occurrences: 9,
                    last_session: 4,
                    mean_phase: 0.1,
                    score: 9_004,
                },
                LearnedFuture {
                    action: LearnedAction::QueuePhrase(2),
                    occurrences: 2,
                    last_session: 3,
                    mean_phase: 0.25,
                    score: 2_003,
                },
            ],
            last_session: Vec::new(),
        };

        assert_eq!(
            learned_phrase_schedule(&library, 1_001, 8),
            vec![(250, SceneId::new(2)), (750, SceneId::new(4))]
        );
        assert!(learned_phrase_schedule(&library, 0, 8).is_empty());
        assert!(learned_phrase_schedule(&library, 1_001, 0).is_empty());
    }

    #[test]
    fn performer_override_grace_suppresses_then_releases_learned_cues() {
        let mut grace = PerformerOverrideGrace::default();
        assert!(!grace.suppresses(100));
        grace.record(100, 400, 1_000);
        assert!(grace.suppresses(100));
        assert!(grace.suppresses(500));
        assert!(!grace.suppresses(501));
        grace.record(900, 400, 1_000);
        assert!(grace.suppresses(999));
        grace.reset();
        assert!(!grace.suppresses(0));
    }

    #[test]
    fn performance_session_capture_checkpoints_parseable_timestamped_toml() {
        let song = meldritch_dsl::load_song_directory(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../..")
                .join("songs/examples/11-session-capture"),
        )
        .expect("session capture example should load");
        let root = std::env::temp_dir().join(format!(
            "meldritch-session-capture-test-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();

        let mut capture =
            PerformanceSessionCapture::create_at(&root, &song, 96_000, "20260712T123456Z")
                .expect("session should be created");
        capture.push_event(test_session_event(7)).unwrap();
        capture.final_state = Some(FinalSessionState {
            cockpit_mode: "Performance".to_owned(),
            transport_state: "Playing".to_owned(),
            transport_position: 12_000,
            selection_track: 1,
            selection_step: 0,
            history_len: 1,
            curated_controls: vec![FinalSessionControl {
                id: "echo-feedback".to_owned(),
                target: "dsp:echo/delay.feedback".to_owned(),
                value: Some(0.4),
            }],
        });
        capture.finish_clean_without_controller().unwrap();
        let second = PerformanceSessionCapture::create_at(&root, &song, 96_000, "20260712T123456Z")
            .expect("second session should get a collision suffix");

        assert_eq!(
            capture.path().file_name().and_then(std::ffi::OsStr::to_str),
            Some("session-20260712T123456Z.mlperformance")
        );
        assert_eq!(
            second.path().file_name().and_then(std::ffi::OsStr::to_str),
            Some("session-20260712T123456Z-001.mlperformance")
        );

        let raw = std::fs::read_to_string(capture.path()).unwrap();
        let value: toml::Value = toml::from_str(&raw).unwrap();
        assert_eq!(
            value["meldritch"]["kind"].as_str(),
            Some("performance_session")
        );
        assert_eq!(
            value["session"]["source_performance"].as_str(),
            Some("session-capture")
        );
        assert_eq!(
            value["session"]["source_fingerprint"].as_str(),
            Some(format!("{:016x}", song.fingerprint()).as_str())
        );
        assert_eq!(value["session"]["clean_termination"].as_bool(), Some(true));
        let events = value["events"].as_array().unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0]["sequence"].as_integer(), Some(7));
        assert_eq!(events[0]["target_id"].as_str(), Some("echo-feedback"));
        assert_eq!(events[0]["previous"].as_str(), Some("0.350000000000"));
        assert_eq!(events[0]["current"].as_str(), Some("0.400000000000"));
        assert_eq!(
            value["final_state"]["cockpit_mode"].as_str(),
            Some("Performance")
        );
        assert_eq!(
            value["final_controls"][0]["id"].as_str(),
            Some("echo-feedback")
        );
        assert_eq!(value["final_controls"][0]["value"].as_float(), Some(0.4));

        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn session_capture_buffers_events_until_the_configured_checkpoint_limit() {
        let song = meldritch_dsl::load_song_directory(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../..")
                .join("songs/examples/11-session-capture"),
        )
        .expect("session capture example should load");
        let root = std::env::temp_dir().join(format!(
            "meldritch-session-buffer-test-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let mut capture = PerformanceSessionCapture::create_at_with_buffer_limit(
            &root,
            &song,
            96_000,
            "20260712T223344Z",
            2,
        )
        .expect("session should be created");

        capture.push_event(test_session_event(1)).unwrap();
        let raw = std::fs::read_to_string(capture.path()).unwrap();
        let value: toml::Value = toml::from_str(&raw).unwrap();
        assert!(value.get("events").is_none());
        assert_eq!(capture.uncheckpointed_events, 1);

        capture.push_event(test_session_event(2)).unwrap();
        let raw = std::fs::read_to_string(capture.path()).unwrap();
        let value: toml::Value = toml::from_str(&raw).unwrap();
        assert_eq!(value["events"].as_array().unwrap().len(), 2);
        assert_eq!(capture.uncheckpointed_events, 0);

        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn session_capture_classifies_accepted_command_categories() {
        assert_session_kind(
            &meldritch_app::AppInput::TogglePlayback,
            &meldritch_app::AppCommandResult::TransportQueued,
            "transport",
            Some("toggle_playback"),
        );
        assert_session_kind(
            &meldritch_app::AppInput::Stop,
            &meldritch_app::AppCommandResult::TransportQueued,
            "transport",
            Some("stop"),
        );
        assert_session_kind(
            &meldritch_app::AppInput::Rewind,
            &meldritch_app::AppCommandResult::TransportQueued,
            "transport",
            Some("rewind"),
        );
        assert_session_kind(
            &meldritch_app::AppInput::ToggleCockpitMode,
            &meldritch_app::AppCommandResult::CockpitModeChanged {
                previous: meldritch_app::CockpitMode::Performance,
                current: meldritch_app::CockpitMode::AllParameters,
            },
            "cockpit_mode",
            None,
        );
        assert_session_kind(
            &meldritch_app::AppInput::MoveRight,
            &meldritch_app::AppCommandResult::SelectionChanged {
                previous: meldritch_app::Selection {
                    track: TrackId::new(1),
                    step: StepIndex::new(0),
                },
                current: meldritch_app::Selection {
                    track: TrackId::new(1),
                    step: StepIndex::new(1),
                },
            },
            "selection",
            None,
        );
        assert_session_kind(
            &meldritch_app::AppInput::AdjustCuratedControl {
                id: "echo-feedback".to_owned(),
                steps: 1,
            },
            &meldritch_app::AppCommandResult::CuratedControlAdjusted {
                id: "echo-feedback".to_owned(),
                previous: 0.35,
                current: 0.4,
            },
            "curated_control",
            Some("echo-feedback"),
        );
        assert_session_kind(
            &meldritch_app::AppInput::ReturnToLive,
            &meldritch_app::AppCommandResult::AudioSourceSwitched { transformed: false },
            "audio_source",
            None,
        );
        assert_session_kind(
            &meldritch_app::AppInput::ToggleSelected(Step::new(36)),
            &meldritch_app::AppCommandResult::Edit(meldritch_render::live_edit::LiveEditResult {
                command: meldritch_render::live_edit::LiveEditCommand::ToggleStep {
                    track: TrackId::new(1),
                    step: StepIndex::new(0),
                    value: Step::new(36),
                },
                changed: true,
                dirty_ranges: Vec::new(),
                invalidated_chunks: 0,
            }),
            "parameter_edit",
            Some(
                "ToggleSelected(Step { note: 36, velocity: 1.0, gate: 1.0, probability: Probability(1.0), tags: {} })",
            ),
        );
        assert_session_kind(
            &meldritch_app::AppInput::IncreaseCutoff,
            &meldritch_app::AppCommandResult::SynthUpdated {
                invalidated_chunks: 1,
            },
            "synth_control",
            Some("IncreaseCutoff"),
        );
        assert_session_kind(
            &meldritch_app::AppInput::IncreaseDelayFeedback,
            &meldritch_app::AppCommandResult::PerformanceFxUpdated(
                meldritch_render::performance_fx::PerformanceFxSettings::default(),
            ),
            "performance_fx",
            Some("IncreaseDelayFeedback"),
        );
        assert_session_kind(
            &meldritch_app::AppInput::CreateReverse,
            &meldritch_app::AppCommandResult::TransformCreated {
                key: meldritch_render::transforms::TransformArtifactKey {
                    fingerprint: meldritch_render::Fingerprint::new(1),
                    channels: 1,
                    frames: 1,
                },
                status: meldritch_render::transforms::TransformCacheStatus::Miss,
            },
            "transform",
            Some("CreateReverse"),
        );
        assert_session_kind(
            &meldritch_app::AppInput::QueuePhrase(SceneId::new(2)),
            &meldritch_app::AppCommandResult::PerformanceQueued(
                meldritch_render::futures::QueuedPerformanceGesture {
                    gesture: meldritch_app::PerformanceGesture::QueueScene(SceneId::new(2)),
                    launch_frame: 0,
                    fill_end_frame: None,
                },
            ),
            "performance_queue",
            Some("QueuePhrase(SceneId(2))"),
        );
        assert_session_kind(
            &meldritch_app::AppInput::CancelPerformance,
            &meldritch_app::AppCommandResult::PerformanceCancelled(None),
            "performance_cancel",
            Some("CancelPerformance"),
        );
    }

    #[test]
    fn midi_cc_messages_decode_without_controller_specific_mapping() {
        assert_eq!(decode_midi_cc_message(&[0xB0, 77, 64]), Some((1, 77, 64)));
        assert_eq!(decode_midi_cc_message(&[0xB3, 84, 127]), Some((4, 84, 127)));
        assert_eq!(decode_midi_cc_message(&[0x90, 77, 127]), None);
    }

    #[test]
    fn midi_note_messages_decode_for_hardware_discovery() {
        assert_eq!(
            decode_midi_note_message(&[0x90, 60, 100]),
            Some((1, 60, 100, true))
        );
        assert_eq!(
            decode_midi_note_message(&[0x90, 60, 0]),
            Some((1, 60, 0, false))
        );
        assert_eq!(
            decode_midi_note_message(&[0x82, 61, 64]),
            Some((3, 61, 64, false))
        );
        assert_eq!(decode_midi_note_message(&[0xB0, 77, 64]), None);
    }

    #[test]
    fn midi_control_bindings_are_derived_from_song_scripts() {
        let song = meldritch_dsl::load_song_directory(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../..")
                .join("songs/examples/15-launch-control-xl-input"),
        )
        .expect("LaunchControl XL example should load");

        let bindings = midi_control_bindings_for_song(&song);
        assert!(bindings.iter().any(|binding| {
            binding.control_id == "echo-feedback"
                && binding.device == "launch-control-xl"
                && binding.channel == Some(1)
                && binding.cc == 77
                && binding.action == meldritch_app::MidiControlAction::Absolute
        }));
    }

    #[test]
    fn midi_action_bindings_are_derived_from_song_scripts() {
        let song = meldritch_dsl::load_song_directory(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../..")
                .join("songs/examples/16-launch-control-xl-playground"),
        )
        .expect("LaunchControl XL playground example should load");

        let control_bindings = midi_control_bindings_for_song(&song);
        let action_bindings = midi_action_bindings_for_song(&song);
        assert_eq!(
            map_script_midi_control_change(
                &control_bindings,
                &action_bindings,
                meldritch_app::MidiControlInput {
                    device: "launch-control-xl".to_owned(),
                    channel: 1,
                    cc: 41,
                    value: 127,
                },
            )
            .map(|mapped| mapped.input),
            Some(meldritch_app::AppInput::TogglePlayback)
        );
        assert_eq!(
            map_script_midi_control_change(
                &control_bindings,
                &action_bindings,
                meldritch_app::MidiControlInput {
                    device: "launch-control-xl".to_owned(),
                    channel: 1,
                    cc: 41,
                    value: 0,
                },
            ),
            None
        );
        let side_note = map_script_midi_note(&action_bindings, "launch-control-xl", 9, 108, true)
            .expect("side button should map");
        assert_eq!(side_note.input, meldritch_app::AppInput::ToggleCockpitMode);
        assert_eq!(side_note.label.as_deref(), Some("Record Arm Toggle Mode"));
        assert_eq!(
            map_script_midi_note(&action_bindings, "launch-control-xl", 9, 108, false),
            None
        );
    }

    fn assert_session_kind(
        input: &meldritch_app::AppInput,
        result: &meldritch_app::AppCommandResult,
        expected_kind: &str,
        expected_target: Option<&str>,
    ) {
        let (kind, target, _, _) = session_result_fields(input, result);
        assert_eq!(kind, expected_kind);
        assert_eq!(target.as_deref(), expected_target);
    }

    fn test_session_event(sequence: u64) -> CapturedSessionEvent {
        CapturedSessionEvent {
            sequence,
            wall_offset_ms: 42,
            absolute_frame: 12_000,
            musical_beat: 0.5,
            requested_quantization: "immediate".to_owned(),
            actual_frame: 12_000,
            provenance: "performer".to_owned(),
            input: "AdjustCuratedControl { id: \"echo-feedback\", steps: 1 }".to_owned(),
            command: "AdjustCuratedControl { id: \"echo-feedback\", steps: 1 }".to_owned(),
            result: "CuratedControlAdjusted".to_owned(),
            changed: true,
            kind: "curated_control".to_owned(),
            target_id: Some("echo-feedback".to_owned()),
            previous: Some("0.350000000000".to_owned()),
            current: Some("0.400000000000".to_owned()),
        }
    }

    #[test]
    fn session_timestamp_formatter_uses_utc_calendar_dates() {
        assert_eq!(format_unix_timestamp_utc(0), "19700101T000000Z");
        assert_eq!(format_unix_timestamp_utc(86_400), "19700102T000000Z");
        assert_eq!(format_unix_timestamp_utc(951_782_400), "20000229T000000Z");
    }

    #[test]
    fn warehouse_showcase_rejects_invalid_reuse_before_audio_setup() {
        assert_eq!(
            warehouse_showcase(
                PathBuf::from("missing-project.toml"),
                PathBuf::from("/tmp/meldritch-definitely-missing.wav"),
                PathBuf::from("/tmp/meldritch-definitely-missing.json"),
                0,
                true,
                false,
                Some(1),
            ),
            Err("warehouse loop count must be at least one".to_owned())
        );
        assert!(
            warehouse_showcase(
                PathBuf::from("missing-project.toml"),
                PathBuf::from("/tmp/meldritch-definitely-missing.wav"),
                PathBuf::from("/tmp/meldritch-definitely-missing.json"),
                1,
                true,
                false,
                Some(1),
            )
            .unwrap_err()
            .contains("cannot reuse missing warehouse render")
        );
    }

    #[test]
    fn warehouse_soak_rejects_zero_duration_before_audio_setup() {
        assert_eq!(
            warehouse_soak(PathBuf::from("missing.wav"), 0, true),
            Err("warehouse soak duration must be at least one second".to_owned())
        );
    }
}
