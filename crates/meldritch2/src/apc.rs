//! APC mini mk2 adapter for the semantic `step_grid` performance surface.

use crate::{
    compiler::ControlHandle, device::unique_name_match, model::Performance,
    performance_ui::LOSPEC500, sequencer::SequencerControl,
};
use midir::{Ignore, MidiInput, MidiOutput, MidiOutputConnection};
use std::collections::BTreeMap;
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender},
    },
    thread,
    time::{Duration, Instant},
};

pub const PAGE_A_NOTE: u8 = 100;
pub const PAGE_B_NOTE: u8 = 101;
pub const UP_NOTE: u8 = 104;
pub const DOWN_NOTE: u8 = 105;
pub const LEFT_NOTE: u8 = 106;
pub const RIGHT_NOTE: u8 = 107;
pub const PLAY_STOP_NOTE: u8 = 112;
pub const RESTART_NOTE: u8 = 113;
pub const CLEAR_NOTE: u8 = 114;
const BUTTON_NOTES: [u8; 16] = [
    100, 101, 102, 103, 104, 105, 106, 107, 112, 113, 114, 115, 116, 117, 118, 119,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Input {
    Grid(u8, bool),
    Button(u8),
    Fader(u8, u8),
}

fn decode(bytes: &[u8]) -> Option<Input> {
    if bytes.len() >= 3 && bytes[0] & 0xf0 == 0xb0 && (48..=56).contains(&bytes[1]) {
        return Some(Input::Fader(bytes[1] - 48, bytes[2]));
    }
    if bytes.len() < 3 || bytes[0] & 0x0f != 0 {
        return None;
    }
    if bytes[1] <= 63 && matches!(bytes[0] & 0xf0, 0x80 | 0x90) {
        return Some(Input::Grid(
            bytes[1],
            bytes[0] & 0xf0 == 0x90 && bytes[2] > 0,
        ));
    }
    if bytes[2] == 0 || bytes[0] & 0xf0 != 0x90 {
        return None;
    }
    BUTTON_NOTES
        .contains(&bytes[1])
        .then_some(Input::Button(bytes[1]))
}

pub fn apc_rgb(cell: u8, rgb: [u8; 3]) -> Result<[u8; 16], String> {
    apc_rgb_range(cell, cell, rgb)
}

fn apc_rgb_range(start: u8, end: u8, rgb: [u8; 3]) -> Result<[u8; 16], String> {
    if start > end || end >= 64 {
        return Err("APC RGB pad range out of bounds".into());
    }
    Ok([
        0xf0,
        0x47,
        0x7f,
        0x4f,
        0x24,
        0,
        8,
        start,
        end,
        rgb[0] >> 7,
        rgb[0] & 0x7f,
        rgb[1] >> 7,
        rgb[1] & 0x7f,
        rgb[2] >> 7,
        rgb[2] & 0x7f,
        0xf7,
    ])
}

fn apc_rgb_chunk(start: u8, colors: &[[u8; 3]]) -> Result<Vec<u8>, String> {
    const BYTES_PER_PAD: usize = 8;
    if colors.is_empty() || usize::from(start) + colors.len() > 64 {
        return Err("APC RGB chunk must address one to 64 pads".into());
    }
    let payload_len = colors.len() * BYTES_PER_PAD;
    let mut message = Vec::with_capacity(7 + payload_len + 1);
    message.extend_from_slice(&[
        0xf0,
        0x47,
        0x7f,
        0x4f,
        0x24,
        ((payload_len >> 7) & 0x7f) as u8,
        (payload_len & 0x7f) as u8,
    ]);
    for (offset, rgb) in colors.iter().enumerate() {
        let cell = start + offset as u8;
        message.extend_from_slice(&[
            cell,
            cell,
            rgb[0] >> 7,
            rgb[0] & 0x7f,
            rgb[1] >> 7,
            rgb[1] & 0x7f,
            rgb[2] >> 7,
            rgb[2] & 0x7f,
        ]);
    }
    message.push(0xf7);
    Ok(message)
}

fn dirty_rgb_chunks(sent: Option<&[[u8; 3]; 64]>, leds: &[[u8; 3]; 64]) -> [bool; 4] {
    std::array::from_fn(|chunk| {
        let start = chunk * 16;
        sent.is_none_or(|sent| sent[start..start + 16] != leds[start..start + 16])
    })
}

#[derive(Debug)]
struct GridController {
    page: usize,
    selected_track: usize,
    selected_step: usize,
    clear_armed: Option<Instant>,
    sent: Option<([[u8; 3]; 64], [bool; 16])>,
    held_note: Option<(u8, usize, f32)>,
    fader_bank: usize,
    fader_pickups: [FaderPickup; 9],
}

#[derive(Clone, Copy, Debug, Default)]
struct FaderPickup {
    previous: Option<f64>,
    captured: bool,
}
impl GridController {
    fn new() -> Self {
        Self {
            page: 0,
            selected_track: 0,
            selected_step: 0,
            clear_armed: None,
            sent: None,
            held_note: None,
            fader_bank: 0,
            fader_pickups: [FaderPickup::default(); 9],
        }
    }
    fn apply(
        &mut self,
        input: Input,
        sequence: &SequencerControl,
        now: Instant,
    ) -> Result<(), String> {
        match input {
            Input::Grid(cell, true) => {
                let track = 7 - usize::from(cell / 8);
                let column = usize::from(cell % 8);
                self.selected_track = track;
                self.selected_step = self.page * 8 + column;
                sequence.toggle(track, self.page * 8 + column)
            }
            Input::Grid(_, false) => Ok(()),
            Input::Button(PAGE_A_NOTE) => {
                self.page = 0;
                self.selected_step %= 8;
                Ok(())
            }
            Input::Button(PAGE_B_NOTE) => {
                self.page = 1;
                self.selected_step = 8 + self.selected_step % 8;
                Ok(())
            }
            Input::Button(UP_NOTE) => {
                self.selected_track = self.selected_track.saturating_sub(1);
                Ok(())
            }
            Input::Button(DOWN_NOTE) => {
                self.selected_track = (self.selected_track + 1).min(7);
                Ok(())
            }
            Input::Button(LEFT_NOTE) => {
                self.selected_step = self.selected_step.saturating_sub(1);
                self.page = self.selected_step / 8;
                Ok(())
            }
            Input::Button(RIGHT_NOTE) => {
                self.selected_step = (self.selected_step + 1).min(sequence.steps - 1);
                self.page = self.selected_step / 8;
                Ok(())
            }
            Input::Button(PLAY_STOP_NOTE) => {
                sequence.toggle_transport();
                Ok(())
            }
            Input::Button(RESTART_NOTE) => {
                sequence.restart();
                Ok(())
            }
            Input::Button(CLEAR_NOTE) => {
                if self
                    .clear_armed
                    .is_some_and(|armed| now.duration_since(armed) <= Duration::from_millis(1500))
                {
                    self.clear_armed = None;
                    sequence.clear_track(self.selected_track)
                } else {
                    self.clear_armed = Some(now);
                    Ok(())
                }
            }
            Input::Button(_) => Ok(()),
            Input::Fader(_, _) => Ok(()),
        }
    }
}

fn names(input: &MidiInput) -> Vec<String> {
    input
        .ports()
        .iter()
        .map(|p| input.port_name(p).unwrap_or_default())
        .collect()
}
fn output_names(output: &MidiOutput) -> Vec<String> {
    output
        .ports()
        .iter()
        .map(|p| output.port_name(p).unwrap_or_default())
        .collect()
}

fn connect(
    name: &str,
    tx: SyncSender<Input>,
) -> Result<(midir::MidiInputConnection<()>, MidiOutputConnection), String> {
    let mut input = MidiInput::new("meldritch2-apc-input").map_err(|e| e.to_string())?;
    input.ignore(Ignore::None);
    let output = MidiOutput::new("meldritch2-apc-output").map_err(|e| e.to_string())?;
    let input_index = unique_name_match(&names(&input), name, "APC input")?;
    let output_index = unique_name_match(&output_names(&output), name, "APC output")?;
    let input_port = input
        .ports()
        .get(input_index)
        .cloned()
        .ok_or("APC input disappeared")?;
    let output_port = output
        .ports()
        .get(output_index)
        .cloned()
        .ok_or("APC output disappeared")?;
    let connection = input
        .connect(
            &input_port,
            "meldritch2-apc",
            move |_, bytes, ()| {
                if let Some(event) = decode(bytes) {
                    let _ = tx.try_send(event);
                }
            },
            (),
        )
        .map_err(|e| e.to_string())?;
    let output = output
        .connect(&output_port, "meldritch2-apc-leds")
        .map_err(|e| e.to_string())?;
    Ok((connection, output))
}

fn led_frame(sequence: &SequencerControl, state: &GridController) -> ([[u8; 3]; 64], [bool; 16]) {
    let pattern = sequence.pattern();
    let first = state.page * 8;
    let playing = sequence.playing();
    let current = sequence.current_step();
    let mut leds = [[0u8; 3]; 64];
    for track in 0..8 {
        for column in 0..8 {
            let step = first + column;
            let active = pattern.velocities[track][step] > 0.0;
            let selected = state.selected_track == track && state.selected_step == step;
            let rgb = if playing && current == step && selected {
                if active {
                    [127, 127, 127]
                } else {
                    [0, 90, 127]
                }
            } else if playing && current == step {
                if active { [127, 127, 0] } else { [0, 30, 127] }
            } else if selected {
                if active { [127, 30, 127] } else { [30, 0, 60] }
            } else if active {
                [0, 127, 0]
            } else {
                [2, 2, 2]
            };
            let cell = ((7 - track) * 8 + column) as u8;
            leds[usize::from(cell)] = rgb;
        }
    }
    let mut buttons = [false; 16];
    buttons[0] = state.page == 0;
    buttons[1] = state.page == 1;
    buttons[4] = state.selected_track > 0;
    buttons[5] = state.selected_track < 7;
    buttons[6] = state.selected_step > 0;
    buttons[7] = state.selected_step + 1 < sequence.steps;
    buttons[8] = playing;
    buttons[10] = state.clear_armed.is_some();
    (leds, buttons)
}
fn authored_led_frame(
    sequence: &SequencerControl,
    state: &GridController,
    mapping: &crate::model::ApcMapping,
) -> ([[u8; 3]; 64], [bool; 16]) {
    if let Some(roll) = &mapping.piano_roll {
        return piano_roll_led_frame(sequence, state, roll, mapping);
    }
    if mapping.pages[state.page] == "steps" {
        let mut view = GridController::new();
        view.page = sequence.step_bank();
        view.selected_track = sequence.selected_track();
        view.selected_step = sequence.selected_step();
        return led_frame(sequence, &view);
    }
    let mut leds = [[2u8; 3]; 64];
    for row in 0..8 {
        let role = &mapping.performance_rows[row];
        for column in 0..8 {
            let rgb = match role.as_str() {
                "pattern" => {
                    if sequence.active_pattern() == column {
                        [0, 127, 0]
                    } else if sequence.queued_pattern() == Some(column) {
                        [127, 90, 0]
                    } else {
                        [2, 2, 2]
                    }
                }
                "scene" => {
                    if let Some(pattern) = mapping.scene_patterns.get(column) {
                        if sequence.active_pattern() == *pattern {
                            [0, 127, 0]
                        } else if sequence.queued_pattern() == Some(*pattern) {
                            [127, 90, 0]
                        } else {
                            [2, 2, 2]
                        }
                    } else {
                        [0, 0, 0]
                    }
                }
                "clip" => {
                    let track = sequence.selected_track();
                    if let Some(pattern) = sequence.clip_choices(track).get(column) {
                        if sequence.active_clip(track) == Some(*pattern) {
                            [0, 100, 127]
                        } else if sequence.queued_clip(track) == Some(*pattern) {
                            [127, 90, 0]
                        } else {
                            [1, 3, 4]
                        }
                    } else {
                        [0, 0, 0]
                    }
                }
                "mute" => {
                    if sequence.muted(column) {
                        [127, 0, 0]
                    } else {
                        [3, 1, 1]
                    }
                }
                "solo" => {
                    if sequence.soloed(column) {
                        [127, 127, 0]
                    } else {
                        [2, 2, 0]
                    }
                }
                "track" => {
                    if sequence.selected_track() == column {
                        [0, 80, 127]
                    } else {
                        [1, 2, 3]
                    }
                }
                "restart" => [40, 0, 80],
                _ => [0, 0, 0],
            };
            leds[(7 - row) * 8 + column] = rgb;
        }
    }
    (leds, {
        let mut buttons = [false; 16];
        buttons[0] = state.page == 0;
        buttons[1] = state.page == 1;
        buttons[4..8].fill(true);
        buttons[8] = sequence.playing();
        buttons
    })
}

fn midi_hz(note: u8) -> f32 {
    440.0 * 2.0_f32.powf((f32::from(note) - 69.0) / 12.0)
}

fn piano_note(sequence: &SequencerControl, roll: &crate::model::ApcPianoRoll, row: usize) -> u8 {
    let degree = roll.scale[7 - row];
    sequence.key_root().saturating_add(degree).min(127)
}

fn piano_roll_led_frame(
    sequence: &SequencerControl,
    state: &GridController,
    roll: &crate::model::ApcPianoRoll,
    mapping: &crate::model::ApcMapping,
) -> ([[u8; 3]; 64], [bool; 16]) {
    let pattern = sequence.pattern();
    let zoom = sequence.step_zoom();
    let first = sequence.step_bank() * 8;
    let mut leds = [[[2, 2, 2]; 8]; 8];
    for row in 0..3 {
        let track = roll.drum_tracks[row];
        let length = usize::from(pattern.lengths[track]).max(1);
        for column in 0..8 {
            let start = first + column * zoom;
            let end = start + zoom;
            let hits = (start..end)
                .filter(|step| pattern.velocities[track][*step % length] > 0.0)
                .count();
            let base = if hits > 1 {
                LOSPEC500[15]
            } else if hits == 1 {
                LOSPEC500[[4, 8, 20][row]]
            } else {
                LOSPEC500[0]
            };
            leds[row][column] = if sequence.playing()
                && sequence.current_step() >= start
                && sequence.current_step() < end
            {
                if hits == 0 {
                    LOSPEC500[26]
                } else {
                    LOSPEC500[41]
                }
            } else {
                base
            };
        }
    }
    let track = roll.melodic_track;
    let length = usize::from(pattern.lengths[track]);
    for row in 3..8 {
        let note = piano_note(sequence, roll, row);
        let pitch = midi_hz(note);
        let base = LOSPEC500[crate::performance_ui::pitch_color(f64::from(pitch))];
        for column in 0..8 {
            let start = first + column * zoom;
            let end = (start + zoom).min(length);
            let mut events = 0;
            let mut full = false;
            for event_step in 0..length {
                if pattern.velocities[track][event_step] <= 0.0
                    || (69.0 + 12.0 * (pattern.pitches_hz[track][event_step] / 440.0).log2())
                        .round() as u8
                        != note
                {
                    continue;
                }
                let event_end = event_step as f32 + pattern.gates[track][event_step].max(0.01);
                if event_step < end && event_end > start as f32 {
                    events += 1;
                    full |= event_step <= start && event_end >= end as f32;
                }
            }
            let base = if events > 1 {
                LOSPEC500[36]
            } else if full {
                base
            } else if events == 1 {
                [base[0] / 3, base[1] / 3, base[2] / 3]
            } else {
                LOSPEC500[0]
            };
            leds[row][column] = if sequence.playing()
                && sequence.current_step() >= start
                && sequence.current_step() < end
            {
                if events == 0 {
                    LOSPEC500[26]
                } else {
                    LOSPEC500[41]
                }
            } else {
                base
            };
        }
    }
    if let Some((cell, _, _)) = state.held_note {
        let row = 7 - usize::from(cell / 8);
        let column = usize::from(cell % 8);
        leds[row][column] = LOSPEC500[14];
    }
    let mut physical = [[0; 3]; 64];
    for (row, cells) in leds.iter().enumerate() {
        for (column, color) in cells.iter().enumerate() {
            physical[(7 - row) * 8 + column] = *color;
        }
    }
    let active = |command: &str| {
        if command == "transport" {
            sequence.playing()
        } else if let Some(bank) = command.strip_prefix("faders:bank:") {
            bank.parse::<usize>().ok() == Some(state.fader_bank)
        } else {
            false
        }
    };
    let mut buttons = [false; 16];
    for (index, command) in mapping.track_button_commands.iter().enumerate() {
        buttons[index] =
            active(command) || (!command.is_empty() && !command.starts_with("faders:bank:"));
    }
    for (index, command) in mapping.scene_button_commands.iter().enumerate() {
        buttons[8 + index] =
            active(command) || (!command.is_empty() && !command.starts_with("faders:bank:"));
    }
    (physical, buttons)
}

fn apply_piano_grid(
    state: &mut GridController,
    cell: u8,
    pressed: bool,
    sequence: &SequencerControl,
    roll: &crate::model::ApcPianoRoll,
) -> Result<(), String> {
    let row = 7 - usize::from(cell / 8);
    let column = usize::from(cell % 8);
    let zoom = sequence.step_zoom();
    let step = sequence.step_bank() * 8 + column * zoom;
    if row < 3 {
        if pressed {
            return sequence.toggle(roll.drum_tracks[row], step);
        }
        return Ok(());
    }
    let track = roll.melodic_track;
    let length = usize::from(sequence.pattern().lengths[track]);
    if step >= length {
        return Ok(());
    }
    if !pressed {
        if state.held_note.is_some_and(|(held, _, _)| held == cell) {
            let (_, start, pitch) = state.held_note.take().expect("held note");
            return sequence.set_note(track, start, pitch, 0.85, 1.0);
        }
        return Ok(());
    }
    let pitch = midi_hz(piano_note(sequence, roll, row));
    let pattern = sequence.pattern();
    if pattern.velocities[track][step] > 0.0
        && (pattern.pitches_hz[track][step] - pitch).abs() < 0.01
    {
        state.held_note = None;
        return sequence.set_note(track, step, pitch, 0.0, 1.0);
    }
    if let Some((_, start, held_pitch)) = state.held_note.take() {
        if step > start {
            let duration = (step + zoom).min(length) - start;
            return sequence.set_note(track, start, held_pitch, 0.85, duration as f32);
        }
        sequence.set_note(track, start, held_pitch, 0.85, 1.0)?;
    }
    state.held_note = Some((cell, step, pitch));
    Ok(())
}
fn refresh(
    output: &mut MidiOutputConnection,
    sequence: &SequencerControl,
    state: &mut GridController,
    mapping: Option<&crate::model::ApcMapping>,
) -> Result<(), String> {
    if let Some(mapping) = mapping {
        sync_fader_bank(state, mapping);
    }
    let (leds, buttons) = mapping.map_or_else(
        || led_frame(sequence, state),
        |mapping| authored_led_frame(sequence, state, mapping),
    );
    // Send complete 16-pad quadrants whenever any pad in that quadrant
    // changes.  The APC can lose individual RGB messages during playhead
    // animation, leaving old colors behind even though our diff cache thinks
    // they arrived.  A bounded quadrant refresh is both substantially fewer
    // MIDI messages and self-healing for every pad beside the changed one.
    let dirty = dirty_rgb_chunks(state.sent.as_ref().map(|sent| &sent.0), &leds);
    for (chunk, colors) in leds.chunks(16).enumerate() {
        let start = chunk * 16;
        if dirty[chunk] {
            output
                .send(&apc_rgb_chunk(start as u8, colors)?)
                .map_err(|e| e.to_string())?;
            if dirty[chunk + 1..].contains(&true) {
                thread::sleep(Duration::from_millis(5));
            }
        }
    }
    for (index, note) in BUTTON_NOTES.into_iter().enumerate() {
        if state
            .sent
            .as_ref()
            .is_none_or(|sent| sent.1[index] != buttons[index])
        {
            output
                .send(&[0x90, note, if buttons[index] { 127 } else { 0 }])
                .map_err(|e| e.to_string())?;
        }
    }
    state.sent = Some((leds, buttons));
    Ok(())
}

fn sync_fader_bank(state: &mut GridController, mapping: &crate::model::ApcMapping) {
    let bank = mapping
        .fader_bank
        .load(Ordering::Acquire)
        .min(mapping.fader_banks.len().saturating_sub(1));
    if state.fader_bank != bank {
        state.fader_bank = bank;
        state.fader_pickups[..8].fill(FaderPickup::default());
    }
}

fn apply_fader_bank_command(
    state: &mut GridController,
    mapping: &crate::model::ApcMapping,
    command: &str,
) -> Result<bool, String> {
    let next = if let Some(bank) = command.strip_prefix("faders:bank:") {
        Some(
            bank.parse::<usize>()
                .map_err(|_| "invalid APC fader bank")?,
        )
    } else if command == "faders:next" {
        Some((state.fader_bank + 1) % mapping.fader_banks.len())
    } else if command == "faders:previous" {
        Some((state.fader_bank + mapping.fader_banks.len() - 1) % mapping.fader_banks.len())
    } else {
        None
    };
    let Some(next) = next else { return Ok(false) };
    if next >= mapping.fader_banks.len() {
        return Err(format!("APC fader bank {next} is out of range"));
    }
    mapping.fader_bank.store(next, Ordering::Release);
    sync_fader_bank(state, mapping);
    state.sent = None;
    Ok(true)
}

fn shutdown_color(cell: usize, frame: usize) -> [u8; 3] {
    let row = cell / 8;
    let column = cell % 8;
    LOSPEC500[(row + column + frame) % LOSPEC500.len()]
}

fn shutdown_animation(output: &mut MidiOutputConnection) -> Result<(), String> {
    const FRAME_TIME: Duration = Duration::from_millis(75);
    let mut first_error = None;
    for frame in 0..20 {
        let mut colors = [[0; 3]; 64];
        for (cell, color) in colors.iter_mut().enumerate() {
            let checker = (cell / 8 + cell % 8 + frame) % 4;
            *color = if checker < 2 {
                shutdown_color(cell, frame * 2)
            } else {
                shutdown_color(cell, frame * 2 + 12)
            };
        }
        // Keep each SysEx transfer comfortably below the device/USB packet
        // boundary.  The controller truncates a 64-record message after the
        // first half even though the protocol's length field can represent it.
        for (chunk, colors) in colors.chunks(16).enumerate() {
            let message = apc_rgb_chunk((chunk * 16) as u8, colors).expect("valid APC grid chunk");
            if let Err(error) = output.send(&message) {
                first_error.get_or_insert_with(|| error.to_string());
            }
            // The XL grid accepts all four quadrants reliably when bulk writes
            // are paced; back-to-back USB/MIDI transfers can starve the upper
            // half while the lower packets are still being applied.
            if chunk != 3 {
                thread::sleep(Duration::from_millis(5));
            }
        }
        thread::sleep(FRAME_TIME);
    }
    // Repeat the terminal frame after a short settling interval.  This gives
    // the controller time to consume the animation before its definitive
    // all-black state without extending shutdown noticeably.
    for pass in 0..2 {
        if let Err(error) =
            output.send(&apc_rgb_range(0, 63, [0, 0, 0]).expect("valid full APC grid clear"))
        {
            first_error.get_or_insert_with(|| error.to_string());
        }
        thread::sleep(Duration::from_millis(20));
        // The protocol defines exactly eight track and eight scene-launch LEDs.
        for note in (100..=107).chain(112..=119) {
            if let Err(error) = output.send(&[0x90, note, 0]) {
                first_error.get_or_insert_with(|| error.to_string());
            }
            if let Err(error) = output.send(&[0x80, note, 0]) {
                first_error.get_or_insert_with(|| error.to_string());
            }
        }
        if pass == 0 {
            thread::sleep(Duration::from_millis(75));
        }
    }
    first_error.map_or(Ok(()), |error| {
        Err(format!("APC shutdown animation failed: {error}"))
    })
}

pub struct ApcSession {
    stop: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<Result<(), String>>>,
}
impl ApcSession {
    pub fn start(
        name: String,
        performance: Performance,
        controls: Vec<(ControlHandle, f64)>,
    ) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = Arc::clone(&stop);
        let thread = thread::spawn(move || {
            let mut state = GridController::new();
            let sequence = Arc::clone(&performance.sequencer);
            let handles = controls
                .into_iter()
                .map(|(handle, value)| (handle.id().to_owned(), (handle, value)))
                .collect::<BTreeMap<_, _>>();
            while !worker_stop.load(Ordering::Acquire) {
                let (tx, rx): (SyncSender<Input>, Receiver<Input>) = mpsc::sync_channel(128);
                let Ok((_input, mut output)) = connect(&name, tx) else {
                    thread::sleep(Duration::from_secs(1));
                    continue;
                };
                state.sent = None;
                if refresh(&mut output, &sequence, &mut state, performance.apc.as_ref()).is_err() {
                    continue;
                }
                while !worker_stop.load(Ordering::Acquire) {
                    while let Ok(event) = rx.try_recv() {
                        let result = if let Some(mapping) = &performance.apc {
                            apply_authored(&mut state, event, &sequence, mapping, &handles)
                        } else {
                            state.apply(event, &sequence, Instant::now())
                        };
                        if let Err(error) = result {
                            eprintln!("APC performance mapping error: {error}");
                        }
                    }
                    if state
                        .clear_armed
                        .is_some_and(|at| at.elapsed() > Duration::from_millis(1500))
                    {
                        state.clear_armed = None;
                    }
                    if refresh(&mut output, &sequence, &mut state, performance.apc.as_ref())
                        .is_err()
                    {
                        break;
                    }
                    thread::sleep(Duration::from_millis(25));
                }
                if worker_stop.load(Ordering::Acquire) {
                    return shutdown_animation(&mut output);
                }
            }
            Ok(())
        });
        Self {
            stop,
            thread: Some(thread),
        }
    }

    pub fn begin_shutdown(&self) {
        self.stop.store(true, Ordering::Release);
    }

    pub fn finish(&mut self) -> Result<(), String> {
        self.begin_shutdown();
        match self.thread.take() {
            Some(thread) => thread
                .join()
                .map_err(|_| "APC worker panicked".to_owned())?,
            None => Ok(()),
        }
    }
}

