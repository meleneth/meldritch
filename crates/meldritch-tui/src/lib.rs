//! Ratatui/Crossterm frontend for the headless application controller.

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use meldritch_app::{AppCommandResult, AppController, AppInput, AppViewModel};
use meldritch_core::Step;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use std::io::{self, Stdout};
use std::time::Duration;

#[derive(Clone, Debug, PartialEq)]
pub enum TuiAction {
    Quit,
    Input(AppInput),
}

pub fn map_key(key: KeyEvent, default_step: &Step) -> Option<TuiAction> {
    if key.kind != KeyEventKind::Press {
        return None;
    }
    let action = match key.code {
        KeyCode::Char('q') => TuiAction::Quit,
        KeyCode::Left | KeyCode::Char('h') => TuiAction::Input(AppInput::MoveLeft),
        KeyCode::Right | KeyCode::Char('l') => TuiAction::Input(AppInput::MoveRight),
        KeyCode::Up | KeyCode::Char('k') => TuiAction::Input(AppInput::MoveUp),
        KeyCode::Down | KeyCode::Char('j') => TuiAction::Input(AppInput::MoveDown),
        KeyCode::Char(' ') => TuiAction::Input(AppInput::ToggleSelected(default_step.clone())),
        KeyCode::Char('p') => TuiAction::Input(AppInput::TogglePlayback),
        KeyCode::Char('s') => TuiAction::Input(AppInput::Stop),
        KeyCode::Char('r') => TuiAction::Input(AppInput::Rewind),
        KeyCode::Char('+') | KeyCode::Char('=') => TuiAction::Input(AppInput::IncreaseVelocity),
        KeyCode::Char('-') => TuiAction::Input(AppInput::DecreaseVelocity),
        KeyCode::Char(']') => TuiAction::Input(AppInput::IncreaseGate),
        KeyCode::Char('[') => TuiAction::Input(AppInput::DecreaseGate),
        KeyCode::Char('>') | KeyCode::Char('.') => TuiAction::Input(AppInput::IncreaseProbability),
        KeyCode::Char('<') | KeyCode::Char(',') => TuiAction::Input(AppInput::DecreaseProbability),
        KeyCode::Char('a') => TuiAction::Input(AppInput::IncreaseCutoff),
        KeyCode::Char('z') => TuiAction::Input(AppInput::DecreaseCutoff),
        KeyCode::Char('d') => TuiAction::Input(AppInput::IncreaseResonance),
        KeyCode::Char('x') => TuiAction::Input(AppInput::DecreaseResonance),
        KeyCode::Char('w') => TuiAction::Input(AppInput::CycleWaveform),
        KeyCode::Char('g') => TuiAction::Input(AppInput::IncreaseFilterEnvelope),
        KeyCode::Char('b') => TuiAction::Input(AppInput::DecreaseFilterEnvelope),
        KeyCode::Char('t') => TuiAction::Input(AppInput::IncreaseDrive),
        KeyCode::Char('y') => TuiAction::Input(AppInput::DecreaseDrive),
        KeyCode::Char('v') => TuiAction::Input(AppInput::IncreaseSynthLevel),
        KeyCode::Char('c') => TuiAction::Input(AppInput::DecreaseSynthLevel),
        KeyCode::Char('2') => TuiAction::Input(AppInput::IncreaseAttack),
        KeyCode::Char('1') => TuiAction::Input(AppInput::DecreaseAttack),
        KeyCode::Char('4') => TuiAction::Input(AppInput::IncreaseDecay),
        KeyCode::Char('3') => TuiAction::Input(AppInput::DecreaseDecay),
        KeyCode::Char('6') => TuiAction::Input(AppInput::IncreaseSustain),
        KeyCode::Char('5') => TuiAction::Input(AppInput::DecreaseSustain),
        KeyCode::Char('8') => TuiAction::Input(AppInput::IncreaseRelease),
        KeyCode::Char('7') => TuiAction::Input(AppInput::DecreaseRelease),
        KeyCode::Char('m') => TuiAction::Input(AppInput::IncreaseSubLevel),
        KeyCode::Char('n') => TuiAction::Input(AppInput::DecreaseSubLevel),
        KeyCode::Char('o') => TuiAction::Input(AppInput::IncreaseGlide),
        KeyCode::Char('i') => TuiAction::Input(AppInput::DecreaseGlide),
        KeyCode::Char('0') => TuiAction::Input(AppInput::IncreaseDucking),
        KeyCode::Char('9') => TuiAction::Input(AppInput::DecreaseDucking),
        KeyCode::Char('M') => TuiAction::Input(AppInput::IncreaseDuckingRelease),
        KeyCode::Char('N') => TuiAction::Input(AppInput::DecreaseDuckingRelease),
        KeyCode::Char('G') => TuiAction::Input(AppInput::IncreaseHatFilter),
        KeyCode::Char('B') => TuiAction::Input(AppInput::DecreaseHatFilter),
        KeyCode::Char('T') => TuiAction::Input(AppInput::IncreaseHatFilterRelease),
        KeyCode::Char('Y') => TuiAction::Input(AppInput::DecreaseHatFilterRelease),
        KeyCode::Char('\'') => TuiAction::Input(AppInput::IncreaseNote),
        KeyCode::Char(';') => TuiAction::Input(AppInput::DecreaseNote),
        KeyCode::Char('L') => TuiAction::Input(AppInput::TransposeChordUp),
        KeyCode::Char('H') => TuiAction::Input(AppInput::TransposeChordDown),
        KeyCode::Char('I') => TuiAction::Input(AppInput::InvertChordUp),
        KeyCode::Char('U') => TuiAction::Input(AppInput::InvertChordDown),
        KeyCode::Char('R') => TuiAction::Input(AppInput::CreateReverse),
        KeyCode::Char('S') => TuiAction::Input(AppInput::CreateReslice),
        KeyCode::Char('F') => TuiAction::Input(AppInput::CreateFreeze),
        KeyCode::Char('E') => TuiAction::Input(AppInput::CreateSmear),
        KeyCode::Char('A') => TuiAction::Input(AppInput::AuditionTransform),
        KeyCode::Char('D') => TuiAction::Input(AppInput::ReturnToLive),
        KeyCode::Char('Q') => TuiAction::Input(AppInput::QueueNextScene),
        KeyCode::Char('Z') => TuiAction::Input(AppInput::ToggleTrackMute),
        KeyCode::Char('P') => TuiAction::Input(AppInput::TriggerFill),
        KeyCode::Char('C') => TuiAction::Input(AppInput::CancelPerformance),
        _ => return None,
    };
    Some(action)
}

