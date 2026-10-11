//! Protocol codecs and logical device simulation, outside musical semantics.
//! No hardware connection is implied by these functions. See CONTROL_PLANE.md.

pub use crate::pickup::MidiPickup;
use crate::{ControlPlane, Input, Result, SurfaceFrame};

#[derive(Clone, Debug)]
pub struct ObservedMidi {
    pub device: std::sync::Arc<str>,
    pub bytes: [u8; 3],
    current: std::sync::Arc<std::sync::atomic::AtomicBool>,
}
impl ObservedMidi {
    pub fn is_current(&self) -> bool {
        self.current.load(std::sync::atomic::Ordering::Acquire)
    }
}

pub struct MidiInputQueue {
    sender: std::sync::mpsc::SyncSender<ObservedMidi>,
    receiver: std::sync::mpsc::Receiver<ObservedMidi>,
    dropped: std::sync::Arc<std::sync::atomic::AtomicU64>,
}
impl Default for MidiInputQueue {
    fn default() -> Self {
        let (sender, receiver) = std::sync::mpsc::sync_channel(crate::QUEUE_CAPACITY);
        Self {
            sender,
            receiver,
            dropped: Default::default(),
        }
    }
}
impl MidiInputQueue {
    pub fn route(&self, device: &str) -> MidiInputRoute {
        MidiInputRoute {
            device: device.into(),
            sender: self.sender.clone(),
            dropped: self.dropped.clone(),
            current: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true)),
        }
    }
    pub fn next(&self) -> Option<ObservedMidi> {
        self.receiver.try_recv().ok()
    }
    pub fn take_dropped(&self) -> u64 {
        self.dropped.swap(0, std::sync::atomic::Ordering::Relaxed)
    }
}

