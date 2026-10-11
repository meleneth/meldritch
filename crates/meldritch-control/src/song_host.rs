use meldritch_app::{AppCommandResult, AppViewModel};
use meldritch_audio::audio_publication::AudioSnapshotPublisher;
use meldritch_dsl::{ParameterOwner, ParameterTargetDefinition, ValidatedSong};
use meldritch_render::song_host::*;
use std::collections::{BTreeMap, BTreeSet};

/// Control-thread owner of render caches; musical state belongs to AppController.
pub struct SongAudioHost {
    song: ValidatedSong,
    patch: CompiledSongPatch,
    overrides: SongLiveOverrides,
    targets: BTreeMap<String, LiveSongControlTarget>,
    worker: SongRerenderWorker,
    generation: u64,
    publisher: AudioSnapshotPublisher,
    chunk_frames: u32,
}

impl SongAudioHost {
    pub fn generation(&self) -> u64 {
        self.generation
    }
    pub fn new(
        song: ValidatedSong,
        patch: CompiledSongPatch,
        publisher: AudioSnapshotPublisher,
        frames: u32,
        chunk_frames: u32,
    ) -> Self {
        Self {
            targets: live_song_control_targets(&song),
            song,
            patch,
            overrides: SongLiveOverrides::default(),
            worker: SongRerenderWorker::new(frames),
            generation: 0,
            publisher,
            chunk_frames,
        }
    }

    pub fn replace_patch(&mut self, patch: CompiledSongPatch) {
        self.patch = patch;
        self.submit();
    }

    fn submit(&mut self) {
        self.generation += 1;
        self.worker
            .submit(self.generation, self.patch.clone(), self.overrides.clone());
    }

    /// Observe an already accepted command against the complete authoritative view.
    /// Compile errors preserve the last published audio and are returned to the host.
    pub fn accepted(
        &mut self,
        view: &AppViewModel,
        result: &AppCommandResult,
    ) -> Result<bool, String> {
        let recompile = match result {
            AppCommandResult::LaneOctaveSet {
                lane_id, current, ..
            } => {
                if *current == 0 {
                    self.overrides.lane_transpose_semitones.remove(lane_id);
                } else {
                    self.overrides
                        .lane_transpose_semitones
                        .insert(lane_id.clone(), current.saturating_mul(12));
                }
                true
            }
            AppCommandResult::LaneVariationSelected { .. }
            | AppCommandResult::LanePatternBankSelected { .. }
            | AppCommandResult::LaneMuteToggled { .. }
            | AppCommandResult::LaneSoloToggled { .. } => true,
            AppCommandResult::CuratedControlAdjusted { id, current, .. } => {
                let Some(target) = self.targets.get(id) else {
                    return Ok(false);
                };
                match target {
                    LiveSongControlTarget::DelayFeedback => {
                        self.overrides.feedback = Some(*current)
                    }
                    LiveSongControlTarget::DelayMix => self.overrides.mix = Some(*current),
                    LiveSongControlTarget::FilterCutoff(target) => {
                        self.overrides.cutoff_hz = Some(*current);
                        self.overrides
                            .synth_filters
                            .entry(target.clone())
                            .or_default()
                            .cutoff_hz = Some(*current);
                    }
                    LiveSongControlTarget::FilterResonance(target) => {
                        self.overrides.resonance = Some(*current);
                        self.overrides
                            .synth_filters
                            .entry(target.clone())
                            .or_default()
                            .resonance = Some(*current);
                    }
                    LiveSongControlTarget::LaneLevel(lane) => {
                        self.overrides.lane_levels.insert(lane.clone(), *current);
                    }
                }
                false
            }
            _ => return Ok(false),
        };
        if recompile {
            let variations = active_lane_variations(view);
            // Keep the delayed single-track path, including its DSP graph.
            self.patch = if self.song.performance().tracks().len() == 1
                && self.overrides.lane_transpose_semitones.is_empty()
                && included_lanes_from_view(view) == default_included_lanes(&self.song)
            {
                let pattern = variations
                    .values()
                    .next()
                    .ok_or("no active lane variation")?;
                let patch = meldritch_render::song_render::compile_delayed_note_song_for_pattern(
                    &self.song, pattern,
                )
                .map_err(|e| format!("failed to compile active song: {e:?}"))?;
                CompiledSongPatch::Delayed(patch)
            } else {
                compile_mixed_song_for_active_lanes(
                    &self.song,
                    &variations,
                    &self.overrides.lane_transpose_semitones,
                    &included_lanes_from_view(view),
                )?
            };
        }
        self.submit();
        Ok(true)
    }

