//! Explicit generic keyboard transport; no musical state in the MIDI callback.
use meldritch_control::devices::{MidiInputQueue, MidiInputRoute, decode_keyboard};
use std::time::{Duration, Instant};

fn unique_port(names: &[String], selection: &str) -> Result<Option<usize>, String> {
    if selection.trim().is_empty() {
        return Err("MIDI keyboard port match must not be empty".into());
    }
    let matches: Vec<_> = names
        .iter()
        .enumerate()
        .filter(|(_, name)| name.to_lowercase().contains(&selection.to_lowercase()))
        .map(|(index, _)| index)
        .collect();
    match matches.as_slice() {
        [] => Ok(None),
        [index] => Ok(Some(*index)),
        _ => Err(format!(
            "ambiguous MIDI keyboard match '{selection}'; use a more specific port name"
        )),
    }
}

struct Connection {
    _input: midir::MidiInputConnection<()>,
    route: MidiInputRoute,
    id: String,
    name: String,
}

pub(crate) struct NativeKeyboard {
    selection: String,
    queue: MidiInputQueue,
    connection: Option<Connection>,
    next_probe: Instant,
    pub(crate) status: String,
}

impl NativeKeyboard {
    #[cfg(test)]
    pub(crate) fn simulated() -> (Self, MidiInputRoute) {
        let queue = MidiInputQueue::default();
        let route = queue.route("keyboard");
        (
            Self {
                selection: "simulated".into(),
                queue,
                connection: None,
                next_probe: Instant::now() + Duration::from_secs(60),
                status: "MIDI: simulated construction entry".into(),
            },
            route,
        )
    }
    pub(crate) fn new(selection: String) -> Result<Self, String> {
        unique_port(&[], &selection)?;
        let mut keyboard = Self {
            selection,
            queue: MidiInputQueue::default(),
            connection: None,
            next_probe: Instant::now(),
            status: String::new(),
        };
        keyboard.probe()?;
        Ok(keyboard)
    }

    fn probe(&mut self) -> Result<(), String> {
        self.next_probe = Instant::now() + Duration::from_secs(1);
        let mut midi = midir::MidiInput::new("meldritch-keyboard")
            .map_err(|e| format!("MIDI keyboard input unavailable: {e}"))?;
        midi.ignore(midir::Ignore::None);
        let ports = midi.ports();
        let names = ports
            .iter()
            .map(|p| midi.port_name(p).unwrap_or_default())
            .collect::<Vec<_>>();
        // Invalidate disconnected epochs before accepting observations or reconnecting.
        if self.connection.as_ref().is_some_and(|c| {
            !ports
                .iter()
                .zip(&names)
                .any(|(port, name)| port.id() == c.id && name == &c.name)
        }) {
            self.disconnect();
        }
        let index = match unique_port(&names, &self.selection) {
            Ok(index) => index,
            Err(error) => {
                self.disconnect();
                return Err(error);
            }
        };
        if self.connection.is_none() {
            if let Some(index) = index {
                let route = self.queue.route("keyboard");
                let callback_route = route.clone();
                let connection = midi
                    .connect(
                        &ports[index],
                        "meldritch-keyboard-input",
                        move |_, bytes, ()| {
                            callback_route.observe(bytes);
                        },
                        (),
                    )
                    .map_err(|e| format!("MIDI keyboard connection failed: {e}"))?;
                self.status = format!("MIDI: connected {}", names[index]);
                self.connection = Some(Connection {
                    _input: connection,
                    route,
                    id: ports[index].id(),
                    name: names[index].clone(),
                });
            } else {
                self.status = format!("MIDI: waiting for '{}'", self.selection);
            }
        }
        Ok(())
    }

    fn disconnect(&mut self) {
        if let Some(connection) = self.connection.take() {
            connection.route.invalidate();
        }
    }

    pub(crate) fn poll(&mut self) {
        if Instant::now() >= self.next_probe
            && let Err(error) = self.probe()
        {
            // A failed probe cannot leave observations from an unverified connection active.
            self.disconnect();
            self.status = format!("MIDI: {error}");
        }
        let dropped = self.queue.take_dropped();
        if dropped != 0 {
            self.status =
                format!("MIDI: dropped {dropped} messages; entry is not lossless recording");
        }
    }

    pub(crate) fn next(&self) -> Option<meldritch_control::devices::ObservedMidi> {
        self.queue.next()
    }
}

pub(crate) fn check(
    port: Option<String>,
    seconds: u64,
    modulation_cc: u8,
    sustain_cc: u8,
) -> Result<(), String> {
    if modulation_cc == sustain_cc {
        return Err("modulation and sustain CCs must be distinct".into());
    }
    let midi = midir::MidiInput::new("meldritch-keyboard-list")
        .map_err(|e| format!("MIDI input unavailable: {e}"))?;
    println!("visible MIDI input ports:");
    if midi.ports().is_empty() {
        println!("  <none>");
    }
    for (index, port) in midi.ports().iter().enumerate() {
        println!(
            "  {index}: {}",
            midi.port_name(port).map_err(|e| e.to_string())?
        );
    }
    drop(midi);
    let Some(port) = port else { return Ok(()) };
    let mut keyboard = NativeKeyboard::new(port)?;
    println!("{}", keyboard.status);
    let deadline = Instant::now()
        .checked_add(Duration::from_secs(seconds))
        .ok_or("MIDI monitoring duration is too large")?;
    let mut previous = keyboard.status.clone();
    while Instant::now() < deadline {
        keyboard.poll();
        if keyboard.status != previous {
            println!("{}", keyboard.status);
            previous = keyboard.status.clone();
        }
        for _ in 0..meldritch_control::QUEUE_CAPACITY {
            let Some(event) = keyboard.next() else { break };
            if event.is_current()
                && let Some(input) = decode_keyboard(&event.bytes, modulation_cc, sustain_cc)
            {
                println!("{input:?}");
            }
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn keyboard_port_selection_never_guesses_between_matches() {
        let names = vec!["MicroLab mk3 MIDI 1".into(), "Other keyboard".into()];
        assert_eq!(unique_port(&names, "microlab").unwrap(), Some(0));
        assert_eq!(unique_port(&names, "missing").unwrap(), None);
        assert!(unique_port(&names, " ").is_err());
        let duplicate = vec![names[0].clone(), "MicroLab mk3 MIDI 2".into()];
        assert!(unique_port(&duplicate, "MicroLab").is_err());
        assert_eq!(unique_port(&duplicate, "MIDI 2").unwrap(), Some(1));
        assert!(check(None, 0, 1, 1).unwrap_err().contains("distinct"));
    }
    #[test]
    fn keyboard_queue_reports_overflow_and_discards_old_connection_observations() {
        let (mut keyboard, route) = NativeKeyboard::simulated();
        for _ in 0..=meldritch_control::QUEUE_CAPACITY {
            assert!(route.observe(&[0x90, 60, 127]));
        }
        keyboard.poll();
        assert!(keyboard.status.contains("dropped 1 messages"));
        route.invalidate();
        for _ in 0..meldritch_control::QUEUE_CAPACITY {
            assert!(!keyboard.next().unwrap().is_current());
        }
        assert!(keyboard.next().is_none());
        let fresh = keyboard.queue.route("keyboard");
        fresh.observe(&[0x90, 62, 127]);
        assert!(keyboard.next().unwrap().is_current());
    }
}
