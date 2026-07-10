use clap::{Parser, Subcommand};
use meldritch_core::{
    DirtyRange, EntityId, Event, EventTag, FrameRange, Pattern, SourceId, StepIndex,
};
use meldritch_render::{ArtifactCache, CacheStatus, RenderSettings};
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Parser)]
#[command(name = "meldritch")]
#[command(about = "Headless Meldritch project tooling")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Validate {
        #[arg(value_name = "PROJECT")]
        project: PathBuf,
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
        #[arg(long)]
        normalize: bool,
        #[arg(long)]
        cache_probe: bool,
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
        Command::Inspect { project } => inspect_project(project),
        Command::SummaryJson { project } => summarize_project_json(project),
        Command::GraphJson { project } => graph_json(project),
        Command::SamplesJson { project } => samples_json(project),
        Command::EventsJson {
            project,
            pattern_id,
            frames,
        } => events_json(project, pattern_id, frames),
        Command::DirtyJson {
            project,
            source_id,
            start,
            end,
        } => dirty_json(project, source_id, start, end),
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
            normalize,
            cache_probe,
        } => render_samples(
            project,
            pattern_id,
            frames,
            channels,
            output,
            normalize,
            cache_probe,
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

#[derive(Debug, Serialize)]
struct ProjectSummary {
    schema_version: u32,
    name: String,
    tempo: TempoSummary,
    samples: Vec<SampleSummary>,
    patterns: Vec<PatternSummary>,
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
enum CompiledRelationKindSummary {
    SampleToPattern { note: u8, pattern_id: u64 },
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

    if let Some(output) = output {
        meldritch_audio::write_wav_f32(&output, &block, project.tempo().sample_rate())
            .map_err(|err| format!("failed to write {}: {err}", output.display()))?;
        println!("wrote: {}", output.display());
    }

    Ok(())
}

fn render_samples(
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
    let samples_by_note = load_project_samples(&project, &path)?;
    let range = FrameRange::new(0, frames).map_err(|err| err.to_string())?;
    let mut block = if cache_probe {
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
        "rendered samples: frames={}, channels={}, finite={}, nonzero_samples={}, peak={}",
        block.frames(),
        block.channels(),
        finite,
        nonzero_samples,
        peak
    );

    if let Some(output) = output {
        meldritch_audio::write_wav_f32(&output, &block, project.tempo().sample_rate())
            .map_err(|err| format!("failed to write {}: {err}", output.display()))?;
        println!("wrote: {}", output.display());
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
