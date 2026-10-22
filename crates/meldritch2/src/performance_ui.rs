//! Reusable sequencer performance-surface layout and Lospec 500 theme.

use std::cell::RefCell;
use std::time::{Duration, Instant};

use crate::{
    event_bridge::EventUiState,
    model::PerformanceSurface,
    panel::ControlRow,
    sequencer::{MAX_STEPS, SequencerControl},
};

const SURFACE_TRACKS: usize = 8;

/// Authoritative 42-color Lospec 500 palette:
/// https://lospec.com/palette-list/lospec500
pub(crate) const LOSPEC500: [[u8; 3]; 42] = [
    [0x10, 0x12, 0x1c],
    [0x2c, 0x1e, 0x31],
    [0x6b, 0x26, 0x43],
    [0xac, 0x28, 0x47],
    [0xec, 0x27, 0x3f],
    [0x94, 0x49, 0x3a],
    [0xde, 0x5d, 0x3a],
    [0xe9, 0x85, 0x37],
    [0xf3, 0xa8, 0x33],
    [0x4d, 0x35, 0x33],
    [0x6e, 0x4c, 0x30],
    [0xa2, 0x6d, 0x3f],
    [0xce, 0x92, 0x48],
    [0xda, 0xb1, 0x63],
    [0xe8, 0xd2, 0x82],
    [0xf7, 0xf3, 0xb7],
    [0x1e, 0x40, 0x44],
    [0x00, 0x65, 0x54],
    [0x26, 0x85, 0x4c],
    [0x5a, 0xb5, 0x52],
    [0x9d, 0xe6, 0x4e],
    [0x00, 0x8b, 0x8b],
    [0x62, 0xa4, 0x77],
    [0xa6, 0xcb, 0x96],
    [0xd3, 0xee, 0xd3],
    [0x3e, 0x3b, 0x65],
    [0x38, 0x59, 0xb3],
    [0x33, 0x88, 0xde],
    [0x36, 0xc5, 0xf4],
    [0x6d, 0xea, 0xd6],
    [0x5e, 0x5b, 0x8c],
    [0x8c, 0x78, 0xa5],
    [0xb0, 0xa7, 0xb8],
    [0xde, 0xce, 0xed],
    [0x9a, 0x4d, 0x76],
    [0xc8, 0x78, 0xaf],
    [0xcc, 0x99, 0xff],
    [0xfa, 0x6e, 0x79],
    [0xff, 0xa2, 0xac],
    [0xff, 0xd1, 0xd5],
    [0xf6, 0xe8, 0xe0],
    [0xff, 0xff, 0xff],
];

pub(crate) fn palette_index(name: &str) -> Option<usize> {
    match name {
        "red" => Some(4),
        "orange" => Some(7),
        "yellow" => Some(14),
        "green" => Some(20),
        "teal" => Some(21),
        "cyan" => Some(28),
        "blue" => Some(27),
        "violet" => Some(36),
        "pink" => Some(37),
        "cream" => Some(40),
        "white" => Some(41),
        value if value.starts_with('#') && value.len() == 7 => LOSPEC500.iter().position(|rgb| {
            format!("#{:02x}{:02x}{:02x}", rgb[0], rgb[1], rgb[2]) == value.to_ascii_lowercase()
        }),
        _ => None,
    }
}

const PITCH_COLORS: [usize; 12] = [4, 6, 8, 14, 20, 24, 29, 28, 27, 36, 37, 38];

