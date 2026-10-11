//! Silent project palette construction through the existing typed dispatcher.
use super::*;
use meldritch_app::{
    AppCommand, AppController, PatchCreation, PatternSignal, PatternVoiceDefinition,
    ProjectDirectory, ProjectOverview, ValidatedSong,
};

pub fn project_level_command(
    key: KeyEvent,
    view: &AppViewModel,
    source: &ValidatedSong,
) -> Option<AppCommand> {
    if key.kind != KeyEventKind::Press || key.modifiers != KeyModifiers::NONE {
        return None;
    }
    let synth = &view.instrument_editor.as_ref()?.synth;
    if view.cockpit_mode != meldritch_app::CockpitMode::Performance {
        if key.code == KeyCode::Char('x')
            && !view.instrument_editor.as_ref()?.visible
            && !view
                .palette_pattern_editor
                .as_ref()
                .is_some_and(|editor| editor.visible)
        {
            return Some(AppCommand::ExposeOutputLevel {
                synth: synth.clone(),
                control: "level".into(),
            });
        }
        return None;
    }
    let controls: Vec<_> = source
        .pattern_voices()
        .values()
        .filter(|voice| {
            source.synths()[&voice.synth]
                .output_level_control()
                .is_some()
        })
        .collect();
    if matches!(key.code, KeyCode::Up | KeyCode::Down) {
        if controls.is_empty() {
            return None;
        }
        let index = controls
            .iter()
            .position(|voice| &voice.synth == synth)
            .unwrap_or(0);
        let next = if key.code == KeyCode::Down {
            (index + 1) % controls.len()
        } else {
            (index + controls.len() - 1) % controls.len()
        };
        return Some(AppCommand::SelectInstrument(controls[next].synth.clone()));
    }
    let voice = controls.into_iter().find(|voice| &voice.synth == synth)?;
    let control = source.synths()[synth].output_level_control()?.to_owned();
    let current = view
        .output_levels
        .get(&(voice.id.clone(), control.clone()))
        .copied()
        .unwrap_or(voice.level);
    let value = match key.code {
        KeyCode::Left => Some((current - 0.1).max(0.0)),
        KeyCode::Right => Some((current + 0.1).min(1.0)),
        KeyCode::Home => Some(0.0),
        KeyCode::End => None,
        _ => return None,
    };
    Some(AppCommand::SetOutputLevel {
        voice: voice.id.clone(),
        control,
        value,
    })
}

/// Composition relationships are deliberate typed actions, separate from creation.
pub fn project_binding_command(
    key: KeyEvent,
    view: &AppViewModel,
    source: &ValidatedSong,
) -> Option<AppCommand> {
    if key.kind != KeyEventKind::Press
        || key.modifiers != KeyModifiers::NONE
        || view.cockpit_mode == meldritch_app::CockpitMode::Performance
        || view
            .instrument_editor
            .as_ref()
            .is_some_and(|editor| editor.visible)
        || view
            .palette_pattern_editor
            .as_ref()
            .is_some_and(|editor| editor.visible)
    {
        return None;
    }
    let synth = &view.instrument_editor.as_ref()?.synth;
    let pattern = &view.palette_pattern_editor.as_ref()?.pattern;
    let id = format!("{synth}-instance");
    if key.code == KeyCode::Char('v') {
        return Some(AppCommand::CreatePatternVoice(PatternVoiceDefinition {
            id,
            synth: synth.clone(),
            length_ticks: pattern.length_ticks,
            looped: pattern.looped,
            initial_pitch: 48,
            level: 0.8,
            pitch_pattern: None,
            gate_pattern: None,
        }));
    }
    let voice = source.pattern_voices().get(&id)?;
    let (signal, binding) = match key.code {
        KeyCode::Char('p') => (PatternSignal::Pitch, &voice.pitch_pattern),
        KeyCode::Char('g') => (PatternSignal::Gate, &voice.gate_pattern),
        _ => return None,
    };
    Some(AppCommand::BindPatternVoice {
        voice: id,
        signal,
        pattern: if binding.as_deref() == Some(&pattern.id) {
            None
        } else {
            Some(pattern.id.clone())
        },
    })
}

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

