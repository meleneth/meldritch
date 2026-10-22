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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Input {
    Grid(u8),
    Button(u8),
    Fader(u8, u8),
}

fn decode(bytes: &[u8]) -> Option<Input> {
    if bytes.len() >= 3 && bytes[0] & 0xf0 == 0xb0 && (48..=56).contains(&bytes[1]) {
        return Some(Input::Fader(bytes[1] - 48, bytes[2]));
    }
    if bytes.len() < 3 || bytes[2] == 0 || bytes[0] & 0xf0 != 0x90 || bytes[0] & 0x0f != 0 {
        return None;
    }
    match bytes[1] {
        cell @ 0..=63 => Some(Input::Grid(cell)),
        note @ (PAGE_A_NOTE | PAGE_B_NOTE | UP_NOTE | DOWN_NOTE | LEFT_NOTE | RIGHT_NOTE
        | PLAY_STOP_NOTE | RESTART_NOTE | CLEAR_NOTE) => Some(Input::Button(note)),
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
    selected_step: usize,
    clear_armed: Option<Instant>,
    sent: Option<([[u8; 3]; 64], [bool; 9])>,
}
impl GridController {
    fn new() -> Self {
        Self {
            page: 0,
            selected_track: 0,
            selected_step: 0,
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
                self.selected_step = self.page * 8 + column;
                sequence.toggle(track, self.page * 8 + column)
            }
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

fn led_frame(sequence: &SequencerControl, state: &GridController) -> ([[u8; 3]; 64], [bool; 9]) {
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
    let buttons = [
        state.page == 0,
        state.page == 1,
        state.selected_track > 0,
        state.selected_track < 7,
        state.selected_step > 0,
        state.selected_step + 1 < sequence.steps,
        playing,
        false,
        state.clear_armed.is_some(),
    ];
    (leds, buttons)
}
fn authored_led_frame(
    sequence: &SequencerControl,
    state: &GridController,
    mapping: &crate::model::ApcMapping,
) -> ([[u8; 3]; 64], [bool; 9]) {
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
    (
        leds,
        [
            state.page == 0,
            state.page == 1,
            true,
            true,
            true,
            true,
            sequence.playing(),
            false,
            false,
        ],
    )
}
fn refresh(
    output: &mut MidiOutputConnection,
    sequence: &SequencerControl,
    state: &mut GridController,
    mapping: Option<&crate::model::ApcMapping>,
) -> Result<(), String> {
    let (leds, buttons) = mapping.map_or_else(
        || led_frame(sequence, state),
        |mapping| authored_led_frame(sequence, state, mapping),
    );
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
        UP_NOTE,
        DOWN_NOTE,
        LEFT_NOTE,
        RIGHT_NOTE,
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

fn shutdown_color(cell: usize, frame: usize) -> [u8; 3] {
    let row = cell / 8;
    let column = cell % 8;
    LOSPEC500[(row + column + frame) % LOSPEC500.len()]
}

fn shutdown_animation(output: &mut MidiOutputConnection) {
    for frame in 0..20 {
        for cell in 0..64 {
            let checker = (cell / 8 + cell % 8 + frame) % 4;
            let rgb = if checker < 2 {
                shutdown_color(cell, frame * 2)
            } else {
                shutdown_color(cell, frame * 2 + 12)
            };
            if output
                .send(&apc_rgb(cell as u8, rgb).expect("valid APC cell"))
                .is_err()
            {
                return;
            }
        }
        thread::sleep(Duration::from_millis(50));
    }
    for cell in 0..64 {
        let _ = output.send(&apc_rgb(cell as u8, [0, 0, 0]).expect("valid APC cell"));
    }
    for note in [
        PAGE_A_NOTE,
        PAGE_B_NOTE,
        UP_NOTE,
        DOWN_NOTE,
        LEFT_NOTE,
        RIGHT_NOTE,
        PLAY_STOP_NOTE,
        RESTART_NOTE,
        CLEAR_NOTE,
    ] {
        let _ = output.send(&[0x90, note, 0]);
    }
}

pub struct ApcSession {
    stop: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
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
                .map(|(handle, _)| (handle.id().to_owned(), handle))
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
                    shutdown_animation(&mut output);
                    return;
                }
            }
        });
        Self {
            stop,
            thread: Some(thread),
        }
    }

    pub fn begin_shutdown(&self) {
        self.stop.store(true, Ordering::Release);
    }
}

fn publish_fader(handle: &ControlHandle, value: u8) -> Result<(), String> {
    let (min, max) = handle.range();
    let normalized = f64::from(value) / 127.0;
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
    handles: &BTreeMap<String, ControlHandle>,
) -> Result<(), String> {
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
        Input::Grid(cell) => {
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
        Input::Fader(index, value) => {
            let id = if index == 8 {
                &mapping.master_control
            } else {
                &mapping.fader_controls[usize::from(index)]
            };
            let handle = handles
                .get(id)
                .ok_or_else(|| format!("APC fader targets unknown control '{id}'"))?;
            publish_fader(handle, value)
        }
        Input::Button(_) => Ok(()),
    }
}
impl Drop for ApcSession {
    fn drop(&mut self) {
        self.begin_shutdown();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ApcMapping;

    fn authored_mapping(control: &str) -> ApcMapping {
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
            fader_controls: std::array::from_fn(|_| control.into()),
            master_control: control.into(),
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
        assert!(buttons[6]);
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

        apply_authored(&mut state, Input::Grid(56), &sequence, &mapping, &handles).unwrap();
        assert_eq!(sequence.pattern().velocities[0][0], 1.0);
        apply_authored(
            &mut state,
            Input::Button(RIGHT_NOTE),
            &sequence,
            &mapping,
            &handles,
        )
        .unwrap();
        apply_authored(&mut state, Input::Grid(57), &sequence, &mapping, &handles).unwrap();
        assert_eq!(sequence.pattern().velocities[0][9], 1.0);

        apply_authored(
            &mut state,
            Input::Button(PAGE_B_NOTE),
            &sequence,
            &mapping,
            &handles,
        )
        .unwrap();
        apply_authored(&mut state, Input::Grid(57), &sequence, &mapping, &handles).unwrap();
        assert_eq!(sequence.queued_pattern(), Some(1));
        apply_authored(&mut state, Input::Grid(48), &sequence, &mapping, &handles).unwrap();
        assert!(sequence.muted(0));
        apply_authored(&mut state, Input::Grid(40), &sequence, &mapping, &handles).unwrap();
        assert!(sequence.soloed(0));
        apply_authored(&mut state, Input::Grid(33), &sequence, &mapping, &handles).unwrap();
        assert_eq!(sequence.selected_track(), 1);

        let (leds, _) = authored_led_frame(&sequence, &state, &mapping);
        assert_eq!(leds[57], [127, 90, 0]);
        assert_eq!(leds[48], [127, 0, 0]);
        assert_eq!(leds[40], [127, 127, 0]);
        assert_eq!(leds[33], [0, 80, 127]);
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
        handles.insert("level".into(), handle);
        handles.insert("master".into(), machine.control_handle("master").unwrap());
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
        machine.process(&mut rendered);
        assert_eq!(rendered, vec![0.0; 480]);
    }

    #[test]
    fn shutdown_checkerboard_uses_distinct_lospec_colors() {
        assert_ne!(shutdown_color(0, 0), shutdown_color(1, 0));
        assert_ne!(shutdown_color(0, 0), shutdown_color(0, 1));
        assert!(shutdown_color(63, 19).iter().any(|channel| *channel > 0));
    }
}