pub fn run(controller: &mut AppController, default_step: Step) -> io::Result<()> {
    run_with_tick(controller, default_step, |_| Ok(None))
}

pub fn run_with_tick<F>(
    controller: &mut AppController,
    default_step: Step,
    tick: F,
) -> io::Result<()>
where
    F: FnMut(&mut AppController) -> Result<Option<String>, String>,
{
    run_with_hooks(controller, default_step, tick, |_, _| {})
}

pub fn run_with_hooks<F, I>(
    controller: &mut AppController,
    default_step: Step,
    tick: F,
    on_input: I,
) -> io::Result<()>
where
    F: FnMut(&mut AppController) -> Result<Option<String>, String>,
    I: FnMut(&AppController, &AppInput),
{
    let mut terminal = TerminalGuard::enter()?;
    let result = run_loop(
        &mut terminal.terminal,
        controller,
        &default_step,
        tick,
        on_input,
    );
    terminal.restore()?;
    result
}

fn run_loop<F, I>(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    controller: &mut AppController,
    default_step: &Step,
    mut tick: F,
    mut on_input: I,
) -> io::Result<()>
where
    F: FnMut(&mut AppController) -> Result<Option<String>, String>,
    I: FnMut(&AppController, &AppInput),
{
    let mut status = StatusMessage::info("Ready");
    loop {
        match tick(controller) {
            Ok(Some(message)) => status = StatusMessage::info(message),
            Err(message) => status = StatusMessage::error(message),
            Ok(None) => {}
        }
        let view = controller.view_model();
        terminal.draw(|frame| draw_with_status(frame, &view, &status))?;
        if !event::poll(Duration::from_millis(50))? {
            continue;
        }
        let Event::Key(key) = event::read()? else {
            continue;
        };
        match map_key(key, default_step) {
            Some(TuiAction::Quit) => return Ok(()),
            Some(TuiAction::Input(input)) => {
                status = match controller.handle_input(input.clone()) {
                    Ok(result) => {
                        on_input(controller, &input);
                        StatusMessage::info(command_result_text(&result))
                    }
                    Err(error) => StatusMessage::error(format!("{error:?}")),
                };
            }
            None => {}
        }
    }
}

struct TerminalGuard {
    terminal: Terminal<CrosstermBackend<Stdout>>,
    restored: bool,
}

impl TerminalGuard {
    fn enter() -> io::Result<Self> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        if let Err(error) = execute!(stdout, EnterAlternateScreen) {
            let _ = disable_raw_mode();
            return Err(error);
        }
        let backend = CrosstermBackend::new(stdout);
        match Terminal::new(backend) {
            Ok(terminal) => Ok(Self {
                terminal,
                restored: false,
            }),
            Err(error) => {
                let _ = disable_raw_mode();
                let _ = execute!(io::stdout(), LeaveAlternateScreen);
                Err(error)
            }
        }
    }

    fn restore(&mut self) -> io::Result<()> {
        if self.restored {
            return Ok(());
        }
        disable_raw_mode()?;
        execute!(self.terminal.backend_mut(), LeaveAlternateScreen)?;
        self.terminal.show_cursor()?;
        self.restored = true;
        Ok(())
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        if !self.restored {
            let _ = disable_raw_mode();
            let _ = execute!(self.terminal.backend_mut(), LeaveAlternateScreen);
            let _ = self.terminal.show_cursor();
        }
    }
}

pub fn draw(frame: &mut ratatui::Frame<'_>, view: &AppViewModel) {
    draw_with_status(frame, view, &StatusMessage::info("Ready"));
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StatusMessage {
    pub text: String,
    pub error: bool,
}

impl StatusMessage {
    #[must_use]
    pub fn info(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            error: false,
        }
    }

    #[must_use]
    pub fn error(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            error: true,
        }
    }
}

