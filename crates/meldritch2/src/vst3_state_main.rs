use std::{
    io::{self, BufRead, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex, mpsc},
    time::Duration,
};

use meldritch2::vst3::{self, PatchBank, PatchEntry, PluginState};
use sha2::{Digest, Sha256};
use vst3_host::{StateContext, Vst3Host, audio::AudioBuffers, window::PluginWindow};

fn usage() -> ! {
    eprintln!(
        "usage:\n  meldritch-vst3-state investigate PLUGIN [PLUGIN_PATH] [NATIVE_PRESET]\n  meldritch-vst3-state capture PLUGIN BANK_DIRECTORY [PLUGIN_PATH]\n  meldritch-vst3-state subset SOURCE_BANK DESTINATION_BANK PATCH_ID...\n\nPLUGIN is a class ID or unique display name. capture opens the native editor; select a preset, then enter `capture ID NAME`, repeating as needed."
    );
    std::process::exit(2)
}

fn paths(argument: Option<String>) -> Vec<PathBuf> {
    argument
        .map(|value| std::env::split_paths(&value).collect())
        .unwrap_or_default()
}

fn load_plugin(
    selector: &str,
    paths: &[PathBuf],
) -> Result<(vst3::Descriptor, vst3_host::Plugin), String> {
    let descriptor = vst3::resolve(selector, paths)?;
    let mut builder = Vst3Host::builder()
        .sample_rate(48_000.0)
        .block_size(vst3::MAX_BLOCK_SIZE)
        .input_channels(if descriptor.instrument { 0 } else { 2 })
        .output_channels(2)
        .with_process_isolation(false);
    for path in paths {
        builder = builder.add_scan_path(path);
    }
    let mut host = builder.build().map_err(|error| error.to_string())?;
    let plugin = host
        .load_plugin_class(&descriptor.path, &descriptor.class_id)
        .map_err(|error| error.to_string())?;
    Ok((descriptor, plugin))
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn component_digest(state: &[u8]) -> Result<String, String> {
    const MAGIC: &[u8; 16] = b"VST3HOST_STATE\0\0";
    if !state.starts_with(MAGIC) {
        return Ok(digest(state));
    }
    if state.len() < 28 {
        return Err("truncated vst3-host state snapshot".into());
    }
    let length = u32::from_le_bytes(state[20..24].try_into().unwrap()) as usize;
    let end = 28usize
        .checked_add(length)
        .ok_or("component state size overflow")?;
    let component = state.get(28..end).ok_or("truncated component state")?;
    Ok(digest(component))
}

fn investigate(
    selector: &str,
    paths: &[PathBuf],
    native_preset: Option<&Path>,
) -> Result<(), String> {
    let (descriptor, mut plugin) = load_plugin(selector, paths)?;
    println!(
        "plugin: {} {} by {}",
        descriptor.name, descriptor.version, descriptor.vendor
    );
    println!("class-id: {}", descriptor.class_id);
    println!("native-editor: {}", plugin.has_editor());
    let baseline = plugin
        .save_state()
        .map_err(|error| format!("state capture failed: {error}"))?;
    let state = PluginState::capture(
        &descriptor,
        "Investigation baseline".into(),
        baseline.clone(),
    )?;
    println!(
        "state: component={} controller={} snapshot_sha256={}",
        state.metadata.component_state_bytes,
        state
            .metadata
            .controller_state_bytes
            .map_or_else(|| "unsupported".into(), |size| size.to_string()),
        digest(&baseline)
    );
    let units = plugin.get_units().map_err(|error| error.to_string())?;
    if units.is_empty() {
        println!("program-lists: IUnitInfo unavailable or empty");
    }
    for unit in units {
        println!(
            "unit: id={} name={:?} program-list={:?} programs={}",
            unit.id,
            unit.name,
            unit.program_list_id,
            unit.programs.len()
        );
        for (index, name) in unit.programs.iter().enumerate().take(8) {
            println!("  {index}: {name}");
        }
        if !unit.programs.is_empty() {
            if let Some(program_list_id) = unit.program_list_id {
                match plugin.get_program_data(program_list_id, 0) {
                    Ok(Some(data)) => println!(
                        "  IProgramListData[0]: {} bytes sha256={}",
                        data.len(),
                        digest(&data)
                    ),
                    Ok(None) => println!("  IProgramListData: unavailable or unsupported"),
                    Err(error) => println!("  IProgramListData[0]: rejected: {error}"),
                }
            }
            for index in [0usize, 1, unit.programs.len().saturating_sub(1)] {
                if index >= unit.programs.len() {
                    continue;
                }
                plugin
                    .select_program(unit.id, index as i32)
                    .map_err(|error| error.to_string())?;
                let selected = plugin.save_state().map_err(|error| error.to_string())?;
                println!("  selected {index}: state_sha256={}", digest(&selected));
            }
            plugin
                .load_state(&baseline)
                .map_err(|error| error.to_string())?;
        }
    }
    if let Some(path) = native_preset {
        let bytes = std::fs::read(path).map_err(|error| error.to_string())?;
        match plugin.load_state_with_context(&bytes, &StateContext::preset_from_path(path)) {
            Ok(()) => {
                let loaded = plugin.save_state().map_err(|error| error.to_string())?;
                println!(
                    "native-preset-direct-load: accepted; state_sha256={}",
                    digest(&loaded)
                );
            }
            Err(error) => println!("native-preset-direct-load: rejected: {error}"),
        }
        plugin
            .load_state(&baseline)
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn capture(selector: &str, directory: &Path, paths: &[PathBuf]) -> Result<(), String> {
    let (descriptor, mut plugin) = load_plugin(selector, paths)?;
    let initial_component =
        component_digest(&plugin.save_state().map_err(|error| error.to_string())?)?;
    plugin
        .start_processing()
        .map_err(|error| error.to_string())?;
    let plugin = Arc::new(Mutex::new(plugin));
    let mut buffers = AudioBuffers::new(0, 2, 1024, 48_000.0);
    let mut window = PluginWindow::new(Arc::clone(&plugin));
    window
        .open()
        .map_err(|error| format!("failed to open native editor: {error}"))?;
    let mut bank = if directory.join("manifest.json").is_file() {
        let bytes =
            std::fs::read(directory.join("manifest.json")).map_err(|error| error.to_string())?;
        serde_json::from_slice::<PatchBank>(&bytes).map_err(|error| error.to_string())?
    } else {
        PatchBank::new(&descriptor)
    };
    println!("Native editor opened. Select an authentic preset in Serum.");
    println!("Commands: capture ID NAME | list | quit");
    print!("> ");
    io::stdout().flush().map_err(|error| error.to_string())?;
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        for line in io::stdin().lock().lines() {
            if sender.send(line.unwrap_or_default()).is_err() {
                break;
            }
        }
    });
    loop {
        window
            .service_platform_events()
            .map_err(|error| error.to_string())?;
        {
            let mut plugin = plugin.lock().unwrap_or_else(|poison| poison.into_inner());
            plugin.service_run_loop();
            plugin
                .process_audio(&mut buffers)
                .map_err(|error| error.to_string())?;
            let flags = plugin
                .service_host_requests()
                .map_err(|error| error.to_string())?;
            if flags.reload_component() {
                return Err("plugin requested component reload while selecting a preset".into());
            }
        }
        let Ok(line) = receiver.recv_timeout(Duration::from_millis(16)) else {
            continue;
        };
        let line = line.trim();
        if line == "quit" || line == "exit" {
            break;
        }
        if line == "list" {
            for patch in &bank.patches {
                println!("{}\t{}", patch.id, patch.name);
            }
        } else if let Some(arguments) = line.strip_prefix("capture ") {
            let Some((id, name)) = arguments.split_once(' ') else {
                println!("capture requires an ID and a name");
                continue;
            };
            if id.is_empty()
                || !id
                    .chars()
                    .all(|character| character.is_ascii_alphanumeric() || character == '_')
            {
                println!("ID must contain only ASCII letters, digits, and underscores");
                continue;
            }
            let bytes = {
                let mut plugin = plugin.lock().unwrap_or_else(|poison| poison.into_inner());
                plugin
                    .stop_processing()
                    .map_err(|error| error.to_string())?;
                let state = plugin.save_state().map_err(|error| error.to_string())?;
                plugin
                    .start_processing()
                    .map_err(|error| error.to_string())?;
                state
            };
            let mut state = PluginState::capture(&descriptor, name.trim().to_owned(), bytes)?;
            let captured_component = component_digest(&state.state)?;
            if captured_component == initial_component {
                println!(
                    "refused {id}: processor state is still Serum Init; load the factory preset and try again"
                );
                print!("> ");
                io::stdout().flush().map_err(|error| error.to_string())?;
                continue;
            }
            let duplicate = bank.patches.iter().find_map(|patch| {
                let bytes = std::fs::read(directory.join(&patch.file)).ok()?;
                let existing = PluginState::from_bytes(&bytes).ok()?;
                (component_digest(&existing.state).ok()? == captured_component)
                    .then(|| patch.id.clone())
            });
            if let Some(existing) = duplicate.filter(|existing| existing != id) {
                println!(
                    "refused {id}: processor state is byte-identical to existing patch {existing}"
                );
                print!("> ");
                io::stdout().flush().map_err(|error| error.to_string())?;
                continue;
            }
            state.metadata.source = Some(serde_json::json!({"mechanism":"native-editor"}));
            let file = PathBuf::from(format!("{id}.meldritch-vst3-state"));
            std::fs::create_dir_all(directory).map_err(|error| error.to_string())?;
            let temporary = directory.join(format!(".{id}.meldritch-vst3-state.tmp"));
            std::fs::write(&temporary, state.to_bytes()?).map_err(|error| error.to_string())?;
            std::fs::rename(temporary, directory.join(&file)).map_err(|error| error.to_string())?;
            let entry = PatchEntry {
                id: id.into(),
                name: name.trim().into(),
                file,
                source: state.metadata.source.clone(),
            };
            if let Some(existing) = bank.patches.iter_mut().find(|patch| patch.id == id) {
                *existing = entry;
            } else {
                bank.patches.push(entry);
            }
            // A bank becomes loadable at two entries, but keeping an in-progress manifest is useful.
            bank.write(directory)?;
            println!(
                "captured {id}: {} (component={} controller={})",
                name.trim(),
                state.metadata.component_state_bytes,
                state
                    .metadata
                    .controller_state_bytes
                    .map_or_else(|| "unsupported".into(), |size| size.to_string())
            );
        } else {
            println!("unknown command; use capture ID NAME, list, or quit");
        }
        print!("> ");
        io::stdout().flush().map_err(|error| error.to_string())?;
    }
    window.close();
    if bank.patches.len() >= 2 {
        bank.validate(directory, &descriptor)?;
    }
    Ok(())
}

fn subset(source: &Path, destination: &Path, ids: &[String]) -> Result<(), String> {
    if ids.len() < 2 {
        return Err("a subset bank requires at least two patch IDs".into());
    }
    let bytes = std::fs::read(source.join("manifest.json")).map_err(|error| error.to_string())?;
    let source_bank: PatchBank =
        serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
    let mut bank = PatchBank {
        format_version: source_bank.format_version,
        plugin: source_bank.plugin.clone(),
        patches: Vec::new(),
    };
    std::fs::create_dir_all(destination).map_err(|error| error.to_string())?;
    for id in ids {
        let patch = source_bank
            .patches
            .iter()
            .find(|patch| &patch.id == id)
            .ok_or_else(|| format!("source bank has no patch '{id}'"))?;
        std::fs::copy(source.join(&patch.file), destination.join(&patch.file))
            .map_err(|error| format!("failed to copy patch '{id}': {error}"))?;
        bank.patches.push(patch.clone());
    }
    bank.write(destination)
}

fn main() {
    let mut arguments = std::env::args().skip(1);
    let Some(command) = arguments.next() else {
        usage()
    };
    let result = match command.as_str() {
        "investigate" => {
            let Some(plugin) = arguments.next() else {
                usage()
            };
            let paths = paths(arguments.next());
            let preset = arguments.next().map(PathBuf::from);
            if arguments.next().is_some() {
                usage()
            }
            investigate(&plugin, &paths, preset.as_deref())
        }
        "capture" => {
            let Some(plugin) = arguments.next() else {
                usage()
            };
            let Some(directory) = arguments.next().map(PathBuf::from) else {
                usage()
            };
            let paths = paths(arguments.next());
            if arguments.next().is_some() {
                usage()
            }
            capture(&plugin, &directory, &paths)
        }
        "subset" => {
            let Some(source) = arguments.next().map(PathBuf::from) else {
                usage()
            };
            let Some(destination) = arguments.next().map(PathBuf::from) else {
                usage()
            };
            let ids = arguments.collect::<Vec<_>>();
            subset(&source, &destination, &ids)
        }
        _ => usage(),
    };
    if let Err(error) = result {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
