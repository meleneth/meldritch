use clap::{Parser, Subcommand};
use meldritch_core::FrameRange;
use meldritch_render::RenderSettings;
use std::collections::BTreeMap;
use std::path::PathBuf;

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
    RenderClicks {
        #[arg(value_name = "PROJECT")]
        project: PathBuf,
        #[arg(long, default_value_t = 96_000)]
        frames: u64,
        #[arg(long, default_value_t = 2)]
        channels: u16,
        #[arg(long, value_name = "WAV")]
        output: Option<PathBuf>,
        #[arg(long)]
        normalize: bool,
    },
    RenderSamples {
        #[arg(value_name = "PROJECT")]
        project: PathBuf,
        #[arg(long, default_value_t = 96_000)]
        frames: u64,
        #[arg(long, default_value_t = 2)]
        channels: u16,
        #[arg(long, value_name = "WAV")]
        output: Option<PathBuf>,
        #[arg(long)]
        normalize: bool,
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
        Command::RenderClicks {
            project,
            frames,
            channels,
            output,
            normalize,
        } => render_clicks(project, frames, channels, output, normalize),
        Command::RenderSamples {
            project,
            frames,
            channels,
            output,
            normalize,
        } => render_samples(project, frames, channels, output, normalize),
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

fn render_clicks(
    path: PathBuf,
    frames: u64,
    channels: u16,
    output: Option<PathBuf>,
    normalize: bool,
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
    let pattern = project
        .patterns()
        .first()
        .ok_or_else(|| "project has no patterns to render".to_owned())?;
    let settings =
        RenderSettings::new(channels).map_err(|err| format!("invalid render settings: {err:?}"))?;
    let mut block = meldritch_render::render_pattern_clicks(
        pattern,
        project.tempo(),
        FrameRange::new(0, frames).map_err(|err| err.to_string())?,
        project.probability_seed(),
        settings,
    );
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
    frames: u64,
    channels: u16,
    output: Option<PathBuf>,
    normalize: bool,
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
    let pattern = project
        .patterns()
        .first()
        .ok_or_else(|| "project has no patterns to render".to_owned())?;
    let settings =
        RenderSettings::new(channels).map_err(|err| format!("invalid render settings: {err:?}"))?;
    let samples_by_note = load_project_samples(&project)?;
    let mut block = meldritch_render::render_pattern_samples(
        pattern,
        project.tempo(),
        FrameRange::new(0, frames).map_err(|err| err.to_string())?,
        project.probability_seed(),
        settings,
        &samples_by_note,
    );
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

fn load_project_samples(
    project: &meldritch_dsl::ValidatedProject,
) -> Result<BTreeMap<u8, meldritch_audio::SampleBuffer>, String> {
    let mut samples = BTreeMap::new();
    for sample_ref in project.samples() {
        let path = PathBuf::from(sample_ref.path());
        let sample = meldritch_audio::read_wav(&path)
            .map_err(|err| format!("failed to read sample {}: {err}", path.display()))?;
        samples.insert(sample_ref.note(), sample);
    }

    Ok(samples)
}
