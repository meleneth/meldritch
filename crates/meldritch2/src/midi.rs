use crate::compiler::{ControlHandle, Machine};

/// Fixed lookup from raw MIDI CC bytes to an instrument-declared control.
#[derive(Clone)]
pub struct MidiRouter {
    routes: Vec<Option<ControlHandle>>,
}

impl MidiRouter {
    pub fn from_machine(machine: &Machine) -> Self {
        let mut routes = vec![None; 16 * 128];
        for binding in machine.midi_ccs() {
            let index = usize::from(binding.channel - 1) * 128 + usize::from(binding.number);
            routes[index] = machine.control_handle(&binding.control_id);
        }
        Self { routes }
    }

    /// Returns true only for a mapped, valid three-byte Control Change message.
    pub fn dispatch(&self, message: &[u8]) -> bool {
        if message.len() != 3 || message[0] & 0xF0 != 0xB0 || message[1] > 127 || message[2] > 127 {
            return false;
        }
        let index = usize::from(message[0] & 0x0F) * 128 + usize::from(message[1]);
        let Some(handle) = &self.routes[index] else {
            return false;
        };
        let (min, max) = handle.range();
        let value = if message[2] == 127 {
            max
        } else {
            min + (max - min) * f64::from(message[2]) / 127.0
        };
        handle.publish(value.clamp(min, max)).is_ok()
    }
}
