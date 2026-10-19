use std::{env, fs, process, thread, time::Duration};

use meldritch2::{
    compiler::compile,
    device::{DeviceOutput, output_device_names},
    lua,
    midi_device::{MidiInputSession, input_port_names},
    model::{Graph, ParameterId},
    tui,
};

fn load_graph(path: &str) -> Result<Graph, String> {
    fs::read_to_string(path)
        .map_err(|error| error.to_string())
        .and_then(|script| lua::read(&script))
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

fn main() {
    let mut args = env::args().skip(1);
    let Some(first) = args.next() else {
        eprintln!(
            "usage: meldritch2 GRAPH.lua [--set=MODULE.PARAM=VALUE] | meldritch2 devices | meldritch2 midi-ports | meldritch2 play GRAPH.lua SECONDS [--device=INDEX] | meldritch2 perform GRAPH.lua [--device=INDEX] [--midi=INDEX]\nAudio and MIDI names may instead be configured in the Lua script with graph:devices {{ audio_output = NAME, midi_input = NAME }}."
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
        let result = load_graph(&path).and_then(|graph| {
            let configured_midi = graph.devices.midi_input.clone();
            open_output(&graph, device_arg).map(|output| (output, configured_midi))
        });
        match result {
            Ok((mut output, configured_midi)) => {
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
                let keyboard = if midi_session.is_none() {
                    output
                        .take_timed_midi_router()
                        .and_then(tui::TuiKeyboard::new)
                } else {
                    None
                };
                if let Err(error) = output
                    .start()
                    .and_then(|_| tui::perform(&output, midi_session.as_ref(), keyboard))
                {
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
