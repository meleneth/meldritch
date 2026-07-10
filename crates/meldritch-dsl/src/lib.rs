//! Minimal project DSL parsing and validation.
//!
//! This crate accepts string-heavy TOML input and converts it into typed core
//! model values. It intentionally stays out of audio, rendering, and realtime.

use meldritch_core::{
    Diagnostic, EventTag, Linearity, NodeId, NodeProperties, Param, Pattern, PatternId,
    Probability, ProbabilitySeed, RelationGraph, SampleRate, Source, SourceGraph, SourceId, Step,
    StepIndex, Tempo, TrackId,
};
use serde::Deserialize;
use std::collections::BTreeSet;
use std::fmt;

#[derive(Clone, Debug, PartialEq)]
pub struct ValidatedProject {
    name: String,
    tempo: Tempo,
    probability_seed: ProbabilitySeed,
    samples: Vec<SampleRef>,
    patterns: Vec<Pattern>,
}

impl ValidatedProject {
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub const fn tempo(&self) -> Tempo {
        self.tempo
    }

    #[must_use]
    pub const fn probability_seed(&self) -> ProbabilitySeed {
        self.probability_seed
    }

    #[must_use]
    pub fn patterns(&self) -> &[Pattern] {
        &self.patterns
    }

    #[must_use]
    pub fn samples(&self) -> &[SampleRef] {
        &self.samples
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SampleRef {
    note: u8,
    path: String,
}

#[derive(Clone, Debug)]
pub struct CompiledProject {
    sources: SourceGraph,
    relations: RelationGraph,
    source_bindings: Vec<SourceBinding>,
}

impl CompiledProject {
    #[must_use]
    pub fn sources(&self) -> &SourceGraph {
        &self.sources
    }

    #[must_use]
    pub fn relations(&self) -> &RelationGraph {
        &self.relations
    }

    #[must_use]
    pub fn source_bindings(&self) -> &[SourceBinding] {
        &self.source_bindings
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceBinding {
    source: SourceId,
    node: NodeId,
    kind: SourceBindingKind,
}

impl SourceBinding {
    #[must_use]
    pub const fn source(&self) -> SourceId {
        self.source
    }

    #[must_use]
    pub const fn node(&self) -> NodeId {
        self.node
    }

    #[must_use]
    pub const fn kind(&self) -> &SourceBindingKind {
        &self.kind
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SourceBindingKind {
    Sample { note: u8, path: String },
    Pattern { pattern: PatternId },
}

#[derive(Clone, Debug, PartialEq)]
pub struct CompileProjectError {
    diagnostics: Vec<Diagnostic>,
}

impl CompileProjectError {
    #[must_use]
    pub fn new(diagnostics: Vec<Diagnostic>) -> Self {
        Self { diagnostics }
    }

    #[must_use]
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }
}

impl fmt::Display for CompileProjectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "project compilation failed")
    }
}

impl std::error::Error for CompileProjectError {}

impl SampleRef {
    #[must_use]
    pub fn new(note: u8, path: impl Into<String>) -> Self {
        Self {
            note,
            path: path.into(),
        }
    }

    #[must_use]
    pub const fn note(&self) -> u8 {
        self.note
    }

    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProjectValidationError {
    diagnostics: Vec<Diagnostic>,
}

impl ProjectValidationError {
    #[must_use]
    pub fn new(diagnostics: Vec<Diagnostic>) -> Self {
        Self { diagnostics }
    }

    #[must_use]
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }
}

impl fmt::Display for ProjectValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "project validation failed")
    }
}

impl std::error::Error for ProjectValidationError {}

#[derive(Debug)]
pub enum ParseProjectError {
    Toml(toml::de::Error),
    Validation(ProjectValidationError),
}

impl ParseProjectError {
    #[must_use]
    pub fn diagnostics(&self) -> Vec<Diagnostic> {
        match self {
            Self::Toml(err) => vec![Diagnostic::new(format!("toml parse error: {err}"))],
            Self::Validation(err) => err.diagnostics().to_vec(),
        }
    }
}

impl fmt::Display for ParseProjectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Toml(err) => write!(f, "{err}"),
            Self::Validation(err) => write!(f, "{err}"),
        }
    }
}

impl std::error::Error for ParseProjectError {}

pub fn parse_project(input: &str) -> Result<ValidatedProject, ParseProjectError> {
    let raw = toml::from_str::<RawProject>(input).map_err(ParseProjectError::Toml)?;
    validate_project(raw).map_err(ParseProjectError::Validation)
}

