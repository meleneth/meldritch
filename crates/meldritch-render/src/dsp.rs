use meldritch_audio::{AudioBlock, SampleBuffer};
use meldritch_core::{
    AutomationLane, AutomationTarget, AutomationValue, FrameRange, Pattern, ProbabilitySeed,
    SampleRate, Tempo,
};

use crate::RenderSettings;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Waveform {
    Sine,
    Triangle,
    Saw,
    Pulse,
    SyncFold,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Oscillator {
    waveform: Waveform,
    phase: f64,
    slave_phase: f64,
    pulse_width: f64,
    sync_ratio: f64,
    phase_modulation: f64,
    fold_amount: f64,
}

impl Oscillator {
    #[must_use]
    pub fn new(waveform: Waveform) -> Self {
        Self {
            waveform,
            phase: 0.0,
            slave_phase: 0.0,
            pulse_width: 0.5,
            sync_ratio: 3.0,
            phase_modulation: 0.35,
            fold_amount: 2.5,
        }
    }

    #[must_use]
    pub fn with_pulse_width(mut self, pulse_width: f64) -> Self {
        self.pulse_width = pulse_width.clamp(0.01, 0.99);
        self
    }

    #[must_use]
    pub fn with_sync_fold(mut self, sync_ratio: f64, phase_modulation: f64, fold: f64) -> Self {
        self.sync_ratio = sync_ratio.clamp(1.0, 16.0);
        self.phase_modulation = phase_modulation.clamp(0.0, 2.0);
        self.fold_amount = fold.clamp(1.0, 8.0);
        self
    }

    pub fn set_waveform(&mut self, waveform: Waveform) {
        self.waveform = waveform;
    }

    pub fn next(&mut self, frequency: f64, sample_rate: SampleRate) -> f64 {
        let increment = (frequency / f64::from(sample_rate)).clamp(0.0, 0.5);
        let value = match self.waveform {
            Waveform::Sine => (std::f64::consts::TAU * self.phase).sin(),
            Waveform::Triangle => 1.0 - 4.0 * (self.phase - 0.5).abs(),
            Waveform::Saw => self.phase.mul_add(2.0, -1.0) - poly_blep(self.phase, increment),
            Waveform::Pulse => {
                let shifted = (self.phase - self.pulse_width).rem_euclid(1.0);
                let raw = if self.phase < self.pulse_width {
                    1.0
                } else {
                    -1.0
                };
                raw + poly_blep(self.phase, increment) - poly_blep(shifted, increment)
            }
            Waveform::SyncFold => {
                let modulation =
                    (std::f64::consts::TAU * self.phase).sin() * self.phase_modulation * 0.25;
                let modulated_phase = (self.slave_phase + modulation).rem_euclid(1.0);
                let raw = modulated_phase.mul_add(2.0, -1.0);
                wavefold(raw * self.fold_amount)
            }
        };
        let next_phase = self.phase + increment;
        let wrapped = next_phase >= 1.0;
        self.phase = next_phase.fract();
        if wrapped {
            self.slave_phase = 0.0;
        } else {
            self.slave_phase = (self.slave_phase + increment * self.sync_ratio).rem_euclid(1.0);
        }
        value
    }
}

fn wavefold(value: f64) -> f64 {
    let wrapped = (value + 1.0).rem_euclid(4.0);
    if wrapped <= 2.0 {
        wrapped - 1.0
    } else {
        3.0 - wrapped
    }
}

fn poly_blep(phase: f64, increment: f64) -> f64 {
    if increment <= 0.0 {
        return 0.0;
    }
    if phase < increment {
        let normalized = phase / increment;
        normalized + normalized - normalized * normalized - 1.0
    } else if phase > 1.0 - increment {
        let normalized = (phase - 1.0) / increment;
        normalized * normalized + normalized + normalized + 1.0
    } else {
        0.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AdsrSettings {
    pub attack_seconds: f64,
    pub decay_seconds: f64,
    pub sustain_level: f64,
    pub release_seconds: f64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EnvelopeStage {
    Idle,
    Attack,
    Decay,
    Sustain,
    Release,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AdsrEnvelope {
    settings: AdsrSettings,
    sample_rate: SampleRate,
    stage: EnvelopeStage,
    level: f64,
    release_step: f64,
}

impl AdsrEnvelope {
    #[must_use]
    pub fn new(settings: AdsrSettings, sample_rate: SampleRate) -> Self {
        Self {
            settings,
            sample_rate,
            stage: EnvelopeStage::Idle,
            level: 0.0,
            release_step: 0.0,
        }
    }

    pub fn note_on(&mut self) {
        self.stage = EnvelopeStage::Attack;
    }

    pub fn note_off(&mut self) {
        let frames = seconds_to_frames(self.settings.release_seconds, self.sample_rate);
        if frames == 0 {
            self.level = 0.0;
            self.stage = EnvelopeStage::Idle;
        } else {
            self.release_step = self.level / frames as f64;
            self.stage = EnvelopeStage::Release;
        }
    }

    #[must_use]
    pub const fn stage(&self) -> EnvelopeStage {
        self.stage
    }

    pub fn next_value(&mut self) -> f64 {
        match self.stage {
            EnvelopeStage::Idle => self.level = 0.0,
            EnvelopeStage::Attack => {
                let frames = seconds_to_frames(self.settings.attack_seconds, self.sample_rate);
                if frames == 0 {
                    self.level = 1.0;
                    self.stage = EnvelopeStage::Decay;
                } else {
                    self.level = (self.level + 1.0 / frames as f64).min(1.0);
                    if self.level >= 1.0 {
                        self.stage = EnvelopeStage::Decay;
                    }
                }
            }
            EnvelopeStage::Decay => {
                let sustain = self.settings.sustain_level.clamp(0.0, 1.0);
                let frames = seconds_to_frames(self.settings.decay_seconds, self.sample_rate);
                if frames == 0 {
                    self.level = sustain;
                    self.stage = EnvelopeStage::Sustain;
                } else {
                    self.level = (self.level - (1.0 - sustain) / frames as f64).max(sustain);
                    if self.level <= sustain {
                        self.stage = EnvelopeStage::Sustain;
                    }
                }
            }
            EnvelopeStage::Sustain => self.level = self.settings.sustain_level.clamp(0.0, 1.0),
            EnvelopeStage::Release => {
                self.level = (self.level - self.release_step).max(0.0);
                if self.level <= 0.0 {
                    self.stage = EnvelopeStage::Idle;
                }
            }
        }
        self.level
    }
}

fn seconds_to_frames(seconds: f64, sample_rate: SampleRate) -> u64 {
    (seconds.max(0.0) * f64::from(sample_rate)).round() as u64
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FilterMode {
    LowPass,
    BandPass,
    HighPass,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct StateVariableFilter {
    integrator_one: f64,
    integrator_two: f64,
}

impl StateVariableFilter {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            integrator_one: 0.0,
            integrator_two: 0.0,
        }
    }

    pub fn process(
        &mut self,
        input: f64,
        cutoff_hz: f64,
        resonance: f64,
        mode: FilterMode,
        sample_rate: SampleRate,
    ) -> f64 {
        let rate = f64::from(sample_rate);
        let cutoff = cutoff_hz.clamp(1.0, rate * 0.49);
        let resonance = resonance.clamp(0.0, 0.99);
        let damping = 2.0 - 2.0 * resonance;
        let coefficient = (std::f64::consts::PI * cutoff / rate).tan();
        let normalization = 1.0 / (1.0 + coefficient * (coefficient + damping));
        let delta = input - self.integrator_two;
        let band = normalization * (self.integrator_one + coefficient * delta);
        let low = self.integrator_two + coefficient * band;
        self.integrator_one = 2.0 * band - self.integrator_one;
        self.integrator_two = 2.0 * low - self.integrator_two;
        let high = input - damping * band - low;
        match mode {
            FilterMode::LowPass => low,
            FilterMode::BandPass => band,
            FilterMode::HighPass => high,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BassVoiceSettings {
    pub level: f64,
    pub waveform: Waveform,
    pub attack_seconds: f64,
    pub decay_seconds: f64,
    pub sustain_level: f64,
    pub release_seconds: f64,
    pub cutoff_hz: f64,
    pub resonance: f64,
    pub filter_envelope_octaves: f64,
    pub pre_filter_drive: f64,
    pub drive: f64,
    pub sub_level: f64,
    pub glide_seconds: f64,
    pub ducking_amount: f64,
    pub ducking_release_seconds: f64,
    pub hat_filter_octaves: f64,
    pub hat_filter_release_seconds: f64,
    pub accent_velocity_gain: f64,
    pub accent_filter_octaves: f64,
    pub accent_release_seconds: f64,
}

impl Default for BassVoiceSettings {
    fn default() -> Self {
        Self {
            level: 0.35,
            waveform: Waveform::Saw,
            attack_seconds: 0.005,
            decay_seconds: 0.08,
            sustain_level: 0.72,
            release_seconds: 0.08,
            cutoff_hz: 180.0,
            resonance: 0.55,
            filter_envelope_octaves: 2.5,
            pre_filter_drive: 1.0,
            drive: 1.6,
            sub_level: 0.25,
            glide_seconds: 0.04,
            ducking_amount: 0.35,
            ducking_release_seconds: 0.12,
            hat_filter_octaves: 1.25,
            hat_filter_release_seconds: 0.08,
            accent_velocity_gain: 1.2,
            accent_filter_octaves: 0.75,
            accent_release_seconds: 0.06,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BassVoice {
    settings: BassVoiceSettings,
    sample_rate: SampleRate,
    oscillator: Oscillator,
    sub_oscillator: Oscillator,
    envelope: AdsrEnvelope,
    filter: StateVariableFilter,
    frequency: f64,
    target_frequency: f64,
    glide_step: f64,
    glide_frames_remaining: u64,
    velocity: f64,
}

impl BassVoice {
    #[must_use]
    pub fn new(settings: BassVoiceSettings, sample_rate: SampleRate) -> Self {
        Self {
            settings,
            sample_rate,
            oscillator: Oscillator::new(settings.waveform),
            sub_oscillator: Oscillator::new(Waveform::Sine),
            envelope: AdsrEnvelope::new(
                AdsrSettings {
                    attack_seconds: settings.attack_seconds,
                    decay_seconds: settings.decay_seconds,
                    sustain_level: settings.sustain_level,
                    release_seconds: settings.release_seconds,
                },
                sample_rate,
            ),
            filter: StateVariableFilter::new(),
            frequency: 0.0,
            target_frequency: 0.0,
            glide_step: 0.0,
            glide_frames_remaining: 0,
            velocity: 0.0,
        }
    }

    pub fn note_on(&mut self, note: u8, velocity: f64) {
        self.set_note_frequency(midi_note_hz(note));
        self.velocity = velocity.clamp(0.0, 1.0);
        self.envelope.note_on();
    }

    fn note_on_frequency(&mut self, frequency: f64, velocity: f64) {
        self.set_note_frequency(frequency);
        self.velocity = velocity.clamp(0.0, 1.0);
        self.envelope.note_on();
    }

    pub fn legato_note_on(&mut self, note: u8, velocity: f64) {
        self.set_note_frequency(midi_note_hz(note));
        self.velocity = velocity.clamp(0.0, 1.0);
        if self.envelope.stage() == EnvelopeStage::Idle {
            self.envelope.note_on();
        }
    }

    fn set_note_frequency(&mut self, frequency: f64) {
        self.target_frequency = frequency;
        let glide_frames = seconds_to_frames(self.settings.glide_seconds, self.sample_rate);
        if self.frequency <= 0.0 || glide_frames == 0 {
            self.frequency = frequency;
            self.glide_step = 0.0;
            self.glide_frames_remaining = 0;
        } else {
            self.glide_step = (frequency - self.frequency) / glide_frames as f64;
            self.glide_frames_remaining = glide_frames;
        }
    }

    pub fn note_off(&mut self) {
        self.envelope.note_off();
    }

    pub fn set_waveform(&mut self, waveform: Waveform) {
        self.oscillator.set_waveform(waveform);
    }

    #[must_use]
    pub const fn is_idle(&self) -> bool {
        matches!(self.envelope.stage(), EnvelopeStage::Idle)
    }

    #[must_use]
    pub const fn current_frequency(&self) -> f64 {
        self.frequency
    }

    #[must_use]
    pub const fn envelope_stage(&self) -> EnvelopeStage {
        self.envelope.stage()
    }

    pub fn next_sample(&mut self) -> f64 {
        self.next_sample_with_filter_offset(0.0)
    }

    pub fn next_sample_with_filter_offset(&mut self, filter_octaves: f64) -> f64 {
        self.next_sample_with_parameters(
            filter_octaves,
            self.settings.cutoff_hz,
            self.settings.resonance,
            self.settings.filter_envelope_octaves,
            self.settings.drive,
            self.settings.level,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn next_sample_with_parameters(
        &mut self,
        filter_octaves: f64,
        cutoff_hz: f64,
        resonance: f64,
        filter_envelope_octaves: f64,
        drive: f64,
        level: f64,
    ) -> f64 {
        if self.glide_frames_remaining > 0 {
            self.frequency += self.glide_step;
            self.glide_frames_remaining -= 1;
            if self.glide_frames_remaining == 0 {
                self.frequency = self.target_frequency;
            }
        }
        let main = self.oscillator.next(self.frequency, self.sample_rate);
        let sub_level = self.settings.sub_level.clamp(0.0, 1.0);
        let sub = self
            .sub_oscillator
            .next(self.frequency * 0.5, self.sample_rate);
        let source = (main + sub * sub_level) / (1.0 + sub_level);
        let envelope_level = self.envelope.next_value();
        let cutoff = cutoff_hz.max(1.0)
            * 2.0_f64.powf(envelope_level * filter_envelope_octaves.max(0.0) + filter_octaves);
        let pre_driven = normalized_drive(source, self.settings.pre_filter_drive);
        let filtered = self.filter.process(
            pre_driven,
            cutoff,
            resonance,
            FilterMode::LowPass,
            self.sample_rate,
        );
        let driven = normalized_drive(filtered, drive);
        driven * envelope_level * self.velocity * level.clamp(0.0, 1.0)
    }

    pub fn render_add(&mut self, output: &mut [f64]) {
        for sample in output {
            *sample += self.next_sample();
        }
    }
}

fn normalized_drive(sample: f64, drive: f64) -> f64 {
    if drive <= 0.0 {
        sample
    } else {
        let drive = drive.clamp(0.01, 20.0);
        (sample * drive).tanh() / drive.tanh()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PolyphonicSynthError {
    ZeroVoices,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PolyphonicScheduleDiagnostics {
    pub active_voices: usize,
    pub peak_voices: usize,
    pub stolen_voices: u64,
}

pub fn polyphonic_schedule_diagnostics(
    pattern: &Pattern,
    tempo: Tempo,
    range: FrameRange,
    playhead: u64,
    probability_seed: ProbabilitySeed,
    voice_count: usize,
) -> Result<PolyphonicScheduleDiagnostics, PolyphonicSynthError> {
    if voice_count == 0 {
        return Err(PolyphonicSynthError::ZeroVoices);
    }
    let mut events = Vec::new();
    pattern.events_between(tempo, range, probability_seed, &mut events);
    let active_voices = events
        .iter()
        .filter(|event| event.range().contains_frame(playhead))
        .count()
        .min(voice_count);
    let mut boundaries = Vec::with_capacity(events.len() * 2);
    for event in events {
        boundaries.push((event.range().start(), true, event.note()));
        boundaries.push((event.range().end(), false, event.note()));
    }
    boundaries.sort_by_key(|(frame, starts, note)| (*frame, *starts, *note));
    let mut slots = Vec::<(u8, bool, u64)>::new();
    let mut age = 0_u64;
    let mut peak_voices = 0;
    let mut stolen_voices = 0;
    for (_, starts, note) in boundaries {
        if !starts {
            if let Some(slot) = slots
                .iter_mut()
                .find(|(active_note, held, _)| *active_note == note && *held)
            {
                slot.1 = false;
            }
            continue;
        }
        if let Some(slot) = slots
            .iter_mut()
            .find(|(active_note, held, _)| *active_note == note && *held)
        {
            slot.2 = age;
        } else if slots.len() < voice_count {
            slots.push((note, true, age));
        } else {
            let index = slots
                .iter()
                .enumerate()
                .filter(|(_, (_, held, _))| !*held)
                .min_by_key(|(_, (_, _, slot_age))| *slot_age)
                .or_else(|| {
                    slots
                        .iter()
                        .enumerate()
                        .min_by_key(|(_, (_, _, slot_age))| *slot_age)
                })
                .map_or(0, |(index, _)| index);
            slots[index] = (note, true, age);
            stolen_voices += 1;
        }
        age = age.wrapping_add(1);
        peak_voices = peak_voices.max(slots.iter().filter(|(_, held, _)| *held).count());
    }
    Ok(PolyphonicScheduleDiagnostics {
        active_voices,
        peak_voices,
        stolen_voices,
    })
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct VoiceSlot {
    voice: BassVoice,
    note: Option<u8>,
    held: bool,
    age: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PolyphonicSynth {
    settings: BassVoiceSettings,
    sample_rate: SampleRate,
    voices: Vec<VoiceSlot>,
    next_age: u64,
    stolen_voices: u64,
}

impl PolyphonicSynth {
    pub fn new(
        settings: BassVoiceSettings,
        sample_rate: SampleRate,
        voice_count: usize,
    ) -> Result<Self, PolyphonicSynthError> {
        if voice_count == 0 {
            return Err(PolyphonicSynthError::ZeroVoices);
        }
        Ok(Self {
            settings,
            sample_rate,
            voices: (0..voice_count)
                .map(|_| VoiceSlot {
                    voice: BassVoice::new(settings, sample_rate),
                    note: None,
                    held: false,
                    age: 0,
                })
                .collect(),
            next_age: 0,
            stolen_voices: 0,
        })
    }

    pub fn note_on(&mut self, note: u8, velocity: f64) {
        if let Some(slot) = self
            .voices
            .iter_mut()
            .find(|slot| slot.note == Some(note) && slot.held)
        {
            slot.voice.note_on(note, velocity);
            slot.age = self.next_age;
            self.next_age = self.next_age.wrapping_add(1);
            return;
        }
        let index = self
            .voices
            .iter()
            .position(|slot| slot.note.is_none() || slot.voice.is_idle())
            .or_else(|| {
                self.voices
                    .iter()
                    .enumerate()
                    .filter(|(_, slot)| !slot.held)
                    .min_by_key(|(_, slot)| slot.age)
                    .map(|(index, _)| index)
            })
            .unwrap_or_else(|| {
                self.voices
                    .iter()
                    .enumerate()
                    .min_by_key(|(_, slot)| slot.age)
                    .map_or(0, |(index, _)| index)
            });
        if self.voices[index].note.is_some() && !self.voices[index].voice.is_idle() {
            self.stolen_voices += 1;
            self.voices[index].voice = BassVoice::new(self.settings, self.sample_rate);
        }
        let slot = &mut self.voices[index];
        slot.note = Some(note);
        slot.held = true;
        slot.age = self.next_age;
        slot.voice.note_on(note, velocity);
        self.next_age = self.next_age.wrapping_add(1);
    }

    pub fn note_off(&mut self, note: u8) {
        for slot in &mut self.voices {
            if slot.note == Some(note) && slot.held {
                slot.held = false;
                slot.voice.note_off();
            }
        }
    }

    pub fn set_waveform(&mut self, waveform: Waveform) {
        for slot in &mut self.voices {
            slot.voice.set_waveform(waveform);
        }
    }

    pub fn next_sample(&mut self) -> f64 {
        self.next_sample_with_parameters(
            0.0,
            self.settings.cutoff_hz,
            self.settings.resonance,
            self.settings.filter_envelope_octaves,
            self.settings.drive,
            self.settings.level,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn next_sample_with_parameters(
        &mut self,
        filter_octaves: f64,
        cutoff_hz: f64,
        resonance: f64,
        filter_envelope_octaves: f64,
        drive: f64,
        level: f64,
    ) -> f64 {
        let mut mixed = 0.0;
        for slot in &mut self.voices {
            mixed += slot.voice.next_sample_with_parameters(
                filter_octaves,
                cutoff_hz,
                resonance,
                filter_envelope_octaves,
                drive,
                level,
            );
            if !slot.held && slot.voice.is_idle() {
                slot.note = None;
            }
        }
        mixed / (self.voices.len() as f64).sqrt()
    }

    #[must_use]
    pub fn active_notes(&self) -> Vec<u8> {
        self.voices.iter().filter_map(|slot| slot.note).collect()
    }

    #[must_use]
    pub const fn stolen_voices(&self) -> u64 {
        self.stolen_voices
    }
}

pub fn render_polyphonic_pattern(
    pattern: &Pattern,
    tempo: Tempo,
    range: FrameRange,
    probability_seed: ProbabilitySeed,
    render_settings: RenderSettings,
    settings: BassVoiceSettings,
    voice_count: usize,
) -> Result<AudioBlock, PolyphonicSynthError> {
    let frames = (range.end() - range.start()).min(u64::from(u32::MAX)) as u32;
    let mut block = AudioBlock::silent(render_settings.channels(), frames);
    let history = FrameRange::new(0, range.end()).expect("polyphonic history is ordered");
    let mut events = Vec::new();
    pattern.events_between(tempo, history, probability_seed, &mut events);
    events.sort_by_key(|event| {
        (
            event.range().start(),
            event.track().raw(),
            event.step().raw(),
        )
    });
    let mut note_offs = events
        .iter()
        .map(|event| (event.range().end(), event.note()))
        .collect::<Vec<_>>();
    note_offs.sort_unstable();
    let mut synth = PolyphonicSynth::new(settings, tempo.sample_rate(), voice_count)?;
    let mut start_index = 0;
    let mut end_index = 0;
    let channels = usize::from(render_settings.channels());

    for absolute_frame in 0..range.end() {
        while note_offs
            .get(end_index)
            .is_some_and(|(frame, _)| *frame == absolute_frame)
        {
            synth.note_off(note_offs[end_index].1);
            end_index += 1;
        }
        while events
            .get(start_index)
            .is_some_and(|event| event.range().start() == absolute_frame)
        {
            let event = &events[start_index];
            synth.note_on(event.note(), event.velocity());
            start_index += 1;
        }
        let sample = synth.next_sample();
        if absolute_frame >= range.start() {
            let relative = (absolute_frame - range.start()) as usize;
            let offset = relative * channels;
            block.samples_mut()[offset..offset + channels].fill(sample);
        }
    }
    Ok(block)
}

#[allow(clippy::too_many_arguments)]
pub fn render_polyphonic_pattern_with_automation(
    pattern: &Pattern,
    tempo: Tempo,
    range: FrameRange,
    probability_seed: ProbabilitySeed,
    render_settings: RenderSettings,
    settings: BassVoiceSettings,
    voice_count: usize,
    lanes: &[AutomationLane],
) -> Result<AudioBlock, PolyphonicSynthError> {
    let frames = (range.end() - range.start()).min(u64::from(u32::MAX)) as u32;
    let mut block = AudioBlock::silent(render_settings.channels(), frames);
    let history = FrameRange::new(0, range.end()).expect("polyphonic history is ordered");
    let mut events = Vec::new();
    pattern.events_between(tempo, history, probability_seed, &mut events);
    events.sort_by_key(|event| {
        (
            event.range().start(),
            event.track().raw(),
            event.step().raw(),
        )
    });
    let mut note_offs = events
        .iter()
        .map(|event| {
            (
                event.range().end(),
                automated_note(event.note(), lanes, event.range().start()),
            )
        })
        .collect::<Vec<_>>();
    note_offs.sort_unstable();
    let mut synth = PolyphonicSynth::new(settings, tempo.sample_rate(), voice_count)?;
    let mut start_index = 0;
    let mut end_index = 0;
    let channels = usize::from(render_settings.channels());
    for absolute_frame in 0..range.end() {
        while note_offs
            .get(end_index)
            .is_some_and(|(frame, _)| *frame == absolute_frame)
        {
            synth.note_off(note_offs[end_index].1);
            end_index += 1;
        }
        while events
            .get(start_index)
            .is_some_and(|event| event.range().start() == absolute_frame)
        {
            let event = &events[start_index];
            synth.note_on(
                automated_note(event.note(), lanes, absolute_frame),
                event.velocity(),
            );
            start_index += 1;
        }
        if let Some(waveform) = automated_waveform(lanes, absolute_frame) {
            synth.set_waveform(waveform);
        }
        let mut sample = synth.next_sample_with_parameters(
            automated_value(lanes, AutomationTarget::Modulation, absolute_frame, 0.0),
            automated_value(
                lanes,
                AutomationTarget::Cutoff,
                absolute_frame,
                settings.cutoff_hz,
            ),
            automated_value(
                lanes,
                AutomationTarget::Resonance,
                absolute_frame,
                settings.resonance,
            ),
            automated_value(
                lanes,
                AutomationTarget::FilterEnvelope,
                absolute_frame,
                settings.filter_envelope_octaves,
            ),
            automated_value(
                lanes,
                AutomationTarget::Drive,
                absolute_frame,
                settings.drive,
            ),
            automated_value(
                lanes,
                AutomationTarget::Level,
                absolute_frame,
                settings.level,
            ),
        );
        if discrete_automation_value(lanes, AutomationTarget::Mute, absolute_frame)
            .is_some_and(|value| value != 0)
        {
            sample = 0.0;
        }
        if absolute_frame >= range.start() {
            let relative = (absolute_frame - range.start()) as usize;
            let offset = relative * channels;
            block.samples_mut()[offset..offset + channels].fill(sample);
        }
    }
    Ok(block)
}

#[allow(clippy::too_many_arguments)]
pub fn render_polyphonic_pattern_chunk(
    pattern: &Pattern,
    tempo: Tempo,
    range: FrameRange,
    probability_seed: ProbabilitySeed,
    render_settings: RenderSettings,
    settings: BassVoiceSettings,
    voice_count: usize,
) -> Result<AudioBlock, PolyphonicSynthError> {
    render_polyphonic_pattern(
        pattern,
        tempo,
        range,
        probability_seed,
        render_settings,
        settings,
        voice_count,
    )
}

#[must_use]
pub fn midi_note_hz(note: u8) -> f64 {
    440.0 * 2.0_f64.powf((f64::from(note) - 69.0) / 12.0)
}

pub fn render_pattern_bass(
    pattern: &Pattern,
    tempo: Tempo,
    range: FrameRange,
    probability_seed: ProbabilitySeed,
    render_settings: RenderSettings,
    voice: BassVoiceSettings,
) -> AudioBlock {
    let frames = range
        .end()
        .saturating_sub(range.start())
        .min(u64::from(u32::MAX)) as u32;
    let mut block = AudioBlock::silent(render_settings.channels(), frames);
    let mut events = Vec::new();
    pattern.events_between(tempo, range, probability_seed, &mut events);
    for event in events {
        let start = event.range().start().saturating_sub(range.start()) as u32;
        let event_frames = event.range().end().saturating_sub(event.range().start());
        render_bass_event(
            &mut block,
            start,
            event_frames,
            tempo.sample_rate(),
            midi_note_hz(event.note()),
            event.velocity(),
            voice,
        );
    }
    block
}

pub fn render_monophonic_pattern_bass(
    pattern: &Pattern,
    tempo: Tempo,
    range: FrameRange,
    probability_seed: ProbabilitySeed,
    render_settings: RenderSettings,
    settings: BassVoiceSettings,
) -> AudioBlock {
    render_monophonic_pattern_bass_inner(
        pattern,
        tempo,
        range,
        probability_seed,
        render_settings,
        settings,
        None,
    )
}

/// Render a monophonic synth with sample-accurate continuous automation.
/// Rendering prerolls from frame zero so arbitrary output ranges remain
/// sample-identical to a complete render.
#[allow(clippy::too_many_arguments)]
pub fn render_monophonic_pattern_bass_with_automation(
    pattern: &Pattern,
    ducking_control: Option<(&Pattern, meldritch_core::TrackId)>,
    tempo: Tempo,
    range: FrameRange,
    probability_seed: ProbabilitySeed,
    render_settings: RenderSettings,
    settings: BassVoiceSettings,
    lanes: &[AutomationLane],
) -> AudioBlock {
    let frames = (range.end() - range.start()).min(u64::from(u32::MAX)) as u32;
    let mut block = AudioBlock::silent(render_settings.channels(), frames);
    let history = FrameRange::new(0, range.end()).expect("automation preroll range is ordered");
    let mut events = Vec::new();
    pattern.events_between(tempo, history, probability_seed, &mut events);
    events.sort_by_key(|event| {
        (
            event.range().start(),
            event.track().raw(),
            event.step().raw(),
        )
    });
    let mut duck_triggers = Vec::new();
    if let Some((control, track)) = ducking_control {
        let mut control_events = Vec::new();
        control.events_between(tempo, history, probability_seed, &mut control_events);
        duck_triggers.extend(
            control_events
                .into_iter()
                .filter(|event| event.track() == track)
                .map(|event| event.range().start()),
        );
        duck_triggers.sort_unstable();
    }
    let mut voice = BassVoice::new(settings, tempo.sample_rate());
    let mut event_index = 0;
    let mut active_end = None;
    let mut duck_index = 0;
    let mut latest_duck = None;
    let duck_release =
        seconds_to_frames(settings.ducking_release_seconds, tempo.sample_rate()).max(1) as f64;
    let channels = usize::from(render_settings.channels());

    for absolute_frame in 0..range.end() {
        let starts_here = events
            .get(event_index)
            .is_some_and(|event| event.range().start() == absolute_frame);
        if active_end == Some(absolute_frame) && !starts_here {
            voice.note_off();
            active_end = None;
        }
        while let Some(event) = events.get(event_index) {
            if event.range().start() != absolute_frame {
                break;
            }
            let note = automated_note(event.note(), lanes, absolute_frame);
            if active_end.is_some_and(|end| end >= absolute_frame) {
                voice.legato_note_on(note, event.velocity());
            } else {
                voice.note_on(note, event.velocity());
            }
            active_end = Some(event.range().end());
            event_index += 1;
        }
        while duck_triggers
            .get(duck_index)
            .is_some_and(|trigger| *trigger <= absolute_frame)
        {
            latest_duck = Some(duck_triggers[duck_index]);
            duck_index += 1;
        }
        if let Some(waveform) = automated_waveform(lanes, absolute_frame) {
            voice.set_waveform(waveform);
        }
        let cutoff = automated_value(
            lanes,
            AutomationTarget::Cutoff,
            absolute_frame,
            settings.cutoff_hz,
        );
        let resonance = automated_value(
            lanes,
            AutomationTarget::Resonance,
            absolute_frame,
            settings.resonance,
        );
        let filter_envelope = automated_value(
            lanes,
            AutomationTarget::FilterEnvelope,
            absolute_frame,
            settings.filter_envelope_octaves,
        );
        let drive = automated_value(
            lanes,
            AutomationTarget::Drive,
            absolute_frame,
            settings.drive,
        );
        let level = automated_value(
            lanes,
            AutomationTarget::Level,
            absolute_frame,
            settings.level,
        );
        let modulation = automated_value(lanes, AutomationTarget::Modulation, absolute_frame, 0.0);
        let mut sample = voice.next_sample_with_parameters(
            modulation,
            cutoff,
            resonance,
            filter_envelope,
            drive,
            level,
        );
        if let Some(trigger) = latest_duck {
            let amount = automated_value(
                lanes,
                AutomationTarget::Ducking,
                absolute_frame,
                settings.ducking_amount,
            )
            .clamp(0.0, 1.0);
            let elapsed = (absolute_frame - trigger) as f64;
            sample *= 1.0 - amount * (-elapsed / duck_release).exp();
        }
        if discrete_automation_value(lanes, AutomationTarget::Mute, absolute_frame)
            .is_some_and(|value| value != 0)
        {
            sample = 0.0;
        }
        if absolute_frame >= range.start() {
            let relative = (absolute_frame - range.start()) as usize;
            let offset = relative * channels;
            block.samples_mut()[offset..offset + channels].fill(sample);
        }
    }
    block
}

fn automated_value(
    lanes: &[AutomationLane],
    target: AutomationTarget,
    frame: u64,
    default: f64,
) -> f64 {
    lanes
        .iter()
        .find(|lane| lane.target() == target)
        .and_then(|lane| match lane.value_at(frame) {
            AutomationValue::Continuous(value) => Some(value),
            AutomationValue::Discrete(_) => None,
        })
        .unwrap_or(default)
}

#[must_use]
pub fn discrete_automation_value(
    lanes: &[AutomationLane],
    target: AutomationTarget,
    frame: u64,
) -> Option<i64> {
    lanes
        .iter()
        .find(|lane| lane.target() == target)
        .and_then(|lane| match lane.value_at(frame) {
            AutomationValue::Discrete(value) => Some(value),
            AutomationValue::Continuous(_) => None,
        })
}

fn automated_note(note: u8, lanes: &[AutomationLane], frame: u64) -> u8 {
    let transpose = discrete_automation_value(lanes, AutomationTarget::Voicing, frame).unwrap_or(0);
    (i64::from(note) + transpose).clamp(0, 127) as u8
}

fn automated_waveform(lanes: &[AutomationLane], frame: u64) -> Option<Waveform> {
    discrete_automation_value(lanes, AutomationTarget::Waveform, frame).map(|value| match value {
        0 => Waveform::Sine,
        1 => Waveform::Triangle,
        2 => Waveform::Saw,
        3 => Waveform::Pulse,
        _ => Waveform::SyncFold,
    })
}

#[allow(clippy::too_many_arguments)]
pub fn render_monophonic_pattern_bass_with_filter_control(
    pattern: &Pattern,
    control_pattern: &Pattern,
    control_track: meldritch_core::TrackId,
    tempo: Tempo,
    range: FrameRange,
    probability_seed: ProbabilitySeed,
    render_settings: RenderSettings,
    settings: BassVoiceSettings,
) -> AudioBlock {
    render_monophonic_pattern_bass_inner(
        pattern,
        tempo,
        range,
        probability_seed,
        render_settings,
        settings,
        Some((control_pattern, control_track)),
    )
}

#[allow(clippy::too_many_arguments)]
fn render_monophonic_pattern_bass_inner(
    pattern: &Pattern,
    tempo: Tempo,
    range: FrameRange,
    probability_seed: ProbabilitySeed,
    render_settings: RenderSettings,
    settings: BassVoiceSettings,
    filter_control: Option<(&Pattern, meldritch_core::TrackId)>,
) -> AudioBlock {
    let frames = range
        .end()
        .saturating_sub(range.start())
        .min(u64::from(u32::MAX)) as u32;
    let mut block = AudioBlock::silent(render_settings.channels(), frames);
    let mut events = Vec::new();
    pattern.events_between(tempo, range, probability_seed, &mut events);
    events.retain(|event| event.range().start() >= range.start());
    events.sort_by_key(|event| {
        (
            event.range().start(),
            event.track().raw(),
            event.step().raw(),
        )
    });
    let mut voice = BassVoice::new(settings, tempo.sample_rate());
    let mut event_index = 0;
    let mut active_end = None;
    let channels = usize::from(render_settings.channels());
    let mut control_triggers = Vec::new();
    if let Some((control_pattern, control_track)) = filter_control {
        let history = FrameRange::new(0, range.end()).expect("filter history is ordered");
        let mut control_events = Vec::new();
        control_pattern.events_between(tempo, history, probability_seed, &mut control_events);
        control_triggers.extend(
            control_events
                .into_iter()
                .filter(|event| event.track() == control_track)
                .map(|event| event.range().start()),
        );
        control_triggers.sort_unstable();
    }
    let mut control_index = 0;
    let mut latest_control = None;
    let mut latest_accent = None;
    let control_release =
        seconds_to_frames(settings.hat_filter_release_seconds, tempo.sample_rate()).max(1) as f64;
    let accent_release =
        seconds_to_frames(settings.accent_release_seconds, tempo.sample_rate()).max(1) as f64;

    for relative_frame in 0..frames {
        let absolute_frame = range.start() + u64::from(relative_frame);
        let starts_here = events
            .get(event_index)
            .is_some_and(|event| event.range().start() == absolute_frame);
        if active_end == Some(absolute_frame) && !starts_here {
            voice.note_off();
            active_end = None;
        }
        while let Some(event) = events.get(event_index) {
            if event.range().start() != absolute_frame {
                break;
            }
            let accented = event.tags().contains(&meldritch_core::EventTag::Accent);
            let velocity = if accented {
                (event.velocity() * settings.accent_velocity_gain.max(0.0)).min(1.0)
            } else {
                event.velocity()
            };
            if accented {
                latest_accent = Some(absolute_frame);
            }
            if active_end.is_some_and(|end| end >= absolute_frame) {
                voice.legato_note_on(event.note(), velocity);
            } else {
                voice.note_on(event.note(), velocity);
            }
            active_end = Some(event.range().end());
            event_index += 1;
        }
        while control_triggers
            .get(control_index)
            .is_some_and(|trigger| *trigger <= absolute_frame)
        {
            latest_control = Some(control_triggers[control_index]);
            control_index += 1;
        }
        let control_offset = latest_control.map_or(0.0, |trigger| {
            let elapsed = (absolute_frame - trigger) as f64;
            settings.hat_filter_octaves.max(0.0) * (-elapsed / control_release).exp()
        });
        let accent_offset = latest_accent.map_or(0.0, |trigger| {
            let elapsed = (absolute_frame - trigger) as f64;
            settings.accent_filter_octaves.max(0.0) * (-elapsed / accent_release).exp()
        });
        let filter_offset = control_offset + accent_offset;
        let sample = voice.next_sample_with_filter_offset(filter_offset);
        let offset = relative_frame as usize * channels;
        block.samples_mut()[offset..offset + channels].fill(sample);
    }
    block
}

pub fn render_monophonic_pattern_bass_chunk(
    pattern: &Pattern,
    tempo: Tempo,
    range: FrameRange,
    probability_seed: ProbabilitySeed,
    render_settings: RenderSettings,
    settings: BassVoiceSettings,
) -> AudioBlock {
    let preroll_range =
        FrameRange::new(0, range.end()).expect("chunk end always forms an ordered preroll range");
    let preroll = render_monophonic_pattern_bass(
        pattern,
        tempo,
        preroll_range,
        probability_seed,
        render_settings,
        settings,
    );
    let frames = range
        .end()
        .saturating_sub(range.start())
        .min(u64::from(u32::MAX)) as u32;
    let channels = usize::from(render_settings.channels());
    let start = range.start() as usize * channels;
    let end = start + frames as usize * channels;
    let mut chunk = AudioBlock::silent(render_settings.channels(), frames);
    chunk
        .samples_mut()
        .copy_from_slice(&preroll.samples()[start..end]);
    chunk
}

#[allow(clippy::too_many_arguments)]
pub fn render_monophonic_pattern_bass_chunk_with_filter_control(
    pattern: &Pattern,
    control_pattern: &Pattern,
    control_track: meldritch_core::TrackId,
    tempo: Tempo,
    range: FrameRange,
    probability_seed: ProbabilitySeed,
    render_settings: RenderSettings,
    settings: BassVoiceSettings,
) -> AudioBlock {
    let preroll_range =
        FrameRange::new(0, range.end()).expect("chunk end always forms an ordered preroll range");
    let preroll = render_monophonic_pattern_bass_with_filter_control(
        pattern,
        control_pattern,
        control_track,
        tempo,
        preroll_range,
        probability_seed,
        render_settings,
        settings,
    );
    let frames = (range.end() - range.start()).min(u64::from(u32::MAX)) as u32;
    let channels = usize::from(render_settings.channels());
    let start = range.start() as usize * channels;
    let end = start + frames as usize * channels;
    let mut chunk = AudioBlock::silent(render_settings.channels(), frames);
    chunk
        .samples_mut()
        .copy_from_slice(&preroll.samples()[start..end]);
    chunk
}

#[allow(clippy::too_many_arguments)]
pub fn apply_pattern_ducking(
    block: &mut AudioBlock,
    control_pattern: &Pattern,
    control_track: meldritch_core::TrackId,
    tempo: Tempo,
    range: FrameRange,
    probability_seed: ProbabilitySeed,
    amount: f64,
    release_seconds: f64,
) {
    let amount = amount.clamp(0.0, 1.0);
    if amount == 0.0 || block.frames() == 0 {
        return;
    }
    let history = FrameRange::new(0, range.end()).expect("ducking history is ordered");
    let mut events = Vec::new();
    control_pattern.events_between(tempo, history, probability_seed, &mut events);
    let mut triggers = events
        .into_iter()
        .filter(|event| event.track() == control_track)
        .map(|event| event.range().start())
        .collect::<Vec<_>>();
    triggers.sort_unstable();
    let release_frames = seconds_to_frames(release_seconds, tempo.sample_rate()).max(1) as f64;
    let channels = usize::from(block.channels());
    let mut trigger_index = 0;
    let mut latest_trigger = None;
    for relative_frame in 0..block.frames() {
        let absolute_frame = range.start() + u64::from(relative_frame);
        while triggers
            .get(trigger_index)
            .is_some_and(|trigger| *trigger <= absolute_frame)
        {
            latest_trigger = Some(triggers[trigger_index]);
            trigger_index += 1;
        }
        let gain = latest_trigger.map_or(1.0, |trigger| {
            let elapsed = (absolute_frame - trigger) as f64;
            1.0 - amount * (-elapsed / release_frames).exp()
        });
        let offset = relative_frame as usize * channels;
        for sample in &mut block.samples_mut()[offset..offset + channels] {
            *sample *= gain;
        }
    }
}

#[must_use]
pub fn synthesize_bass_sample(
    note: u8,
    sample_rate: SampleRate,
    frames: u32,
    voice: BassVoiceSettings,
) -> SampleBuffer {
    let mut block = AudioBlock::silent(1, frames);
    let release_frames = (voice.release_seconds.max(0.0) * f64::from(sample_rate)).round() as u64;
    let gate_frames = u64::from(frames).saturating_sub(release_frames);
    render_bass_event(
        &mut block,
        0,
        gate_frames,
        sample_rate,
        midi_note_hz(note),
        1.0,
        voice,
    );
    SampleBuffer::new(1, sample_rate, block.samples().to_vec())
}

#[allow(clippy::too_many_arguments)]
fn render_bass_event(
    block: &mut AudioBlock,
    start_frame: u32,
    gate_frames: u64,
    sample_rate: SampleRate,
    frequency: f64,
    velocity: f64,
    voice: BassVoiceSettings,
) {
    let rate = f64::from(sample_rate);
    let release = (voice.release_seconds.max(0.0) * rate).round() as u64;
    let total = gate_frames.saturating_add(release);
    let available = u64::from(block.frames().saturating_sub(start_frame));
    let frame_count = total.min(available);
    let channels = usize::from(block.channels());
    let mut bass_voice = BassVoice::new(voice, sample_rate);
    bass_voice.note_on_frequency(frequency, velocity);

    for frame in 0..frame_count {
        if frame == gate_frames {
            bass_voice.note_off();
        }
        let sample = bass_voice.next_sample();
        let output_frame = (start_frame as usize + frame as usize) * channels;
        for channel in 0..channels {
            block.samples_mut()[output_frame + channel] += sample;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use meldritch_core::{
        AutomationInterpolation, AutomationPoint, PatternId, Step, StepIndex, TrackId,
    };

    #[test]
    fn midi_note_frequency_uses_concert_a() {
        assert!((midi_note_hz(69) - 440.0).abs() < f64::EPSILON);
        assert!((midi_note_hz(57) - 220.0).abs() < f64::EPSILON);
    }

    #[test]
    fn automated_bass_is_audible_and_chunk_identical() {
        let tempo = Tempo::new(120.0, 48_000).unwrap();
        let mut bass = Pattern::new(PatternId::new(1), 4, 4).unwrap();
        bass.set_step(
            TrackId::new(1),
            StepIndex::new(0),
            Step::new(36).with_gate(1.0),
        )
        .unwrap();
        let lanes = vec![
            AutomationLane::new(
                AutomationTarget::Cutoff,
                AutomationInterpolation::Linear,
                vec![
                    AutomationPoint {
                        frame: 0,
                        value: AutomationValue::Continuous(80.0),
                    },
                    AutomationPoint {
                        frame: 12_000,
                        value: AutomationValue::Continuous(3_000.0),
                    },
                ],
            )
            .unwrap(),
            AutomationLane::new(
                AutomationTarget::Level,
                AutomationInterpolation::Linear,
                vec![
                    AutomationPoint {
                        frame: 0,
                        value: AutomationValue::Continuous(0.1),
                    },
                    AutomationPoint {
                        frame: 12_000,
                        value: AutomationValue::Continuous(0.7),
                    },
                ],
            )
            .unwrap(),
        ];
        let settings = BassVoiceSettings::default();
        let render_settings = RenderSettings::new(1).unwrap();
        let full = render_monophonic_pattern_bass_with_automation(
            &bass,
            None,
            tempo,
            FrameRange::new(0, 12_000).unwrap(),
            ProbabilitySeed::new(1),
            render_settings,
            settings,
            &lanes,
        );
        let tail = render_monophonic_pattern_bass_with_automation(
            &bass,
            None,
            tempo,
            FrameRange::new(6_000, 12_000).unwrap(),
            ProbabilitySeed::new(1),
            render_settings,
            settings,
            &lanes,
        );

        assert!(full.peak_abs() > 0.0);
        assert_eq!(&full.samples()[6_000..], tail.samples());
        assert!(full.samples().iter().all(|sample| sample.is_finite()));
    }

    #[test]
    fn discrete_automation_changes_waveform_voicing_mute_and_scene() {
        let tempo = Tempo::new(120.0, 48_000).unwrap();
        let mut bass = Pattern::new(PatternId::new(1), 4, 4).unwrap();
        bass.set_step(TrackId::new(1), StepIndex::new(0), Step::new(36))
            .unwrap();
        let stepped = |target, values: &[(u64, i64)]| {
            AutomationLane::new(
                target,
                AutomationInterpolation::Step,
                values
                    .iter()
                    .map(|(frame, value)| AutomationPoint {
                        frame: *frame,
                        value: AutomationValue::Discrete(*value),
                    })
                    .collect(),
            )
            .unwrap()
        };
        let lanes = vec![
            stepped(AutomationTarget::Waveform, &[(0, 0), (3_000, 3)]),
            stepped(AutomationTarget::Voicing, &[(0, 12)]),
            stepped(AutomationTarget::Mute, &[(0, 0), (6_000, 1)]),
            stepped(AutomationTarget::Scene, &[(0, 7), (6_000, 8)]),
        ];
        let block = render_monophonic_pattern_bass_with_automation(
            &bass,
            None,
            tempo,
            FrameRange::new(0, 12_000).unwrap(),
            ProbabilitySeed::new(1),
            RenderSettings::new(1).unwrap(),
            BassVoiceSettings::default(),
            &lanes,
        );

        assert!(block.samples()[..6_000].iter().any(|sample| *sample != 0.0));
        assert!(block.samples()[6_000..].iter().all(|sample| *sample == 0.0));
        assert_eq!(
            discrete_automation_value(&lanes, AutomationTarget::Scene, 7_000),
            Some(8)
        );
    }

    #[test]
    fn oscillators_are_finite_and_bounded_across_waveforms() {
        for waveform in [
            Waveform::Sine,
            Waveform::Triangle,
            Waveform::Saw,
            Waveform::Pulse,
            Waveform::SyncFold,
        ] {
            let mut oscillator = Oscillator::new(waveform).with_pulse_width(0.25);
            let values = (0..48_000)
                .map(|_| oscillator.next(8_000.0, 48_000))
                .collect::<Vec<_>>();
            assert!(values.iter().all(|value| value.is_finite()));
            assert!(values.iter().all(|value| value.abs() <= 1.01));
        }
    }

    #[test]
    fn sync_fold_is_aggressive_bounded_and_parameter_sensitive() {
        let render = |ratio, modulation, fold| {
            let mut oscillator =
                Oscillator::new(Waveform::SyncFold).with_sync_fold(ratio, modulation, fold);
            (0..2_048)
                .map(|_| oscillator.next(110.0, 48_000))
                .collect::<Vec<_>>()
        };
        let base = render(3.0, 0.35, 2.5);
        let changed = render(7.0, 0.9, 5.0);
        assert!(base.iter().all(|sample| sample.is_finite()));
        assert!(base.iter().all(|sample| sample.abs() <= 1.0));
        assert_ne!(base, changed);
        assert!(
            base.windows(2)
                .any(|window| (window[1] - window[0]).abs() > 0.25)
        );
    }

    #[test]
    fn adsr_moves_through_gate_and_release_stages() {
        let mut envelope = AdsrEnvelope::new(
            AdsrSettings {
                attack_seconds: 0.002,
                decay_seconds: 0.002,
                sustain_level: 0.5,
                release_seconds: 0.002,
            },
            1_000,
        );
        envelope.note_on();
        assert_eq!(envelope.next_value(), 0.5);
        assert_eq!(envelope.next_value(), 1.0);
        assert_eq!(envelope.stage(), EnvelopeStage::Decay);
        assert_eq!(envelope.next_value(), 0.75);
        assert_eq!(envelope.next_value(), 0.5);
        assert_eq!(envelope.stage(), EnvelopeStage::Sustain);
        envelope.note_off();
        assert_eq!(envelope.next_value(), 0.25);
        assert_eq!(envelope.next_value(), 0.0);
        assert_eq!(envelope.stage(), EnvelopeStage::Idle);
    }

    #[test]
    fn state_variable_filter_modes_are_finite_and_distinct() {
        let render_mode = |mode| {
            let mut filter = StateVariableFilter::new();
            (0..512)
                .map(|frame| {
                    let input = if frame == 0 { 1.0 } else { 0.0 };
                    filter.process(input, 1_000.0, 0.7, mode, 48_000)
                })
                .collect::<Vec<_>>()
        };
        let low = render_mode(FilterMode::LowPass);
        let band = render_mode(FilterMode::BandPass);
        let high = render_mode(FilterMode::HighPass);

        assert!(
            low.iter()
                .chain(&band)
                .chain(&high)
                .all(|value| value.is_finite())
        );
        assert_ne!(low, band);
        assert_ne!(band, high);
        assert!(low.iter().any(|value| value.abs() > 0.0));
    }

    #[test]
    fn state_variable_filter_clamps_extreme_parameters() {
        let mut filter = StateVariableFilter::new();
        for _ in 0..48_000 {
            let output = filter.process(0.5, f64::INFINITY, 10.0, FilterMode::LowPass, 48_000);
            assert!(output.is_finite());
        }
    }

    #[test]
    fn stateful_voice_is_sample_identical_across_chunk_boundaries() {
        let settings = BassVoiceSettings::default();
        let mut contiguous = BassVoice::new(settings, 48_000);
        contiguous.note_on(36, 0.8);
        let mut expected = vec![0.0; 4_096];
        contiguous.render_add(&mut expected[..2_048]);
        contiguous.note_off();
        contiguous.render_add(&mut expected[2_048..]);

        let mut chunked = BassVoice::new(settings, 48_000);
        chunked.note_on(36, 0.8);
        let mut actual = Vec::new();
        for frames in [500, 700, 848] {
            let mut chunk = vec![0.0; frames];
            chunked.render_add(&mut chunk);
            actual.extend(chunk);
        }
        chunked.note_off();
        for frames in [333, 1_715] {
            let mut chunk = vec![0.0; frames];
            chunked.render_add(&mut chunk);
            actual.extend(chunk);
        }

        assert_eq!(actual, expected);
    }

    #[test]
    fn cloned_voice_resumes_from_an_exact_dsp_snapshot() {
        let mut original = BassVoice::new(BassVoiceSettings::default(), 48_000);
        original.note_on(31, 0.7);
        original.render_add(&mut [0.0; 777]);
        let mut resumed = original;

        assert_eq!(original.next_sample(), resumed.next_sample());
        assert_eq!(original.next_sample(), resumed.next_sample());
    }

    #[test]
    fn drive_changes_timbre_without_producing_non_finite_samples() {
        let render = |drive| {
            let mut voice = BassVoice::new(
                BassVoiceSettings {
                    drive,
                    ..BassVoiceSettings::default()
                },
                48_000,
            );
            voice.note_on(36, 1.0);
            (0..2_000).map(|_| voice.next_sample()).collect::<Vec<_>>()
        };
        let clean = render(0.0);
        let driven = render(8.0);

        assert_ne!(clean, driven);
        assert!(driven.iter().all(|sample| sample.is_finite()));
        assert!(driven.iter().all(|sample| sample.abs() <= 1.0));
    }

    #[test]
    fn pre_and_post_filter_drive_are_distinct_bounded_stages() {
        let render = |pre_filter_drive, drive| {
            let mut voice = BassVoice::new(
                BassVoiceSettings {
                    waveform: Waveform::SyncFold,
                    resonance: 0.82,
                    cutoff_hz: 420.0,
                    pre_filter_drive,
                    drive,
                    ..BassVoiceSettings::default()
                },
                48_000,
            );
            voice.note_on(36, 1.0);
            (0..4_000).map(|_| voice.next_sample()).collect::<Vec<_>>()
        };
        let pre = render(7.0, 0.0);
        let post = render(0.0, 7.0);
        let both = render(7.0, 7.0);
        assert_ne!(pre, post);
        assert_ne!(pre, both);
        assert_ne!(post, both);
        assert!(both.iter().all(|sample| sample.is_finite()));
        assert!(both.iter().all(|sample| sample.abs() <= 1.0));
    }

    #[test]
    fn sub_oscillator_changes_timbre() {
        let render = |sub_level| {
            let mut voice = BassVoice::new(
                BassVoiceSettings {
                    sub_level,
                    ..BassVoiceSettings::default()
                },
                48_000,
            );
            voice.note_on(36, 1.0);
            (0..1_000).map(|_| voice.next_sample()).collect::<Vec<_>>()
        };
        assert_ne!(render(0.0), render(1.0));
    }

    #[test]
    fn legato_note_glides_without_retriggering_the_envelope() {
        let settings = BassVoiceSettings {
            attack_seconds: 0.0,
            decay_seconds: 0.0,
            sustain_level: 1.0,
            glide_seconds: 0.01,
            ..BassVoiceSettings::default()
        };
        let mut voice = BassVoice::new(settings, 1_000);
        voice.note_on(36, 1.0);
        voice.next_sample();
        voice.next_sample();
        assert_eq!(voice.envelope.stage(), EnvelopeStage::Sustain);
        let start = voice.current_frequency();
        voice.legato_note_on(48, 0.8);
        assert_eq!(voice.envelope.stage(), EnvelopeStage::Sustain);
        voice.next_sample();
        assert!(voice.current_frequency() > start);
        for _ in 1..10 {
            voice.next_sample();
        }
        assert!((voice.current_frequency() - midi_note_hz(48)).abs() < f64::EPSILON);
    }

    #[test]
    fn bass_render_is_finite_stereo_and_audible() {
        let mut pattern = Pattern::new(PatternId::new(2), 16, 4).unwrap();
        pattern
            .set_step(TrackId::new(1), StepIndex::new(0), Step::new(36))
            .unwrap();
        let tempo = Tempo::new(120.0, 48_000).unwrap();
        let block = render_pattern_bass(
            &pattern,
            tempo,
            FrameRange::new(0, 12_000).unwrap(),
            ProbabilitySeed::new(1),
            RenderSettings::new(2).unwrap(),
            BassVoiceSettings::default(),
        );

        assert_eq!(block.channels(), 2);
        assert!(block.peak_abs() > 0.0);
        assert!(block.samples().iter().all(|sample| sample.is_finite()));
        assert!(
            block
                .samples()
                .chunks_exact(2)
                .all(|frame| frame[0] == frame[1])
        );
    }

    #[test]
    fn monophonic_pattern_renderer_applies_glide_between_notes() {
        let mut pattern = Pattern::new(PatternId::new(2), 4, 4).unwrap();
        pattern
            .set_step(TrackId::new(1), StepIndex::new(0), Step::new(36))
            .unwrap();
        pattern
            .set_step(TrackId::new(1), StepIndex::new(1), Step::new(48))
            .unwrap();
        let tempo = Tempo::new(120.0, 48_000).unwrap();
        let render = |glide_seconds| {
            render_monophonic_pattern_bass(
                &pattern,
                tempo,
                FrameRange::new(0, 12_000).unwrap(),
                ProbabilitySeed::new(1),
                RenderSettings::new(1).unwrap(),
                BassVoiceSettings {
                    glide_seconds,
                    ..BassVoiceSettings::default()
                },
            )
        };
        let immediate = render(0.0);
        let gliding = render(0.05);

        assert_ne!(immediate.samples(), gliding.samples());
        assert!(gliding.samples().iter().all(|sample| sample.is_finite()));
        assert!(
            gliding.samples()[6_000..]
                .iter()
                .any(|sample| *sample != 0.0)
        );
    }

    #[test]
    fn monophonic_chunks_are_identical_to_the_full_phrase() {
        let mut pattern = Pattern::new(PatternId::new(2), 4, 4).unwrap();
        for (step, note) in [(0, 36), (1, 43), (3, 31)] {
            pattern
                .set_step(TrackId::new(1), StepIndex::new(step), Step::new(note))
                .unwrap();
        }
        let tempo = Tempo::new(120.0, 48_000).unwrap();
        let settings = BassVoiceSettings::default();
        let render_settings = RenderSettings::new(2).unwrap();
        let seed = ProbabilitySeed::new(1);
        let full = render_monophonic_pattern_bass(
            &pattern,
            tempo,
            FrameRange::new(0, 24_000).unwrap(),
            seed,
            render_settings,
            settings,
        );
        let mut joined = Vec::new();
        for (start, end) in [(0, 4_096), (4_096, 11_000), (11_000, 24_000)] {
            joined.extend_from_slice(
                render_monophonic_pattern_bass_chunk(
                    &pattern,
                    tempo,
                    FrameRange::new(start, end).unwrap(),
                    seed,
                    render_settings,
                    settings,
                )
                .samples(),
            );
        }

        assert_eq!(joined, full.samples());
    }

    #[test]
    fn kick_ducking_is_deterministic_across_chunk_boundaries() {
        let mut controls = Pattern::new(PatternId::new(3), 4, 4).unwrap();
        controls
            .set_step(TrackId::new(1), StepIndex::new(0), Step::new(36))
            .unwrap();
        let tempo = Tempo::new(120.0, 48_000).unwrap();
        let seed = ProbabilitySeed::new(1);
        let mut full = AudioBlock::silent(1, 12_000);
        full.samples_mut().fill(1.0);
        apply_pattern_ducking(
            &mut full,
            &controls,
            TrackId::new(1),
            tempo,
            FrameRange::new(0, 12_000).unwrap(),
            seed,
            0.5,
            0.1,
        );
        let mut joined = Vec::new();
        for (start, end) in [(0, 4_096), (4_096, 12_000)] {
            let mut chunk = AudioBlock::silent(1, end - start);
            chunk.samples_mut().fill(1.0);
            apply_pattern_ducking(
                &mut chunk,
                &controls,
                TrackId::new(1),
                tempo,
                FrameRange::new(u64::from(start), u64::from(end)).unwrap(),
                seed,
                0.5,
                0.1,
            );
            joined.extend_from_slice(chunk.samples());
        }

        assert_eq!(full.samples()[0], 0.5);
        assert!(full.samples()[1_000] > full.samples()[0]);
        assert_eq!(joined, full.samples());
    }

    #[test]
    fn hat_events_open_the_bass_filter_deterministically() {
        let mut bass = Pattern::new(PatternId::new(2), 4, 4).unwrap();
        bass.set_step(TrackId::new(4), StepIndex::new(0), Step::new(24))
            .unwrap();
        let mut hats = Pattern::new(PatternId::new(3), 4, 4).unwrap();
        hats.set_step(TrackId::new(3), StepIndex::new(0), Step::new(42))
            .unwrap();
        let tempo = Tempo::new(120.0, 48_000).unwrap();
        let range = FrameRange::new(0, 8_000).unwrap();
        let seed = ProbabilitySeed::new(1);
        let render_settings = RenderSettings::new(1).unwrap();
        let settings = BassVoiceSettings::default();
        let plain =
            render_monophonic_pattern_bass(&bass, tempo, range, seed, render_settings, settings);
        let opened = render_monophonic_pattern_bass_with_filter_control(
            &bass,
            &hats,
            TrackId::new(3),
            tempo,
            range,
            seed,
            render_settings,
            settings,
        );

        assert_ne!(plain.samples(), opened.samples());
        assert!(opened.samples().iter().all(|sample| sample.is_finite()));
    }

    #[test]
    fn accent_tag_changes_bass_velocity_and_filter_response() {
        let render = |accented| {
            let mut pattern = Pattern::new(PatternId::new(2), 4, 4).unwrap();
            let mut step = Step::new(24).with_velocity(0.5);
            if accented {
                step = step.with_tag(meldritch_core::EventTag::Accent);
            }
            pattern
                .set_step(TrackId::new(4), StepIndex::new(0), step)
                .unwrap();
            render_monophonic_pattern_bass(
                &pattern,
                Tempo::new(120.0, 48_000).unwrap(),
                FrameRange::new(0, 4_000).unwrap(),
                ProbabilitySeed::new(1),
                RenderSettings::new(1).unwrap(),
                BassVoiceSettings::default(),
            )
        };

        assert_ne!(render(false).samples(), render(true).samples());
    }

    #[test]
    fn polyphonic_voice_bank_renders_chords_and_routes_note_off() {
        let mut synth = PolyphonicSynth::new(BassVoiceSettings::default(), 48_000, 4).unwrap();
        for note in [60, 64, 67] {
            synth.note_on(note, 0.7);
        }
        let chord = (0..2_000).map(|_| synth.next_sample()).collect::<Vec<_>>();
        assert!(chord.iter().any(|sample| *sample != 0.0));
        assert!(chord.iter().all(|sample| sample.is_finite()));
        synth.note_off(64);
        assert_eq!(synth.active_notes(), vec![60, 64, 67]);
    }

    #[test]
    fn polyphonic_voice_stealing_is_deterministic() {
        let mut synth = PolyphonicSynth::new(BassVoiceSettings::default(), 48_000, 2).unwrap();
        synth.note_on(60, 1.0);
        synth.note_on(64, 1.0);
        synth.note_on(67, 1.0);

        assert_eq!(synth.active_notes(), vec![67, 64]);
        assert_eq!(synth.stolen_voices(), 1);
        assert_eq!(
            PolyphonicSynth::new(BassVoiceSettings::default(), 48_000, 0),
            Err(PolyphonicSynthError::ZeroVoices)
        );
    }

    #[test]
    fn polyphonic_snapshot_resumes_sample_identically() {
        let mut synth = PolyphonicSynth::new(BassVoiceSettings::default(), 48_000, 4).unwrap();
        for note in [48, 55, 60] {
            synth.note_on(note, 0.8);
        }
        for _ in 0..777 {
            synth.next_sample();
        }
        let mut resumed = synth.clone();
        for _ in 0..1_000 {
            assert_eq!(synth.next_sample(), resumed.next_sample());
        }
    }

    #[test]
    fn polyphonic_pattern_renders_independent_note_lanes_as_a_chord() {
        let mut pattern = Pattern::new(PatternId::new(4), 4, 4).unwrap();
        for (track, note) in [(10, 60), (11, 63), (12, 67)] {
            pattern
                .set_step(TrackId::new(track), StepIndex::new(0), Step::new(note))
                .unwrap();
        }
        let block = render_polyphonic_pattern(
            &pattern,
            Tempo::new(120.0, 48_000).unwrap(),
            FrameRange::new(0, 8_000).unwrap(),
            ProbabilitySeed::new(1),
            RenderSettings::new(2).unwrap(),
            BassVoiceSettings::default(),
            8,
        )
        .unwrap();

        assert!(block.peak_abs() > 0.0);
        assert!(block.samples().iter().all(|sample| sample.is_finite()));
        assert!(
            block
                .samples()
                .chunks_exact(2)
                .all(|frame| frame[0] == frame[1])
        );
    }

    #[test]
    fn polyphonic_pattern_chunks_match_the_full_render() {
        let mut pattern = Pattern::new(PatternId::new(4), 8, 4).unwrap();
        for (track, note, step) in [(10, 60, 0), (11, 64, 0), (12, 67, 0), (10, 62, 4)] {
            pattern
                .set_step(TrackId::new(track), StepIndex::new(step), Step::new(note))
                .unwrap();
        }
        let tempo = Tempo::new(120.0, 48_000).unwrap();
        let seed = ProbabilitySeed::new(1);
        let render_settings = RenderSettings::new(1).unwrap();
        let settings = BassVoiceSettings::default();
        let full = render_polyphonic_pattern(
            &pattern,
            tempo,
            FrameRange::new(0, 48_000).unwrap(),
            seed,
            render_settings,
            settings,
            4,
        )
        .unwrap();
        let mut joined = Vec::new();
        for (start, end) in [(0, 4_096), (4_096, 19_000), (19_000, 48_000)] {
            joined.extend_from_slice(
                render_polyphonic_pattern_chunk(
                    &pattern,
                    tempo,
                    FrameRange::new(start, end).unwrap(),
                    seed,
                    render_settings,
                    settings,
                    4,
                )
                .unwrap()
                .samples(),
            );
        }

        assert_eq!(joined, full.samples());
    }

    #[test]
    fn polyphonic_schedule_reports_concurrency_and_stealing() {
        let mut pattern = Pattern::new(PatternId::new(4), 4, 4).unwrap();
        for (track, note) in [(10, 60), (11, 64), (12, 67)] {
            pattern
                .set_step(TrackId::new(track), StepIndex::new(0), Step::new(note))
                .unwrap();
        }
        let diagnostics = polyphonic_schedule_diagnostics(
            &pattern,
            Tempo::new(120.0, 48_000).unwrap(),
            FrameRange::new(0, 6_000).unwrap(),
            1_000,
            ProbabilitySeed::new(1),
            2,
        )
        .unwrap();

        assert_eq!(diagnostics.active_voices, 2);
        assert_eq!(diagnostics.peak_voices, 2);
        assert_eq!(diagnostics.stolen_voices, 1);
    }

    #[test]
    fn synthesized_bass_sample_contains_release_and_is_finite() {
        let sample = synthesize_bass_sample(36, 48_000, 8_000, BassVoiceSettings::default());

        assert_eq!(sample.channels(), 1);
        assert_eq!(sample.frames(), 8_000);
        assert!(sample.samples().iter().any(|value| *value != 0.0));
        assert!(sample.samples().iter().all(|value| value.is_finite()));
    }
}
