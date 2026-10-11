//! Read-only opening view for a project with no selected composition.
use super::*;
use meldritch_app::ProjectOverview;

pub fn empty_project_quit(key: KeyEvent) -> bool {
    key.kind == KeyEventKind::Press
        && (matches!(key.code, KeyCode::Char('q') | KeyCode::Esc)
            || (key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL)))
}

pub fn draw_empty_project(frame: &mut ratatui::Frame<'_>, view: &ProjectOverview) {
    let text = vec![
        Line::from(view.root.display().to_string()),
        Line::from(""),
        Line::from("No composition loaded"),
        Line::from(format!(
            "Instruments: {}    Patterns: {}",
            view.instruments, view.patterns
        )),
        Line::from(""),
        Line::from("q / Esc: quit"),
    ];
    frame.render_widget(
        Paragraph::new(text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("MELDRITCH · Project"),
            )
            .wrap(Wrap { trim: false }),
        frame.area(),
    );
}

/// No engine, files, MIDI or musical definitions are created just to open a root.
pub fn run_empty_project(view: &ProjectOverview) -> io::Result<()> {
    if view.composition.is_some() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "loaded compositions use the existing song cockpit",
        ));
    }
    let mut terminal = TerminalGuard::enter()?;
    let result = (|| {
        loop {
            terminal
                .terminal
                .draw(|frame| draw_empty_project(frame, view))?;
            if event::poll(Duration::from_millis(100))?
                && let Event::Key(key) = event::read()?
                && empty_project_quit(key)
            {
                return Ok(());
            }
        }
    })();
    terminal.restore()?;
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;

    #[test]
    fn empty_project_is_readable_at_small_terminal_sizes() {
        let view = ProjectOverview {
            root: "/tmp/black-cathedral".into(),
            composition: None,
            instruments: 0,
            patterns: 0,
        };
        for (width, height) in [(80, 24), (40, 10)] {
            let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
            terminal
                .draw(|frame| draw_empty_project(frame, &view))
                .unwrap();
            let buffer = terminal.backend().buffer();
            let text = buffer
                .content()
                .iter()
                .map(|cell| cell.symbol())
                .collect::<String>();
            assert!(text.contains("No composition loaded"));
            assert!(text.contains("Instruments: 0"));
            assert!(text.contains("Patterns: 0"));
            assert!(text.contains("q / Esc: quit"));
        }
    }

    #[test]
    fn empty_project_only_accepts_quit_presses() {
        for code in [KeyCode::Char('q'), KeyCode::Esc] {
            let press = KeyEvent::new(code, KeyModifiers::NONE);
            assert!(empty_project_quit(press));
            assert!(!empty_project_quit(KeyEvent {
                kind: KeyEventKind::Release,
                ..press
            }));
        }
        assert!(empty_project_quit(KeyEvent::new(
            KeyCode::Char('c'),
            KeyModifiers::CONTROL
        )));
        for code in [KeyCode::Char('p'), KeyCode::Char('s'), KeyCode::Char(' ')] {
            assert!(!empty_project_quit(KeyEvent::new(code, KeyModifiers::NONE)));
        }
    }
}
