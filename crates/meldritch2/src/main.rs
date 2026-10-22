use std::{
    env, fs,
    io::{self, Write},
    process,
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

use meldritch2::{
    compiler::{OfflineRenderer, compile, compile_at_rate},
    device::{DeviceOutput, output_device_names},
    lua,
    midi_device::{MidiInputSession, MidiMonitorSession, describe_message, input_port_names},
    model::{Graph, ParameterId},
    tui,
};

#[derive(Clone, Copy)]
enum RenderMode {
    Auto { max_workers: usize },
    Scalar,
    Block { workers: usize },
}

fn render_wav(graph: &Graph, path: &str, seconds: f64, mode: RenderMode) -> Result<(), String> {
    const RATE: u32 = 48_000;
    const BLOCK: usize = 1_024;
    let frames = (seconds * f64::from(RATE)).ceil() as usize;
    let preparation_started = Instant::now();
    let machine = compile_at_rate(graph, f64::from(RATE))?;
    let preparation = preparation_started.elapsed();
    let mode = match mode {
        // Small graphs benchmark faster on the scalar reference. Larger
        // graphs expose enough independent branches to amortize barriers.
        RenderMode::Auto { max_workers } if machine.order().len() >= 32 && max_workers > 1 => {
            RenderMode::Block {
                workers: max_workers.min(4),
            }
        }
        RenderMode::Auto { .. } => RenderMode::Scalar,
        selected => selected,
    };
    let (mut scalar, mut block) = match mode {
        RenderMode::Scalar => (Some(machine), None),
        RenderMode::Block { workers } => {
            (None, Some(OfflineRenderer::new(machine, BLOCK, workers)?))
        }
        RenderMode::Auto { .. } => unreachable!("automatic mode was resolved above"),
    };
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: RATE,
        bits_per_sample: 24,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(path, spec).map_err(|e| e.to_string())?;
    let mut left = vec![0.0; BLOCK];
    let mut right = vec![0.0; BLOCK];
    let mut remaining = frames;
    let render_started = Instant::now();
    let mut processing = Duration::ZERO;
    let mut encoding = Duration::ZERO;
    while remaining > 0 {
        let count = remaining.min(left.len());
        let processing_started = Instant::now();
        match (&mut scalar, &mut block) {
            (Some(machine), None) => {
                machine.process_stereo(&mut left[..count], &mut right[..count])
            }
            (None, Some(renderer)) => {
                renderer.process_stereo(&mut left[..count], &mut right[..count])
            }
            _ => unreachable!("exactly one offline execution mode is active"),
        }
        processing += processing_started.elapsed();
        let encoding_started = Instant::now();
        for (&l, &r) in left[..count].iter().zip(&right[..count]) {
            let scale = 8_388_607.0;
            writer
                .write_sample((l.clamp(-1.0, 1.0) * scale).round() as i32)
                .map_err(|e| e.to_string())?;
            writer
                .write_sample((r.clamp(-1.0, 1.0) * scale).round() as i32)
                .map_err(|e| e.to_string())?;
        }
        encoding += encoding_started.elapsed();
        remaining -= count;
    }
    let render_time = render_started.elapsed();
    let finalize_started = Instant::now();
    writer.finalize().map_err(|e| e.to_string())?;
    let finalize = finalize_started.elapsed();
    let (executor, workers, modules, levels, width) = match (&scalar, &block) {
        (Some(machine), None) => ("scalar", 1, machine.order().len(), 0, 1),
        (None, Some(renderer)) => (
            "block",
            renderer.workers(),
            renderer.module_count(),
            renderer.dependency_levels(),
            renderer.maximum_parallel_width(),
        ),
        _ => unreachable!(),
    };
    eprintln!(
        "render profile: executor={executor} workers={workers} modules={modules} levels={levels} max_width={width} block={BLOCK} prepare={:.3}s processing={:.3}s encoding={:.3}s finalize={:.3}s total_loop={:.3}s throughput={:.2}x",
        preparation.as_secs_f64(),
        processing.as_secs_f64(),
        encoding.as_secs_f64(),
        finalize.as_secs_f64(),
        render_time.as_secs_f64(),
        seconds / render_time.as_secs_f64()
    );
    if let Some(renderer) = &block {
        let profile = renderer.profile();
        eprintln!(
            "render profile: musical_scheduling={:.3}s dependency_graph_dsp_and_mix={:.3}s blocks={} frames={}",
            profile.sequencing.as_secs_f64(),
            profile.graph_execution.as_secs_f64(),
            profile.blocks,
            profile.frames
        );
    }
    Ok(())
}

fn load_graph(path: &str) -> Result<Graph, String> {
    let path = std::path::Path::new(path);
    fs::read_to_string(path)
        .map_err(|error| error.to_string())
        .and_then(|script| {
            lua::read_with_root(&script, path.parent().unwrap_or(std::path::Path::new(".")))
        })
}

fn load_program(path: &str) -> Result<lua::Program, String> {
    let path = std::path::Path::new(path);
    fs::read_to_string(path)
        .map_err(|error| error.to_string())
        .and_then(|script| {
            lua::read_program_with_root(&script, path.parent().unwrap_or(std::path::Path::new(".")))
        })
}

fn open_output(graph: &Graph, device_arg: Option<String>) -> Result<DeviceOutput, String> {
    match device_arg {
        None => match graph.devices.audio_output.as_deref() {
            Some(name) => DeviceOutput::open_name(graph, name),
            None => DeviceOutput::open_default(graph),
        },
        Some(argument) => {
            let index = argument
                .strip_prefix("--device=")
                .ok_or("expected --device=INDEX")?
                .parse::<usize>()
                .map_err(|_| "device index must be a nonnegative integer")?;
            DeviceOutput::open_index(graph, index)
        }
    }
}

fn midi_sniff(port: &str, seconds: f64) -> Result<(), String> {
    if !seconds.is_finite() || seconds <= 0.0 || seconds > 3600.0 {
        return Err("midi-sniff seconds must be finite and between 0 and 3600".into());
    }
    let (sender, receiver) = mpsc::channel();
    let session = match port.parse::<usize>() {
        Ok(index) => MidiMonitorSession::open_index(index, sender)?,
        Err(_) => MidiMonitorSession::open_name(port, sender)?,
    };
    println!(
        "capturing '{}' for {seconds:.1}s; press and release one control at a time",
        session.port_name
    );
    io::stdout().flush().map_err(|error| error.to_string())?;
    let started = Instant::now();
    let duration = Duration::from_secs_f64(seconds);
    while let Some(remaining) = duration.checked_sub(started.elapsed()) {
        match receiver.recv_timeout(remaining) {
            Ok(message) => println!(
                "{:>12} us  {}",
                message.timestamp_us,
                describe_message(&message.bytes)
            ),
            Err(mpsc::RecvTimeoutError::Timeout) => break,
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                return Err("MIDI monitor disconnected unexpectedly".into());
            }
        }
    }
    drop(session);
    Ok(())
}