pub fn compile_project(project: &ValidatedProject) -> Result<CompiledProject, CompileProjectError> {
    let mut diagnostics = Vec::new();
    let mut sources = SourceGraph::new();
    let mut relations = RelationGraph::new();
    let mut source_bindings = Vec::new();

    for sample in project.samples() {
        let source = sample_source_id(sample.note());
        let node = NodeId::new(source.raw());
        sources.insert(Source::new(source, node));
        relations.insert_node(node, NodeProperties::new(Linearity::Linear));
        source_bindings.push(SourceBinding {
            source,
            node,
            kind: SourceBindingKind::Sample {
                note: sample.note(),
                path: sample.path().to_owned(),
            },
        });
    }

    for pattern in project.patterns() {
        match pattern_source_id(pattern.id()) {
            Some(source) => {
                let node = NodeId::new(source.raw());
                sources.insert(Source::new(source, node));
                relations.insert_node(node, NodeProperties::new(Linearity::Linear));
                source_bindings.push(SourceBinding {
                    source,
                    node,
                    kind: SourceBindingKind::Pattern {
                        pattern: pattern.id(),
                    },
                });
            }
            None => diagnostics.push(Diagnostic::new(format!(
                "pattern {} cannot be compiled to a source id",
                pattern.id().raw()
            ))),
        }
    }

    if diagnostics.is_empty() {
        Ok(CompiledProject {
            sources,
            relations,
            source_bindings,
        })
    } else {
        Err(CompileProjectError::new(diagnostics))
    }
}

fn sample_source_id(note: u8) -> SourceId {
    SourceId::new(1_000 + u64::from(note))
}

fn pattern_source_id(pattern: PatternId) -> Option<SourceId> {
    1_000_000_000u64
        .checked_add(pattern.raw())
        .map(SourceId::new)
}

fn validate_project(raw: RawProject) -> Result<ValidatedProject, ProjectValidationError> {
    let mut diagnostics = Vec::new();

    let tempo = match Tempo::new(raw.project.bpm, raw.project.sample_rate) {
        Ok(tempo) => tempo,
        Err(err) => {
            diagnostics.push(Diagnostic::new(format!("project tempo is invalid: {err}")));
            Tempo::new(120.0, 48_000).expect("fallback tempo is valid")
        }
    };

    let samples = validate_samples(raw.samples, &mut diagnostics);

    let mut seen_patterns = BTreeSet::new();
    let mut patterns = Vec::new();
    for raw_pattern in raw.patterns {
        if !seen_patterns.insert(raw_pattern.id) {
            diagnostics.push(Diagnostic::new(format!(
                "pattern {} is mapped more than once",
                raw_pattern.id
            )));
            continue;
        }

        match validate_pattern(raw_pattern) {
            Ok(pattern) => patterns.push(pattern),
            Err(mut pattern_diagnostics) => diagnostics.append(&mut pattern_diagnostics),
        }
    }

    if diagnostics.is_empty() {
        Ok(ValidatedProject {
            name: raw.project.name,
            tempo,
            probability_seed: ProbabilitySeed::new(raw.project.seed.unwrap_or(0)),
            samples,
            patterns,
        })
    } else {
        Err(ProjectValidationError::new(diagnostics))
    }
}

fn validate_samples(
    raw_samples: Vec<RawSample>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<SampleRef> {
    let mut seen_notes = BTreeSet::new();
    let mut samples = Vec::new();

    for raw_sample in raw_samples {
        if raw_sample.path.trim().is_empty() {
            diagnostics.push(Diagnostic::new(format!(
                "sample for note {} has an empty path",
                raw_sample.note
            )));
            continue;
        }

        if !seen_notes.insert(raw_sample.note) {
            diagnostics.push(Diagnostic::new(format!(
                "sample note {} is mapped more than once",
                raw_sample.note
            )));
            continue;
        }

        samples.push(SampleRef::new(raw_sample.note, raw_sample.path));
    }

    samples
}

fn validate_pattern(raw: RawPattern) -> Result<Pattern, Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    let mut pattern =
        match Pattern::new(PatternId::new(raw.id), raw.length_steps, raw.steps_per_beat) {
            Ok(pattern) => pattern,
            Err(err) => {
                return Err(vec![Diagnostic::new(format!(
                    "pattern {} is invalid: {err}",
                    raw.id
                ))]);
            }
        };

    for raw_track in raw.tracks {
        let track = TrackId::new(raw_track.id);
        for raw_step in raw_track.steps {
            match validate_step(&raw_step) {
                Ok(step) => {
                    if let Err(err) = pattern.set_step(track, StepIndex::new(raw_step.step), step) {
                        diagnostics.push(Diagnostic::new(format!(
                            "pattern {} track {} step {} is invalid: {err}",
                            raw.id, raw_track.id, raw_step.step
                        )));
                    }
                }
                Err(mut step_diagnostics) => diagnostics.append(&mut step_diagnostics),
            }
        }
    }

    if diagnostics.is_empty() {
        Ok(pattern)
    } else {
        Err(diagnostics)
    }
}