pub fn draw_with_status(
    frame: &mut ratatui::Frame<'_>,
    view: &AppViewModel,
    status: &StatusMessage,
) {
    let performance_visible = view.performance.queued.is_some()
        || view.performance.active_scene.is_some()
        || !view.performance.muted_tracks.is_empty()
        || view.performance.active_fill.is_some()
        || view.performance.diagnostics.last_launch.is_some();
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(if view.arrangement.is_some() { 6 } else { 3 }),
            Constraint::Length(if view.automation.is_some() { 6 } else { 0 }),
            Constraint::Length(if view.effect_sends.is_some() { 5 } else { 0 }),
            Constraint::Length(if view.sidechain.is_some() { 3 } else { 0 }),
            Constraint::Length(if view.transform.is_some() { 4 } else { 0 }),
            Constraint::Length(if view.futures.is_some() { 5 } else { 0 }),
            Constraint::Length(if performance_visible { 4 } else { 0 }),
            Constraint::Min(7),
            Constraint::Length(5),
            Constraint::Length(3),
            Constraint::Length(6),
        ])
        .split(frame.area());
    if view.arrangement.is_some() {
        let transport_rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Length(3)])
            .split(rows[0]);
        draw_transport(frame, transport_rows[0], view);
        draw_arrangement(frame, transport_rows[1], view);
    } else {
        draw_transport(frame, rows[0], view);
    }
    if view.automation.is_some() {
        draw_automation(frame, rows[1], view);
    }
    if view.effect_sends.is_some() {
        draw_effect_sends(frame, rows[2], view);
    }
    if view.sidechain.is_some() {
        draw_sidechain(frame, rows[3], view);
    }
    if view.transform.is_some() {
        draw_transform(frame, rows[4], view);
    }
    if view.futures.is_some() {
        draw_futures(frame, rows[5], view);
    }
    if performance_visible {
        draw_performance(frame, rows[6], view);
    }
    let middle = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
        .split(rows[7]);
    draw_grid(frame, middle[0], view);
    draw_inspector(frame, middle[1], view);
    let bottom = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(rows[8]);
    draw_diagnostics(frame, bottom[0], view);
    draw_history(frame, bottom[1], view);
    draw_status(frame, rows[9], status);
    draw_key_legend(frame, rows[10]);
}

fn draw_transport(frame: &mut ratatui::Frame<'_>, area: Rect, view: &AppViewModel) {
    let mut text = format!(
        "{:?}  frame {}  callbacks {}  underruns {}  misses {}",
        view.transport.state,
        view.transport.position,
        view.transport.callbacks,
        view.transport.underruns,
        view.transport.missed_artifacts
    );
    if let Some(queued) = view.performance.queued {
        text.push_str(&format!(
            "  queued {:?}@{}",
            queued.gesture, queued.launch_frame
        ));
    }
    frame.render_widget(panel("Transport", Paragraph::new(text)), area);
}

