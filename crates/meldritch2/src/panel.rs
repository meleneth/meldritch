use std::sync::{
    Arc,
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
}

impl Panel {
    pub fn new(controls: Vec<(ControlHandle, f64)>) -> Self {
        Self::with_selection(controls, Arc::new(AtomicUsize::new(0)))
    }

    pub(crate) fn with_selection(
        controls: Vec<(ControlHandle, f64)>,
        selected: Arc<AtomicUsize>,
    ) -> Self {
        Self {
            rows: controls
                .into_iter()
                .map(|(handle, authored_value)| ControlRow::new(handle, authored_value))
                .collect(),
            selected,
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
        Ok(())
    }

    pub fn publish_selected_normalized(&self, normalized: f64) -> Result<(), String> {
        let Some(index) = self.selected() else {
            return Ok(());
        };
        let row = &self.rows[index];
        let (min, max) = row.range();
        let normalized = normalized.clamp(0.0, 1.0);
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
            }
            Gesture::Next => {
                let selected = (self.selected().unwrap_or(0) + 1).min(self.rows.len() - 1);
                self.selected.store(selected, Ordering::Release);
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
    use super::{Gesture, Panel};
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
}
