use std::{
    cell::RefCell,
    io::{BufWriter, Write, stdout},
    time::{Duration, Instant},
};

use crossterm::{
    cursor::{Hide, MoveTo, Show},
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute, queue,
    style::{Attribute, Color, Print, ResetColor, SetAttribute, SetForegroundColor},
    terminal::{
        self, BeginSynchronizedUpdate, Clear, ClearType, EndSynchronizedUpdate,
        EnterAlternateScreen, LeaveAlternateScreen,
    },
};

use crate::{
    device::DeviceOutput,
    midi::{TimedMidiError, TimedMidiRouter},
    midi_device::MidiInputSession,
    navigation::NavigationRuntime,
    panel::{Gesture, Panel, PickupDirection},
    performance_ui::{ColorMode, Frame, LOSPEC500},
    screen_panel::{ScreenPanel, WidgetView},
};

fn terminal_color(index: usize, mode: ColorMode) -> Option<Color> {
    let [r, g, b] = LOSPEC500[index];
    match mode {
        ColorMode::TrueColor => Some(Color::Rgb { r, g, b }),
        ColorMode::Ansi256 => Some(Color::AnsiValue(
            16 + 36 * (r / 51) + 6 * (g / 51) + (b / 51),
        )),
        ColorMode::Mono => None,
    }
}

fn print_palette(
    out: &mut impl Write,
    mode: ColorMode,
    palette: usize,
    bold: bool,
    text: impl std::fmt::Display,
) -> Result<(), String> {
    if let Some(color) = terminal_color(palette, mode) {
        queue!(out, SetForegroundColor(color)).map_err(|error| error.to_string())?;
    }
    queue!(
        out,
        SetAttribute(if bold {
            Attribute::Bold
        } else {
            Attribute::NormalIntensity
        }),
        Print(text),
        ResetColor
    )
    .map_err(|error| error.to_string())
}

#[derive(Clone, Copy)]
enum SemanticLine {
    Controls,
    Grid { row_color: usize },
}

fn print_semantic_line(
    out: &mut impl Write,
    mode: ColorMode,
    text: &str,
    style: SemanticLine,
) -> Result<(), String> {
    let chars = text.chars().collect::<Vec<_>>();
    let classify = |index: usize, character: char| match style {
        SemanticLine::Controls => match character {
            '┌' | '┐' | '└' | '┘' | '├' | '┤' | '─' | '│' => (30, false),
            '█' => (28, true),
            '░' => (25, false),
            '>' => (8, true),
            '◆' => (20, true),
            '↑' => (8, true),
            '↓' => (37, true),
            '1'..='9'
                if chars.get(index + 1) == Some(&' ')
                    && matches!(
                        index.checked_sub(1).and_then(|i| chars.get(i)),
                        Some('>' | ' ')
                    ) =>
            {
                (14, true)
            }
            _ => (33, false),
        },
        SemanticLine::Grid { row_color } => match character {
            '┌' | '┐' | '└' | '┘' | '├' | '┤' | '┬' | '┴' | '┼' | '─' | '│' => {
                (30, false)
            }
            '█' => (41, true),
            '◆' => (row_color, true),
            '◐' => (row_color, false),
            '✦' => (36, true),
            '▏' => (27, true),
            '·' => (25, false),
            _ => (row_color, false),
        },
    };
    let mut start = 0;
    while start < chars.len() {
        let current = classify(start, chars[start]);
        let mut end = start + 1;
        while end < chars.len() && classify(end, chars[end]) == current {
            end += 1;
        }
        print_palette(
            out,
            mode,
            current.0,
            current.1,
            chars[start..end].iter().collect::<String>(),
        )?;
        start = end;
    }
    Ok(())
}

fn fit_cell(mut text: String, width: usize) -> String {
    let count = text.chars().count();
    if count > width {
        text = text
            .chars()
            .take(width.saturating_sub(1))
            .collect::<String>()
            + "…";
    } else {
        text.push_str(&" ".repeat(width - count));
    }
    text
}

fn control_focus_lines(
    panel: &Panel,
    apc: Option<&crate::model::ApcMapping>,
    terminal_width: usize,
    title: &str,
) -> Vec<String> {
    let mut inner = terminal_width.saturating_sub(4).clamp(36, 98);
    let columns = usize::from(inner >= 72) + 1;
    if columns == 2 && inner.is_multiple_of(2) {
        inner -= 1;
    }
    let gap = columns - 1;
    let cell_width = (inner - gap) / columns;
    let mut lines = vec![format!("┌{}┐", "─".repeat(inner))];
    lines.push(format!(
        "│{}│",
        fit_cell(format!(" {title} CONTROLS").to_uppercase(), inner)
    ));
    lines.push(format!("├{}┤", "─".repeat(inner)));
    let selected = panel.selected();
    for start in (0..panel.rows().len()).step_by(columns) {
        let mut cells = Vec::with_capacity(columns);
        for offset in 0..columns {
            let index = start + offset;
            let Some(row) = panel.rows().get(index) else {
                cells.push(" ".repeat(cell_width));
                continue;
            };
            let (min, max) = row.range();
            let normalized = ((row.value() - min) / (max - min)).clamp(0.0, 1.0);
            let label_width = if columns == 2 { 14 } else { 20 };
            let bar_width = cell_width.saturating_sub(label_width + 13).max(4);
            let filled = (normalized * bar_width as f64).round() as usize;
            let pickup = match row.hardware_pickup() {
                -1 => "↑",
                0 => "◆",
                1 => "↓",
                _ if selected == Some(index) => match panel.pickup_direction() {
                    PickupDirection::Below => "↑",
                    PickupDirection::Above => "↓",
                    PickupDirection::Captured => "◆",
                    PickupDirection::Unknown => "○",
                },
                _ => " ",
            };
            let fader = apc.and_then(|mapping| {
                let bank = mapping
                    .fader_bank
                    .load(std::sync::atomic::Ordering::Acquire)
                    .min(mapping.fader_banks.len().saturating_sub(1));
                mapping.fader_banks[bank]
                    .iter()
                    .position(|id| id == row.id())
                    .map(|index| index + 1)
                    .or_else(|| (mapping.master_control == row.id()).then_some(9))
            });
            let label = fit_cell(
                fader.map_or_else(
                    || row.label().to_owned(),
                    |number| format!("{number} {}", row.label()),
                ),
                label_width,
            );
            let marker = if selected == Some(index) { ">" } else { " " };
            cells.push(fit_cell(
                format!(
                    "{marker}{label}[{}{}] {:>7.3} {pickup}",
                    "█".repeat(filled),
                    "░".repeat(bar_width - filled),
                    row.value()
                ),
                cell_width,
            ));
        }
        lines.push(format!("│{}│", cells.join(" ")));
    }
    lines.push(format!("└{}┘", "─".repeat(inner)));
    lines
}