fn publish_fader(
    handle: &ControlHandle,
    _authored_value: f64,
    value: u8,
    pickup: &mut FaderPickup,
) -> Result<(), String> {
    let (min, max) = handle.range();
    let normalized = f64::from(value) / 127.0;
    let current_value = handle.effective_value();
    let current = if handle.scale() == crate::model::ControlScale::Logarithmic {
        (current_value / min).ln() / (max / min).ln()
    } else {
        (current_value - min) / (max - min)
    }
    .clamp(0.0, 1.0);
    let crossed = pickup
        .previous
        .is_some_and(|previous| (previous - current) * (normalized - current) <= 0.0);
    pickup.previous = Some(normalized);
    if !pickup.captured && !crossed && (normalized - current).abs() > 0.025 {
        handle.set_hardware_pickup(if normalized < current { -1 } else { 1 });
        return Ok(());
    }
    pickup.captured = true;
    handle.set_hardware_pickup(0);
    let target = if handle.scale() == crate::model::ControlScale::Logarithmic {
        min * (max / min).powf(normalized)
    } else {
        min + (max - min) * normalized
    };
    handle.publish(target)
}

fn apply_authored(
    state: &mut GridController,
    input: Input,
    sequence: &SequencerControl,
    mapping: &crate::model::ApcMapping,
    handles: &BTreeMap<String, (ControlHandle, f64)>,
) -> Result<(), String> {
    if let Some(roll) = &mapping.piano_roll {
        match input {
            Input::Grid(cell, pressed) => {
                return apply_piano_grid(state, cell, pressed, sequence, roll);
            }
            Input::Button(note) => {
                let command = if (100..=107).contains(&note) {
                    &mapping.track_button_commands[usize::from(note - 100)]
                } else if (112..=119).contains(&note) {
                    &mapping.scene_button_commands[usize::from(note - 112)]
                } else {
                    ""
                };
                return if command.is_empty() || apply_fader_bank_command(state, mapping, command)? {
                    Ok(())
                } else {
                    sequence.execute(command)
                };
            }
            Input::Fader(_, _) => {}
        }
    }
    match input {
        Input::Button(PAGE_A_NOTE) => {
            state.page = 0;
            Ok(())
        }
        Input::Button(PAGE_B_NOTE) => {
            state.page = 1;
            Ok(())
        }
        Input::Button(LEFT_NOTE) => sequence.set_step_bank(sequence.step_bank().saturating_sub(1)),
        Input::Button(RIGHT_NOTE) => {
            sequence.set_step_bank((sequence.step_bank() + 1).min((sequence.steps - 1) / 8))
        }
        Input::Button(PLAY_STOP_NOTE) => sequence.execute("transport"),
        Input::Button(RESTART_NOTE) => sequence.execute("restart"),
        Input::Button(CLEAR_NOTE) => sequence.execute("clear"),
        Input::Grid(cell, true) => {
            let track = 7 - usize::from(cell / 8);
            let column = usize::from(cell % 8);
            let page = &mapping.pages[state.page];
            if page == "steps" {
                sequence.select_track(track)?;
                sequence.execute(&format!("toggle:{}", column + 1))
            } else {
                let role = &mapping.performance_rows[track];
                if role.is_empty() {
                    Ok(())
                } else if role == "restart" {
                    sequence.execute("restart")
                } else if role == "scene" {
                    if let Some(pattern) = mapping.scene_patterns.get(column) {
                        sequence.queue_scene(*pattern, mapping.scene_quantization)
                    } else {
                        Ok(())
                    }
                } else if role == "clip" {
                    let track = sequence.selected_track();
                    if let Some(pattern) = sequence.clip_choices(track).get(column) {
                        sequence.queue_clip(track, *pattern, mapping.scene_quantization)
                    } else {
                        Ok(())
                    }
                } else if (role == "pattern" && column >= sequence.pattern_names().len())
                    || matches!(role.as_str(), "mute" | "solo" | "track")
                        && column >= sequence.track_names().len()
                {
                    Ok(())
                } else {
                    sequence.execute(&format!("{role}:{column}"))
                }
            }
        }
        Input::Grid(_, false) => Ok(()),
        Input::Fader(index, value) => {
            let id = if index == 8 {
                &mapping.master_control
            } else {
                &mapping.fader_banks[state.fader_bank][usize::from(index)]
            };
            let (handle, authored_value) = handles
                .get(id)
                .ok_or_else(|| format!("APC fader targets unknown control '{id}'"))?;
            publish_fader(
                handle,
                *authored_value,
                value,
                &mut state.fader_pickups[usize::from(index)],
            )
        }
        Input::Button(_) => Ok(()),
    }
}
impl Drop for ApcSession {
    fn drop(&mut self) {
        let _ = self.finish();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ApcMapping;

    fn authored_mapping(control: &str) -> ApcMapping {
        let faders = std::array::from_fn(|_| control.into());
        ApcMapping {
            pages: ["steps".into(), "performance".into()],
            performance_rows: [
                "pattern".into(),
                "mute".into(),
                "solo".into(),
                "track".into(),
                "restart".into(),
                String::new(),
                String::new(),
                String::new(),
            ],
            scene_patterns: Vec::new(),
            scene_quantization: 16,
            fader_controls: faders.clone(),
            fader_banks: vec![faders],
            fader_bank: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            master_control: control.into(),
            piano_roll: None,
            track_button_commands: std::array::from_fn(|_| String::new()),
            scene_button_commands: std::array::from_fn(|_| String::new()),
        }
    }

    #[test]
    fn grid_pages_toggle_correct_halves_and_clear_is_guarded() {
        let sequence = SequencerControl::new("test".into(), 120.0, 16, 4).unwrap();
        for n in 0..8 {
            sequence.add_track(format!("T{n}")).unwrap();
        }
        let mut grid = GridController::new();
        let now = Instant::now();
        grid.apply(Input::Grid(56, true), &sequence, now).unwrap();
        assert_eq!(sequence.pattern().velocities[0][0], 1.0);
        grid.apply(Input::Button(PAGE_B_NOTE), &sequence, now)
            .unwrap();
        grid.apply(Input::Grid(56, true), &sequence, now).unwrap();
        assert_eq!(sequence.pattern().velocities[0][8], 1.0);
        grid.apply(Input::Button(CLEAR_NOTE), &sequence, now)
            .unwrap();
        assert_eq!(sequence.pattern().velocities[0][8], 1.0);
        grid.apply(
            Input::Button(CLEAR_NOTE),
            &sequence,
            now + Duration::from_millis(10),
        )
        .unwrap();
        assert_eq!(sequence.pattern().velocities[0][8], 0.0);
    }
    #[test]
    fn rgb_message_matches_akai_protocol_shape() {
        let m = apc_rgb(63, [255, 128, 1]).unwrap();
        assert_eq!(&m[..5], &[0xf0, 0x47, 0x7f, 0x4f, 0x24]);
        assert_eq!(m[15], 0xf7);
    }

    #[test]
    fn bounded_rgb_chunk_and_range_clear_follow_akai_protocol() {
        let colors =
            std::array::from_fn::<_, 16, _>(|offset| [offset as u8, 255 - offset as u8, 17]);
        let message = apc_rgb_chunk(32, &colors).unwrap();
        assert_eq!(&message[..7], &[0xf0, 0x47, 0x7f, 0x4f, 0x24, 1, 0]);
        assert_eq!(message.len(), 136);
        assert_eq!(&message[7..15], &[32, 32, 0, 0, 1, 127, 0, 17]);
        assert_eq!(&message[127..135], &[47, 47, 0, 15, 1, 112, 0, 17]);
        assert_eq!(message[135], 0xf7);

        let clear = apc_rgb_range(0, 63, [0, 0, 0]).unwrap();
        assert_eq!(&clear[5..9], &[0, 8, 0, 63]);
    }

    #[test]
    fn rgb_diff_resends_the_complete_quadrant_containing_a_changed_pad() {
        let before = [[2, 2, 2]; 64];
        let mut after = before;
        after[31] = [127, 0, 0];
        assert_eq!(
            dirty_rgb_chunks(Some(&before), &after),
            [false, true, false, false]
        );
        assert_eq!(dirty_rgb_chunks(None, &after), [true; 4]);
    }
    #[test]
    fn arrow_buttons_navigate_tracks_and_all_sixteen_steps() {
        let sequence = SequencerControl::new("test".into(), 120.0, 16, 4).unwrap();
        for n in 0..8 {
            sequence.add_track(format!("T{n}")).unwrap();
        }
        let mut grid = GridController::new();
        let now = Instant::now();
        grid.apply(Input::Button(DOWN_NOTE), &sequence, now)
            .unwrap();
        for _ in 0..8 {
            grid.apply(Input::Button(RIGHT_NOTE), &sequence, now)
                .unwrap();
        }
        assert_eq!(grid.selected_track, 1);
        assert_eq!(grid.selected_step, 8);
        assert_eq!(grid.page, 1);
        grid.apply(Input::Button(LEFT_NOTE), &sequence, now)
            .unwrap();
        assert_eq!(grid.selected_step, 7);
        assert_eq!(grid.page, 0);
        grid.apply(Input::Button(UP_NOTE), &sequence, now).unwrap();
        assert_eq!(grid.selected_track, 0);
    }
    #[test]
    fn external_edits_and_playhead_are_reflected_in_led_frame() {
        let sequence = SequencerControl::new("test".into(), 120.0, 16, 4).unwrap();
        for n in 0..8 {
            sequence.add_track(format!("T{n}")).unwrap();
        }
        sequence.toggle(0, 0).unwrap();
        let mut runtime = crate::sequencer::SequencerRuntime::new(Arc::clone(&sequence));
        runtime.begin_frame(48_000.0);
        let state = GridController::new();
        let (leds, buttons) = led_frame(&sequence, &state);
        assert_eq!(leds[56], [127, 127, 127]);
        assert!(buttons[0]);
        assert!(buttons[8]);
    }

    #[test]
    fn authored_pages_share_step_pattern_mute_solo_and_selection_commands() {
        let sequence = SequencerControl::new("test".into(), 120.0, 16, 4).unwrap();
        for n in 0..8 {
            sequence.add_track(format!("T{n}")).unwrap();
        }
        sequence.define_pattern("Foundation".into()).unwrap();
        sequence.define_pattern("Broken".into()).unwrap();
        let mapping = authored_mapping("level");
        let handles = BTreeMap::new();
        let mut state = GridController::new();

        apply_authored(
            &mut state,
            Input::Grid(56, true),
            &sequence,
            &mapping,
            &handles,
        )
        .unwrap();
        assert_eq!(sequence.pattern().velocities[0][0], 1.0);
        apply_authored(
            &mut state,
            Input::Button(RIGHT_NOTE),
            &sequence,
            &mapping,
            &handles,
        )
        .unwrap();
        apply_authored(
            &mut state,
            Input::Grid(57, true),
            &sequence,
            &mapping,
            &handles,
        )
        .unwrap();
        assert_eq!(sequence.pattern().velocities[0][9], 1.0);

        apply_authored(
            &mut state,
            Input::Button(PAGE_B_NOTE),
            &sequence,
            &mapping,
            &handles,
        )
        .unwrap();
        apply_authored(
            &mut state,
            Input::Grid(57, true),
            &sequence,
            &mapping,
            &handles,
        )
        .unwrap();
        assert_eq!(sequence.queued_pattern(), Some(1));
        apply_authored(
            &mut state,
            Input::Grid(48, true),
            &sequence,
            &mapping,
            &handles,
        )
        .unwrap();
        assert!(sequence.muted(0));
        apply_authored(
            &mut state,
            Input::Grid(40, true),
            &sequence,
            &mapping,
            &handles,
        )
        .unwrap();
        assert!(sequence.soloed(0));
        apply_authored(
            &mut state,
            Input::Grid(33, true),
            &sequence,
            &mapping,
            &handles,
        )
        .unwrap();
        assert_eq!(sequence.selected_track(), 1);

        let (leds, _) = authored_led_frame(&sequence, &state, &mapping);
        assert_eq!(leds[57], [127, 90, 0]);
        assert_eq!(leds[48], [127, 0, 0]);
        assert_eq!(leds[40], [127, 127, 0]);
        assert_eq!(leds[33], [0, 80, 127]);
    }

    #[test]
    fn authored_scene_row_ignores_clip_storage_and_shows_pending_then_active() {
        let sequence = SequencerControl::new("scenes".into(), 120.0, 32, 4).unwrap();
        sequence.add_track("T".into()).unwrap();
        sequence.define_pattern("clip:hidden".into()).unwrap();
        let first = sequence.define_pattern("First".into()).unwrap();
        let second = sequence.define_pattern("Second".into()).unwrap();
        let mut mapping = authored_mapping("level");
        mapping.performance_rows[0] = "scene".into();
        mapping.scene_patterns = vec![first, second];
        mapping.scene_quantization = 4;
        let handles = BTreeMap::new();
        let mut state = GridController::new();
        apply_authored(
            &mut state,
            Input::Button(PAGE_B_NOTE),
            &sequence,
            &mapping,
            &handles,
        )
        .unwrap();
        apply_authored(
            &mut state,
            Input::Grid(57, true),
            &sequence,
            &mapping,
            &handles,
        )
        .unwrap();
        assert_eq!(sequence.queued_pattern(), Some(second));
        assert_eq!(
            authored_led_frame(&sequence, &state, &mapping).0[57],
            [127, 90, 0]
        );
        let mut runtime = crate::sequencer::SequencerRuntime::new(Arc::clone(&sequence));
        runtime.begin_frame(8.0);
        assert_eq!(sequence.active_pattern(), second);
        assert_eq!(
            authored_led_frame(&sequence, &state, &mapping).0[57],
            [0, 127, 0]
        );
    }

    #[test]
    fn authored_keylocked_piano_roll_edits_drums_notes_durations_and_key() {
        let sequence = SequencerControl::new("lab".into(), 120.0, 64, 4).unwrap();
        for name in ["Kick", "Snare", "Hat", "Bass"] {
            sequence.add_track(name.into()).unwrap();
        }
        sequence.select_track(3).unwrap();
        sequence.set_key_root(36).unwrap();
        sequence.set_key_track(3).unwrap();
        let mut mapping = authored_mapping("level");
        mapping.piano_roll = Some(crate::model::ApcPianoRoll {
            drum_tracks: [0, 1, 2],
            melodic_track: 3,
            scale: [0, 2, 3, 5, 7],
        });
        mapping.track_button_commands[5] = "key:up".into();
        let mut state = GridController::new();
        let handles = BTreeMap::new();

        apply_authored(
            &mut state,
            Input::Grid(56, true),
            &sequence,
            &mapping,
            &handles,
        )
        .unwrap();
        assert_eq!(sequence.pattern().velocities[0][0], 1.0);

        apply_authored(
            &mut state,
            Input::Grid(1, true),
            &sequence,
            &mapping,
            &handles,
        )
        .unwrap();
        apply_authored(
            &mut state,
            Input::Grid(3, true),
            &sequence,
            &mapping,
            &handles,
        )
        .unwrap();
        let pattern = sequence.pattern();
        assert!((pattern.pitches_hz[3][1] - midi_hz(36)).abs() < 0.01);
        assert_eq!(pattern.gates[3][1], 3.0);

        apply_authored(
            &mut state,
            Input::Button(105),
            &sequence,
            &mapping,
            &handles,
        )
        .unwrap();
        assert_eq!(sequence.key_root(), 37);
        assert!((sequence.pattern().pitches_hz[3][1] - midi_hz(37)).abs() < 0.01);
    }

    #[test]
    fn production_delay_lab_apc_grid_edits_its_authored_pattern() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../examples/experiments/02_delay_feedback.lua");
        let script = std::fs::read_to_string(&path).unwrap();
        let graph = crate::lua::read_with_root(&script, path.parent().unwrap()).unwrap();
        let performance = graph.performance.as_ref().unwrap();
        let mapping = performance.apc.as_ref().unwrap();
        let roll = mapping.piano_roll.as_ref().unwrap();
        let sequence = &performance.sequencer;
        let mut state = GridController::new();
        let handles = BTreeMap::new();

        assert_eq!(sequence.pattern().velocities[roll.drum_tracks[0]][1], 0.0);
        apply_authored(
            &mut state,
            Input::Grid(57, true),
            sequence,
            mapping,
            &handles,
        )
        .unwrap();
        assert_eq!(sequence.pattern().velocities[roll.drum_tracks[0]][1], 1.0);

        let bass_cell = 2;
        assert_eq!(sequence.pattern().velocities[roll.melodic_track][2], 0.0);
        apply_authored(
            &mut state,
            Input::Grid(bass_cell, true),
            sequence,
            mapping,
            &handles,
        )
        .unwrap();
        apply_authored(
            &mut state,
            Input::Grid(bass_cell, false),
            sequence,
            mapping,
            &handles,
        )
        .unwrap();
        let pattern = sequence.pattern();
        assert_eq!(pattern.velocities[roll.melodic_track][2], 0.85);
        assert!((pattern.pitches_hz[roll.melodic_track][2] - midi_hz(36)).abs() < 0.01);
        assert_eq!(pattern.gates[roll.melodic_track][2], 1.0);
        let (leds, _) = piano_roll_led_frame(sequence, &state, roll, mapping);
        assert_eq!(
            leds[usize::from(bass_cell)],
            LOSPEC500[crate::performance_ui::pitch_color(f64::from(midi_hz(36)))]
        );

        let mut runtime = crate::sequencer::SequencerRuntime::new(Arc::clone(sequence));
        for _ in 0..100 {
            runtime.begin_frame(8.0);
            if sequence.current_step() == 16 {
                break;
            }
        }
        assert_eq!(sequence.current_step(), 16);
        sequence.set_step_bank(2).unwrap();
        let (wrapped, _) = piano_roll_led_frame(sequence, &state, roll, mapping);
        assert_eq!(
            wrapped[56], LOSPEC500[41],
            "the 16-step kick row must show its wrapped playhead during bass step 17"
        );
    }

