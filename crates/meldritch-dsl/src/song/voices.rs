//! Bounded composition relationships over existing palette definitions.
use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PatternSignal {
    Pitch,
    Gate,
}

/// One mono patch instance with independently authored input bindings.
/// Unbound pitch holds initial_pitch; unbound gate is low. Velocity is not
/// implicitly carried by either cable: this subset has an explicit output level.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PatternVoiceDefinition {
    pub id: String,
    pub synth: String,
    pub length_ticks: u64,
    pub looped: bool,
    pub initial_pitch: u8,
    pub level: f64,
    pub pitch_pattern: Option<String>,
    pub gate_pattern: Option<String>,
}

impl ValidatedSong {
    #[must_use]
    pub fn pattern_voices(&self) -> &BTreeMap<String, PatternVoiceDefinition> {
        &self.pattern_voices
    }

    pub fn validate_pattern_voice(&self, voice: &PatternVoiceDefinition) -> Result<(), String> {
        if voice.id.is_empty()
            || voice.id.len() > 64
            || !voice
                .id
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_'))
        {
            return Err("invalid pattern voice identity".into());
        }
        let synth = self.synths.get(&voice.synth).ok_or("unknown voice synth")?;
        if synth.polyphony != 1 {
            return Err("independent pattern inputs currently require a mono patch".into());
        }
        for (id, signal) in [("pitch", SignalType::Pitch), ("gate", SignalType::Gate)] {
            if !synth
                .inputs
                .iter()
                .any(|input| input.id == id && input.signal == signal)
            {
                return Err(format!("voice requires typed input.{id}"));
            }
        }
        let length = self.authoring_frame(voice.length_ticks)?;
        if length == 0
            || voice.initial_pitch > 127
            || !voice.level.is_finite()
            || !(0.0..=1.0).contains(&voice.level)
        {
            return Err("invalid voice duration, initial pitch or output level".into());
        }
        for (signal, binding) in [
            (PatternSignal::Pitch, &voice.pitch_pattern),
            (PatternSignal::Gate, &voice.gate_pattern),
        ] {
            let Some(id) = binding else { continue };
            let pattern = self
                .note_patterns
                .get(id)
                .ok_or("unknown bound note pattern")?;
            if pattern.length_frames != length || pattern.looped != voice.looped {
                return Err("bound pattern must share the voice length and loop policy".into());
            }
            let mut prior: Option<&NoteEventDefinition> = None;
            for event in &pattern.events {
                if event.sample_slot.is_some()
                    || event.sample_slice.is_some()
                    || event.root_note.is_some()
                    || event.pitch_semitones != 0.0
                {
                    return Err("independent voice bindings do not translate sample events".into());
                }
                if prior.is_some_and(|previous| match signal {
                    PatternSignal::Pitch => previous.start_frame == event.start_frame,
                    PatternSignal::Gate => {
                        previous.start_frame + previous.duration_frames > event.start_frame
                    }
                }) {
                    return Err("ambiguous simultaneous pitch or overlapping gate events require an explicit processor".into());
                }
                prior = Some(event);
            }
        }
        Ok(())
    }

    pub fn create_pattern_voice(&self, voice: PatternVoiceDefinition) -> Result<Self, String> {
        if self.pattern_voices.contains_key(&voice.id)
            || self
                .performance
                .tracks
                .iter()
                .any(|track| track.id == voice.id)
            || self
                .performance
                .lanes
                .iter()
                .any(|lane| lane.id == voice.id)
        {
            return Err("duplicate pattern voice identity".into());
        }
        if self.pattern_voices.len() >= 32 {
            return Err("pattern voice limit is 32".into());
        }
        self.validate_pattern_voice(&voice)?;
        let mut next = self.clone();
        next.pattern_voices.insert(voice.id.clone(), voice);
        next.refresh_authoring_fingerprint();
        Ok(next)
    }

    pub fn bind_pattern_voice(
        &self,
        voice: &str,
        signal: PatternSignal,
        pattern: Option<String>,
    ) -> Result<Self, String> {
        let mut next = self.clone();
        let definition = next
            .pattern_voices
            .get_mut(voice)
            .ok_or("unknown pattern voice")?;
        match signal {
            PatternSignal::Pitch => definition.pitch_pattern = pattern,
            PatternSignal::Gate => definition.gate_pattern = pattern,
        }
        next.validate_pattern_voice(&next.pattern_voices[voice])?;
        next.refresh_authoring_fingerprint();
        Ok(next)
    }

    pub(super) fn fingerprint_pattern_voices(&mut self) {
        if self.pattern_voices.is_empty() {
            return;
        }
        let mut fingerprint = Fnv64::new();
        fingerprint.string("meldritch-independent-pattern-voices-v1");
        fingerprint.u64(self.fingerprint);
        for voice in self.pattern_voices.values() {
            fingerprint.string(&voice.id);
            fingerprint.string(&voice.synth);
            fingerprint.u64(voice.length_ticks);
            fingerprint.u64(u64::from(voice.looped));
            fingerprint.u64(u64::from(voice.initial_pitch));
            fingerprint.u64(voice.level.to_bits());
            fingerprint.optional_string(voice.pitch_pattern.as_deref());
            fingerprint.optional_string(voice.gate_pattern.as_deref());
        }
        self.fingerprint = fingerprint.finish();
    }
}
