//! Ratatui/Crossterm frontend for the headless application controller.

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
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
use std::collections::BTreeSet;
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
        KeyCode::Tab if key.modifiers.contains(KeyModifiers::CONTROL) => {
            TuiAction::Input(AppInput::ToggleCockpitMode)
        }
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
        KeyCode::F(number @ 1..=4) if key.modifiers.contains(KeyModifiers::SHIFT) => {
            TuiAction::Input(AppInput::QueuePhraseVariation(
                meldritch_core::SceneId::new(u64::from(number)),
                1,
            ))
        }
        KeyCode::F(number @ 1..=4) => TuiAction::Input(AppInput::QueuePhrase(
            meldritch_core::SceneId::new(u64::from(number)),
        )),
        KeyCode::Char('}') => TuiAction::Input(AppInput::IncreaseDelayFeedback),
        KeyCode::Char('{') => TuiAction::Input(AppInput::DecreaseDelayFeedback),
        KeyCode::Char('f') => TuiAction::Input(AppInput::IncreasePhaserMix),
        KeyCode::Char('e') => TuiAction::Input(AppInput::DecreasePhaserMix),
        KeyCode::Char('V') => TuiAction::Input(AppInput::ToggleReverbFreeze),
        KeyCode::Char('O') => TuiAction::Input(AppInput::IncreaseModulationDepth),
        KeyCode::Char('K') => TuiAction::Input(AppInput::DecreaseModulationDepth),
        KeyCode::Char('W') => TuiAction::Input(AppInput::IncreaseMasterDrive),
        KeyCode::Char('X') => TuiAction::Input(AppInput::DecreaseMasterDrive),
        _ => return None,
    };
    Some(action)
}

pub fn map_key_for_view(
    key: KeyEvent,
    default_step: &Step,
    view: &AppViewModel,
) -> Option<TuiAction> {
    if key.kind != KeyEventKind::Press {
        return None;
    }
    if view.cockpit_mode == meldritch_app::CockpitMode::Performance {
        if let KeyCode::Char(character) = key.code {
            let normalized = character.to_ascii_lowercase().to_string();
            if let Some(control) = view
                .curated_controls
                .iter()
                .find(|control| control.binding.to_ascii_lowercase() == normalized)
            {
                return Some(TuiAction::Input(AppInput::AdjustCuratedControl {
                    id: control.id.clone(),
                    steps: if key.modifiers.contains(KeyModifiers::SHIFT) {
                        -1
                    } else {
                        1
                    },
                }));
            }
        }
    }
    map_key(key, default_step)
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
    run_with_hooks(controller, default_step, tick, |_, _, _| {})
}

pub fn run_with_hooks<F, I>(
    controller: &mut AppController,
    default_step: Step,
    tick: F,
    on_input: I,
) -> io::Result<()>
where
    F: FnMut(&mut AppController) -> Result<Option<String>, String>,
    I: FnMut(&AppController, &AppInput, &AppCommandResult),
{
    run_with_hooks_and_external_inputs(controller, default_step, tick, || None, on_input)
}

#[derive(Clone, Debug, PartialEq)]
pub struct ExternalInputEvent {
    pub input: Option<AppInput>,
    pub label: Option<String>,
}

impl ExternalInputEvent {
    #[must_use]
    pub fn new(input: AppInput, label: Option<String>) -> Self {
        Self {
            input: Some(input),
            label,
        }
    }

    #[must_use]
    pub fn status(label: String) -> Self {
        Self {
            input: None,
            label: Some(label),
        }
    }
}

pub fn run_with_hooks_and_external_inputs<F, E, I>(
    controller: &mut AppController,
    default_step: Step,
    tick: F,
    external_input: E,
    on_input: I,
) -> io::Result<()>
where
    F: FnMut(&mut AppController) -> Result<Option<String>, String>,
    E: FnMut() -> Option<ExternalInputEvent>,
    I: FnMut(&AppController, &AppInput, &AppCommandResult),
{
    let mut terminal = TerminalGuard::enter()?;
    let result = run_loop(
        &mut terminal.terminal,
        controller,
        &default_step,
        tick,
        external_input,
        on_input,
    );
    terminal.restore()?;
    result
}