#[derive(Clone)]
pub struct MidiInputRoute {
    device: std::sync::Arc<str>,
    sender: std::sync::mpsc::SyncSender<ObservedMidi>,
    dropped: std::sync::Arc<std::sync::atomic::AtomicU64>,
    current: std::sync::Arc<std::sync::atomic::AtomicBool>,
}
impl MidiInputRoute {
    /// Invalidate every queued observation from this connection, without
    /// disturbing another connection's FIFO entries or accepted domain facts.
    pub fn invalidate(&self) {
        self.current
            .store(false, std::sync::atomic::Ordering::Release);
    }
    pub fn observe(&self, bytes: &[u8]) -> bool {
        if decode_midi(bytes).is_none() {
            return false;
        }
        let bytes = [bytes[0], bytes[1], bytes[2]];
        if self
            .sender
            .try_send(ObservedMidi {
                device: self.device.clone(),
                bytes,
                current: self.current.clone(),
            })
            .is_err()
        {
            self.dropped
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
        true
    }
}

/// Exclusive pad ownership prevents Lua and TOML from applying one press twice.
/// Bounded observation queue; overflow is counted for control-thread diagnostics.
#[derive(Clone)]
pub struct ApcInputRoute {
    cells: std::collections::BTreeSet<u8>,
    sender: std::sync::mpsc::SyncSender<u8>,
    dropped: std::sync::Arc<std::sync::atomic::AtomicU64>,
}
impl ApcInputRoute {
    pub fn owns_cell(&self, cell: u8) -> bool {
        self.cells.contains(&cell)
    }
    pub fn new(cells: std::collections::BTreeSet<u8>) -> (Self, std::sync::mpsc::Receiver<u8>) {
        let (sender, receiver) = std::sync::mpsc::sync_channel(crate::QUEUE_CAPACITY);
        (
            Self {
                cells,
                sender,
                dropped: Default::default(),
            },
            receiver,
        )
    }
    /// True means owned, including releases and observed presses under backpressure.
    pub fn observe(&self, bytes: &[u8]) -> bool {
        let Some(ApcInput::Grid { cell, pressed }) = decode_apc(0, bytes) else {
            return false;
        };
        if !self.cells.contains(&cell) {
            return false;
        }
        if pressed && self.sender.try_send(cell).is_err() {
            self.dropped
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
        true
    }
    pub fn take_dropped(&self) -> u64 {
        self.dropped.swap(0, std::sync::atomic::Ordering::Relaxed)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum MidiObservation {
    Note {
        channel: u8,
        note: u8,
        velocity: u8,
        pressed: bool,
    },
    Control {
        channel: u8,
        cc: u8,
        value: u8,
    },
    Pitch {
        channel: u8,
        value: u16,
    },
}

/// Decode complete channel messages, including note-on velocity zero as release.
/// Transport owns packet assembly, running status and USB port identity.
pub fn decode_midi(bytes: &[u8]) -> Option<MidiObservation> {
    if bytes.len() != 3 || bytes[1] > 127 || bytes[2] > 127 {
        return None;
    }
    let channel = bytes[0] & 0x0f;
    match bytes[0] & 0xf0 {
        0x80 | 0x90 => Some(MidiObservation::Note {
            channel,
            note: bytes[1],
            velocity: bytes[2],
            pressed: bytes[0] & 0xf0 == 0x90 && bytes[2] != 0,
        }),
        0xb0 => Some(MidiObservation::Control {
            channel,
            cc: bytes[1],
            value: bytes[2],
        }),
        0xe0 => Some(MidiObservation::Pitch {
            channel,
            value: u16::from(bytes[1]) | (u16::from(bytes[2]) << 7),
        }),
        _ => None,
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum ApcInput {
    Grid { cell: u8, pressed: bool },
    Button { note: u8, pressed: bool },
    Fader { strip: u8, normalized: f64 },
}

/// APC Mini MK2 USB port 0 in Session mode; other ports/modes are explicit.
pub fn decode_apc(port: u8, bytes: &[u8]) -> Option<ApcInput> {
    if port != 0 {
        return None;
    }
    match decode_midi(bytes)? {
        MidiObservation::Note {
            channel: 0,
            note: cell @ 0..=63,
            pressed,
            ..
        } => Some(ApcInput::Grid { cell, pressed }),
        MidiObservation::Note {
            channel: 0,
            note,
            pressed,
            ..
        } if (0x64..=0x6b).contains(&note) || (0x70..=0x77).contains(&note) || note == 0x7a => {
            Some(ApcInput::Button { note, pressed })
        }
        MidiObservation::Control {
            channel: 0,
            cc: 0x30..=0x38,
            value,
        } => Some(ApcInput::Fader {
            strip: bytes[1] - 0x30,
            normalized: f64::from(value) / 127.0,
        }),
        _ => None,
    }
}

/// Exact RGB SysEx for one pad, per Akai communication protocol v1.0 p9.
pub fn apc_rgb(cell: u8, rgb: [u8; 3]) -> Result<[u8; 16]> {
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

/// Semantic widget colors encoded outside Lua. Commit the diff only after all
/// sends succeed; a failed send is retried on the next refresh.
#[derive(Default)]
pub struct ApcFeedback {
    sent: std::collections::BTreeMap<u8, [u8; 3]>,
    force: bool,
}
impl ApcFeedback {
    pub fn update(
        &mut self,
        frame: &SurfaceFrame,
        mut send: impl FnMut(&[u8]) -> Result<()>,
    ) -> Result<usize> {
        let mut next = std::collections::BTreeMap::new();
        for (cell, presentation) in &frame.cells {
            if !crate::SurfaceKind::ApcMiniMk2.accepts_cell(*cell) {
                return Err("APC presentation cell out of range".into());
            }
            let rgb = match presentation.color.as_str() {
                "off" => [0, 0, 0],
                "gray" => [24, 24, 24],
                "green" => [0, 180, 0],
                "amber" => [180, 70, 0],
                "yellow" => [180, 180, 0],
                "red" => [180, 0, 0],
                "blue" => [0, 0, 180],
                "white" => [180, 180, 180],
                color => return Err(format!("unsupported APC presentation color '{color}'")),
            };
            next.insert(*cell, rgb);
        }
        let cells = self
            .sent
            .keys()
            .chain(next.keys())
            .copied()
            .collect::<std::collections::BTreeSet<_>>();
        let mut count = 0;
        for cell in cells {
            let rgb = next.get(&cell).copied().unwrap_or([0; 3]);
            if self.force || self.sent.get(&cell) != Some(&rgb) {
                if cell < 64 {
                    send(&apc_rgb(cell, rgb)?)?;
                } else {
                    send(&[0x90, cell, if rgb == [0, 0, 0] { 0 } else { 127 }])?;
                }
                count += 1;
            }
        }
        self.sent = next;
        self.force = false;
        Ok(count)
    }

    /// Reconnection must restore desired state even when it is unchanged.
    pub fn invalidate(&mut self) {
        self.force = true;
    }
}

/// Host must reset on page/context change or external target changes. The
/// first observation never assumes a previous physical position in this bank.
#[derive(Clone, Debug)]
pub struct SoftTakeover {
    target: f64,
    previous: Option<f64>,
    acquired: bool,
}
impl SoftTakeover {
    pub fn new(target: f64) -> Result<Self> {
        if !target.is_finite() || !(0.0..=1.0).contains(&target) {
            return Err("invalid takeover target".into());
        }
        Ok(Self {
            target,
            previous: None,
            acquired: false,
        })
    }
    pub fn reset(&mut self, target: f64) -> Result<()> {
        *self = Self::new(target)?;
        Ok(())
    }
    pub fn observe(&mut self, value: f64) -> Result<Option<f64>> {
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return Err("invalid fader position".into());
        }
        let crossed = self
            .previous
            .is_some_and(|last| (last - self.target) * (value - self.target) <= 0.0);
        self.acquired |= crossed || (value - self.target).abs() <= 1.0 / 127.0;
        self.previous = Some(value);
        Ok(self.acquired.then_some(value))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum KeyboardInput {
    Note {
        channel: u8,
        note: u8,
        velocity: f64,
        pressed: bool,
    },
    Pitch {
        channel: u8,
        value: u16,
    },
    Modulation {
        channel: u8,
        normalized: f64,
    },
    Sustain {
        channel: u8,
        pressed: bool,
    },
    Unmapped(MidiObservation),
}

/// MicroLab mk3 supports configurable pedal/hold CCs. Supply the configured
/// modulation/sustain mappings; do not confuse expression CC11 with sustain.
pub fn decode_keyboard(bytes: &[u8], modulation_cc: u8, sustain_cc: u8) -> Option<KeyboardInput> {
    Some(match decode_midi(bytes)? {
        MidiObservation::Note {
            channel,
            note,
            velocity,
            pressed,
        } => KeyboardInput::Note {
            channel,
            note,
            velocity: f64::from(velocity) / 127.0,
            pressed,
        },
        MidiObservation::Pitch { channel, value } => KeyboardInput::Pitch { channel, value },
        MidiObservation::Control { channel, cc, value } if cc == modulation_cc => {
            KeyboardInput::Modulation {
                channel,
                normalized: f64::from(value) / 127.0,
            }
        }
        MidiObservation::Control { channel, cc, value } if cc == sustain_cc => {
            KeyboardInput::Sustain {
                channel,
                pressed: value >= 64,
            }
        }
        observation => KeyboardInput::Unmapped(observation),
    })
}

/// Simulated transport sends real dispatcher inputs and observes its output.
/// The same semantic frames can be encoded by MIDI, HID or TUI adapters.
pub struct SimulatedDevice {
    pub id: String,
}
impl SimulatedDevice {
    pub fn new(id: impl Into<String>) -> Self {
        Self { id: id.into() }
    }
    pub fn press(&self, plane: &mut ControlPlane, frame: u64, cell: u8) -> Result<()> {
        plane.enqueue(&self.id, frame, Input::Press(cell))
    }
    pub fn connection(&self, plane: &mut ControlPlane, frame: u64, connected: bool) -> Result<()> {
        plane.enqueue(&self.id, frame, Input::Connection(connected))
    }
    pub fn output<'a>(&self, plane: &'a ControlPlane) -> Option<&'a SurfaceFrame> {
        plane.output(&self.id)
    }
    pub fn apc_message(&self, plane: &mut ControlPlane, frame: u64, bytes: &[u8]) -> Result<()> {
        match decode_apc(0, bytes) {
            Some(ApcInput::Grid {
                cell,
                pressed: true,
            }) => self.press(plane, frame, cell),
            Some(ApcInput::Grid { pressed: false, .. }) => Ok(()),
            _ => Err("no authored binding for this APC input".into()),
        }
    }
}