    pub fn poll_publish(&mut self) -> Result<Option<u64>, String> {
        let Some((generation, block, _)) = self.worker.latest_completed() else {
            return Ok(None);
        };
        if generation != self.generation {
            return Ok(None);
        }
        let block = block.map_err(|e| format!("song render failed: {e:?}"))?;
        publish_song_audio(&self.publisher, &block, self.chunk_frames)?;
        Ok(Some(generation))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LiveSongControlTarget {
    DelayFeedback,
    DelayMix,
    FilterCutoff(SynthFilterTarget),
    FilterResonance(SynthFilterTarget),
    LaneLevel(String),
}

pub fn live_song_control_targets(
    song: &meldritch_dsl::ValidatedSong,
) -> BTreeMap<String, LiveSongControlTarget> {
    song.performance()
        .controls()
        .iter()
        .filter_map(|control| {
            let target = parameter_target_label(control.target());
            let target = match target.as_str() {
                "dsp:echo/delay.feedback" => LiveSongControlTarget::DelayFeedback,
                "dsp:echo/delay.mix" => LiveSongControlTarget::DelayMix,
                value if value.ends_with("/filter.cutoff_hz") => {
                    LiveSongControlTarget::FilterCutoff(SynthFilterTarget {
                        synth_id: control.target().definition_id().to_owned(),
                        module_id: control.target().module_id().to_owned(),
                    })
                }
                value if value.ends_with("/filter.resonance") => {
                    LiveSongControlTarget::FilterResonance(SynthFilterTarget {
                        synth_id: control.target().definition_id().to_owned(),
                        module_id: control.target().module_id().to_owned(),
                    })
                }
                value if value.starts_with("lane:") && value.ends_with("/level") => {
                    LiveSongControlTarget::LaneLevel(control.target().definition_id().to_owned())
                }
                _ => return None,
            };
            Some((control.id().to_owned(), target))
        })
        .collect()
}

pub fn song_controls_for_view(
    song: &meldritch_dsl::ValidatedSong,
    default_feedback: Option<f64>,
    default_mix: Option<f64>,
    default_cutoff_hz: Option<f64>,
    default_resonance: Option<f64>,
) -> Vec<meldritch_app::CuratedControlView> {
    song.performance()
        .controls()
        .iter()
        .map(|control| {
            let (minimum, maximum) = control.range();
            let target = parameter_target_label(control.target());
            let value = match target.as_str() {
                "dsp:echo/delay.feedback" => {
                    default_feedback.map(|feedback| feedback.clamp(minimum, maximum))
                }
                "dsp:echo/delay.mix" => default_mix.map(|mix| mix.clamp(minimum, maximum)),
                value if value.ends_with("/filter.cutoff_hz") => {
                    default_cutoff_hz.map(|cutoff| cutoff.clamp(minimum, maximum))
                }
                value if value.ends_with("/filter.resonance") => {
                    default_resonance.map(|resonance| resonance.clamp(minimum, maximum))
                }
                _ => None,
            };
            meldritch_app::CuratedControlView {
                id: control.id().to_owned(),
                label: control.label().to_owned(),
                target,
                minimum,
                maximum,
                step: control.step(),
                binding: control.binding().to_owned(),
                value,
            }
        })
        .collect()
}

/// Seed unknown pickup projections from existing renderer defaults. Does not
/// apply overrides or change audio. Legacy hosts can retain their old display.
pub fn initialize_pickup_controls(
    song: &ValidatedSong,
    controls: &mut [meldritch_app::CuratedControlView],
) {
    for view in controls {
        if view.value.is_some() {
            continue;
        }
        let Some(control) = song
            .performance()
            .controls()
            .iter()
            .find(|control| control.id() == view.id)
        else {
            continue;
        };
        let target = control.target();
        let value =
            if matches!(target.owner(), ParameterOwner::Lane) && target.parameter() == "level" {
                Some(1.0)
            } else if matches!(target.owner(), ParameterOwner::Synth) {
                song.synths()
                    .get(target.definition_id())
                    .and_then(|synth| {
                        synth
                            .modules()
                            .iter()
                            .find(|module| module.id() == target.module_id())
                    })
                    .and_then(|module| match target.parameter() {
                        "cutoff_hz" => Some(module.cutoff_hz().unwrap_or(20_000.0)),
                        "resonance" => Some(module.resonance().unwrap_or(0.0)),
                        _ => None,
                    })
            } else {
                None
            };
        // Unknown/out-of-range projections stay unavailable, rather than
        // inventing a pickup point that differs from the rendered parameter.
        view.value = value.filter(|value| (view.minimum..=view.maximum).contains(value));
    }
}

pub fn song_performance_pages_for_view(
    song: &meldritch_dsl::ValidatedSong,
) -> Vec<meldritch_app::PerformancePageView> {
    let lanes = song
        .performance()
        .lanes()
        .iter()
        .map(|lane| (lane.id(), lane))
        .collect::<BTreeMap<_, _>>();
    song.performance()
        .pages()
        .iter()
        .map(|page| meldritch_app::PerformancePageView {
            id: page.id().to_owned(),
            label: page.label().to_owned(),
            strips: page
                .strips()
                .iter()
                .filter_map(|strip| {
                    let lane = lanes.get(strip.lane_id())?;
                    Some(meldritch_app::PerformanceStripView {
                        strip: strip.strip(),
                        lane_id: lane.id().to_owned(),
                        lane_label: lane.label().to_owned(),
                        lane_role: lane.role().to_owned(),
                        track_id: lane.track_id().map(str::to_owned),
                        launch_quantization: lane.launch_quantization().map(str::to_owned),
                        muted: lane.default_muted(),
                        soloed: lane.default_soloed(),
                        active_pattern_bank_id: lane
                            .pattern_banks()
                            .first()
                            .map(|bank| bank.id().to_owned()),
                        active_variation_id: lane
                            .track_id()
                            .and_then(|id| {
                                song.performance()
                                    .tracks()
                                    .iter()
                                    .find(|track| track.id() == id)
                            })
                            .and_then(|track| track.initial_pattern())
                            .map(str::to_owned)
                            .or_else(|| lane.variation_ids().first().cloned()),
                        octave: 0,
                        variation_ids: lane.variation_ids().to_vec(),
                        pattern_banks: lane
                            .pattern_banks()
                            .iter()
                            .map(|bank| meldritch_app::PerformancePatternBankView {
                                id: bank.id().to_owned(),
                                label: bank.label().to_owned(),
                                variation_ids: bank.variation_ids().to_vec(),
                            })
                            .collect(),
                        pattern_monitors: lane
                            .variation_ids()
                            .iter()
                            .filter_map(|variation_id| {
                                let pattern = song.note_patterns().get(variation_id)?;
                                Some(performance_pattern_monitor_for_song_pattern(
                                    variation_id,
                                    pattern,
                                ))
                            })
                            .collect(),
                        control_ids: strip.control_ids().to_vec(),
                    })
                })
                .collect(),
        })
        .collect()
}

pub fn performance_pattern_monitor_for_song_pattern(
    variation_id: &str,
    pattern: &meldritch_dsl::NotePatternDefinition,
) -> meldritch_app::PerformancePatternMonitorView {
    let length_steps = 16;
    let mut active_steps = pattern
        .events()
        .iter()
        .map(|event| {
            let step = if pattern.length_frames() == 0 {
                0
            } else {
                event.start_frame().saturating_mul(u64::from(length_steps))
                    / pattern.length_frames()
            };
            u32::try_from(step)
                .unwrap_or(length_steps - 1)
                .min(length_steps - 1)
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    active_steps.sort_unstable();
    meldritch_app::PerformancePatternMonitorView {
        variation_id: variation_id.to_owned(),
        length_frames: pattern.length_frames(),
        length_steps,
        active_steps,
    }
}

pub fn active_lane_variations(view: &meldritch_app::AppViewModel) -> BTreeMap<String, String> {
    view.performance
        .pages
        .iter()
        .flat_map(|page| page.strips.iter())
        .filter_map(|strip| {
            Some((
                strip.lane_id.clone(),
                strip.active_variation_id.as_ref()?.clone(),
            ))
        })
        .collect()
}

pub fn included_lanes_from_view(view: &meldritch_app::AppViewModel) -> BTreeSet<String> {
    let strips = view
        .performance
        .pages
        .iter()
        .flat_map(|page| page.strips.iter())
        .collect::<Vec<_>>();
    let soloed = strips
        .iter()
        .filter(|strip| strip.soloed)
        .map(|strip| strip.lane_id.clone())
        .collect::<BTreeSet<_>>();
    if !soloed.is_empty() {
        return soloed;
    }
    strips
        .into_iter()
        .filter(|strip| !strip.muted)
        .map(|strip| strip.lane_id.clone())
        .collect()
}

pub fn default_included_lanes(song: &meldritch_dsl::ValidatedSong) -> BTreeSet<String> {
    let soloed = song
        .performance()
        .lanes()
        .iter()
        .filter(|lane| lane.default_soloed())
        .map(|lane| lane.id().to_owned())
        .collect::<BTreeSet<_>>();
    if !soloed.is_empty() {
        return soloed;
    }
    song.performance()
        .lanes()
        .iter()
        .filter(|lane| !lane.default_muted())
        .map(|lane| lane.id().to_owned())
        .collect()
}

pub fn compile_mixed_song_for_active_lanes(
    song: &meldritch_dsl::ValidatedSong,
    active_variations: &BTreeMap<String, String>,
    lane_transpose_semitones: &BTreeMap<String, i8>,
    included_lanes: &BTreeSet<String>,
) -> Result<CompiledSongPatch, String> {
    let patch = meldritch_render::song_render::compile_mixed_note_song_with_lane_state(
        song,
        active_variations,
        lane_transpose_semitones,
        included_lanes,
    )
    .map_err(|error| format!("failed to compile active mixed song: {error:?}"))?;
    Ok(CompiledSongPatch::Mixed(patch))
}

pub fn parameter_target_label(target: &ParameterTargetDefinition) -> String {
    if matches!(target.owner(), ParameterOwner::Lane) {
        return format!("lane:{}/{}", target.definition_id(), target.parameter());
    }
    let owner = match target.owner() {
        ParameterOwner::Synth => "synth",
        ParameterOwner::Dsp => "dsp",
        ParameterOwner::Lane => unreachable!("lane targets are formatted above"),
    };
    format!(
        "{}:{}/{}.{}",
        owner,
        target.definition_id(),
        target.module_id(),
        target.parameter()
    )
}