fn draw_frame(out: &mut impl Write, frame: &Frame, mode: ColorMode) -> Result<(), String> {
    for line in &frame.lines {
        for span in line {
            if let Some(color) = span.color.and_then(|index| terminal_color(index, mode)) {
                queue!(out, SetForegroundColor(color)).map_err(|e| e.to_string())?;
            }
            queue!(
                out,
                SetAttribute(if span.bold {
                    Attribute::Bold
                } else {
                    Attribute::NormalIntensity
                }),
                Print(&span.text),
                ResetColor
            )
            .map_err(|e| e.to_string())?;
        }
        queue!(out, Print("\r\n")).map_err(|e| e.to_string())?;
    }
    Ok(())
}

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

thread_local! {
    static PREVIOUS_FRAME: RefCell<Vec<Vec<u8>>> = const { RefCell::new(Vec::new()) };
}

struct Terminal;

impl Terminal {
    fn enter() -> Result<Self, String> {
        PREVIOUS_FRAME.with(|previous| previous.borrow_mut().clear());
        terminal::enable_raw_mode().map_err(|error| error.to_string())?;
        if let Err(error) = execute!(stdout(), EnterAlternateScreen, Hide) {
            let _ = terminal::disable_raw_mode();
            return Err(error.to_string());
        }
        Ok(Self)
    }
}

fn frame_lines(body: &[u8]) -> Vec<Vec<u8>> {
    body.split(|byte| *byte == b'\n')
        .map(|line| line.strip_suffix(b"\r").unwrap_or(line).to_vec())
        .collect()
}

fn present_frame(body: Vec<u8>) -> Result<(), String> {
    let lines = frame_lines(&body);
    let (changed, clear_from) = PREVIOUS_FRAME.with(|previous| {
        let mut previous = previous.borrow_mut();
        let changed = lines
            .iter()
            .enumerate()
            .filter(|(row, line)| previous.get(*row) != Some(*line))
            .map(|(row, line)| (row, line.clone()))
            .collect::<Vec<_>>();
        let clear_from = (previous.len() > lines.len()).then_some(lines.len());
        previous.clone_from(&lines);
        (changed, clear_from)
    });
    if changed.is_empty() && clear_from.is_none() {
        return Ok(());
    }
    let stdout = stdout();
    let mut out = BufWriter::with_capacity(body.len().min(16 * 1024) + 256, stdout.lock());
    queue!(out, BeginSynchronizedUpdate).map_err(|error| error.to_string())?;
    for (row, line) in changed {
        queue!(out, MoveTo(0, row as u16)).map_err(|error| error.to_string())?;
        out.write_all(&line).map_err(|error| error.to_string())?;
        queue!(out, Clear(ClearType::UntilNewLine)).map_err(|error| error.to_string())?;
    }
    if let Some(row) = clear_from {
        queue!(out, MoveTo(0, row as u16), Clear(ClearType::FromCursorDown))
            .map_err(|error| error.to_string())?;
    }
    queue!(out, EndSynchronizedUpdate).map_err(|error| error.to_string())?;
    out.flush().map_err(|error| error.to_string())
}

