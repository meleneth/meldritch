use std::{
    io::{Write, stdout},
    time::{Duration, Instant},
};

use crossterm::{
    cursor::{Hide, MoveTo, Show},
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute, queue,
    style::Print,
    terminal::{self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
};

use crate::{
    device::DeviceOutput,
    midi::{TimedMidiError, TimedMidiRouter},
    midi_device::MidiInputSession,
    panel::{Gesture, Panel},
    screen_panel::{ScreenPanel, WidgetView},
};

const NOTE_KEYS: [(char, u8); 12] = [
    ('z', 60),
    ('s', 61),
    ('x', 62),
    ('d', 63),
    ('c', 64),
    ('v', 65),
    ('g', 66),
    ('b', 67),
    ('h', 68),
    ('n', 69),
    ('j', 70),
    ('m', 71),
];
const NOTE_NAMES: [&str; 12] = [
    "C4", "C#4", "D4", "D#4", "E4", "F4", "F#4", "G4", "G#4", "A4", "A#4", "B4",
];

/// A latched one-octave TUI keyboard using the compiled timed-note route.
pub struct TuiKeyboard {
    router: TimedMidiRouter,
    channel: u8,
    active: Option<u8>,
    started: Instant,
    last_error: Option<String>,
}

impl TuiKeyboard {
    pub fn new(router: TimedMidiRouter) -> Option<Self> {
        let channel = router.first_mapped_channel()?;
        Some(Self {
            router,
            channel,
            active: None,
            started: Instant::now(),
            last_error: None,
        })
    }

    fn elapsed_us(&self) -> u64 {
        u64::try_from(self.started.elapsed().as_micros()).unwrap_or(u64::MAX)
    }

    fn send(&mut self, midi_us: u64, message: [u8; 3]) -> Result<(), TimedMidiError> {
        if let Err(error) = self.router.dispatch_recovering(midi_us, &message) {
            self.router.release_all();
            self.active = None;
            return Err(error);
        }
        Ok(())
    }

    /// One press selects a pitch; Space releases the latched Gate.
    fn handle_key_at(&mut self, code: KeyCode, midi_us: u64) -> Result<bool, TimedMidiError> {
        let note = match code {
            KeyCode::Char(' ') => {
                if let Some(previous) = self.active {
                    self.send(midi_us, [0x80 | (self.channel - 1), previous, 0])?;
                    self.active = None;
                }
                self.last_error = None;
                return Ok(true);
            }
            KeyCode::Char(key) => NOTE_KEYS
                .iter()
                .find(|(candidate, _)| *candidate == key.to_ascii_lowercase())
                .map(|(_, note)| *note),
            _ => None,
        };
        let Some(note) = note else { return Ok(false) };
        if self.active == Some(note) {
            return Ok(true);
        }
        if let Some(previous) = self.active {
            self.send(midi_us, [0x80 | (self.channel - 1), previous, 0])?;
            self.active = None;
        }
        self.send(midi_us, [0x90 | (self.channel - 1), note, 127])?;
        self.active = Some(note);
        self.last_error = None;
        Ok(true)
    }

    fn active_name(&self) -> &'static str {
        self.active
            .map(|note| NOTE_NAMES[usize::from(note - 60)])
            .unwrap_or("off")
    }
}

impl Drop for TuiKeyboard {
    fn drop(&mut self) {
        self.router.release_all();
    }
}

struct Terminal;

impl Terminal {
    fn enter() -> Result<Self, String> {
        terminal::enable_raw_mode().map_err(|error| error.to_string())?;
        if let Err(error) = execute!(stdout(), EnterAlternateScreen, Hide) {
            let _ = terminal::disable_raw_mode();
            return Err(error.to_string());
        }
        Ok(Self)
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        let _ = execute!(stdout(), Show, LeaveAlternateScreen);
        let _ = terminal::disable_raw_mode();
    }
}

