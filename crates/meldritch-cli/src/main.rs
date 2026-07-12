use clap::{Parser, Subcommand, ValueEnum};
use meldritch_core::{
    Arrangement, ArrangementSection, AutomationInterpolation, AutomationLane, AutomationPoint,
    AutomationTarget, AutomationValue, DirtyRange, EntityId, Event, EventTag, FrameRange, Pattern,
    SceneId, SourceId, Step, StepIndex, TrackId,
};
use meldritch_render::coordinator::{RenderCoordinator, RenderCoordinatorConfig};
use meldritch_render::{ArtifactCache, CacheStatus, RenderSettings};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

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
    ToggleTrackMute,
    TriggerFill,
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
            AppInput::ToggleTrackMute => Self::ToggleTrackMute,
            AppInput::TriggerFill => Self::TriggerFill,
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
            Self::ToggleTrackMute => AppInput::ToggleTrackMute,
            Self::TriggerFill => AppInput::TriggerFill,
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
            Some(1_536_000),
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
        controller
            .configure_phrase_scenes(
                project
                    .patterns()
                    .iter()
                    .enumerate()
                    .map(|(index, pattern)| (SceneId::new(index as u64 + 1), pattern.clone())),
            )
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
    controller.show_learned_phrase_cues(
        learned_phrase_cues
            .iter()
            .map(|(frame, scene)| meldritch_app::LearnedPhraseCueView {
                scene: *scene,
                frame: u64::from(*frame),
            })
            .collect(),
    );
    for learned in ranked.into_iter().take(4) {
        match learned.action {
            LearnedAction::QueuePhrase(_) => continue,
            action => {
                controller
                    .handle_input(action.input().expect("non-phrase action has an input"))
                    .map_err(|err| format!("failed to prepare learned future: {err:?}"))?;
            }
        }
    }
    let wanted_ready = warm_chunks.min(frame_count.div_ceil(chunk_frames) as usize);
    if !controller
        .coordinator()
        .wait_for_ready_chunks(wanted_ready.max(1), Duration::from_secs(10))
    {
        return Err("live showcase could not prepare its initial audio horizon".to_owned());
    }
    controller
        .dispatch(meldritch_app::AppCommand::Play)
        .map_err(|err| format!("failed to start showcase transport: {err:?}"))?;
    let captured = Arc::new(std::sync::Mutex::new(Vec::<CapturedFuture>::new()));
    let performer_capture = Arc::clone(&captured);
    let mut fired = vec![false; cue_frames.len()];
    let mut phrase_fired = vec![false; learned_phrase_cues.len()];
    let mut previous_position = 0;
    meldritch_tui::run_with_hooks(
        &mut controller,
        Step::new(note),
        move |controller| {
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
            }
            previous_position = position;
            if warehouse && controller.view_model().performance.queued.is_none() {
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
            for (index, (frame, description)) in cue_frames.iter().enumerate() {
                if !fired[index] && position >= *frame {
                    fired[index] = true;
                    return Ok(Some(format!("Autopilot: {description}")));
                }
            }
            Ok(None)
        },
        move |controller, input| {
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
}
