use std::{env, fs, process};

use meldritch2::{compiler::compile, lua, model::ParameterId};

fn main() {
    let mut args = env::args().skip(1);
    let Some(path) = args.next() else {
        eprintln!("usage: meldritch2 GRAPH.lua [--set MODULE.PARAM=VALUE]");
        process::exit(2);
    };
    let update = args.next();
    if args.next().is_some() {
        eprintln!("too many arguments");
        process::exit(2);
    }
    let result = fs::read_to_string(&path)
        .map_err(|e| e.to_string())
        .and_then(|script| lua::read(&script))
        .and_then(|graph| compile(&graph));
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