    #[test]
    fn zoomed_piano_roll_distinguishes_full_partial_and_multiple_notes() {
        let sequence = SequencerControl::new("lab".into(), 120.0, 64, 4).unwrap();
        for name in ["Kick", "Snare", "Hat", "Bass"] {
            sequence.add_track(name.into()).unwrap();
        }
        sequence.select_track(3).unwrap();
        sequence.set_key_root(36).unwrap();
        sequence.set_key_track(3).unwrap();
        sequence.stop();
        let mut runtime = crate::sequencer::SequencerRuntime::new(Arc::clone(&sequence));
        runtime.begin_frame(48_000.0);
        sequence.execute("zoom:out").unwrap();
        sequence.execute("zoom:out").unwrap();
        let pitch = midi_hz(36);
        sequence.set_note(3, 0, pitch, 0.8, 4.0).unwrap();
        sequence.set_note(3, 4, pitch, 0.8, 1.0).unwrap();
        sequence.set_note(3, 8, pitch, 0.8, 4.0).unwrap();
        sequence.set_note(3, 10, pitch, 0.8, 1.0).unwrap();
        let mut mapping = authored_mapping("level");
        mapping.piano_roll = Some(crate::model::ApcPianoRoll {
            drum_tracks: [0, 1, 2],
            melodic_track: 3,
            scale: [0, 2, 3, 5, 7],
        });
        let (leds, _) = piano_roll_led_frame(
            &sequence,
            &GridController::new(),
            mapping.piano_roll.as_ref().unwrap(),
            &mapping,
        );
        let base = LOSPEC500[crate::performance_ui::pitch_color(f64::from(pitch))];
        assert_eq!(leds[0], base);
        assert_eq!(leds[1], [base[0] / 3, base[1] / 3, base[2] / 3]);
        assert_eq!(leds[2], LOSPEC500[36]);
    }

