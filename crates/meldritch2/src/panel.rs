use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};

use crate::compiler::ControlHandle;
use crate::model::ControlScale;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gesture {
    Previous,
    Next,
    Decrease,
    Increase,
    Minimum,
    Maximum,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PickupDirection {
    Below,
    Captured,
    Above,
    Unknown,
}

#[derive(Debug, Default)]
pub(crate) struct PickupState {
    previous: Option<f64>,
    captured: bool,
    direction: Option<std::cmp::Ordering>,
}

#[derive(Clone)]
pub struct ControlRow {
    handle: ControlHandle,
    authored_value: f64,
}

impl ControlRow {
    pub(crate) fn new(handle: ControlHandle, authored_value: f64) -> Self {
        Self {
            handle,
            authored_value,
        }
    }
    pub fn id(&self) -> &str {
        self.handle.id()
    }
    pub fn label(&self) -> &str {
        self.handle.label()
    }
    pub fn target(&self) -> &crate::model::ParameterId {
        self.handle.target()
    }
    pub fn range(&self) -> (f64, f64) {
        self.handle.range()
    }
    pub fn value(&self) -> f64 {
        self.handle.override_value().unwrap_or(self.authored_value)
    }
    pub fn hardware_pickup(&self) -> i8 {
        self.handle.hardware_pickup()
    }
    pub(crate) fn publish(&self, value: f64) -> Result<(), String> {
        self.handle.publish(value)
    }
    pub(crate) fn adjust(&mut self, gesture: Gesture) -> Result<(), String> {
        let (min, max) = self.handle.range();
        let value = match gesture {
            Gesture::Decrease if self.handle.scale() == ControlScale::Logarithmic => {
                (self.value() / (max / min).powf(0.01)).clamp(min, max)
            }
            Gesture::Increase if self.handle.scale() == ControlScale::Logarithmic => {
                (self.value() * (max / min).powf(0.01)).clamp(min, max)
            }
            Gesture::Decrease => (self.value() - (max - min) / 100.0).clamp(min, max),
            Gesture::Increase => (self.value() + (max - min) / 100.0).clamp(min, max),
            Gesture::Minimum => min,
            Gesture::Maximum => max,
            _ => return Ok(()),
        };
        self.handle.publish(value)
    }
}

/// Presentation state only; the control handles own publication to DSP.
#[derive(Clone)]
pub struct Panel {
    rows: Vec<ControlRow>,
    selected: Arc<AtomicUsize>,
    pickup: Arc<Mutex<PickupState>>,
}

impl Panel {
    pub fn new(controls: Vec<(ControlHandle, f64)>) -> Self {
        Self::with_selection(controls, Arc::new(AtomicUsize::new(0)))
    }

    pub(crate) fn with_selection(
        controls: Vec<(ControlHandle, f64)>,
        selected: Arc<AtomicUsize>,
    ) -> Self {
        Self::with_selection_and_pickup(
            controls,
            selected,
            Arc::new(Mutex::new(PickupState::default())),
        )
    }

    pub(crate) fn with_selection_and_pickup(
        controls: Vec<(ControlHandle, f64)>,
        selected: Arc<AtomicUsize>,
        pickup: Arc<Mutex<PickupState>>,
    ) -> Self {
        Self {
            rows: controls
                .into_iter()
                .map(|(handle, authored_value)| ControlRow::new(handle, authored_value))
                .collect(),
            selected,
            pickup,
        }
    }

    pub fn rows(&self) -> &[ControlRow] {
        &self.rows
    }
    pub fn selected(&self) -> Option<usize> {
        (!self.rows.is_empty()).then_some(
            self.selected
                .load(Ordering::Acquire)
                .min(self.rows.len().saturating_sub(1)),
        )
    }

    pub fn select_id(&self, id: &str) -> Result<(), String> {
        let index = self
            .rows
            .iter()
            .position(|row| row.id() == id)
            .ok_or_else(|| format!("unknown focus control '{id}'"))?;
        self.selected.store(index, Ordering::Release);
        self.reset_pickup();
        Ok(())
    }

    pub fn pickup_direction(&self) -> PickupDirection {
        let Ok(state) = self.pickup.lock() else {
            return PickupDirection::Unknown;
        };
        if state.captured {
            PickupDirection::Captured
        } else {
            match state.direction {
                Some(std::cmp::Ordering::Less) => PickupDirection::Below,
                Some(std::cmp::Ordering::Greater) => PickupDirection::Above,
                Some(std::cmp::Ordering::Equal) => PickupDirection::Captured,
                None => PickupDirection::Unknown,
            }
        }
    }

    fn reset_pickup(&self) {
        if let Ok(mut state) = self.pickup.lock() {
            *state = PickupState::default();
        }
    }

    pub fn publish_selected_normalized(&self, normalized: f64) -> Result<(), String> {
        let Some(index) = self.selected() else {
            return Ok(());
        };
        let row = &self.rows[index];
        let (min, max) = row.range();
        let normalized = normalized.clamp(0.0, 1.0);
        let current = if row.handle.scale() == ControlScale::Logarithmic {
            (row.value() / min).ln() / (max / min).ln()
        } else {
            (row.value() - min) / (max - min)
        }
        .clamp(0.0, 1.0);
        let mut pickup = self
            .pickup
            .lock()
            .map_err(|_| "control pickup state poisoned")?;
        let crossed = pickup
            .previous
            .is_some_and(|previous| (previous - current) * (normalized - current) <= 0.0);
        if !pickup.captured && !crossed && (normalized - current).abs() > 0.025 {
            pickup.previous = Some(normalized);
            pickup.direction = normalized.partial_cmp(&current);
            return Ok(());
        }
        pickup.captured = true;
        pickup.previous = Some(normalized);
        pickup.direction = Some(std::cmp::Ordering::Equal);
        drop(pickup);
        let value = if row.handle.scale() == ControlScale::Logarithmic {
            min * (max / min).powf(normalized)
        } else {
            min + (max - min) * normalized
        };
        row.publish(value)
    }

    pub fn apply(&mut self, gesture: Gesture) -> Result<(), String> {
        if self.rows.is_empty() {
            return Ok(());
        }
        match gesture {
            Gesture::Previous => {
                let selected = self.selected().unwrap_or(0).saturating_sub(1);
                self.selected.store(selected, Ordering::Release);
                self.reset_pickup();
            }
            Gesture::Next => {
                let selected = (self.selected().unwrap_or(0) + 1).min(self.rows.len() - 1);
                self.selected.store(selected, Ordering::Release);
                self.reset_pickup();
            }
            Gesture::Decrease | Gesture::Increase | Gesture::Minimum | Gesture::Maximum => {
                let selected = self.selected().unwrap_or(0);
                self.rows[selected].adjust(gesture)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{Gesture, Panel, PickupDirection};
    use crate::{compiler::compile, lua};

    #[test]
    fn gestures_follow_instrument_control_bindings() {
        let graph = lua::read(include_str!("../examples/09-controls.lua")).unwrap();
        let mut machine = compile(&graph).unwrap();
        let controls = machine
            .control_ids()
            .map(|id| {
                let handle = machine.control_handle(id).unwrap();
                let value = machine.effective_value(handle.target()).unwrap();
                (handle, value)
            })
            .collect();
        let mut panel = Panel::new(controls);
        assert_eq!(panel.rows().len(), 2);
        assert_eq!(panel.rows()[0].id(), "left-value");
        assert_eq!(panel.selected(), Some(0));
        panel.apply(Gesture::Next).unwrap();
        assert_eq!(panel.selected(), Some(1));
        panel.apply(Gesture::Increase).unwrap();
        assert!((panel.rows()[1].value() - 0.51).abs() < 1e-12);
        let mut samples = [0.0; 4];
        machine.process(&mut samples);
        assert!(
            samples
                .iter()
                .all(|sample| (*sample - 0.19125).abs() < 1e-12)
        );
        panel.apply(Gesture::Minimum).unwrap();
        machine.process(&mut samples);
        assert_eq!(samples, [0.0; 4]);
        panel.apply(Gesture::Maximum).unwrap();
        machine.process(&mut samples);
        assert_eq!(samples, [0.375; 4]);
        panel.apply(Gesture::Previous).unwrap();
        assert_eq!(panel.selected(), Some(0));
    }

    #[test]
    fn external_publication_updates_display_and_next_gesture() {
        let graph = lua::read(include_str!("../examples/10-midi-control.lua")).unwrap();
        let machine = compile(&graph).unwrap();
        let handle = machine.control_handle("level").unwrap();
        let authored = machine.effective_value(handle.target()).unwrap();
        let mut panel = Panel::new(vec![(handle.clone(), authored)]);
        handle.publish(0.75).unwrap();
        assert_eq!(panel.rows()[0].value(), 0.75);
        panel.apply(Gesture::Increase).unwrap();
        assert!((panel.rows()[0].value() - 0.76).abs() < 1e-12);
        handle.clear().unwrap();
        assert_eq!(panel.rows()[0].value(), authored);
    }

    #[test]
    fn physical_control_must_pick_up_current_value_before_publishing() {
        let graph = lua::read(include_str!("../examples/10-midi-control.lua")).unwrap();
        let machine = compile(&graph).unwrap();
        let handle = machine.control_handle("level").unwrap();
        let authored = machine.effective_value(handle.target()).unwrap();
        let panel = Panel::new(vec![(handle.clone(), authored)]);
        panel.publish_selected_normalized(1.0).unwrap();
        assert_eq!(handle.override_value(), None);
        assert_eq!(panel.pickup_direction(), PickupDirection::Above);
        panel.publish_selected_normalized(authored).unwrap();
        assert_eq!(panel.pickup_direction(), PickupDirection::Captured);
        panel.publish_selected_normalized(0.8).unwrap();
        assert_eq!(handle.override_value(), Some(0.8));
    }
}
