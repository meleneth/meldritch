//! Shared authored pattern state and audio-domain sequencer transport.

use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicU8, Ordering},
};

pub const MAX_TRACKS: usize = 8;
pub const MAX_STEPS: usize = 64;

#[derive(Clone, Copy, Debug)]
pub struct Pattern {
    pub velocities: [[f32; MAX_STEPS]; MAX_TRACKS],
}

impl Default for Pattern {
    fn default() -> Self {
        Self {
            velocities: [[0.0; MAX_STEPS]; MAX_TRACKS],
        }
    }
}

#[derive(Debug)]
struct PublishedPattern {
    authored: Pattern,
    pending: Option<Pattern>,
}

/// Control-side state shared by Lua, TUI, MIDI and the compiled audio runtime.
#[derive(Debug)]
pub struct SequencerControl {
    pub name: String,
    pub steps: usize,
    pub steps_per_beat: u32,
    pub bpm: std::sync::atomic::AtomicU64,
    track_names: Mutex<Vec<String>>,
    pattern: Mutex<PublishedPattern>,
    command: AtomicU8,
    playing: AtomicBool,
    current_step: AtomicU8,
}

impl SequencerControl {
    pub fn new(
        name: String,
        bpm: f64,
        steps: usize,
        steps_per_beat: u32,
    ) -> Result<Arc<Self>, String> {
        if !(1..=MAX_STEPS).contains(&steps) {
            return Err(format!("sequencer steps must be 1..={MAX_STEPS}"));
        }
        if steps_per_beat == 0 || !bpm.is_finite() || bpm <= 0.0 {
            return Err("sequencer requires positive BPM and resolution".into());
        }
        Ok(Arc::new(Self {
            name,
            steps,
            steps_per_beat,
            bpm: std::sync::atomic::AtomicU64::new(bpm.to_bits()),
            track_names: Mutex::new(Vec::new()),
            pattern: Mutex::new(PublishedPattern {
                authored: Pattern::default(),
                pending: None,
            }),
            command: AtomicU8::new(1),
            playing: AtomicBool::new(true),
            current_step: AtomicU8::new(0),
        }))
    }
    pub fn add_track(&self, name: String) -> Result<usize, String> {
        let mut names = self
            .track_names
            .lock()
            .map_err(|_| "sequencer track state poisoned")?;
        if names.len() >= MAX_TRACKS {
            return Err(format!("sequencer permits at most {MAX_TRACKS} tracks"));
        }
        if names.iter().any(|n| n == &name) {
            return Err(format!("duplicate sequencer track '{name}'"));
        }
        names.push(name);
        Ok(names.len() - 1)
    }
    pub fn track_names(&self) -> Vec<String> {
        self.track_names
            .lock()
            .map(|v| v.clone())
            .unwrap_or_default()
    }
    pub fn set_steps(&self, track: usize, values: &[f32]) -> Result<(), String> {
        if values.len() != self.steps {
            return Err(format!("pattern requires exactly {} steps", self.steps));
        }
        if values
            .iter()
            .any(|velocity| !velocity.is_finite() || !(0.0..=1.0).contains(velocity))
        {
            return Err("step velocities must be finite values in 0..1".into());
        }
        let mut state = self
            .pattern
            .lock()
            .map_err(|_| "sequencer pattern state poisoned")?;
        state.authored.velocities[track][..self.steps].copy_from_slice(values);
        state.pending = Some(state.authored);
        Ok(())
    }
    pub fn toggle(&self, track: usize, step: usize) -> Result<(), String> {
        if track >= self.track_names().len() || step >= self.steps {
            return Err("sequencer cell is out of range".into());
        }
        let mut state = self
            .pattern
            .lock()
            .map_err(|_| "sequencer pattern state poisoned")?;
        let cell = &mut state.authored.velocities[track][step];
        *cell = if *cell > 0.0 { 0.0 } else { 1.0 };
        state.pending = Some(state.authored);
        Ok(())
    }
    pub fn clear_track(&self, track: usize) -> Result<(), String> {
        let mut state = self
            .pattern
            .lock()
            .map_err(|_| "sequencer pattern state poisoned")?;
        state.authored.velocities[track][..self.steps].fill(0.0);
        state.pending = Some(state.authored);
        Ok(())
    }
    pub fn pattern(&self) -> Pattern {
        self.pattern.lock().map(|s| s.authored).unwrap_or_default()
    }
    pub fn publish_bpm(&self, bpm: f64) -> Result<(), String> {
        if !bpm.is_finite() || bpm <= 0.0 {
            return Err("BPM must be positive".into());
        }
        self.bpm.store(bpm.to_bits(), Ordering::Release);
        Ok(())
    }
    pub fn bpm(&self) -> f64 {
        f64::from_bits(self.bpm.load(Ordering::Acquire))
    }
    pub fn start(&self) {
        self.command.store(1, Ordering::Release);
    }
    pub fn stop(&self) {
        self.command.store(2, Ordering::Release);
    }
    pub fn restart(&self) {
        self.command.store(3, Ordering::Release);
    }
    pub fn toggle_transport(&self) {
        if self.playing() {
            self.stop()
        } else {
            self.start()
        }
    }
    pub fn playing(&self) -> bool {
        self.playing.load(Ordering::Acquire)
    }
    pub fn current_step(&self) -> usize {
        self.current_step.load(Ordering::Acquire) as usize
    }
}

