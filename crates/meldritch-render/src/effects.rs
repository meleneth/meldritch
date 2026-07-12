//! Deterministic event-aware effect sends.

use crate::{
    Fingerprint, FingerprintBuilder, RenderSettings, render_pattern_samples_with_event_gain,
    sample_signature, write_event_fingerprint,
};
use meldritch_audio::{AudioBlock, SampleBuffer};
use meldritch_core::{Event, EventTag, FrameRange, Pattern, ProbabilitySeed, Sample, Tempo};
use std::collections::BTreeMap;
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum EffectBus {
    Delay,
    Reverb,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EffectSendRule {
    pub bus: EffectBus,
    pub required_tag: EventTag,
    pub send_gain: Sample,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TempoDelaySettings {
    /// Delay length in quarter-note beats. `0.75` is a dotted eighth note.
    pub beats: f64,
    pub feedback: f64,
    pub feedback_lowpass_hz: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ModulatedReverbSettings {
    pub predelay_seconds: f64,
    pub size: f64,
    pub decay: f64,
    pub damping_hz: f64,
    pub modulation_depth_seconds: f64,
    pub modulation_cycle_beats: f64,
    pub mix: f64,
    pub freeze: bool,
}

impl Default for ModulatedReverbSettings {
    fn default() -> Self {
        Self {
            predelay_seconds: 0.024,
            size: 1.15,
            decay: 0.78,
            damping_hz: 5_500.0,
            modulation_depth_seconds: 0.0015,
            modulation_cycle_beats: 12.0,
            mix: 0.38,
            freeze: false,
        }
    }
}

impl Default for TempoDelaySettings {
    fn default() -> Self {
        Self {
            beats: 0.75,
            feedback: 0.52,
            feedback_lowpass_hz: 4_800.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ActiveSendExplanation {
    pub pattern: meldritch_core::PatternId,
    pub track: meldritch_core::TrackId,
    pub step: meldritch_core::StepIndex,
    pub frame: u64,
    pub bus: EffectBus,
    pub matched_tag: EventTag,
    pub send_gain: Sample,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EffectSendRender {
    pub mix: AudioBlock,
    pub delay_bus: AudioBlock,
    pub reverb_bus: AudioBlock,
    pub explanations: Vec<ActiveSendExplanation>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct EffectArtifactKey {
    pub range: FrameRange,
    pub fingerprint: Fingerprint,
}

#[must_use]
pub fn effect_artifact_key(
    pattern: &Pattern,
    tempo: Tempo,
    range: FrameRange,
    probability_seed: ProbabilitySeed,
    settings: RenderSettings,
    samples_by_note: &BTreeMap<u8, SampleBuffer>,
    rules: &[EffectSendRule],
) -> EffectArtifactKey {
    let mut state = FingerprintBuilder::new();
    state.write_u64(pattern.id().raw());
    state.write_u64(range.start());
    state.write_u64(range.end());
    state.write_u64(u64::from(tempo.sample_rate()));
    state.write_u64(tempo.bpm().to_bits());
    state.write_u64(probability_seed.raw());
    state.write_u64(u64::from(settings.channels()));
    state.write_u64(rules.len() as u64);
    for rule in rules {
        state.write_u64(rule.bus as u64);
        state.write_u64(rule.required_tag as u64);
        state.write_u64(rule.send_gain.to_bits());
    }
    let tail = effect_tail_frames(tempo);
    let sample_lookbehind = samples_by_note
        .values()
        .map(SampleBuffer::frames)
        .max()
        .unwrap_or(0);
    let expanded = FrameRange::new(
        range
            .start()
            .saturating_sub(u64::from(tail.saturating_add(sample_lookbehind))),
        range.end(),
    )
    .expect("effect key lookbehind is ordered");
    let mut events = Vec::new();
    pattern.events_between(tempo, expanded, probability_seed, &mut events);
    events.retain(|event| {
        rules
            .iter()
            .any(|rule| rule.send_gain > 0.0 && event.tags().contains(&rule.required_tag))
    });
    write_event_fingerprint(&mut state, &events);
    for note in events.iter().map(Event::note).collect::<BTreeSet<_>>() {
        if let Some(sample) = samples_by_note.get(&note) {
            state.write_u64(u64::from(note));
            state.write_u64(sample_signature(sample));
        }
    }
    EffectArtifactKey {
        range,
        fingerprint: state.finish(),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn render_event_aware_effects(
    pattern: &Pattern,
    tempo: Tempo,
    range: FrameRange,
    probability_seed: ProbabilitySeed,
    settings: RenderSettings,
    samples_by_note: &BTreeMap<u8, SampleBuffer>,
    rules: &[EffectSendRule],
) -> EffectSendRender {
    let frames = (range.end() - range.start()).min(u64::from(u32::MAX)) as u32;
    let mut delay_input = AudioBlock::silent(settings.channels(), frames);
    let mut reverb_input = AudioBlock::silent(settings.channels(), frames);
    for rule in rules {
        let send = render_pattern_samples_with_event_gain(
            pattern,
            tempo,
            range,
            probability_seed,
            settings,
            samples_by_note,
            |event| {
                if event.tags().contains(&rule.required_tag) {
                    rule.send_gain.clamp(0.0, 1.0)
                } else {
                    0.0
                }
            },
        );
        let target = match rule.bus {
            EffectBus::Delay => &mut delay_input,
            EffectBus::Reverb => &mut reverb_input,
        };
        add_block(target, &send);
    }
    let delay_bus = apply_tempo_ping_pong_delay(&delay_input, tempo, TempoDelaySettings::default());
    let reverb_bus =
        apply_modulated_reverb(&reverb_input, tempo, ModulatedReverbSettings::default());
    let mut mix = crate::render_pattern_samples(
        pattern,
        tempo,
        range,
        probability_seed,
        settings,
        samples_by_note,
    );
    add_block(&mut mix, &delay_bus);
    add_block(&mut mix, &reverb_bus);

    let explanations = explain_effect_sends(pattern, tempo, range, probability_seed, rules);
    EffectSendRender {
        mix,
        delay_bus,
        reverb_bus,
        explanations,
    }
}

#[must_use]
pub fn explain_effect_sends(
    pattern: &Pattern,
    tempo: Tempo,
    range: FrameRange,
    probability_seed: ProbabilitySeed,
    rules: &[EffectSendRule],
) -> Vec<ActiveSendExplanation> {
    let mut events = Vec::new();
    pattern.events_between(tempo, range, probability_seed, &mut events);
    let mut explanations = events
        .iter()
        .flat_map(|event| matching_explanations(event, rules))
        .collect::<Vec<_>>();
    explanations.sort_by_key(|explanation| {
        (
            explanation.frame,
            explanation.track.raw(),
            explanation.step.raw(),
            explanation.bus,
        )
    });
    explanations
}

#[allow(clippy::too_many_arguments)]
pub fn render_event_aware_effects_chunk(
    pattern: &Pattern,
    tempo: Tempo,
    range: FrameRange,
    probability_seed: ProbabilitySeed,
    settings: RenderSettings,
    samples_by_note: &BTreeMap<u8, SampleBuffer>,
    rules: &[EffectSendRule],
) -> EffectSendRender {
    let preroll_range = FrameRange::new(0, range.end()).expect("effect preroll range is ordered");
    let full = render_event_aware_effects(
        pattern,
        tempo,
        preroll_range,
        probability_seed,
        settings,
        samples_by_note,
        rules,
    );
    EffectSendRender {
        mix: crop_block(&full.mix, range),
        delay_bus: crop_block(&full.delay_bus, range),
        reverb_bus: crop_block(&full.reverb_bus, range),
        explanations: full
            .explanations
            .into_iter()
            .filter(|explanation| range.contains_frame(explanation.frame))
            .collect(),
    }
}

fn matching_explanations(event: &Event, rules: &[EffectSendRule]) -> Vec<ActiveSendExplanation> {
    rules
        .iter()
        .filter(|rule| event.tags().contains(&rule.required_tag) && rule.send_gain > 0.0)
        .map(|rule| ActiveSendExplanation {
            pattern: event.pattern(),
            track: event.track(),
            step: event.step(),
            frame: event.range().start(),
            bus: rule.bus,
            matched_tag: rule.required_tag,
            send_gain: rule.send_gain.clamp(0.0, 1.0),
        })
        .collect()
}

const DELAY_TAIL_REPEATS: u32 = 6;

fn effect_tail_frames(tempo: Tempo) -> u32 {
    let delay =
        delay_frames(tempo, TempoDelaySettings::default().beats).saturating_mul(DELAY_TAIL_REPEATS);
    let reverb = tempo.sample_rate().saturating_mul(3);
    delay.max(reverb)
}

fn delay_frames(tempo: Tempo, beats: f64) -> u32 {
    (tempo.frames_per_beat() * beats.clamp(1.0 / 64.0, 16.0))
        .round()
        .clamp(1.0, f64::from(u32::MAX)) as u32
}

#[must_use]
pub fn apply_tempo_ping_pong_delay(
    input: &AudioBlock,
    tempo: Tempo,
    settings: TempoDelaySettings,
) -> AudioBlock {
    let mut output = input.clone();
    let delay_frames = delay_frames(tempo, settings.beats);
    let channels = usize::from(input.channels());
    let mut delay_line = vec![0.0; delay_frames as usize * channels];
    let feedback = settings.feedback.clamp(0.0, 0.98);
    let cutoff = settings
        .feedback_lowpass_hz
        .clamp(20.0, f64::from(tempo.sample_rate()) * 0.49);
    let coefficient =
        1.0 - (-2.0 * std::f64::consts::PI * cutoff / f64::from(tempo.sample_rate())).exp();
    let mut filtered = vec![0.0; channels];
    for frame in 0..input.frames() {
        let position = frame as usize % delay_frames as usize;
        for (channel, filtered_sample) in filtered.iter_mut().enumerate() {
            let index = frame as usize * channels + channel;
            let opposite = if channels == 2 { 1 - channel } else { channel };
            let delayed = delay_line[position * channels + opposite];
            *filtered_sample += coefficient * (delayed - *filtered_sample);
            output.samples_mut()[index] += delayed;
        }
        for (channel, filtered_sample) in filtered.iter().copied().enumerate() {
            let index = frame as usize * channels + channel;
            delay_line[position * channels + channel] =
                input.samples()[index] + filtered_sample * feedback;
        }
    }
    output
}

#[must_use]
pub fn apply_modulated_reverb(
    input: &AudioBlock,
    tempo: Tempo,
    settings: ModulatedReverbSettings,
) -> AudioBlock {
    let channels = usize::from(input.channels());
    let mut output = input.clone();
    let sample_rate = tempo.sample_rate();
    let size = settings.size.clamp(0.25, 2.5);
    let modulation_depth = (settings.modulation_depth_seconds.clamp(0.0, 0.02)
        * f64::from(sample_rate))
    .round() as usize;
    let base_seconds = [0.0297, 0.0371, 0.0411, 0.0437];
    let mut lines = base_seconds
        .iter()
        .enumerate()
        .map(|(line, seconds)| {
            (0..channels)
                .map(|channel| {
                    let stereo_offset = channel as f64 * 0.0017 + line as f64 * 0.0003;
                    let base = ((seconds * size + stereo_offset) * f64::from(sample_rate)).round()
                        as usize;
                    vec![0.0; base.saturating_add(modulation_depth * 2).max(2)]
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let mut positions = vec![vec![0_usize; channels]; base_seconds.len()];
    let mut damped = vec![vec![0.0; channels]; base_seconds.len()];
    let damping = settings
        .damping_hz
        .clamp(20.0, f64::from(sample_rate) * 0.49);
    let damping_coefficient =
        1.0 - (-2.0 * std::f64::consts::PI * damping / f64::from(sample_rate)).exp();
    let feedback = if settings.freeze {
        0.9995
    } else {
        settings.decay.clamp(0.0, 0.97)
    };
    let predelay =
        (settings.predelay_seconds.clamp(0.0, 0.5) * f64::from(sample_rate)).round() as usize;
    let mix = settings.mix.clamp(0.0, 1.0);
    for frame in 0..input.frames() as usize {
        let modulation_phase = frame as f64
            / (tempo.frames_per_beat() * settings.modulation_cycle_beats.clamp(0.25, 256.0));
        for channel in 0..channels {
            let input_index = frame * channels + channel;
            let injected = frame
                .checked_sub(predelay)
                .map_or(0.0, |source| input.samples()[source * channels + channel]);
            let mut wet = 0.0;
            for line_index in 0..lines.len() {
                let line = &mut lines[line_index][channel];
                let position = positions[line_index][channel];
                let phase = modulation_phase + line_index as f64 * 0.17 + channel as f64 * 0.25;
                let offset = ((std::f64::consts::TAU * phase).sin() * modulation_depth as f64)
                    .round() as isize;
                let nominal = line.len().saturating_sub(modulation_depth).max(1) as isize;
                let delay = (nominal + offset).clamp(1, line.len() as isize - 1) as usize;
                let read = (position + line.len() - delay) % line.len();
                let delayed = line[read];
                damped[line_index][channel] +=
                    damping_coefficient * (delayed - damped[line_index][channel]);
                line[position] = injected * 0.25 + damped[line_index][channel] * feedback;
                positions[line_index][channel] = (position + 1) % line.len();
                wet += delayed;
            }
            wet *= 0.5;
            output.samples_mut()[input_index] =
                input.samples()[input_index] * (1.0 - mix) + wet * mix;
        }
    }
    output
}

fn add_block(target: &mut AudioBlock, source: &AudioBlock) {
    for (target, source) in target.samples_mut().iter_mut().zip(source.samples()) {
        *target += source;
    }
}

fn crop_block(block: &AudioBlock, range: FrameRange) -> AudioBlock {
    let frames = (range.end() - range.start()).min(u64::from(u32::MAX)) as u32;
    let channels = usize::from(block.channels());
    let start = range.start() as usize * channels;
    let end = start + frames as usize * channels;
    let mut cropped = AudioBlock::silent(block.channels(), frames);
    cropped
        .samples_mut()
        .copy_from_slice(&block.samples()[start..end]);
    cropped
}

#[cfg(test)]
mod tests {
    use super::*;
    use meldritch_core::{PatternId, Step, StepIndex, TrackId};

    #[test]
    fn tags_route_only_matching_events_and_explain_the_send() {
        let tempo = Tempo::new(120.0, 48_000).unwrap();
        let mut pattern = Pattern::new(PatternId::new(1), 4, 4).unwrap();
        pattern
            .set_step(
                TrackId::new(1),
                StepIndex::new(0),
                Step::new(36).with_tag(EventTag::Accent),
            )
            .unwrap();
        pattern
            .set_step(
                TrackId::new(2),
                StepIndex::new(1),
                Step::new(38).with_tag(EventTag::Ghost),
            )
            .unwrap();
        pattern
            .set_step(TrackId::new(3), StepIndex::new(2), Step::new(42))
            .unwrap();
        let mut samples = BTreeMap::new();
        for note in [36, 38, 42] {
            samples.insert(note, SampleBuffer::new(1, 48_000, vec![1.0; 4]));
        }
        let rendered = render_event_aware_effects(
            &pattern,
            tempo,
            FrameRange::new(0, 24_000).unwrap(),
            ProbabilitySeed::new(1),
            RenderSettings::new(1).unwrap(),
            &samples,
            &[
                EffectSendRule {
                    bus: EffectBus::Delay,
                    required_tag: EventTag::Accent,
                    send_gain: 0.5,
                },
                EffectSendRule {
                    bus: EffectBus::Reverb,
                    required_tag: EventTag::Ghost,
                    send_gain: 0.25,
                },
            ],
        );

        assert!(rendered.delay_bus.samples()[0] > 0.0);
        assert_eq!(rendered.delay_bus.samples()[6_000], 0.0);
        assert!(rendered.reverb_bus.samples()[6_000] > 0.0);
        assert!(rendered.reverb_bus.samples()[12_000] > 0.0);
        assert_eq!(rendered.explanations.len(), 2);
        assert_eq!(rendered.explanations[0].matched_tag, EventTag::Accent);
        assert_eq!(rendered.explanations[1].matched_tag, EventTag::Ghost);
        assert!(rendered.mix.peak_abs() > 0.0);
    }

    #[test]
    fn effect_chunks_preserve_delay_tails_and_match_full_render() {
        let tempo = Tempo::new(120.0, 48_000).unwrap();
        let mut pattern = Pattern::new(PatternId::new(1), 16, 4).unwrap();
        pattern
            .set_step(
                TrackId::new(1),
                StepIndex::new(0),
                Step::new(36).with_tag(EventTag::Accent),
            )
            .unwrap();
        let mut samples = BTreeMap::new();
        samples.insert(36, SampleBuffer::new(1, 48_000, vec![1.0; 16]));
        let rules = [EffectSendRule {
            bus: EffectBus::Delay,
            required_tag: EventTag::Accent,
            send_gain: 0.5,
        }];
        let full = render_event_aware_effects(
            &pattern,
            tempo,
            FrameRange::new(0, 20_000).unwrap(),
            ProbabilitySeed::new(1),
            RenderSettings::new(1).unwrap(),
            &samples,
            &rules,
        );
        let chunk = render_event_aware_effects_chunk(
            &pattern,
            tempo,
            FrameRange::new(8_000, 20_000).unwrap(),
            ProbabilitySeed::new(1),
            RenderSettings::new(1).unwrap(),
            &samples,
            &rules,
        );

        assert_eq!(&full.mix.samples()[8_000..], chunk.mix.samples());
        assert_eq!(
            &full.delay_bus.samples()[8_000..],
            chunk.delay_bus.samples()
        );
        assert!(chunk.delay_bus.samples()[10_000] > 0.0);
    }

    #[test]
    fn ping_pong_delay_tracks_tempo_and_alternates_stereo_echoes() {
        let mut impulse = AudioBlock::silent(2, 80_000);
        impulse.samples_mut()[0] = 1.0;
        let settings = TempoDelaySettings {
            beats: 0.75,
            feedback: 0.6,
            feedback_lowpass_hz: 20_000.0,
        };
        let fast =
            apply_tempo_ping_pong_delay(&impulse, Tempo::new(120.0, 48_000).unwrap(), settings);
        let slow =
            apply_tempo_ping_pong_delay(&impulse, Tempo::new(60.0, 48_000).unwrap(), settings);

        let fast_delay = 18_000_usize;
        let slow_delay = 36_000_usize;
        assert_eq!(fast.samples()[fast_delay * 2], 0.0);
        assert_eq!(fast.samples()[fast_delay * 2 + 1], 1.0);
        assert_eq!(slow.samples()[slow_delay * 2 + 1], 1.0);
        assert!(fast.samples()[fast_delay * 4] > 0.0);
        assert!(fast.samples().iter().all(|sample| sample.is_finite()));
    }

    #[test]
    fn ping_pong_feedback_filter_damps_repeated_echoes() {
        let mut impulse = AudioBlock::silent(1, 50_000);
        impulse.samples_mut()[0] = 1.0;
        let tempo = Tempo::new(120.0, 48_000).unwrap();
        let render = |cutoff| {
            apply_tempo_ping_pong_delay(
                &impulse,
                tempo,
                TempoDelaySettings {
                    beats: 0.5,
                    feedback: 0.8,
                    feedback_lowpass_hz: cutoff,
                },
            )
        };
        let dark = render(200.0);
        let bright = render(20_000.0);
        assert!(dark.samples()[24_000] < bright.samples()[24_000]);
        assert!(bright.peak_abs() <= 1.0);
    }

    #[test]
    fn modulated_reverb_honors_predelay_and_decorrelates_stereo() {
        let tempo = Tempo::new(120.0, 48_000).unwrap();
        let mut impulse = AudioBlock::silent(2, 30_000);
        impulse.samples_mut()[0] = 1.0;
        impulse.samples_mut()[1] = 1.0;
        let output = apply_modulated_reverb(
            &impulse,
            tempo,
            ModulatedReverbSettings {
                mix: 1.0,
                ..ModulatedReverbSettings::default()
            },
        );
        assert!(
            output.samples()[2..2_000 * 2]
                .iter()
                .all(|sample| *sample == 0.0)
        );
        assert!(
            output.samples()[2_000 * 2..8_000 * 2]
                .iter()
                .any(|sample| *sample != 0.0)
        );
        assert!((0..output.frames()).any(|frame| {
            let index = frame as usize * 2;
            output.samples()[index] != output.samples()[index + 1]
        }));
        assert!(output.samples().iter().all(|sample| sample.is_finite()));
    }

    #[test]
    fn reverb_freeze_retains_more_late_energy_than_normal_decay() {
        let tempo = Tempo::new(120.0, 48_000).unwrap();
        let mut impulse = AudioBlock::silent(1, 180_000);
        impulse.samples_mut()[0] = 1.0;
        let render = |freeze| {
            apply_modulated_reverb(
                &impulse,
                tempo,
                ModulatedReverbSettings {
                    mix: 1.0,
                    freeze,
                    ..ModulatedReverbSettings::default()
                },
            )
        };
        let normal = render(false);
        let frozen = render(true);
        let energy = |block: &AudioBlock| {
            block.samples()[150_000..]
                .iter()
                .map(|sample| sample * sample)
                .sum::<f64>()
        };
        assert!(energy(&frozen) > energy(&normal));
        assert!(frozen.peak_abs() < 2.0);
    }

    #[test]
    fn effect_key_tracks_rules_tagged_events_and_used_samples() {
        let tempo = Tempo::new(120.0, 48_000).unwrap();
        let mut pattern = Pattern::new(PatternId::new(1), 4, 4).unwrap();
        pattern
            .set_step(
                TrackId::new(1),
                StepIndex::new(0),
                Step::new(36).with_tag(EventTag::Accent),
            )
            .unwrap();
        let range = FrameRange::new(0, 20_000).unwrap();
        let settings = RenderSettings::new(1).unwrap();
        let mut samples = BTreeMap::new();
        samples.insert(36, SampleBuffer::new(1, 48_000, vec![0.5]));
        samples.insert(99, SampleBuffer::new(1, 48_000, vec![0.2]));
        let rule = |gain| EffectSendRule {
            bus: EffectBus::Delay,
            required_tag: EventTag::Accent,
            send_gain: gain,
        };
        let base = effect_artifact_key(
            &pattern,
            tempo,
            range,
            ProbabilitySeed::new(1),
            settings,
            &samples,
            &[rule(0.5)],
        );
        samples.insert(99, SampleBuffer::new(1, 48_000, vec![0.9]));
        let unused_changed = effect_artifact_key(
            &pattern,
            tempo,
            range,
            ProbabilitySeed::new(1),
            settings,
            &samples,
            &[rule(0.5)],
        );
        let rule_changed = effect_artifact_key(
            &pattern,
            tempo,
            range,
            ProbabilitySeed::new(1),
            settings,
            &samples,
            &[rule(0.8)],
        );

        assert_eq!(base, unused_changed);
        assert_ne!(base, rule_changed);
    }
}
