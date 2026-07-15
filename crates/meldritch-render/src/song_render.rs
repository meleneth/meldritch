//! Compilation and offline rendering for directory-based modular songs.

use crate::dsp::{
    AdsrEnvelope, AdsrSettings, EnvelopeStage, FilterMode, Oscillator, StateVariableFilter,
    Waveform,
};
use meldritch_audio::{AudioBlock, SampleBuffer, read_wav};
use meldritch_core::FrameRange;
use meldritch_dsl::{
    ModuleKind, ParameterInterpolation, ParameterOwner, SampleSlotDefinition, TrackDefinition,
    ValidatedSong,
};
use std::collections::BTreeMap;
use std::fmt;

#[derive(Clone, Debug, PartialEq)]
pub struct CompiledDronePatch {
    song_fingerprint: u64,
    sample_rate: u32,
    channels: u16,
    frequency_hz: f64,
    waveform: Waveform,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct CompiledNoteEvent {
    start: u64,
    end: u64,
    note: u8,
    velocity: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct CompiledParameterPoint {
    frame: u64,
    value: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CompiledFilteredNotePatch {
    sample_rate: u32,
    channels: u16,
    waveform: Waveform,
    envelope: AdsrSettings,
    resonance: f64,
    pattern_length: u64,
    note_looped: bool,
    parameter_length: u64,
    parameter_looped: bool,
    cutoff_points: Vec<CompiledParameterPoint>,
    events: Vec<CompiledNoteEvent>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CompiledDelayedNotePatch {
    source: CompiledNotePatch,
    delay_frames: u64,
    feedback: f64,
    mix: f64,
    feedback_pattern_length: Option<u64>,
    feedback_pattern_looped: bool,
    feedback_points: Vec<CompiledParameterPoint>,
}

impl CompiledDelayedNotePatch {
    #[must_use]
    pub const fn delay_frames(&self) -> u64 {
        self.delay_frames
    }

    #[must_use]
    pub const fn feedback(&self) -> f64 {
        self.feedback
    }

    #[must_use]
    pub fn cutoff_hz(&self) -> Option<f64> {
        self.source.cutoff_hz()
    }

    #[must_use]
    pub fn resonance(&self) -> Option<f64> {
        self.source.resonance()
    }

    #[must_use]
    pub fn feedback_at(&self, absolute_frame: u64) -> f64 {
        let Some(length) = self.feedback_pattern_length else {
            return self.feedback;
        };
        let frame = if self.feedback_pattern_looped {
            absolute_frame % length
        } else {
            absolute_frame.min(length)
        };
        stepped_value(&self.feedback_points, frame)
    }

    #[must_use]
    pub const fn mix(&self) -> f64 {
        self.mix
    }

    pub fn render(&self, range: FrameRange) -> Result<AudioBlock, SongRenderError> {
        self.render_with_overrides(range, None, None)
    }

    pub fn render_with_feedback_override(
        &self,
        range: FrameRange,
        feedback_override: Option<f64>,
    ) -> Result<AudioBlock, SongRenderError> {
        self.render_with_overrides(range, feedback_override, None)
    }

    pub fn render_with_overrides(
        &self,
        range: FrameRange,
        feedback_override: Option<f64>,
        cutoff_override: Option<f64>,
    ) -> Result<AudioBlock, SongRenderError> {
        self.render_with_extended_overrides(range, feedback_override, cutoff_override, None, None)
    }

    pub fn render_with_extended_overrides(
        &self,
        range: FrameRange,
        feedback_override: Option<f64>,
        cutoff_override: Option<f64>,
        resonance_override: Option<f64>,
        mix_override: Option<f64>,
    ) -> Result<AudioBlock, SongRenderError> {
        if feedback_override.is_some_and(|value| !value.is_finite() || !(0.0..1.0).contains(&value))
        {
            return Err(SongRenderError::InvalidFeedbackOverride);
        }
        if mix_override.is_some_and(|value| !value.is_finite() || !(0.0..=1.0).contains(&value)) {
            return Err(SongRenderError::InvalidMixOverride);
        }
        let history = self.source.render_with_filter_overrides(
            FrameRange::new(0, range.end()).expect("history range is ordered"),
            cutoff_override,
            resonance_override,
        )?;
        let mix = mix_override.unwrap_or(self.mix);
        let frame_count = range.end() - range.start();
        let frames = u32::try_from(frame_count).map_err(|_| SongRenderError::RangeTooLong {
            frames: frame_count,
        })?;
        let channels = usize::from(self.source.channels);
        let delay_len = usize::try_from(self.delay_frames)
            .map_err(|_| SongRenderError::DelayTooLong(self.delay_frames))?
            .max(1);
        let mut delay = vec![0.0; delay_len * channels];
        let mut output = AudioBlock::silent(self.source.channels, frames);
        for absolute_frame in 0..range.end() {
            let history_frame =
                usize::try_from(absolute_frame).expect("AudioBlock range fits usize");
            let delay_frame = history_frame % delay_len;
            for channel in 0..channels {
                let source_index = history_frame * channels + channel;
                let delay_index = delay_frame * channels + channel;
                let dry = history.samples()[source_index];
                let wet = delay[delay_index];
                let feedback =
                    feedback_override.unwrap_or_else(|| self.feedback_at(absolute_frame));
                delay[delay_index] = dry + wet * feedback;
                if absolute_frame >= range.start() {
                    let relative = usize::try_from(absolute_frame - range.start())
                        .expect("u32 range fits usize");
                    output.samples_mut()[relative * channels + channel] =
                        dry * (1.0 - mix) + wet * mix;
                }
            }
        }
        Ok(output)
    }
}

impl CompiledFilteredNotePatch {
    #[must_use]
    pub fn cutoff_at(&self, absolute_frame: u64) -> f64 {
        let frame = if self.parameter_looped {
            absolute_frame % self.parameter_length
        } else {
            absolute_frame.min(self.parameter_length)
        };
        interpolated_value(&self.cutoff_points, frame)
    }

    pub fn render(&self, range: FrameRange) -> Result<AudioBlock, SongRenderError> {
        let frame_count = range.end() - range.start();
        let frames = u32::try_from(frame_count).map_err(|_| SongRenderError::RangeTooLong {
            frames: frame_count,
        })?;
        let mut block = AudioBlock::silent(self.channels, frames);
        let mut oscillator = Oscillator::new(self.waveform);
        let mut envelope = AdsrEnvelope::new(self.envelope, self.sample_rate);
        let mut filter = StateVariableFilter::new();
        let mut frequency = 440.0;
        let mut velocity = 0.0;
        let mut active_end = None;
        for absolute_frame in 0..range.end() {
            if active_end == Some(absolute_frame) {
                envelope.note_off();
                active_end = None;
            }
            let pattern_frame = if self.note_looped {
                absolute_frame % self.pattern_length
            } else {
                absolute_frame
            };
            if self.note_looped && absolute_frame > 0 && pattern_frame == 0 {
                envelope.note_off();
                active_end = None;
            }
            for event in self
                .events
                .iter()
                .filter(|event| event.start == pattern_frame)
            {
                frequency = midi_frequency(event.note);
                velocity = event.velocity;
                envelope.note_on();
                active_end = Some(absolute_frame + (event.end - event.start));
            }
            let raw = oscillator.next(frequency, self.sample_rate);
            let filtered = filter.process(
                raw,
                self.cutoff_at(absolute_frame),
                self.resonance,
                FilterMode::LowPass,
                self.sample_rate,
            );
            let sample = filtered * envelope.next_value() * velocity;
            if absolute_frame < range.start() {
                continue;
            }
            let relative = usize::try_from(absolute_frame - range.start())
                .expect("u32 render length fits usize");
            let frame_start = relative * usize::from(self.channels);
            for channel in 0..usize::from(self.channels) {
                block.samples_mut()[frame_start + channel] = sample;
            }
        }
        Ok(block)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct CompiledNotePatch {
    song_fingerprint: u64,
    synth_id: String,
    sample_rate: u32,
    channels: u16,
    polyphony: u16,
    waveform: Waveform,
    envelope: AdsrSettings,
    filter: Option<CompiledNoteFilter>,
    pattern_length: u64,
    looped: bool,
    events: Vec<CompiledNoteEvent>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CompiledMixedNoteSong {
    song_fingerprint: u64,
    sample_rate: u32,
    channels: u16,
    tracks: Vec<CompiledMixedTrack>,
}

#[derive(Clone, Debug, PartialEq)]
enum CompiledMixedTrack {
    Note(CompiledNotePatch),
    Sample(CompiledSampleTrack),
}

impl CompiledMixedTrack {
    const fn channels(&self) -> u16 {
        match self {
            Self::Note(track) => track.channels,
            Self::Sample(track) => track.channels,
        }
    }

    fn render(
        &self,
        range: FrameRange,
        overrides: &[CompiledSynthFilterOverride],
    ) -> Result<AudioBlock, SongRenderError> {
        match self {
            Self::Note(track) => {
                let (cutoff_hz, resonance) = track.synth_filter_overrides(overrides);
                track.render_with_filter_overrides(range, cutoff_hz, resonance)
            }
            Self::Sample(track) => track.render(range),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct CompiledSampleTrack {
    sample_rate: u32,
    channels: u16,
    level: f64,
    pitch_envelope: Option<CompiledSamplePitchEnvelope>,
    pattern_length: u64,
    looped: bool,
    events: Vec<CompiledSampleEvent>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct CompiledSamplePitchEnvelope {
    amount_semitones: f64,
    attack_frames: u64,
    decay_frames: u64,
    sustain_level: f64,
}

#[derive(Clone, Debug, PartialEq)]
struct CompiledSampleEvent {
    start: u64,
    duration: u64,
    velocity: f64,
    sample: SampleBuffer,
    slice_start: u64,
    slice_end: u64,
    pitch_ratio: f64,
}

impl CompiledSampleTrack {
    fn render(&self, range: FrameRange) -> Result<AudioBlock, SongRenderError> {
        let frame_count = range.end() - range.start();
        let frames = u32::try_from(frame_count).map_err(|_| SongRenderError::RangeTooLong {
            frames: frame_count,
        })?;
        let mut block = AudioBlock::silent(self.channels, frames);
        if self.events.is_empty() {
            return Ok(block);
        }
        let channels = usize::from(self.channels);
        for event in &self.events {
            if event.pitch_ratio <= 0.0 || !event.pitch_ratio.is_finite() {
                return Err(SongRenderError::InvalidSamplePitch);
            }
            if event.slice_end <= event.slice_start {
                continue;
            }
            let sample_channels = usize::from(event.sample.channels());
            if self.pattern_length == 0 {
                continue;
            }
            let starts = if self.looped {
                let first_cycle = range.start().saturating_sub(event.start) / self.pattern_length;
                let last_cycle = range.end().saturating_sub(event.start) / self.pattern_length + 1;
                (first_cycle..=last_cycle)
                    .map(|cycle| event.start + cycle * self.pattern_length)
                    .collect::<Vec<_>>()
            } else {
                vec![event.start]
            };
            for start in starts {
                let max_render_frames = ((event.slice_end - event.slice_start) as f64
                    / event.pitch_ratio)
                    .ceil() as u64
                    + self
                        .pitch_envelope
                        .map_or(0, |envelope| envelope.attack_frames + envelope.decay_frames);
                let trigger_end = start + max_render_frames.max(event.duration);
                if trigger_end <= range.start() || start >= range.end() {
                    continue;
                }
                let out_start = start.max(range.start());
                let out_end = trigger_end.min(range.end());
                let mut source_position = 0.0;
                for absolute_frame in out_start..out_end {
                    if absolute_frame == out_start && out_start > start {
                        source_position = self.sample_source_position(event, out_start - start);
                    }
                    let source_frame = event.slice_start + source_position.floor() as u64;
                    if source_frame >= event.slice_end {
                        break;
                    }
                    let source_index = usize::try_from(source_frame)
                        .expect("sample frame index fits usize")
                        * sample_channels;
                    let relative = usize::try_from(absolute_frame - range.start())
                        .expect("render range fits usize");
                    let output_index = relative * channels;
                    let gain = self.level * event.velocity;
                    for channel in 0..channels {
                        let sample_channel = channel.min(sample_channels.saturating_sub(1));
                        block.samples_mut()[output_index + channel] +=
                            event.sample.samples()[source_index + sample_channel] * gain;
                    }
                    source_position += self.sample_pitch_ratio_at(event, absolute_frame - start);
                }
            }
        }
        Ok(block)
    }

    fn sample_source_position(&self, event: &CompiledSampleEvent, frame_offset: u64) -> f64 {
        (0..frame_offset)
            .map(|offset| self.sample_pitch_ratio_at(event, offset))
            .sum()
    }

    fn sample_pitch_ratio_at(&self, event: &CompiledSampleEvent, frame_offset: u64) -> f64 {
        let envelope_semitones = self
            .pitch_envelope
            .map_or(0.0, |envelope| envelope.value_at(frame_offset));
        event.pitch_ratio * 2.0_f64.powf(envelope_semitones / 12.0)
    }
}

impl CompiledSamplePitchEnvelope {
    fn value_at(self, frame: u64) -> f64 {
        let level = if self.attack_frames > 0 && frame < self.attack_frames {
            frame as f64 / self.attack_frames as f64
        } else if self.decay_frames > 0 && frame < self.attack_frames + self.decay_frames {
            let decay_frame = frame - self.attack_frames;
            1.0 - (1.0 - self.sustain_level) * decay_frame as f64 / self.decay_frames as f64
        } else {
            self.sustain_level
        };
        self.amount_semitones * level
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct CompiledSynthFilterOverride {
    synth_id: String,
    module_id: String,
    cutoff_hz: Option<f64>,
    resonance: Option<f64>,
}

impl CompiledSynthFilterOverride {
    #[must_use]
    pub fn new(
        synth_id: impl Into<String>,
        module_id: impl Into<String>,
        cutoff_hz: Option<f64>,
        resonance: Option<f64>,
    ) -> Self {
        Self {
            synth_id: synth_id.into(),
            module_id: module_id.into(),
            cutoff_hz,
            resonance,
        }
    }

    #[must_use]
    pub fn synth_id(&self) -> &str {
        &self.synth_id
    }

    #[must_use]
    pub fn module_id(&self) -> &str {
        &self.module_id
    }

    #[must_use]
    pub const fn cutoff_hz(&self) -> Option<f64> {
        self.cutoff_hz
    }

    #[must_use]
    pub const fn resonance(&self) -> Option<f64> {
        self.resonance
    }
}

impl CompiledMixedNoteSong {
    #[must_use]
    pub const fn song_fingerprint(&self) -> u64 {
        self.song_fingerprint
    }

    #[must_use]
    pub const fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    #[must_use]
    pub const fn channels(&self) -> u16 {
        self.channels
    }

    #[must_use]
    pub fn track_count(&self) -> usize {
        self.tracks.len()
    }

    pub fn render(&self, range: FrameRange) -> Result<AudioBlock, SongRenderError> {
        self.render_with_synth_filter_overrides(range, &[])
    }

    pub fn render_with_synth_filter_overrides(
        &self,
        range: FrameRange,
        overrides: &[CompiledSynthFilterOverride],
    ) -> Result<AudioBlock, SongRenderError> {
        let frame_count = range.end() - range.start();
        let frames = u32::try_from(frame_count).map_err(|_| SongRenderError::RangeTooLong {
            frames: frame_count,
        })?;
        let mut mix = AudioBlock::silent(self.channels, frames);
        if self.tracks.is_empty() {
            return Ok(mix);
        }
        let gain = 1.0 / (self.tracks.len() as f64).sqrt();
        for track in &self.tracks {
            let block = track.render(range, overrides)?;
            if block.channels() != self.channels || block.frames() != frames {
                return Err(SongRenderError::IncompatibleTrackRender);
            }
            for (output, input) in mix.samples_mut().iter_mut().zip(block.samples()) {
                *output += input * gain;
            }
        }
        Ok(mix)
    }
}

#[derive(Clone, Debug, PartialEq)]
struct CompiledNoteFilter {
    module_id: String,
    cutoff_hz: f64,
    resonance: f64,
}

impl CompiledNotePatch {
    fn synth_filter_overrides(
        &self,
        overrides: &[CompiledSynthFilterOverride],
    ) -> (Option<f64>, Option<f64>) {
        let Some(filter) = &self.filter else {
            return (None, None);
        };
        overrides
            .iter()
            .rev()
            .find(|override_| {
                override_.synth_id == self.synth_id && override_.module_id == filter.module_id
            })
            .map_or((None, None), |override_| {
                (override_.cutoff_hz, override_.resonance)
            })
    }

    #[must_use]
    pub const fn pattern_length(&self) -> u64 {
        self.pattern_length
    }

    #[must_use]
    pub fn cutoff_hz(&self) -> Option<f64> {
        self.filter.as_ref().map(|filter| filter.cutoff_hz)
    }

    #[must_use]
    pub fn resonance(&self) -> Option<f64> {
        self.filter.as_ref().map(|filter| filter.resonance)
    }

    pub fn render(&self, range: FrameRange) -> Result<AudioBlock, SongRenderError> {
        self.render_with_cutoff_override(range, None)
    }

    pub fn render_with_cutoff_override(
        &self,
        range: FrameRange,
        cutoff_override: Option<f64>,
    ) -> Result<AudioBlock, SongRenderError> {
        self.render_with_filter_overrides(range, cutoff_override, None)
    }

    pub fn render_with_filter_overrides(
        &self,
        range: FrameRange,
        cutoff_override: Option<f64>,
        resonance_override: Option<f64>,
    ) -> Result<AudioBlock, SongRenderError> {
        if self.polyphony > 1 {
            return self.render_polyphonic_with_filter_overrides(
                range,
                cutoff_override,
                resonance_override,
            );
        }
        if cutoff_override.is_some_and(|value| !value.is_finite() || value <= 0.0) {
            return Err(SongRenderError::InvalidCutoffOverride);
        }
        if resonance_override
            .is_some_and(|value| !value.is_finite() || !(0.0..=1.0).contains(&value))
        {
            return Err(SongRenderError::InvalidResonanceOverride);
        }
        let frame_count = range.end() - range.start();
        let frames = u32::try_from(frame_count).map_err(|_| SongRenderError::RangeTooLong {
            frames: frame_count,
        })?;
        let mut block = AudioBlock::silent(self.channels, frames);
        let mut oscillator = Oscillator::new(self.waveform);
        let mut envelope = AdsrEnvelope::new(self.envelope, self.sample_rate);
        let mut filter = StateVariableFilter::new();
        let mut frequency = 440.0;
        let mut velocity = 0.0;
        let mut active_end = None;
        for absolute_frame in 0..range.end() {
            if active_end == Some(absolute_frame) {
                envelope.note_off();
                active_end = None;
            }
            let pattern_frame = if self.looped {
                absolute_frame % self.pattern_length
            } else {
                absolute_frame
            };
            if self.looped && absolute_frame > 0 && pattern_frame == 0 {
                envelope.note_off();
                active_end = None;
            }
            for event in self
                .events
                .iter()
                .filter(|event| event.start == pattern_frame)
            {
                frequency = midi_frequency(event.note);
                velocity = event.velocity;
                envelope.note_on();
                active_end = Some(absolute_frame + (event.end - event.start));
            }
            let mut sample = oscillator.next(frequency, self.sample_rate);
            if let Some(filter_settings) = &self.filter {
                sample = filter.process(
                    sample,
                    cutoff_override.unwrap_or(filter_settings.cutoff_hz),
                    resonance_override.unwrap_or(filter_settings.resonance),
                    FilterMode::LowPass,
                    self.sample_rate,
                );
            }
            let sample = sample * envelope.next_value() * velocity;
            if absolute_frame < range.start() {
                continue;
            }
            let relative = usize::try_from(absolute_frame - range.start())
                .expect("u32 render length fits usize");
            let frame_start = relative * usize::from(self.channels);
            for channel in 0..usize::from(self.channels) {
                block.samples_mut()[frame_start + channel] = sample;
            }
        }
        Ok(block)
    }

    fn render_polyphonic_with_filter_overrides(
        &self,
        range: FrameRange,
        cutoff_override: Option<f64>,
        resonance_override: Option<f64>,
    ) -> Result<AudioBlock, SongRenderError> {
        if cutoff_override.is_some_and(|value| !value.is_finite() || value <= 0.0) {
            return Err(SongRenderError::InvalidCutoffOverride);
        }
        if resonance_override
            .is_some_and(|value| !value.is_finite() || !(0.0..=1.0).contains(&value))
        {
            return Err(SongRenderError::InvalidResonanceOverride);
        }
        let frame_count = range.end() - range.start();
        let frames = u32::try_from(frame_count).map_err(|_| SongRenderError::RangeTooLong {
            frames: frame_count,
        })?;
        let mut block = AudioBlock::silent(self.channels, frames);
        let mut voices = Vec::<CompiledNoteVoice>::new();
        for absolute_frame in 0..range.end() {
            for voice in voices
                .iter_mut()
                .filter(|voice| !voice.released && voice.end == absolute_frame)
            {
                voice.envelope.note_off();
                voice.released = true;
            }
            let pattern_frame = if self.looped {
                absolute_frame % self.pattern_length
            } else {
                absolute_frame
            };
            if self.looped && absolute_frame > 0 && pattern_frame == 0 {
                for voice in voices.iter_mut().filter(|voice| !voice.released) {
                    voice.envelope.note_off();
                    voice.released = true;
                }
            }
            for event in self
                .events
                .iter()
                .filter(|event| event.start == pattern_frame)
            {
                if voices.len() >= usize::from(self.polyphony) {
                    voices.remove(0);
                }
                let mut envelope = AdsrEnvelope::new(self.envelope, self.sample_rate);
                envelope.note_on();
                voices.push(CompiledNoteVoice {
                    oscillator: Oscillator::new(self.waveform),
                    envelope,
                    filter: StateVariableFilter::new(),
                    frequency: midi_frequency(event.note),
                    velocity: event.velocity,
                    end: absolute_frame + (event.end - event.start),
                    released: false,
                });
            }
            let mut sample = 0.0;
            for voice in &mut voices {
                let mut voice_sample = voice.oscillator.next(voice.frequency, self.sample_rate);
                if let Some(filter_settings) = &self.filter {
                    voice_sample = voice.filter.process(
                        voice_sample,
                        cutoff_override.unwrap_or(filter_settings.cutoff_hz),
                        resonance_override.unwrap_or(filter_settings.resonance),
                        FilterMode::LowPass,
                        self.sample_rate,
                    );
                }
                sample += voice_sample * voice.envelope.next_value() * voice.velocity;
            }
            voices.retain(|voice| voice.envelope.stage() != EnvelopeStage::Idle);
            let sample = sample / f64::from(self.polyphony).sqrt();
            if absolute_frame < range.start() {
                continue;
            }
            let relative = usize::try_from(absolute_frame - range.start())
                .expect("u32 render length fits usize");
            let frame_start = relative * usize::from(self.channels);
            for channel in 0..usize::from(self.channels) {
                block.samples_mut()[frame_start + channel] = sample;
            }
        }
        Ok(block)
    }
}

struct CompiledNoteVoice {
    oscillator: Oscillator,
    envelope: AdsrEnvelope,
    filter: StateVariableFilter,
    frequency: f64,
    velocity: f64,
    end: u64,
    released: bool,
}

impl CompiledDronePatch {
    #[must_use]
    pub const fn song_fingerprint(&self) -> u64 {
        self.song_fingerprint
    }

    #[must_use]
    pub const fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    #[must_use]
    pub const fn channels(&self) -> u16 {
        self.channels
    }

    pub fn render(&self, range: FrameRange) -> Result<AudioBlock, SongRenderError> {
        let frame_count = range.end() - range.start();
        let frames = u32::try_from(frame_count).map_err(|_| SongRenderError::RangeTooLong {
            frames: frame_count,
        })?;
        let mut block = AudioBlock::silent(self.channels, frames);
        let mut oscillator = Oscillator::new(self.waveform);
        for absolute_frame in 0..range.end() {
            let sample = oscillator.next(self.frequency_hz, self.sample_rate);
            if absolute_frame < range.start() {
                continue;
            }
            let relative = usize::try_from(absolute_frame - range.start())
                .expect("u32 render length fits usize");
            let frame_start = relative * usize::from(self.channels);
            for channel in 0..usize::from(self.channels) {
                block.samples_mut()[frame_start + channel] = sample;
            }
        }
        Ok(block)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SongRenderError {
    TrackCount { found: usize },
    MissingLane { id: String },
    MissingSynth { id: String },
    UnsupportedModule { id: String, kind: ModuleKind },
    MissingOutput,
    MultipleOutputs,
    UnconnectedOutput { id: String },
    MultiplyDrivenOutput { id: String },
    OutputNotDrivenByOscillator { endpoint: String },
    MissingFrequency { id: String },
    UnsupportedWaveform { id: String, waveform: String },
    MissingModule { kind: ModuleKind },
    MultipleModules { kind: ModuleKind },
    MissingCable { from: String, to: String },
    MissingPattern { id: String },
    MissingSampleBank { id: String },
    MissingSampleSlot { id: String },
    MissingSampleSlice { id: String },
    SampleLoad { path: String, message: String },
    UnsupportedSampleRate { found: u32, expected: u32 },
    UnsupportedInterpolation,
    MissingDsp { id: String },
    DspCount { found: usize },
    InvalidTempoDuration { value: String },
    DelayTooLong(u64),
    InvalidFeedbackOverride,
    InvalidCutoffOverride,
    InvalidResonanceOverride,
    InvalidMixOverride,
    InvalidSamplePitch,
    IncompatibleTrackRender,
    RangeTooLong { frames: u64 },
}

impl fmt::Display for SongRenderError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TrackCount { found } => write!(
                formatter,
                "initial drone compiler requires exactly one track, found {found}"
            ),
            Self::MissingLane { id } => {
                write!(formatter, "performance references missing lane '{id}'")
            }
            Self::MissingSynth { id } => write!(formatter, "track references missing synth '{id}'"),
            Self::UnsupportedModule { id, kind } => {
                write!(
                    formatter,
                    "module '{id}' of type {kind:?} is not supported by the drone compiler"
                )
            }
            Self::MissingOutput => write!(formatter, "synth patch has no audio_output module"),
            Self::MultipleOutputs => write!(
                formatter,
                "synth patch has more than one audio_output module"
            ),
            Self::UnconnectedOutput { id } => {
                write!(formatter, "audio output '{id}' is not connected")
            }
            Self::MultiplyDrivenOutput { id } => {
                write!(formatter, "audio output '{id}' has more than one driver")
            }
            Self::OutputNotDrivenByOscillator { endpoint } => write!(
                formatter,
                "audio output is driven by '{endpoint}', not an oscillator audio port"
            ),
            Self::MissingFrequency { id } => {
                write!(formatter, "oscillator '{id}' has no fixed frequency")
            }
            Self::UnsupportedWaveform { id, waveform } => {
                write!(
                    formatter,
                    "oscillator '{id}' has unsupported waveform '{waveform}'"
                )
            }
            Self::MissingModule { kind } => write!(formatter, "patch has no {kind:?} module"),
            Self::MultipleModules { kind } => {
                write!(formatter, "patch has more than one {kind:?} module")
            }
            Self::MissingCable { from, to } => {
                write!(formatter, "patch requires cable '{from} -> {to}'")
            }
            Self::MissingPattern { id } => {
                write!(formatter, "track references missing note pattern '{id}'")
            }
            Self::MissingSampleBank { id } => {
                write!(formatter, "track references missing sample bank '{id}'")
            }
            Self::MissingSampleSlot { id } => {
                write!(formatter, "sample bank has no sample slot '{id}'")
            }
            Self::MissingSampleSlice { id } => {
                write!(formatter, "sample slot has no sample slice '{id}'")
            }
            Self::SampleLoad { path, message } => {
                write!(formatter, "failed to load sample '{path}': {message}")
            }
            Self::UnsupportedSampleRate { found, expected } => write!(
                formatter,
                "sample rate {found} does not match song sample rate {expected}"
            ),
            Self::UnsupportedInterpolation => write!(
                formatter,
                "parameter lane interpolation is incompatible with the target compiler"
            ),
            Self::MissingDsp { id } => write!(formatter, "track references missing DSP '{id}'"),
            Self::DspCount { found } => write!(
                formatter,
                "tempo-delay compiler requires exactly one DSP graph, found {found}"
            ),
            Self::InvalidTempoDuration { value } => {
                write!(formatter, "unsupported tempo duration '{value}'")
            }
            Self::DelayTooLong(frames) => {
                write!(
                    formatter,
                    "delay length of {frames} frames exceeds host capacity"
                )
            }
            Self::InvalidFeedbackOverride => write!(
                formatter,
                "live delay feedback override must be finite and within 0.0..1.0"
            ),
            Self::InvalidCutoffOverride => {
                write!(
                    formatter,
                    "live cutoff override must be finite and positive"
                )
            }
            Self::InvalidResonanceOverride => write!(
                formatter,
                "live resonance override must be finite and within 0.0..=1.0"
            ),
            Self::InvalidMixOverride => {
                write!(
                    formatter,
                    "live delay mix override must be finite and within 0.0..=1.0"
                )
            }
            Self::InvalidSamplePitch => {
                write!(
                    formatter,
                    "sample pitch ratio must be finite and greater than zero"
                )
            }
            Self::IncompatibleTrackRender => {
                write!(formatter, "compiled track rendered incompatible audio")
            }
            Self::RangeTooLong { frames } => write!(
                formatter,
                "render range of {frames} frames exceeds AudioBlock capacity"
            ),
        }
    }
}

pub fn compile_note_song(song: &ValidatedSong) -> Result<CompiledNotePatch, SongRenderError> {
    let [track] = song.performance().tracks() else {
        return Err(SongRenderError::TrackCount {
            found: song.performance().tracks().len(),
        });
    };
    let pattern_id = track
        .initial_pattern()
        .ok_or_else(|| SongRenderError::MissingPattern { id: String::new() })?;
    compile_note_song_for_pattern(song, pattern_id)
}

pub fn compile_note_song_for_pattern(
    song: &ValidatedSong,
    pattern_id: &str,
) -> Result<CompiledNotePatch, SongRenderError> {
    let [track] = song.performance().tracks() else {
        return Err(SongRenderError::TrackCount {
            found: song.performance().tracks().len(),
        });
    };
    compile_note_track_for_pattern(song, track, pattern_id, 0)
}

fn compile_note_track_for_pattern(
    song: &ValidatedSong,
    track: &TrackDefinition,
    pattern_id: &str,
    transpose_semitones: i8,
) -> Result<CompiledNotePatch, SongRenderError> {
    let synth =
        song.synths()
            .get(track.synth_id())
            .ok_or_else(|| SongRenderError::MissingSynth {
                id: track.synth_id().to_owned(),
            })?;
    let oscillator = exactly_one_module(synth.modules(), ModuleKind::Oscillator)?;
    let envelope = exactly_one_module(synth.modules(), ModuleKind::Adsr)?;
    let amplifier = exactly_one_module(synth.modules(), ModuleKind::Vca)?;
    let output = exactly_one_module(synth.modules(), ModuleKind::AudioOutput)?;
    let filter = optional_one_module(synth.modules(), ModuleKind::LowPass)?;
    for (from, to) in [
        (
            "input.pitch".to_owned(),
            format!("{}.pitch", oscillator.id()),
        ),
        ("input.gate".to_owned(), format!("{}.gate", envelope.id())),
        (
            format!("{}.audio", oscillator.id()),
            format!("{}.audio", amplifier.id()),
        ),
        (
            format!("{}.control", envelope.id()),
            format!("{}.level", amplifier.id()),
        ),
    ] {
        if !synth
            .cables()
            .iter()
            .any(|cable| cable.from() == from && cable.to() == to)
        {
            return Err(SongRenderError::MissingCable { from, to });
        }
    }
    if let Some(filter) = filter {
        for (from, to) in [
            (
                format!("{}.audio", amplifier.id()),
                format!("{}.audio", filter.id()),
            ),
            (
                format!("{}.audio", filter.id()),
                format!("{}.audio", output.id()),
            ),
        ] {
            require_cable(synth, from, to)?;
        }
    } else {
        require_cable(
            synth,
            format!("{}.audio", amplifier.id()),
            format!("{}.audio", output.id()),
        )?;
    }
    let pattern =
        song.note_patterns()
            .get(pattern_id)
            .ok_or_else(|| SongRenderError::MissingPattern {
                id: pattern_id.to_owned(),
            })?;
    let waveform_name = oscillator.waveform().unwrap_or("sine");
    let waveform = parse_waveform(oscillator.id(), waveform_name)?;
    Ok(CompiledNotePatch {
        song_fingerprint: song.fingerprint(),
        synth_id: synth.id().to_owned(),
        sample_rate: song.performance().sample_rate(),
        channels: output.channels().unwrap_or(1),
        polyphony: synth.polyphony(),
        waveform,
        envelope: AdsrSettings {
            attack_seconds: envelope.attack().unwrap_or(0.0),
            decay_seconds: envelope.decay().unwrap_or(0.0),
            sustain_level: envelope.sustain().unwrap_or(1.0),
            release_seconds: envelope.release().unwrap_or(0.0),
        },
        filter: filter.map(|filter| CompiledNoteFilter {
            module_id: filter.id().to_owned(),
            cutoff_hz: filter.cutoff_hz().unwrap_or(20_000.0),
            resonance: filter.resonance().unwrap_or(0.0),
        }),
        pattern_length: pattern.length_frames(),
        looped: pattern.is_looped(),
        events: pattern
            .events()
            .iter()
            .map(|event| CompiledNoteEvent {
                start: event.start_frame(),
                end: event.start_frame() + event.duration_frames(),
                note: transposed_midi_note(event.note(), transpose_semitones),
                velocity: event.velocity(),
            })
            .collect(),
    })
}

fn transposed_midi_note(note: u8, transpose_semitones: i8) -> u8 {
    (i16::from(note) + i16::from(transpose_semitones)).clamp(0, 127) as u8
}

pub fn compile_mixed_note_song(
    song: &ValidatedSong,
) -> Result<CompiledMixedNoteSong, SongRenderError> {
    compile_mixed_note_song_with_lane_transposes(song, &BTreeMap::new())
}

pub fn compile_mixed_note_song_with_lane_transposes(
    song: &ValidatedSong,
    lane_transpose_semitones: &BTreeMap<String, i8>,
) -> Result<CompiledMixedNoteSong, SongRenderError> {
    compile_mixed_note_song_with_track_patterns_and_transposes(
        song,
        |track| track.initial_pattern().map(ToOwned::to_owned),
        lane_transpose_semitones,
    )
}

pub fn compile_mixed_note_song_with_lane_variation(
    song: &ValidatedSong,
    lane_id: &str,
    variation_id: &str,
) -> Result<CompiledMixedNoteSong, SongRenderError> {
    let lane = song
        .performance()
        .lanes()
        .iter()
        .find(|lane| lane.id() == lane_id)
        .ok_or_else(|| SongRenderError::MissingLane {
            id: lane_id.to_owned(),
        })?;
    if !lane
        .variation_ids()
        .iter()
        .any(|available| available == variation_id)
    {
        return Err(SongRenderError::MissingPattern {
            id: variation_id.to_owned(),
        });
    }
    let track_id = lane
        .track_id()
        .ok_or_else(|| SongRenderError::MissingLane {
            id: lane_id.to_owned(),
        })?;
    compile_mixed_note_song_with_track_patterns_and_transposes(
        song,
        |track| {
            if track.id() == track_id {
                Some(variation_id.to_owned())
            } else {
                track.initial_pattern().map(ToOwned::to_owned)
            }
        },
        &BTreeMap::new(),
    )
}

pub fn compile_mixed_note_song_with_lane_variation_and_transposes(
    song: &ValidatedSong,
    lane_id: &str,
    variation_id: &str,
    lane_transpose_semitones: &BTreeMap<String, i8>,
) -> Result<CompiledMixedNoteSong, SongRenderError> {
    let lane = song
        .performance()
        .lanes()
        .iter()
        .find(|lane| lane.id() == lane_id)
        .ok_or_else(|| SongRenderError::MissingLane {
            id: lane_id.to_owned(),
        })?;
    if !lane
        .variation_ids()
        .iter()
        .any(|available| available == variation_id)
    {
        return Err(SongRenderError::MissingPattern {
            id: variation_id.to_owned(),
        });
    }
    let track_id = lane
        .track_id()
        .ok_or_else(|| SongRenderError::MissingLane {
            id: lane_id.to_owned(),
        })?;
    compile_mixed_note_song_with_track_patterns_and_transposes(
        song,
        |track| {
            if track.id() == track_id {
                Some(variation_id.to_owned())
            } else {
                track.initial_pattern().map(ToOwned::to_owned)
            }
        },
        lane_transpose_semitones,
    )
}

pub fn compile_mixed_note_song_with_lane_variations_and_transposes(
    song: &ValidatedSong,
    lane_variations: &BTreeMap<String, String>,
    lane_transpose_semitones: &BTreeMap<String, i8>,
) -> Result<CompiledMixedNoteSong, SongRenderError> {
    let mut track_patterns = BTreeMap::new();
    for (lane_id, variation_id) in lane_variations {
        let lane = song
            .performance()
            .lanes()
            .iter()
            .find(|lane| lane.id() == lane_id)
            .ok_or_else(|| SongRenderError::MissingLane {
                id: lane_id.clone(),
            })?;
        if !lane
            .variation_ids()
            .iter()
            .any(|available| available == variation_id)
        {
            return Err(SongRenderError::MissingPattern {
                id: variation_id.clone(),
            });
        }
        let track_id = lane
            .track_id()
            .ok_or_else(|| SongRenderError::MissingLane {
                id: lane_id.clone(),
            })?;
        track_patterns.insert(track_id.to_owned(), variation_id.clone());
    }
    compile_mixed_note_song_with_track_patterns_and_transposes(
        song,
        |track| {
            track_patterns
                .get(track.id())
                .cloned()
                .or_else(|| track.initial_pattern().map(ToOwned::to_owned))
        },
        lane_transpose_semitones,
    )
}

fn compile_mixed_note_song_with_track_patterns_and_transposes(
    song: &ValidatedSong,
    pattern_for_track: impl Fn(&TrackDefinition) -> Option<String>,
    lane_transpose_semitones: &BTreeMap<String, i8>,
) -> Result<CompiledMixedNoteSong, SongRenderError> {
    let mut tracks = Vec::with_capacity(song.performance().tracks().len());
    let mut channels = None;
    for track in song.performance().tracks() {
        let pattern_id =
            pattern_for_track(track).ok_or_else(|| SongRenderError::MissingPattern {
                id: format!("initial pattern for track '{}'", track.id()),
            })?;
        let transpose = track_lane_transpose_semitones(song, track, lane_transpose_semitones);
        let patch = if track.sample_bank_id().is_some() {
            CompiledMixedTrack::Sample(compile_sample_track_for_pattern(
                song,
                track,
                &pattern_id,
                transpose,
            )?)
        } else {
            CompiledMixedTrack::Note(compile_note_track_for_pattern(
                song,
                track,
                &pattern_id,
                transpose,
            )?)
        };
        if let Some(channels) = channels {
            if patch.channels() != channels {
                return Err(SongRenderError::IncompatibleTrackRender);
            }
        } else {
            channels = Some(patch.channels());
        }
        tracks.push(patch);
    }
    Ok(CompiledMixedNoteSong {
        song_fingerprint: song.fingerprint(),
        sample_rate: song.performance().sample_rate(),
        channels: channels.unwrap_or(1),
        tracks,
    })
}

fn track_lane_transpose_semitones(
    song: &ValidatedSong,
    track: &TrackDefinition,
    lane_transpose_semitones: &BTreeMap<String, i8>,
) -> i8 {
    song.performance()
        .lanes()
        .iter()
        .find(|lane| lane.track_id() == Some(track.id()))
        .and_then(|lane| lane_transpose_semitones.get(lane.id()))
        .copied()
        .unwrap_or(0)
}

fn compile_sample_track_for_pattern(
    song: &ValidatedSong,
    track: &TrackDefinition,
    pattern_id: &str,
    transpose_semitones: i8,
) -> Result<CompiledSampleTrack, SongRenderError> {
    let sampler = track.sampler_id().and_then(|id| song.samplers().get(id));
    let sample_bank_id =
        track
            .sample_bank_id()
            .ok_or_else(|| SongRenderError::MissingSampleBank {
                id: format!("sample bank for track '{}'", track.id()),
            })?;
    let sample_bank = song.sample_banks().get(sample_bank_id).ok_or_else(|| {
        SongRenderError::MissingSampleBank {
            id: sample_bank_id.to_owned(),
        }
    })?;
    let sample_bank_path =
        track
            .sample_bank_path()
            .ok_or_else(|| SongRenderError::MissingSampleBank {
                id: sample_bank_id.to_owned(),
            })?;
    let pattern =
        song.note_patterns()
            .get(pattern_id)
            .ok_or_else(|| SongRenderError::MissingPattern {
                id: pattern_id.to_owned(),
            })?;
    let pitch_envelope = sampler
        .and_then(|sampler| sampler.pitch_envelope())
        .map(|envelope| CompiledSamplePitchEnvelope {
            amount_semitones: envelope.amount_semitones(),
            attack_frames: seconds_to_frames(
                envelope.attack_seconds(),
                song.performance().sample_rate(),
            ),
            decay_frames: seconds_to_frames(
                envelope.decay_seconds(),
                song.performance().sample_rate(),
            ),
            sustain_level: envelope.sustain_level(),
        });
    let mut events = Vec::with_capacity(pattern.events().len());
    for event in pattern.events() {
        let slot = resolve_sample_slot(sample_bank, sampler, event.sample_slot())?;
        let sample_path = song
            .root()
            .join(sample_bank_path)
            .parent()
            .expect("sample bank path has a parent")
            .join(slot.path());
        let sample = read_wav(&sample_path).map_err(|error| SongRenderError::SampleLoad {
            path: sample_path.display().to_string(),
            message: error.to_string(),
        })?;
        if sample.sample_rate() != song.performance().sample_rate() {
            return Err(SongRenderError::UnsupportedSampleRate {
                found: sample.sample_rate(),
                expected: song.performance().sample_rate(),
            });
        }
        let (slice_start, slice_end) = compile_sample_slice(
            slot,
            event
                .sample_slice()
                .or_else(|| sampler.and_then(|sampler| sampler.default_slice())),
            &sample,
        )?;
        let root_note = event
            .root_note()
            .or_else(|| sampler.and_then(|sampler| sampler.root_note()))
            .unwrap_or(event.note());
        let tracking_semitones = if sampler.is_none_or(|sampler| sampler.pitch_tracking()) {
            f64::from(event.note()) - f64::from(root_note)
        } else {
            0.0
        };
        let base_semitones = sampler.map_or(0.0, |sampler| sampler.pitch_semitones())
            + slot.pitch_semitones()
            + event.pitch_semitones()
            + tracking_semitones
            + f64::from(transpose_semitones);
        events.push(CompiledSampleEvent {
            start: event.start_frame(),
            duration: event.duration_frames(),
            velocity: event.velocity(),
            sample,
            slice_start,
            slice_end,
            pitch_ratio: 2.0_f64.powf(base_semitones / 12.0),
        });
    }
    Ok(CompiledSampleTrack {
        sample_rate: song.performance().sample_rate(),
        channels: events.first().map_or(1, |event| event.sample.channels()),
        level: sampler.map_or(1.0, |sampler| sampler.level()),
        pitch_envelope,
        pattern_length: pattern.length_frames(),
        looped: pattern.is_looped(),
        events,
    })
}

fn compile_sample_slice(
    slot: &SampleSlotDefinition,
    slice_id: Option<&str>,
    sample: &SampleBuffer,
) -> Result<(u64, u64), SongRenderError> {
    let slice = if let Some(slice_id) = slice_id {
        slot.slices()
            .iter()
            .find(|slice| slice.id() == slice_id)
            .ok_or_else(|| SongRenderError::MissingSampleSlice {
                id: slice_id.to_owned(),
            })?
    } else {
        let Some(slice) = slot.slices().first() else {
            return Ok((0, u64::from(sample.frames())));
        };
        slice
    };
    let start = parse_sample_timestamp(slice.start(), sample.sample_rate()).ok_or_else(|| {
        SongRenderError::MissingSampleSlice {
            id: slice.id().to_owned(),
        }
    })?;
    let end = parse_sample_timestamp(slice.end(), sample.sample_rate()).ok_or_else(|| {
        SongRenderError::MissingSampleSlice {
            id: slice.id().to_owned(),
        }
    })?;
    Ok((
        start.min(u64::from(sample.frames())),
        end.min(u64::from(sample.frames())),
    ))
}

fn resolve_sample_slot<'a>(
    sample_bank: &'a meldritch_dsl::SampleBankDefinition,
    sampler: Option<&meldritch_dsl::SamplerDefinition>,
    event_slot: Option<&str>,
) -> Result<&'a SampleSlotDefinition, SongRenderError> {
    let slot_id = event_slot.or_else(|| sampler.and_then(|sampler| sampler.default_slot()));
    if let Some(slot_id) = slot_id {
        sample_bank
            .slots()
            .iter()
            .find(|slot| slot.id() == slot_id)
            .ok_or_else(|| SongRenderError::MissingSampleSlot {
                id: slot_id.to_owned(),
            })
    } else {
        sample_bank
            .slots()
            .first()
            .ok_or_else(|| SongRenderError::MissingSampleSlot {
                id: format!("first slot in sample bank '{}'", sample_bank.id()),
            })
    }
}

fn seconds_to_frames(seconds: f64, sample_rate: u32) -> u64 {
    (seconds.max(0.0) * f64::from(sample_rate)).round() as u64
}

fn parse_sample_timestamp(value: &str, sample_rate: u32) -> Option<u64> {
    let (minutes, seconds) = value.split_once(':')?;
    let minutes = minutes.parse::<u64>().ok()?;
    let seconds = seconds.parse::<f64>().ok()?;
    if !seconds.is_finite() || seconds < 0.0 {
        return None;
    }
    Some(((minutes as f64 * 60.0 + seconds) * f64::from(sample_rate)).round() as u64)
}

pub fn compile_filtered_note_song(
    song: &ValidatedSong,
) -> Result<CompiledFilteredNotePatch, SongRenderError> {
    let [track] = song.performance().tracks() else {
        return Err(SongRenderError::TrackCount {
            found: song.performance().tracks().len(),
        });
    };
    let synth =
        song.synths()
            .get(track.synth_id())
            .ok_or_else(|| SongRenderError::MissingSynth {
                id: track.synth_id().to_owned(),
            })?;
    let oscillator = exactly_one_module(synth.modules(), ModuleKind::Oscillator)?;
    let filter = exactly_one_module(synth.modules(), ModuleKind::LowPass)?;
    let envelope = exactly_one_module(synth.modules(), ModuleKind::Adsr)?;
    let amplifier = exactly_one_module(synth.modules(), ModuleKind::Vca)?;
    let output = exactly_one_module(synth.modules(), ModuleKind::AudioOutput)?;
    for (from, to) in [
        (
            "input.pitch".to_owned(),
            format!("{}.pitch", oscillator.id()),
        ),
        ("input.gate".to_owned(), format!("{}.gate", envelope.id())),
        (
            format!("{}.audio", oscillator.id()),
            format!("{}.audio", filter.id()),
        ),
        (
            format!("{}.audio", filter.id()),
            format!("{}.audio", amplifier.id()),
        ),
        (
            format!("{}.control", envelope.id()),
            format!("{}.level", amplifier.id()),
        ),
        (
            format!("{}.audio", amplifier.id()),
            format!("{}.audio", output.id()),
        ),
    ] {
        require_cable(synth, from, to)?;
    }
    let note_pattern_id = track
        .initial_pattern()
        .ok_or_else(|| SongRenderError::MissingPattern { id: String::new() })?;
    let note_pattern = song.note_patterns().get(note_pattern_id).ok_or_else(|| {
        SongRenderError::MissingPattern {
            id: note_pattern_id.to_owned(),
        }
    })?;
    let [parameter_pattern_id] = track.parameter_pattern_ids() else {
        return Err(SongRenderError::MissingPattern {
            id: "one active parameter pattern is required".to_owned(),
        });
    };
    let parameter_pattern = song
        .parameter_patterns()
        .get(parameter_pattern_id)
        .ok_or_else(|| SongRenderError::MissingPattern {
            id: parameter_pattern_id.to_owned(),
        })?;
    let lane = parameter_pattern
        .lanes()
        .iter()
        .find(|lane| {
            lane.target().owner() == &ParameterOwner::Synth
                && lane.target().definition_id() == synth.id()
                && lane.target().module_id() == filter.id()
                && lane.target().parameter() == "cutoff_hz"
        })
        .ok_or_else(|| SongRenderError::MissingPattern {
            id: format!("cutoff lane in {parameter_pattern_id}"),
        })?;
    let waveform = parse_waveform(oscillator.id(), oscillator.waveform().unwrap_or("sine"))?;
    if lane.interpolation() != ParameterInterpolation::Linear {
        return Err(SongRenderError::UnsupportedInterpolation);
    }
    Ok(CompiledFilteredNotePatch {
        sample_rate: song.performance().sample_rate(),
        channels: output.channels().unwrap_or(1),
        waveform,
        envelope: AdsrSettings {
            attack_seconds: envelope.attack().unwrap_or(0.0),
            decay_seconds: envelope.decay().unwrap_or(0.0),
            sustain_level: envelope.sustain().unwrap_or(1.0),
            release_seconds: envelope.release().unwrap_or(0.0),
        },
        resonance: filter.resonance().unwrap_or(0.0),
        pattern_length: note_pattern.length_frames(),
        note_looped: note_pattern.is_looped(),
        parameter_length: parameter_pattern.length_frames(),
        parameter_looped: parameter_pattern.is_looped(),
        cutoff_points: lane
            .points()
            .iter()
            .map(|point| CompiledParameterPoint {
                frame: point.frame(),
                value: point.value(),
            })
            .collect(),
        events: note_pattern
            .events()
            .iter()
            .map(|event| CompiledNoteEvent {
                start: event.start_frame(),
                end: event.start_frame() + event.duration_frames(),
                note: event.note(),
                velocity: event.velocity(),
            })
            .collect(),
    })
}

pub fn compile_delayed_note_song(
    song: &ValidatedSong,
) -> Result<CompiledDelayedNotePatch, SongRenderError> {
    let source = compile_note_song(song)?;
    compile_delayed_note_song_from_source(song, source)
}

pub fn compile_delayed_note_song_for_pattern(
    song: &ValidatedSong,
    pattern_id: &str,
) -> Result<CompiledDelayedNotePatch, SongRenderError> {
    let source = compile_note_song_for_pattern(song, pattern_id)?;
    compile_delayed_note_song_from_source(song, source)
}

fn compile_delayed_note_song_from_source(
    song: &ValidatedSong,
    source: CompiledNotePatch,
) -> Result<CompiledDelayedNotePatch, SongRenderError> {
    let [track] = song.performance().tracks() else {
        return Err(SongRenderError::TrackCount {
            found: song.performance().tracks().len(),
        });
    };
    let [dsp_id] = track.dsp_ids() else {
        return Err(SongRenderError::DspCount {
            found: track.dsp_ids().len(),
        });
    };
    let dsp = song
        .dsps()
        .get(dsp_id)
        .ok_or_else(|| SongRenderError::MissingDsp { id: dsp_id.clone() })?;
    let delay = exactly_one_module(dsp.modules(), ModuleKind::TempoDelay)?;
    let output = exactly_one_module(dsp.modules(), ModuleKind::AudioOutput)?;
    require_dsp_cable(
        dsp,
        "input.audio".to_owned(),
        format!("{}.audio", delay.id()),
    )?;
    require_dsp_cable(
        dsp,
        format!("{}.audio", delay.id()),
        format!("{}.audio", output.id()),
    )?;
    let duration = delay
        .time()
        .ok_or_else(|| SongRenderError::InvalidTempoDuration {
            value: String::new(),
        })?;
    let beats = tempo_duration_beats(duration)?;
    let delay_frames = (beats * f64::from(song.performance().sample_rate()) * 60.0
        / song.performance().bpm())
    .round() as u64;
    Ok(CompiledDelayedNotePatch {
        source,
        delay_frames,
        feedback: delay.feedback().unwrap_or(0.0),
        mix: delay.mix().unwrap_or(0.0),
        feedback_pattern_length: None,
        feedback_pattern_looped: false,
        feedback_points: Vec::new(),
    })
}

pub fn compile_automated_delayed_note_song(
    song: &ValidatedSong,
) -> Result<CompiledDelayedNotePatch, SongRenderError> {
    let mut patch = compile_delayed_note_song(song)?;
    let [track] = song.performance().tracks() else {
        return Err(SongRenderError::TrackCount {
            found: song.performance().tracks().len(),
        });
    };
    let [pattern_id] = track.parameter_pattern_ids() else {
        return Err(SongRenderError::MissingPattern {
            id: "one active DSP parameter pattern is required".to_owned(),
        });
    };
    let pattern = song.parameter_patterns().get(pattern_id).ok_or_else(|| {
        SongRenderError::MissingPattern {
            id: pattern_id.to_owned(),
        }
    })?;
    let [dsp_id] = track.dsp_ids() else {
        return Err(SongRenderError::DspCount {
            found: track.dsp_ids().len(),
        });
    };
    let lane = pattern
        .lanes()
        .iter()
        .find(|lane| {
            lane.target().owner() == &ParameterOwner::Dsp
                && lane.target().definition_id() == dsp_id
                && lane.target().parameter() == "feedback"
        })
        .ok_or_else(|| SongRenderError::MissingPattern {
            id: format!("delay feedback lane in {pattern_id}"),
        })?;
    if lane.interpolation() != ParameterInterpolation::Step {
        return Err(SongRenderError::UnsupportedInterpolation);
    }
    patch.feedback_pattern_length = Some(pattern.length_frames());
    patch.feedback_pattern_looped = pattern.is_looped();
    patch.feedback_points = lane
        .points()
        .iter()
        .map(|point| CompiledParameterPoint {
            frame: point.frame(),
            value: point.value(),
        })
        .collect();
    Ok(patch)
}

fn require_dsp_cable(
    dsp: &meldritch_dsl::DspDefinition,
    from: String,
    to: String,
) -> Result<(), SongRenderError> {
    if dsp
        .cables()
        .iter()
        .any(|cable| cable.from() == from && cable.to() == to)
    {
        Ok(())
    } else {
        Err(SongRenderError::MissingCable { from, to })
    }
}

fn tempo_duration_beats(value: &str) -> Result<f64, SongRenderError> {
    let (numerator, denominator) =
        value
            .split_once('/')
            .ok_or_else(|| SongRenderError::InvalidTempoDuration {
                value: value.to_owned(),
            })?;
    let numerator = numerator
        .parse::<u32>()
        .ok()
        .filter(|value| *value > 0)
        .ok_or_else(|| SongRenderError::InvalidTempoDuration {
            value: value.to_owned(),
        })?;
    let denominator = denominator
        .parse::<u32>()
        .ok()
        .filter(|value| *value > 0)
        .ok_or_else(|| SongRenderError::InvalidTempoDuration {
            value: value.to_owned(),
        })?;
    Ok(4.0 * f64::from(numerator) / f64::from(denominator))
}

fn require_cable(
    synth: &meldritch_dsl::SynthDefinition,
    from: String,
    to: String,
) -> Result<(), SongRenderError> {
    if synth
        .cables()
        .iter()
        .any(|cable| cable.from() == from && cable.to() == to)
    {
        Ok(())
    } else {
        Err(SongRenderError::MissingCable { from, to })
    }
}

fn interpolated_value(points: &[CompiledParameterPoint], frame: u64) -> f64 {
    let first = points
        .first()
        .expect("validated parameter lanes contain points");
    if frame <= first.frame {
        return first.value;
    }
    for pair in points.windows(2) {
        let left = pair[0];
        let right = pair[1];
        if frame <= right.frame {
            let span = (right.frame - left.frame) as f64;
            let position = (frame - left.frame) as f64 / span;
            return left.value + (right.value - left.value) * position;
        }
    }
    points.last().expect("lane is not empty").value
}

fn stepped_value(points: &[CompiledParameterPoint], frame: u64) -> f64 {
    points
        .iter()
        .rev()
        .find(|point| point.frame <= frame)
        .or_else(|| points.first())
        .expect("validated parameter lanes contain points")
        .value
}

fn exactly_one_module(
    modules: &[meldritch_dsl::ModuleDefinition],
    kind: ModuleKind,
) -> Result<&meldritch_dsl::ModuleDefinition, SongRenderError> {
    let found = modules
        .iter()
        .filter(|module| module.kind() == kind)
        .collect::<Vec<_>>();
    match found.as_slice() {
        [] => Err(SongRenderError::MissingModule { kind }),
        [module] => Ok(*module),
        _ => Err(SongRenderError::MultipleModules { kind }),
    }
}

fn optional_one_module(
    modules: &[meldritch_dsl::ModuleDefinition],
    kind: ModuleKind,
) -> Result<Option<&meldritch_dsl::ModuleDefinition>, SongRenderError> {
    let found = modules
        .iter()
        .filter(|module| module.kind() == kind)
        .collect::<Vec<_>>();
    match found.as_slice() {
        [] => Ok(None),
        [module] => Ok(Some(*module)),
        _ => Err(SongRenderError::MultipleModules { kind }),
    }
}

fn parse_waveform(id: &str, waveform: &str) -> Result<Waveform, SongRenderError> {
    match waveform {
        "sine" => Ok(Waveform::Sine),
        "triangle" => Ok(Waveform::Triangle),
        "saw" => Ok(Waveform::Saw),
        "pulse" | "square" => Ok(Waveform::Pulse),
        _ => Err(SongRenderError::UnsupportedWaveform {
            id: id.to_owned(),
            waveform: waveform.to_owned(),
        }),
    }
}

fn midi_frequency(note: u8) -> f64 {
    440.0 * 2.0_f64.powf((f64::from(note) - 69.0) / 12.0)
}

impl std::error::Error for SongRenderError {}

pub fn compile_drone_song(song: &ValidatedSong) -> Result<CompiledDronePatch, SongRenderError> {
    let [track] = song.performance().tracks() else {
        return Err(SongRenderError::TrackCount {
            found: song.performance().tracks().len(),
        });
    };
    let synth =
        song.synths()
            .get(track.synth_id())
            .ok_or_else(|| SongRenderError::MissingSynth {
                id: track.synth_id().to_owned(),
            })?;

    let outputs = synth
        .modules()
        .iter()
        .filter(|module| module.kind() == ModuleKind::AudioOutput)
        .collect::<Vec<_>>();
    let output = match outputs.as_slice() {
        [] => return Err(SongRenderError::MissingOutput),
        [output] => *output,
        _ => return Err(SongRenderError::MultipleOutputs),
    };
    let output_endpoint = format!("{}.audio", output.id());
    let drivers = synth
        .cables()
        .iter()
        .filter(|cable| cable.to() == output_endpoint)
        .collect::<Vec<_>>();
    let driver = match drivers.as_slice() {
        [] => {
            return Err(SongRenderError::UnconnectedOutput {
                id: output.id().to_owned(),
            });
        }
        [driver] => *driver,
        _ => {
            return Err(SongRenderError::MultiplyDrivenOutput {
                id: output.id().to_owned(),
            });
        }
    };
    let (oscillator_id, port) = driver
        .from()
        .split_once('.')
        .expect("validated cable endpoints contain a dot");
    let oscillator = synth
        .modules()
        .iter()
        .find(|module| module.id() == oscillator_id && module.kind() == ModuleKind::Oscillator)
        .ok_or_else(|| SongRenderError::OutputNotDrivenByOscillator {
            endpoint: driver.from().to_owned(),
        })?;
    if port != "audio" {
        return Err(SongRenderError::OutputNotDrivenByOscillator {
            endpoint: driver.from().to_owned(),
        });
    }
    for module in synth.modules() {
        if !matches!(
            module.kind(),
            ModuleKind::Oscillator | ModuleKind::AudioOutput
        ) {
            return Err(SongRenderError::UnsupportedModule {
                id: module.id().to_owned(),
                kind: module.kind(),
            });
        }
    }
    let waveform_name = oscillator.waveform().unwrap_or("sine");
    let waveform = match waveform_name {
        "sine" => Waveform::Sine,
        "triangle" => Waveform::Triangle,
        "saw" => Waveform::Saw,
        "pulse" => Waveform::Pulse,
        _ => {
            return Err(SongRenderError::UnsupportedWaveform {
                id: oscillator.id().to_owned(),
                waveform: waveform_name.to_owned(),
            });
        }
    };
    Ok(CompiledDronePatch {
        song_fingerprint: song.fingerprint(),
        sample_rate: song.performance().sample_rate(),
        channels: output.channels().unwrap_or(1),
        frequency_hz: oscillator.frequency_hz().ok_or_else(|| {
            SongRenderError::MissingFrequency {
                id: oscillator.id().to_owned(),
            }
        })?,
        waveform,
    })
}
