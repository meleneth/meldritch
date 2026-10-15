use std::{env, fs, process, thread, time::Duration};

use meldritch2::{
    compiler::compile,
    device::DeviceOutput,
    lua,
    model::{Graph, ParameterId},
    tui,
};

fn load_graph(path: &str) -> Result<Graph, String> {
    fs::read_to_string(path)
        .map_err(|error| error.to_string())
        .and_then(|script| lua::read(&script))
}

fn main() {
    let mut args = env::args().skip(1);
    let Some(first) = args.next() else {
        eprintln!(
            "usage: meldritch2 GRAPH.lua [--set=MODULE.PARAM=VALUE] | meldritch2 play GRAPH.lua SECONDS | meldritch2 perform GRAPH.lua"
        );
        process::exit(2);
    };
    if first == "play" {
        let Some(path) = args.next() else {
            eprintln!("play requires GRAPH.lua");
            process::exit(2);
        };
        let Some(seconds) = args.next() else {
            eprintln!("play requires SECONDS");
            process::exit(2);
        };
        if args.next().is_some() {
            eprintln!("too many arguments");
            process::exit(2);
        }
        let seconds: f64 = seconds.parse().unwrap_or(0.0);
        if !seconds.is_finite() || seconds <= 0.0 || seconds > 3600.0 {
            eprintln!("SECONDS must be finite and between 0 and 3600");
            process::exit(2);
        }
        let result = load_graph(&path).and_then(|graph| DeviceOutput::open_default(&graph));
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
        if args.next().is_some() {
            eprintln!("too many arguments");
            process::exit(2);
        }
        let result = load_graph(&path).and_then(|graph| DeviceOutput::open_default(&graph));
        match result {
            Ok(output) => {
                if let Err(error) = output.start().and_then(|_| tui::perform(&output)) {
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
