//! APC mini mk2 adapter for the semantic `step_grid` performance surface.

use crate::{device::unique_name_match, sequencer::SequencerControl};
use midir::{Ignore, MidiInput, MidiOutput, MidiOutputConnection};
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
pub const PLAY_STOP_NOTE: u8 = 112;
pub const RESTART_NOTE: u8 = 113;
pub const CLEAR_NOTE: u8 = 114;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Input {
    Grid(u8),
    Button(u8),
}

fn decode(bytes: &[u8]) -> Option<Input> {
    if bytes.len() < 3 || bytes[2] == 0 || bytes[0] & 0xf0 != 0x90 || bytes[0] & 0x0f != 0 {
        return None;
    }
    match bytes[1] {
        cell @ 0..=63 => Some(Input::Grid(cell)),
        note @ (PAGE_A_NOTE | PAGE_B_NOTE | PLAY_STOP_NOTE | RESTART_NOTE | CLEAR_NOTE) => {
            Some(Input::Button(note))
        }
        _ => None,
    }
}

pub fn apc_rgb(cell: u8, rgb: [u8; 3]) -> Result<[u8; 16], String> {
    if cell >= 64 {
        return Err("APC RGB pad out of range".into());
    }
    Ok([
        0xf0,
        0x47,
        0x7f,
        0x4f,
        0x24,
        0,
        8,
        cell,
        cell,
        rgb[0] >> 7,
        rgb[0] & 0x7f,
        rgb[1] >> 7,
        rgb[1] & 0x7f,
        rgb[2] >> 7,
        rgb[2] & 0x7f,
        0xf7,
    ])
}

#[derive(Debug)]
struct GridController {
    page: usize,
    selected_track: usize,
    clear_armed: Option<Instant>,
    sent: Option<([[u8; 3]; 64], [bool; 5])>,
}
impl GridController {
    fn new() -> Self {
        Self {
            page: 0,
            selected_track: 0,
            clear_armed: None,
            sent: None,
        }
    }
    fn apply(
        &mut self,
        input: Input,
        sequence: &SequencerControl,
        now: Instant,
    ) -> Result<(), String> {
        match input {
            Input::Grid(cell) => {
                let track = 7 - usize::from(cell / 8);
                let column = usize::from(cell % 8);
                self.selected_track = track;
                sequence.toggle(track, self.page * 8 + column)
            }
            Input::Button(PAGE_A_NOTE) => {
                self.page = 0;
                Ok(())
            }
            Input::Button(PAGE_B_NOTE) => {
                self.page = 1;
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

fn led_frame(sequence: &SequencerControl, state: &GridController) -> ([[u8; 3]; 64], [bool; 5]) {
    let pattern = sequence.pattern();
    let first = state.page * 8;
    let playing = sequence.playing();
    let current = sequence.current_step();
    let mut leds = [[0u8; 3]; 64];
    for track in 0..8 {
        for column in 0..8 {
            let step = first + column;
            let active = pattern.velocities[track][step] > 0.0;
            let rgb = if playing && current == step {
                if active { [127, 127, 0] } else { [0, 30, 127] }
            } else if active {
                [0, 127, 0]
            } else {
                [2, 2, 2]
            };
            let cell = ((7 - track) * 8 + column) as u8;
            leds[usize::from(cell)] = rgb;
        }
    }
    let buttons = [
        state.page == 0,
        state.page == 1,
        playing,
        false,
        state.clear_armed.is_some(),
    ];
    (leds, buttons)
}
fn refresh(
    output: &mut MidiOutputConnection,
    sequence: &SequencerControl,
    state: &mut GridController,
) -> Result<(), String> {
    let (leds, buttons) = led_frame(sequence, state);
    for (cell, rgb) in leds.iter().enumerate() {
        if state.sent.as_ref().is_none_or(|sent| sent.0[cell] != *rgb) {
            output
                .send(&apc_rgb(cell as u8, *rgb)?)
                .map_err(|e| e.to_string())?;
        }
    }
    for (index, note) in [
        PAGE_A_NOTE,
        PAGE_B_NOTE,
        PLAY_STOP_NOTE,
        RESTART_NOTE,
        CLEAR_NOTE,
    ]
    .into_iter()
    .enumerate()
    {
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

pub struct ApcSession {
    stop: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
}
impl ApcSession {
    pub fn start(name: String, sequence: Arc<SequencerControl>) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = Arc::clone(&stop);
        let thread = thread::spawn(move || {
            let mut state = GridController::new();
            while !worker_stop.load(Ordering::Acquire) {
                let (tx, rx): (SyncSender<Input>, Receiver<Input>) = mpsc::sync_channel(128);
                let Ok((_input, mut output)) = connect(&name, tx) else {
                    thread::sleep(Duration::from_secs(1));
                    continue;
                };
                state.sent = None;
                if refresh(&mut output, &sequence, &mut state).is_err() {
                    continue;
                }
                while !worker_stop.load(Ordering::Acquire) {
                    while let Ok(event) = rx.try_recv() {
                        let _ = state.apply(event, &sequence, Instant::now());
                    }
                    if state
                        .clear_armed
                        .is_some_and(|at| at.elapsed() > Duration::from_millis(1500))
                    {
                        state.clear_armed = None;
                    }
                    if refresh(&mut output, &sequence, &mut state).is_err() {
                        break;
                    }
                    thread::sleep(Duration::from_millis(25));
                }
            }
        });
        Self {
            stop,
            thread: Some(thread),
        }
    }
}
impl Drop for ApcSession {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn grid_pages_toggle_correct_halves_and_clear_is_guarded() {
        let sequence = SequencerControl::new("test".into(), 120.0, 16, 4).unwrap();
        for n in 0..8 {
            sequence.add_track(format!("T{n}")).unwrap();
        }
        let mut grid = GridController::new();
        let now = Instant::now();
        grid.apply(Input::Grid(56), &sequence, now).unwrap();
        assert_eq!(sequence.pattern().velocities[0][0], 1.0);
        grid.apply(Input::Button(PAGE_B_NOTE), &sequence, now)
            .unwrap();
        grid.apply(Input::Grid(56), &sequence, now).unwrap();
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
        assert_eq!(leds[56], [127, 127, 0]);
        assert!(buttons[0]);
        assert!(buttons[2]);
    }
}