fn validate_step(raw: &RawStep) -> Result<Step, Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    let probability = match raw.probability {
        Some(chance) => match Probability::new(chance) {
            Ok(probability) => probability,
            Err(err) => {
                diagnostics.push(Diagnostic::new(format!(
                    "step {} probability is invalid: {err}",
                    raw.step
                )));
                Probability::ALWAYS
            }
        },
        None => Probability::ALWAYS,
    };

    let mut step = Step::new(raw.note)
        .with_velocity(raw.velocity.unwrap_or(1.0))
        .with_gate(raw.gate.unwrap_or(1.0))
        .with_probability(probability);

    for tag in &raw.tags {
        match parse_tag(tag) {
            Some(tag) => step = step.with_tag(tag),
            None => diagnostics.push(Diagnostic::new(format!(
                "step {} has unknown tag '{tag}'",
                raw.step
            ))),
        }
    }

    if diagnostics.is_empty() {
        Ok(step)
    } else {
        Err(diagnostics)
    }
}

fn parse_tag(tag: &str) -> Option<EventTag> {
    match tag {
        "accent" => Some(EventTag::Accent),
        "ghost" => Some(EventTag::Ghost),
        "fill" => Some(EventTag::Fill),
        "ratchet" => Some(EventTag::Ratchet),
        "probabilistic" => Some(EventTag::Probabilistic),
        "humanized" => Some(EventTag::Humanized),
        "scene_transition" => Some(EventTag::SceneTransition),
        _ => None,
    }
}

#[derive(Debug, Deserialize)]
struct RawProject {
    project: RawProjectMeta,
    #[serde(default)]
    samples: Vec<RawSample>,
    #[serde(default)]
    patterns: Vec<RawPattern>,
}

#[derive(Debug, Deserialize)]
struct RawProjectMeta {
    name: String,
    bpm: f64,
    sample_rate: SampleRate,
    seed: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct RawSample {
    note: u8,
    path: String,
}

#[derive(Debug, Deserialize)]
struct RawPattern {
    id: u64,
    length_steps: u32,
    steps_per_beat: u32,
    #[serde(default)]
    tracks: Vec<RawTrack>,
}

#[derive(Debug, Deserialize)]
struct RawTrack {
    id: u64,
    #[serde(default)]
    steps: Vec<RawStep>,
}

#[derive(Debug, Deserialize)]
struct RawStep {
    step: u32,
    note: u8,
    velocity: Option<Param>,
    gate: Option<f64>,
    probability: Option<f64>,
    #[serde(default)]
    tags: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use meldritch_core::FrameRange;

    const MINIMAL_PROJECT: &str = r#"
[project]
name = "Minimal Kick"
bpm = 120.0
sample_rate = 48000
seed = 7

[[samples]]
note = 36
path = "audio/kick.wav"

[[samples]]
note = 38
path = "audio/snare.wav"

[[patterns]]
id = 1
length_steps = 16
steps_per_beat = 4

[[patterns.tracks]]
id = 1

[[patterns.tracks.steps]]
step = 0
note = 36
velocity = 0.9
gate = 0.5
tags = ["accent"]

[[patterns.tracks.steps]]
step = 4
note = 38
probability = 0.25
tags = ["ghost", "probabilistic"]
"#;

    #[test]
    fn parses_minimal_project_into_typed_model() {
        let project = parse_project(MINIMAL_PROJECT).unwrap();

        assert_eq!(project.name(), "Minimal Kick");
        assert_eq!(project.tempo(), Tempo::new(120.0, 48_000).unwrap());
        assert_eq!(project.probability_seed(), ProbabilitySeed::new(7));
        assert_eq!(
            project.samples(),
            &[
                SampleRef::new(36, "audio/kick.wav"),
                SampleRef::new(38, "audio/snare.wav"),
            ]
        );
        assert_eq!(project.patterns().len(), 1);

        let pattern = &project.patterns()[0];
        let kick = pattern
            .get_step(TrackId::new(1), StepIndex::new(0))
            .expect("kick step should parse");
        assert_eq!(kick.note(), 36);
        assert_eq!(kick.velocity(), 0.9);
        assert_eq!(kick.gate(), 0.5);
        assert!(kick.tags().contains(&EventTag::Accent));

        let snare = pattern
            .get_step(TrackId::new(1), StepIndex::new(4))
            .expect("snare step should parse");
        assert_eq!(snare.probability().chance(), 0.25);
        assert!(snare.tags().contains(&EventTag::Ghost));
        assert!(snare.tags().contains(&EventTag::Probabilistic));
    }