fn run_loop<F, E, I>(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    controller: &mut AppController,
    default_step: &Step,
    mut tick: F,
    mut external_input: E,
    mut on_input: I,
) -> io::Result<()>
where
    F: FnMut(&mut AppController) -> Result<Option<String>, String>,
    E: FnMut() -> Option<ExternalInputEvent>,
    I: FnMut(&AppController, &AppInput, &AppCommandResult),
{
    let mut status = StatusMessage::info("Ready");
    loop {
        match tick(controller) {
            Ok(Some(message)) => status = StatusMessage::info(message),
            Err(message) => status = StatusMessage::error(message),
            Ok(None) => {}
        }
        for _ in 0..16 {
            let Some(event) = external_input() else {
                break;
            };
            status = handle_external_input(controller, event, &mut on_input);
        }
        let view = controller.view_model();
        terminal.draw(|frame| draw_with_status(frame, &view, &status))?;
        if !event::poll(Duration::from_millis(50))? {
            continue;
        }
        let Event::Key(key) = event::read()? else {
            continue;
        };
        match map_key_for_view(key, default_step, &view) {
            Some(TuiAction::Quit) => return Ok(()),
            Some(TuiAction::Input(input)) => {
                status = handle_app_input(controller, input, &mut on_input);
            }
            None => {}
        }
    }
}

fn handle_external_input<I>(
    controller: &mut AppController,
    event: ExternalInputEvent,
    on_input: &mut I,
) -> StatusMessage
where
    I: FnMut(&AppController, &AppInput, &AppCommandResult),
{
    let Some(input) = event.input else {
        return StatusMessage::info(event.label.map_or_else(
            || "External input".to_owned(),
            |label| format!("MIDI: {label}"),
        ));
    };
    let mut status = handle_app_input(controller, input, on_input);
    if let Some(label) = event.label {
        status.text = format!("MIDI: {label} · {}", status.text);
    }
    status
}

fn handle_app_input<I>(
    controller: &mut AppController,
    input: AppInput,
    on_input: &mut I,
) -> StatusMessage
where
    I: FnMut(&AppController, &AppInput, &AppCommandResult),
{
    match controller.handle_input(input.clone()) {
        Ok(result) => {
            on_input(controller, &input, &result);
            StatusMessage::info(command_result_text(&result))
        }
        Err(error) => StatusMessage::error(format!("{error:?}")),
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
    if view.cockpit_mode == meldritch_app::CockpitMode::Performance {
        draw_curated_performance_mode(frame, view, status);
        return;
    }
    let performance_visible = view.performance.queued.is_some()
        || view.performance.active_scene.is_some()
        || !view.performance.muted_tracks.is_empty()
        || view.performance.active_fill.is_some()
        || !view.performance.learned_phrase_cues.is_empty()
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
            Constraint::Length(if performance_visible { 5 } else { 0 }),
            Constraint::Min(5),
            Constraint::Length(5),
            Constraint::Length(3),
            Constraint::Length(8),
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

fn draw_curated_performance_mode(
    frame: &mut ratatui::Frame<'_>,
    view: &AppViewModel,
    status: &StatusMessage,
) {
    let groovebox_height = if view.performance.pages.is_empty() {
        4
    } else {
        10
    };
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(groovebox_height),
            Constraint::Min(6),
            Constraint::Length(5),
            Constraint::Length(3),
            Constraint::Length(3),
        ])
        .split(frame.area());
    draw_transport(frame, rows[0], view);
    draw_groovebox_surface(frame, rows[1], view);
    draw_grid(frame, rows[2], view);
    frame.render_widget(
        panel(
            "Control Telemetry · last MIDI/action in status",
            Paragraph::new(performance_control_lines(view)).wrap(Wrap { trim: false }),
        ),
        rows[3],
    );
    draw_status(frame, rows[4], status);
    frame.render_widget(
        panel(
            "Performance Keys",
            Paragraph::new("Ctrl-Tab all parameters · p play/stop · r rewind · q quit"),
        ),
        rows[5],
    );
}

