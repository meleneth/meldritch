use clap::{Parser, Subcommand, ValueEnum};
use meldritch_audio::published_audio::PublishedAudio;
use meldritch_control::session::*;
use meldritch_control::song_host::*;
use meldritch_core::{
    Arrangement, ArrangementSection, AutomationInterpolation, AutomationLane, AutomationPoint,
    AutomationTarget, AutomationValue, DirtyRange, EntityId, Event, EventTag, FrameRange, Pattern,
    PatternId, ProbabilitySeed, SceneId, SourceId, Step, StepIndex, Tempo, TrackId,
};
use meldritch_render::coordinator::{RenderCoordinator, RenderCoordinatorConfig};
use meldritch_render::song_host::*;
use meldritch_render::{ArtifactCache, CacheStatus, RenderSettings};
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::{Duration, Instant};

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
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
    SelectPerformancePage(String),
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
            AppInput::SelectPerformancePage(page_id) => {
                Self::SelectPerformancePage(page_id.clone())
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

    fn input(self) -> Option<meldritch_app::AppInput> {
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
            Self::SelectPerformancePage(page_id) => AppInput::SelectPerformancePage(page_id),
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
        .map(|future| (future.action.clone(), future))
        .collect::<BTreeMap<_, _>>();
    for event in captured.iter().filter(|event| event.origin == "performer") {
        let future = learned
            .entry(event.action.clone())
            .or_insert(LearnedFuture {
                action: event.action.clone(),
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
        .filter_map(|future| match &future.action {
            LearnedAction::QueuePhrase(scene) => Some((
                (future.mean_phase.clamp(0.0, 1.0) * f64::from(last_frame)).round() as u32,
                SceneId::new(*scene),
                future.score,
            )),
            LearnedAction::QueuePhraseVariation(scene, _) => Some((
                (future.mean_phase.clamp(0.0, 1.0) * f64::from(last_frame)).round() as u32,
                SceneId::new(*scene),
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

fn is_dsp_action(action: &LearnedAction) -> bool {
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
        .filter(|future| is_dsp_action(&future.action))
        .map(|future| {
            (
                (future.mean_phase.clamp(0.0, 1.0) * f64::from(last)).round() as u32,
                future.action.clone(),
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
    /// Run a bounded Lua-authored APC Mini MK2 LED animation (no audio).
    ApcLuaColors {
        #[arg(value_name = "LUA_FILE")]
        source: PathBuf,
        /// Validate and print MIDI messages without opening hardware.
        #[arg(long)]
        dry_run: bool,
        /// Output-port name substring; the default selects the Session Control port.
        #[arg(long, default_value = "APC mini mk2 Control")]
        port: String,
        /// Turn off pads/buttons after the animation instead of keeping its last frame.
        #[arg(long)]
        clear: bool,
    },
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
        /// Explicitly enable script-defined MIDI control input. This is the default.
        #[arg(long = "midi-controls", action = clap::ArgAction::SetTrue)]
        midi_controls: bool,
        /// Disable script-defined MIDI control input.
        #[arg(long = "no-midi-controls", action = clap::ArgAction::SetTrue)]
        no_midi_controls: bool,
        /// Start playback immediately after opening the TUI.
        #[arg(long, conflicts_with = "no_autoplay")]
        autoplay: bool,
        /// Open the TUI with playback stopped until `p` is pressed.
        #[arg(long, conflicts_with = "autoplay")]
        no_autoplay: bool,
        /// Show raw/unmapped MIDI input in the TUI status line while testing hardware.
        #[arg(long)]
        midi_debug: bool,
        /// Show live song-publication/device read diagnostics in the TUI status line.
        #[arg(long)]
        audio_debug: bool,
        /// Confined Lua control composition, evaluated only on the TUI thread.
        #[arg(long, value_name = "PROJECT_RELATIVE_LUA")]
        lua: Option<PathBuf>,
        /// Require physical pickup before absolute MIDI controls can edit values.
        /// Enabled automatically by --lua; legacy TOML defaults remain direct.
        #[arg(long)]
        soft_takeover: bool,
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
        Command::ApcLuaColors {
            source,
            dry_run,
            port,
            clear,
        } => apc_lua_colors(source, dry_run, port, clear),
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
            no_midi_controls,
            autoplay,
            no_autoplay,
            midi_debug,
            audio_debug,
            lua,
            soft_takeover,
        } => tui_song(
            song,
            frames,
            chunk_frames,
            workers,
            midi_controls || !no_midi_controls,
            autoplay || !no_autoplay,
            midi_debug,
            effective_song_audio_debug(midi_debug, audio_debug),
            lua,
            soft_takeover,
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
        "valid song '{}' · {} track(s) · {} synth(s) · {} sample bank(s) · {} DSP graph(s) · {} note pattern(s) · {} parameter pattern(s) · fingerprint {:016x}",
        song.performance().title(),
        song.performance().tracks().len(),
        song.synths().len(),
        song.sample_banks().len(),
        song.dsps().len(),
        song.note_patterns().len(),
        song.parameter_patterns().len(),
        song.fingerprint()
    );
    Ok(())
}

struct MidiControlInputConnection {
    _connection: midir::MidiInputConnection<()>,
    port_id: String,
    port_name: String,
    route: Option<meldritch_control::devices::MidiInputRoute>,
}
impl Drop for MidiControlInputConnection {
    fn drop(&mut self) {
        if let Some(route) = &self.route {
            route.invalidate();
        }
    }
}

struct MidiFeedbackConnection {
    connection: midir::MidiOutputConnection,
    port_id: String,
    port_name: String,
}

fn midi_input_present(connection: &MidiControlInputConnection) -> Result<bool, String> {
    let midi = midir::MidiInput::new("meldritch-input-probe").map_err(|e| e.to_string())?;
    Ok(midi.ports().iter().any(|port| {
        port.id() == connection.port_id
            && midi
                .port_name(port)
                .is_ok_and(|name| name == connection.port_name)
    }))
}

fn midi_output_present(connection: &MidiFeedbackConnection) -> Result<bool, String> {
    let midi = midir::MidiOutput::new("meldritch-output-probe").map_err(|e| e.to_string())?;
    Ok(midi.ports().iter().any(|port| {
        port.id() == connection.port_id
            && midi
                .port_name(port)
                .is_ok_and(|name| name == connection.port_name)
    }))
}

#[derive(Clone, Debug, PartialEq)]
struct MidiActionBinding {
    device: String,
    message: MidiActionMessage,
    input: meldritch_app::AppInput,
    label: String,
}

#[derive(Clone, Debug, PartialEq)]
struct MidiModifierBinding {
    device: String,
    message: MidiActionMessage,
    modifier_id: String,
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

fn apc_lua_colors(
    source: PathBuf,
    dry_run: bool,
    port_name: String,
    clear: bool,
) -> Result<(), String> {
    use meldritch_control::led_demo::ApcLedDemo;
    // Read a bounded source before allocating/evaluating a project VM.
    let file = std::fs::File::open(&source).map_err(|e| format!("{}: {e}", source.display()))?;
    let mut text = String::new();
    std::io::Read::read_to_string(&mut std::io::Read::take(file, 256 * 1024 + 1), &mut text)
        .map_err(|e| e.to_string())?;
    let plan = ApcLedDemo::load(&text)?;
    if dry_run {
        for (index, frame) in plan.frames.iter().enumerate() {
            println!(
                "frame {index}, hold {}ms: {:?}",
                frame.hold_ms,
                frame.messages()
            );
        }
        println!(
            "validated {} frames, {}ms; clear={clear}",
            plan.frames.len(),
            plan.duration_ms()
        );
        return Ok(());
    }
    if port_name.trim().is_empty() {
        return Err("MIDI output port match cannot be empty".into());
    }
    let midi = midir::MidiOutput::new("meldritch-apc-lua-colors").map_err(|e| e.to_string())?;
    let matches = midi
        .ports()
        .into_iter()
        .filter_map(|port| {
            let name = midi.port_name(&port).ok()?;
            midi_port_name_matches(&name, &port_name).then_some((port, name))
        })
        .collect::<Vec<_>>();
    if matches.len() != 1 {
        return Err(format!(
            "expected one MIDI output matching '{port_name}', found {}; use --port to choose the Control port",
            matches.len()
        ));
    }
    let (port, name) = matches.into_iter().next().expect("one match checked");
    let mut connection = midi
        .connect(&port, "meldritch-apc-leds")
        .map_err(|e| e.to_string())?;
    println!(
        "APC LED output: {name}; {} validated frames",
        plan.frames.len()
    );
    for frame in &plan.frames {
        for message in frame.messages() {
            connection.send(&message).map_err(|e| e.to_string())?;
        }
        thread::sleep(Duration::from_millis(frame.hold_ms));
    }
    if clear {
        for message in ApcLedDemo::clear_messages() {
            connection.send(&message).map_err(|e| e.to_string())?;
        }
    }
    println!(
        "sent LED plan; {}",
        if clear {
            "LEDs cleared"
        } else {
            "final presentation retained"
        }
    );
    Ok(())
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
    sender: mpsc::Sender<meldritch_tui::ExternalInputEvent>,
    bindings: Vec<meldritch_app::MidiControlBinding>,
    modifier_control_bindings: Vec<meldritch_app::MidiModifierControlBinding>,
    modifier_bindings: Vec<MidiModifierBinding>,
    action_bindings: Vec<MidiActionBinding>,
    active_page: Arc<Mutex<Option<String>>>,
    active_modifiers: Arc<Mutex<Vec<String>>>,
    debug: bool,
    lua_route: Option<meldritch_control::devices::ApcInputRoute>,
    control_route: Option<meldritch_control::devices::MidiInputRoute>,
) -> Result<Option<MidiControlInputConnection>, String> {
    let mut midi_input = midir::MidiInput::new(&format!("meldritch-midi-{}", device.id()))
        .map_err(|error| midi_input_client_error("failed to create MIDI input client", &error))?;
    midi_input.ignore(midir::Ignore::None);
    let ports = midi_input
        .ports()
        .into_iter()
        .filter_map(|port| {
            let name = midi_input.port_name(&port).ok()?;
            midi_port_name_matches(&name, device.name_contains()).then_some((port, name))
        })
        .collect::<Vec<_>>();
    if ports.is_empty() {
        return Ok(None);
    }
    if ports.len() != 1 {
        return Err(format!(
            "MIDI input '{}' has ambiguous matching ports",
            device.id()
        ));
    }
    let (port, name) = ports.into_iter().next().unwrap();
    let port_id = port.id();
    let route = control_route.clone();
    let device_id = device.id().to_owned();
    let connection = midi_input
        .connect(
            &port,
            &format!("meldritch-midi-{}-input", device.id()),
            move |_timestamp, message, _state| {
                if control_route
                    .as_ref()
                    .is_some_and(|route| route.observe(message))
                {
                    return;
                }
                if lua_route
                    .as_ref()
                    .is_some_and(|route| route.observe(message))
                {
                    return;
                }
                let page = active_page
                    .lock()
                    .expect("active page lock poisoned")
                    .clone();
                let modifiers = active_modifiers
                    .lock()
                    .expect("active modifier lock poisoned")
                    .clone();
                let input = decode_script_midi_input(
                    message,
                    &device_id,
                    &bindings,
                    &modifier_control_bindings,
                    &modifier_bindings,
                    &action_bindings,
                    page.as_deref(),
                    &modifiers,
                );
                let Some(input) = input else {
                    if debug {
                        let diagnostic = decode_midi_diagnostic_message(message).map_or_else(
                            || format!("raw {message:?}"),
                            |message| message.to_string(),
                        );
                        let _ = sender.send(meldritch_tui::ExternalInputEvent::status(format!(
                            "{diagnostic} -> unmapped"
                        )));
                    }
                    return;
                };
                if let meldritch_app::AppInput::SetPerformanceModifier {
                    modifier_id,
                    active,
                } = &input.input
                {
                    let mut modifiers = active_modifiers
                        .lock()
                        .expect("active modifier lock poisoned");
                    if *active {
                        if !modifiers.iter().any(|candidate| candidate == modifier_id) {
                            modifiers.push(modifier_id.clone());
                        }
                    } else {
                        modifiers.retain(|candidate| candidate != modifier_id);
                    }
                }
                let label = input
                    .label
                    .clone()
                    .unwrap_or_else(|| format!("{:?}", input.input));
                let _ = sender.send(meldritch_tui::ExternalInputEvent::new(
                    input.input,
                    Some(label),
                ));
            },
            (),
        )
        .map_err(|error| format!("failed to open MIDI input '{name}': {error}"))?;
    eprintln!("MIDI control input '{}' connected: {name}", device.id());
    Ok(Some(MidiControlInputConnection {
        _connection: connection,
        port_id,
        port_name: name,
        route,
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
                            None,
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
        port_id: port.id(),
        port_name,
        route: None,
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
                    page,
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
                    meldritch_dsl::ControlBindingAction::Centered { center } => {
                        meldritch_app::MidiControlAction::Centered { center: *center }
                    }
                    meldritch_dsl::ControlBindingAction::Overdrive {
                        normal,
                        normal_midi,
                    } => meldritch_app::MidiControlAction::Overdrive {
                        normal: *normal,
                        normal_midi: *normal_midi,
                    },
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
                    page_id: page.clone(),
                    channel: *channel,
                    cc: *cc,
                    minimum: control.range().0,
                    maximum: control.range().1,
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
                        page: _,
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

fn midi_modifier_bindings_for_song(
    song: &meldritch_dsl::ValidatedSong,
) -> Vec<MidiModifierBinding> {
    song.performance()
        .modifiers()
        .iter()
        .flat_map(|modifier| {
            modifier
                .bindings()
                .iter()
                .filter_map(move |binding| match binding {
                    meldritch_dsl::ControlBindingDefinition::MidiCc {
                        device,
                        page: _,
                        channel,
                        cc,
                        ..
                    } => Some(MidiModifierBinding {
                        device: device.clone(),
                        message: MidiActionMessage::ControlChange {
                            channel: *channel,
                            cc: *cc,
                        },
                        modifier_id: modifier.id().to_owned(),
                        label: modifier.label().to_owned(),
                    }),
                    meldritch_dsl::ControlBindingDefinition::MidiNote {
                        device,
                        channel,
                        note,
                    } => Some(MidiModifierBinding {
                        device: device.clone(),
                        message: MidiActionMessage::Note {
                            channel: *channel,
                            note: *note,
                        },
                        modifier_id: modifier.id().to_owned(),
                        label: modifier.label().to_owned(),
                    }),
                    meldritch_dsl::ControlBindingDefinition::Key { .. } => None,
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

fn midi_modifier_bindings_for_device(
    bindings: &[MidiModifierBinding],
    device: &str,
) -> Vec<MidiModifierBinding> {
    bindings
        .iter()
        .filter(|binding| binding.device == device)
        .cloned()
        .collect()
}

fn midi_modifier_control_bindings_for_song(
    song: &meldritch_dsl::ValidatedSong,
) -> Vec<meldritch_app::MidiModifierControlBinding> {
    song.performance()
        .modifier_controls()
        .iter()
        .flat_map(|control| {
            control.bindings().iter().filter_map(|binding| {
                let meldritch_dsl::ControlBindingDefinition::MidiCc {
                    device,
                    page,
                    channel,
                    cc,
                    ..
                } = binding
                else {
                    return None;
                };
                let action = match control.action() {
                    meldritch_dsl::ModifierControlActionDefinition::SetLaneOctave {
                        lane,
                        minimum,
                        maximum,
                    } => meldritch_app::MidiModifierControlAction::SetLaneOctave {
                        lane_id: lane.clone(),
                        minimum: *minimum,
                        maximum: *maximum,
                    },
                };
                Some(meldritch_app::MidiModifierControlBinding {
                    modifier_id: control.modifier().to_owned(),
                    device: device.clone(),
                    page_id: page.clone(),
                    channel: *channel,
                    cc: *cc,
                    action,
                })
            })
        })
        .collect()
}

fn midi_modifier_control_bindings_for_device(
    bindings: &[meldritch_app::MidiModifierControlBinding],
    device: &str,
) -> Vec<meldritch_app::MidiModifierControlBinding> {
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
        meldritch_dsl::PerformanceActionDefinition::SelectPage { page } => {
            meldritch_app::AppInput::SelectPerformancePage(page.clone())
        }
        meldritch_dsl::PerformanceActionDefinition::SelectLaneVariation { lane, variation } => {
            meldritch_app::AppInput::SelectLaneVariation {
                lane_id: lane.clone(),
                variation_id: variation.clone(),
            }
        }
        meldritch_dsl::PerformanceActionDefinition::SelectLanePatternBank { lane, bank } => {
            meldritch_app::AppInput::SelectLanePatternBank {
                lane_id: lane.clone(),
                bank_id: bank.clone(),
            }
        }
        meldritch_dsl::PerformanceActionDefinition::ToggleLaneMute { lane } => {
            meldritch_app::AppInput::ToggleLaneMute(lane.clone())
        }
        meldritch_dsl::PerformanceActionDefinition::ToggleLaneSolo { lane } => {
            meldritch_app::AppInput::ToggleLaneSolo(lane.clone())
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

struct ScriptMidiMapper {
    takeover: bool,
    controls: Vec<meldritch_app::MidiControlBinding>,
    modifier_controls: Vec<meldritch_app::MidiModifierControlBinding>,
    modifiers: Vec<MidiModifierBinding>,
    actions: Vec<MidiActionBinding>,
}

enum MidiMappedOutcome {
    Applied(MappedMidiInput, meldritch_app::AppCommandResult),
    Waiting(String),
    Unmapped,
}

fn disconnected_modifier_inputs(
    device: &str,
    bindings: &[MidiModifierBinding],
    view: &meldritch_app::AppViewModel,
) -> Vec<meldritch_app::AppInput> {
    bindings
        .iter()
        .filter(|binding| {
            binding.device == device
                && view
                    .performance
                    .active_modifiers
                    .iter()
                    .any(|modifier| modifier.id == binding.modifier_id && modifier.active)
        })
        .map(|binding| binding.modifier_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .map(
            |modifier_id| meldritch_app::AppInput::SetPerformanceModifier {
                modifier_id,
                active: false,
            },
        )
        .collect()
}

impl ScriptMidiMapper {
    fn for_song(song: &meldritch_dsl::ValidatedSong) -> Self {
        Self {
            takeover: true,
            controls: midi_control_bindings_for_song(song),
            modifier_controls: midi_modifier_control_bindings_for_song(song),
            modifiers: midi_modifier_bindings_for_song(song),
            actions: midi_action_bindings_for_song(song),
        }
    }

    fn apply(
        &self,
        observed: &meldritch_control::devices::ObservedMidi,
        app: &mut meldritch_app::AppController,
        pickup: &mut meldritch_control::devices::MidiPickup,
    ) -> Result<MidiMappedOutcome, String> {
        let view = app.view_model();
        let modifiers = view
            .performance
            .active_modifiers
            .iter()
            .filter(|modifier| modifier.active)
            .map(|modifier| modifier.id.clone())
            .collect::<Vec<_>>();
        let page = active_page_id(&view);
        let Some(mapped) = decode_script_midi_input(
            &observed.bytes,
            &observed.device,
            &self.controls,
            &self.modifier_controls,
            &self.modifiers,
            &self.actions,
            page.as_deref(),
            &modifiers,
        ) else {
            return Ok(MidiMappedOutcome::Unmapped);
        };
        let physical = decode_midi_cc_message(&observed.bytes).map(|(channel, cc, value)| {
            meldritch_app::MidiControlInput {
                device: observed.device.to_string(),
                channel,
                cc,
                value,
            }
        });
        if self.takeover
            && let Some(physical) = &physical
            && !pickup.observe(physical, &mapped.input, &view)?
        {
            return Ok(MidiMappedOutcome::Waiting(format!(
                "{} pickup waiting: {:?}",
                observed.device, mapped.input
            )));
        }
        let result = app
            .handle_input(mapped.input.clone())
            .map_err(|e| format!("MIDI command rejected: {e:?}"))?;
        if matches!(
            mapped.input,
            meldritch_app::AppInput::SetCuratedControlNormalized { .. }
        ) && let Some(physical) = &physical
        {
            pickup.acknowledge(physical, &app.view_model());
        }
        Ok(MidiMappedOutcome::Applied(mapped, result))
    }
}

fn decode_script_midi_input(
    message: &[u8],
    device: &str,
    control_bindings: &[meldritch_app::MidiControlBinding],
    modifier_control_bindings: &[meldritch_app::MidiModifierControlBinding],
    modifier_bindings: &[MidiModifierBinding],
    action_bindings: &[MidiActionBinding],
    active_page: Option<&str>,
    active_modifiers: &[String],
) -> Option<MappedMidiInput> {
    if let Some((channel, cc, value)) = decode_midi_cc_message(message) {
        if let Some(input) =
            map_script_midi_modifier_cc(modifier_bindings, device, channel, cc, value)
        {
            return Some(input);
        }
        return map_script_midi_control_change_with_modifiers(
            control_bindings,
            modifier_control_bindings,
            action_bindings,
            meldritch_app::MidiControlInput {
                device: device.to_owned(),
                channel,
                cc,
                value,
            },
            active_page,
            active_modifiers,
        );
    }
    let (channel, note, _velocity, on) = decode_midi_note_message(message)?;
    map_script_midi_modifier_note(modifier_bindings, device, channel, note, on)
        .or_else(|| map_script_midi_note(action_bindings, device, channel, note, on))
}

fn map_script_midi_control_change(
    control_bindings: &[meldritch_app::MidiControlBinding],
    action_bindings: &[MidiActionBinding],
    input: meldritch_app::MidiControlInput,
    active_page: Option<&str>,
) -> Option<MappedMidiInput> {
    map_script_midi_control_change_with_modifiers(
        control_bindings,
        &[],
        action_bindings,
        input,
        active_page,
        &[],
    )
}

fn map_script_midi_control_change_with_modifiers(
    control_bindings: &[meldritch_app::MidiControlBinding],
    modifier_control_bindings: &[meldritch_app::MidiModifierControlBinding],
    action_bindings: &[MidiActionBinding],
    input: meldritch_app::MidiControlInput,
    active_page: Option<&str>,
    active_modifiers: &[String],
) -> Option<MappedMidiInput> {
    meldritch_app::map_midi_modifier_control_input_for_page(
        modifier_control_bindings,
        input.clone(),
        active_page,
        active_modifiers,
    )
    .map(|input| MappedMidiInput {
        input,
        label: Some("modifier control".to_owned()),
    })
    .or_else(|| {
        meldritch_app::map_midi_control_input_for_page(control_bindings, input.clone(), active_page)
            .map(|input| MappedMidiInput { input, label: None })
    })
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

fn map_script_midi_modifier_cc(
    bindings: &[MidiModifierBinding],
    device: &str,
    channel: u8,
    cc: u8,
    value: u8,
) -> Option<MappedMidiInput> {
    bindings
        .iter()
        .find(|binding| {
            binding.device == device
                && matches!(
                    binding.message,
                    MidiActionMessage::ControlChange {
                        channel: binding_channel,
                        cc: binding_cc,
                    } if binding_cc == cc
                        && binding_channel.is_none_or(|binding_channel| binding_channel == channel)
                )
        })
        .map(|binding| MappedMidiInput {
            input: meldritch_app::AppInput::SetPerformanceModifier {
                modifier_id: binding.modifier_id.clone(),
                active: value != 0,
            },
            label: Some(binding.label.clone()),
        })
}

fn map_script_midi_modifier_note(
    bindings: &[MidiModifierBinding],
    device: &str,
    channel: u8,
    note: u8,
    on: bool,
) -> Option<MappedMidiInput> {
    bindings
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
            input: meldritch_app::AppInput::SetPerformanceModifier {
                modifier_id: binding.modifier_id.clone(),
                active: on,
            },
            label: Some(binding.label.clone()),
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

fn decode_midi_diagnostic_message(message: &[u8]) -> Option<MidiDiagnosticMessage> {
    if let Some((channel, cc, value)) = decode_midi_cc_message(message) {
        return Some(MidiDiagnosticMessage::ControlChange { channel, cc, value });
    }
    if let Some((channel, note, velocity, on)) = decode_midi_note_message(message) {
        return Some(MidiDiagnosticMessage::Note {
            channel,
            note,
            velocity,
            on,
        });
    }
    Some(MidiDiagnosticMessage::Raw(message.to_vec()))
}

fn load_lua_controls(
    song: &meldritch_dsl::ValidatedSong,
    source: &Path,
) -> Result<meldritch_control::LuaProject, String> {
    let root = song.root().canonicalize().map_err(|e| e.to_string())?;
    let file_path = root
        .join(source)
        .canonicalize()
        .map_err(|e| e.to_string())?;
    if source.is_absolute() || !file_path.starts_with(&root) {
        return Err("Lua source must be inside the song root".into());
    }
    let file = std::fs::File::open(&file_path).map_err(|e| e.to_string())?;
    let mut source = String::new();
    std::io::Read::read_to_string(&mut std::io::Read::take(file, 256 * 1024 + 1), &mut source)
        .map_err(|e| e.to_string())?;
    let project = meldritch_control::LuaProject::load(&root, &source)?;
    if project.song.fingerprint() != song.fingerprint() {
        return Err("Lua project must compose the loaded song".into());
    }
    Ok(project)
}

fn connect_apc_feedback(
    device: &meldritch_dsl::MidiDeviceDefinition,
) -> Result<Option<MidiFeedbackConnection>, String> {
    let midi = midir::MidiOutput::new("meldritch-lua-feedback").map_err(|e| e.to_string())?;
    let ports = midi
        .ports()
        .into_iter()
        .filter_map(|port| {
            let name = midi.port_name(&port).ok()?;
            midi_port_name_matches(&name, device.name_contains()).then_some((port, name))
        })
        .collect::<Vec<_>>();
    if ports.is_empty() {
        return Ok(None);
    }
    if ports.len() != 1 {
        return Err("Lua APC output requires one matching Session Control port".into());
    }
    let (port, name) = ports.into_iter().next().unwrap();
    let port_id = port.id();
    let connection = midi
        .connect(&port, "meldritch-lua-feedback")
        .map_err(|e| e.to_string())?;
    eprintln!("Lua APC feedback connected: {name}");
    Ok(Some(MidiFeedbackConnection {
        connection,
        port_id,
        port_name: name,
    }))
}

fn tui_song(
    path: PathBuf,
    frames: Option<u64>,
    chunk_frames: u32,
    workers: usize,
    midi_controls: bool,
    autoplay: bool,
    midi_debug: bool,
    audio_debug: bool,
    lua_source: Option<PathBuf>,
    soft_takeover: bool,
) -> Result<(), String> {
    let song = meldritch_dsl::load_song_directory(&path).map_err(|error| error.to_string())?;
    let lua_project = lua_source
        .map(|source| load_lua_controls(&song, &source))
        .transpose()?;
    let soft_takeover = soft_takeover || lua_project.is_some();
    let patch = CompiledSongPatch::compile_initial(&song)?;
    let scene_bank = song_scene_bank(&song, tempo_from_song(&song)?)?;
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
        .render_default(FrameRange::new(0, u64::from(frame_count)).expect("song range is ordered"))
        .map_err(|error| format!("failed to render initial song audio: {error:?}"))?;
    eprintln!(
        "initial song render: frames={}, channels={}, peak={:.3}",
        initial.frames(),
        initial.channels(),
        initial.peak_abs()
    );
    let initial_song_audio = PublishedAudio::from_block(&initial, chunk_frames)
        .map_err(|error| format!("failed to prepare initial song audio: {error:?}"))?;
    let (song_audio_publisher, song_audio_reader) =
        meldritch_audio::audio_publication::audio_publication(initial_song_audio);
    let debug_song_audio_reader = song_audio_reader.clone();
    let worker_count = if workers == 0 {
        std::thread::available_parallelism().map_or(1, std::num::NonZero::get)
    } else {
        workers
    };
    let (playback, engine) = meldritch_audio::device_output::playback_session_parts(32)?;
    let settings = RenderSettings::new(initial.channels())
        .map_err(|error| format!("invalid render settings: {error:?}"))?;
    let tempo = tempo_from_song(&song)?;
    let pattern = initial_song_pattern_to_grid(&song, tempo, PatternId::new(1))?;
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
    let mut controls = song_controls_for_view(
        &song,
        patch.feedback(),
        patch.mix(),
        patch.cutoff_hz(),
        patch.resonance(),
    );
    if soft_takeover {
        initialize_pickup_controls(&song, &mut controls);
    }
    controller.set_curated_controls(controls);
    controller.set_performance_pages(song_performance_pages_for_view(&song));
    controller.set_performance_modifiers(song_performance_modifiers_for_view(&song));
    controller.set_performance_ui(
        song_performance_ui_sections_for_view(&song),
        song_performance_key_hints_for_view(&song),
    );
    if let Some(phrase_variations) = scene_bank.phrase_variations.clone() {
        controller
            .configure_phrase_variations(phrase_variations)
            .map_err(|error| format!("failed to configure song phrase scenes: {error}"))?;
    }
    let lua_surface = lua_project
        .as_ref()
        .map(|project| {
            let surfaces = project
                .surfaces()
                .filter(|(_, kind)| *kind == meldritch_control::SurfaceKind::ApcMiniMk2)
                .map(|(id, _)| id.to_owned())
                .collect::<Vec<_>>();
            if surfaces.len() != 1 {
                return Err("live Lua host requires exactly one APC surface".to_owned());
            }
            Ok(surfaces[0].clone())
        })
        .transpose()?;
    let (lua_route, lua_receiver) = meldritch_control::devices::ApcInputRoute::new(
        lua_project
            .as_ref()
            .zip(lua_surface.as_deref())
            .map_or_else(BTreeSet::new, |(project, surface)| {
                project.surface_cells(surface)
            }),
    );
    let lua_performance = Rc::new(RefCell::new(
        lua_project
            .map(|project| {
                meldritch_control::performance::LuaPerformance::new(project, &controller, tempo)
            })
            .transpose()?,
    ));
    let input_lua_performance = Rc::clone(&lua_performance);
    let lua_output_device = if midi_controls {
        lua_surface
            .as_deref()
            .and_then(|surface| {
                song.performance()
                    .midi_devices()
                    .iter()
                    .find(|device| device.id() == surface)
            })
            .cloned()
    } else {
        None
    };
    let mut lua_output = meldritch_control::reconnect::Connection::default();
    let mut lua_feedback = meldritch_control::devices::ApcFeedback::default();
    let (external_input_sender, external_input_receiver) = mpsc::channel();
    let control_queue = meldritch_control::devices::MidiInputQueue::default();
    let midi_control_bindings = midi_control_bindings_for_song(&song);
    let midi_modifier_control_bindings = midi_modifier_control_bindings_for_song(&song);
    let midi_modifier_bindings = midi_modifier_bindings_for_song(&song);
    let midi_action_bindings = midi_action_bindings_for_song(&song);
    let midi_active_page = Arc::new(Mutex::new(active_page_id(&controller.view_model())));
    let midi_active_modifiers = Arc::new(Mutex::new(Vec::<String>::new()));
    let mut midi_inputs = if midi_controls {
        let mut connections = Vec::new();
        for device in song.performance().midi_devices() {
            let bindings = midi_control_bindings_for_device(&midi_control_bindings, device.id());
            let modifier_control_bindings = midi_modifier_control_bindings_for_device(
                &midi_modifier_control_bindings,
                device.id(),
            );
            let modifier_bindings =
                midi_modifier_bindings_for_device(&midi_modifier_bindings, device.id());
            let action_bindings =
                midi_action_bindings_for_device(&midi_action_bindings, device.id());
            if bindings.is_empty()
                && modifier_control_bindings.is_empty()
                && modifier_bindings.is_empty()
                && action_bindings.is_empty()
                && lua_surface.as_deref() != Some(device.id())
            {
                continue;
            }
            connections.push((
                device.clone(),
                meldritch_control::reconnect::Connection::default(),
            ));
        }
        connections
    } else {
        Vec::new()
    };
    let _session = meldritch_audio::device_output::RealtimePlaybackSession::open_default(
        song_audio_reader,
        song.performance().sample_rate(),
        u32::MAX,
        controller.playback_control(),
        engine,
    )?;
    if autoplay {
        controller
            .dispatch(meldritch_app::AppCommand::Play)
            .map_err(|error| format!("failed to start autoplay: {error:?}"))?;
    }
    let host = Arc::new(Mutex::new(SongAudioHost::new(
        song.clone(),
        patch,
        song_audio_publisher,
        frame_count,
        chunk_frames,
    )));
    let input_host = Arc::clone(&host);
    let scene_patches = scene_bank.patches;
    let mut last_audio_debug = Instant::now()
        .checked_sub(Duration::from_millis(500))
        .unwrap_or_else(Instant::now);
    let capture = Arc::new(Mutex::new(PerformanceSessionCapture::create(
        &song,
        frame_count,
    )?));
    let input_capture = Arc::clone(&capture);
    let tick_capture = Arc::clone(&capture);
    let input_midi_active_page = Arc::clone(&midi_active_page);
    let mut mapper = ScriptMidiMapper::for_song(&song);
    mapper.takeover = soft_takeover;
    let midi_clock = Instant::now();
    let mut pickup = meldritch_control::devices::MidiPickup::default();
    let accepted = Rc::new(RefCell::new(
        move |controller: &meldritch_app::AppController,
              input: &meldritch_app::AppInput,
              result: &meldritch_app::AppCommandResult| {
            if let Err(error) = input_capture
                .lock()
                .expect("performance session capture lock poisoned")
                .record(controller, input, result, frame_count, tempo)
            {
                eprintln!("session capture failed: {error}");
            }
            if matches!(
                result,
                meldritch_app::AppCommandResult::PerformancePageSelected { .. }
            ) {
                *input_midi_active_page
                    .lock()
                    .expect("active page lock poisoned") = active_page_id(&controller.view_model());
            }
            let mut host = input_host.lock().expect("song host lock poisoned");
            if let Some((scene, variation)) = song_scene_selection(input, result)
                && let Some(patch) = scene_patches.get(&(scene, variation)).cloned()
            {
                host.replace_patch(patch);
            } else if let Err(error) = host.accepted(&controller.view_model(), result) {
                eprintln!("song update failed: {error}");
            }
            if let Some(performance) = input_lua_performance.borrow_mut().as_mut()
                && let Err(error) = performance.external_accepted(controller, result)
            {
                eprintln!("Lua presentation failed: {error}");
            }
        },
    ));
    let tick_accepted = Rc::clone(&accepted);
    let run_result = meldritch_tui::run_with_hooks_and_external_inputs(
        &mut controller,
        Step::new(36),
        move |controller| {
            use meldritch_control::reconnect::Change;
            let mut message = None;
            let now = midi_clock.elapsed();
            for (device, connection) in &mut midi_inputs {
                let was_connected = connection.get_mut().is_some();
                let change = connection.poll(now, midi_input_present, || {
                    connect_midi_control_input(
                        device,
                        external_input_sender.clone(),
                        midi_control_bindings_for_device(&midi_control_bindings, device.id()),
                        midi_modifier_control_bindings_for_device(
                            &midi_modifier_control_bindings,
                            device.id(),
                        ),
                        midi_modifier_bindings_for_device(&midi_modifier_bindings, device.id()),
                        midi_action_bindings_for_device(&midi_action_bindings, device.id()),
                        Arc::clone(&midi_active_page),
                        Arc::clone(&midi_active_modifiers),
                        midi_debug,
                        None,
                        Some(control_queue.route(device.id())),
                    )
                });
                let connected = connection.get_mut().is_some();
                if !matches!(change, Change::Unchanged) {
                    message = Some(format!("MIDI {}: {change:?}", device.id()));
                    if !connected {
                        pickup.disconnect(device.id());
                    }
                    if lua_surface.as_deref() == Some(device.id())
                        && let Some(performance) = lua_performance.borrow_mut().as_mut()
                    {
                        if connected {
                            lua_feedback.invalidate();
                        }
                        if let Err(error) = performance.connection(
                            device.id(),
                            connected,
                            controller,
                            &mut host.lock().expect("song host lock poisoned"),
                            &mut tick_capture.lock().expect("capture lock poisoned"),
                        ) {
                            message = Some(error);
                        }
                    }
                }
                // A lost note-off must not leave a physical momentary layer held.
                // Release via the same accepted command/capture hook as input.
                if was_connected && !connected {
                    for input in disconnected_modifier_inputs(
                        device.id(),
                        &midi_modifier_bindings,
                        &controller.view_model(),
                    ) {
                        match controller.handle_input(input.clone()) {
                            Ok(result) => tick_accepted.borrow_mut()(controller, &input, &result),
                            Err(error) => {
                                message = Some(format!("MIDI disconnect release: {error:?}"))
                            }
                        }
                    }
                }
            }
            if let Some(device) = &lua_output_device {
                let change =
                    lua_output.poll(now, midi_output_present, || connect_apc_feedback(device));
                if change == Change::Connected {
                    lua_feedback.invalidate();
                }
                if !matches!(change, Change::Unchanged) {
                    message = Some(format!("APC feedback: {change:?}"));
                }
            }
            let dropped_control = control_queue.take_dropped();
            let mut stale_control = 0;
            for _ in 0..meldritch_control::QUEUE_CAPACITY {
                let Some(observed) = control_queue.next() else {
                    break;
                };
                if !observed.is_current() {
                    stale_control += 1;
                    continue;
                }
                if lua_surface.as_deref() == Some(observed.device.as_ref())
                    && let Some(meldritch_control::devices::ApcInput::Grid { cell, pressed }) =
                        meldritch_control::devices::decode_apc(0, &observed.bytes)
                    && lua_route.owns_cell(cell)
                {
                    if pressed && let Some(performance) = lua_performance.borrow_mut().as_mut() {
                        let frame = u64::from(controller.view_model().transport.position);
                        if let Err(error) = performance.press(
                            &observed.device,
                            cell,
                            frame,
                            controller,
                            &mut host.lock().expect("song host lock poisoned"),
                            &mut tick_capture.lock().expect("capture lock poisoned"),
                        ) {
                            message = Some(format!("Lua host: {error}"));
                        }
                    }
                    continue;
                }
                match mapper.apply(&observed, controller, &mut pickup) {
                    Ok(MidiMappedOutcome::Applied(mapped, result)) => {
                        tick_accepted.borrow_mut()(controller, &mapped.input, &result);
                        if midi_debug {
                            message = Some(format!("MIDI accepted: {:?}", mapped.input));
                        }
                    }
                    Ok(MidiMappedOutcome::Waiting(waiting)) => {
                        message = Some(waiting);
                    }
                    Ok(MidiMappedOutcome::Unmapped) => {
                        if midi_debug {
                            message = Some(format!("MIDI {:?} -> unmapped", observed.bytes));
                        }
                    }
                    Err(error) => {
                        message = Some(error);
                    }
                }
            }
            let dropped_lua = lua_route.take_dropped();
            if let Some(performance) = lua_performance.borrow_mut().as_mut() {
                for cell in lua_receiver
                    .try_iter()
                    .take(meldritch_control::QUEUE_CAPACITY)
                {
                    let frame = u64::from(controller.view_model().transport.position);
                    if let Err(error) = performance.press(
                        lua_surface.as_deref().unwrap(),
                        cell,
                        frame,
                        controller,
                        &mut host.lock().expect("song host lock poisoned"),
                        &mut tick_capture.lock().expect("capture lock poisoned"),
                    ) {
                        message = Some(format!("Lua host: {error}"));
                    }
                }
                if let Err(error) = performance.plane.refresh(controller) {
                    message = Some(format!("Lua presentation: {error}"));
                }
                if let Some(output) = lua_output.get_mut()
                    && let Some(frame) = performance.plane.output(lua_surface.as_deref().unwrap())
                    && let Err(error) = lua_feedback.update(frame, |bytes| {
                        output.connection.send(bytes).map_err(|e| e.to_string())
                    })
                {
                    lua_output.fail(now);
                    lua_feedback.invalidate();
                    message = Some(format!("Lua APC feedback: {error}"));
                }
            }
            match host.lock().expect("song host lock poisoned").poll_publish() {
                Ok(Some(generation)) => {
                    message = Some(format!("Song rerender published: generation {generation}"));
                }
                Err(error) => {
                    message = Some(error);
                }
                Ok(None) => {}
            }
            if dropped_control != 0 || dropped_lua != 0 || stale_control != 0 {
                message = Some(format!(
                    "Input: {dropped_control} MIDI overflow, {dropped_lua} Lua overflow, {stale_control} stale observations discarded; {}",
                    message.unwrap_or_default()
                ));
            }
            if message.is_some() {
                return Ok(message);
            }
            if audio_debug && last_audio_debug.elapsed() >= Duration::from_millis(500) {
                last_audio_debug = Instant::now();
                return Ok(Some(song_audio_debug_line(
                    &debug_song_audio_reader,
                    controller.playback_control().status_monitor().snapshot(),
                )));
            }
            Ok(None)
        },
        move || external_input_receiver.try_recv().ok(),
        move |controller, input, result| {
            accepted.borrow_mut()(controller, input, result);
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

fn effective_song_audio_debug(midi_debug: bool, audio_debug: bool) -> bool {
    midi_debug || audio_debug
}

fn song_performance_modifiers_for_view(
    song: &meldritch_dsl::ValidatedSong,
) -> Vec<meldritch_app::PerformanceModifierView> {
    song.performance()
        .modifiers()
        .iter()
        .map(|modifier| meldritch_app::PerformanceModifierView {
            id: modifier.id().to_owned(),
            label: modifier.label().to_owned(),
            active: false,
        })
        .collect()
}

fn song_performance_ui_sections_for_view(
    song: &meldritch_dsl::ValidatedSong,
) -> Vec<meldritch_app::PerformanceUiSectionView> {
    song.performance()
        .ui()
        .sections()
        .iter()
        .map(|section| meldritch_app::PerformanceUiSectionView {
            kind: match section.kind() {
                meldritch_dsl::PerformanceUiSectionKind::Transport => {
                    meldritch_app::PerformanceUiSectionKind::Transport
                }
                meldritch_dsl::PerformanceUiSectionKind::PageOverview => {
                    meldritch_app::PerformanceUiSectionKind::PageOverview
                }
                meldritch_dsl::PerformanceUiSectionKind::PatternGrid => {
                    meldritch_app::PerformanceUiSectionKind::PatternGrid
                }
                meldritch_dsl::PerformanceUiSectionKind::VisibleControls => {
                    meldritch_app::PerformanceUiSectionKind::VisibleControls
                }
                meldritch_dsl::PerformanceUiSectionKind::Status => {
                    meldritch_app::PerformanceUiSectionKind::Status
                }
                meldritch_dsl::PerformanceUiSectionKind::KeyHints => {
                    meldritch_app::PerformanceUiSectionKind::KeyHints
                }
            },
            title: section.title().map(ToOwned::to_owned),
            height: section.height(),
        })
        .collect()
}

fn song_performance_key_hints_for_view(
    song: &meldritch_dsl::ValidatedSong,
) -> Vec<meldritch_app::PerformanceKeyHintView> {
    song.performance()
        .ui()
        .key_hints()
        .iter()
        .map(|hint| meldritch_app::PerformanceKeyHintView {
            keys: hint.keys().to_owned(),
            label: hint.label().to_owned(),
        })
        .collect()
}

fn active_page_id(view: &meldritch_app::AppViewModel) -> Option<String> {
    let index = view.performance.active_page.unwrap_or(0);
    view.performance
        .pages
        .get(index)
        .or_else(|| view.performance.pages.first())
        .map(|page| page.id.clone())
}

fn song_audio_debug_line(
    reader: &meldritch_audio::audio_publication::AudioSnapshotReader,
    status: meldritch_audio::realtime_status::RealtimeStatusSnapshot,
) -> String {
    let snapshot = reader.snapshot();
    let frames = snapshot.frames().max(1);
    let position = status.position % frames;
    let current = snapshot.frame(position).map_or(0.0, |frame| {
        frame
            .iter()
            .fold(0.0_f64, |peak, sample| peak.max(sample.abs()))
    });
    let mut window_peak = 0.0_f64;
    let window = 2048_u32.min(frames);
    for offset in 0..window {
        let frame_index = (position + offset) % frames;
        if let Ok(frame) = snapshot.frame(frame_index) {
            for sample in frame {
                window_peak = window_peak.max(sample.abs());
            }
        }
    }
    format!(
        "Audio debug: {:?} pos {} callbacks {} frames {} current {:.4} next_peak {:.4}",
        status.state, status.position, status.callbacks, frames, current, window_peak
    )
}

struct SongSceneBank {
    patches: BTreeMap<(SceneId, usize), CompiledSongPatch>,
    #[cfg(test)]
    lane_patches: BTreeMap<(String, String), CompiledSongPatch>,
    phrase_variations: Option<Vec<(SceneId, Vec<Pattern>)>>,
}

fn tempo_from_song(song: &meldritch_dsl::ValidatedSong) -> Result<Tempo, String> {
    Tempo::new(song.performance().bpm(), song.performance().sample_rate())
        .map_err(|error| format!("invalid song tempo: {error}"))
}

fn song_scene_bank(
    song: &meldritch_dsl::ValidatedSong,
    tempo: Tempo,
) -> Result<SongSceneBank, String> {
    if song.performance().tracks().is_empty() {
        return Ok(SongSceneBank {
            patches: BTreeMap::new(),
            #[cfg(test)]
            lane_patches: BTreeMap::new(),
            phrase_variations: None,
        });
    };
    let single_track = song.performance().tracks().len() == 1;
    let track = &song.performance().tracks()[0];
    let pattern_ids = track.pattern_ids();
    if pattern_ids.is_empty() {
        return Ok(SongSceneBank {
            patches: BTreeMap::new(),
            #[cfg(test)]
            lane_patches: BTreeMap::new(),
            phrase_variations: None,
        });
    }
    let mut patches = BTreeMap::new();
    #[cfg(test)]
    let mut lane_patches = BTreeMap::new();
    let mut phrase_variations = Vec::new();
    // Retain fixture compiler checks; production recompiles the complete view.
    #[cfg(test)]
    for lane in song.performance().lanes() {
        let Some(track_id) = lane.track_id() else {
            continue;
        };
        let Some(track) = song
            .performance()
            .tracks()
            .iter()
            .find(|track| track.id() == track_id)
        else {
            continue;
        };
        for variation_id in lane.variation_ids() {
            if !track
                .pattern_ids()
                .iter()
                .any(|pattern| pattern == variation_id)
            {
                continue;
            }
            let patch = if single_track {
                CompiledSongPatch::Delayed(
                    meldritch_render::song_render::compile_delayed_note_song_for_pattern(
                        song,
                        variation_id,
                    )
                    .map_err(|error| {
                        format!(
                            "failed to compile lane '{}' variation '{}': {error:?}",
                            lane.id(),
                            variation_id
                        )
                    })?,
                )
            } else {
                CompiledSongPatch::Mixed(
                    meldritch_render::song_render::compile_mixed_note_song_with_lane_variation(
                        song,
                        lane.id(),
                        variation_id,
                    )
                    .map_err(|error| {
                        format!(
                            "failed to compile lane '{}' variation '{}': {error:?}",
                            lane.id(),
                            variation_id
                        )
                    })?,
                )
            };
            lane_patches.insert((lane.id().to_owned(), variation_id.clone()), patch);
        }
    }
    if !single_track {
        return Ok(SongSceneBank {
            patches,
            #[cfg(test)]
            lane_patches,
            phrase_variations: None,
        });
    }
    for (scene_index, scene_patterns) in pattern_ids.chunks(2).enumerate() {
        let scene = SceneId::new(scene_index as u64 + 1);
        let mut variations = Vec::new();
        for (variation, pattern_id) in scene_patterns.iter().enumerate() {
            let patch = meldritch_render::song_render::compile_delayed_note_song_for_pattern(
                song, pattern_id,
            )
            .map_err(|error| {
                format!(
                    "failed to compile song scene {} variation {} pattern '{}': {error:?}",
                    scene.raw(),
                    variation,
                    pattern_id
                )
            })?;
            patches.insert((scene, variation), CompiledSongPatch::Delayed(patch));
            let note_pattern = song.note_patterns().get(pattern_id).ok_or_else(|| {
                format!(
                    "track '{}' references missing note pattern '{}'",
                    track.id(),
                    pattern_id
                )
            })?;
            variations.push(song_note_pattern_to_grid(
                note_pattern,
                tempo,
                PatternId::new(scene.raw().saturating_mul(10) + variation as u64),
            )?);
        }
        phrase_variations.push((scene, variations));
    }
    Ok(SongSceneBank {
        patches,
        #[cfg(test)]
        lane_patches,
        phrase_variations: Some(phrase_variations),
    })
}

fn song_note_pattern_to_grid(
    pattern: &meldritch_dsl::NotePatternDefinition,
    tempo: Tempo,
    id: PatternId,
) -> Result<Pattern, String> {
    let frames_per_step = tempo.step_start_frame(1, 4).max(1);
    let length_steps =
        ((pattern.length_frames() as f64 / frames_per_step as f64).round() as u32).max(1);
    let mut grid = Pattern::new(id, length_steps, 4).map_err(|error| {
        format!(
            "failed to create phrase grid for '{}': {error:?}",
            pattern.id()
        )
    })?;
    for event in pattern.events() {
        let step = ((event.start_frame() as f64 / frames_per_step as f64).round() as u32)
            .min(length_steps.saturating_sub(1));
        let gate = (event.duration_frames() as f64 / frames_per_step as f64).max(0.05);
        grid.set_step(
            TrackId::new(1),
            StepIndex::new(step),
            Step::new(event.note())
                .with_velocity(event.velocity())
                .with_gate(gate),
        )
        .map_err(|error| {
            format!(
                "failed to map event at frame {} in pattern '{}': {error:?}",
                event.start_frame(),
                pattern.id()
            )
        })?;
    }
    Ok(grid)
}

fn initial_song_pattern_to_grid(
    song: &meldritch_dsl::ValidatedSong,
    tempo: Tempo,
    id: PatternId,
) -> Result<Pattern, String> {
    let [track] = song.performance().tracks() else {
        return Pattern::new(id, 16, 4)
            .map_err(|error| format!("failed to create empty song grid: {error:?}"));
    };
    let Some(pattern_id) = track.initial_pattern() else {
        return Pattern::new(id, 16, 4)
            .map_err(|error| format!("failed to create empty song grid: {error:?}"));
    };
    let pattern = song.note_patterns().get(pattern_id).ok_or_else(|| {
        format!(
            "track '{}' references missing initial note pattern '{}'",
            track.id(),
            pattern_id
        )
    })?;
    song_note_pattern_to_grid(pattern, tempo, id)
}

fn song_scene_selection(
    input: &meldritch_app::AppInput,
    result: &meldritch_app::AppCommandResult,
) -> Option<(SceneId, usize)> {
    let meldritch_app::AppCommandResult::PerformanceQueued(queued) = result else {
        return None;
    };
    let meldritch_render::futures::PerformanceGesture::QueueScene(scene) = queued.gesture else {
        return None;
    };
    let variation = match input {
        meldritch_app::AppInput::QueuePhraseVariation(input_scene, variation)
            if *input_scene == scene =>
        {
            *variation
        }
        _ => 0,
    };
    Some((scene, variation))
}

#[cfg(test)]
fn song_lane_selection(result: &meldritch_app::AppCommandResult) -> Option<(String, String)> {
    match result {
        meldritch_app::AppCommandResult::LaneVariationSelected {
            lane_id, current, ..
        } => Some((lane_id.clone(), current.clone())),
        meldritch_app::AppCommandResult::LanePatternBankSelected {
            lane_id,
            current_variation,
            ..
        } => Some((lane_id.clone(), current_variation.clone())),
        _ => None,
    }
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
            action if is_dsp_action(&action) => continue,
            action => {
                controller
                    .handle_input(
                        action
                            .clone()
                            .input()
                            .expect("non-phrase action has an input"),
                    )
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
                            .handle_input(action.clone().input().expect("DSP action has an input"))
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

    fn example_song_path(relative: &str) -> PathBuf {
        let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
        let workspace_path = manifest.join(relative);
        if workspace_path.exists() {
            workspace_path
        } else {
            manifest.join("../..").join(relative)
        }
    }

    #[test]
    fn control_thread_mapper_applies_page_before_fader_and_preserves_curves() {
        use meldritch_control::devices::{MidiInputQueue, MidiPickup};
        let song = meldritch_dsl::load_song_directory(example_song_path(
            "songs/examples/22-midi-soft-takeover",
        ))
        .unwrap();
        let (playback, _engine) =
            meldritch_audio::device_output::playback_session_parts(8).unwrap();
        let state = meldritch_render::coordinator::SampleRenderState::new(
            Pattern::new(PatternId::new(1), 4, 4).unwrap(),
            Tempo::new(120.0, 48000).unwrap(),
            ProbabilitySeed::new(1),
            RenderSettings::new(1).unwrap(),
            Arc::new(BTreeMap::new()),
        );
        let coordinator = RenderCoordinator::new_from_state(
            RenderCoordinatorConfig::new(1, 24000, 6000, 1, Duration::from_millis(2)).unwrap(),
            state.clone(),
            playback.status_monitor(),
        )
        .unwrap();
        let mut app = meldritch_app::AppController::new(
            playback,
            coordinator,
            meldritch_render::live_edit::LivePatternEditor::new(state, 24000),
            meldritch_app::Selection {
                track: TrackId::new(1),
                step: StepIndex::new(0),
            },
            256,
        );
        app.set_performance_pages(song_performance_pages_for_view(&song));
        app.set_performance_modifiers(song_performance_modifiers_for_view(&song));
        let mut controls = song_controls_for_view(&song, None, None, None, None);
        initialize_pickup_controls(&song, &mut controls);
        app.set_curated_controls(controls);
        let mut mapper = ScriptMidiMapper::for_song(&song);
        let mut pickup = MidiPickup::default();
        let queue = MidiInputQueue::default();
        let route = queue.route("apc-mini");
        route.observe(&[0x90, 101, 127]);
        route.observe(&[0xb0, 48, 0]);
        route.observe(&[0xb0, 48, 127]);
        assert!(matches!(
            mapper
                .apply(&queue.next().unwrap(), &mut app, &mut pickup)
                .unwrap(),
            MidiMappedOutcome::Applied(
                _,
                meldritch_app::AppCommandResult::PerformancePageSelected { .. }
            )
        ));
        assert!(matches!(
            mapper
                .apply(&queue.next().unwrap(), &mut app, &mut pickup)
                .unwrap(),
            MidiMappedOutcome::Waiting(_)
        ));
        assert!(
            matches!(mapper.apply(&queue.next().unwrap(), &mut app, &mut pickup).unwrap(), MidiMappedOutcome::Applied(MappedMidiInput { input: meldritch_app::AppInput::SetCuratedControlNormalized { id, .. }, .. }, _) if id == "b-level")
        );
        assert_eq!(app.history().len(), 2);
        app.dispatch(meldritch_app::AppCommand::SetCuratedControlNormalized {
            id: "b-level".into(),
            value: 0.7,
        })
        .unwrap();
        mapper
            .controls
            .iter_mut()
            .find(|binding| binding.control_id == "b-level")
            .unwrap()
            .action = meldritch_app::MidiControlAction::Overdrive {
            normal: 0.7,
            normal_midi: 80,
        };
        route.observe(&[0xb0, 48, 80]);
        assert!(
            matches!(mapper.apply(&queue.next().unwrap(), &mut app, &mut pickup).unwrap(), MidiMappedOutcome::Applied(MappedMidiInput { input: meldritch_app::AppInput::SetCuratedControlNormalized { value, .. }, .. }, _) if (value - 0.7).abs() < 1e-12)
        );
        app.dispatch(meldritch_app::AppCommand::SetCuratedControlNormalized {
            id: "b-level".into(),
            value: 0.5,
        })
        .unwrap();
        mapper
            .controls
            .iter_mut()
            .find(|binding| binding.control_id == "b-level")
            .unwrap()
            .action = meldritch_app::MidiControlAction::Centered { center: 0.5 };
        route.observe(&[0xb0, 48, 64]);
        assert!(matches!(
            mapper
                .apply(&queue.next().unwrap(), &mut app, &mut pickup)
                .unwrap(),
            MidiMappedOutcome::Applied(
                MappedMidiInput {
                    input: meldritch_app::AppInput::SetCuratedControlNormalized { value: 0.5, .. },
                    ..
                },
                _
            )
        ));
        // Momentary layer messages are mapped on the same owner and FIFO.
        route.observe(&[0x90, 122, 127]);
        route.observe(&[0xb0, 48, 127]);
        route.observe(&[0x90, 122, 0]);
        route.observe(&[0xb0, 48, 10]);
        assert!(matches!(
            mapper
                .apply(&queue.next().unwrap(), &mut app, &mut pickup)
                .unwrap(),
            MidiMappedOutcome::Applied(
                _,
                meldritch_app::AppCommandResult::PerformanceModifierChanged { active: true, .. }
            )
        ));
        assert!(matches!(
            mapper
                .apply(&queue.next().unwrap(), &mut app, &mut pickup)
                .unwrap(),
            MidiMappedOutcome::Applied(
                _,
                meldritch_app::AppCommandResult::LaneOctaveSet { current: 2, .. }
            )
        ));
        assert!(matches!(
            mapper
                .apply(&queue.next().unwrap(), &mut app, &mut pickup)
                .unwrap(),
            MidiMappedOutcome::Applied(
                _,
                meldritch_app::AppCommandResult::PerformanceModifierChanged { active: false, .. }
            )
        ));
        assert!(matches!(
            mapper
                .apply(&queue.next().unwrap(), &mut app, &mut pickup)
                .unwrap(),
            MidiMappedOutcome::Waiting(_)
        ));
        mapper
            .controls
            .iter_mut()
            .find(|binding| binding.control_id == "b-level")
            .unwrap()
            .action = meldritch_app::MidiControlAction::Increment;
        route.observe(&[0xb0, 48, 1]);
        assert!(matches!(
            mapper
                .apply(&queue.next().unwrap(), &mut app, &mut pickup)
                .unwrap(),
            MidiMappedOutcome::Applied(
                MappedMidiInput {
                    input: meldritch_app::AppInput::AdjustCuratedControl { .. },
                    ..
                },
                _
            )
        ));
        // Losing a note-off releases only this device's active layer, once.
        app.handle_input(meldritch_app::AppInput::SetPerformanceModifier {
            modifier_id: "octave-layer".into(),
            active: true,
        })
        .unwrap();
        assert!(
            disconnected_modifier_inputs("other", &mapper.modifiers, &app.view_model()).is_empty()
        );
        let releases =
            disconnected_modifier_inputs("apc-mini", &mapper.modifiers, &app.view_model());
        assert_eq!(releases.len(), 1);
        app.handle_input(releases[0].clone()).unwrap();
        assert!(
            disconnected_modifier_inputs("apc-mini", &mapper.modifiers, &app.view_model())
                .is_empty()
        );
        // Legacy direct mode still applies a distant absolute fader observation.
        mapper.takeover = false;
        mapper
            .controls
            .iter_mut()
            .find(|binding| binding.control_id == "b-level")
            .unwrap()
            .action = meldritch_app::MidiControlAction::Absolute;
        route.observe(&[0xb0, 48, 0]);
        assert!(matches!(
            mapper
                .apply(&queue.next().unwrap(), &mut app, &mut pickup)
                .unwrap(),
            MidiMappedOutcome::Applied(_, _)
        ));
        let direct = Cli::try_parse_from([
            "meldritch",
            "tui-song",
            "songs/examples/22-midi-soft-takeover",
        ])
        .unwrap();
        assert!(matches!(
            direct.command,
            Command::TuiSong {
                soft_takeover: false,
                ..
            }
        ));
        let enabled = Cli::try_parse_from([
            "meldritch",
            "tui-song",
            "songs/examples/22-midi-soft-takeover",
            "--soft-takeover",
        ])
        .unwrap();
        assert!(matches!(
            enabled.command,
            Command::TuiSong {
                soft_takeover: true,
                ..
            }
        ));
    }

    #[test]
    fn lua_song_host_loads_confined_source_before_opening_devices() {
        let root = example_song_path("songs/examples/21-lua-apc-performance");
        let song = meldritch_dsl::load_song_directory(&root).unwrap();
        let project = load_lua_controls(&song, Path::new("main.lua")).unwrap();
        assert_eq!(project.song.fingerprint(), song.fingerprint());
        assert!(load_lua_controls(&song, &root.join("main.lua").canonicalize().unwrap()).is_err());
        assert!(load_lua_controls(&song, Path::new("../18-lua-control-plane/main.lua")).is_err());
        let parsed = Cli::try_parse_from([
            "meldritch",
            "tui-song",
            "songs/examples/21-lua-apc-performance",
            "--lua",
            "main.lua",
            "--no-midi-controls",
        ])
        .unwrap();
        assert!(matches!(
            parsed.command,
            Command::TuiSong {
                lua: Some(_),
                no_midi_controls: true,
                ..
            }
        ));
    }

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
        let song = meldritch_dsl::load_song_directory(example_song_path(
            "songs/examples/11-session-capture",
        ))
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
        let song = meldritch_dsl::load_song_directory(example_song_path(
            "songs/examples/11-session-capture",
        ))
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
    fn tui_song_autoplays_by_default_and_can_opt_out() {
        let default = Cli::try_parse_from([
            "meldritch",
            "tui-song",
            "songs/examples/16-launch-control-xl-playground",
        ])
        .unwrap();
        let Command::TuiSong {
            autoplay,
            no_autoplay,
            ..
        } = default.command
        else {
            panic!("expected tui-song command");
        };
        assert!(!autoplay);
        assert!(!no_autoplay);
        assert!(autoplay || !no_autoplay);

        let stopped = Cli::try_parse_from([
            "meldritch",
            "tui-song",
            "songs/examples/16-launch-control-xl-playground",
            "--no-autoplay",
        ])
        .unwrap();
        let Command::TuiSong {
            autoplay,
            no_autoplay,
            ..
        } = stopped.command
        else {
            panic!("expected tui-song command");
        };
        assert!(!autoplay);
        assert!(no_autoplay);
        assert!(!(autoplay || !no_autoplay));

        assert!(
            Cli::try_parse_from([
                "meldritch",
                "tui-song",
                "songs/examples/16-launch-control-xl-playground",
                "--autoplay",
                "--no-autoplay",
            ])
            .is_err()
        );
    }

    #[test]
    fn tui_song_midi_debug_enables_audio_debug_telemetry() {
        assert!(!effective_song_audio_debug(false, false));
        assert!(effective_song_audio_debug(true, false));
        assert!(effective_song_audio_debug(false, true));
        assert!(effective_song_audio_debug(true, true));
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
        let song = meldritch_dsl::load_song_directory(example_song_path(
            "songs/examples/15-launch-control-xl-input",
        ))
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
        let song = meldritch_dsl::load_song_directory(example_song_path(
            "songs/examples/16-launch-control-xl-playground",
        ))
        .expect("LaunchControl XL playground example should load");

        let control_bindings = midi_control_bindings_for_song(&song);
        let action_bindings = midi_action_bindings_for_song(&song);
        assert_eq!(
            map_script_midi_control_change(
                &control_bindings,
                &action_bindings,
                meldritch_app::MidiControlInput {
                    device: "launch-control-xl".to_owned(),
                    channel: 9,
                    cc: 13,
                    value: 64,
                },
                None,
            )
            .map(|mapped| mapped.input),
            Some(meldritch_app::AppInput::SetCuratedControlNormalized {
                id: "knob-01".to_owned(),
                value: 0.2 / 0.95,
            })
        );
        assert_eq!(
            map_script_midi_control_change(
                &control_bindings,
                &action_bindings,
                meldritch_app::MidiControlInput {
                    device: "launch-control-xl".to_owned(),
                    channel: 9,
                    cc: 49,
                    value: 64,
                },
                None,
            )
            .map(|mapped| mapped.input),
            Some(meldritch_app::AppInput::SetCuratedControlNormalized {
                id: "knob-17".to_owned(),
                value: 0.25,
            })
        );
        let launch_note = map_script_midi_note(&action_bindings, "launch-control-xl", 9, 41, true)
            .expect("launch button should map");
        assert_eq!(launch_note.input, meldritch_app::AppInput::TogglePlayback);
        assert_eq!(
            launch_note.label.as_deref(),
            Some("Button A01 Toggle Playback")
        );
        assert_eq!(
            map_script_midi_note(&action_bindings, "launch-control-xl", 9, 41, false),
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
        let page_cc = map_script_midi_control_change(
            &control_bindings,
            &action_bindings,
            meldritch_app::MidiControlInput {
                device: "launch-control-xl".to_owned(),
                channel: 9,
                cc: 106,
                value: 127,
            },
            None,
        )
        .expect("page select CC should map");
        assert_eq!(
            page_cc.input,
            meldritch_app::AppInput::SelectPerformancePage("main".to_owned())
        );
        assert_eq!(
            page_cc.label.as_deref(),
            Some("Track Select Prev Main Page")
        );
        assert_eq!(
            map_script_midi_control_change(
                &control_bindings,
                &action_bindings,
                meldritch_app::MidiControlInput {
                    device: "launch-control-xl".to_owned(),
                    channel: 9,
                    cc: 77,
                    value: 108,
                },
                None,
            ),
            None
        );
        assert_eq!(
            map_script_midi_control_change(
                &control_bindings,
                &action_bindings,
                meldritch_app::MidiControlInput {
                    device: "launch-control-xl".to_owned(),
                    channel: 9,
                    cc: 77,
                    value: 108,
                },
                Some("main"),
            )
            .map(|mapped| mapped.input),
            Some(meldritch_app::AppInput::SetCuratedControlNormalized {
                id: "fader-01".to_owned(),
                value: (4350.0 - 100.0) / (5000.0 - 100.0),
            })
        );
    }

    #[test]
    fn apc_playground_commands_change_authoritative_lanes_and_audio() {
        let song = meldritch_dsl::load_song_directory(example_song_path(
            "songs/examples/19-apc-mini-mk2-playground",
        ))
        .unwrap();
        let (playback, _engine) =
            meldritch_audio::device_output::playback_session_parts(8).unwrap();
        let state = meldritch_render::coordinator::SampleRenderState::new(
            meldritch_core::Pattern::new(PatternId::new(1), 4, 4).unwrap(),
            Tempo::new(118.0, 48000).unwrap(),
            ProbabilitySeed::new(1),
            meldritch_render::RenderSettings::new(1).unwrap(),
            Arc::new(BTreeMap::new()),
        );
        let coordinator = RenderCoordinator::new_from_state(
            RenderCoordinatorConfig::new(1, 48000, 6000, 1, Duration::from_millis(2)).unwrap(),
            state.clone(),
            playback.status_monitor(),
        )
        .unwrap();
        let mut app = meldritch_app::AppController::new(
            playback,
            coordinator,
            meldritch_render::live_edit::LivePatternEditor::new(state, 48000),
            meldritch_app::Selection {
                track: TrackId::new(1),
                step: StepIndex::new(0),
            },
            256,
        );
        app.set_performance_pages(song_performance_pages_for_view(&song));
        let actions = midi_action_bindings_for_song(&song);
        let range = FrameRange::new(0, 48000).unwrap();
        let before = CompiledSongPatch::compile_initial(&song)
            .unwrap()
            .render_default(range)
            .unwrap();
        let input = map_script_midi_note(&actions, "apc-mini", 1, 8, true)
            .unwrap()
            .input;
        let result = app.handle_input(input.clone()).unwrap();
        assert_eq!(
            song_lane_selection(&result),
            Some(("beat-drum".to_owned(), "beat-drum-b".to_owned()))
        );
        let view = app.view_model();
        assert_eq!(active_lane_variations(&view)["beat-drum"], "beat-drum-b");
        let after = compile_mixed_song_for_active_lanes(
            &song,
            &active_lane_variations(&view),
            &BTreeMap::new(),
            &included_lanes_from_view(&view),
        )
        .unwrap()
        .render_default(range)
        .unwrap();
        assert_ne!(before.samples(), after.samples());
        let mute = map_script_midi_note(&actions, "apc-mini", 1, 32, true)
            .unwrap()
            .input;
        let muted = app.handle_input(mute.clone()).unwrap();
        assert_session_kind(&mute, &muted, "lane_mute", Some("beat-drum"));
        assert!(!included_lanes_from_view(&app.view_model()).contains("beat-drum"));
        app.handle_input(
            map_script_midi_note(&actions, "apc-mini", 1, 42, true)
                .unwrap()
                .input,
        )
        .unwrap();
        assert_eq!(
            included_lanes_from_view(&app.view_model()),
            BTreeSet::from(["bass-a".to_owned()])
        );
    }

    #[test]
    fn apc_playground_maps_grid_buttons_and_all_nine_faders() {
        use meldritch_app::AppInput;
        let song = meldritch_dsl::load_song_directory(example_song_path(
            "songs/examples/19-apc-mini-mk2-playground",
        ))
        .unwrap();
        let controls = midi_control_bindings_for_song(&song);
        let actions = midi_action_bindings_for_song(&song);
        assert_eq!(controls.len(), 9);
        assert_eq!(actions.len(), 32);
        let lanes = ["beat-drum", "rhythm-drum-a", "bass-a", "pad"];
        for (column, lane) in lanes.iter().enumerate() {
            for (row, variation) in ['a', 'b', 'c', 'd'].iter().enumerate() {
                let note = (row * 8 + column) as u8;
                assert_eq!(
                    map_script_midi_note(&actions, "apc-mini", 1, note, true)
                        .unwrap()
                        .input,
                    AppInput::SelectLaneVariation {
                        lane_id: (*lane).to_owned(),
                        variation_id: format!("{lane}-{variation}"),
                    }
                );
                assert!(map_script_midi_note(&actions, "apc-mini", 1, note, false).is_none());
                assert!(map_script_midi_note(&actions, "apc-mini", 2, note, true).is_none());
                assert!(map_script_midi_note(&actions, "other", 1, note, true).is_none());
            }
            for note in [32 + column as u8, 100 + column as u8] {
                assert_eq!(
                    map_script_midi_note(&actions, "apc-mini", 1, note, true)
                        .unwrap()
                        .input,
                    AppInput::ToggleLaneMute((*lane).to_owned())
                );
            }
            assert_eq!(
                map_script_midi_note(&actions, "apc-mini", 1, 40 + column as u8, true)
                    .unwrap()
                    .input,
                AppInput::ToggleLaneSolo((*lane).to_owned())
            );
        }
        assert!(map_script_midi_note(&actions, "apc-mini", 1, 63, true).is_none());
        for (note, input) in [
            (112, AppInput::TogglePlayback),
            (113, AppInput::Stop),
            (114, AppInput::Rewind),
            (115, AppInput::ToggleCockpitMode),
        ] {
            assert_eq!(
                map_script_midi_note(&actions, "apc-mini", 1, note, true)
                    .unwrap()
                    .input,
                input
            );
        }
        let ids = [
            "beat-drum-level",
            "rhythm-drum-a-level",
            "bass-a-level",
            "pad-level",
            "beat-drum-cutoff",
            "rhythm-drum-a-cutoff",
            "bass-a-cutoff",
            "pad-cutoff",
            "pad-resonance",
        ];
        for (index, id) in ids.iter().enumerate() {
            for value in [0, 127] {
                let input = meldritch_app::MidiControlInput {
                    device: "apc-mini".to_owned(),
                    channel: 1,
                    cc: 48 + index as u8,
                    value,
                };
                assert_eq!(
                    map_script_midi_control_change(&controls, &actions, input, Some("main"))
                        .unwrap()
                        .input,
                    AppInput::SetCuratedControlNormalized {
                        id: (*id).to_owned(),
                        value: f64::from(value) / 127.0
                    }
                );
            }
        }
    }

    #[test]
    fn ensemble_lane_actions_are_derived_from_song_scripts() {
        let song = meldritch_dsl::load_song_directory(example_song_path(
            "songs/examples/17-launch-control-xl-ensemble",
        ))
        .expect("LaunchControl XL ensemble example should load");

        let control_bindings = midi_control_bindings_for_song(&song);
        let modifier_control_bindings = midi_modifier_control_bindings_for_song(&song);
        let modifier_bindings = midi_modifier_bindings_for_song(&song);
        let action_bindings = midi_action_bindings_for_song(&song);
        let octave_modifier =
            map_script_midi_modifier_note(&modifier_bindings, "launch-control-xl", 9, 108, true)
                .expect("octave modifier note should map");
        assert_eq!(
            octave_modifier.input,
            meldritch_app::AppInput::SetPerformanceModifier {
                modifier_id: "octave-layer".to_owned(),
                active: true,
            }
        );
        let octave_fader = map_script_midi_control_change_with_modifiers(
            &control_bindings,
            &modifier_control_bindings,
            &action_bindings,
            meldritch_app::MidiControlInput {
                device: "launch-control-xl".to_owned(),
                channel: 9,
                cc: 77,
                value: 127,
            },
            Some("main"),
            &["octave-layer".to_owned()],
        )
        .expect("modifier fader should map to lane octave");
        assert_eq!(
            octave_fader.input,
            meldritch_app::AppInput::SetLaneOctave {
                lane_id: "rhythm-drum-a".to_owned(),
                octave: 2,
            }
        );
        let fill_bank = map_script_midi_note(&action_bindings, "launch-control-xl", 9, 106, true)
            .expect("fill bank note should map");
        assert_eq!(
            fill_bank.input,
            meldritch_app::AppInput::SelectLanePatternBank {
                lane_id: "rhythm-drum-a".to_owned(),
                bank_id: "fill".to_owned(),
            }
        );
        let variation_b = map_script_midi_note(&action_bindings, "launch-control-xl", 9, 107, true)
            .expect("variation note should map");
        assert_eq!(
            variation_b.input,
            meldritch_app::AppInput::SelectLaneVariation {
                lane_id: "rhythm-drum-a".to_owned(),
                variation_id: "rhythm-drum-a-b".to_owned(),
            }
        );
        let mute = map_script_midi_control_change(
            &control_bindings,
            &action_bindings,
            meldritch_app::MidiControlInput {
                device: "launch-control-xl".to_owned(),
                channel: 9,
                cc: 104,
                value: 127,
            },
            None,
        )
        .expect("lane mute CC should map");
        assert_eq!(
            mute.input,
            meldritch_app::AppInput::ToggleLaneMute("rhythm-drum-a".to_owned())
        );
        assert_eq!(mute.label.as_deref(), Some("Rhythm Drum A Mute"));
    }

    #[test]
    fn ensemble_live_controls_target_mixed_synth_filter_overrides() {
        let song = meldritch_dsl::load_song_directory(example_song_path(
            "songs/examples/17-launch-control-xl-ensemble",
        ))
        .expect("LaunchControl XL ensemble example should load");

        let targets = live_song_control_targets(&song);
        assert_eq!(
            targets.get("main-fader-01"),
            Some(&LiveSongControlTarget::LaneLevel(
                "rhythm-drum-a".to_owned()
            ))
        );
        let main_page_lanes = (1..=8)
            .map(|strip| {
                let id = format!("main-fader-{strip:02}");
                match targets
                    .get(&id)
                    .expect("main fader should target live audio")
                {
                    LiveSongControlTarget::LaneLevel(lane_id) => lane_id.clone(),
                    target => panic!("main fader {id} should target lane level, got {target:?}"),
                }
            })
            .collect::<BTreeSet<_>>();
        assert_eq!(
            main_page_lanes,
            BTreeSet::from([
                "rhythm-drum-a".to_owned(),
                "rhythm-drum-b".to_owned(),
                "pad".to_owned(),
                "bass-a".to_owned(),
                "bass-b".to_owned(),
                "sample-a".to_owned(),
                "sample-b".to_owned(),
                "sample-c".to_owned(),
            ])
        );
        assert_eq!(
            targets.get("main-knob-01"),
            Some(&LiveSongControlTarget::FilterResonance(SynthFilterTarget {
                synth_id: "rhythm-drum-a".to_owned(),
                module_id: "filter".to_owned(),
            }))
        );
    }

    #[test]
    fn performance_pages_are_derived_from_song_scripts() {
        let song = meldritch_dsl::load_song_directory(example_song_path(
            "songs/examples/16-launch-control-xl-playground",
        ))
        .expect("LaunchControl XL playground example should load");

        let pages = song_performance_pages_for_view(&song);
        assert_eq!(pages.len(), 1);
        assert_eq!(pages[0].id, "main");
        assert_eq!(pages[0].label, "Main");
        assert_eq!(pages[0].strips.len(), 1);
        let strip = &pages[0].strips[0];
        assert_eq!(strip.strip, 1);
        assert_eq!(strip.lane_id, "playground");
        assert_eq!(strip.lane_label, "Playground Synth");
        assert_eq!(strip.lane_role, "monophonic_synth");
        assert_eq!(strip.track_id.as_deref(), Some("playground"));
        assert_eq!(strip.variation_ids.len(), 8);
        assert_eq!(
            strip.control_ids,
            ["knob-01", "knob-09", "knob-17", "fader-01"]
        );

        let ensemble = meldritch_dsl::load_song_directory(example_song_path(
            "songs/examples/17-launch-control-xl-ensemble",
        ))
        .expect("LaunchControl XL ensemble example should load");
        let pages = song_performance_pages_for_view(&ensemble);
        assert_eq!(pages.len(), 2);
        assert_eq!(pages[0].strips.len(), 8);
        let strip = &pages[0].strips[0];
        assert_eq!(strip.lane_id, "rhythm-drum-a");
        assert_eq!(strip.launch_quantization.as_deref(), Some("1 bar"));
        assert!(!strip.muted);
        assert!(!strip.soloed);
        assert_eq!(
            strip.active_variation_id.as_deref(),
            Some("rhythm-drum-a-a")
        );
        assert_eq!(strip.pattern_banks.len(), 2);
        assert_eq!(strip.pattern_banks[0].id, "groove");
        assert_eq!(
            strip.pattern_banks[0].variation_ids,
            ["rhythm-drum-a-a", "rhythm-drum-a-b"]
        );
        assert_eq!(strip.pattern_monitors.len(), 4);
        assert_eq!(strip.pattern_monitors[0].variation_id, "rhythm-drum-a-a");
        assert_eq!(strip.pattern_monitors[0].length_steps, 16);
        assert!(!strip.pattern_monitors[0].active_steps.is_empty());

        let sections = song_performance_ui_sections_for_view(&ensemble);
        assert_eq!(sections.len(), 5);
        assert!(sections
            .iter()
            .all(|section| section.kind != meldritch_app::PerformanceUiSectionKind::PatternGrid));
        assert_eq!(sections[1].title.as_deref(), Some("LaunchControl Ensemble"));
        let key_hints = song_performance_key_hints_for_view(&ensemble);
        assert_eq!(key_hints.len(), 4);
        assert_eq!(key_hints[0].keys, "Ctrl-Tab");
    }

    #[test]
    fn launch_control_playground_builds_song_scene_bank_from_patterns() {
        let song = meldritch_dsl::load_song_directory(example_song_path(
            "songs/examples/16-launch-control-xl-playground",
        ))
        .expect("LaunchControl XL playground example should load");

        let bank = song_scene_bank(&song, tempo_from_song(&song).unwrap())
            .expect("LaunchControl XL playground scene bank should build");
        assert_eq!(bank.patches.len(), 8);
        assert_eq!(bank.lane_patches.len(), 8);
        assert!(
            bank.lane_patches
                .contains_key(&("playground".to_owned(), "playground-bounce-fill".to_owned()))
        );
        for scene in 1..=4 {
            assert!(bank.patches.contains_key(&(SceneId::new(scene), 0)));
            assert!(bank.patches.contains_key(&(SceneId::new(scene), 1)));
        }
        let variations = bank.phrase_variations.expect("scene grid should exist");
        assert_eq!(variations.len(), 4);
        assert!(variations.iter().all(|(_, patterns)| {
            patterns.len() == 2
                && patterns
                    .iter()
                    .all(|pattern| pattern.length_steps() == 16 && pattern.steps_per_beat() == 4)
        }));

        let initial_grid =
            initial_song_pattern_to_grid(&song, tempo_from_song(&song).unwrap(), PatternId::new(1))
                .expect("initial playground grid should be built from the authored pattern");
        assert_eq!(initial_grid.length_steps(), 16);
        assert_eq!(initial_grid.active_step_count(), 6);
        assert_eq!(
            initial_grid
                .get_step(TrackId::new(1), StepIndex::new(0))
                .map(|step| step.note()),
            Some(48)
        );
        assert_eq!(
            initial_grid
                .get_step(TrackId::new(1), StepIndex::new(2))
                .map(|step| step.note()),
            Some(51)
        );
    }

    #[test]
    fn launch_control_ensemble_builds_lane_scene_bank_for_mixed_rendering() {
        let song = meldritch_dsl::load_song_directory(example_song_path(
            "songs/examples/17-launch-control-xl-ensemble",
        ))
        .expect("LaunchControl XL ensemble example should load");

        let bank = song_scene_bank(&song, tempo_from_song(&song).unwrap())
            .expect("LaunchControl XL ensemble scene bank should build");
        assert_eq!(song.note_patterns().len(), 36);
        assert_eq!(bank.patches.len(), 0);
        assert_eq!(bank.lane_patches.len(), 36);
        assert!(
            bank.lane_patches
                .contains_key(&("rhythm-drum-a".to_owned(), "rhythm-drum-a-b".to_owned()))
        );
        assert!(
            bank.lane_patches
                .values()
                .all(|patch| matches!(patch, CompiledSongPatch::Mixed(_)))
        );
        assert!(bank.phrase_variations.is_none());
    }

    #[test]
    fn lane_selection_results_identify_song_rerender_targets() {
        assert_eq!(
            song_lane_selection(&meldritch_app::AppCommandResult::LaneVariationSelected {
                lane_id: "rhythm-drum-a".to_owned(),
                previous: Some("rhythm-drum-a-a".to_owned()),
                current: "rhythm-drum-a-b".to_owned(),
            }),
            Some(("rhythm-drum-a".to_owned(), "rhythm-drum-a-b".to_owned()))
        );
        assert_eq!(
            song_lane_selection(&meldritch_app::AppCommandResult::LanePatternBankSelected {
                lane_id: "rhythm-drum-a".to_owned(),
                previous_bank: Some("groove".to_owned()),
                current_bank: "fill".to_owned(),
                previous_variation: Some("rhythm-drum-a-b".to_owned()),
                current_variation: "rhythm-drum-a-c".to_owned(),
            }),
            Some(("rhythm-drum-a".to_owned(), "rhythm-drum-a-c".to_owned()))
        );
        assert_eq!(
            song_lane_selection(&meldritch_app::AppCommandResult::LaneMuteToggled {
                lane_id: "rhythm-drum-a".to_owned(),
                muted: true,
            }),
            None
        );
    }

    #[test]
    fn song_audio_publication_is_independent_from_backing_coordinator() {
        let mut initial = meldritch_audio::AudioBlock::silent(1, 4);
        initial
            .samples_mut()
            .copy_from_slice(&[0.25, 0.5, 0.25, 0.5]);
        let (song_publisher, song_reader) = meldritch_audio::audio_publication::audio_publication(
            PublishedAudio::from_block(&initial, 2).unwrap(),
        );

        let (playback, _engine) =
            meldritch_audio::device_output::playback_session_parts(4).unwrap();
        let config = RenderCoordinatorConfig::new(1, 4, 2, 2, Duration::from_millis(1)).unwrap();
        let dummy_pattern = Pattern::new(PatternId::new(1), 4, 4).unwrap();
        let dummy_state = meldritch_render::coordinator::SampleRenderState::new(
            dummy_pattern,
            Tempo::new(120.0, 48_000).unwrap(),
            ProbabilitySeed::new(0),
            RenderSettings::new(1).unwrap(),
            Arc::new(BTreeMap::new()),
        );
        let coordinator =
            RenderCoordinator::new_from_state(config, dummy_state, playback.status_monitor())
                .unwrap();
        let _ = coordinator.wait_for_ready_chunks(2, Duration::from_millis(250));

        assert_eq!(song_reader.snapshot().frame(0).unwrap()[0], 0.25);
        assert_eq!(song_reader.snapshot().frame(1).unwrap()[0], 0.5);
        assert_ne!(
            coordinator.audio_reader().snapshot().frame(0).unwrap()[0],
            0.25
        );

        let mut rerender = meldritch_audio::AudioBlock::silent(1, 4);
        rerender
            .samples_mut()
            .copy_from_slice(&[0.75, 0.25, 0.75, 0.25]);
        publish_song_audio(&song_publisher, &rerender, 2).unwrap();
        assert_eq!(song_reader.snapshot().frame(0).unwrap()[0], 0.75);
        assert_eq!(song_reader.snapshot().frame(1).unwrap()[0], 0.25);
    }

    #[test]
    fn song_audio_debug_line_reports_publication_peak_near_playhead() {
        let mut block = meldritch_audio::AudioBlock::silent(1, 4);
        block.samples_mut().copy_from_slice(&[0.0, 0.0, 0.5, -0.25]);
        let (_, reader) = meldritch_audio::audio_publication::audio_publication(
            PublishedAudio::from_block(&block, 2).unwrap(),
        );
        let status = meldritch_audio::realtime_status::RealtimeStatusSnapshot {
            state: meldritch_audio::transport::TransportState::Playing,
            position: 0,
            callbacks: 7,
            stream_errors: 0,
            underruns: 0,
            missed_artifacts: 0,
        };

        let line = song_audio_debug_line(&reader, status);

        assert!(line.contains("Playing"));
        assert!(line.contains("callbacks 7"));
        assert!(line.contains("current 0.0000"));
        assert!(line.contains("next_peak 0.5000"));
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