pub(crate) fn pitch_color(frequency: f64) -> usize {
    if !frequency.is_finite() || frequency <= 0.0 {
        return PITCH_COLORS[0];
    }
    let midi = (69.0 + 12.0 * (frequency / 440.0).log2()).round() as i32;
    PITCH_COLORS[midi.rem_euclid(12) as usize]
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ColorMode {
    TrueColor,
    Ansi256,
    Mono,
}

impl ColorMode {
    pub(crate) fn detect() -> Self {
        if std::env::var_os("NO_COLOR").is_some()
            || std::env::var("TERM").is_ok_and(|v| v == "dumb")
        {
            Self::Mono
        } else if std::env::var("COLORTERM")
            .is_ok_and(|v| matches!(v.as_str(), "truecolor" | "24bit"))
        {
            Self::TrueColor
        } else {
            Self::Ansi256
        }
    }
}

pub(crate) struct TrackView {
    pub(crate) label: String,
    pub(crate) volume: ControlRow,
    pub(crate) controls: Vec<ControlRow>,
    pub(crate) color: usize,
    pub(crate) melodic: bool,
}

pub(crate) struct SurfaceView {
    pub(crate) title: String,
    pub(crate) tracks: Vec<TrackView>,
    pub(crate) activity: RefCell<ActivityAnimator>,
    pub(crate) events: std::sync::Arc<EventUiState>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Span {
    pub text: String,
    pub color: Option<usize>,
    pub bold: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Frame {
    pub lines: Vec<Vec<Span>>,
}

impl Frame {
    fn line(&mut self, text: impl Into<String>, color: Option<usize>, bold: bool) {
        self.lines.push(vec![Span {
            text: text.into(),
            color,
            bold,
        }]);
    }
    fn spans(&mut self, spans: Vec<Span>) {
        self.lines.push(spans);
    }
    #[cfg(test)]
    pub(crate) fn plain_lines(&self) -> Vec<String> {
        self.lines
            .iter()
            .map(|line| line.iter().map(|s| s.text.as_str()).collect())
            .collect()
    }
}

pub(crate) struct ActivityAnimator {
    seen: [u64; SURFACE_TRACKS],
    pitch: [f64; SURFACE_TRACKS],
    until: [Option<Instant>; SURFACE_TRACKS],
}

impl Default for ActivityAnimator {
    fn default() -> Self {
        Self {
            seen: [0; SURFACE_TRACKS],
            pitch: [440.0; SURFACE_TRACKS],
            until: [None; SURFACE_TRACKS],
        }
    }
}

impl ActivityAnimator {
    fn update(&mut self, events: &EventUiState, now: Instant) {
        for track in 0..SURFACE_TRACKS {
            if let Some((generation, pitch, _velocity)) = events.activity(track)
                && generation != self.seen[track]
            {
                self.seen[track] = generation;
                self.pitch[track] = pitch;
                self.until[track] = Some(now + Duration::from_millis(180));
            }
            if self.until[track].is_some_and(|until| now >= until) {
                self.until[track] = None;
            }
        }
    }
    pub(crate) fn active(&self, track: usize) -> bool {
        self.until[track].is_some()
    }
    pub(crate) fn pitch(&self, track: usize) -> f64 {
        self.pitch[track]
    }
}

fn fit(value: &str, width: usize) -> String {
    let mut value = value.chars().take(width).collect::<String>();
    value.extend(std::iter::repeat_n(
        ' ',
        width.saturating_sub(value.chars().count()),
    ));
    value
}

fn border(width: usize, left: char, fill: char, right: char) -> String {
    format!(
        "{left}{}{right}",
        fill.to_string().repeat(width.saturating_sub(2))
    )
}

fn note_name(frequency: f64) -> String {
    let midi = (69.0 + 12.0 * (frequency / 440.0).log2()).round() as i32;
    const NAMES: [&str; 12] = [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];
    format!(
        "{}{}",
        NAMES[midi.rem_euclid(12) as usize],
        midi.div_euclid(12) - 1
    )
}

fn transport(frame: &mut Frame, view: &SurfaceView, sequence: &SequencerControl, width: usize) {
    let names = sequence.pattern_names();
    let active = names
        .get(view.events.pattern())
        .map(String::as_str)
        .unwrap_or("?");
    let queued = sequence
        .queued_pattern()
        .and_then(|i| names.get(i))
        .map_or(String::new(), |v| format!(" → {v}"));
    let state = if sequence.playing() { "▶" } else { "■" };
    let bar = view.events.position() / 16 + 1;
    let text = format!(
        " {state} {:.0} BPM │ {active}{queued} │ BAR {bar:02} STEP {:02} │ {} ",
        sequence.bpm(),
        view.events.step() + 1,
        view.title
    );
    frame.line(
        format!("╔{}╗", "═".repeat(width.saturating_sub(2))),
        Some(27),
        true,
    );
    frame.line(
        format!("║{}║", fit(&text, width.saturating_sub(2))),
        Some(41),
        true,
    );
    frame.line(
        format!("╚{}╝", "═".repeat(width.saturating_sub(2))),
        Some(27),
        true,
    );
}

fn full_strips(
    frame: &mut Frame,
    view: &SurfaceView,
    sequence: &SequencerControl,
    activity: &ActivityAnimator,
    width: usize,
) {
    let strip = ((width.saturating_sub(7)) / 8).max(10);
    let mut rows = vec![Vec::<Span>::new(); 9];
    for (index, track) in view.tracks.iter().take(8).enumerate() {
        let inner = strip - 2;
        let (min, max) = track.volume.range();
        let normalized = ((track.volume.value() - min) / (max - min)).clamp(0.0, 1.0);
        let active = activity.active(index) && sequence.track_enabled(index);
        let state = if sequence.muted(index) {
            "MUTE"
        } else if sequence.soloed(index) {
            "SOLO"
        } else if active {
            "●"
        } else {
            "○"
        };
        let selected = sequence.selected_track() == index;
        let parts = [
            border(
                strip,
                if selected { '╔' } else { '┌' },
                if selected { '═' } else { '─' },
                if selected { '╗' } else { '┐' },
            ),
            format!("│{}│", fit(&track.label.to_uppercase(), inner)),
            format!("│{}│", fit(state, inner)),
            format!(
                "│{}│",
                fit(if normalized >= 0.80 { "  █" } else { "  ░" }, inner)
            ),
            format!(
                "│{}│",
                fit(if normalized >= 0.60 { "  █" } else { "  ░" }, inner)
            ),
            format!(
                "│{}│",
                fit(if normalized >= 0.40 { "  █" } else { "  ░" }, inner)
            ),
            format!(
                "│{}│",
                fit(if normalized >= 0.20 { "  █" } else { "  ░" }, inner)
            ),
            format!(
                "│{}│",
                fit(&format!("{:>3}%", (normalized * 100.0).round()), inner)
            ),
            border(
                strip,
                if selected { '╚' } else { '└' },
                if selected { '═' } else { '─' },
                if selected { '╝' } else { '┘' },
            ),
        ];
        let active_color = if active {
            if track.melodic {
                pitch_color(activity.pitch(index))
            } else {
                track.color
            }
        } else if sequence.muted(index) {
            5
        } else {
            32
        };
        for (row_index, (row, part)) in rows.iter_mut().zip(parts).enumerate() {
            if index > 0 {
                row.push(Span {
                    text: " ".into(),
                    color: None,
                    bold: false,
                });
            }
            row.push(Span {
                text: part,
                color: Some(if row_index == 2 {
                    active_color
                } else if selected {
                    29
                } else {
                    32
                }),
                bold: selected || (active && row_index == 2),
            });
        }
    }
    for row in rows {
        frame.spans(row);
    }
}

fn compact_strips(
    frame: &mut Frame,
    view: &SurfaceView,
    sequence: &SequencerControl,
    activity: &ActivityAnimator,
    width: usize,
    tiny: bool,
) {
    let columns = 4;
    let strip = (width.saturating_sub(columns - 1) / columns).max(12);
    for chunk in 0..2 {
        let mut top = Vec::new();
        let mut info = Vec::new();
        let mut bottom = Vec::new();
        for column in 0..columns {
            let index = chunk * columns + column;
            let track = &view.tracks[index];
            let inner = strip - 2;
            let (min, max) = track.volume.range();
            let normalized = ((track.volume.value() - min) / (max - min)).clamp(0.0, 1.0);
            let blocks = (normalized * (inner.saturating_sub(7)) as f64).round() as usize;
            let audible_activity = activity.active(index) && sequence.track_enabled(index);
            let state = if sequence.muted(index) {
                "M"
            } else if sequence.soloed(index) {
                "S"
            } else if audible_activity {
                "●"
            } else {
                "○"
            };
            let selected = sequence.selected_track() == index;
            if column > 0 {
                for row in [&mut top, &mut info, &mut bottom] {
                    row.push(Span {
                        text: " ".into(),
                        color: None,
                        bold: false,
                    });
                }
            }
            top.push(Span {
                text: border(
                    strip,
                    if selected { '╔' } else { '┌' },
                    if selected { '═' } else { '─' },
                    if selected { '╗' } else { '┐' },
                ),
                color: Some(if selected { 29 } else { 32 }),
                bold: selected,
            });
            let active_color = if audible_activity {
                if track.melodic {
                    pitch_color(activity.pitch(index))
                } else {
                    track.color
                }
            } else if sequence.muted(index) {
                5
            } else {
                40
            };
            info.push(Span {
                text: format!(
                    "│{}│",
                    fit(
                        &format!("{state} {:4} {}", track.label, "█".repeat(blocks)),
                        inner
                    )
                ),
                color: Some(active_color),
                bold: audible_activity,
            });
            bottom.push(Span {
                text: border(
                    strip,
                    if selected { '╚' } else { '└' },
                    if selected { '═' } else { '─' },
                    if selected { '╝' } else { '┘' },
                ),
                color: Some(if selected { 29 } else { 32 }),
                bold: selected,
            });
        }
        if !tiny {
            frame.spans(top);
        }
        frame.spans(info);
        if !tiny {
            frame.spans(bottom);
        }
    }
}

fn steps(frame: &mut Frame, view: &SurfaceView, sequence: &SequencerControl) {
    let pattern = sequence.pattern();
    let first = (sequence.step_bank() / 2) * 16;
    let viewed = sequence
        .pattern_names()
        .get(sequence.viewed_pattern())
        .cloned()
        .unwrap_or_else(|| "?".into());
    frame.line(
        format!(
            "┌─ SEQUENCER · VIEW {viewed} · {:02}-{:02} ─┐",
            first + 1,
            (first + 16).min(MAX_STEPS)
        ),
        Some(31),
        true,
    );
    for (track, meta) in view.tracks.iter().enumerate() {
        let mut spans = vec![Span {
            text: format!(
                "{}{:4} ",
                if sequence.selected_track() == track {
                    "▶"
                } else {
                    " "
                },
                fit(&meta.label, 4)
            ),
            color: Some(if sequence.selected_track() == track {
                29
            } else {
                32
            }),
            bold: sequence.selected_track() == track,
        }];
        let length = usize::from(pattern.lengths[track]);
        for step in first..(first + 16).min(length) {
            let velocity = pattern.velocities[track][step];
            let playhead = sequence.playing() && view.events.step() % length == step;
            let glyph = match (velocity > 0.0, velocity >= 0.85, playhead) {
                (false, _, false) => "·",
                (true, false, false) => "●",
                (true, true, false) => "◆",
                (false, _, true) => "│",
                (true, _, true) => "█",
            };
            let color = if sequence.muted(track) {
                5
            } else if playhead {
                8
            } else if velocity > 0.0 {
                meta.color
            } else {
                25
            };
            spans.push(Span {
                text: glyph.into(),
                color: Some(color),
                bold: playhead,
            });
            if step % 4 == 3 && step != first + 15 {
                spans.push(Span {
                    text: " ".into(),
                    color: None,
                    bold: false,
                });
            }
        }
        frame.spans(spans);
    }
    frame.line("└────────────────────────────────┘", Some(31), false);
}

fn controls(frame: &mut Frame, view: &SurfaceView, sequence: &SequencerControl, width: usize) {
    let index = sequence.selected_track().min(view.tracks.len() - 1);
    let track = &view.tracks[index];
    let pattern = sequence.pattern();
    let step = sequence
        .selected_step()
        .min(usize::from(pattern.lengths[index]).saturating_sub(1));
    let mut heading = format!(" {} CONTROLS ", track.label.to_uppercase());
    if track.melodic {
        heading.push_str(&format!(
            "│ {} {:.1}Hz V{:.0}% G{:.0}% ",
            note_name(pattern.pitches_hz[index][step] as f64),
            pattern.pitches_hz[index][step],
            pattern.velocities[index][step] * 100.0,
            pattern.gates[index][step] * 100.0
        ));
    }
    frame.line(
        format!(
            "┌─{heading}{}┐",
            "─".repeat(width.saturating_sub(heading.chars().count() + 3))
        ),
        Some(29),
        true,
    );
    let controls = std::iter::once(&track.volume)
        .chain(&track.controls)
        .map(|row| {
            let (min, max) = row.range();
            let norm = ((row.value() - min) / (max - min)).clamp(0.0, 1.0);
            let filled = (norm * 8.0).round() as usize;
            format!(
                "{} [{}{}] {:.2}",
                row.label(),
                "█".repeat(filled),
                "░".repeat(8 - filled),
                row.value()
            )
        })
        .collect::<Vec<_>>();
    let mut row = String::new();
    for control in controls {
        let separator = if row.is_empty() { "" } else { "  │  " };
        if !row.is_empty()
            && row.chars().count() + separator.len() + control.chars().count()
                > width.saturating_sub(3)
        {
            frame.line(
                format!("│ {}", fit(&row, width.saturating_sub(3))),
                Some(40),
                false,
            );
            row.clear();
        }
        if !row.is_empty() {
            row.push_str("  │  ");
        }
        row.push_str(&control);
    }
    frame.line(
        format!("│ {}", fit(&row, width.saturating_sub(3))),
        Some(40),
        false,
    );
    frame.line(border(width, '└', '─', '┘'), Some(29), false);
}

pub(crate) fn render(
    view: &SurfaceView,
    sequence: &SequencerControl,
    activity: &mut ActivityAnimator,
    now: Instant,
    width: u16,
    height: u16,
) -> Frame {
    activity.update(&view.events, now);
    let width = usize::from(width).max(40);
    let mut frame = Frame::default();
    transport(&mut frame, view, sequence, width);
    if width >= 112 {
        full_strips(&mut frame, view, sequence, activity, width);
    } else {
        compact_strips(&mut frame, view, sequence, activity, width, height < 27);
    }
    steps(&mut frame, view, sequence);
    controls(&mut frame, view, sequence, width);
    if height >= 27 {
        frame.line(" F1-F4 pattern  1-8 steps  ↑↓ track  [ ] bank  m mute  o solo  j/k control  h/l adjust  q quit", Some(32), false);
    }
    for line in &mut frame.lines {
        let mut remaining = width;
        for span in line.iter_mut() {
            span.text = span.text.chars().take(remaining).collect();
            remaining = remaining.saturating_sub(span.text.chars().count());
        }
        line.retain(|span| !span.text.is_empty());
    }
    frame.lines.truncate(usize::from(height));
    frame
}

pub(crate) fn validate_surface(surface: &PerformanceSurface) -> Result<(), String> {
    if surface.theme != "lospec500" {
        return Err(format!("unknown performance theme '{}'", surface.theme));
    }
    if surface.tracks.len() != 8 {
        return Err("performance surface requires exactly eight tracks".into());
    }
    for track in &surface.tracks {
        if palette_index(&track.color).is_none() {
            return Err(format!("unknown Lospec 500 color '{}'", track.color));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{compiler::compile, lua};
    use std::sync::Arc;

    fn fixture() -> (SurfaceView, Arc<SequencerControl>, crate::compiler::Machine) {
        let graph = lua::read(r#"
          local i=meldritch.instrument("surface")
          local silence=i:constant {value=0};i:connect(silence.out,i.output.left);i:connect(silence.out,i.output.right)
          local q=i:sequencer {bpm=120,steps=16,steps_per_beat=4}
          for n=1,8 do
            q:track("T"..n)
            local volume=i:constant {value=.5};local voice=i:constant {value=.25}
            i:control {id="volume_"..n,label="Volume "..n,target=volume.value,range={0,1}}
            i:control {id="voice_"..n,label="Voice "..n,target=voice.value,range={0,1}}
          end
          i:performance {layout="step_grid",sequencer=q}
          return i
        "#).unwrap();
        let sequence = Arc::clone(&graph.performance.as_ref().unwrap().sequencer);
        let machine = compile(&graph).unwrap();
        let row = |id: &str| {
            let handle = machine.control_handle(id).unwrap();
            let value = machine.effective_value(handle.target()).unwrap();
            ControlRow::new(handle, value)
        };
        let tracks = (0..8)
            .map(|index| TrackView {
                label: format!("T{}", index + 1),
                volume: row(&format!("volume_{}", index + 1)),
                controls: if index == 0 {
                    Vec::new()
                } else {
                    vec![row(&format!("voice_{}", index + 1))]
                },
                color: [4, 7, 14, 20, 37, 36, 27, 28][index],
                melodic: index >= 6,
            })
            .collect();
        (
            SurfaceView {
                title: "TEST".into(),
                tracks,
                activity: RefCell::new(Default::default()),
                events: Arc::new(Default::default()),
            },
            sequence,
            machine,
        )
    }

    #[test]
    fn authoritative_palette_and_pitch_mapping_are_stable() {
        assert_eq!(LOSPEC500.len(), 42);
        assert_eq!(LOSPEC500[0], [0x10, 0x12, 0x1c]);
        assert_eq!(LOSPEC500[41], [0xff, 0xff, 0xff]);
        assert_eq!(palette_index("cyan"), Some(28));
        assert_eq!(pitch_color(261.625_58), pitch_color(523.251_1));
        assert_ne!(pitch_color(261.625_58), pitch_color(293.664_76));
    }

    #[test]
    fn channel_values_states_queue_and_responsive_sizes_render_semantically() {
        let (view, sequence, _machine) = fixture();
        sequence.define_pattern("Foundation".into()).unwrap();
        sequence.define_pattern("Broken".into()).unwrap();
        sequence.queue_pattern(1).unwrap();
        sequence.toggle_mute(0).unwrap();
        sequence.toggle_solo(1).unwrap();
        let now = Instant::now();
        for (width, height) in [(120, 35), (100, 30), (80, 24)] {
            let frame = render(
                &view,
                &sequence,
                &mut view.activity.borrow_mut(),
                now,
                width,
                height,
            );
            let lines = frame.plain_lines();
            assert!(lines.len() <= height as usize);
            assert!(
                lines
                    .iter()
                    .all(|line| line.chars().count() <= width as usize)
            );
            let text = lines.join("\n");
            assert!(text.contains("Foundation → Broken"));
            assert!(text.contains("MUTE") || text.contains("M T1"));
            assert!(text.contains("SOLO") || text.contains("S T2"));
            assert!(text.contains("SEQUENCER"));
            assert!(text.contains("T1 CONTROLS"));
        }
        let handle = view.tracks[0].volume.id().to_owned();
        assert_eq!(handle, "volume_1");
        view.tracks[0].volume.publish(1.0).unwrap();
        let text = render(
            &view,
            &sequence,
            &mut view.activity.borrow_mut(),
            now,
            120,
            35,
        )
        .plain_lines()
        .join("\n");
        assert!(text.contains("100%"));
    }

    #[test]
    fn dispatched_activity_uses_latest_pitch_and_decays() {
        let (view, sequence, _machine) = fixture();
        let base = Instant::now();
        view.events.observe_trigger(7, 261.625_58, 1.0).unwrap();
        render(
            &view,
            &sequence,
            &mut view.activity.borrow_mut(),
            base,
            120,
            35,
        );
        assert!(view.activity.borrow().active(6));
        assert_eq!(
            pitch_color(view.activity.borrow().pitch(6)),
            pitch_color(261.625_58)
        );
        view.events.observe_trigger(7, 293.664_76, 0.8).unwrap();
        render(
            &view,
            &sequence,
            &mut view.activity.borrow_mut(),
            base + Duration::from_millis(20),
            120,
            35,
        );
        assert_eq!(
            pitch_color(view.activity.borrow().pitch(6)),
            pitch_color(293.664_76)
        );
        render(
            &view,
            &sequence,
            &mut view.activity.borrow_mut(),
            base + Duration::from_millis(250),
            120,
            35,
        );
        assert!(!view.activity.borrow().active(6));
    }

    #[test]
    fn playhead_and_transport_position_advance_without_touching_audio_graph_state() {
        let (view, sequence, _machine) = fixture();
        let base = Instant::now();
        view.events.observe_step(1, 1, 0);
        let first = render(
            &view,
            &sequence,
            &mut view.activity.borrow_mut(),
            base,
            80,
            24,
        )
        .plain_lines()
        .join("\n");
        view.events.observe_step(2, 1, 1);
        let second = render(
            &view,
            &sequence,
            &mut view.activity.borrow_mut(),
            base,
            80,
            24,
        )
        .plain_lines()
        .join("\n");
        assert_ne!(first, second);
        assert_eq!(view.events.position(), 1);
    }
}