fn draw_groovebox_surface(frame: &mut ratatui::Frame<'_>, area: Rect, view: &AppViewModel) {
    if !view.performance.pages.is_empty() {
        draw_performance_pages(frame, area, view);
        return;
    }
    let active = view
        .performance
        .active_scene
        .map_or_else(|| "—".to_owned(), |scene| scene.raw().to_string());
    let queued = view.performance.queued.map_or_else(
        || "—".to_owned(),
        |queued| format!("{:?}@{}", queued.gesture, queued.launch_frame),
    );
    let lines = vec![
        Line::from("B01-B04 scenes 1-4 · B05-B08 fills/variations 1-4"),
        Line::from(format!(
            "State: active scene {active} · queued {queued} · Pattern {} · {} steps",
            view.pattern_grid.pattern.raw(),
            view.pattern_grid.length_steps
        )),
    ];
    frame.render_widget(
        panel(
            "Groovebox Scenes",
            Paragraph::new(lines).wrap(Wrap { trim: false }),
        ),
        area,
    );
}

fn draw_performance_pages(frame: &mut ratatui::Frame<'_>, area: Rect, view: &AppViewModel) {
    let active_page_index = view.performance.active_page.unwrap_or(0);
    let page = view
        .performance
        .pages
        .get(active_page_index)
        .or_else(|| view.performance.pages.first())
        .expect("performance pages are not empty");
    let page_tabs = view
        .performance
        .pages
        .iter()
        .enumerate()
        .map(|(index, page)| {
            if index == active_page_index {
                format!("[{}]", page.label)
            } else {
                page.label.clone()
            }
        })
        .collect::<Vec<_>>()
        .join(" · ");
    let mut lines = vec![
        Line::from(format!("Pages: {page_tabs}")),
        Line::from(format!(
            "Active page: {} · {} visible strips · Pattern {} · {} steps",
            page.label,
            page.strips.len(),
            view.pattern_grid.pattern.raw(),
            view.pattern_grid.length_steps
        )),
    ];
    lines.extend(page.strips.iter().map(|strip| {
        let track = strip
            .track_id
            .as_ref()
            .map_or_else(|| "—".to_owned(), String::clone);
        let quantization = strip
            .launch_quantization
            .as_ref()
            .map_or_else(|| "free".to_owned(), String::clone);
        let active_variation = strip
            .active_variation_id
            .as_ref()
            .map_or_else(|| "—".to_owned(), String::clone);
        let active_bank = strip
            .active_pattern_bank_id
            .as_ref()
            .map_or_else(|| "—".to_owned(), String::clone);
        let status = match (strip.muted, strip.soloed) {
            (true, true) => "muted+solo",
            (true, false) => "muted",
            (false, true) => "solo",
            (false, false) => "live",
        };
        let banks = if strip.pattern_banks.is_empty() {
            "banks —".to_owned()
        } else {
            format!(
                "banks {}",
                strip
                    .pattern_banks
                    .iter()
                    .map(|bank| format!("{}:{}", bank.label, bank.variation_ids.len()))
                    .collect::<Vec<_>>()
                    .join("/")
            )
        };
        Line::from(format!(
            "F{:02}: {} ({}) · track {} · {status} · bank {active_bank} · var {active_variation} · q {quantization} · {} variations · {banks}",
            strip.strip,
            strip.lane_label,
            strip.lane_role,
            track,
            strip.variation_ids.len()
        ))
    }));
    frame.render_widget(
        panel(
            "Groovebox Pages",
            Paragraph::new(lines).wrap(Wrap { trim: false }),
        ),
        area,
    );
}

