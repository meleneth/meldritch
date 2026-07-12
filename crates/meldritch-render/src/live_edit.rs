//! Typed control-side pattern edits for realtime rendering.

use crate::coordinator::{RenderCoordinator, RenderStateUpdateError, SampleRenderState};
use meldritch_core::{DirtyRange, EntityId, FrameRange, PatternError, Step, StepIndex, TrackId};
use std::collections::BTreeMap;
use std::sync::Arc;

#[derive(Clone, Debug, PartialEq)]
pub enum LiveEditCommand {
    SetStep {
        track: TrackId,
        step: StepIndex,
        value: Step,
    },
    ClearStep {
        track: TrackId,
        step: StepIndex,
    },
    ToggleStep {
        track: TrackId,
        step: StepIndex,
        value: Step,
    },
}

impl LiveEditCommand {
    const fn location(&self) -> (TrackId, StepIndex) {
        match self {
            Self::SetStep { track, step, .. }
            | Self::ClearStep { track, step }
            | Self::ToggleStep { track, step, .. } => (*track, *step),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct LiveEditResult {
    pub command: LiveEditCommand,
    pub changed: bool,
    pub dirty_ranges: Vec<DirtyRange>,
    pub invalidated_chunks: usize,
}

#[derive(Debug)]
pub enum LiveEditError {
    Pattern(PatternError),
    IncompatiblePattern,
    RenderState(RenderStateUpdateError),
}

pub struct LivePatternEditor {
    state: SampleRenderState,
    timeline_frames: u32,
}

impl LivePatternEditor {
    #[must_use]
    pub const fn new(state: SampleRenderState, timeline_frames: u32) -> Self {
        Self {
            state,
            timeline_frames,
        }
    }

    #[must_use]
    pub const fn state(&self) -> &SampleRenderState {
        &self.state
    }

    pub fn apply(
        &mut self,
        coordinator: &RenderCoordinator,
        command: LiveEditCommand,
    ) -> Result<LiveEditResult, LiveEditError> {
        let (track, step) = command.location();
        let old_step = self.state.pattern().get_step(track, step).cloned();
        let mut pattern = self.state.pattern().clone();
        let changed = match &command {
            LiveEditCommand::SetStep { value, .. } => {
                pattern
                    .set_step(track, step, value.clone())
                    .map_err(LiveEditError::Pattern)?
                    != Some(value.clone())
            }
            LiveEditCommand::ClearStep { .. } => pattern
                .clear_step(track, step)
                .map_err(LiveEditError::Pattern)?
                .is_some(),
            LiveEditCommand::ToggleStep { value, .. } => {
                if old_step.is_some() {
                    pattern
                        .clear_step(track, step)
                        .map_err(LiveEditError::Pattern)?;
                } else {
                    pattern
                        .set_step(track, step, value.clone())
                        .map_err(LiveEditError::Pattern)?;
                }
                true
            }
        };
        if !changed {
            return Ok(LiveEditResult {
                command,
                changed: false,
                dirty_ranges: Vec::new(),
                invalidated_chunks: 0,
            });
        }

        let new_step = pattern.get_step(track, step).cloned();
        let dirty_ranges = self.dirty_ranges(track, step, old_step.as_ref(), new_step.as_ref())?;
        let frame_ranges = dirty_ranges
            .iter()
            .map(DirtyRange::range)
            .collect::<Vec<_>>();
        let next_state = self.state.with_pattern(pattern);
        let invalidated_chunks = coordinator
            .update_render_state_ranges(next_state.clone(), &frame_ranges)
            .map_err(LiveEditError::RenderState)?;
        self.state = next_state;
        Ok(LiveEditResult {
            command,
            changed: true,
            dirty_ranges,
            invalidated_chunks,
        })
    }

    pub fn replace_samples(
        &mut self,
        coordinator: &RenderCoordinator,
        samples_by_note: Arc<BTreeMap<u8, meldritch_audio::SampleBuffer>>,
    ) -> Result<usize, LiveEditError> {
        let range = FrameRange::new(0, u64::from(self.timeline_frames))
            .expect("live editor timeline is ordered");
        let next_state = self.state.with_samples(samples_by_note);
        let invalidated = coordinator
            .update_render_state(next_state.clone(), range)
            .map_err(LiveEditError::RenderState)?;
        self.state = next_state;
        Ok(invalidated)
    }

    /// Replaces the repeating pattern as one control-side operation. Phrase
    /// launchers use this only for layout-compatible patterns at quantized
    /// boundaries, allowing the coordinator to rebuild the complete horizon.
    pub fn replace_pattern(
        &mut self,
        coordinator: &RenderCoordinator,
        pattern: meldritch_core::Pattern,
    ) -> Result<usize, LiveEditError> {
        if pattern.length_steps() != self.state.pattern().length_steps()
            || pattern.steps_per_beat() != self.state.pattern().steps_per_beat()
        {
            return Err(LiveEditError::IncompatiblePattern);
        }
        let pattern = pattern.reidentified(self.state.pattern().id());
        if pattern == *self.state.pattern() {
            return Ok(0);
        }
        let range = FrameRange::new(0, u64::from(self.timeline_frames))
            .expect("live editor timeline is ordered");
        let next_state = self.state.with_pattern(pattern);
        let invalidated = coordinator
            .update_render_state(next_state.clone(), range)
            .map_err(LiveEditError::RenderState)?;
        self.state = next_state;
        Ok(invalidated)
    }

    pub fn replace_bass_synth(
        &mut self,
        coordinator: &RenderCoordinator,
        track: TrackId,
        settings: crate::dsp::BassVoiceSettings,
        samples_by_note: Arc<BTreeMap<u8, meldritch_audio::SampleBuffer>>,
    ) -> Result<usize, LiveEditError> {
        let range = FrameRange::new(0, u64::from(self.timeline_frames))
            .expect("live editor timeline is ordered");
        let next_state = self
            .state
            .with_samples(samples_by_note)
            .with_bass_layer(track, settings);
        let invalidated = coordinator
            .update_render_state(next_state.clone(), range)
            .map_err(LiveEditError::RenderState)?;
        self.state = next_state;
        Ok(invalidated)
    }

    fn dirty_ranges(
        &self,
        track: TrackId,
        step: StepIndex,
        old_step: Option<&Step>,
        new_step: Option<&Step>,
    ) -> Result<Vec<DirtyRange>, LiveEditError> {
        let pattern = self.state.pattern();
        let tempo = self.state.tempo();
        let pattern_frames =
            tempo.step_start_frame(u64::from(pattern.length_steps()), pattern.steps_per_beat());
        let sample_tail = old_step
            .into_iter()
            .chain(new_step)
            .filter_map(|value| self.state.samples_by_note().get(&value.note()))
            .map(meldritch_audio::SampleBuffer::frames)
            .max()
            .unwrap_or(0);
        let synth_settings = self
            .state
            .bass_layer()
            .filter(|(bass_track, _)| *bass_track == track)
            .map(|(_, settings)| settings)
            .or_else(|| {
                self.state.chord_layer().and_then(|chord| {
                    (chord.first_track <= track && track <= chord.last_track)
                        .then_some(chord.settings)
                })
            });
        let synth_impact = synth_settings.map_or(0, |settings| {
            let step_start =
                tempo.step_start_frame(u64::from(step.raw()), pattern.steps_per_beat());
            let next_start =
                tempo.step_start_frame(u64::from(step.raw()) + 1, pattern.steps_per_beat());
            let step_frames = next_start - step_start;
            let gate = old_step
                .into_iter()
                .chain(new_step)
                .map(Step::gate)
                .fold(0.0, f64::max)
                .clamp(0.0, 1.0);
            let gate_frames = (step_frames as f64 * gate).round() as u64;
            let release_frames =
                (settings.release_seconds.max(0.0) * f64::from(tempo.sample_rate())).round() as u64;
            gate_frames.saturating_add(release_frames)
        });
        let mut ranges = Vec::new();
        let mut cycle = 0_u64;
        while cycle.saturating_mul(pattern_frames) < u64::from(self.timeline_frames) {
            let dirty = pattern
                .step_dirty_range(tempo, step, cycle)
                .map_err(LiveEditError::Pattern)?;
            let start = dirty.range().start();
            if start >= u64::from(self.timeline_frames) {
                break;
            }
            let impact_frames = u64::from(sample_tail.max(1)).max(synth_impact.max(1));
            let end = start
                .saturating_add(impact_frames)
                .min(u64::from(self.timeline_frames));
            let range = FrameRange::new(start, end).expect("live edit dirty range is ordered");
            ranges.push(DirtyRange::new(EntityId::Pattern(pattern.id()), range));
            cycle += 1;
        }
        Ok(ranges)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RenderSettings;
    use crate::coordinator::{ChordLayer, RenderCoordinatorConfig};
    use crate::dsp::BassVoiceSettings;
    use meldritch_audio::SampleBuffer;
    use meldritch_audio::realtime_status::realtime_status;
    use meldritch_core::{Pattern, PatternId, ProbabilitySeed, Tempo};
    use std::collections::BTreeMap;
    use std::sync::Arc;
    use std::time::Duration;

    #[test]
    fn typed_step_edits_rebuild_only_the_sample_impact_range() {
        let pattern = Pattern::new(PatternId::new(1), 4, 4).unwrap();
        let mut samples = BTreeMap::new();
        samples.insert(36, SampleBuffer::new(1, 48_000, vec![0.6; 3]));
        let state = SampleRenderState::new(
            pattern,
            Tempo::new(120.0, 48_000).unwrap(),
            ProbabilitySeed::new(1),
            RenderSettings::new(1).unwrap(),
            Arc::new(samples),
        );
        let (_, status, _) = realtime_status();
        let config = RenderCoordinatorConfig::new(2, 8, 2, 1, Duration::from_millis(2)).unwrap();
        let mut coordinator = RenderCoordinator::new(
            config,
            state.pattern().clone(),
            state.tempo(),
            state.probability_seed(),
            state.settings(),
            Arc::clone(state.samples_by_note()),
            status,
        )
        .unwrap();
        let mut editor = LivePatternEditor::new(state, 8);
        assert!(coordinator.wait_for_ready_chunks(4, Duration::from_secs(1)));

        let set = editor
            .apply(
                &coordinator,
                LiveEditCommand::SetStep {
                    track: TrackId::new(1),
                    step: StepIndex::new(0),
                    value: Step::new(36),
                },
            )
            .unwrap();
        assert!(set.changed);
        assert_eq!(set.dirty_ranges.len(), 1);
        assert_eq!(set.dirty_ranges[0].range(), FrameRange::new(0, 3).unwrap());
        assert_eq!(set.invalidated_chunks, 2);
        assert!(coordinator.wait_for_ready_chunks(4, Duration::from_secs(1)));
        let rendered = coordinator.audio_reader().snapshot();
        assert_eq!(rendered.frame(0), Ok([0.6].as_slice()));
        assert_eq!(rendered.frame(2), Ok([0.6].as_slice()));
        assert_eq!(rendered.frame(3), Ok([0.0].as_slice()));

        let unchanged = editor
            .apply(
                &coordinator,
                LiveEditCommand::SetStep {
                    track: TrackId::new(1),
                    step: StepIndex::new(0),
                    value: Step::new(36),
                },
            )
            .unwrap();
        assert!(!unchanged.changed);
        assert!(unchanged.dirty_ranges.is_empty());

        let toggled = editor
            .apply(
                &coordinator,
                LiveEditCommand::ToggleStep {
                    track: TrackId::new(1),
                    step: StepIndex::new(0),
                    value: Step::new(36),
                },
            )
            .unwrap();
        assert!(toggled.changed);
        assert!(coordinator.wait_for_ready_chunks(4, Duration::from_secs(1)));
        assert_eq!(
            coordinator.audio_reader().snapshot().frame(0),
            Ok([0.0].as_slice())
        );
        coordinator.shutdown();
    }

    #[test]
    fn replacing_generated_samples_rebuilds_the_complete_timeline() {
        let mut pattern = Pattern::new(PatternId::new(1), 4, 4).unwrap();
        pattern
            .set_step(TrackId::new(1), StepIndex::new(0), Step::new(24))
            .unwrap();
        let mut initial_samples = BTreeMap::new();
        initial_samples.insert(24, SampleBuffer::new(1, 48_000, vec![0.2; 8]));
        let state = SampleRenderState::new(
            pattern,
            Tempo::new(120.0, 48_000).unwrap(),
            ProbabilitySeed::new(1),
            RenderSettings::new(1).unwrap(),
            Arc::new(initial_samples),
        );
        let (_, status, _) = realtime_status();
        let config = RenderCoordinatorConfig::new(2, 8, 2, 1, Duration::from_millis(2)).unwrap();
        let mut coordinator = RenderCoordinator::new(
            config,
            state.pattern().clone(),
            state.tempo(),
            state.probability_seed(),
            state.settings(),
            Arc::clone(state.samples_by_note()),
            status,
        )
        .unwrap();
        let mut editor = LivePatternEditor::new(state, 8);
        assert!(coordinator.wait_for_ready_chunks(4, Duration::from_secs(1)));

        let mut replacement = BTreeMap::new();
        replacement.insert(24, SampleBuffer::new(1, 48_000, vec![0.7; 8]));
        assert_eq!(
            editor
                .replace_samples(&coordinator, Arc::new(replacement))
                .unwrap(),
            4
        );
        assert!(coordinator.wait_for_ready_chunks(4, Duration::from_secs(1)));
        assert_eq!(
            coordinator.audio_reader().snapshot().frame(0),
            Ok([0.7].as_slice())
        );
        coordinator.shutdown();
    }

    #[test]
    fn replacing_phrase_pattern_rebuilds_horizon_and_rejects_layout_changes() {
        let pattern = Pattern::new(PatternId::new(1), 4, 4).unwrap();
        let mut samples = BTreeMap::new();
        samples.insert(36, SampleBuffer::new(1, 48_000, vec![0.8; 2]));
        let state = SampleRenderState::new(
            pattern,
            Tempo::new(120.0, 48_000).unwrap(),
            ProbabilitySeed::new(1),
            RenderSettings::new(1).unwrap(),
            Arc::new(samples),
        );
        let (_, status, _) = realtime_status();
        let config = RenderCoordinatorConfig::new(1, 8, 2, 1, Duration::from_millis(2)).unwrap();
        let mut coordinator =
            RenderCoordinator::new_from_state(config, state.clone(), status).unwrap();
        let mut editor = LivePatternEditor::new(state, 8);
        assert!(coordinator.wait_for_ready_chunks(4, Duration::from_secs(1)));

        let mut phrase = Pattern::new(PatternId::new(2), 4, 4).unwrap();
        phrase
            .set_step(TrackId::new(1), StepIndex::new(0), Step::new(36))
            .unwrap();
        assert_eq!(editor.replace_pattern(&coordinator, phrase).unwrap(), 4);
        assert!(coordinator.wait_for_ready_chunks(4, Duration::from_secs(1)));
        assert_eq!(editor.state().pattern().id(), PatternId::new(1));
        assert_eq!(
            coordinator.audio_reader().snapshot().frame(0),
            Ok([0.8].as_slice())
        );

        let incompatible = Pattern::new(PatternId::new(3), 8, 4).unwrap();
        assert!(matches!(
            editor.replace_pattern(&coordinator, incompatible),
            Err(LiveEditError::IncompatiblePattern)
        ));
        coordinator.shutdown();
    }

    #[test]
    fn chord_note_edit_invalidates_gate_and_release_tail() {
        let mut pattern = Pattern::new(PatternId::new(1), 4, 4).unwrap();
        pattern
            .set_step(TrackId::new(10), StepIndex::new(0), Step::new(60))
            .unwrap();
        let synth_settings = BassVoiceSettings::default();
        let state = SampleRenderState::new(
            pattern,
            Tempo::new(120.0, 48_000).unwrap(),
            ProbabilitySeed::new(1),
            RenderSettings::new(1).unwrap(),
            Arc::new(BTreeMap::new()),
        )
        .with_chord_layer(ChordLayer {
            first_track: TrackId::new(10),
            last_track: TrackId::new(12),
            settings: synth_settings,
            voice_count: 8,
        });
        let (_, status, _) = realtime_status();
        let config =
            RenderCoordinatorConfig::new(1, 12_000, 2_000, 1, Duration::from_millis(2)).unwrap();
        let mut coordinator =
            RenderCoordinator::new_from_state(config, state.clone(), status).unwrap();
        let mut editor = LivePatternEditor::new(state, 12_000);

        let edit = editor
            .apply(
                &coordinator,
                LiveEditCommand::SetStep {
                    track: TrackId::new(10),
                    step: StepIndex::new(0),
                    value: Step::new(61),
                },
            )
            .unwrap();
        let expected_end = 6_000 + (synth_settings.release_seconds * 48_000.0).round() as u64;
        assert_eq!(
            edit.dirty_ranges[0].range(),
            FrameRange::new(0, expected_end).unwrap()
        );
        coordinator.shutdown();
    }
}