fn draw_pattern_grid(
    out: &mut impl Write,
    output: &DeviceOutput,
    page: usize,
    selected_track: usize,
    mode: ColorMode,
) -> Result<(), String> {
    let performance = output
        .performance()
        .ok_or("pattern grid requires a sequencer")?;
    let sequence = &performance.sequencer;
    if let Some(roll) = performance
        .apc
        .as_ref()
        .and_then(|mapping| mapping.piano_roll.as_ref())
    {
        return draw_piano_roll_grid(
            out,
            sequence,
            roll,
            usize::from(terminal::size().map_or(80, |(width, _)| width)),
            mode,
        );
    }
    let names = sequence.track_names();
    let patterns = sequence.pattern_names();
    let pattern = sequence.pattern();
    let first = page * 8;
    let active = patterns
        .get(sequence.active_pattern())
        .map(String::as_str)
        .unwrap_or("?");
    let viewed = patterns
        .get(sequence.viewed_pattern())
        .map(String::as_str)
        .unwrap_or("?");
    let queued = sequence
        .queued_pattern()
        .and_then(|index| patterns.get(index))
        .map_or(String::new(), |name| format!(" -> {name}"));
    queue!(
        out,
        Print(format!(
            "  Playing: {active}{queued} | Viewing: {viewed} | {} | {}\r\n  Status: {}\r\n",
            if sequence.playing() {
                "PLAYING"
            } else {
                "STOPPED"
            },
            sequence.bpm(),
            sequence.status()
        ))
    )
    .map_err(|error| error.to_string())?;
    queue!(out, Print("          ")).map_err(|error| error.to_string())?;
    for step in first..(first + 8).min(sequence.steps) {
        queue!(out, Print(format!("{:02} ", step + 1))).map_err(|e| e.to_string())?;
    }
    queue!(out, Print("\r\n")).map_err(|e| e.to_string())?;
    for (track, name) in names.iter().enumerate() {
        queue!(
            out,
            Print(format!(
                "{}{}{}{:6} ",
                if track == selected_track { ">" } else { " " },
                if sequence.muted(track) { "M" } else { " " },
                if sequence.soloed(track) { "S" } else { " " },
                fit_cell(name.to_owned(), 6)
            ))
        )
        .map_err(|e| e.to_string())?;
        for step in first..(first + 8).min(sequence.steps) {
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
    let selected_step = sequence.selected_step();
    let selected_name = names.get(selected_track).map(String::as_str).unwrap_or("?");
    if selected_track < names.len() && selected_step < usize::from(pattern.lengths[selected_track])
    {
        queue!(out, Print(format!(
            "\r\n  {selected_name} step {:02}/{:02} | velocity {:.2} | pitch {:.2} Hz | gate {:.2}\r\n",
            selected_step + 1,
            pattern.lengths[selected_track],
            pattern.velocities[selected_track][selected_step],
            pattern.pitches_hz[selected_track][selected_step],
            pattern.gates[selected_track][selected_step],
        )))
        .map_err(|e| e.to_string())?;
    }
    queue!(
        out,
        Print("\r\n1-8 toggle | [ ] bank | arrows select track/step | n/N viewed pattern | F1-F4 playing pattern\r\nd duplicate | ,/. rotate | +/- velocity | p/P pitch | g/G gate | C clear | S save | R reload\r\n")
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

fn midi_note_name(note: u8) -> String {
    const NAMES: [&str; 12] = [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];
    format!(
        "{}{}",
        NAMES[usize::from(note % 12)],
        i16::from(note / 12) - 1
    )
}

fn midi_note_hz(note: u8) -> f64 {
    440.0 * 2.0_f64.powf((f64::from(note) - 69.0) / 12.0)
}

fn piano_roll_lines(
    sequence: &crate::sequencer::SequencerControl,
    roll: &crate::model::ApcPianoRoll,
    terminal_width: usize,
) -> Vec<String> {
    let pattern = sequence.pattern();
    let zoom = sequence.step_zoom();
    let first = sequence.step_bank() * 8;
    let current = sequence.current_step();
    let max_length = roll
        .drum_tracks
        .iter()
        .copied()
        .chain(std::iter::once(roll.melodic_track))
        .map(|track| usize::from(pattern.lengths[track]))
        .max()
        .unwrap_or(8);
    let remaining_cells = max_length.saturating_sub(first).div_ceil(zoom).max(1);
    let available = terminal_width.saturating_sub(4).max(20);
    let mut cell_count = 8.min(remaining_cells);
    for candidate in (8..=remaining_cells.min(64)).step_by(8) {
        let group_gaps = candidate.saturating_sub(1) / 4;
        if 11 + candidate + group_gaps <= available {
            cell_count = candidate;
        }
    }
    let group_gaps = cell_count.saturating_sub(1) / 4;
    let cell_width = ((available.saturating_sub(11 + group_gaps)) / cell_count).clamp(1, 3);
    let grid_width = cell_count * cell_width + group_gaps;
    let border = format!("┌────────┬{}┐", "─".repeat(grid_width));
    let last = (first + cell_count * zoom).min(max_length);
    let metadata = format!(
        "  {:02}-{:02} · {} · Z{}",
        first + 1,
        last,
        midi_note_name(sequence.key_root()),
        zoom
    );
    let mut top = border;
    if top.chars().count() + metadata.chars().count() <= terminal_width.saturating_sub(2) {
        top.push_str(&metadata);
    }
    let format_cells = |values: Vec<String>| {
        values
            .chunks(4)
            .map(|group| group.concat())
            .collect::<Vec<_>>()
            .join(" ")
    };
    let headings = (0..cell_count)
        .map(|column| {
            let step = first + column * zoom + 1;
            fit_cell(
                if cell_width == 1 {
                    (step % 10).to_string()
                } else {
                    format!("{step:02}")
                },
                cell_width,
            )
        })
        .collect::<Vec<_>>();
    let mut lines = vec![top];
    lines.push(format!(
        "│{}│{}│",
        fit_cell(
            format!("{} z{}", midi_note_name(sequence.key_root()), zoom),
            8
        ),
        format_cells(headings)
    ));
    lines.push(format!("├────────┼{}┤", "─".repeat(grid_width)));

    let cells = |track: usize, pitch: Option<u8>| {
        let length = usize::from(pattern.lengths[track]);
        (0..cell_count)
            .map(|column| {
                let start = first + column * zoom;
                let end = start + zoom;
                let mut overlaps = 0;
                let mut full = false;
                if let Some(note) = pitch {
                    for event_step in 0..length {
                        if pattern.velocities[track][event_step] <= 0.0 {
                            continue;
                        }
                        let event_note = (69.0
                            + 12.0 * (pattern.pitches_hz[track][event_step] / 440.0).log2())
                        .round() as u8;
                        if event_note != note {
                            continue;
                        }
                        let event_end =
                            event_step as f32 + pattern.gates[track][event_step].max(0.01);
                        if event_step < end && event_end > start as f32 {
                            overlaps += 1;
                            full |= event_step <= start && event_end >= end as f32;
                        }
                    }
                } else {
                    for global_step in start..end {
                        if pattern.velocities[track][global_step % length.max(1)] > 0.0 {
                            overlaps += 1;
                        }
                    }
                    full = zoom == 1 && overlaps == 1;
                }
                let under_playhead = sequence.playing() && current >= start && current < end;
                match (overlaps, full, under_playhead) {
                    (0, _, false) => "·",
                    (0, _, true) => "▏",
                    (_, _, true) => "█",
                    (1, true, false) => "◆",
                    (1, false, false) => "◐",
                    (_, _, false) => "✦",
                }
            })
            .map(|glyph| {
                let left = cell_width.saturating_sub(1) / 2;
                format!(
                    "{}{}{}",
                    " ".repeat(left),
                    glyph,
                    " ".repeat(cell_width - left - 1)
                )
            })
            .collect::<Vec<_>>()
    };
    let row = |label: String, values: Vec<String>| {
        format!("│{}│{}│", fit_cell(label, 8), format_cells(values))
    };
    let names = sequence.track_names();
    for &track in &roll.drum_tracks {
        lines.push(row(
            names.get(track).cloned().unwrap_or_else(|| "DRUM".into()),
            cells(track, None),
        ));
    }
    lines.push(format!("├────────┼{}┤", "─".repeat(grid_width)));
    for scale_index in (0..5).rev() {
        let note = sequence.key_root().saturating_add(roll.scale[scale_index]);
        lines.push(row(
            format!("BASS {}", midi_note_name(note)),
            cells(roll.melodic_track, Some(note)),
        ));
    }
    let mut bottom = format!("└────────┴{}┘", "─".repeat(grid_width));
    let legend = "  ◆FULL ◐PART ✦MULT █PLAY";
    if bottom.chars().count() + legend.chars().count() <= terminal_width.saturating_sub(2) {
        bottom.push_str(legend);
    }
    lines.push(bottom);
    lines
}

fn draw_piano_roll_grid(
    out: &mut impl Write,
    sequence: &crate::sequencer::SequencerControl,
    roll: &crate::model::ApcPianoRoll,
    terminal_width: usize,
    mode: ColorMode,
) -> Result<(), String> {
    let drum_colors = [4, 7, 14];
    for (index, line) in piano_roll_lines(sequence, roll, terminal_width)
        .into_iter()
        .enumerate()
    {
        queue!(out, Print("  ")).map_err(|e| e.to_string())?;
        let row_color = match index {
            3..=5 => drum_colors[index - 3],
            7..=11 => {
                let scale_index = 4 - (index - 7);
                let note = sequence.key_root().saturating_add(roll.scale[scale_index]);
                crate::performance_ui::pitch_color(midi_note_hz(note))
            }
            1 => 28,
            _ => 31,
        };
        print_semantic_line(out, mode, &line, SemanticLine::Grid { row_color })?;
        queue!(out, Print("\r\n")).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn navigation_key(code: KeyCode) -> Option<String> {
    match code {
        KeyCode::F(number) => Some(format!("F{number}")),
        KeyCode::Char(character) => Some(character.to_string()),
        _ => None,
    }
}

fn performance_key(code: KeyCode) -> Option<String> {
    match code {
        KeyCode::F(number) => Some(format!("F{number}")),
        KeyCode::Up => Some("Up".into()),
        KeyCode::Down => Some("Down".into()),
        KeyCode::Left => Some("Left".into()),
        KeyCode::Right => Some("Right".into()),
        KeyCode::Tab => Some("Tab".into()),
        KeyCode::BackTab => Some("BackTab".into()),
        KeyCode::Char(' ') => Some("Space".into()),
        KeyCode::Char(c) => Some(c.to_string()),
        _ => None,
    }
}

fn dispatch_performance_binding(
    performance: &crate::model::Performance,
    key_name: &str,
    panel: Option<&mut Panel>,
) -> Result<bool, String> {
    let Some(binding) = performance
        .key_bindings
        .iter()
        .find(|binding| binding.key == key_name)
    else {
        return Ok(false);
    };
    if let Some(gesture) = match binding.command.as_str() {
        "control:previous" => Some(Gesture::Previous),
        "control:next" => Some(Gesture::Next),
        "control:decrease" => Some(Gesture::Decrease),
        "control:increase" => Some(Gesture::Increase),
        _ => None,
    } {
        panel
            .ok_or("control navigation requires a performance control panel")?
            .apply(gesture)?;
    } else if let Some(id) = binding.command.strip_prefix("control:select:") {
        panel
            .ok_or("control selection requires a performance control panel")?
            .select_id(id)?;
    } else if binding.command == "save" {
        let path = performance
            .project_file
            .as_ref()
            .ok_or("performance save command requires project_file")?;
        performance
            .sequencer
            .save_project(std::path::Path::new(path))?;
    } else if binding.command == "reload" {
        let path = performance
            .project_file
            .as_ref()
            .ok_or("performance reload command requires project_file")?;
        performance
            .sequencer
            .load_project(std::path::Path::new(path))?;
    } else if let Some(path) = binding.command.strip_prefix("save_as:") {
        if path.is_empty() {
            return Err("save_as requires a destination".into());
        }
        performance
            .sequencer
            .save_project(std::path::Path::new(path))?;
    } else {
        performance.sequencer.execute(&binding.command)?;
    }
    Ok(true)
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
    let mut out = Vec::with_capacity(16 * 1024);
    let mode = ColorMode::detect();
    if let Some(screens) = screens
        && let Some(WidgetView::PerformanceSurface(surface)) = screens
            .current()
            .widgets
            .iter()
            .find(|widget| matches!(widget, WidgetView::PerformanceSurface(_)))
    {
        let performance = output
            .performance()
            .ok_or("performance surface requires a sequencer")?;
        let (width, height) = terminal::size().unwrap_or((80, 24));
        let frame = crate::performance_ui::render(
            surface,
            &performance.sequencer,
            &mut surface.activity.borrow_mut(),
            Instant::now(),
            width,
            height,
        );
        draw_frame(&mut out, &frame, mode)?;
        return present_frame(out);
    }
    print_palette(&mut out, mode, 28, true, "MELDRITCH 2")?;
    print_palette(&mut out, mode, 31, false, "  ◆  ")?;
    print_palette(&mut out, mode, 40, true, &output.device_name)?;
    print_palette(&mut out, mode, 31, false, "  │  MIDI ")?;
    print_palette(
        &mut out,
        mode,
        if midi_session.is_some() { 20 } else { 4 },
        true,
        midi_session.map_or("OFFLINE", |session| session.port_name.as_str()),
    )?;
    print_palette(&mut out, mode, 31, false, "  │  ")?;
    print_palette(&mut out, mode, 37, true, "Q QUIT")?;
    queue!(out, Print("\r\n")).map_err(|error| error.to_string())?;
    print_palette(
        &mut out,
        mode,
        32,
        false,
        "↑/↓ select  ←/→ adjust  Home/End bounds",
    )?;
    if screens.is_some() {
        print_palette(
            &mut out,
            mode,
            31,
            false,
            "  │  Lua navigation  Enter activate",
        )?;
    }
    queue!(out, Print("\r\n\r\n")).map_err(|error| error.to_string())?;
    if screens.is_none()
        && let Some((page, selected_track)) = step_grid
    {
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
    if let Some(screens) = screens {
        let screen = screens.current();
        let (position, count) = screens.position();
        print_palette(&mut out, mode, 36, true, &screen.title)?;
        print_palette(&mut out, mode, 31, false, format!("  {position}/{count}"))?;
        queue!(out, Print("\r\n\r\n")).map_err(|error| error.to_string())?;
        for (index, widget) in screen.widgets.iter().enumerate() {
            let marker = if screen.selected == Some(index) {
                ">"
            } else {
                " "
            };
            match widget {
                WidgetView::Label(text) => {
                    print_palette(&mut out, mode, 32, false, format!("  {text}"))?;
                    queue!(out, Print("\r\n")).map_err(|error| error.to_string())
                }
                WidgetView::Control { label, row } => {
                    let (min, max) = row.range();
                    print_palette(
                        &mut out,
                        mode,
                        if marker == ">" { 8 } else { 33 },
                        marker == ">",
                        format!(
                            "{marker} {label} = {:.4}  [{min:.4}, {max:.4}]",
                            row.value()
                        ),
                    )?;
                    queue!(out, Print("\r\n")).map_err(|error| error.to_string())
                }
                WidgetView::Link { label, .. } => {
                    print_palette(&mut out, mode, 36, true, format!("{marker} {label}  →"))?;
                    queue!(out, Print("\r\n")).map_err(|error| error.to_string())
                }
                WidgetView::Button { label, .. } => {
                    print_palette(&mut out, mode, 28, true, format!("{marker} [{label}]"))?;
                    queue!(out, Print("\r\n")).map_err(|error| error.to_string())
                }
                WidgetView::PerformanceControls => {
                    print_palette(&mut out, mode, 36, true, "  LIVE CONTROLS")?;
                    queue!(out, Print("\r\n")).map_err(|error| error.to_string())?;
                    for (control_index, row) in panel.rows().iter().enumerate() {
                        let selected = if panel.selected() == Some(control_index) {
                            ">"
                        } else {
                            " "
                        };
                        let (min, max) = row.range();
                        let active = selected == ">";
                        print_palette(
                            &mut out,
                            mode,
                            if active { 8 } else { 33 },
                            active,
                            format!(
                                "{selected} {} = {:.4}  [{min:.4}, {max:.4}]",
                                row.label(),
                                row.value()
                            ),
                        )?;
                        queue!(out, Print("\r\n")).map_err(|error| error.to_string())?;
                    }
                    Ok(())
                }
                WidgetView::ControlFocus => {
                    if panel.selected().is_none() {
                        queue!(
                            out,
                            Print("┌─ PARAMETER ─┐\r\n│ unavailable │\r\n└─────────────┘\r\n")
                        )
                        .map_err(|error| error.to_string())?;
                        continue;
                    }
                    let width = terminal::size().map_or(80, |(width, _)| usize::from(width));
                    let apc = output
                        .performance()
                        .and_then(|performance| performance.apc.as_ref());
                    for line in control_focus_lines(panel, apc, width, &screen.title) {
                        print_semantic_line(&mut out, mode, &line, SemanticLine::Controls)?;
                        queue!(out, Print("\r\n")).map_err(|error| error.to_string())?;
                    }
                    Ok(())
                }
                WidgetView::PerformanceStatus => {
                    if let Some(performance) = output.performance() {
                        let sequence = &performance.sequencer;
                        let patterns = sequence.pattern_names();
                        let active = patterns
                            .get(sequence.active_pattern())
                            .map(String::as_str)
                            .unwrap_or("?");
                        let queued = sequence
                            .queued_pattern()
                            .and_then(|index| patterns.get(index))
                            .map_or(String::new(), |name| format!(" -> {name}"));
                        print_palette(&mut out, mode, 31, false, "  ")?;
                        print_palette(&mut out, mode, 40, true, &sequence.name)?;
                        print_palette(&mut out, mode, 31, false, "  │  ")?;
                        print_palette(
                            &mut out,
                            mode,
                            if sequence.playing() { 20 } else { 4 },
                            true,
                            if sequence.playing() {
                                "PLAYING"
                            } else {
                                "STOPPED"
                            },
                        )?;
                        print_palette(&mut out, mode, 31, false, "  │  ")?;
                        print_palette(
                            &mut out,
                            mode,
                            14,
                            true,
                            format!("{:.0} BPM", sequence.bpm()),
                        )?;
                        print_palette(&mut out, mode, 31, false, "  │  PATTERN ")?;
                        print_palette(&mut out, mode, 28, true, format!("{active}{queued}"))?;
                        print_palette(&mut out, mode, 31, false, "  │  STEP ")?;
                        print_palette(
                            &mut out,
                            mode,
                            37,
                            true,
                            format!("{:02}", sequence.current_step() + 1),
                        )?;
                        queue!(out, Print("\r\n  ")).map_err(|error| error.to_string())?;
                        print_palette(&mut out, mode, 32, false, sequence.status())?;
                        queue!(out, Print("\r\n")).map_err(|error| error.to_string())?;
                        Ok(())
                    } else {
                        queue!(out, Print("  Performance status unavailable\r\n"))
                            .map_err(|error| error.to_string())
                    }
                }
                WidgetView::ArrangementStatus => {
                    let sequence = &output
                        .performance()
                        .ok_or("arrangement status requires a sequencer")?
                        .sequencer;
                    if let Some(status) = sequence.arrangement_status() {
                        print_palette(&mut out, mode, 36, true, format!("  {}", status.name))?;
                        print_palette(&mut out, mode, 31, false, "  │  ")?;
                        print_palette(&mut out, mode, 28, true, &status.current_section)?;
                        if let Some(next) = &status.upcoming_section {
                            print_palette(&mut out, mode, 31, false, "  →  ")?;
                            print_palette(&mut out, mode, 32, false, next)?;
                        }
                        print_palette(&mut out, mode, 31, false, "  │  ")?;
                        print_palette(
                            &mut out,
                            mode,
                            14,
                            true,
                            format!("BAR {:02} · BEAT {}", status.bar, status.beat),
                        )?;
                        queue!(out, Print("\r\n  ")).map_err(|e| e.to_string())?;
                        if let Some(total) = status.total_bars {
                            let width = 48usize;
                            let filled = ((status.bar.min(total) * width as u64) / total) as usize;
                            print_palette(&mut out, mode, 28, true, "█".repeat(filled))?;
                            print_palette(&mut out, mode, 25, false, "░".repeat(width - filled))?;
                            print_palette(
                                &mut out,
                                mode,
                                32,
                                false,
                                format!("  {:>3}%", status.bar.min(total) * 100 / total),
                            )?;
                        }
                        queue!(out, Print("\r\n")).map_err(|e| e.to_string())?;
                        let pending = status.pending_scene.as_deref().unwrap_or("—");
                        print_palette(&mut out, mode, 31, false, "  SCENE ")?;
                        print_palette(&mut out, mode, 20, true, &status.current_scene)?;
                        print_palette(&mut out, mode, 31, false, "  │  PENDING ")?;
                        print_palette(
                            &mut out,
                            mode,
                            if status.pending_scene.is_some() {
                                8
                            } else {
                                30
                            },
                            status.pending_scene.is_some(),
                            pending,
                        )?;
                        queue!(out, Print("\r\n")).map_err(|e| e.to_string())?;
                        for clip in sequence.clip_status() {
                            print_palette(
                                &mut out,
                                mode,
                                31,
                                false,
                                format!("  {:<12}", clip.track),
                            )?;
                            if let Some(name) = clip.clip {
                                print_palette(&mut out, mode, 33, true, format!("{name:<18}"))?;
                                print_palette(
                                    &mut out,
                                    mode,
                                    28,
                                    false,
                                    format!("{:03}/{:03}", clip.position, clip.length),
                                )?;
                            } else {
                                print_palette(&mut out, mode, 30, false, "— SILENT")?;
                            }
                            queue!(out, Print("\r\n")).map_err(|e| e.to_string())?;
                        }
                        Ok(())
                    } else {
                        print_palette(&mut out, mode, 4, true, "  NO ARRANGEMENT\r\n")
                    }
                }
                WidgetView::PatternGrid => {
                    draw_pattern_grid(
                        &mut out,
                        output,
                        step_grid.unwrap_or((0, 0)).0,
                        step_grid.unwrap_or((0, 0)).1,
                        mode,
                    )?;
                    Ok(())
                }
                WidgetView::TransportStatus => {
                    let sequence = &output
                        .performance()
                        .ok_or("transport status requires a sequencer")?
                        .sequencer;
                    print_palette(
                        &mut out,
                        mode,
                        14,
                        true,
                        format!("  {:.0} BPM", sequence.bpm()),
                    )?;
                    print_palette(&mut out, mode, 31, false, "  │  ")?;
                    print_palette(
                        &mut out,
                        mode,
                        if sequence.playing() { 20 } else { 4 },
                        true,
                        if sequence.playing() {
                            "PLAYING"
                        } else {
                            "STOPPED"
                        },
                    )?;
                    print_palette(&mut out, mode, 31, false, "  │  STEP ")?;
                    print_palette(
                        &mut out,
                        mode,
                        37,
                        true,
                        format!("{:02}", sequence.current_step() + 1),
                    )?;
                    queue!(out, Print("\r\n")).map_err(|error| error.to_string())
                }
                WidgetView::PerformanceSurface(_) => Ok(()),
                WidgetView::EventActivity { label, state } => {
                    let (_, pitch, velocity) = state.activity(0).unwrap_or((0, 440.0, 0.0));
                    queue!(
                        out,
                        Print(format!(
                            "{label}: step {:02} | pitch {:.2} Hz | velocity {:.2}\r\n",
                            state.step() + 1,
                            pitch,
                            velocity
                        ))
                    )
                    .map_err(|error| error.to_string())
                }
            }?;
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
    let stream_errors = output.stream_errors();
    let timed_errors = output.timed_note_errors();
    let midi_errors = midi_session.map_or(0, MidiInputSession::note_errors);
    queue!(out, Print("\r\n")).map_err(|error| error.to_string())?;
    print_palette(&mut out, mode, 31, false, "AUDIO ")?;
    print_palette(
        &mut out,
        mode,
        if stream_errors == 0 { 20 } else { 4 },
        true,
        stream_errors,
    )?;
    print_palette(&mut out, mode, 31, false, "  │  TIMED ")?;
    print_palette(
        &mut out,
        mode,
        if timed_errors == 0 { 20 } else { 4 },
        true,
        timed_errors,
    )?;
    print_palette(&mut out, mode, 31, false, "  │  MIDI ")?;
    print_palette(
        &mut out,
        mode,
        if midi_errors == 0 { 20 } else { 4 },
        true,
        midi_errors,
    )?;
    queue!(out, Print("\r\n")).map_err(|error| error.to_string())?;
    present_frame(out)
}

pub fn perform(
    output: &DeviceOutput,
    midi_session: Option<&MidiInputSession>,
    mut keyboard: Option<TuiKeyboard>,
    mut events: Option<&mut crate::lua::EventOwner>,
) -> Result<(), String> {
    let mut apc = match (output.apc_mini(), output.performance()) {
        (Some(name), Some(performance)) => Some(crate::apc::ApcSession::start(
            name.to_owned(),
            performance.clone(),
            output.controls(),
        )),
        _ => None,
    };
    let mut stream_decks = output
        .control_surfaces()
        .iter()
        .cloned()
        .map(|surface| {
            crate::stream_deck::StreamDeckSession::start(
                crate::stream_deck::SurfaceRuntime::new(
                    surface,
                    output.controls(),
                    output.performance().cloned(),
                )
                .with_shared_focus(
                    output.focus_controls(),
                    output.focus_selection(),
                    output.focus_pickup(),
                ),
            )
        })
        .collect::<Vec<_>>();
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
    let navigation = output
        .navigation()
        .map(NavigationRuntime::new)
        .transpose()?;
    if let Some(navigation) = &navigation {
        screens
            .as_mut()
            .ok_or("Lua navigation requires at least one screen")?
            .activate_screen(navigation.initial_screen())?;
    }
    if let (Some(screens), Some(events)) = (&screens, &mut events) {
        events.mount_screen(screens.current_id())?;
    }
    let mut panel = Panel::with_selection_and_pickup(
        output.focus_controls(),
        output.focus_selection(),
        output.focus_pickup(),
    );
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
    let mut callback_errors = Vec::new();
    let mut callback_failure_count = 0;
    loop {
        for stream_deck in &mut stream_decks {
            stream_deck.poll()?;
        }
        if let Some(events) = &mut events {
            events.dispatch();
            for error in events.take_errors() {
                if callback_errors.len() < 32 {
                    callback_errors.push(error);
                }
            }
            callback_failure_count = events.failure_count();
        }
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
        if let (Some(navigation), Some(key_name), Some(screens)) =
            (&navigation, navigation_key(key.code), &mut screens)
            && let Some(screen) = navigation.dispatch(&key_name)?
        {
            let previous = screens.current_id().to_owned();
            screens.activate_screen(&screen)?;
            if previous != screens.current_id()
                && let Some(events) = &mut events
            {
                events.unmount_screen(&previous);
                events.mount_screen(screens.current_id())?;
            }
            draw(
                &panel,
                Some(screens),
                output,
                midi_session,
                keyboard.as_ref(),
                output.performance().map(|_| (grid_page, grid_track)),
            )?;
            continue;
        }
        let grid_active = screens.as_ref().map_or(
            output.performance().is_some(),
            ScreenPanel::has_pattern_grid,
        );
        if grid_active
            && let Some(performance) = output.performance()
            && let Some(key_name) = performance_key(key.code)
            && dispatch_performance_binding(performance, &key_name, Some(&mut panel))?
        {
            grid_page = performance.sequencer.step_bank();
            grid_track = performance.sequencer.selected_track();
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
        if grid_active && let Some(performance) = output.performance() {
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
            let previous = screens.current_id().to_owned();
            match key.code {
                KeyCode::Tab if navigation.is_none() => screens.switch(1),
                KeyCode::BackTab if navigation.is_none() => screens.switch(-1),
                KeyCode::Enter => screens.activate()?,
                _ => {}
            }
            if previous != screens.current_id()
                && let Some(events) = &mut events
            {
                events.unmount_screen(&previous);
                events.mount_screen(screens.current_id())?;
            }
            if (navigation.is_none() && matches!(key.code, KeyCode::Tab | KeyCode::BackTab))
                || key.code == KeyCode::Enter
            {
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
            if screens
                .as_ref()
                .is_some_and(ScreenPanel::has_performance_controls)
            {
                panel.apply(gesture)?;
            } else if let Some(screens) = &mut screens {
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
    // Signal all attached surfaces before joining either worker so the two
    // one-second shutdown sweeps run together rather than serially.
    if let Some(apc) = &apc {
        apc.begin_shutdown();
    }
    for stream_deck in &stream_decks {
        stream_deck.begin_shutdown();
    }
    let mut shutdown_errors = Vec::new();
    for stream_deck in &mut stream_decks {
        if let Err(error) = stream_deck.finish() {
            shutdown_errors.push(error);
        }
    }
    if let Some(apc) = &mut apc
        && let Err(error) = apc.finish()
    {
        shutdown_errors.push(error);
    }
    if !shutdown_errors.is_empty() {
        return Err(format!(
            "control-surface shutdown failed:\n{}",
            shutdown_errors.join("\n")
        ));
    }
    if callback_errors.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "Lua event callback failures ({callback_failure_count}; showing up to 32):\n{}",
            callback_errors.join("\n")
        ))
    }
}

#[cfg(test)]
mod tests {
    use crossterm::event::KeyCode;

    use super::{
        TuiKeyboard, control_focus_lines, dispatch_performance_binding, frame_lines,
        navigation_key, piano_roll_lines, terminal_color,
    };
    use crate::{
        compiler::compile,
        live_notes::live_note_queue,
        lua,
        midi::TimedMidiRouter,
        model::ApcPianoRoll,
        sequencer::SequencerControl,
        timed::{TimedNotePlan, TimedNoteState},
    };

    #[test]
    fn hybrid_drum_and_piano_roll_is_compact_aligned_and_musical() {
        let sequence = SequencerControl::new("lab".into(), 118.0, 64, 4).unwrap();
        for name in ["Kick", "Snare", "Closed Hat", "Bassline"] {
            sequence.add_track(name.into()).unwrap();
        }
        sequence.set_key_root(36).unwrap();
        sequence
            .set_pattern_steps(0, 0, &[1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0])
            .unwrap();
        sequence.set_pattern_length(0, 1, 16).unwrap();
        sequence.set_pattern_length(0, 2, 16).unwrap();
        sequence.set_pattern_length(0, 3, 32).unwrap();
        sequence.set_note(3, 0, 65.406_39, 0.9, 2.0).unwrap();
        let roll = ApcPianoRoll {
            drum_tracks: [0, 1, 2],
            melodic_track: 3,
            scale: [0, 2, 3, 5, 7],
        };
        let lines = piano_roll_lines(&sequence, &roll, 80);
        assert_eq!(lines.len(), 13);
        assert!(lines[1].contains("C2 z1"));
        assert!(lines.iter().any(|line| line.contains("Closed …")));
        assert!(
            lines
                .iter()
                .any(|line| line.contains("BASS C2") && line.contains('◆'))
        );
        for line in &lines[1..12] {
            assert_eq!(line.chars().count(), 50);
            assert!(matches!(line.chars().last(), Some('│' | '┤')));
            if line.starts_with('│') {
                assert_eq!(line.chars().last(), Some('│'));
            }
        }
        let wide = piano_roll_lines(&sequence, &roll, 120);
        assert!(wide[1].contains("01 02 03 04  05"));
        assert_eq!(wide[1].chars().count(), 114);

        let mut runtime = crate::sequencer::SequencerRuntime::new(sequence.clone());
        for _ in 0..100 {
            runtime.begin_frame(8.0);
            if sequence.current_step() == 16 {
                break;
            }
        }
        assert_eq!(sequence.current_step(), 16);
        let wrapped = piano_roll_lines(&sequence, &roll, 80);
        assert!(
            wrapped
                .iter()
                .any(|line| line.starts_with("│Kick    │") && line.contains('█')),
            "the repeating kick row must retain the global playhead during bass step 17"
        );
    }

    #[test]
    fn function_keys_are_delivered_as_authored_navigation_names() {
        assert_eq!(navigation_key(KeyCode::F(1)).as_deref(), Some("F1"));
        assert_eq!(navigation_key(KeyCode::F(2)).as_deref(), Some("F2"));
        assert_eq!(navigation_key(KeyCode::Left), None);
    }

    #[test]
    fn terminal_colors_support_truecolor_ansi_and_monochrome() {
        use crate::performance_ui::ColorMode;
        assert_eq!(terminal_color(4, ColorMode::Mono), None);
        assert_eq!(
            terminal_color(4, ColorMode::TrueColor),
            Some(crossterm::style::Color::Rgb {
                r: 0xec,
                g: 0x27,
                b: 0x3f
            })
        );
        assert!(matches!(
            terminal_color(4, ColorMode::Ansi256),
            Some(crossterm::style::Color::AnsiValue(_))
        ));
    }

    #[test]
    fn focused_control_bank_has_aligned_borders_at_supported_widths() {
        let graph = lua::read(include_str!("../examples/09-controls.lua")).unwrap();
        let machine = compile(&graph).unwrap();
        let controls = machine
            .control_ids()
            .map(|id| {
                let handle = machine.control_handle(id).unwrap();
                let value = machine.effective_value(handle.target()).unwrap();
                (handle, value)
            })
            .collect();
        let panel = crate::panel::Panel::new(controls);
        for width in [80, 100, 120] {
            let lines = control_focus_lines(&panel, None, width, "Instrument");
            let expected = lines[0].chars().count();
            assert!(lines.iter().all(|line| line.chars().count() == expected));
            assert!(
                lines
                    .iter()
                    .all(|line| line.ends_with(['┐', '│', '┤', '┘']))
            );
        }
        let title = control_focus_lines(&panel, None, 100, "Ghosts in the Machinery").join("\n");
        assert!(title.contains("GHOSTS IN THE MACHINERY CONTROLS"));
        assert!(!title.contains("DELAY LAB"));
    }

    #[test]
    fn focused_controls_show_their_active_apc_fader_number_inline() {
        let graph = lua::read(include_str!(
            "../../../examples/experiments/02_delay_feedback.lua"
        ))
        .unwrap();
        let machine = compile(&graph).unwrap();
        let controls = machine
            .control_ids()
            .map(|id| {
                let handle = machine.control_handle(id).unwrap();
                let value = machine.effective_value(handle.target()).unwrap();
                (handle, value)
            })
            .collect();
        let panel = crate::panel::Panel::new(controls);
        let apc = graph.performance.as_ref().unwrap().apc.as_ref().unwrap();
        apc.fader_bank
            .store(1, std::sync::atomic::Ordering::Release);
        let text = control_focus_lines(&panel, Some(apc), 120, "Delay Lab").join("\n");
        assert!(text.contains("1 Left echo"));
        assert!(text.contains("8 Delay mix"));
        assert!(text.contains("9 Master"));
        assert!(!text.contains("ACTIVE BANK"));
    }

    #[test]
    fn buffered_frames_are_split_into_stable_terminal_rows() {
        assert_eq!(
            frame_lines(b"short\r\nnext\r\n"),
            vec![b"short".to_vec(), b"next".to_vec(), Vec::new()]
        );
        assert_eq!(frame_lines(b"no newline"), vec![b"no newline".to_vec()]);
    }

    #[test]
    fn authored_keyboard_bindings_dispatch_the_same_semantic_commands() {
        let graph = lua::read(
            r#"
            local i=meldritch.instrument("keys")
            local o=i:constant {value=.5}
            i:connect(o.out,i.output.left);i:connect(o.out,i.output.right)
            i:control {id="level",label="Level",target=o.value,range={0,1}}
            local q=i:sequencer {bpm=120,steps=16,steps_per_beat=4}
            q:track("One")
            i:performance {layout="step_grid",sequencer=q,keys={m="mute",l="control:increase",["1"]="toggle:1"}}
            return i
            "#,
        )
        .unwrap();
        let performance = graph.performance.as_ref().unwrap();
        assert!(!dispatch_performance_binding(performance, "x", None).unwrap());
        assert!(dispatch_performance_binding(performance, "m", None).unwrap());
        assert!(performance.sequencer.muted(0));
        assert!(dispatch_performance_binding(performance, "1", None).unwrap());
        assert_eq!(performance.sequencer.pattern().velocities[0][0], 1.0);
        let machine = compile(&graph).unwrap();
        let handle = machine.control_handle("level").unwrap();
        let value = machine.effective_value(handle.target()).unwrap();
        let mut panel = crate::panel::Panel::new(vec![(handle, value)]);
        assert!(dispatch_performance_binding(performance, "l", Some(&mut panel)).unwrap());
        assert!((panel.rows()[0].value() - 0.51).abs() < 1e-12);
    }

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
