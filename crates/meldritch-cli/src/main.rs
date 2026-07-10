use clap::{Parser, Subcommand};
use meldritch_core::{FrameRange, Pattern, StepIndex};
use meldritch_render::{ArtifactCache, CacheStatus, RenderSettings};
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
    }

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
    let base_dir = project_path.parent().unwrap_or_else(|| Path::new("."));
    for sample_ref in project.samples() {
        let sample_path = PathBuf::from(sample_ref.path());
        let path = if sample_path.is_absolute() {
            sample_path
        } else {
            base_dir.join(sample_path)
        };
        let sample = meldritch_audio::read_wav(&path)
            .map_err(|err| format!("failed to read sample {}: {err}", path.display()))?;
        samples.insert(sample_ref.note(), sample);
    }

    Ok(samples)
}