fn draw_arrangement(frame: &mut ratatui::Frame<'_>, area: Rect, view: &AppViewModel) {
    let Some(arrangement) = &view.arrangement else {
        return;
    };
    let mut spans = Vec::new();
    for section in &arrangement.sections {
        let label = format!(
            " {}:S{}×{}{} ",
            section.index + 1,
            section.scene.raw(),
            section.repeats,
            if section.active {
                arrangement
                    .active_repeat
                    .map_or(String::new(), |repeat| format!(" R{}", repeat + 1))
            } else {
                String::new()
            }
        );
        let style = if section.active {
            Style::default()
                .fg(Color::Black)
                .bg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else if section.in_loop {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        spans.push(Span::styled(label, style));
    }
    frame.render_widget(
        panel(
            &format!(
                "Arrangement loop {}..{}",
                arrangement.loop_sections.0, arrangement.loop_sections.1
            ),
            Paragraph::new(Line::from(spans)),
        ),
        area,
    );
}

fn draw_automation(frame: &mut ratatui::Frame<'_>, area: Rect, view: &AppViewModel) {
    let Some(automation) = &view.automation else {
        return;
    };
    let lines = automation
        .lanes
        .iter()
        .map(|lane| {
            let current = match lane.current {
                meldritch_core::AutomationValue::Continuous(value) => format!("{value:.3}"),
                meldritch_core::AutomationValue::Discrete(value) => value.to_string(),
            };
            let next = lane.next_point.map_or_else(
                || "end".to_owned(),
                |(frame, value)| {
                    let value = match value {
                        meldritch_core::AutomationValue::Continuous(value) => {
                            format!("{value:.3}")
                        }
                        meldritch_core::AutomationValue::Discrete(value) => value.to_string(),
                    };
                    format!("{value}@{frame}")
                },
            );
            Span::styled(
                format!(
                    " {:?}={current} ({:?} → {next}) ",
                    lane.target, lane.interpolation
                ),
                if lane.target == meldritch_core::AutomationTarget::Scene {
                    Style::default().fg(Color::Yellow)
                } else {
                    Style::default().fg(Color::Green)
                },
            )
        })
        .collect::<Vec<_>>();
    let rows = lines
        .chunks(3)
        .map(|chunk| Line::from(chunk.to_vec()))
        .collect::<Vec<_>>();
    frame.render_widget(
        panel(
            &format!(
                "Automation · scene {}",
                automation
                    .scene
                    .map_or("—".to_owned(), |scene| scene.to_string())
            ),
            Paragraph::new(rows).wrap(Wrap { trim: true }),
        ),
        area,
    );
}

fn draw_effect_sends(frame: &mut ratatui::Frame<'_>, area: Rect, view: &AppViewModel) {
    let Some(sends) = &view.effect_sends else {
        return;
    };
    let lines = sends
        .recent
        .iter()
        .rev()
        .map(|send| {
            Line::from(format!(
                "f{} T{}:{} {:?} tag {:?} gain {:.2}",
                send.frame,
                send.track.raw(),
                send.step.raw(),
                send.bus,
                send.matched_tag,
                send.send_gain
            ))
        })
        .collect::<Vec<_>>();
    frame.render_widget(panel("Effect Sends · why", Paragraph::new(lines)), area);
}

fn draw_sidechain(frame: &mut ratatui::Frame<'_>, area: Rect, view: &AppViewModel) {
    let Some(sidechain) = view.sidechain else {
        return;
    };
    let bands = match (sidechain.bands.low, sidechain.bands.high) {
        (true, true) => "low+high",
        (true, false) => "low",
        (false, true) => "high",
        (false, false) => "none",
    };
    frame.render_widget(
        panel(
            "Sidechain · attenuation",
            Paragraph::new(format!(
                "{:?} → {:?}  bands {bands}  attenuation {:.1}%",
                sidechain.source_role,
                sidechain.target_role,
                sidechain.attenuation * 100.0
            )),
        ),
        area,
    );
}

fn draw_transform(frame: &mut ratatui::Frame<'_>, area: Rect, view: &AppViewModel) {
    let Some(transform) = &view.transform else {
        return;
    };
    frame.render_widget(
        panel(
            "Derived Transform · provenance",
            Paragraph::new(format!(
                "{:?}  {:?}  {}\n{}ch × {}f  fingerprint {}",
                transform.transform,
                transform.status,
                if transform.auditioning {
                    "AUDITION"
                } else {
                    "LIVE"
                },
                transform.channels,
                transform.frames,
                transform.key.fingerprint.raw()
            )),
        ),
        area,
    );
}

fn draw_grid(frame: &mut ratatui::Frame<'_>, area: Rect, view: &AppViewModel) {
    let available_rows = area.height.saturating_sub(2) as usize;
    let selected_row = view
        .pattern_grid
        .tracks
        .iter()
        .position(|row| row.selected)
        .unwrap_or(0);
    let row_offset = viewport_offset(selected_row, available_rows, view.pattern_grid.tracks.len());
    let available_steps = area.width.saturating_sub(7) as usize / 3;
    let selected_step = view.inspector.selection.step.raw() as usize;
    let step_offset = viewport_offset(
        selected_step,
        available_steps,
        view.pattern_grid.length_steps as usize,
    );
    let lines = view
        .pattern_grid
        .tracks
        .iter()
        .skip(row_offset)
        .take(available_rows)
        .map(|row| {
            let mut spans = vec![Span::styled(
                format!("T{:>2} ", row.track.raw()),
                if row.selected {
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                },
            )];
            spans.extend(
                row.steps
                    .iter()
                    .skip(step_offset)
                    .take(available_steps)
                    .map(|cell| {
                        let symbol = if cell.value.is_some() { "●" } else { "·" };
                        let style = if cell.selected {
                            Style::default().fg(Color::Black).bg(Color::Yellow)
                        } else if cell.value.is_some() {
                            Style::default().fg(Color::Green)
                        } else {
                            Style::default().fg(Color::DarkGray)
                        };
                        Span::styled(format!(" {symbol} "), style)
                    }),
            );
            Line::from(spans)
        })
        .collect::<Vec<_>>();
    frame.render_widget(
        panel(
            &format!("Pattern {}", view.pattern_grid.pattern.raw()),
            Paragraph::new(lines),
        ),
        area,
    );
}

fn viewport_offset(selected: usize, visible: usize, total: usize) -> usize {
    if visible == 0 || total <= visible {
        return 0;
    }
    selected
        .saturating_sub(visible / 2)
        .min(total.saturating_sub(visible))
}

fn draw_inspector(frame: &mut ratatui::Frame<'_>, area: Rect, view: &AppViewModel) {
    let selection = view.inspector.selection;
    let text = match &view.inspector.value {
        Some(step) => format!(
            "track {}\nstep {}\nnote {}\nvelocity {:.3}\ngate {:.3}\nprobability {:.3}\ntags {:?}",
            selection.track.raw(),
            selection.step.raw(),
            step.note(),
            step.velocity(),
            step.gate(),
            step.probability().chance(),
            step.tags()
        ),
        None => format!(
            "track {}\nstep {}\n(empty)",
            selection.track.raw(),
            selection.step.raw()
        ),
    };
    let text = if let Some(voice) = view.bass_voice {
        format!(
            "{text}\n{:?} cutoff {:.1} Hz resonance {:.2}\nenv {:.2} oct drive {:.2} level {:.2}\nADSR {:.3} {:.3} {:.2} {:.3}\nsub {:.2} glide {:.3}s\nduck {:.2} recovery {:.3}s\nhat {:.2} oct recovery {:.3}s",
            voice.waveform,
            voice.cutoff_hz,
            voice.resonance,
            voice.filter_envelope_octaves,
            voice.drive,
            voice.level,
            voice.attack_seconds,
            voice.decay_seconds,
            voice.sustain_level,
            voice.release_seconds,
            voice.sub_level,
            voice.glide_seconds,
            voice.ducking_amount,
            voice.ducking_release_seconds,
            voice.hat_filter_octaves,
            voice.hat_filter_release_seconds,
        )
    } else {
        text
    };
    frame.render_widget(panel("Inspector", Paragraph::new(text)), area);
}

fn draw_diagnostics(frame: &mut ratatui::Frame<'_>, area: Rect, view: &AppViewModel) {
    let render = view.diagnostics.render;
    let text = format!(
        "workers q:{} active:{} done:{}\nchunks ready:{} published:{} invalidated:{} stale:{}\ncommands applied:{} dropped:{}  voices {}/{} steals:{}",
        render.workers.queued_jobs,
        render.workers.active_jobs,
        render.workers.completed_jobs,
        render.publication.ready_chunks,
        render.publication.published_artifacts,
        render.publication.invalidated_chunks,
        render.publication.stale_artifacts,
        view.diagnostics.transport_commands.applied,
        view.diagnostics.transport_commands.dropped,
        render.chord_active_voices,
        render.chord_peak_voices,
        render.chord_voice_steals,
    );
    frame.render_widget(panel("Diagnostics", Paragraph::new(text)), area);
}

fn draw_futures(frame: &mut ratatui::Frame<'_>, area: Rect, view: &AppViewModel) {
    let Some(futures) = &view.futures else {
        return;
    };
    let diagnostics = futures.diagnostics;
    let state = if diagnostics.sleeping {
        "sleeping"
    } else {
        "working"
    };
    let mut lines = vec![Line::from(format!(
        "{state}  clean {}/{}  q:{} active:{} done:{} unresolved:{}",
        diagnostics.clean_artifacts,
        diagnostics.desired_artifacts,
        diagnostics.queued_jobs,
        diagnostics.active_jobs,
        diagnostics.completed_jobs,
        futures.unresolved,
    ))];
    lines.extend(
        futures
            .candidates
            .iter()
            .take(area.height.saturating_sub(3) as usize)
            .map(|candidate| {
                Line::from(format!(
                    "{:?} score:{} {:?} fp:{}",
                    candidate.gesture,
                    candidate.score,
                    candidate.status,
                    candidate.key.fingerprint().raw(),
                ))
            }),
    );
    frame.render_widget(panel("Performance Futures", Paragraph::new(lines)), area);
}

fn draw_performance(frame: &mut ratatui::Frame<'_>, area: Rect, view: &AppViewModel) {
    let performance = &view.performance;
    let queued = performance.queued.map_or_else(
        || "none".to_owned(),
        |queued| format!("{:?}@{}", queued.gesture, queued.launch_frame),
    );
    let active = format!(
        "scene:{:?} muted:{:?} fill:{:?}→{:?}",
        performance.active_scene,
        performance.muted_tracks,
        performance.active_fill,
        performance.fill_end_frame,
    );
    let diagnostics = performance.diagnostics;
    let launches = format!(
        "queued:{} cancel:{} launch prepared:{} fallback:{} returns:{} last:{:?}",
        diagnostics.queued_gestures,
        diagnostics.cancelled_gestures,
        diagnostics.speculative_launches,
        diagnostics.fallback_launches,
        diagnostics.fill_returns,
        diagnostics.last_launch.map(|launch| launch.source),
    );
    frame.render_widget(
        panel(
            "Live Performance",
            Paragraph::new(vec![
                Line::from(format!("queued {queued}  {active}")),
                Line::from(launches),
            ]),
        ),
        area,
    );
}

fn draw_history(frame: &mut ratatui::Frame<'_>, area: Rect, view: &AppViewModel) {
    let lines = view
        .history
        .iter()
        .rev()
        .take(area.height.saturating_sub(2) as usize)
        .map(|record| Line::from(format!("#{} {:?}", record.sequence, record.command)))
        .collect::<Vec<_>>();
    frame.render_widget(
        panel("History", Paragraph::new(lines).wrap(Wrap { trim: true })),
        area,
    );
}

fn draw_key_legend(frame: &mut ratatui::Frame<'_>, area: Rect) {
    let keys = [
        ("arrows/hjkl", "move"),
        ("space", "toggle step"),
        ("+/-", "velocity"),
        ("[]", "gate"),
        ("<>", "probability"),
        ("Q/Z/P/C", "perform"),
        ("a/z", "cutoff"),
        ("d/x", "resonance"),
        ("w", "waveform"),
        ("g/b", "filter env"),
        ("t/y", "drive"),
        ("v/c", "level"),
        ("1-8", "ADSR -/+"),
        ("n/m", "sub"),
        ("i/o", "glide"),
        ("9/0", "duck"),
        ("N/M", "recovery"),
        ("B/G", "hat env"),
        ("Y/T", "hat decay"),
        (";/'", "note"),
        ("H/L", "chord +/-"),
        ("U/I", "invert"),
        ("R/S/F/E", "transform"),
        ("A/D", "audition/live"),
        ("p", "play/pause"),
        ("s", "stop"),
        ("r", "rewind"),
        ("q", "quit"),
    ];
    let mut lines = vec![Vec::new(), Vec::new(), Vec::new(), Vec::new()];
    for (index, (key, action)) in keys.into_iter().enumerate() {
        let line = if index < 8 {
            0
        } else if index < 15 {
            1
        } else if index < 22 {
            2
        } else {
            3
        };
        if !lines[line].is_empty() {
            lines[line].push(Span::raw("  "));
        }
        lines[line].push(Span::styled(
            key,
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ));
        lines[line].push(Span::raw(format!(" {action}")));
    }
    frame.render_widget(
        panel(
            "Keys",
            Paragraph::new(lines.into_iter().map(Line::from).collect::<Vec<_>>()),
        ),
        area,
    );
}

fn draw_status(frame: &mut ratatui::Frame<'_>, area: Rect, status: &StatusMessage) {
    let style = if status.error {
        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Cyan)
    };
    frame.render_widget(
        panel("Status", Paragraph::new(status.text.clone()).style(style)),
        area,
    );
}

fn command_result_text(result: &AppCommandResult) -> String {
    match result {
        AppCommandResult::TransportQueued => "Transport command queued".to_owned(),
        AppCommandResult::SelectionChanged { current, .. } => format!(
            "Selected track {} step {}",
            current.track.raw(),
            current.step.raw()
        ),
        AppCommandResult::Edit(edit) if edit.changed => format!(
            "Edited {} dirty range(s), invalidated {} chunk(s)",
            edit.dirty_ranges.len(),
            edit.invalidated_chunks
        ),
        AppCommandResult::Edit(_) => "Edit made no change".to_owned(),
        AppCommandResult::SynthUpdated { invalidated_chunks } => {
            format!("Synth updated, invalidated {invalidated_chunks} chunk(s)")
        }
        AppCommandResult::TransformCreated { key, status } => format!(
            "Transform {status:?}: fingerprint {}",
            key.fingerprint.raw()
        ),
        AppCommandResult::AudioSourceSwitched { transformed } => {
            if *transformed {
                "Auditioning transformed source".to_owned()
            } else {
                "Returned to live worker audio".to_owned()
            }
        }
        AppCommandResult::PerformanceQueued(queued) => format!(
            "Queued {:?} for frame {}",
            queued.gesture, queued.launch_frame
        ),
        AppCommandResult::PerformanceCancelled(Some(queued)) => {
            format!("Cancelled {:?}", queued.gesture)
        }
        AppCommandResult::PerformanceCancelled(None) => "No performance gesture queued".to_owned(),
    }
}

fn panel<'a>(title: &'a str, paragraph: Paragraph<'a>) -> Paragraph<'a> {
    paragraph.block(Block::default().title(title).borders(Borders::ALL))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyModifiers;
    use meldritch_app::{
        AppDiagnostics, ArrangementSectionView, ArrangementView, AutomationLaneView,
        AutomationView, EffectSendView, FutureCacheView, PatternGridView, PerformanceView,
        Selection, SidechainView, StepCellView, StepInspectorView, TrackRowView, TransformView,
        TransportView,
    };
    use meldritch_audio::realtime_queue::QueueDiagnostics;
    use meldritch_audio::realtime_status::RealtimeStatusSnapshot;
    use meldritch_audio::transport::TransportState;
    use meldritch_core::{
        AutomationInterpolation, AutomationTarget, AutomationValue, PatternId, SceneId, StepIndex,
        TrackId,
    };
    use meldritch_render::Fingerprint;
    use meldritch_render::coordinator::RenderCoordinatorDiagnostics;
    use meldritch_render::dynamics::DuckBands;
    use meldritch_render::effects::{ActiveSendExplanation, EffectBus};
    use meldritch_render::futures::{
        FutureCandidateStatus, FuturePerformanceState, FutureWorkerDiagnostics,
        PerformanceFuturePlan, PerformanceGesture, PerformanceLaunch, PerformanceLaunchSource,
        PlannedFuture, QueuedPerformanceGesture, resolve_renderable_futures,
    };
    use meldritch_render::transforms::{
        ChunkTransform, TransformArtifactKey, TransformCacheStatus,
    };
    use ratatui::backend::TestBackend;

    fn view() -> AppViewModel {
        let selection = Selection {
            track: TrackId::new(1),
            step: StepIndex::new(0),
        };
        let playback = RealtimeStatusSnapshot {
            state: TransportState::Stopped,
            position: 0,
            callbacks: 2,
            stream_errors: 0,
            underruns: 1,
            missed_artifacts: 1,
        };
        AppViewModel {
            transport: TransportView {
                state: playback.state,
                position: playback.position,
                callbacks: playback.callbacks,
                underruns: playback.underruns,
                missed_artifacts: playback.missed_artifacts,
            },
            arrangement: None,
            automation: None,
            effect_sends: None,
            sidechain: None,
            transform: None,
            futures: None,
            performance: PerformanceView {
                queued: None,
                active_scene: None,
                muted_tracks: Vec::new(),
                active_fill: None,
                fill_end_frame: None,
                diagnostics: meldritch_render::futures::PerformanceLauncherDiagnostics::default(),
            },
            pattern_grid: PatternGridView {
                pattern: PatternId::new(1),
                length_steps: 2,
                tracks: vec![TrackRowView {
                    track: TrackId::new(1),
                    selected: true,
                    steps: vec![
                        StepCellView {
                            step: StepIndex::new(0),
                            selected: true,
                            value: Some(Step::new(36)),
                        },
                        StepCellView {
                            step: StepIndex::new(1),
                            selected: false,
                            value: None,
                        },
                    ],
                }],
            },
            inspector: StepInspectorView {
                selection,
                value: Some(Step::new(36)),
            },
            diagnostics: AppDiagnostics {
                selection,
                history_len: 0,
                playback,
                transport_commands: QueueDiagnostics {
                    applied: 1,
                    dropped: 0,
                },
                render: RenderCoordinatorDiagnostics::default(),
            },
            history: Vec::new(),
            bass_voice: None,
        }
    }

    #[test]
    fn key_mapping_produces_semantic_actions() {
        let default_step = Step::new(36);
        assert_eq!(
            map_key(
                KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE),
                &default_step
            ),
            Some(TuiAction::Input(AppInput::ToggleSelected(default_step)))
        );
        assert_eq!(
            map_key(
                KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE),
                &Step::new(36)
            ),
            Some(TuiAction::Quit)
        );
        for (key, input) in [
            ('+', AppInput::IncreaseVelocity),
            ('-', AppInput::DecreaseVelocity),
            (']', AppInput::IncreaseGate),
            ('[', AppInput::DecreaseGate),
            ('>', AppInput::IncreaseProbability),
            ('<', AppInput::DecreaseProbability),
            ('a', AppInput::IncreaseCutoff),
            ('z', AppInput::DecreaseCutoff),
            ('d', AppInput::IncreaseResonance),
            ('x', AppInput::DecreaseResonance),
            ('w', AppInput::CycleWaveform),
            ('g', AppInput::IncreaseFilterEnvelope),
            ('b', AppInput::DecreaseFilterEnvelope),
            ('t', AppInput::IncreaseDrive),
            ('y', AppInput::DecreaseDrive),
            ('v', AppInput::IncreaseSynthLevel),
            ('c', AppInput::DecreaseSynthLevel),
            ('2', AppInput::IncreaseAttack),
            ('1', AppInput::DecreaseAttack),
            ('4', AppInput::IncreaseDecay),
            ('3', AppInput::DecreaseDecay),
            ('6', AppInput::IncreaseSustain),
            ('5', AppInput::DecreaseSustain),
            ('8', AppInput::IncreaseRelease),
            ('7', AppInput::DecreaseRelease),
            ('m', AppInput::IncreaseSubLevel),
            ('n', AppInput::DecreaseSubLevel),
            ('o', AppInput::IncreaseGlide),
            ('i', AppInput::DecreaseGlide),
            ('0', AppInput::IncreaseDucking),
            ('9', AppInput::DecreaseDucking),
            ('M', AppInput::IncreaseDuckingRelease),
            ('N', AppInput::DecreaseDuckingRelease),
            ('G', AppInput::IncreaseHatFilter),
            ('B', AppInput::DecreaseHatFilter),
            ('T', AppInput::IncreaseHatFilterRelease),
            ('Y', AppInput::DecreaseHatFilterRelease),
            ('\'', AppInput::IncreaseNote),
            (';', AppInput::DecreaseNote),
            ('L', AppInput::TransposeChordUp),
            ('H', AppInput::TransposeChordDown),
            ('I', AppInput::InvertChordUp),
            ('U', AppInput::InvertChordDown),
            ('R', AppInput::CreateReverse),
            ('S', AppInput::CreateReslice),
            ('F', AppInput::CreateFreeze),
            ('E', AppInput::CreateSmear),
            ('A', AppInput::AuditionTransform),
            ('D', AppInput::ReturnToLive),
            ('Q', AppInput::QueueNextScene),
            ('Z', AppInput::ToggleTrackMute),
            ('P', AppInput::TriggerFill),
            ('C', AppInput::CancelPerformance),
        ] {
            assert_eq!(
                map_key(
                    KeyEvent::new(KeyCode::Char(key), KeyModifiers::NONE),
                    &Step::new(36)
                ),
                Some(TuiAction::Input(input))
            );
        }
    }

    #[test]
    fn test_backend_renders_all_primary_panels() {
        let backend = TestBackend::new(100, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| draw(frame, &view())).unwrap();

        let content = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        for title in [
            "Transport",
            "Pattern 1",
            "Inspector",
            "Diagnostics",
            "History",
            "Status",
            "Keys",
        ] {
            assert!(content.contains(title), "missing panel {title}");
        }
        for label in [
            "toggle step",
            "velocity",
            "gate",
            "probability",
            "play/pause",
            "rewind",
            "quit",
        ] {
            assert!(content.contains(label), "missing key legend label {label}");
        }
        assert!(content.contains("Ready"));
    }

    #[test]
    fn future_panel_shows_cache_health_and_ranked_candidate() {
        let pattern = PatternId::new(1);
        let renderable = resolve_renderable_futures(
            &PerformanceFuturePlan {
                candidates: vec![PlannedFuture {
                    gesture: PerformanceGesture::MuteTrack(TrackId::new(2)),
                    score: 3_000,
                }],
            },
            &FuturePerformanceState {
                active_pattern: pattern,
                scene_patterns: std::collections::BTreeMap::new(),
                muted_tracks: std::collections::BTreeSet::new(),
            },
            meldritch_core::FrameRange::new(0, 48_000).unwrap(),
            48_000,
            &[(pattern, Fingerprint::new(12))].into_iter().collect(),
        );
        let candidate = &renderable.candidates[0];
        let mut view = view();
        view.futures = Some(FutureCacheView {
            diagnostics: FutureWorkerDiagnostics {
                desired_artifacts: 1,
                clean_artifacts: 1,
                queued_jobs: 0,
                active_jobs: 0,
                completed_jobs: 1,
                sleeping: true,
            },
            candidates: vec![meldritch_render::futures::FutureCandidateInspection {
                gesture: candidate.gesture,
                score: candidate.score,
                key: candidate.key,
                status: FutureCandidateStatus::Clean,
            }],
            unresolved: 0,
        });
        let backend = TestBackend::new(120, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| draw(frame, &view)).unwrap();
        let content = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        assert!(content.contains("Performance Futures"));
        assert!(content.contains("sleeping  clean 1/1"));
        assert!(content.contains("MuteTrack(TrackId(2)) score:3000 Clean"));
    }

    #[test]
    fn live_performance_panel_shows_queue_active_state_and_fallback_counts() {
        let gesture = PerformanceGesture::TriggerFill(PatternId::new(9));
        let mut view = view();
        view.performance.queued = Some(QueuedPerformanceGesture {
            gesture,
            launch_frame: 96_000,
            fill_end_frame: Some(192_000),
        });
        view.performance.active_scene = Some(SceneId::new(2));
        view.performance.muted_tracks = vec![TrackId::new(3)];
        view.performance.diagnostics.queued_gestures = 4;
        view.performance.diagnostics.fallback_launches = 1;
        view.performance.diagnostics.last_launch = Some(PerformanceLaunch {
            gesture: PerformanceGesture::MuteTrack(TrackId::new(3)),
            frame: 48_000,
            source: PerformanceLaunchSource::LiveFallback,
        });
        let backend = TestBackend::new(140, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| draw(frame, &view)).unwrap();
        let content = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        assert!(content.contains("Live Performance"));
        assert!(content.contains("TriggerFill(PatternId(9))@96000"));
        assert!(content.contains("scene:Some(SceneId(2))"));
        assert!(content.contains("fallback:1"));
        assert!(content.contains("last:Some(LiveFallback)"));
    }

    #[test]
    fn arrangement_strip_highlights_active_and_looped_sections() {
        let mut view = view();
        view.arrangement = Some(ArrangementView {
            sections: (0..4)
                .map(|index| ArrangementSectionView {
                    index,
                    scene: SceneId::new(index as u64 + 1),
                    repeats: 2,
                    active: index == 2,
                    in_loop: (1..3).contains(&index),
                })
                .collect(),
            active_repeat: Some(1),
            loop_sections: (1, 3),
        });
        let backend = TestBackend::new(100, 27);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| draw(frame, &view)).unwrap();
        let content = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();

        assert!(content.contains("Arrangement loop 1..3"));
        assert!(content.contains("3:S3×2 R2"));
    }

    #[test]
    fn automation_inspector_shows_current_scene_and_upcoming_point() {
        let mut view = view();
        view.automation = Some(AutomationView {
            lanes: vec![AutomationLaneView {
                target: AutomationTarget::Scene,
                interpolation: AutomationInterpolation::Step,
                current: AutomationValue::Discrete(2),
                next_point: Some((96_000, AutomationValue::Discrete(3))),
            }],
            scene: Some(2),
        });
        let backend = TestBackend::new(120, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| draw(frame, &view)).unwrap();
        let content = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();

        assert!(content.contains("Automation · scene 2"));
        assert!(content.contains("Scene=2"));
        assert!(content.contains("3@96000"));
    }

    #[test]
    fn effect_panel_explains_why_a_send_fired() {
        let mut view = view();
        view.effect_sends = Some(EffectSendView {
            recent: vec![ActiveSendExplanation {
                pattern: PatternId::new(1),
                track: TrackId::new(2),
                step: StepIndex::new(4),
                frame: 24_000,
                bus: EffectBus::Delay,
                matched_tag: meldritch_core::EventTag::Accent,
                send_gain: 0.5,
            }],
        });
        let backend = TestBackend::new(120, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| draw(frame, &view)).unwrap();
        let content = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();

        assert!(content.contains("Effect Sends · why"));
        assert!(content.contains("Delay tag Accent gain 0.50"));
    }

    #[test]
    fn sidechain_panel_shows_roles_bands_and_attenuation() {
        let mut view = view();
        view.sidechain = Some(SidechainView {
            source_role: meldritch_core::SourceRole::Kick,
            target_role: meldritch_core::SourceRole::Bass,
            bands: DuckBands::LOW_ONLY,
            attenuation: 0.42,
        });
        let backend = TestBackend::new(120, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| draw(frame, &view)).unwrap();
        let content = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        assert!(content.contains("Sidechain · attenuation"));
        assert!(content.contains("Kick → Bass"));
        assert!(content.contains("bands low"));
        assert!(content.contains("42.0%"));
    }

    #[test]
    fn transform_panel_shows_provenance_and_fingerprint() {
        let mut view = view();
        view.transform = Some(TransformView {
            transform: ChunkTransform::Reverse,
            key: TransformArtifactKey {
                fingerprint: meldritch_render::Fingerprint::new(42),
                channels: 2,
                frames: 96_000,
            },
            status: TransformCacheStatus::Miss,
            channels: 2,
            frames: 96_000,
            auditioning: false,
        });
        let backend = TestBackend::new(120, 32);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| draw(frame, &view)).unwrap();
        let content = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        assert!(content.contains("Derived Transform · provenance"));
        assert!(content.contains("Reverse"));
        assert!(content.contains("fingerprint 42"));
    }

    #[test]
    fn viewport_keeps_selection_visible_and_clamps_to_bounds() {
        assert_eq!(viewport_offset(0, 4, 16), 0);
        assert_eq!(viewport_offset(8, 4, 16), 6);
        assert_eq!(viewport_offset(15, 4, 16), 12);
        assert_eq!(viewport_offset(3, 8, 4), 0);
    }
}
