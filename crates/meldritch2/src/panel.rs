use crate::compiler::ControlHandle;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gesture {
    Previous,
    Next,
    Decrease,
    Increase,
    Minimum,
    Maximum,
}

pub struct ControlRow {
    handle: ControlHandle,
    value: f64,
}

impl ControlRow {
    pub fn id(&self) -> &str {
        self.handle.id()
    }
    pub fn target(&self) -> &crate::model::ParameterId {
        self.handle.target()
    }
    pub fn range(&self) -> (f64, f64) {
        self.handle.range()
    }
    pub fn value(&self) -> f64 {
        self.value
    }
}

/// Presentation state only; the control handles own publication to DSP.
pub struct Panel {
    rows: Vec<ControlRow>,
    selected: usize,
}

impl Panel {
    pub fn new(controls: Vec<(ControlHandle, f64)>) -> Self {
        Self {
            rows: controls
                .into_iter()
                .map(|(handle, value)| ControlRow { handle, value })
                .collect(),
            selected: 0,
        }
    }

    pub fn rows(&self) -> &[ControlRow] {
        &self.rows
    }
    pub fn selected(&self) -> Option<usize> {
        (!self.rows.is_empty()).then_some(self.selected)
    }

    pub fn apply(&mut self, gesture: Gesture) -> Result<(), String> {
        if self.rows.is_empty() {
            return Ok(());
        }
        match gesture {
            Gesture::Previous => self.selected = self.selected.saturating_sub(1),
            Gesture::Next => self.selected = (self.selected + 1).min(self.rows.len() - 1),
            Gesture::Decrease | Gesture::Increase | Gesture::Minimum | Gesture::Maximum => {
                let row = &mut self.rows[self.selected];
                let (min, max) = row.handle.range();
                let step = (max - min) / 100.0;
                let value = match gesture {
                    Gesture::Decrease => (row.value - step).clamp(min, max),
                    Gesture::Increase => (row.value + step).clamp(min, max),
                    Gesture::Minimum => min,
                    Gesture::Maximum => max,
                    _ => unreachable!(),
                };
                row.handle.publish(value)?;
                row.value = value;
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
}