fn performance_control_lines(view: &AppViewModel) -> Vec<Line<'static>> {
    if view.curated_controls.is_empty() {
        return vec![Line::from("No controls exposed by this performance")];
    }
    if let Some(lines) = active_page_control_lines(view) {
        return lines;
    }
    if has_launch_control_strip_surface(view) {
        return launch_control_strip_lines(view);
    }
    view.curated_controls
        .iter()
        .map(|control| {
            let value = control
                .value
                .map_or_else(|| "—".to_owned(), |value| format!("{value:.3}"));
            Line::from(format!(
                "[{}] {}  {}  range {:.3}..{:.3} step {:.3}  → {}",
                control.binding,
                control.label,
                value,
                control.minimum,
                control.maximum,
                control.step,
                control.target
            ))
        })
        .collect()
}

fn active_page_control_lines(view: &AppViewModel) -> Option<Vec<Line<'static>>> {
    let page = active_performance_page(view)?;
    if page.strips.iter().all(|strip| strip.control_ids.is_empty()) {
        return None;
    }
    let mut lines = vec![Line::from(format!(
        "Active page {} · declared controls only",
        page.label
    ))];
    lines.extend(page.strips.iter().map(|strip| {
        let controls = if strip.control_ids.is_empty() {
            "—".to_owned()
        } else {
            strip
                .control_ids
                .iter()
                .map(|id| compact_control_value(view, id))
                .collect::<Vec<_>>()
                .join(" · ")
        };
        Line::from(format!(
            "F{:02} {}: {}",
            strip.strip, strip.lane_label, controls
        ))
    }));
    Some(lines)
}

fn active_performance_page(view: &AppViewModel) -> Option<&meldritch_app::PerformancePageView> {
    let index = view.performance.active_page.unwrap_or(0);
    view.performance
        .pages
        .get(index)
        .or_else(|| view.performance.pages.first())
}

fn has_launch_control_strip_surface(view: &AppViewModel) -> bool {
    (1..=8).all(|strip| {
        view.curated_controls
            .iter()
            .any(|control| control.id == format!("fader-{strip:02}"))
            && [strip, strip + 8, strip + 16].iter().all(|knob| {
                view.curated_controls
                    .iter()
                    .any(|control| control.id == format!("knob-{knob:02}"))
            })
    })
}

fn launch_control_strip_lines(view: &AppViewModel) -> Vec<Line<'static>> {
    vec![
        Line::from(format!(
            "Surface: {} · values here are telemetry, not the main performance UI",
            control_source_summary(&view.curated_controls)
        )),
        Line::from(format!(
            "Strip 01: resonance K01 {} · feedback K09 {} · mix K17 {} · cutoff F01 {}",
            compact_control_value(view, "knob-01"),
            compact_control_value(view, "knob-09"),
            compact_control_value(view, "knob-17"),
            compact_control_value(view, "fader-01"),
        )),
        Line::from(format!(
            "Top resonance: {}",
            compact_control_values(view, "K", (1..=8).map(|knob| format!("knob-{knob:02}")))
        )),
        Line::from(format!(
            "Mid feedback: {}",
            compact_control_values(view, "K", (9..=16).map(|knob| format!("knob-{knob:02}")))
        )),
        Line::from(format!(
            "Bot mix: {}",
            compact_control_values(view, "K", (17..=24).map(|knob| format!("knob-{knob:02}")))
        )),
        Line::from(format!(
            "Faders cutoff: {}",
            compact_control_values(view, "F", (1..=8).map(|strip| format!("fader-{strip:02}")))
        )),
    ]
}

fn compact_control_value(view: &AppViewModel, id: &str) -> String {
    view.curated_controls
        .iter()
        .find(|control| control.id == id)
        .map_or_else(
            || "—".to_owned(),
            |control| {
                let value = control
                    .value
                    .map_or_else(|| "—".to_owned(), |value| format!("{value:.3}"));
                let target = control
                    .target
                    .rsplit('/')
                    .next()
                    .unwrap_or(control.target.as_str());
                format!("{value} {target}")
            },
        )
}

