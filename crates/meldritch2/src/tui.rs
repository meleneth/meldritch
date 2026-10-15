use std::{
    io::{Write, stdout},
    time::Duration,
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
    panel::{Gesture, Panel},
};

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

fn draw(panel: &Panel, output: &DeviceOutput) -> Result<(), String> {
    let mut out = stdout();
    queue!(
        out,
        MoveTo(0, 0),
        Clear(ClearType::All),
        Print("Meldritch 2 | "),
        Print(&output.device_name),
        Print(" | q: quit\r\n"),
        Print("Up/Down: select  Left/Right: adjust  Home/End: bounds\r\n\r\n")
    )
    .map_err(|error| error.to_string())?;
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
    queue!(
        out,
        Print(format!("\r\nStream errors: {}\r\n", output.stream_errors()))
    )
    .map_err(|error| error.to_string())?;
    out.flush().map_err(|error| error.to_string())
}

pub fn perform(output: &DeviceOutput) -> Result<(), String> {
    let mut panel = Panel::new(output.controls());
    let _terminal = Terminal::enter()?;
    draw(&panel, output)?;
    loop {
        if !event::poll(Duration::from_millis(100)).map_err(|error| error.to_string())? {
            continue;
        }
        let Event::Key(key) = event::read().map_err(|error| error.to_string())? else {
            continue;
        };
        if key.kind != KeyEventKind::Press {
            continue;
        }
        let gesture = match key.code {
            KeyCode::Char('q') | KeyCode::Esc => break,
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => break,
            KeyCode::Up => Some(Gesture::Previous),
            KeyCode::Down => Some(Gesture::Next),
            KeyCode::Left => Some(Gesture::Decrease),
            KeyCode::Right => Some(Gesture::Increase),
            KeyCode::Home => Some(Gesture::Minimum),
            KeyCode::End => Some(Gesture::Maximum),
            _ => None,
        };
        if let Some(gesture) = gesture {
            panel.apply(gesture)?;
            draw(&panel, output)?;
        }
    }
    Ok(())
}
