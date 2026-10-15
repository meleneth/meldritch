use crate::midi::MidiRouter;

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
            port_name,
        })
    }
}
