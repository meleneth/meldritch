//! Bounded presentation-only APC diagnostic; no app or audio mutation.
use crate::{Result, constrained_vm, error, tables};
use mlua::{LuaSerdeExt, Table, Value};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ApcLedDemo {
    version: u32,
    pub frames: Vec<LedFrame>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct LedFrame {
    pub hold_ms: u64,
    pads: Vec<Pad>,
    buttons: Vec<Button>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Pad {
    cell: u8,
    color: u8,
    #[serde(default = "default_brightness")]
    brightness: u8,
}
fn default_brightness() -> u8 {
    2
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Button {
    note: u8,
    mode: ButtonMode,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum ButtonMode {
    Off,
    On,
    Blink,
}

impl ApcLedDemo {
    /// Completely evaluate and validate before a host obtains MIDI access.
    pub fn load(source: &str) -> Result<Self> {
        let (lua, _budget) = constrained_vm(source)?;
        let definition: Table = lua
            .load(source)
            .set_name("apc-colors.lua")
            .set_mode(mlua::chunk::ChunkMode::Text)
            .eval()
            .map_err(error)?;
        // Serde's Vec conversion need not diagnose unrelated table keys. Keep
        // authored arrays contiguous and reject malformed sparse/map arrays.
        for frame in tables(definition.get("frames").map_err(error)?)? {
            tables(frame.get("pads").map_err(error)?)?;
            tables(frame.get("buttons").map_err(error)?)?;
        }
        let demo: Self = lua.from_value(Value::Table(definition)).map_err(error)?;
        if demo.version != 1 {
            return Err("unsupported APC LED API version".into());
        }
        if demo.frames.is_empty() || demo.frames.len() > 32 {
            return Err("LED plan must have 1..32 frames".into());
        }
        for frame in &demo.frames {
            if frame.hold_ms > 1000 || frame.pads.len() > 64 || frame.buttons.len() > 16 {
                return Err("LED frame exceeds hold/update capacity".into());
            }
            let mut pads = BTreeSet::new();
            for pad in &frame.pads {
                if pad.cell >= 64 || pad.color > 127 || pad.brightness > 6 {
                    return Err("invalid pad cell/color/brightness".into());
                }
                if !pads.insert(pad.cell) {
                    return Err("duplicate pad in LED frame".into());
                }
            }
            let mut buttons = BTreeSet::new();
            for button in &frame.buttons {
                if !(100..=107).contains(&button.note) && !(112..=119).contains(&button.note) {
                    return Err("invalid peripheral button note".into());
                }
                if !buttons.insert(button.note) {
                    return Err("duplicate button in LED frame".into());
                }
            }
        }
        if demo.duration_ms() > 10000 {
            return Err("LED plan exceeds 10 seconds".into());
        }
        Ok(demo)
    }

    pub fn duration_ms(&self) -> u64 {
        self.frames.iter().map(|frame| frame.hold_ms).sum()
    }

    pub fn clear_messages() -> Vec<[u8; 3]> {
        (0..64)
            .chain(100..108)
            .chain(112..120)
            .map(|note| [0x90, note, 0])
            .collect()
    }
}

impl LedFrame {
    /// Authored update order is deterministic. Only the control-thread host sends.
    pub fn messages(&self) -> Vec<[u8; 3]> {
        self.pads
            .iter()
            .map(|pad| [0x90 + pad.brightness, pad.cell, pad.color])
            .chain(self.buttons.iter().map(|button| {
                [
                    0x90,
                    button.note,
                    match button.mode {
                        ButtonMode::Off => 0,
                        ButtonMode::On => 1,
                        ButtonMode::Blink => 2,
                    },
                ]
            }))
            .collect()
    }
}
