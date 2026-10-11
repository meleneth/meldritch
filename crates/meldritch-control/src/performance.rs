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
        Ok(processed)
    }

    pub fn external_accepted(
        &mut self,
        app: &AppController,
        _result: &AppCommandResult,
    ) -> Result<()> {
        self.plane.refresh(app)
    }
}
