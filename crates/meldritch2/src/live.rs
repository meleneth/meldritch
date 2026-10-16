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
        let activated = self.begin_buffer();
        self.process_prepared(buffer);
        activated
    }

    pub(crate) fn begin_buffer(&mut self) -> bool {
        let mut activated = false;
        if let Ok(mut handoff) = self.handoff.try_lock() {
            if handoff.retired.is_none() {
                if let Some(candidate) = handoff.pending.take() {
                    handoff.retired = Some(std::mem::replace(&mut self.current, candidate));
                    activated = true;
                }
            }
        }
        self.current.prepare_buffer();
        activated
    }

    pub(crate) fn process_prepared(&mut self, buffer: &mut [f64]) {
        self.current.process_prepared(buffer);
    }

    pub fn current(&self) -> &Machine {
        &self.current
    }

    pub(crate) fn current_mut(&mut self) -> &mut Machine {
        &mut self.current
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
    use crate::{compiler::compile, lua, model::ParameterId};

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

    #[test]
    fn chunked_callback_keeps_one_revision_and_parameter_snapshot() {
        let basic = lua::read(include_str!("../examples/01-basic.lua")).unwrap();
        let mix = lua::read(include_str!("../examples/05-mix.lua")).unwrap();
        let (mut runtime, control) = LiveRuntime::new(compile(&basic).unwrap());
        let gain = runtime
            .current()
            .parameter_handle(&ParameterId::new("gain", "gain"))
            .unwrap();

        assert!(!runtime.begin_buffer());
        let mut first_chunk = [f64::NAN; 256];
        runtime.process_prepared(&mut first_chunk);
        assert_eq!(first_chunk, [0.125; 256]);

        gain.publish(Some(0.75)).unwrap();
        control.submit(&mix).unwrap();
        let mut second_chunk = [f64::NAN; 44];
        runtime.process_prepared(&mut second_chunk);
        assert_eq!(second_chunk, [0.125; 44]);

        let mut next_buffer = [f64::NAN; 4];
        assert!(runtime.process(&mut next_buffer));
        assert_eq!(next_buffer, [0.1875; 4]);
    }
}