fn compact_control_values<I>(view: &AppViewModel, prefix: &str, ids: I) -> String
where
    I: IntoIterator<Item = String>,
{
    ids.into_iter()
        .map(|id| {
            let number = id.rsplit('-').next().unwrap_or("??");
            format!("{prefix}{number} {}", compact_control_value(view, &id))
        })
        .collect::<Vec<_>>()
        .join(" · ")
}

fn control_source_summary(controls: &[meldritch_app::CuratedControlView]) -> String {
    let sources = controls
        .iter()
        .filter_map(|control| control.target.split('/').next())
        .collect::<BTreeSet<_>>();
    if sources.is_empty() {
        "curated controls".to_owned()
    } else {
        sources.into_iter().collect::<Vec<_>>().join(" + ")
    }
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
        |queued| {
            format!(
                "{}@{}",
                performance_gesture_text(queued.gesture),
                queued.launch_frame
            )
        },
    );
    let active = format!(
        "phrase:{} muted:{:?} fill:{:?}→{:?}",
        performance
            .active_scene
            .map_or_else(|| "none".to_owned(), |scene| scene.raw().to_string()),
        performance.muted_tracks,
        performance.active_fill,
        performance.fill_end_frame,
    );
    let diagnostics = performance.diagnostics;
    let learned = if performance.learned_phrase_cues.is_empty() {
        "learned none".to_owned()
    } else {
        format!(
            "learned {}",
            performance
                .learned_phrase_cues
                .iter()
                .map(|cue| format!("phrase {}@{}", cue.scene.raw(), cue.frame))
                .collect::<Vec<_>>()
                .join("  ")
        )
    };
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
                Line::from(learned),
                Line::from(launches),
            ]),
        ),
        area,
    );
}

