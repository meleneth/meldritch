use std::sync::{Arc, Mutex};

use crate::{
    compiler::{Machine, compile_at_rate},
    model::Graph,
};

#[derive(Default)]
struct Handoff {
    pending: Option<Machine>,
    retired: Option<Machine>,
}

/// The processor owns its mutable machine. Control shares only a short handoff.
pub struct LiveRuntime {
    current: Machine,
    handoff: Arc<Mutex<Handoff>>,
}

/// Compile and queue complete graph revisions off the processing path.
pub struct LiveControl {
    handoff: Arc<Mutex<Handoff>>,
    sample_rate: f64,
}

impl LiveRuntime {
    pub fn new(initial: Machine) -> (Self, LiveControl) {
        let handoff = Arc::new(Mutex::new(Handoff::default()));
        let sample_rate = initial.sample_rate();
        (
            Self {
                current: initial,
                handoff: Arc::clone(&handoff),
            },
            LiveControl {
                handoff,
                sample_rate,
            },
        )
    }

    /// Returns whether a queued revision was activated at this buffer boundary.
    /// A busy handoff is skipped, never waited on by the processing thread.
    pub fn process(&mut self, buffer: &mut [f64]) -> bool {
        let mut activated = false;
        if let Ok(mut handoff) = self.handoff.try_lock() {
            if handoff.retired.is_none() {
                if let Some(candidate) = handoff.pending.take() {
                    handoff.retired = Some(std::mem::replace(&mut self.current, candidate));
                    activated = true;
                }
            }
        }
        self.current.process(buffer);
        activated
    }

    pub fn current(&self) -> &Machine {
        &self.current
    }
}

impl LiveControl {
    pub fn submit(&self, graph: &Graph) -> Result<(), String> {
        let candidate = compile_at_rate(graph, self.sample_rate)?;
        let mut handoff = self.handoff.lock().map_err(|_| "graph handoff poisoned")?;
        if handoff.pending.is_some() {
            return Err("a graph revision is already pending".into());
        }
        if handoff.retired.is_some() {
            return Err(
                "retired graph revision must be reclaimed before another submission".into(),
            );
        }
        handoff.pending = Some(candidate);
        Ok(())
    }

    /// Remove the old machine for destruction on the control side.
    pub fn reclaim(&self) -> Result<Option<Machine>, String> {
        let mut handoff = self.handoff.lock().map_err(|_| "graph handoff poisoned")?;
        Ok(handoff.retired.take())
    }
}

#[cfg(test)]
mod tests {
    use super::LiveRuntime;
    use crate::{compiler::compile, lua};

    #[test]
    fn busy_handoff_skips_without_waiting_or_losing_candidate() {
        let basic = lua::read(include_str!("../examples/01-basic.lua")).unwrap();
        let mix = lua::read(include_str!("../examples/05-mix.lua")).unwrap();
        let (mut runtime, control) = LiveRuntime::new(compile(&basic).unwrap());
        control.submit(&mix).unwrap();
        let held = control.handoff.lock().unwrap();
        let mut samples = [0.0; 4];
        assert!(!runtime.process(&mut samples));
        assert_eq!(samples, [0.125; 4]);
        drop(held);
        assert!(runtime.process(&mut samples));
        assert_eq!(samples, [0.1875; 4]);
    }
}
