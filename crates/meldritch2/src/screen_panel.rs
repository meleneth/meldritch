use std::collections::BTreeMap;

use crate::{
    compiler::ControlHandle,
    model::{Screen, Widget},
    panel::{ControlRow, Gesture},
    performance_ui::{SurfaceView, TrackView, palette_index},
};

pub(crate) enum WidgetView {
    Label(String),
    Control {
        label: String,
        row: ControlRow,
    },
    Link {
        label: String,
        target: usize,
    },
    Button {
        label: String,
        row: ControlRow,
        value: f64,
    },
    PerformanceControls,
    PerformanceStatus,
    PatternGrid,
    TransportStatus,
    PerformanceSurface(SurfaceView),
}

impl WidgetView {
    fn selectable(&self) -> bool {
        matches!(
            self,
            Self::Control { .. } | Self::Link { .. } | Self::Button { .. }
        )
    }
}

pub(crate) struct ScreenView {
    pub title: String,
    pub widgets: Vec<WidgetView>,
    pub selected: Option<usize>,
}

pub(crate) struct ScreenPanel {
    screens: Vec<ScreenView>,
    ids: BTreeMap<String, usize>,
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
                    Widget::Button {
                        label,
                        control_id,
                        value,
                    } => {
                        let (handle, authored) = controls
                            .get(control_id)
                            .ok_or_else(|| format!("unknown control '{control_id}'"))?;
                        WidgetView::Button {
                            label: label.clone(),
                            row: ControlRow::new(handle.clone(), *authored),
                            value: *value,
                        }
                    }
                    Widget::PerformanceControls => WidgetView::PerformanceControls,
                    Widget::PerformanceStatus => WidgetView::PerformanceStatus,
                    Widget::PatternGrid => WidgetView::PatternGrid,
                    Widget::TransportStatus => WidgetView::TransportStatus,
                    Widget::PerformanceSurface(surface) => {
                        let mut tracks = Vec::with_capacity(surface.tracks.len());
                        for track in &surface.tracks {
                            let row = |id: &str| -> Result<ControlRow, String> {
                                let (handle, authored) = controls
                                    .get(id)
                                    .ok_or_else(|| format!("unknown control '{id}'"))?;
                                Ok(ControlRow::new(handle.clone(), *authored))
                            };
                            tracks.push(TrackView {
                                label: track.label.clone(),
                                volume: row(&track.volume_control)?,
                                controls: track
                                    .control_ids
                                    .iter()
                                    .map(|id| row(id))
                                    .collect::<Result<Vec<_>, _>>()?,
                                color: palette_index(&track.color).ok_or_else(|| {
                                    format!("unknown Lospec 500 color '{}'", track.color)
                                })?,
                                melodic: track.melodic,
                            });
                        }
                        WidgetView::PerformanceSurface(SurfaceView {
                            title: surface.title.clone(),
                            tracks,
                            activity: std::cell::RefCell::new(Default::default()),
                        })
                    }
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
            ids: ids
                .into_iter()
                .map(|(id, index)| (id.to_owned(), index))
                .collect(),
            active: 0,
        })
    }

    pub fn current(&self) -> &ScreenView {
        &self.screens[self.active]
    }

    pub fn position(&self) -> (usize, usize) {
        (self.active + 1, self.screens.len())
    }

    pub fn activate_screen(&mut self, id: &str) -> Result<(), String> {
        self.active = *self
            .ids
            .get(id)
            .ok_or_else(|| format!("unknown screen '{id}'"))?;
        Ok(())
    }

    pub fn has_pattern_grid(&self) -> bool {
        self.current().widgets.iter().any(|widget| {
            matches!(
                widget,
                WidgetView::PatternGrid | WidgetView::PerformanceSurface(_)
            )
        })
    }

    pub fn has_performance_controls(&self) -> bool {
        self.current()
            .widgets
            .iter()
            .any(|widget| matches!(widget, WidgetView::PerformanceControls))
    }

    pub fn switch(&mut self, direction: i32) {
        let len = self.screens.len();
        self.active = if direction < 0 {
            (self.active + len - 1) % len
        } else {
            (self.active + 1) % len
        };
    }

    pub fn activate(&mut self) -> Result<(), String> {
        let Some(selected) = self.current().selected else {
            return Ok(());
        };
        match &self.current().widgets[selected] {
            WidgetView::Link { target, .. } => self.active = *target,
            WidgetView::Button { row, value, .. } => row.publish(*value)?,
            _ => {}
        }
        Ok(())
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
    use crate::{compiler::compile, lua, navigation::NavigationRuntime, panel::Gesture};

    const NAVIGATED_INSTRUMENT: &str = r#"
        local instrument = meldritch.instrument("Navigation test")
        local tone = instrument:oscillator { waveform = "sine", frequency = 110 }
        instrument:connect(tone.out, instrument.output.left)
        instrument:connect(tone.out, instrument.output.right)
        local sequence = instrument:sequencer {
          bpm = 120, steps = 16, steps_per_beat = 4,
        }
        sequence:track("Pulse")
        sequence:steps("Pulse", { 1,0,0,0, 1,0,0,0, 1,0,0,0, 1,0,0,0 })
        instrument:control("Tempo", sequence.bpm, { min = 60, max = 220 })
        instrument:performance { sequencer = sequence, layout = "step_grid" }
        local alpha = instrument:screen("alpha", "Alpha")
        alpha:performance_controls()
        alpha:performance_status()
        local beta = instrument:screen("beta", "Beta")
        beta:pattern_grid()
        beta:transport_status()
        local navigation = instrument:navigation {
          initial = "alpha",
          events = {
            { name = "show_alpha", from = "*", to = "alpha" },
            { name = "show_beta", from = "*", to = "beta" },
          },
        }
        navigation:bind("F1", "show_alpha")
        navigation:bind("F2", "show_beta")
        return instrument
    "#;

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
        panel.activate().unwrap();
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

    #[test]
    fn lua_buttons_mute_and_restore_output() {
        let graph = lua::read(include_str!("../examples/67-screen-buttons.lua")).unwrap();
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
        panel.apply(Gesture::Next).unwrap();
        assert_eq!(panel.current().selected, Some(2));
        panel.activate().unwrap();
        let mut samples = [f64::NAN; 1];
        machine.process(&mut samples);
        assert_eq!(samples, [0.0]);
        panel.apply(Gesture::Next).unwrap();
        assert_eq!(panel.current().selected, Some(3));
        panel.activate().unwrap();
        machine.process(&mut samples);
        assert_eq!(samples, [0.1875]);
    }

    #[test]
    fn lua_navigation_switches_screens_without_changing_music() {
        let graph = lua::read(NAVIGATED_INSTRUMENT).unwrap();
        let navigation = NavigationRuntime::new(graph.navigation.as_ref().unwrap()).unwrap();
        let sequence = graph.performance.as_ref().unwrap().sequencer.clone();
        let authored_pattern = sequence.pattern().velocities[0];
        let mut machine = compile(&graph).unwrap();
        machine.process(&mut [0.0]);
        let controls = machine
            .control_ids()
            .map(|id| {
                let handle = machine.control_handle(id).unwrap();
                let authored = machine.effective_value(handle.target()).unwrap();
                (handle, authored)
            })
            .collect();
        let mut panel = ScreenPanel::new(machine.screens(), controls).unwrap();
        panel.activate_screen(navigation.initial_screen()).unwrap();
        assert_eq!(panel.current().title, "Alpha");

        sequence.stop();
        machine.process(&mut [0.0]);
        assert!(!sequence.playing());
        let target = navigation.dispatch("F2").unwrap().unwrap();
        panel.activate_screen(&target).unwrap();
        assert_eq!(panel.current().title, "Beta");
        assert_eq!(sequence.pattern().velocities[0], authored_pattern);
        assert!(!sequence.playing());

        assert_eq!(navigation.dispatch("unrelated").unwrap(), None);
        let target = navigation.dispatch("F1").unwrap().unwrap();
        panel.activate_screen(&target).unwrap();
        assert_eq!(panel.current().title, "Alpha");
        let target = navigation.dispatch("F1").unwrap().unwrap();
        panel.activate_screen(&target).unwrap();
        assert_eq!(panel.current().title, "Alpha");
        assert!(
            panel
                .activate_screen("missing")
                .unwrap_err()
                .contains("unknown screen")
        );
        assert_eq!(sequence.pattern().velocities[0], authored_pattern);
        assert!(!sequence.playing());
    }
}
