use crate::midi::MidiRouter;
use crate::midi::TimedMidiRouter;
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
    mpsc::Sender,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CapturedMidiMessage {
    pub timestamp_us: u64,
    pub bytes: Vec<u8>,
}

pub struct MidiMonitorSession {
    _connection: midir::MidiInputConnection<Sender<CapturedMidiMessage>>,
    pub port_name: String,
}

impl MidiMonitorSession {
    pub fn open_name(name: &str, sender: Sender<CapturedMidiMessage>) -> Result<Self, String> {
        let index = crate::device::unique_name_match(&input_port_names()?, name, "MIDI input")?;
        Self::open_index(index, sender)
    }

    pub fn open_index(index: usize, sender: Sender<CapturedMidiMessage>) -> Result<Self, String> {
        let mut input = midir::MidiInput::new("meldritch2-raw-monitor")
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
                "meldritch2-raw-monitor-input",
                |timestamp_us, bytes, sender| {
                    let _ = sender.send(CapturedMidiMessage {
                        timestamp_us,
                        bytes: bytes.to_vec(),
                    });
                },
                sender,
            )
            .map_err(|error| format!("failed to connect MIDI input '{port_name}': {error}"))?;
        Ok(Self {
            _connection: connection,
            port_name,
        })
    }
}

pub fn describe_message(bytes: &[u8]) -> String {
    let hex = bytes
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<Vec<_>>()
        .join(" ");
    let meaning = match bytes {
        [status, note, velocity] if status & 0xf0 == 0x90 && *velocity != 0 => {
            format!(
                "note-on ch={} note={note} velocity={velocity}",
                (status & 0x0f) + 1
            )
        }
        [status, note, velocity] if matches!(status & 0xf0, 0x80 | 0x90) => {
            format!(
                "note-off ch={} note={note} velocity={velocity}",
                (status & 0x0f) + 1
            )
        }
        [status, number, value] if status & 0xf0 == 0xb0 => {
            format!(
                "cc ch={} number={number} value={value}",
                (status & 0x0f) + 1
            )
        }
        [status, program] if status & 0xf0 == 0xc0 => {
            format!("program ch={} number={program}", (status & 0x0f) + 1)
        }
        [status, low, high] if status & 0xf0 == 0xe0 => {
            let value = u16::from(*low) | (u16::from(*high) << 7);
            format!("pitch-bend ch={} value={value}", (status & 0x0f) + 1)
        }
        [0xf8] => "timing-clock".into(),
        [0xfa] => "transport-start".into(),
        [0xfb] => "transport-continue".into(),
        [0xfc] => "transport-stop".into(),
        [0xfe] => "active-sensing".into(),
        [0xff] => "system-reset".into(),
        [0xf0, .., 0xf7] => "system-exclusive".into(),
        [status, ..] if *status >= 0xf0 => "system-common-or-realtime".into(),
        _ => "raw-or-malformed".into(),
    };
    format!("len={} [{hex}] {meaning}", bytes.len())
}

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

enum InputConnection {
    Controls(midir::MidiInputConnection<()>),
    Timed(midir::MidiInputConnection<TimedMidiRouter>),
}

pub struct MidiInputSession {
    connection: Option<InputConnection>,
    note_errors: Arc<AtomicU64>,
    pub port_name: String,
}

impl MidiInputSession {
    pub fn open_name(name: &str, router: MidiRouter) -> Result<Self, String> {
        let index = crate::device::unique_name_match(&input_port_names()?, name, "MIDI input")?;
        Self::open_index(index, router)
    }

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
            connection: Some(InputConnection::Controls(connection)),
            note_errors: Arc::new(AtomicU64::new(0)),
            port_name,
        })
    }

    pub fn open_timed_index(index: usize, router: TimedMidiRouter) -> Result<Self, String> {
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
                move |timestamp, message, router: &mut TimedMidiRouter| {
                    if router.dispatch_recovering(timestamp, message).is_err() {
                        callback_errors.fetch_add(1, Ordering::Relaxed);
                    }
                },
                router,
            )
            .map_err(|error| format!("failed to connect MIDI input '{port_name}': {error}"))?;
        Ok(Self {
            connection: Some(InputConnection::Timed(connection)),
            note_errors,
            port_name,
        })
    }

    pub fn open_timed_name(name: &str, router: TimedMidiRouter) -> Result<Self, String> {
        let index = crate::device::unique_name_match(&input_port_names()?, name, "MIDI input")?;
        Self::open_timed_index(index, router)
    }

    pub fn note_errors(&self) -> u64 {
        self.note_errors.load(Ordering::Relaxed)
    }
}

impl Drop for MidiInputSession {
    fn drop(&mut self) {
        match self.connection.take() {
            Some(InputConnection::Timed(connection)) => {
                let (_, mut router) = connection.close();
                router.release_all();
            }
            Some(InputConnection::Controls(connection)) => {
                drop(connection);
            }
            None => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::describe_message;

    #[test]
    fn raw_monitor_preserves_and_describes_channel_and_vendor_messages() {
        assert_eq!(
            describe_message(&[0x99, 36, 127]),
            "len=3 [99 24 7f] note-on ch=10 note=36 velocity=127"
        );
        assert_eq!(
            describe_message(&[0xb0, 64, 0]),
            "len=3 [b0 40 00] cc ch=1 number=64 value=0"
        );
        assert_eq!(
            describe_message(&[0xc0, 3]),
            "len=2 [c0 03] program ch=1 number=3"
        );
        assert_eq!(
            describe_message(&[0xf0, 0x00, 0x20, 0x6b, 0x01, 0xf7]),
            "len=6 [f0 00 20 6b 01 f7] system-exclusive"
        );
        assert_eq!(describe_message(&[0xfa]), "len=1 [fa] transport-start");
    }
}