/// Native note gestures lower to the same NoteEdit commands as Lua/hardware.
pub fn project_pattern_command(
    key: KeyEvent,
    view: &AppViewModel,
    source: &ValidatedSong,
) -> Option<AppCommand> {
    use meldritch_app::NoteEdit;
    if key.kind != KeyEventKind::Press
        || view.cockpit_mode == meldritch_app::CockpitMode::Performance
    {
        return None;
    }
    let editor = view.palette_pattern_editor.as_ref();
    if key.code == KeyCode::Char('n') && key.modifiers == KeyModifiers::CONTROL && editor.is_some()
    {
        return Some(AppCommand::TogglePatternEditor);
    }
    if let Some(editor) = editor
        && editor.visible
    {
        if key.code == KeyCode::Char('e')
            && key.modifiers == KeyModifiers::CONTROL
            && view.instrument_editor.is_some()
        {
            return Some(AppCommand::ToggleInstrumentEditor);
        }
        if key.code == KeyCode::Char('s') && key.modifiers == KeyModifiers::CONTROL {
            return Some(AppCommand::SavePattern);
        }
        if key.modifiers != KeyModifiers::NONE {
            return None;
        }
        let selected = editor
            .selected_note
            .as_ref()
            .and_then(|id| editor.pattern.notes.iter().find(|n| &n.id == id));
        let edit = match key.code {
            KeyCode::Esc => return Some(AppCommand::TogglePatternEditor),
            KeyCode::Left | KeyCode::Right | KeyCode::Up | KeyCode::Down => {
                return Some(AppCommand::MovePatternCursor {
                    tick_delta: match key.code {
                        KeyCode::Left => -480,
                        KeyCode::Right => 480,
                        _ => 0,
                    },
                    pitch_delta: match key.code {
                        KeyCode::Down => -1,
                        KeyCode::Up => 1,
                        _ => 0,
                    },
                });
            }
            KeyCode::Enter => {
                if let Some(note) = editor.pattern.notes.iter().find(|n| {
                    n.start_tick == editor.cursor_tick
                        && n.note == meldritch_app::note_name(editor.cursor_pitch)
                }) {
                    return Some(AppCommand::SelectNote {
                        pattern: editor.pattern.id.clone(),
                        id: note.id.clone(),
                    });
                }
                NoteEdit::Insert {
                    start_tick: editor.cursor_tick,
                    duration_ticks: 480,
                    pitch: editor.cursor_pitch,
                    velocity: 0.8,
                }
            }
            KeyCode::Delete => NoteEdit::Delete {
                id: selected?.id.clone(),
            },
            KeyCode::Char('[' | ']') => NoteEdit::Duration {
                id: selected?.id.clone(),
                duration_ticks: if key.code == KeyCode::Char('[') {
                    selected?.duration_ticks.saturating_sub(240)
                } else {
                    selected?.duration_ticks.saturating_add(240)
                },
            },
            KeyCode::Char('-' | '+' | '=') => NoteEdit::Velocity {
                id: selected?.id.clone(),
                velocity: selected?.velocity
                    + if key.code == KeyCode::Char('-') {
                        -0.1
                    } else {
                        0.1
                    },
            },
            _ => return None,
        };
        return Some(AppCommand::EditNote {
            pattern: editor.pattern.id.clone(),
            edit,
        });
    }
    if key.code == KeyCode::Char('c')
        && key.modifiers == KeyModifiers::NONE
        && !view.instrument_editor.as_ref().is_some_and(|e| e.visible)
    {
        let id = (1..=129).map(|i| format!("pattern-{i}")).find(|id| {
            !source.note_patterns().contains_key(id)
                && !source.parameter_patterns().contains_key(id)
        })?;
        return Some(AppCommand::CreatePalettePattern {
            id,
            length_ticks: 3840,
            looped: true,
        });
    }
    None
}

pub fn draw_project_pattern(
    frame: &mut ratatui::Frame<'_>,
    editor: &meldritch_app::PalettePatternEditorView,
    status: &str,
) {
    let mut lines = vec![
        Line::from(format!(
            "Pattern {} · {} ticks · loop {}",
            editor.pattern.id, editor.pattern.length_ticks, editor.pattern.looped
        )),
        Line::from("Outputs: pitch : Pitch    gate : Gate (independent bindings)"),
        Line::from(format!(
            "Cursor: tick {} · {}   Selected: {}",
            editor.cursor_tick,
            meldritch_app::note_name(editor.cursor_pitch),
            editor.selected_note.as_deref().unwrap_or("none")
        )),
        Line::from("Arrows cursor · Enter insert/select · [ ] duration · - + velocity"),
        Line::from("Delete note · Esc/Ctrl-N close · Ctrl-E patch parameters"),
        Line::from(
            if editor.source.pattern_voices().values().any(|voice| {
                voice.pitch_pattern.as_deref() == Some(&editor.pattern.id)
                    || voice.gate_pattern.as_deref() == Some(&editor.pattern.id)
            }) {
                "Unsaved project · Bindings/playback at overview"
            } else {
                "Unsaved source: no composition bindings · Bind at overview"
            },
        ),
        Line::from(status.to_owned()),
    ];
    for note in &editor.pattern.notes {
        lines.push(Line::from(format!(
            "{} {} {} @{} +{} ticks velocity {:.2}",
            if editor.selected_note.as_ref() == Some(&note.id) {
                ">"
            } else {
                " "
            },
            note.id,
            note.note,
            note.start_tick,
            note.duration_ticks,
            note.velocity
        )));
    }
    frame.render_widget(
        Paragraph::new(lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("MELDRITCH · Project pattern"),
            )
            .wrap(Wrap { trim: false }),
        frame.area(),
    );
}