    #[test]
    fn parsed_project_schedules_events_with_core_model() {
        let project = parse_project(MINIMAL_PROJECT).unwrap();
        let mut events = Vec::new();
        project.patterns()[0].events_between(
            project.tempo(),
            FrameRange::new(0, 30_001).unwrap(),
            project.probability_seed(),
            &mut events,
        );

        assert!(!events.is_empty());
        assert_eq!(events[0].track(), TrackId::new(1));
        assert_eq!(events[0].range(), FrameRange::new(0, 3_000).unwrap());
    }

    #[test]
    fn compiles_project_sources_into_graph_nodes() {
        let project = parse_project(MINIMAL_PROJECT).unwrap();
        let compiled = compile_project(&project).unwrap();

        assert_eq!(compiled.sources().len(), 3);
        assert_eq!(compiled.relations().len_nodes(), 3);
        assert_eq!(compiled.relations().len_edges(), 0);
        assert_eq!(compiled.source_bindings().len(), 3);
        assert_eq!(compiled.source_bindings()[0].source(), SourceId::new(1_036));
        assert_eq!(compiled.source_bindings()[0].node(), NodeId::new(1_036));
        assert_eq!(
            compiled.source_bindings()[0].kind(),
            &SourceBindingKind::Sample {
                note: 36,
                path: "audio/kick.wav".to_owned()
            }
        );
        assert_eq!(
            compiled.source_bindings()[2].kind(),
            &SourceBindingKind::Pattern {
                pattern: PatternId::new(1)
            }
        );
    }

    #[test]
    fn compile_reports_pattern_source_id_overflow() {
        let input = MINIMAL_PROJECT.replace("id = 1", "id = 18446744072709551616");
        let project = parse_project(&input).unwrap();
        let err = compile_project(&project).unwrap_err();
        let messages = err
            .diagnostics()
            .iter()
            .map(|diagnostic| diagnostic.message().to_owned())
            .collect::<Vec<_>>();

        assert_eq!(
            messages,
            vec!["pattern 18446744072709551616 cannot be compiled to a source id"]
        );
    }

    #[test]
    fn unknown_tags_return_useful_diagnostics() {
        let input = MINIMAL_PROJECT.replace("\"accent\"", "\"sparkle\"");
        let err = parse_project(&input).unwrap_err();
        let messages = err
            .diagnostics()
            .into_iter()
            .map(|diagnostic| diagnostic.message().to_owned())
            .collect::<Vec<_>>();

        assert_eq!(messages, vec!["step 0 has unknown tag 'sparkle'"]);
    }

    #[test]
    fn out_of_range_steps_return_useful_diagnostics() {
        let input = MINIMAL_PROJECT.replace("step = 4", "step = 64");
        let err = parse_project(&input).unwrap_err();
        let messages = err
            .diagnostics()
            .into_iter()
            .map(|diagnostic| diagnostic.message().to_owned())
            .collect::<Vec<_>>();

        assert_eq!(
            messages,
            vec!["pattern 1 track 1 step 64 is invalid: step 64 is outside pattern length 16"]
        );
    }

    #[test]
    fn duplicate_sample_notes_return_useful_diagnostics() {
        let input = MINIMAL_PROJECT.replace(
            "note = 38\npath = \"audio/snare.wav\"",
            "note = 36\npath = \"audio/snare.wav\"",
        );
        let err = parse_project(&input).unwrap_err();
        let messages = err
            .diagnostics()
            .into_iter()
            .map(|diagnostic| diagnostic.message().to_owned())
            .collect::<Vec<_>>();

        assert_eq!(messages, vec!["sample note 36 is mapped more than once"]);
    }

    #[test]
    fn duplicate_pattern_ids_return_useful_diagnostics() {
        let input = MINIMAL_PROJECT.to_owned()
            + r#"

[[patterns]]
id = 1
length_steps = 8
steps_per_beat = 4
"#;
        let err = parse_project(&input).unwrap_err();
        let messages = err
            .diagnostics()
            .into_iter()
            .map(|diagnostic| diagnostic.message().to_owned())
            .collect::<Vec<_>>();

        assert_eq!(messages, vec!["pattern 1 is mapped more than once"]);
    }
}
