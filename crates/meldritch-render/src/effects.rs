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
    let reverb_bus = apply_reverb(&reverb_input, tempo.sample_rate());
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
    let reverb = (0.073 * f64::from(tempo.sample_rate())).round() as u32;
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

fn apply_reverb(input: &AudioBlock, sample_rate: u32) -> AudioBlock {
    let mut output = input.clone();
    let channels = usize::from(input.channels());
    for (seconds, gain) in [(0.031, 0.34), (0.047, 0.24), (0.073, 0.16)] {
        let delay = (seconds * f64::from(sample_rate)).round() as u32;
        for frame in delay..input.frames() {
            for channel in 0..channels {
                let target = frame as usize * channels + channel;
                let source = (frame - delay) as usize * channels + channel;
                output.samples_mut()[target] += input.samples()[source] * gain;
            }
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
        assert_eq!(rendered.reverb_bus.samples()[12_000], 0.0);
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