pub fn empty_project_quit(key: KeyEvent) -> bool {
    key.kind == KeyEventKind::Press
        && (matches!(key.code, KeyCode::Char('q') | KeyCode::Esc)
            || (key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL)))
}

/// Reuse accepted editor bindings while the project has no playable composition.
/// Transport/backing showcase actions are not exposed by this silent cockpit.
pub fn project_editor_action(key: KeyEvent, view: &AppViewModel) -> Option<TuiAction> {
    if view.cockpit_mode == meldritch_app::CockpitMode::Performance {
        return None;
    }
    if key.kind == KeyEventKind::Press
        && key.code == KeyCode::Char('c')
        && key.modifiers.contains(KeyModifiers::CONTROL)
    {
        return Some(TuiAction::Quit);
    }
    let action = map_key_for_view(key, &Step::new(48), view)?;
    match action {
        TuiAction::Quit => Some(action),
        TuiAction::Input(ref input)
            if matches!(
                input,
                AppInput::ToggleInstrumentEditor
                    | AppInput::NavigateInstrument { .. }
                    | AppInput::AdjustInstrument(_)
                    | AppInput::EditInstrument(_)
                    | AppInput::TogglePatchEditor
                    | AppInput::PatchCharacter(_)
                    | AppInput::PatchBackspace
                    | AppInput::PatchScroll(_)
                    | AppInput::PatchSubmit
                    | AppInput::SaveInstrument
            ) =>
        {
            Some(action)
        }
        _ => None,
    }
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
        Line::from("q / Esc: quit    n: create voice    c: create pattern"),
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
    draw_project_construction_selected(frame, view, source, status, None);
}

fn draw_project_construction_selected(
    frame: &mut ratatui::Frame<'_>,
    view: &ProjectOverview,
    source: &ValidatedSong,
    status: &str,
    selected: Option<&str>,
) {
    if source.synths().is_empty() && source.note_patterns().is_empty() {
        draw_empty_project(frame, view);
        let area = frame.area();
        if !status.is_empty() && area.height > 2 {
            frame.render_widget(
                Paragraph::new(status),
                ratatui::layout::Rect::new(
                    area.x + 1,
                    area.y + area.height - 2,
                    area.width.saturating_sub(2),
                    1,
                ),
            );
        }
        return;
    }
    let mut lines = vec![
        Line::from(view.root.display().to_string()),
        Line::from(format!(
            "Instruments: {}    Patterns: {}    Voices: {}",
            source.synths().len(),
            source.note_patterns().len(),
            source.pattern_voices().len(),
        )),
        Line::from(
            "n/c: patch/pattern · Ctrl-E/N: editors · Space: play · Ctrl-Tab mode · q: quit",
        ),
        Line::from("Unsaved construction: quitting discards it"),
        Line::from(status.to_owned()),
    ];
    if !source.pattern_voices().is_empty() {
        lines.push(Line::from(
            "v: instance · p/g: pitch/gate binding · x: expose level",
        ));
        for voice in source.pattern_voices().values() {
            lines.push(Line::from(format!(
                "{} ({}) · level {:.2}",
                voice.id, voice.synth, voice.level
            )));
            lines.push(Line::from(format!(
                "  pitch: {}.pitch -> input.pitch (fallback {})",
                voice.pitch_pattern.as_deref().unwrap_or("unbound"),
                meldritch_app::note_name(voice.initial_pitch)
            )));
            lines.push(Line::from(format!(
                "  gate: {}.gate -> input.gate (fallback low)",
                voice.gate_pattern.as_deref().unwrap_or("unbound")
            )));
        }
    } else {
        lines.push(Line::from(
            "v: create unbound instance after selecting a patch and pattern",
        ));
    }
    let graph = selected.and_then(|id| source.synths().get(id)).or_else(|| {
        source.synths().values().max_by_key(|g| {
            g.id()
                .strip_prefix("voice-")
                .and_then(|id| id.parse::<u32>().ok())
                .unwrap_or(0)
        })
    });
    let Some(graph) = graph else {
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
        return;
    };
    lines.push(Line::from(format!(
        "Patch {} (mono) · exposed level: {}",
        graph.id(),
        graph.output_level_control().unwrap_or("none; x to expose")
    )));
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
    let (mut app, _engine) = AppController::new_empty_project(project).map_err(io::Error::other)?;
    struct SilentHost;
    impl ProjectPlaybackHost for SilentHost {
        fn poll(&mut self, _: &mut AppController) -> Result<(), String> {
            Ok(())
        }
        fn toggle_playback(&mut self, _: &mut AppController) -> Result<(), String> {
            Err("No playback host attached".into())
        }
    }
    run_project_with_host(&mut app, project, &mut SilentHost)
}

