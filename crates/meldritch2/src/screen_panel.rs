use std::collections::BTreeMap;

use crate::{
    compiler::ControlHandle,
    model::{Screen, Widget},
    panel::{ControlRow, Gesture},
};

pub(crate) enum WidgetView {
    Label(String),
    Control { label: String, row: ControlRow },
    Link { label: String, target: usize },
}

impl WidgetView {
    fn selectable(&self) -> bool {
        !matches!(self, Self::Label(_))
    }
}

pub(crate) struct ScreenView {
    pub title: String,
    pub widgets: Vec<WidgetView>,
    pub selected: Option<usize>,
}

pub(crate) struct ScreenPanel {
    screens: Vec<ScreenView>,
    active: usize,
}

impl ScreenPanel {
    pub fn new(screens: &[Screen], controls: Vec<(ControlHandle, f64)>) -> Result<Self, String> {
        let controls: BTreeMap<_, _> = controls
            .into_iter()
            .map(|(handle, authored)| (handle.id().to_owned(), (handle, authored)))
            .collect();
        let ids: BTreeMap<_, _> = screens
            .iter()
            .enumerate()
            .map(|(i, screen)| (screen.id.as_str(), i))
            .collect();
        let mut views = Vec::with_capacity(screens.len());
        for screen in screens {
            let mut widgets = Vec::with_capacity(screen.widgets.len());
            for widget in &screen.widgets {
                widgets.push(match widget {
                    Widget::Label(text) => WidgetView::Label(text.clone()),
                    Widget::Control { label, control_id } => {
                        let (handle, authored) = controls
                            .get(control_id)
                            .ok_or_else(|| format!("unknown control '{control_id}'"))?;
                        WidgetView::Control {
                            label: label.clone(),
                            row: ControlRow::new(handle.clone(), *authored),
                        }
                    }
                    Widget::Link { label, screen_id } => WidgetView::Link {
                        label: label.clone(),
                        target: *ids
                            .get(screen_id.as_str())
                            .ok_or_else(|| format!("unknown screen '{screen_id}'"))?,
                    },
                });
            }
            let selected = widgets.iter().position(WidgetView::selectable);
            views.push(ScreenView {
                title: screen.title.clone(),
                widgets,
                selected,
            });
        }
        if views.is_empty() {
            return Err("screen panel requires at least one screen".into());
        }
        Ok(Self {
            screens: views,
            active: 0,
        })
    }

    pub fn current(&self) -> &ScreenView {
        &self.screens[self.active]
    }

    pub fn position(&self) -> (usize, usize) {
        (self.active + 1, self.screens.len())
    }

    pub fn switch(&mut self, direction: i32) {
        let len = self.screens.len();
        self.active = if direction < 0 {
            (self.active + len - 1) % len
        } else {
            (self.active + 1) % len
        };
    }

    pub fn activate(&mut self) {
        let Some(selected) = self.current().selected else {
            return;
        };
        if let WidgetView::Link { target, .. } = self.current().widgets[selected] {
            self.active = target;
        }
    }

    pub fn apply(&mut self, gesture: Gesture) -> Result<(), String> {
        let screen = &mut self.screens[self.active];
        let Some(selected) = screen.selected else {
            return Ok(());
        };
        match gesture {
            Gesture::Previous => {
                screen.selected = screen.widgets[..selected]
                    .iter()
                    .rposition(WidgetView::selectable)
                    .or(Some(selected));
            }
            Gesture::Next => {
                screen.selected = screen.widgets[selected + 1..]
                    .iter()
                    .position(WidgetView::selectable)
                    .map(|offset| selected + 1 + offset)
                    .or(Some(selected));
            }
            _ => {
                if let WidgetView::Control { row, .. } = &mut screen.widgets[selected] {
                    row.adjust(gesture)?;
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{ScreenPanel, WidgetView};
    use crate::{compiler::compile, lua, panel::Gesture};

    #[test]
    fn lua_screens_switch_and_adjust_only_visible_control() {
        let graph = lua::read(include_str!("../examples/66-screens.lua")).unwrap();
        let mut machine = compile(&graph).unwrap();
        let controls = machine
            .control_ids()
            .map(|id| {
                let handle = machine.control_handle(id).unwrap();
                let authored = machine.effective_value(handle.target()).unwrap();
                (handle, authored)
            })
            .collect();
        let mut panel = ScreenPanel::new(machine.screens(), controls).unwrap();
        assert_eq!(panel.current().title, "Home");
        assert_eq!(panel.current().selected, Some(1));
        panel.apply(Gesture::Increase).unwrap();
        panel.apply(Gesture::Next).unwrap();
        panel.activate();
        assert_eq!(panel.current().title, "Source");
        assert_eq!(panel.current().selected, Some(0));
        panel.apply(Gesture::Maximum).unwrap();
        let mut samples = [0.0; 1];
        machine.process(&mut samples);
        assert!((samples[0] - 0.57375).abs() < 1e-12);
        assert_eq!(
            machine.effective_value(&crate::model::ParameterId::new("left", "value")),
            Some(1.0)
        );
        panel.switch(-1);
        assert_eq!(panel.current().title, "Home");
        assert_eq!(panel.current().selected, Some(2));
        assert!(matches!(
            panel.current().widgets[2],
            WidgetView::Link { .. }
        ));
    }
}
