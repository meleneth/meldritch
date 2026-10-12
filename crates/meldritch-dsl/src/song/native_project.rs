//! A bounded persistence projection of the authoritative source, not a new engine.
use super::*;

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NativeProjectMetadata {
    pub id: String,
    pub title: String,
    pub bpm: f64,
    pub sample_rate: u32,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NativeProjectDefinition {
    pub kind: String,
    pub version: u32,
    pub project: NativeProjectMetadata,
    pub patches: Vec<SynthDefinition>,
    pub patterns: Vec<PatternMaterial>,
    pub voices: Vec<PatternVoiceDefinition>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub compositions: Vec<CompositionDefinition>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_composition: Option<String>,
}

impl ValidatedSong {
    /// Validate a complete candidate; no application or published audio is mutated.
    pub fn restore_native_project(
        root: impl AsRef<Path>,
        definition: &NativeProjectDefinition,
    ) -> Result<Self, String> {
        if definition.kind != "project" || !matches!(definition.version, 2 | 3) {
            return Err("unsupported self-contained project kind/version".into());
        }
        if definition.version == 2
            && (!definition.compositions.is_empty() || definition.selected_composition.is_some())
        {
            return Err("named compositions require project version 3".into());
        }
        if definition.version == 3
            && (definition.compositions.is_empty() || definition.selected_composition.is_none())
        {
            return Err("project version 3 requires compositions and explicit selection".into());
        }
        if definition.compositions.len() > 32 {
            return Err("composition limit is 32".into());
        }
        let metadata = &definition.project;
        if metadata.id.is_empty()
            || metadata.id.len() > 128
            || metadata.title.len() > 1024
            || !metadata.bpm.is_finite()
            || metadata.bpm <= 0.0
            || !(8_000..=192_000).contains(&metadata.sample_rate)
        {
            return Err("invalid native project identity or clock".into());
        }
        if definition.patches.len() > 32
            || definition.patterns.len() > 32
            || definition.voices.len() > 32
            || definition.patterns.iter().any(|p| p.notes.len() > 4096)
        {
            return Err("native project exceeds palette, instance or note limits".into());
        }
        let root = root.as_ref().canonicalize().map_err(|e| e.to_string())?;
        if !root.is_dir() {
            return Err("project root is not a directory".into());
        }
        let mut source = project_directory::empty_project_source(&root);
        source.performance.id = metadata.id.clone();
        source.performance.title = metadata.title.clone();
        source.performance.bpm = metadata.bpm;
        source.performance.sample_rate = metadata.sample_rate;
        for patch in &definition.patches {
            source = source.restore_palette_patch(patch.clone())?;
        }
        for pattern in &definition.patterns {
            if source.note_patterns.contains_key(&pattern.id) {
                return Err(format!("duplicate pattern identity '{}'", pattern.id));
            }
            source = source.restore_pattern_material(pattern)?;
        }
        for voice in &definition.voices {
            source = source.create_pattern_voice(voice.clone())?;
        }
        for composition in &definition.compositions {
            source = source.create_composition(composition.clone())?;
        }
        if let Some(id) = &definition.selected_composition {
            source = source.select_composition(id)?;
        }
        source.refresh_authoring_fingerprint();
        Ok(source)
    }

    /// Project all authored semantics or reject loss. Runtime state is not an input.
    pub fn native_project_definition(&self) -> Result<NativeProjectDefinition, String> {
        let definition = NativeProjectDefinition {
            kind: "project".into(),
            version: if self.compositions.is_empty() { 2 } else { 3 },
            project: NativeProjectMetadata {
                id: self.performance.id.clone(),
                title: self.performance.title.clone(),
                bpm: self.performance.bpm,
                sample_rate: self.performance.sample_rate,
            },
            patches: self.synths.values().cloned().collect(),
            patterns: self
                .note_patterns
                .keys()
                .map(|id| self.pattern_material(id))
                .collect::<Result<_, _>>()?,
            voices: self.pattern_voices.values().cloned().collect(),
            compositions: self.compositions.values().cloned().collect(),
            selected_composition: self.selected_composition.clone(),
        };
        // Equality covers metadata and source maps that this bounded format omits.
        // It also protects against unsupported sample transforms and tick rounding.
        let restored = Self::restore_native_project(&self.root, &definition)?;
        if restored != *self {
            return Err(
                "self-contained project export refuses unsupported or lossy source state".into(),
            );
        }
        Ok(definition)
    }
}
