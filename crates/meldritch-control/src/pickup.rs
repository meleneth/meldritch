//! Physical pickup state on the control owner; never a second parameter model.
use crate::{QUEUE_CAPACITY, Result, devices::SoftTakeover};
use meldritch_app::{AppCommand, AppInput, AppViewModel, MidiControlInput};
use std::collections::BTreeMap;

type Physical = (String, u8, u8);
struct PickupState {
    control: String,
    page: Option<usize>,
    expected: f64,
    pickup: SoftTakeover,
}

#[derive(Default)]
pub struct MidiPickup {
    states: BTreeMap<Physical, PickupState>,
    sequence: Option<u64>,
}

fn key(input: &MidiControlInput) -> Physical {
    (input.device.clone(), input.channel, input.cc)
}

fn target(view: &AppViewModel, id: &str) -> Result<f64> {
    let control = view
        .curated_controls
        .iter()
        .find(|control| control.id == id)
        .ok_or("unknown pickup control")?;
    let value = control
        .value
        .ok_or("pickup control has no authoritative value")?;
    if !value.is_finite()
        || !control.minimum.is_finite()
        || !control.maximum.is_finite()
        || control.maximum <= control.minimum
        || !(control.minimum..=control.maximum).contains(&value)
    {
        return Err("invalid pickup control range/value".into());
    }
    Ok((value - control.minimum) / (control.maximum - control.minimum))
}

impl MidiPickup {
    /// Release physical ownership without touching authoritative control values.
    pub fn disconnect(&mut self, device: &str) {
        self.states.retain(|physical, _| physical.0 != device);
    }
    fn synchronize(&mut self, view: &AppViewModel, own: Option<&Physical>) {
        // Inspect bounded typed history so away-and-back page/parameter changes
        // cannot disappear between physical observations. A history gap rearms all.
        if self.sequence.is_some_and(|last| {
            view.history
                .first()
                .is_none_or(|first| first.sequence > last.saturating_add(1))
        }) {
            self.states.clear();
        }
        for record in &view.history {
            if self.sequence.is_some_and(|last| record.sequence <= last) || !record.changed {
                continue;
            }
            match &record.command {
                AppCommand::SelectPerformancePage(_)
                | AppCommand::SetPerformanceModifier { .. } => self.states.clear(),
                AppCommand::SetCuratedControlNormalized { id, .. }
                | AppCommand::AdjustCuratedControl { id, .. } => {
                    self.states
                        .retain(|physical, state| state.control != *id || own == Some(physical));
                }
                _ => {}
            }
        }
        self.sequence = view.history.last().map(|record| record.sequence);
    }

    /// Input is the existing mapper's normalized value, including authored curves.
    /// False is an observed movement waiting for pickup, not an accepted command.
    pub fn observe(
        &mut self,
        physical: &MidiControlInput,
        input: &AppInput,
        view: &AppViewModel,
    ) -> Result<bool> {
        self.synchronize(view, None);
        let AppInput::SetCuratedControlNormalized { id, value } = input else {
            return Ok(true);
        };
        if physical.value > 127
            || physical.cc > 127
            || !(1..=16).contains(&physical.channel)
            || !value.is_finite()
            || !(0.0..=1.0).contains(value)
        {
            return Err("invalid pickup observation".into());
        }
        let expected = target(view, id)?;
        let physical = key(physical);
        if !self.states.contains_key(&physical) && self.states.len() >= QUEUE_CAPACITY {
            return Err("pickup registry full".into());
        }
        let state = self.states.entry(physical).or_insert(PickupState {
            control: id.clone(),
            page: view.performance.active_page,
            expected,
            pickup: SoftTakeover::new(expected)?,
        });
        if state.control != *id
            || state.page != view.performance.active_page
            || state.expected != expected
        {
            *state = PickupState {
                control: id.clone(),
                page: view.performance.active_page,
                expected,
                pickup: SoftTakeover::new(expected)?,
            };
        }
        Ok(state.pickup.observe(*value)?.is_some())
    }

    /// Acknowledge only this physical input's accepted command. Other controllers
    /// are rearmed; app quantization does not release the controller that owns it.
    pub fn acknowledge(&mut self, physical: &MidiControlInput, view: &AppViewModel) {
        let physical = key(physical);
        self.synchronize(view, Some(&physical));
        if let Some(state) = self.states.get_mut(&physical)
            && let Ok(expected) = target(view, &state.control)
        {
            state.expected = expected;
        }
    }
}