fn draw(
    panel: &Panel,
    screens: Option<&ScreenPanel>,
    output: &DeviceOutput,
    midi_session: Option<&MidiInputSession>,
    keyboard: Option<&TuiKeyboard>,
    step_grid: Option<(usize, usize)>,
) -> Result<(), String> {
    let mut out = stdout();
    queue!(
        out,
        MoveTo(0, 0),
        Clear(ClearType::All),
        Print("Meldritch 2 | "),
        Print(&output.device_name),
        Print(" | MIDI: "),
        Print(midi_session.map_or("none", |session| session.port_name.as_str())),
        Print(" | q: quit\r\n"),
        Print("Up/Down: select  Left/Right: adjust  Home/End: bounds\r\n"),
        Print(if screens.is_some() {
            "Tab/Shift+Tab: switch screens  Enter: open link\r\n\r\n"
        } else {
            "\r\n"
        })
    )
    .map_err(|error| error.to_string())?;
    if let Some((page, selected_track)) = step_grid {
        let performance = output.performance().expect("step grid performance");
        let sequence = &performance.sequencer;
        let names = sequence.track_names();
        let pattern = sequence.pattern();
        let first = page * 8;
        queue!(out, Print(format!(
            "{}                 {:.0} BPM    {}\r\nPage {} ({:02}-{:02})                   Step {:02}\r\n\r\n          ",
            sequence.name, sequence.bpm(), if sequence.playing() { "PLAYING" } else { "STOPPED" },
            if page == 0 { "A" } else { "B" }, first + 1, first + 8, sequence.current_step() + 1,
        ))).map_err(|error| error.to_string())?;
        for step in first..first + 8 {
            queue!(out, Print(format!("{:02} ", step + 1))).map_err(|e| e.to_string())?;
        }
        queue!(out, Print("\r\n")).map_err(|e| e.to_string())?;
        for (track, name) in names.iter().enumerate() {
            queue!(
                out,
                Print(format!(
                    "{}{:8} ",
                    if track == selected_track { ">" } else { " " },
                    name
                ))
            )
            .map_err(|e| e.to_string())?;
            for step in first..first + 8 {
                let glyph = if sequence.current_step() == step && sequence.playing() {
                    if pattern.velocities[track][step] > 0.0 {
                        "◆"
                    } else {
                        "◇"
                    }
                } else if pattern.velocities[track][step] > 0.0 {
                    "●"
                } else {
                    "·"
                };
                queue!(out, Print(format!(" {glyph} "))).map_err(|e| e.to_string())?;
            }
            queue!(out, Print("\r\n")).map_err(|e| e.to_string())?;
        }
        queue!(out, Print("\r\n1-8: toggle  Up/Down: track  Tab: page  Space: play/stop  +/-: BPM  r: restart  Shift-C: clear track\r\n"))
            .map_err(|e| e.to_string())?;
    } else if let Some(keyboard) = keyboard {
        queue!(
            out,
            Print(format!(
                "Z S X D C V G B H N J M: C4-B4  Space: release | channel {} | note: {}\r\n",
                keyboard.channel,
                keyboard.active_name(),
            ))
        )
        .map_err(|error| error.to_string())?;
        if let Some(error) = &keyboard.last_error {
            queue!(out, Print(format!("Keyboard note error: {error}\r\n")))
                .map_err(|error| error.to_string())?;
        }
    }
    if step_grid.is_none()
        && let Some(screens) = screens
    {
        let screen = screens.current();
        let (position, count) = screens.position();
        queue!(
            out,
            Print(format!("{} ({position}/{count})\r\n\r\n", screen.title))
        )
        .map_err(|error| error.to_string())?;
        for (index, widget) in screen.widgets.iter().enumerate() {
            let marker = if screen.selected == Some(index) {
                ">"
            } else {
                " "
            };
            match widget {
                WidgetView::Label(text) => queue!(out, Print(format!("  {text}\r\n"))),
                WidgetView::Control { label, row } => {
                    let (min, max) = row.range();
                    queue!(
                        out,
                        Print(format!(
                            "{marker} {label} = {:.4}  [{min:.4}, {max:.4}]\r\n",
                            row.value()
                        ))
                    )
                }
                WidgetView::Link { label, .. } => {
                    queue!(out, Print(format!("{marker} {label} >\r\n")))
                }
                WidgetView::Button { label, .. } => {
                    queue!(out, Print(format!("{marker} [{label}]\r\n")))
                }
            }
            .map_err(|error| error.to_string())?;
        }
    } else if step_grid.is_none() {
        if panel.rows().is_empty() {
            queue!(out, Print("This instrument declares no controls.\r\n"))
                .map_err(|error| error.to_string())?;
        }
        for (index, row) in panel.rows().iter().enumerate() {
            let marker = if panel.selected() == Some(index) {
                ">"
            } else {
                " "
            };
            let (min, max) = row.range();
            queue!(
                out,
                Print(format!(
                    "{marker} {} = {:.4}  [{:.4}, {:.4}]  {}.{}\r\n",
                    row.id(),
                    row.value(),
                    min,
                    max,
                    row.target().module,
                    row.target().name
                ))
            )
            .map_err(|error| error.to_string())?;
        }
    }
    queue!(
        out,
        Print(format!(
            "\r\nStream errors: {} | Timed output errors: {} | MIDI input errors: {}\r\n",
            output.stream_errors(),
            output.timed_note_errors(),
            midi_session.map_or(0, MidiInputSession::note_errors),
        ))
    )
    .map_err(|error| error.to_string())?;
    out.flush().map_err(|error| error.to_string())
}

