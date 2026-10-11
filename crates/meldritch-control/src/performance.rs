//! Lua host wiring shared with the headless acceptance harness.
use crate::session::PerformanceSessionCapture;
use crate::song_host::SongAudioHost;
use crate::{ControlPlane, Input, LuaProject, Result};
use meldritch_app::{AppCommandResult, AppController};
use meldritch_core::Tempo;

pub struct LuaPerformance {
    pub plane: ControlPlane,
    tempo: Tempo,
}

impl LuaPerformance {
    pub(crate) fn tempo(&self) -> Tempo {
        self.tempo
    }
    pub fn pad(
        &mut self,
        source: &str,
        cell: u8,
        pressed: bool,
        frame: u64,
        app: &mut AppController,
        audio: &mut SongAudioHost,
        capture: &mut PerformanceSessionCapture,
    ) -> Result<usize> {
        u32::try_from(frame)
            .map_err(|_| "legacy session frame exceeds u32::MAX; input rejected before mutation")?;
        self.plane
            .enqueue(source, frame, Input::Pad { cell, pressed })?;
        self.pump(app, audio, capture)
    }
    pub fn cancel(
        &mut self,
        source: &str,
        app: &mut AppController,
        audio: &mut SongAudioHost,
        capture: &mut PerformanceSessionCapture,
    ) -> Result<()> {
        let frame = u64::from(app.view_model().transport.position);
        self.plane.interrupt_surface(source, frame)?;
        self.pump(app, audio, capture)?;
        Ok(())
    }
    pub fn new(project: LuaProject, app: &AppController, tempo: Tempo) -> Result<Self> {
        Ok(Self {
            plane: ControlPlane::new(project, app)?,
            tempo,
        })
    }

    /// Run on the TUI/control owner only. MIDI callbacks enqueue observations.
    pub fn press(
        &mut self,
        source: &str,
        cell: u8,
        frame: u64,
        app: &mut AppController,
        audio: &mut SongAudioHost,
        capture: &mut PerformanceSessionCapture,
    ) -> Result<usize> {
        u32::try_from(frame)
            .map_err(|_| "legacy session frame exceeds u32::MAX; input rejected before mutation")?;
        self.plane.enqueue(source, frame, Input::Press(cell))?;
        self.pump(app, audio, capture)
    }

    fn pump(
        &mut self,
        app: &mut AppController,
        audio: &mut SongAudioHost,
        capture: &mut PerformanceSessionCapture,
    ) -> Result<usize> {
        let processed =
            self.plane
                .pump_with_observer(app, |app, request, result, source, frame| {
                    let recorded =
                        capture.record_command(app, request, result, source, frame, self.tempo);
                    let scheduled = audio.accepted(&app.view_model(), result);
                    recorded?;
                    let scheduled = scheduled?;
                    Ok(Some(if scheduled {
                        format!("session fact captured; song audio generation {} queued; older audio work superseded", audio.generation())
                    } else {
                        "session fact captured; no song rerender requested".to_owned()
                    }))
                });
        // Retain the lossless outbox if persistence fails. Checkpoint outside RT.
        capture.checkpoint()?;
        self.plane.take_journal();
        audio.refresh_view(app);
        self.plane.refresh(app)?;
        Ok(processed)
    }

    pub fn external_accepted(
        &mut self,
        app: &AppController,
        _result: &AppCommandResult,
    ) -> Result<()> {
        self.plane.refresh(app)
    }

    pub fn connection(
        &mut self,
        source: &str,
        connected: bool,
        app: &mut AppController,
        audio: &mut SongAudioHost,
        capture: &mut PerformanceSessionCapture,
    ) -> Result<()> {
        let frame = u64::from(app.view_model().transport.position);
        self.plane
            .enqueue(source, frame, Input::Connection(connected))?;
        // Pending causal work must keep its recording/render observer even
        // when the next dispatch is a connection lifecycle observation.
        self.pump(app, audio, capture)?;
        Ok(())
    }
}
