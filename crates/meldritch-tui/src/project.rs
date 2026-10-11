//! Silent project palette construction through the existing typed dispatcher.
use super::*;
use meldritch_app::{
    AppCommand, AppController, PatchCreation, ProjectDirectory, ProjectOverview, ValidatedSong,
};

/// Only a deliberate key press creates a definition; repeats/releases do not.
pub fn project_construction_command(key: KeyEvent, source: &ValidatedSong) -> Option<AppCommand> {
    if key.kind != KeyEventKind::Press
        || key.modifiers != KeyModifiers::NONE
        || key.code != KeyCode::Char('n')
    {
        return None;
    }
    let id = (1..=33)
        .map(|i| format!("voice-{i}"))
        .find(|id| !source.synths().contains_key(id) && !source.samplers().contains_key(id))?;
    Some(AppCommand::CreatePatch(PatchCreation::mono_voice(id)))
}

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
        Line::from("q / Esc: quit    n: create mono voice"),
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

/// Draw accepted definitions, including the independent signal interface.
pub fn draw_project_construction(
    frame: &mut ratatui::Frame<'_>,
    view: &ProjectOverview,
    source: &ValidatedSong,
    status: &str,
) {
    if source.synths().is_empty() {
        draw_empty_project(frame, view);
        return;
    }
    let mut lines = vec![
        Line::from(view.root.display().to_string()),
        Line::from(format!(
            "Instruments: {}    Patterns: {}    No composition loaded",
            source.synths().len(),
            source.note_patterns().len()
        )),
        Line::from("n: create mono voice    q / Esc: quit"),
        Line::from("Unsaved construction: quitting discards it"),
        Line::from(status.to_owned()),
    ];
    let graph = source
        .synths()
        .values()
        .max_by_key(|g| {
            g.id()
                .strip_prefix("voice-")
                .and_then(|id| id.parse::<u32>().ok())
                .unwrap_or(0)
        })
        .unwrap();
    lines.push(Line::from(format!("Patch {} (mono)", graph.id())));
    for input in graph.inputs() {
        lines.push(Line::from(format!(
            "input.{} : {:?}",
            input.id(),
            input.signal()
        )));
    }
    for module in graph.modules() {
        lines.push(Line::from(format!(
            "{} : {}",
            module.id(),
            module.kind().name()
        )));
    }
    for cable in graph.cables() {
        lines.push(Line::from(format!(
            "{} -> {} : {:?}",
            cable.from(),
            cable.to(),
            cable.signal()
        )));
    }
    frame.render_widget(
        Paragraph::new(lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("MELDRITCH · Project construction"),
            )
            .wrap(Wrap { trim: false }),
        frame.area(),
    );
}

/// No files, audio devices or MIDI are touched. In-memory control scaffolding
/// remains separate from the empty authoritative project definitions.
pub fn run_empty_project(project: &ProjectDirectory) -> io::Result<()> {
    let view = ProjectOverview::from_project(project);
    let (mut app, _engine) = AppController::new_empty_project(project).map_err(io::Error::other)?;
    let mut status = String::new();
    let mut terminal = TerminalGuard::enter()?;
    let result = (|| {
        loop {
            terminal.terminal.draw(|frame| {
                draw_project_construction(frame, &view, &app.authored_source().unwrap(), &status)
            })?;
            if event::poll(Duration::from_millis(100))?
                && let Event::Key(key) = event::read()?
            {
                if empty_project_quit(key) {
                    return Ok(());
                }
                if let Some(command) =
                    project_construction_command(key, &app.authored_source().unwrap())
                {
                    status = match app.dispatch(command) {
                        Ok(_) => {
                            "Accepted silent palette definition; no pattern or routing created"
                                .into()
                        }
                        Err(e) => format!("Creation rejected: {e:?}"),
                    };
                }
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
    fn native_creation_presses_dispatch_typed_graphs_and_display_every_connection() {
        let project = meldritch_dsl::load_project_directory(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../songs/examples/34-empty-project-patch"),
        )
        .unwrap();
        let (mut app, _engine) = AppController::new_empty_project(&project).unwrap();
        let key = KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE);
        for kind in [KeyEventKind::Release, KeyEventKind::Repeat] {
            assert!(
                project_construction_command(
                    KeyEvent { kind, ..key },
                    &app.authored_source().unwrap()
                )
                .is_none()
            );
        }
        assert!(
            project_construction_command(
                KeyEvent::new(KeyCode::Char('n'), KeyModifiers::CONTROL),
                &app.authored_source().unwrap()
            )
            .is_none()
        );
        for i in 1..=2 {
            let command =
                project_construction_command(key, &app.authored_source().unwrap()).unwrap();
            assert_eq!(
                command,
                AppCommand::CreatePatch(PatchCreation::mono_voice(format!("voice-{i}")))
            );
            app.dispatch(command).unwrap();
        }
        let source = app.authored_source().unwrap();
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        terminal
            .draw(|frame| {
                draw_project_construction(
                    frame,
                    &ProjectOverview::from_project(&project),
                    &source,
                    "Accepted",
                )
            })
            .unwrap();
        let text = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        for expected in [
            "Instruments: 2",
            "Patterns: 0",
            "Patch voice-2",
            "input.pitch : Pitch",
            "input.gate : Gate",
            "osc : oscillator",
            "env : adsr",
            "amp : vca",
            "out : audio_output",
            "input.pitch -> osc.pitch : Pitch",
            "input.gate -> env.gate : Gate",
            "osc.audio -> amp.audio : Audio",
            "env.control -> amp.level : Control",
            "amp.audio -> out.audio : Audio",
            "quitting discards it",
        ] {
            assert!(text.contains(expected), "missing {expected}");
        }
    }

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