pub fn perform(
    output: &DeviceOutput,
    midi_session: Option<&MidiInputSession>,
    mut keyboard: Option<TuiKeyboard>,
) -> Result<(), String> {
    let _apc = match (output.apc_mini(), output.performance()) {
        (Some(name), Some(performance)) => Some(crate::apc::ApcSession::start(
            name.to_owned(),
            std::sync::Arc::clone(&performance.sequencer),
        )),
        _ => None,
    };
    let controls = output.controls();
    let tempo_control = controls
        .iter()
        .find(|(control, _)| control.id() == "Tempo")
        .map(|(control, _)| control.clone());
    let mut screens = if output.screens().is_empty() {
        None
    } else {
        Some(ScreenPanel::new(output.screens(), controls.clone())?)
    };
    let mut panel = Panel::new(controls);
    let _terminal = Terminal::enter()?;
    draw(
        &panel,
        screens.as_ref(),
        output,
        midi_session,
        keyboard.as_ref(),
        output.performance().map(|_| (0, 0)),
    )?;
    let mut grid_page = 0usize;
    let mut grid_track = 0usize;
    loop {
        if !event::poll(Duration::from_millis(100)).map_err(|error| error.to_string())? {
            draw(
                &panel,
                screens.as_ref(),
                output,
                midi_session,
                keyboard.as_ref(),
                output.performance().map(|_| (grid_page, grid_track)),
            )?;
            continue;
        }
        let Event::Key(key) = event::read().map_err(|error| error.to_string())? else {
            continue;
        };
        if key.kind != KeyEventKind::Press {
            continue;
        }
        if matches!(key.code, KeyCode::Char('q') | KeyCode::Esc)
            || (key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL))
        {
            break;
        }
        if let Some(performance) = output.performance() {
            let sequence = &performance.sequencer;
            match key.code {
                KeyCode::Tab | KeyCode::BackTab => grid_page = 1 - grid_page,
                KeyCode::Up => grid_track = grid_track.saturating_sub(1),
                KeyCode::Down => {
                    grid_track =
                        (grid_track + 1).min(sequence.track_names().len().saturating_sub(1))
                }
                KeyCode::Char(' ') => sequence.toggle_transport(),
                KeyCode::Char('+') | KeyCode::Char('=') => {
                    if let Some(control) = &tempo_control {
                        control.publish((sequence.bpm() + 1.0).min(control.range().1))?;
                    }
                }
                KeyCode::Char('-') => {
                    if let Some(control) = &tempo_control {
                        control.publish((sequence.bpm() - 1.0).max(control.range().0))?;
                    }
                }
                KeyCode::Char('r') => sequence.restart(),
                KeyCode::Char('C') => sequence.clear_track(grid_track)?,
                KeyCode::Char(number @ '1'..='8') => {
                    let column = number as usize - '1' as usize;
                    sequence.toggle(grid_track, grid_page * 8 + column)?;
                }
                _ => {}
            }
            draw(
                &panel,
                screens.as_ref(),
                output,
                midi_session,
                keyboard.as_ref(),
                Some((grid_page, grid_track)),
            )?;
            continue;
        }
        if key.modifiers.difference(KeyModifiers::SHIFT).is_empty()
            && let Some(keyboard) = &mut keyboard
        {
            let midi_us = keyboard.elapsed_us();
            match keyboard.handle_key_at(key.code, midi_us) {
                Ok(true) => {
                    draw(
                        &panel,
                        screens.as_ref(),
                        output,
                        midi_session,
                        Some(keyboard),
                        None,
                    )?;
                    continue;
                }
                Err(error) => {
                    keyboard.last_error = Some(format!("{error:?}"));
                    draw(
                        &panel,
                        screens.as_ref(),
                        output,
                        midi_session,
                        Some(keyboard),
                        None,
                    )?;
                    continue;
                }
                Ok(false) => {}
            }
        }
        if let Some(screens) = &mut screens {
            match key.code {
                KeyCode::Tab => screens.switch(1),
                KeyCode::BackTab => screens.switch(-1),
                KeyCode::Enter => screens.activate()?,
                _ => {}
            }
            if matches!(key.code, KeyCode::Tab | KeyCode::BackTab | KeyCode::Enter) {
                draw(
                    &panel,
                    Some(screens),
                    output,
                    midi_session,
                    keyboard.as_ref(),
                    None,
                )?;
                continue;
            }
        }
        let gesture = match key.code {
            KeyCode::Up => Some(Gesture::Previous),
            KeyCode::Down => Some(Gesture::Next),
            KeyCode::Left => Some(Gesture::Decrease),
            KeyCode::Right => Some(Gesture::Increase),
            KeyCode::Home => Some(Gesture::Minimum),
            KeyCode::End => Some(Gesture::Maximum),
            _ => None,
        };
        if let Some(gesture) = gesture {
            if let Some(screens) = &mut screens {
                screens.apply(gesture)?;
            } else {
                panel.apply(gesture)?;
            }
            draw(
                &panel,
                screens.as_ref(),
                output,
                midi_session,
                keyboard.as_ref(),
                None,
            )?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crossterm::event::KeyCode;

    use super::TuiKeyboard;
    use crate::{
        compiler::compile,
        live_notes::live_note_queue,
        lua,
        midi::TimedMidiRouter,
        timed::{TimedNotePlan, TimedNoteState},
    };

    #[test]
    fn tui_keys_select_pitch_and_space_releases_the_gate() {
        let graph = lua::read(include_str!("../examples/53-tui-keys.lua")).unwrap();
        let mut machine = compile(&graph).unwrap();
        let (producer, mut consumer) = live_note_queue(&machine, 0);
        let router = TimedMidiRouter::new(&machine, producer, 48_000, 3).unwrap();
        let mut keyboard = TuiKeyboard::new(router).unwrap();
        assert_eq!(keyboard.channel, 1);
        assert_eq!(keyboard.handle_key_at(KeyCode::Char('q'), 0), Ok(false));
        assert_eq!(keyboard.handle_key_at(KeyCode::Char('z'), 0), Ok(true));
        assert_eq!(keyboard.handle_key_at(KeyCode::Char('z'), 42), Ok(true));
        assert_eq!(keyboard.handle_key_at(KeyCode::Char('X'), 84), Ok(true));
        assert_eq!(keyboard.active_name(), "D4");
        assert_eq!(keyboard.handle_key_at(KeyCode::Char(' '), 167), Ok(true));
        assert_eq!(keyboard.active_name(), "off");
        let mut actual = [f64::NAN; 16];
        assert_eq!(
            consumer
                .render_next(&mut machine, &mut actual)
                .unwrap()
                .applied,
            4
        );

        let c4 = 440.0 * 2.0_f64.powf((60.0 - 69.0) / 12.0);
        let d4 = 440.0 * 2.0_f64.powf((62.0 - 69.0) / 12.0);
        let mut reference = compile(&graph).unwrap();
        let mut plan = TimedNotePlan::prepare(
            &reference,
            0,
            &[
                TimedNoteState {
                    frame: 3,
                    channel: 1,
                    pitch_hz: c4,
                    gate: true,
                },
                TimedNoteState {
                    frame: 7,
                    channel: 1,
                    pitch_hz: c4,
                    gate: false,
                },
                TimedNoteState {
                    frame: 7,
                    channel: 1,
                    pitch_hz: d4,
                    gate: true,
                },
                TimedNoteState {
                    frame: 11,
                    channel: 1,
                    pitch_hz: d4,
                    gate: false,
                },
            ],
        )
        .unwrap();
        let mut expected = [0.0; 16];
        plan.render_next(&mut reference, &mut expected).unwrap();
        assert_eq!(actual, expected);
    }

    #[test]
    fn closing_tui_keyboard_releases_its_active_note() {
        let graph = lua::read(include_str!("../examples/53-tui-keys.lua")).unwrap();
        let mut machine = compile(&graph).unwrap();
        let (producer, mut consumer) = live_note_queue(&machine, 0);
        let router = TimedMidiRouter::new(&machine, producer, 48_000, 3).unwrap();
        let mut keyboard = TuiKeyboard::new(router).unwrap();
        keyboard.handle_key_at(KeyCode::Char('z'), 0).unwrap();
        let mut opening = [0.0; 4];
        consumer.render_next(&mut machine, &mut opening).unwrap();
        assert!(opening[3].abs() > 0.0);
        drop(keyboard);
        let mut released = [f64::NAN; 4];
        consumer.render_next(&mut machine, &mut released).unwrap();
        assert!(released.iter().any(|sample| sample.abs() > 0.0));
        let mut tail = [0.0; 1024];
        consumer.render_next(&mut machine, &mut tail).unwrap();
        consumer.render_next(&mut machine, &mut tail).unwrap();
        assert!(tail.iter().all(|sample| sample.abs() < 0.001));
    }
}
