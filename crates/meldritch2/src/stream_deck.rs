//! Lua-authored control surfaces projected onto Stream Deck XL hardware.
//!
//! Musical mutations are applied by [`SurfaceRuntime`] on the UI owner thread.
//! The HID worker only discovers devices, reads edges, renders images, and
//! applies bounded presentation updates.
//! Input overflow drops the newest edge and increments `dropped_inputs`;
//! presentation overflow invalidates the frame so the UI owner retries a full
//! refresh. Disconnects retain the latest visual state for the next device.

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::{self, Receiver, SyncSender},
    },
    thread,
    time::{Duration, Instant},
};

use elgato_streamdeck::{StreamDeckInput, info::Kind};
use font8x8::UnicodeFonts;
use image::{DynamicImage, Rgb, RgbImage};

use crate::panel::Panel;
use crate::{
    compiler::ControlHandle,
    model::{ControlSurface, Performance, SurfaceAction, SurfaceButton, SurfaceButtonMode},
};

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ButtonVisual {
    pub id: String,
    pub label: String,
    pub icon: Option<String>,
    pub color: [u8; 3],
    pub active: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ButtonEdge {
    pub index: u8,
    pub pressed: bool,
}

pub struct SurfaceRuntime {
    surface: ControlSurface,
    page: String,
    active: BTreeSet<String>,
    pressed: [bool; 32],
    controls: BTreeMap<String, (ControlHandle, f64)>,
    performance: Option<Performance>,
    sent: [Option<ButtonVisual>; 32],
    invalidated: bool,
    focus: Option<Panel>,
}

impl SurfaceRuntime {
    pub fn new(
        surface: ControlSurface,
        controls: Vec<(ControlHandle, f64)>,
        performance: Option<Performance>,
    ) -> Self {
        Self {
            page: surface.initial_page.clone(),
            surface,
            active: BTreeSet::new(),
            pressed: [false; 32],
            controls: controls
                .into_iter()
                .map(|(handle, initial)| (handle.id().to_owned(), (handle, initial)))
                .collect(),
            performance,
            sent: std::array::from_fn(|_| None),
            invalidated: true,
            focus: None,
        }
    }

    pub fn with_focus(
        mut self,
        controls: Vec<(ControlHandle, f64)>,
        selected: Arc<std::sync::atomic::AtomicUsize>,
    ) -> Self {
        self.focus = Some(Panel::with_selection(controls, selected));
        self
    }

    pub(crate) fn with_shared_focus(
        mut self,
        controls: Vec<(ControlHandle, f64)>,
        selected: Arc<std::sync::atomic::AtomicUsize>,
        pickup: Arc<std::sync::Mutex<crate::panel::PickupState>>,
    ) -> Self {
        self.focus = Some(Panel::with_selection_and_pickup(controls, selected, pickup));
        self
    }

    pub fn page(&self) -> &str {
        &self.page
    }

    pub fn focused_control_id(&self) -> Option<&str> {
        let focus = self.focus.as_ref()?;
        focus
            .selected()
            .and_then(|index| focus.rows().get(index))
            .map(|row| row.id())
    }

    pub fn invalidate(&mut self) {
        self.invalidated = true;
    }

    pub fn input(&mut self, edge: ButtonEdge) -> Result<(), String> {
        let index = usize::from(edge.index);
        if index >= self.pressed.len() || self.pressed[index] == edge.pressed {
            return Ok(());
        }
        self.pressed[index] = edge.pressed;
        let button = self
            .surface
            .pages
            .iter()
            .find(|page| page.id == self.page)
            .and_then(|page| {
                page.buttons
                    .iter()
                    .find(|button| button.index == edge.index)
            })
            .cloned();
        let Some(button) = button else { return Ok(()) };
        match button.mode {
            SurfaceButtonMode::Momentary => {
                if edge.pressed {
                    self.active.insert(button.id.clone());
                    self.apply(button.press.as_ref())?;
                } else {
                    self.active.remove(&button.id);
                    self.apply(button.release.as_ref())?;
                }
            }
            SurfaceButtonMode::Toggle if edge.pressed => {
                if self.active.remove(&button.id) {
                    self.apply(button.release.as_ref())?;
                } else {
                    self.active.insert(button.id.clone());
                    self.apply(button.press.as_ref())?;
                }
            }
            SurfaceButtonMode::Toggle => {}
        }
        Ok(())
    }

    fn apply(&mut self, action: Option<&SurfaceAction>) -> Result<(), String> {
        match action {
            None => {}
            Some(SurfaceAction::Refresh) => self.invalidated = true,
            Some(SurfaceAction::Page(page)) => self.page.clone_from(page),
            Some(SurfaceAction::Command(command)) => {
                if let Some(id) = command.strip_prefix("control:select:") {
                    self.focus
                        .as_ref()
                        .ok_or("surface control selection requires a performance focus")?
                        .select_id(id)?;
                    self.invalidated = true;
                } else {
                    self.performance
                        .as_ref()
                        .ok_or("surface command requires a performance model")?
                        .sequencer
                        .execute(command)?;
                }
            }
            Some(SurfaceAction::Control { id, value }) => self
                .controls
                .get(id)
                .ok_or_else(|| format!("surface targets unavailable control '{id}'"))?
                .0
                .publish(*value)?,
        }
        Ok(())
    }

    fn control_active(&self, button: &SurfaceButton) -> Option<bool> {
        if let Some(SurfaceAction::Command(command)) = button.press.as_ref()
            && let Some(id) = command.strip_prefix("control:select:")
        {
            let focus = self.focus.as_ref()?;
            return Some(
                focus
                    .selected()
                    .and_then(|index| focus.rows().get(index))
                    .is_some_and(|row| row.id() == id),
            );
        }
        let SurfaceAction::Control { id, value } = button.press.as_ref()? else {
            return None;
        };
        let (handle, initial) = self.controls.get(id)?;
        let current = handle.override_value().unwrap_or(*initial);
        Some((current - value).abs() <= 1e-9)
    }

    pub fn updates(&mut self) -> Vec<(u8, ButtonVisual)> {
        let Some(page) = self.surface.pages.iter().find(|page| page.id == self.page) else {
            return Vec::new();
        };
        let mut frame: [Option<ButtonVisual>; 32] = std::array::from_fn(|_| None);
        for button in &page.buttons {
            let active = self
                .control_active(button)
                .unwrap_or_else(|| self.active.contains(&button.id));
            frame[usize::from(button.index)] = Some(ButtonVisual {
                id: button.id.clone(),
                label: button.label.clone(),
                icon: button.icon.clone(),
                color: if active {
                    button.active_color
                } else {
                    button.color
                },
                active,
            });
        }
        let mut updates = Vec::new();
        for (index, visual) in frame.iter().enumerate() {
            if self.invalidated || self.sent[index] != *visual {
                updates.push((
                    index as u8,
                    visual.clone().unwrap_or_else(|| ButtonVisual {
                        id: String::new(),
                        label: String::new(),
                        icon: None,
                        color: [0, 0, 0],
                        active: false,
                    }),
                ));
            }
        }
        self.sent = frame;
        self.invalidated = false;
        updates
    }
}

fn button_image(visual: &ButtonVisual, size: (usize, usize)) -> DynamicImage {
    let mut image = RgbImage::from_pixel(size.0 as u32, size.1 as u32, Rgb(visual.color));
    if let Some(icon) = visual.icon.as_deref() {
        let center = (size.0 / 2, size.1 / 3);
        for y in 0..18usize {
            for x in 0..18usize {
                let lit = match icon {
                    "square" => (3..15).contains(&x) && (3..15).contains(&y),
                    "circle" => {
                        let dx = x as isize - 9;
                        let dy = y as isize - 9;
                        dx * dx + dy * dy <= 55
                    }
                    "triangle" => (3..16).contains(&y) && x >= 9 - y / 2 && x <= 9 + y / 2,
                    "bars" => matches!(x, 3..=5 | 8..=10 | 13..=15) && y >= 3,
                    _ => false,
                };
                if lit {
                    image.put_pixel(
                        (center.0 - 9 + x) as u32,
                        (center.1 - 9 + y) as u32,
                        Rgb([255, 255, 255]),
                    );
                }
            }
        }
    }
    // XL keys are 96 pixels wide on the devices we support.  Pick a scale
    // which fits the authored label instead of letting a long label index
    // beyond the image (and kill the HID worker).
    let label = visual.label.chars().take(10).collect::<Vec<_>>();
    let scale = if label.len().saturating_mul(16) <= size.0 {
        2usize
    } else {
        1usize
    };
    let chars = label
        .into_iter()
        .take(size.0 / (8 * scale).max(1))
        .collect::<Vec<_>>();
    let width = chars.len() * 8 * scale;
    let start_x = size.0.saturating_sub(width) / 2;
    let start_y = size.1.saturating_sub(8 * scale) / 2;
    for (char_index, glyph) in chars
        .into_iter()
        .filter_map(|character| font8x8::BASIC_FONTS.get(character))
        .enumerate()
    {
        for (row, bits) in glyph.iter().enumerate() {
            for column in 0..8 {
                if bits & (1 << column) != 0 {
                    for y in 0..scale {
                        for x in 0..scale {
                            image.put_pixel(
                                (start_x + char_index * 8 * scale + column * scale + x) as u32,
                                (start_y + row * scale + y) as u32,
                                Rgb([255, 255, 255]),
                            );
                        }
                    }
                }
            }
        }
    }
    DynamicImage::ImageRgb8(image)
}

enum Presentation {
    Update(Vec<(u8, ButtonVisual)>),
}

pub struct StreamDeckSession {
    runtime: SurfaceRuntime,
    inputs: Receiver<ButtonEdge>,
    presentation: SyncSender<Presentation>,
    stop: Arc<AtomicBool>,
    dropped_inputs: Arc<AtomicU64>,
    thread: Option<thread::JoinHandle<Result<(), String>>>,
}

impl StreamDeckSession {
    pub fn start(runtime: SurfaceRuntime) -> Self {
        let (input_tx, inputs) = mpsc::sync_channel(128);
        let (presentation, presentation_rx) = mpsc::sync_channel(8);
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = Arc::clone(&stop);
        let dropped_inputs = Arc::new(AtomicU64::new(0));
        let worker_drops = Arc::clone(&dropped_inputs);
        let thread = thread::spawn(move || {
            hardware_worker(worker_stop, input_tx, presentation_rx, worker_drops)
        });
        Self {
            runtime,
            inputs,
            presentation,
            stop,
            dropped_inputs,
            thread: Some(thread),
        }
    }

    pub fn poll(&mut self) -> Result<(), String> {
        while let Ok(edge) = self.inputs.try_recv() {
            self.runtime.input(edge)?;
        }
        let updates = self.runtime.updates();
        if !updates.is_empty()
            && self
                .presentation
                .try_send(Presentation::Update(updates))
                .is_err()
        {
            self.runtime.invalidate();
        }
        Ok(())
    }

    pub fn dropped_inputs(&self) -> u64 {
        self.dropped_inputs.load(Ordering::Relaxed)
    }
}

fn hardware_worker(
    stop: Arc<AtomicBool>,
    input: SyncSender<ButtonEdge>,
    presentation: Receiver<Presentation>,
    dropped_inputs: Arc<AtomicU64>,
) -> Result<(), String> {
    let mut visuals: [Option<ButtonVisual>; 32] = std::array::from_fn(|_| None);
    while !stop.load(Ordering::Acquire) {
        while let Ok(Presentation::Update(updates)) = presentation.try_recv() {
            for (key, visual) in updates {
                visuals[usize::from(key)] = Some(visual);
            }
        }
        let Ok(mut hid) = elgato_streamdeck::new_hidapi() else {
            thread::sleep(Duration::from_secs(1));
            continue;
        };
        let _ = elgato_streamdeck::refresh_device_list(&mut hid);
        let candidate = elgato_streamdeck::list_devices(&hid)
            .into_iter()
            .find(|(kind, _)| matches!(kind, Kind::Xl | Kind::XlV2 | Kind::XlV2Module));
        let Some((kind, serial)) = candidate else {
            thread::sleep(Duration::from_secs(1));
            continue;
        };
        let Ok(deck) = elgato_streamdeck::StreamDeck::connect(&hid, kind, &serial) else {
            thread::sleep(Duration::from_secs(1));
            continue;
        };
        let _ = deck.set_brightness(70);
        let mut buttons = vec![false; usize::from(kind.key_count())];
        let mut connected = true;
        let mut dirty = (0..32).collect::<BTreeSet<_>>();
        while connected && !stop.load(Ordering::Acquire) {
            while let Ok(Presentation::Update(updates)) = presentation.try_recv() {
                for (key, visual) in updates {
                    visuals[usize::from(key)] = Some(visual);
                    dirty.insert(key);
                }
            }
            if !dirty.is_empty() {
                for key in std::mem::take(&mut dirty) {
                    if let Some(visual) = &visuals[usize::from(key)]
                        && deck
                            .set_button_image(
                                key,
                                button_image(visual, kind.key_image_format().size),
                            )
                            .is_err()
                    {
                        connected = false;
                        break;
                    }
                }
                if connected && deck.flush().is_err() {
                    connected = false;
                }
            }
            match deck.read_input(Some(Duration::from_millis(25))) {
                Ok(StreamDeckInput::ButtonStateChange(next)) => {
                    for (index, (&before, &after)) in buttons.iter().zip(&next).enumerate() {
                        if before != after
                            && input
                                .try_send(ButtonEdge {
                                    index: index as u8,
                                    pressed: after,
                                })
                                .is_err()
                        {
                            dropped_inputs.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                    buttons = next;
                }
                Ok(StreamDeckInput::NoData) => {}
                Ok(_) => {}
                Err(_) => connected = false,
            }
        }
        if connected && stop.load(Ordering::Acquire) {
            return shutdown_animation(&deck, kind);
        }
    }
    Ok(())
}

fn shutdown_animation(deck: &elgato_streamdeck::StreamDeck, kind: Kind) -> Result<(), String> {
    const FRAME_TIME: Duration = Duration::from_millis(75);
    const ANIMATION_TIME: Duration = Duration::from_millis(1_500);
    let size = kind.key_image_format().size;
    let count = usize::from(kind.key_count()).min(32);
    let mut first_error = None;
    let started = Instant::now();
    for frame in 0..20 {
        for key in 0..count {
            let row = key / 8;
            let column = key % 8;
            let offset = if (row + column + frame) % 4 < 2 {
                0
            } else {
                12
            };
            let color = crate::performance_ui::LOSPEC500
                [(row + column + frame * 2 + offset) % crate::performance_ui::LOSPEC500.len()];
            let visual = ButtonVisual {
                id: "shutdown".into(),
                label: String::new(),
                icon: None,
                color,
                active: false,
            };
            if let Err(error) = deck.set_button_image(key as u8, button_image(&visual, size)) {
                first_error.get_or_insert_with(|| error.to_string());
            }
        }
        if let Err(error) = deck.flush() {
            first_error.get_or_insert_with(|| error.to_string());
        }
        let deadline = FRAME_TIME * (frame + 1) as u32;
        if started.elapsed() >= ANIMATION_TIME {
            break;
        }
        if let Some(remaining) = deadline.checked_sub(started.elapsed()) {
            thread::sleep(remaining);
        }
    }
    let black = ButtonVisual {
        id: "clear".into(),
        label: String::new(),
        icon: None,
        color: [0, 0, 0],
        active: false,
    };
    for key in 0..count {
        if let Err(error) = deck.set_button_image(key as u8, button_image(&black, size)) {
            first_error.get_or_insert_with(|| error.to_string());
        }
    }
    if let Err(error) = deck.flush() {
        first_error.get_or_insert_with(|| error.to_string());
    }
    first_error.map_or(Ok(()), |error| {
        Err(format!("Stream Deck shutdown animation failed: {error}"))
    })
}

impl Drop for StreamDeckSession {
    fn drop(&mut self) {
        let _ = self.finish();
    }
}

impl StreamDeckSession {
    pub fn begin_shutdown(&self) {
        self.stop.store(true, Ordering::Release);
    }

    pub fn finish(&mut self) -> Result<(), String> {
        self.begin_shutdown();
        match self.thread.take() {
            Some(thread) => thread
                .join()
                .map_err(|_| "Stream Deck worker panicked".to_owned())?,
            None => Ok(()),
        }
    }
}

#[derive(Default)]
pub struct SimulatedAdapter {
    pub uploads: Vec<(u8, ButtonVisual)>,
    connected: bool,
}

impl SimulatedAdapter {
    pub fn connect(&mut self, runtime: &mut SurfaceRuntime) {
        self.connected = true;
        runtime.invalidate();
        self.refresh(runtime);
    }
    pub fn disconnect(&mut self) {
        self.connected = false;
    }
    pub fn input(&mut self, runtime: &mut SurfaceRuntime, edge: ButtonEdge) -> Result<(), String> {
        if self.connected {
            runtime.input(edge)?;
            self.refresh(runtime);
        }
        Ok(())
    }
    pub fn refresh(&mut self, runtime: &mut SurfaceRuntime) {
        if self.connected {
            self.uploads.extend(runtime.updates());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        model::{ControlPage, Performance, SurfaceAction, SurfaceButton},
        sequencer::SequencerControl,
    };

    fn button(
        id: &str,
        index: u8,
        mode: SurfaceButtonMode,
        press: SurfaceAction,
        release: Option<SurfaceAction>,
    ) -> SurfaceButton {
        SurfaceButton {
            id: id.into(),
            index,
            label: id.into(),
            icon: None,
            color: [1, 2, 3],
            active_color: [8, 9, 10],
            mode,
            press: Some(press),
            release,
        }
    }
    fn runtime() -> SurfaceRuntime {
        SurfaceRuntime::new(
            ControlSurface {
                id: "test".into(),
                device: "stream_deck_xl".into(),
                initial_page: "main".into(),
                pages: vec![
                    ControlPage {
                        id: "main".into(),
                        buttons: vec![button(
                            "next",
                            31,
                            SurfaceButtonMode::Momentary,
                            SurfaceAction::Page("modes".into()),
                            None,
                        )],
                    },
                    ControlPage {
                        id: "modes".into(),
                        buttons: vec![
                            button(
                                "toggle",
                                0,
                                SurfaceButtonMode::Toggle,
                                SurfaceAction::Refresh,
                                Some(SurfaceAction::Refresh),
                            ),
                            button(
                                "back",
                                31,
                                SurfaceButtonMode::Momentary,
                                SurfaceAction::Page("main".into()),
                                None,
                            ),
                        ],
                    },
                ],
            },
            Vec::new(),
            None,
        )
    }

    #[test]
    fn simulation_maps_edges_pages_toggles_cache_and_reconnects() {
        let mut runtime = runtime();
        let mut adapter = SimulatedAdapter::default();
        adapter.connect(&mut runtime);
        assert_eq!(adapter.uploads.len(), 32);
        adapter.refresh(&mut runtime);
        assert_eq!(adapter.uploads.len(), 32, "unchanged frame was uploaded");
        adapter
            .input(
                &mut runtime,
                ButtonEdge {
                    index: 31,
                    pressed: true,
                },
            )
            .unwrap();
        assert_eq!(runtime.page(), "modes");
        adapter
            .input(
                &mut runtime,
                ButtonEdge {
                    index: 31,
                    pressed: false,
                },
            )
            .unwrap();
        adapter
            .input(
                &mut runtime,
                ButtonEdge {
                    index: 0,
                    pressed: true,
                },
            )
            .unwrap();
        assert!(
            adapter
                .uploads
                .iter()
                .rev()
                .find(|(_, visual)| visual.id == "toggle")
                .unwrap()
                .1
                .active
        );
        adapter
            .input(
                &mut runtime,
                ButtonEdge {
                    index: 0,
                    pressed: false,
                },
            )
            .unwrap();
        adapter
            .input(
                &mut runtime,
                ButtonEdge {
                    index: 0,
                    pressed: true,
                },
            )
            .unwrap();
        assert!(
            !adapter
                .uploads
                .iter()
                .rev()
                .find(|(_, visual)| visual.id == "toggle")
                .unwrap()
                .1
                .active
        );
        adapter.disconnect();
        let before = adapter.uploads.len();
        adapter.refresh(&mut runtime);
        assert_eq!(adapter.uploads.len(), before);
        adapter.connect(&mut runtime);
        assert_eq!(adapter.uploads.len(), before + 32);
    }

    #[test]
    fn commands_use_the_shared_sequencer_mutation_path() {
        let sequence = SequencerControl::new("surface".into(), 120.0, 16, 4).unwrap();
        sequence.stop();
        let surface = ControlSurface {
            id: "commands".into(),
            device: "stream_deck_xl".into(),
            initial_page: "main".into(),
            pages: vec![ControlPage {
                id: "main".into(),
                buttons: vec![button(
                    "transport",
                    0,
                    SurfaceButtonMode::Momentary,
                    SurfaceAction::Command("transport".into()),
                    None,
                )],
            }],
        };
        let performance = Performance {
            layout: "step_grid".into(),
            sequencer: Arc::clone(&sequence),
            key_bindings: Vec::new(),
            apc: None,
            project_file: None,
            control_focus: None,
        };
        let mut runtime = SurfaceRuntime::new(surface, Vec::new(), Some(performance));
        runtime
            .input(ButtonEdge {
                index: 0,
                pressed: true,
            })
            .unwrap();
        assert!(sequence.playing());
    }

    #[test]
    fn xl_button_renderer_fits_long_authored_labels() {
        let visual = ButtonVisual {
            id: "long-label".into(),
            label: "MODULATION".into(),
            icon: Some("bars".into()),
            color: [12, 34, 56],
            active: false,
        };
        let image = button_image(&visual, (96, 96));
        assert_eq!(image.width(), 96);
        assert_eq!(image.height(), 96);
    }
}