/// Host adapters own devices and the existing song renderer; the TUI owns neither.
pub trait ProjectPlaybackHost {
    fn poll(&mut self, app: &mut AppController) -> Result<(), String>;
    fn toggle_playback(&mut self, app: &mut AppController) -> Result<(), String>;
}

fn draw_project_performance(
    frame: &mut ratatui::Frame<'_>,
    project: &ProjectDirectory,
    source: &ValidatedSong,
    model: &AppViewModel,
    status: &str,
) {
    let mut lines = vec![
        Line::from(project.root().display().to_string()),
        Line::from(format!(
            "Performance · {:?} · frame {}",
            model.transport.state, model.transport.position
        )),
        Line::from(format!(
            "{} active patch instances · Authored definitions retained",
            source.pattern_voices().len()
        )),
        Line::from("Space: play/stop · Ctrl-Tab: construction · q/Esc: quit"),
        Line::from("Up/Down: select · Left/Right: level · Home: mute · End: default"),
        Line::from("Unsaved project; quitting discards authored edits"),
        Line::from(status.to_owned()),
    ];
    let mut count = 0;
    for voice in source.pattern_voices().values() {
        if let Some(control) = source.synths()[&voice.synth].output_level_control() {
            count += 1;
            let value = model
                .output_levels
                .get(&(voice.id.clone(), control.to_owned()))
                .copied()
                .unwrap_or(voice.level);
            let selected = model
                .instrument_editor
                .as_ref()
                .is_some_and(|editor| editor.synth == voice.synth);
            lines.push(Line::from(format!(
                "{} {}/{} = {:.2} (authored {:.2})",
                if selected { ">" } else { " " },
                voice.id,
                control,
                value,
                voice.level
            )));
        }
    }
    if count == 0 {
        lines.push(Line::from(
            "No controls declared; use x in construction to expose level",
        ));
    }
    frame.render_widget(
        Paragraph::new(lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("MELDRITCH · Project performance"),
            )
            .wrap(Wrap { trim: false }),
        frame.area(),
    );
}

