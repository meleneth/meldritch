//! Named membership over one project-owned palette and instance registry.
use super::*;

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CompositionDefinition {
    pub id: String,
    pub voices: Vec<String>,
}

impl ValidatedSong {
    pub fn compositions(&self) -> &BTreeMap<String, CompositionDefinition> {
        &self.compositions
    }
    pub fn selected_composition(&self) -> Option<&str> {
        self.selected_composition.as_deref()
    }
    pub fn is_pattern_voice_active(&self, id: &str) -> bool {
        match &self.selected_composition {
            None => self.pattern_voices.contains_key(id),
            Some(selected) => self.compositions[selected]
                .voices
                .iter()
                .any(|voice| voice == id),
        }
    }
    pub fn active_pattern_voices(&self) -> impl Iterator<Item = &PatternVoiceDefinition> {
        self.pattern_voices
            .values()
            .filter(|voice| self.is_pattern_voice_active(&voice.id))
    }
    pub fn create_composition(
        &self,
        mut definition: CompositionDefinition,
    ) -> Result<Self, String> {
        if !self.performance.tracks.is_empty() {
            return Err("named compositions require the native project source".into());
        }
        if definition.id.is_empty()
            || definition.id.len() > 64
            || !definition
                .id
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_'))
        {
            return Err("invalid composition identity".into());
        }
        if self.compositions.contains_key(&definition.id) {
            return Err("duplicate composition identity".into());
        }
        if self.compositions.len() >= 32 || definition.voices.len() > 32 {
            return Err("composition or member limit is 32".into());
        }
        definition.voices.sort();
        for (index, id) in definition.voices.iter().enumerate() {
            if index > 0 && definition.voices[index - 1] == *id {
                return Err("duplicate composition member".into());
            }
            self.pattern_voices
                .get(id)
                .ok_or("unknown composition member")?;
        }
        let mut next = self.clone();
        if next.selected_composition.is_none() {
            next.selected_composition = Some(definition.id.clone());
        }
        next.compositions.insert(definition.id.clone(), definition);
        next.refresh_authoring_fingerprint();
        Ok(next)
    }
    pub fn select_composition(&self, id: &str) -> Result<Self, String> {
        if !self.compositions.contains_key(id) {
            return Err("unknown composition".into());
        }
        let mut next = self.clone();
        next.selected_composition = Some(id.into());
        next.refresh_authoring_fingerprint();
        Ok(next)
    }
    pub(super) fn fingerprint_compositions(&mut self) {
        if self.compositions.is_empty() {
            return;
        }
        let mut fingerprint = Fnv64::new();
        fingerprint.string("meldritch-named-compositions-v1");
        fingerprint.u64(self.fingerprint);
        fingerprint.optional_string(self.selected_composition.as_deref());
        for composition in self.compositions.values() {
            fingerprint.string(&composition.id);
            fingerprint.u64(composition.voices.len() as u64);
            for voice in &composition.voices {
                fingerprint.string(voice);
            }
        }
        self.fingerprint = fingerprint.finish();
    }
}