fn main() {
    let mut args = env::args().skip(1);
    let Some(first) = args.next() else {
        eprintln!(
            "usage: meldritch2 GRAPH.lua [--set=MODULE.PARAM=VALUE] | meldritch2 devices | meldritch2 midi-ports | meldritch2 midi-sniff PORT [SECONDS] | meldritch2 render GRAPH.lua OUTPUT.wav SECONDS [--workers=N|--scalar] | meldritch2 play GRAPH.lua SECONDS [--device=INDEX] | meldritch2 perform GRAPH.lua [--device=INDEX] [--midi=INDEX]\nAudio and controller names may instead be configured in Lua with graph:devices {{ audio_output = NAME, midi_input = NAME, midi_controls = {{ NAME, ... }}, apc_mini = NAME }}."
        );
        process::exit(2);
    };
    if first == "devices" {
        if args.next().is_some() {
            eprintln!("devices takes no arguments");
            process::exit(2);
        }
        match output_device_names() {
            Ok(devices) => {
                for (index, name) in devices.iter().enumerate() {
                    println!("{index}: {name}");
                }
            }
            Err(error) => {
                eprintln!("{error}");
                process::exit(1);
            }
        }
        return;
    }
    if first == "midi-ports" {
        if args.next().is_some() {
            eprintln!("midi-ports takes no arguments");
            process::exit(2);
        }
        match input_port_names() {
            Ok(ports) => {
                for (index, name) in ports.iter().enumerate() {
                    println!("{index}: {name}");
                }
            }
            Err(error) => {
                eprintln!("{error}");
                process::exit(1);
            }
        }
        return;
    }
    if first == "midi-sniff" {
        let Some(port) = args.next() else {
            eprintln!("midi-sniff requires a port index or unique name");
            process::exit(2);
        };
        let seconds = args
            .next()
            .map(|value| value.parse::<f64>())
            .transpose()
            .unwrap_or_else(|_| {
                eprintln!("midi-sniff seconds must be a number");
                process::exit(2);
            })
            .unwrap_or(30.0);
        if args.next().is_some() {
            eprintln!("midi-sniff accepts only PORT and optional SECONDS");
            process::exit(2);
        }
        if let Err(error) = midi_sniff(&port, seconds) {
            eprintln!("{error}");
            process::exit(1);
        }
        return;
    }
    if first == "play" {
        let Some(path) = args.next() else {
            eprintln!("play requires GRAPH.lua");
            process::exit(2);
        };
        let Some(seconds) = args.next() else {
            eprintln!("play requires SECONDS");
            process::exit(2);
        };
        let device_arg = args.next();
        if args.next().is_some() {
            eprintln!("too many arguments");
            process::exit(2);
        }
        let seconds: f64 = seconds.parse().unwrap_or(0.0);
        if !seconds.is_finite() || seconds <= 0.0 || seconds > 3600.0 {
            eprintln!("SECONDS must be finite and between 0 and 3600");
            process::exit(2);
        }
        let result = load_graph(&path).and_then(|graph| open_output(&graph, device_arg));
        match result {
            Ok(output) => {
                if let Err(error) = output.start() {
                    eprintln!("{error}");
                    process::exit(1);
                }
                println!(
                    "playing on {} at {} Hz ({} channels)",
                    output.device_name, output.sample_rate, output.channels
                );
                thread::sleep(Duration::from_secs_f64(seconds));
                if output.stream_errors() > 0 {
                    eprintln!(
                        "output stream reported {} errors: {}",
                        output.stream_errors(),
                        output.first_stream_error().unwrap_or("unknown error")
                    );
                    process::exit(1);
                }
            }
            Err(error) => {
                eprintln!("{error}");
                process::exit(1);
            }
        }
        return;
    }
    if first == "render" {
        let Some(source) = args.next() else {
            eprintln!("render requires GRAPH.lua");
            process::exit(2)
        };
        let Some(output) = args.next() else {
            eprintln!("render requires OUTPUT.wav");
            process::exit(2)
        };
        let seconds = args
            .next()
            .and_then(|v| v.parse::<f64>().ok())
            .unwrap_or(0.0);
        let option = args.next();
        if args.next().is_some() || !seconds.is_finite() || seconds <= 0.0 || seconds > 3600.0 {
            eprintln!("render requires finite SECONDS between 0 and 3600");
            process::exit(2);
        }
        let mode = match option.as_deref() {
            Some("--scalar") => RenderMode::Scalar,
            Some(value) if value.starts_with("--workers=") => {
                let workers = value["--workers=".len()..]
                    .parse::<usize>()
                    .ok()
                    .filter(|workers| *workers > 0)
                    .unwrap_or_else(|| {
                        eprintln!("--workers requires a positive integer");
                        process::exit(2);
                    });
                RenderMode::Block { workers }
            }
            Some(value) => {
                eprintln!("unknown render option '{value}'");
                process::exit(2);
            }
            None => RenderMode::Auto {
                max_workers: thread::available_parallelism().map_or(1, usize::from),
            },
        };
        let load_started = Instant::now();
        match load_graph(&source).and_then(|graph| {
            eprintln!(
                "render profile: Lua/project load={:.3}s",
                load_started.elapsed().as_secs_f64()
            );
            render_wav(&graph, &output, seconds, mode)
        }) {
            Ok(()) => println!("rendered {seconds:.3}s to {output}"),
            Err(error) => {
                eprintln!("{error}");
                process::exit(1);
            }
        }
        return;
    }
    if first == "perform" {
        let Some(path) = args.next() else {
            eprintln!("perform requires GRAPH.lua");
            process::exit(2);
        };
        let mut device_arg = None;
        let mut midi_index = None;
        for argument in args {
            if argument.starts_with("--device=") {
                if device_arg.replace(argument).is_some() {
                    eprintln!("--device may appear only once");
                    process::exit(2);
                }
            } else if let Some(index) = argument.strip_prefix("--midi=") {
                let index = index.parse::<usize>().unwrap_or_else(|_| {
                    eprintln!("MIDI input index must be a nonnegative integer");
                    process::exit(2);
                });
                if midi_index.replace(index).is_some() {
                    eprintln!("--midi may appear only once");
                    process::exit(2);
                }
            } else {
                eprintln!("unknown perform option '{argument}'");
                process::exit(2);
            }
        }
        let result = load_program(&path).and_then(|program| {
            let configured_midi = program.graph().devices.midi_input.clone();
            let configured_controls = program.graph().devices.midi_controls.clone();
            open_output(program.graph(), device_arg)
                .map(|output| (output, configured_midi, configured_controls, program))
        });
        match result {
            Ok((mut output, configured_midi, configured_controls, mut program)) => {
                let midi_session = match (midi_index, configured_midi) {
                    (Some(index), _) => {
                        let opened = match output.take_timed_midi_router() {
                            Some(router) => MidiInputSession::open_timed_index(index, router),
                            None => MidiInputSession::open_index(index, output.midi_router()),
                        };
                        match opened {
                            Ok(session) => Some(session),
                            Err(error) => {
                                eprintln!("{error}");
                                process::exit(1);
                            }
                        }
                    }
                    (None, Some(name)) => {
                        let opened = match output.take_timed_midi_router() {
                            Some(router) => MidiInputSession::open_timed_name(&name, router),
                            None => MidiInputSession::open_name(&name, output.midi_router()),
                        };
                        match opened {
                            Ok(session) => Some(session),
                            Err(error) => {
                                eprintln!("{error}");
                                process::exit(1);
                            }
                        }
                    }
                    (None, None) => None,
                };
                let mut control_sessions = Vec::new();
                for name in configured_controls {
                    match MidiInputSession::open_name(&name, output.midi_router()) {
                        Ok(session) => control_sessions.push(session),
                        Err(error) => {
                            eprintln!("{error}");
                            process::exit(1);
                        }
                    }
                }
                let keyboard = if midi_session.is_none() {
                    output
                        .take_timed_midi_router()
                        .and_then(tui::TuiKeyboard::new)
                } else {
                    None
                };
                if let Err(error) = output.start().and_then(|_| {
                    tui::perform(
                        &output,
                        midi_session.as_ref(),
                        keyboard,
                        Some(program.events_mut()),
                    )
                }) {
                    eprintln!("{error}");
                    process::exit(1);
                }
                if output.stream_errors() > 0 {
                    eprintln!(
                        "output stream reported {} errors: {}",
                        output.stream_errors(),
                        output.first_stream_error().unwrap_or("unknown error")
                    );
                    process::exit(1);
                }
                if output.timed_note_errors() > 0 {
                    eprintln!(
                        "timed note output reported {} errors",
                        output.timed_note_errors()
                    );
                    process::exit(1);
                }
                if let Some(session) = &midi_session {
                    if session.note_errors() > 0 {
                        eprintln!(
                            "MIDI note input reported {} timing or queue errors",
                            session.note_errors()
                        );
                        process::exit(1);
                    }
                }
                for session in &control_sessions {
                    if session.note_errors() > 0 {
                        eprintln!(
                            "MIDI control input '{}' reported {} errors",
                            session.port_name,
                            session.note_errors()
                        );
                        process::exit(1);
                    }
                }
                drop(midi_session);
            }
            Err(error) => {
                eprintln!("{error}");
                process::exit(1);
            }
        }
        return;
    }
    let path = first;
    let update = args.next();
    if args.next().is_some() {
        eprintln!("too many arguments");
        process::exit(2);
    }
    let result = load_graph(&path).and_then(|graph| compile(&graph));
    match result {
        Ok(mut machine) => {
            let mut samples = [0.0; 8];
            machine.process(&mut samples);
            println!("order: {}", machine.order().join(" -> "));
            if !machine.authored_normalizations().is_empty() {
                for edge in machine.authored_normalizations() {
                    println!(
                        "normalization: {}.{} -> {}.{}",
                        edge.from.module, edge.from.port, edge.to.module, edge.to.port
                    );
                }
                for edge in machine.effective_connections() {
                    println!(
                        "effective: {}.{} -> {}.{}",
                        edge.from.module, edge.from.port, edge.to.module, edge.to.port
                    );
                }
            }
            println!("samples: {samples:?}");
            if let Some(update) = update {
                let result = update.strip_prefix("--set=");
                let Some(assignment) = result else {
                    eprintln!("expected --set=MODULE.PARAM=VALUE");
                    process::exit(2);
                };
                let (name, value) = assignment.split_once('=').unwrap_or_else(|| {
                    eprintln!("expected --set=MODULE.PARAM=VALUE");
                    process::exit(2);
                });
                let (module, param) = name.split_once('.').unwrap_or_else(|| {
                    eprintln!("expected MODULE.PARAM");
                    process::exit(2);
                });
                let value: f64 = value.parse().unwrap_or_else(|_| {
                    eprintln!("value must be a finite number");
                    process::exit(2);
                });
                if let Err(error) =
                    machine.set_override(&ParameterId::new(module, param), Some(value))
                {
                    eprintln!("{error}");
                    process::exit(1);
                }
                machine.process(&mut samples);
                println!("after update: {samples:?}");
            }
        }
        Err(error) => {
            eprintln!("{error}");
            process::exit(1);
        }
    }
}