fn performance_gesture_text(gesture: meldritch_app::PerformanceGesture) -> String {
    match gesture {
        meldritch_app::PerformanceGesture::QueueScene(scene) => {
            format!("phrase {}", scene.raw())
        }
        other => format!("{other:?}"),
    }
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
        ("F1-F4", "phrase pads"),
        ("Shift+F1-F4", "variations"),
        ("{/}", "delay fb"),
        ("e/f", "phaser"),
        ("V", "reverb freeze"),
        ("K/O", "mod depth"),
        ("X/W", "master drive"),
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
    let mut lines = vec![
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
    ];
    for (index, (key, action)) in keys.into_iter().enumerate() {
        let line = if index < 6 {
            0
        } else if index < 12 {
            1
        } else if index < 18 {
            2
        } else if index < 24 {
            3
        } else if index < 30 {
            4
        } else {
            5
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
        AppCommandResult::PerformanceFxUpdated(settings) => format!(
            "FX delay:{:.2} phaser:{:.2} freeze:{} mod:{:.2} drive:{:.2}",
            settings.delay_feedback,
            settings.phaser_mix,
            settings.reverb_freeze,
            settings.modulation_depth,
            settings.master_drive,
        ),
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
        AppCommandResult::PerformancePageSelected { previous, current } => {
            format!("Performance page: {previous:?} → {current}")
        }
        AppCommandResult::LaneVariationSelected {
            lane_id,
            previous,
            current,
        } => format!("Lane {lane_id} variation: {previous:?} → {current}"),
        AppCommandResult::LanePatternBankSelected {
            lane_id,
            previous_bank,
            current_bank,
            previous_variation,
            current_variation,
        } => format!(
            "Lane {lane_id} bank: {previous_bank:?} → {current_bank}; variation {previous_variation:?} → {current_variation}"
        ),
        AppCommandResult::LaneMuteToggled { lane_id, muted } => {
            format!("Lane {lane_id} muted: {muted}")
        }
        AppCommandResult::LaneSoloToggled { lane_id, soloed } => {
            format!("Lane {lane_id} soloed: {soloed}")
        }
        AppCommandResult::PerformanceCancelled(Some(queued)) => {
            format!("Cancelled {:?}", queued.gesture)
        }
        AppCommandResult::PerformanceCancelled(None) => "No performance gesture queued".to_owned(),
        AppCommandResult::CockpitModeChanged { previous, current } => {
            format!("Cockpit mode: {previous:?} → {current:?}")
        }
        AppCommandResult::CuratedControlAdjusted { id, current, .. } => {
            format!("Performance control {id}: {current:.3}")
        }
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
            cockpit_mode: meldritch_app::CockpitMode::AllParameters,
            curated_controls: Vec::new(),
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
                learned_phrase_cues: Vec::new(),
                pages: Vec::new(),
                active_page: None,
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
            performance_fx: None,
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
        assert_eq!(
            map_key(
                KeyEvent::new(KeyCode::Tab, KeyModifiers::CONTROL),
                &Step::new(36)
            ),
            Some(TuiAction::Input(AppInput::ToggleCockpitMode))
        );
        assert_eq!(
            map_key(
                KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE),
                &Step::new(36)
            ),
            None
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
            ('}', AppInput::IncreaseDelayFeedback),
            ('{', AppInput::DecreaseDelayFeedback),
            ('f', AppInput::IncreasePhaserMix),
            ('e', AppInput::DecreasePhaserMix),
            ('V', AppInput::ToggleReverbFreeze),
            ('O', AppInput::IncreaseModulationDepth),
            ('K', AppInput::DecreaseModulationDepth),
            ('W', AppInput::IncreaseMasterDrive),
            ('X', AppInput::DecreaseMasterDrive),
        ] {
            assert_eq!(
                map_key(
                    KeyEvent::new(KeyCode::Char(key), KeyModifiers::NONE),
                    &Step::new(36)
                ),
                Some(TuiAction::Input(input))
            );
        }
        for number in 1..=4 {
            assert_eq!(
                map_key(
                    KeyEvent::new(KeyCode::F(number), KeyModifiers::NONE),
                    &Step::new(36)
                ),
                Some(TuiAction::Input(AppInput::QueuePhrase(SceneId::new(
                    u64::from(number)
                ))))
            );
            assert_eq!(
                map_key(
                    KeyEvent::new(KeyCode::F(number), KeyModifiers::SHIFT),
                    &Step::new(36)
                ),
                Some(TuiAction::Input(AppInput::QueuePhraseVariation(
                    SceneId::new(u64::from(number)),
                    1
                )))
            );
        }
    }

    #[test]
    fn test_backend_renders_all_primary_panels() {
        let mut view = view();
        view.cockpit_mode = meldritch_app::CockpitMode::AllParameters;
        let backend = TestBackend::new(100, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| draw(frame, &view)).unwrap();

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
    fn performance_mode_renders_only_curated_controls_and_hides_dense_editor() {
        let mut view = view();
        view.cockpit_mode = meldritch_app::CockpitMode::Performance;
        view.curated_controls = vec![meldritch_app::CuratedControlView {
            id: "echo-feedback".to_owned(),
            label: "Echo Feedback".to_owned(),
            target: "dsp:echo/delay.feedback".to_owned(),
            minimum: 0.0,
            maximum: 0.85,
            step: 0.05,
            binding: "f".to_owned(),
            value: Some(0.35),
        }];
        let backend = TestBackend::new(100, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| draw(frame, &view)).unwrap();
        let content = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();

        assert!(content.contains("Control Telemetry"));
        assert!(content.contains("Groovebox Scenes"));
        assert!(content.contains("Echo Feedback"));
        assert!(content.contains("dsp:echo/delay.feedback"));
        assert!(content.contains("Pattern 1"));
        assert!(!content.contains("Inspector"));
        assert!(!content.contains("Diagnostics"));
    }

    #[test]
    fn performance_mode_renders_launch_control_strip_and_scene_surface() {
        let mut view = view();
        view.cockpit_mode = meldritch_app::CockpitMode::Performance;
        view.pattern_grid.length_steps = 16;
        for strip in 1..=8 {
            for knob in [strip, strip + 8, strip + 16] {
                view.curated_controls
                    .push(meldritch_app::CuratedControlView {
                        id: format!("knob-{knob:02}"),
                        label: format!("Knob {knob:02}"),
                        target: if knob % 2 == 0 {
                            "dsp:echo/delay.feedback".to_owned()
                        } else {
                            "synth:playground/filter.cutoff_hz".to_owned()
                        },
                        minimum: 0.0,
                        maximum: 5000.0,
                        step: 1.0,
                        binding: format!("k{knob:02}"),
                        value: Some(if knob % 2 == 0 { 0.35 } else { 4350.0 }),
                    });
            }
            view.curated_controls
                .push(meldritch_app::CuratedControlView {
                    id: format!("fader-{strip:02}"),
                    label: format!("Fader {strip:02} Cutoff"),
                    target: "synth:playground/filter.cutoff_hz".to_owned(),
                    minimum: 100.0,
                    maximum: 5000.0,
                    step: 50.0,
                    binding: format!("f{strip:02}"),
                    value: Some(4350.0),
                });
        }
        let backend = TestBackend::new(140, 32);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| draw(frame, &view)).unwrap();
        let content = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();

        assert!(content.contains("Groovebox Scenes"));
        assert!(content.contains("B01-B04 scenes"));
        assert!(content.contains("B05-B08 fills"));
        assert!(content.contains("Control Telemetry"));
        assert!(content.contains("Surface"));
        assert!(content.contains("synth:playground"));
        assert!(content.contains("dsp:echo"));
        assert!(content.contains("K01"));
        assert!(content.contains("K09"));
        assert!(content.contains("K17"));
        assert!(content.contains("F01"));
        assert!(content.contains("resonance"));
        assert!(content.contains("feedback"));
        assert!(content.contains("mix"));
        assert!(content.contains("cutoff"));
        assert!(content.contains("4350.000 filter.cutoff_hz"));
    }

    #[test]
    fn performance_mode_renders_script_declared_pages_when_present() {
        let mut view = view();
        view.cockpit_mode = meldritch_app::CockpitMode::Performance;
        view.performance.pages = vec![
            meldritch_app::PerformancePageView {
                id: "main".to_owned(),
                label: "Main".to_owned(),
                strips: vec![meldritch_app::PerformanceStripView {
                    strip: 1,
                    lane_id: "pad".to_owned(),
                    lane_label: "Pad".to_owned(),
                    lane_role: "polyphonic_synth".to_owned(),
                    track_id: Some("pad-track".to_owned()),
                    launch_quantization: Some("1 bar".to_owned()),
                    muted: false,
                    soloed: false,
                    active_pattern_bank_id: Some("groove".to_owned()),
                    active_variation_id: Some("pad-a".to_owned()),
                    variation_ids: vec![
                        "pad-a".to_owned(),
                        "pad-b".to_owned(),
                        "pad-c".to_owned(),
                        "pad-d".to_owned(),
                    ],
                    pattern_banks: vec![
                        meldritch_app::PerformancePatternBankView {
                            id: "groove".to_owned(),
                            label: "Groove".to_owned(),
                            variation_ids: vec!["pad-a".to_owned(), "pad-b".to_owned()],
                        },
                        meldritch_app::PerformancePatternBankView {
                            id: "fill".to_owned(),
                            label: "Fills".to_owned(),
                            variation_ids: vec!["pad-c".to_owned(), "pad-d".to_owned()],
                        },
                    ],
                    control_ids: vec!["pad-cutoff".to_owned()],
                }],
            },
            meldritch_app::PerformancePageView {
                id: "drums".to_owned(),
                label: "Drums".to_owned(),
                strips: vec![meldritch_app::PerformanceStripView {
                    strip: 8,
                    lane_id: "kick".to_owned(),
                    lane_label: "Kick".to_owned(),
                    lane_role: "drum".to_owned(),
                    track_id: Some("kick-track".to_owned()),
                    launch_quantization: Some("1 bar".to_owned()),
                    muted: true,
                    soloed: false,
                    active_pattern_bank_id: Some("drums".to_owned()),
                    active_variation_id: Some("kick-a".to_owned()),
                    variation_ids: vec!["kick-a".to_owned()],
                    pattern_banks: vec![meldritch_app::PerformancePatternBankView {
                        id: "drums".to_owned(),
                        label: "Drums".to_owned(),
                        variation_ids: vec!["kick-a".to_owned()],
                    }],
                    control_ids: Vec::new(),
                }],
            },
        ];
        view.curated_controls = vec![meldritch_app::CuratedControlView {
            id: "pad-cutoff".to_owned(),
            label: "Pad Cutoff".to_owned(),
            target: "synth:pad/filter.cutoff_hz".to_owned(),
            minimum: 100.0,
            maximum: 5000.0,
            step: 50.0,
            binding: "f01".to_owned(),
            value: Some(1234.0),
        }];
        view.performance.active_page = Some(0);
        let backend = TestBackend::new(140, 32);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| draw(frame, &view)).unwrap();
        let content = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();

        assert!(content.contains("Groovebox Pages"));
        assert!(content.contains("[Main]"));
        assert!(content.contains("Drums"));
        assert!(content.contains("F01"));
        assert!(content.contains("Pad"));
        assert!(content.contains("polyphonic_synth"));
        assert!(content.contains("4 variations"));
        assert!(content.contains("live"));
        assert!(content.contains("bank groove"));
        assert!(content.contains("var pad-a"));
        assert!(content.contains("q 1 bar"));
        assert!(content.contains("banks Groove:2/Fills:2"));
        assert!(content.contains("Active page Main"));
        assert!(content.contains("1234.000 filter.cutoff_hz"));
        assert!(!content.contains("B01-B04 scenes"));
    }

    #[test]
    fn performance_bindings_override_legacy_keys_and_shift_reverses_direction() {
        let mut view = view();
        view.cockpit_mode = meldritch_app::CockpitMode::Performance;
        view.curated_controls = vec![meldritch_app::CuratedControlView {
            id: "echo-feedback".to_owned(),
            label: "Echo Feedback".to_owned(),
            target: "dsp:echo/delay.feedback".to_owned(),
            minimum: 0.0,
            maximum: 0.85,
            step: 0.05,
            binding: "f".to_owned(),
            value: Some(0.35),
        }];
        assert_eq!(
            map_key_for_view(
                KeyEvent::new(KeyCode::Char('f'), KeyModifiers::NONE),
                &Step::new(36),
                &view,
            ),
            Some(TuiAction::Input(AppInput::AdjustCuratedControl {
                id: "echo-feedback".to_owned(),
                steps: 1,
            }))
        );
        assert_eq!(
            map_key_for_view(
                KeyEvent::new(KeyCode::Char('F'), KeyModifiers::SHIFT),
                &Step::new(36),
                &view,
            ),
            Some(TuiAction::Input(AppInput::AdjustCuratedControl {
                id: "echo-feedback".to_owned(),
                steps: -1,
            }))
        );
        view.cockpit_mode = meldritch_app::CockpitMode::AllParameters;
        assert_eq!(
            map_key_for_view(
                KeyEvent::new(KeyCode::Char('f'), KeyModifiers::NONE),
                &Step::new(36),
                &view,
            ),
            Some(TuiAction::Input(AppInput::IncreasePhaserMix))
        );
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
        let gesture = PerformanceGesture::QueueScene(SceneId::new(3));
        let mut view = view();
        view.performance.queued = Some(QueuedPerformanceGesture {
            gesture,
            launch_frame: 96_000,
            fill_end_frame: None,
        });
        view.performance.active_scene = Some(SceneId::new(2));
        view.performance.learned_phrase_cues = vec![meldritch_app::LearnedPhraseCueView {
            scene: SceneId::new(4),
            frame: 144_000,
        }];
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
        assert!(content.contains("phrase 3@96000"));
        assert!(content.contains("phrase:2"));
        assert!(content.contains("learned phrase 4@144000"));
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
