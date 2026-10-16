use crate::midi::MidiRouter;
use crate::midi::TimedMidiRouter;
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

pub fn input_port_names() -> Result<Vec<String>, String> {
    let input = midir::MidiInput::new("meldritch2-port-list")
        .map_err(|error| format!("failed to create MIDI input: {error}"))?;
    Ok(input
        .ports()
        .iter()
        .map(|port| {
            input
                .port_name(port)
                .unwrap_or_else(|_| "unknown MIDI input".into())
        })
        .collect())
}

pub struct MidiInputSession {
    _connection: midir::MidiInputConnection<()>,
    note_errors: Arc<AtomicU64>,
    pub port_name: String,
}

impl MidiInputSession {
    pub fn open_index(index: usize, router: MidiRouter) -> Result<Self, String> {
        let mut input = midir::MidiInput::new("meldritch2-control")
            .map_err(|error| format!("failed to create MIDI input: {error}"))?;
        input.ignore(midir::Ignore::None);
        let ports = input.ports();
        let port = ports
            .get(index)
            .ok_or_else(|| format!("MIDI input index {index} does not exist"))?;
        let port_name = input
            .port_name(port)
            .unwrap_or_else(|_| "unknown MIDI input".into());
        let connection = input
            .connect(
                port,
                "meldritch2-control-input",
                move |_, message, ()| {
                    router.dispatch(message);
                },
                (),
            )
            .map_err(|error| format!("failed to connect MIDI input '{port_name}': {error}"))?;
        Ok(Self {
            _connection: connection,
            note_errors: Arc::new(AtomicU64::new(0)),
            port_name,
        })
    }

    pub fn open_timed_index(index: usize, mut router: TimedMidiRouter) -> Result<Self, String> {
        let mut input = midir::MidiInput::new("meldritch2-timed-notes")
            .map_err(|error| format!("failed to create MIDI input: {error}"))?;
        input.ignore(midir::Ignore::None);
        let ports = input.ports();
        let port = ports
            .get(index)
            .ok_or_else(|| format!("MIDI input index {index} does not exist"))?;
        let port_name = input
            .port_name(port)
            .unwrap_or_else(|_| "unknown MIDI input".into());
        let note_errors = Arc::new(AtomicU64::new(0));
        let callback_errors = Arc::clone(&note_errors);
        let connection = input
            .connect(
                port,
                "meldritch2-timed-note-input",
                move |timestamp, message, ()| {
                    if router.dispatch(timestamp, message).is_err() {
                        callback_errors.fetch_add(1, Ordering::Relaxed);
                    }
                },
                (),
            )
            .map_err(|error| format!("failed to connect MIDI input '{port_name}': {error}"))?;
        Ok(Self {
            _connection: connection,
            note_errors,
            port_name,
        })
    }

    pub fn note_errors(&self) -> u64 {
        self.note_errors.load(Ordering::Relaxed)
    }
}
