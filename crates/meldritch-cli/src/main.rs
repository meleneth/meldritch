use clap::{Parser, Subcommand};
use meldritch_core::{
    DirtyRange, EntityId, Event, EventTag, FrameRange, Pattern, SourceId, StepIndex,
};
use meldritch_render::{ArtifactCache, CacheStatus, RenderSettings};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
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
        Command::RenderControlledSamples {
            project,
            pattern_id,
            frames,
            channels,
            output,
            active_scale,
            normalize,
        } => render_controlled_samples(
            project,
            RenderControlledSamplesOptions {
                pattern_id,
                frames,
                channels,
                output,
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
        } => manifest_check(
            manifest,
            ManifestCheckOptions {
                pattern_id,
                sample_sources,
                relations,
                relation_kinds,
                finite,
                nonzero,
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
enum CompiledRelationKindSummary {
    SampleToPattern {
        note: u8,
        pattern_id: u64,
    },
    PatternControlsPattern {
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
enum RenderGraphRelationKindSummary {
    SampleToPattern {
        note: u8,
        pattern_id: u64,
    },
    PatternControlsPattern {
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
    active_scale: f64,
    normalize: bool,
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
    let settings = RenderSettings::new(options.channels)
        .map_err(|err| format!("invalid render settings: {err:?}"))?;
    let samples_by_note = load_project_samples(&project, &path)?;
    let range = FrameRange::new(0, options.frames).map_err(|err| err.to_string())?;
    let control = build_control_event_schedule(&path, options.pattern_id, options.frames)?;
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