pub fn run_project_with_host<H: ProjectPlaybackHost>(
    app: &mut AppController,
    project: &ProjectDirectory,
    host: &mut H,
) -> io::Result<()> {
    let view = ProjectOverview::from_project(project);
    let mut status = String::new();
    let mut terminal = TerminalGuard::enter()?;
    let result = (|| {
        loop {
            if let Err(error) = host.poll(app) {
                status = format!("Audio unchanged: {error}");
            }
            let model = app.view_model();
            terminal.terminal.draw(|frame| {
                if model.cockpit_mode == meldritch_app::CockpitMode::Performance {
                    draw_project_performance(
                        frame,
                        project,
                        &app.authored_source().unwrap(),
                        &model,
                        &status,
                    );
                } else if let Some(editor) = &model.instrument_editor
                    && editor.visible
                {
                    draw_instrument_editor(
                        frame,
                        &model,
                        editor,
                        &StatusMessage::info(format!("{status} · Unsaved project")),
                    );
                } else if let Some(editor) = &model.palette_pattern_editor
                    && editor.visible
                {
                    draw_project_pattern(frame, editor, &status);
                } else {
                    draw_project_construction_selected(
                        frame,
                        &view,
                        &app.authored_source().unwrap(),
                        &status,
                        model.instrument_editor.as_ref().map(|e| e.synth.as_str()),
                    );
                }
            })?;
            if event::poll(Duration::from_millis(100))?
                && let Event::Key(key) = event::read()?
            {
                let editing = model.instrument_editor.as_ref().is_some_and(|e| e.visible)
                    || model
                        .palette_pattern_editor
                        .as_ref()
                        .is_some_and(|e| e.visible);
                if !editing && empty_project_quit(key) {
                    return Ok(());
                }
                if !editing
                    && key.kind == KeyEventKind::Press
                    && key.code == KeyCode::Char(' ')
                    && key.modifiers == KeyModifiers::NONE
                {
                    status = match host.toggle_playback(app) {
                        Ok(()) => "Transport request accepted; project remains unsaved".into(),
                        Err(error) => format!("Playback unavailable: {error}"),
                    };
                    continue;
                }
                if key.kind == KeyEventKind::Press
                    && key.code == KeyCode::Tab
                    && key.modifiers == KeyModifiers::CONTROL
                {
                    if editing {
                        if model.instrument_editor.as_ref().is_some_and(|e| e.visible) {
                            app.dispatch(AppCommand::ToggleInstrumentEditor)
                                .map_err(|e| io::Error::other(format!("{e:?}")))?;
                        } else {
                            app.dispatch(AppCommand::TogglePatternEditor)
                                .map_err(|e| io::Error::other(format!("{e:?}")))?;
                        }
                    }
                    app.handle_input(AppInput::ToggleCockpitMode)
                        .map_err(|e| io::Error::other(format!("{e:?}")))?;
                    status = "Mode changed; transport and authored source retained".into();
                    continue;
                }
                if let Some(command) =
                    project_level_command(key, &model, &app.authored_source().unwrap())
                {
                    status = match app.dispatch(command) {
                        Ok(result) => command_result_text(&result),
                        Err(error) => format!("Control rejected: {error:?}"),
                    };
                    continue;
                }
                if model.cockpit_mode == meldritch_app::CockpitMode::Performance {
                    continue;
                }
                if let Some(command) =
                    project_binding_command(key, &model, &app.authored_source().unwrap())
                {
                    status = match app.dispatch(command) {
                        Ok(result) => {
                            format!("{} · Rendering requested", command_result_text(&result))
                        }
                        Err(e) => format!("Binding rejected; source retained: {e:?}"),
                    };
                } else if let Some(command) =
                    project_pattern_command(key, &model, &app.authored_source().unwrap())
                {
                    status = match app.dispatch(command) {
                        Ok(AppCommandResult::PatternSaveRequested) => {
                            "New-project saving is not implemented; edits retained in memory".into()
                        }
                        Ok(_) => "Accepted pattern action; project remains unsaved".into(),
                        Err(e) => format!("Note rejected; source retained: {e:?}"),
                    };
                } else if !editing
                    && let Some(command) =
                        project_construction_command(key, &app.authored_source().unwrap())
                {
                    status = match app.dispatch(command) {
                        Ok(AppCommandResult::PatchEdited { synth, .. }) => {
                            app.dispatch(AppCommand::SelectInstrument(synth))
                                .map_err(|e| io::Error::other(format!("{e:?}")))?;
                            app.observe_instrument_save(
                                "Unsaved new project; quitting discards edits".into(),
                            );
                            "Accepted silent palette definition; no pattern or routing created"
                                .into()
                        }
                        Ok(_) => unreachable!("creation returns a typed patch result"),
                        Err(e) => format!("Creation rejected: {e:?}"),
                    };
                } else if let Some(action) = project_editor_action(key, &model) {
                    match action {
                        TuiAction::Quit => return Ok(()),
                        TuiAction::Input(input) => {
                            status = match app.handle_input(input) {
                                Ok(AppCommandResult::InstrumentSaveRequested) => {
                                    let message = "New-project saving is not implemented; edits retained in memory";
                                    app.observe_instrument_save(message.into());
                                    message.into()
                                }
                                Ok(_) => "Accepted editor action; project remains unsaved".into(),
                                Err(e) => format!("Edit rejected; source retained: {e:?}"),
                            };
                        }
                    }
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
    fn native_level_is_explicit_and_performance_keys_preserve_authored_defaults() {
        let project = meldritch_dsl::load_project_directory(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../songs/examples/34-empty-project-patch"),
        )
        .unwrap();
        let (mut app, _engine) = AppController::new_empty_project(&project).unwrap();
        app.dispatch(AppCommand::CreatePatch(PatchCreation::mono_voice(
            "voice-1",
        )))
        .unwrap();
        app.dispatch(AppCommand::SelectInstrument("voice-1".into()))
            .unwrap();
        app.dispatch(AppCommand::CreatePatternVoice(PatternVoiceDefinition {
            id: "voice-1-instance".into(),
            synth: "voice-1".into(),
            length_ticks: 3840,
            looped: true,
            initial_pitch: 48,
            level: 0.8,
            pitch_pattern: None,
            gate_pattern: None,
        }))
        .unwrap();
        let key = |code| KeyEvent::new(code, KeyModifiers::NONE);
        let expose = project_level_command(
            key(KeyCode::Char('x')),
            &app.view_model(),
            &app.authored_source().unwrap(),
        )
        .unwrap();
        app.dispatch(expose).unwrap();
        let source = app.authored_source().unwrap();
        app.handle_input(AppInput::ToggleCockpitMode).unwrap();
        for code in [KeyCode::Home, KeyCode::Right] {
            let command = project_level_command(key(code), &app.view_model(), &source).unwrap();
            app.dispatch(command).unwrap();
        }
        assert_eq!(
            app.view_model().output_levels[&("voice-1-instance".into(), "level".into())],
            0.1
        );
        assert!(std::sync::Arc::ptr_eq(
            &source,
            &app.authored_source().unwrap()
        ));
        for kind in [KeyEventKind::Repeat, KeyEventKind::Release] {
            assert!(
                project_level_command(
                    KeyEvent {
                        kind,
                        ..key(KeyCode::Home)
                    },
                    &app.view_model(),
                    &source
                )
                .is_none()
            );
        }
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        terminal
            .draw(|frame| draw_project_performance(frame, &project, &source, &app.view_model(), ""))
            .unwrap();
        let screen = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect::<String>();
        assert!(screen.contains("> voice-1-instance/level = 0.10 (authored 0.80)"));
        assert!(!screen.contains("amp.audio"));
        let reset = project_level_command(key(KeyCode::End), &app.view_model(), &source).unwrap();
        app.dispatch(reset).unwrap();
        assert!(app.view_model().output_levels.is_empty());
    }

    #[test]
    fn native_composition_bindings_are_deliberate_independent_and_inspectable() {
        let project = meldritch_dsl::load_project_directory(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../songs/examples/34-empty-project-patch"),
        )
        .unwrap();
        let (mut app, _engine) = AppController::new_empty_project(&project).unwrap();
        app.dispatch(AppCommand::CreatePatch(PatchCreation::mono_voice(
            "voice-1",
        )))
        .unwrap();
        app.dispatch(AppCommand::SelectInstrument("voice-1".into()))
            .unwrap();
        app.dispatch(AppCommand::CreatePalettePattern {
            id: "pattern-1".into(),
            length_ticks: 3840,
            looped: true,
        })
        .unwrap();
        let press = |key| KeyEvent::new(KeyCode::Char(key), KeyModifiers::NONE);
        assert!(
            project_binding_command(
                press('v'),
                &app.view_model(),
                &app.authored_source().unwrap()
            )
            .is_none()
        );
        app.dispatch(AppCommand::TogglePatternEditor).unwrap();
        for key in ['v', 'p', 'g'] {
            let command = project_binding_command(
                press(key),
                &app.view_model(),
                &app.authored_source().unwrap(),
            )
            .unwrap();
            app.dispatch(command).unwrap();
        }
        let accepted = app.authored_source().unwrap();
        let voice = &accepted.pattern_voices()["voice-1-instance"];
        assert_eq!(voice.pitch_pattern.as_deref(), Some("pattern-1"));
        assert_eq!(voice.gate_pattern.as_deref(), Some("pattern-1"));
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        terminal
            .draw(|frame| {
                draw_project_construction(
                    frame,
                    &ProjectOverview::from_project(&project),
                    &accepted,
                    "",
                )
            })
            .unwrap();
        let screen = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect::<String>();
        for expected in [
            "voice-1-instance",
            "pattern-1.pitch -> input.pitch",
            "pattern-1.gate -> input.gate",
            "fallback low",
            "fallback C3",
            "amp.audio -> out.audio",
        ] {
            assert!(screen.contains(expected), "missing {expected} in {screen}");
        }
        let gate = project_binding_command(press('g'), &app.view_model(), &accepted).unwrap();
        app.dispatch(gate).unwrap();
        let next = app.authored_source().unwrap();
        assert_eq!(
            next.pattern_voices()["voice-1-instance"].pitch_pattern,
            voice.pitch_pattern
        );
        assert!(
            next.pattern_voices()["voice-1-instance"]
                .gate_pattern
                .is_none()
        );
        assert!(
            project_binding_command(
                KeyEvent::new_with_kind(
                    KeyCode::Char('g'),
                    KeyModifiers::NONE,
                    KeyEventKind::Repeat
                ),
                &app.view_model(),
                &next
            )
            .is_none()
        );
        app.dispatch(AppCommand::TogglePatternEditor).unwrap();
        assert!(app.view_model().palette_pattern_editor.unwrap().visible);
        app.handle_input(AppInput::ToggleCockpitMode).unwrap();
        assert!(
            project_binding_command(
                press('g'),
                &app.view_model(),
                &app.authored_source().unwrap()
            )
            .is_none()
        );
        assert!(
            project_pattern_command(
                press('c'),
                &app.view_model(),
                &app.authored_source().unwrap()
            )
            .is_none()
        );
        assert!(
            project_editor_action(
                KeyEvent::new(KeyCode::Char('e'), KeyModifiers::CONTROL),
                &app.view_model()
            )
            .is_none()
        );
        terminal
            .draw(|frame| {
                draw_project_performance(
                    frame,
                    &project,
                    &app.authored_source().unwrap(),
                    &app.view_model(),
                    "",
                )
            })
            .unwrap();
        let screen = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect::<String>();
        assert!(screen.contains("Project performance"));
        assert!(screen.contains("Ctrl-Tab: construction"));
        assert!(!screen.contains("amp.audio"));
    }

    #[test]
    fn native_palette_pattern_notes_need_no_instrument_or_track_and_preserve_ids() {
        let project = meldritch_dsl::load_project_directory(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../songs/examples/34-empty-project-patch"),
        )
        .unwrap();
        let (mut app, _engine) = AppController::new_empty_project(&project).unwrap();
        let press = |app: &mut AppController, code, modifiers| {
            let command = project_pattern_command(
                KeyEvent::new(code, modifiers),
                &app.view_model(),
                &app.authored_source().unwrap(),
            )
            .unwrap();
            app.dispatch(command)
        };
        press(&mut app, KeyCode::Char('c'), KeyModifiers::NONE).unwrap();
        let empty = app.authored_source().unwrap();
        assert!(empty.synths().is_empty());
        assert!(empty.performance().tracks().is_empty());
        assert!(
            empty
                .pattern_material("pattern-1")
                .unwrap()
                .notes
                .is_empty()
        );
        assert!(empty.authored_pattern("pattern-1").is_err());
        assert!(app.view_model().pattern_editor.is_none());
        press(&mut app, KeyCode::Enter, KeyModifiers::NONE).unwrap();
        let first = app.authored_source().unwrap();
        for _ in 0..2 {
            press(&mut app, KeyCode::Right, KeyModifiers::NONE).unwrap();
        }
        for _ in 0..7 {
            press(&mut app, KeyCode::Up, KeyModifiers::NONE).unwrap();
        }
        assert!(std::sync::Arc::ptr_eq(
            &first,
            &app.authored_source().unwrap()
        ));
        press(&mut app, KeyCode::Enter, KeyModifiers::NONE).unwrap();
        press(&mut app, KeyCode::Char(']'), KeyModifiers::NONE).unwrap();
        press(&mut app, KeyCode::Char('-'), KeyModifiers::NONE).unwrap();
        let material = app
            .authored_source()
            .unwrap()
            .pattern_material("pattern-1")
            .unwrap();
        assert_eq!(
            (
                &material.notes[0].id,
                &material.notes[0].note,
                material.notes[0].start_tick,
                material.notes[0].duration_ticks
            ),
            (&"note-1".to_owned(), &"C3".to_owned(), 0, 480)
        );
        assert_eq!(
            (
                &material.notes[1].note,
                material.notes[1].start_tick,
                material.notes[1].duration_ticks
            ),
            (&"G3".to_owned(), 960, 720)
        );
        assert!((material.notes[1].velocity - 0.7).abs() < 1e-12);
        press(&mut app, KeyCode::Delete, KeyModifiers::NONE).unwrap();
        press(&mut app, KeyCode::Enter, KeyModifiers::NONE).unwrap();
        press(&mut app, KeyCode::Enter, KeyModifiers::NONE).unwrap(); // select, never duplicate
        let accepted = app.authored_source().unwrap();
        let material = accepted.pattern_material("pattern-1").unwrap();
        assert_eq!(material.notes.len(), 2);
        assert_eq!(material.notes[1].id, "note-3");
        assert_eq!(material.next_note_id, 4);
        let history = app.history().len();
        assert!(
            app.dispatch(AppCommand::EditNote {
                pattern: "pattern-1".into(),
                edit: meldritch_app::NoteEdit::Duration {
                    id: "note-3".into(),
                    duration_ticks: 4000
                }
            })
            .is_err()
        );
        assert_eq!(app.history().len(), history);
        assert!(std::sync::Arc::ptr_eq(
            &accepted,
            &app.authored_source().unwrap()
        ));
        for kind in [KeyEventKind::Repeat, KeyEventKind::Release] {
            assert!(
                project_pattern_command(
                    KeyEvent {
                        kind,
                        ..KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)
                    },
                    &app.view_model(),
                    &accepted
                )
                .is_none()
            );
        }
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        terminal
            .draw(|frame| {
                draw_project_pattern(
                    frame,
                    &app.view_model().palette_pattern_editor.unwrap(),
                    "Accepted",
                )
            })
            .unwrap();
        let text = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect::<String>();
        for expected in [
            "pitch : Pitch",
            "gate : Gate",
            "note-1 C3 @0 +480",
            "note-3 G3 @960 +480",
            "no composition bindings",
        ] {
            assert!(text.contains(expected), "missing {expected}");
        }
        press(&mut app, KeyCode::Esc, KeyModifiers::NONE).unwrap();
        press(&mut app, KeyCode::Char('n'), KeyModifiers::CONTROL).unwrap();
        assert!(std::sync::Arc::ptr_eq(
            &accepted,
            &app.authored_source().unwrap()
        ));
        assert!(accepted.performance().tracks().is_empty());
        let (mut replay, _replay_engine) = AppController::new_empty_project(&project).unwrap();
        for record in app.history() {
            replay.dispatch(record.command.clone()).unwrap();
        }
        assert_eq!(
            replay.authored_source().unwrap().fingerprint(),
            accepted.fingerprint()
        );
    }

    #[test]
    fn native_patch_editing_keeps_project_provenance_and_accepts_only_complete_drafts() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../songs/examples/34-empty-project-patch");
        let project = meldritch_dsl::load_project_directory(&root).unwrap();
        let (mut app, _engine) = AppController::new_empty_project(&project).unwrap();
        let baseline = app.authored_source().unwrap();
        app.dispatch(
            project_construction_command(
                KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE),
                &baseline,
            )
            .unwrap(),
        )
        .unwrap();
        let created = app.authored_source().unwrap();
        app.dispatch(AppCommand::SelectInstrument("voice-1".into()))
            .unwrap();
        let press = |app: &mut AppController, code, modifiers| {
            let action =
                project_editor_action(KeyEvent::new(code, modifiers), &app.view_model()).unwrap();
            let TuiAction::Input(input) = action else {
                panic!("editor input")
            };
            app.handle_input(input)
        };
        press(&mut app, KeyCode::Char('e'), KeyModifiers::CONTROL).unwrap();
        press(&mut app, KeyCode::Right, KeyModifiers::NONE).unwrap();
        assert_ne!(
            created.fingerprint(),
            app.authored_source().unwrap().fingerprint()
        );
        assert_eq!(
            app.view_model().instrument_editor.unwrap().base_fingerprint,
            baseline.fingerprint()
        );
        press(&mut app, KeyCode::Char('g'), KeyModifiers::CONTROL).unwrap();
        let line = |app: &mut AppController, text: &str| {
            for c in text.chars() {
                press(app, KeyCode::Char(c), KeyModifiers::NONE).unwrap();
            }
            press(app, KeyCode::Enter, KeyModifiers::NONE)
        };
        let script = std::fs::read_to_string(root.join("keyboard-patch.txt")).unwrap();
        let lines: Vec<_> = script
            .lines()
            .filter(|l| !l.starts_with('#') && !l.is_empty())
            .collect();
        let accepted = app.authored_source().unwrap();
        line(&mut app, lines[0]).unwrap();
        assert!(std::sync::Arc::ptr_eq(
            &accepted,
            &app.authored_source().unwrap()
        ));
        let history = app.history().len();
        assert!(line(&mut app, "commit").is_err());
        assert_eq!(app.history().len(), history + "commit".len());
        assert!(std::sync::Arc::ptr_eq(
            &accepted,
            &app.authored_source().unwrap()
        ));
        for _ in 0.."commit".len() {
            press(&mut app, KeyCode::Backspace, KeyModifiers::NONE).unwrap();
        }
        for text in &lines[1..] {
            line(&mut app, text).unwrap();
        }
        let committed = app.authored_source().unwrap();
        assert_eq!(committed.synths()["voice-1"].modules().len(), 5);
        assert_eq!(committed.synths()["voice-1"].cables().len(), 6);
        assert_eq!(
            committed.synths()["voice-1"]
                .modules()
                .iter()
                .find(|m| m.id() == "tone")
                .unwrap()
                .cutoff_hz(),
            Some(1800.0)
        );
        press(&mut app, KeyCode::Char('q'), KeyModifiers::NONE).unwrap();
        assert_eq!(app.view_model().instrument_editor.unwrap().patch_input, "q");
        press(&mut app, KeyCode::Esc, KeyModifiers::NONE).unwrap();
        press(&mut app, KeyCode::Esc, KeyModifiers::NONE).unwrap();
        assert!(!app.view_model().instrument_editor.unwrap().visible);
        for kind in [KeyEventKind::Repeat, KeyEventKind::Release] {
            assert!(
                project_editor_action(
                    KeyEvent {
                        kind,
                        ..KeyEvent::new(KeyCode::Right, KeyModifiers::NONE)
                    },
                    &app.view_model()
                )
                .is_none()
            );
        }
        assert!(std::sync::Arc::ptr_eq(
            &committed,
            &app.authored_source().unwrap()
        ));
        assert_eq!(
            app.view_model().instrument_editor.unwrap().base_fingerprint,
            baseline.fingerprint()
        );
        assert!(committed.performance().tracks().is_empty());
        assert_eq!(
            project_editor_action(
                KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE),
                &app.view_model()
            ),
            Some(TuiAction::Quit)
        );
    }

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