#[derive(Debug)]
pub(crate) struct SequencerRuntime {
    control: Arc<SequencerControl>,
    pattern: Pattern,
    step: usize,
    frames_until_step: f64,
    trigger_step: Option<usize>,
    initialized: bool,
}

impl SequencerRuntime {
    pub fn new(control: Arc<SequencerControl>) -> Self {
        Self {
            control,
            pattern: Pattern::default(),
            step: 0,
            frames_until_step: 0.0,
            trigger_step: None,
            initialized: false,
        }
    }
    pub fn begin_frame(&mut self, sample_rate: f64) {
        match self.control.command.swap(0, Ordering::AcqRel) {
            1 => {
                self.control.playing.store(true, Ordering::Release);
                if !self.initialized {
                    self.frames_until_step = 0.0;
                }
            }
            2 => self.control.playing.store(false, Ordering::Release),
            3 => {
                self.step = 0;
                self.frames_until_step = 0.0;
                self.control.playing.store(true, Ordering::Release);
            }
            _ => {}
        }
        self.trigger_step = None;
        if !self.control.playing.load(Ordering::Acquire) {
            return;
        }
        if self.frames_until_step <= 0.0 {
            if let Ok(mut state) = self.control.pattern.try_lock()
                && let Some(next) = state.pending.take()
            {
                self.pattern = next;
            }
            self.trigger_step = Some(self.step);
            self.control
                .current_step
                .store(self.step as u8, Ordering::Release);
            self.step = (self.step + 1) % self.control.steps;
            let bpm = self.control.bpm().max(1.0);
            self.frames_until_step +=
                sample_rate * 60.0 / (bpm * self.control.steps_per_beat as f64);
            self.initialized = true;
        }
        self.frames_until_step -= 1.0;
    }
    pub fn trigger(&self, track: usize) -> f64 {
        self.trigger_step
            .map_or(0.0, |s| self.pattern.velocities[track][s] as f64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (Arc<SequencerControl>, SequencerRuntime) {
        let control = SequencerControl::new("test".into(), 120.0, 16, 4).unwrap();
        control.add_track("Kick".into()).unwrap();
        control.add_track("Snare".into()).unwrap();
        control
            .set_steps(
                0,
                &[
                    1.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
                ],
            )
            .unwrap();
        control
            .set_steps(
                1,
                &[
                    1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
                ],
            )
            .unwrap();
        (Arc::clone(&control), SequencerRuntime::new(control))
    }
    #[test]
    fn boundaries_are_sample_accurate_and_simultaneous() {
        let (_, mut runtime) = fixture();
        let mut events = Vec::new();
        for frame in 0..250 {
            runtime.begin_frame(800.0);
            if runtime.trigger(0) > 0.0 {
                events.push((frame, 0));
            }
            if runtime.trigger(1) > 0.0 {
                events.push((frame, 1));
            }
        }
        assert_eq!(events, vec![(0, 0), (0, 1), (200, 0)]);
    }
    #[test]
    fn tempo_stop_restart_and_boundary_edits_are_coherent() {
        let (control, mut runtime) = fixture();
        runtime.begin_frame(800.0);
        control.stop();
        for _ in 0..200 {
            runtime.begin_frame(800.0);
            assert_eq!(runtime.trigger(0), 0.0);
        }
        control
            .set_steps(
                0,
                &[
                    0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
                ],
            )
            .unwrap();
        control.publish_bpm(240.0).unwrap();
        control.restart();
        runtime.begin_frame(800.0);
        assert_eq!(runtime.trigger(0), 0.0);
        for _ in 0..49 {
            runtime.begin_frame(800.0);
        }
        runtime.begin_frame(800.0);
        assert_eq!(runtime.trigger(0), 1.0);
    }
}