    #[test]
    fn midi_cc_faders_target_authored_controls() {
        assert_eq!(decode(&[0xb0, 48, 127]), Some(Input::Fader(0, 127)));
        assert_eq!(decode(&[0xb0, 56, 0]), Some(Input::Fader(8, 0)));
        let graph = crate::lua::read(
            r#"
            local i=meldritch.instrument("fader")
            local o=i:oscillator {waveform="sine",frequency=110}
            local master=i:gain {gain=.5}
            i:connect(o.out,master.input)
            i:connect(master.out,i.output.left);i:connect(master.out,i.output.right)
            i:control {id="level",label="Level",target=o.frequency,range={80,880},curve="log"}
            i:control {id="master",label="Master",target=master.gain,range={0,1}}
            return i
            "#,
        )
        .unwrap();
        let mut machine = crate::compiler::compile(&graph).unwrap();
        let handle = machine.control_handle("level").unwrap();
        let mut handles = BTreeMap::new();
        handles.insert("level".into(), (handle, 110.0));
        handles.insert(
            "master".into(),
            (machine.control_handle("master").unwrap(), 0.5),
        );
        let sequence = SequencerControl::new("test".into(), 120.0, 16, 4).unwrap();
        for n in 0..8 {
            sequence.add_track(format!("T{n}")).unwrap();
        }
        let mut state = GridController::new();
        let mut mapping = authored_mapping("level");
        mapping.master_control = "master".into();
        apply_authored(
            &mut state,
            Input::Fader(0, 127),
            &sequence,
            &mapping,
            &handles,
        )
        .unwrap();
        assert_eq!(handles["level"].0.override_value(), None);
        apply_authored(
            &mut state,
            Input::Fader(0, 18),
            &sequence,
            &mapping,
            &handles,
        )
        .unwrap();
        apply_authored(
            &mut state,
            Input::Fader(0, 127),
            &sequence,
            &mapping,
            &handles,
        )
        .unwrap();
        let mut rendered = vec![0.0; 480];
        machine.process(&mut rendered);
        assert!(
            rendered
                .windows(2)
                .filter(|v| v[0] < 0.0 && v[1] >= 0.0)
                .count()
                >= 8
        );
        apply_authored(
            &mut state,
            Input::Fader(8, 0),
            &sequence,
            &mapping,
            &handles,
        )
        .unwrap();
        assert_eq!(handles["master"].0.override_value(), None);
        apply_authored(
            &mut state,
            Input::Fader(8, 64),
            &sequence,
            &mapping,
            &handles,
        )
        .unwrap();
        apply_authored(
            &mut state,
            Input::Fader(8, 0),
            &sequence,
            &mapping,
            &handles,
        )
        .unwrap();
        machine.process(&mut rendered);
        assert_eq!(rendered, vec![0.0; 480]);

        mapping
            .fader_banks
            .push(std::array::from_fn(|_| "master".into()));
        assert!(apply_fader_bank_command(&mut state, &mapping, "faders:bank:1").unwrap());
        assert_eq!(state.fader_bank, 1);
        handles["master"].0.publish(0.5).unwrap();
        apply_authored(
            &mut state,
            Input::Fader(0, 0),
            &sequence,
            &mapping,
            &handles,
        )
        .unwrap();
        apply_authored(
            &mut state,
            Input::Fader(0, 64),
            &sequence,
            &mapping,
            &handles,
        )
        .unwrap();
        assert!((handles["master"].0.override_value().unwrap() - 64.0 / 127.0).abs() < 1e-9);
    }

    #[test]
    fn shutdown_checkerboard_uses_distinct_lospec_colors() {
        assert_ne!(shutdown_color(0, 0), shutdown_color(1, 0));
        assert_ne!(shutdown_color(0, 0), shutdown_color(0, 1));
        assert!(shutdown_color(63, 19).iter().any(|channel| *channel > 0));
    }
}
