//! Minimal project DSL parsing and validation.
//!
//! This crate accepts string-heavy TOML input and converts it into typed core
//! model values. It intentionally stays out of audio, rendering, and realtime.

use meldritch_core::{
    Diagnostic, EdgeKind, EventTag, Linearity, NodeId, NodeProperties, Param, Pattern, PatternId,
    Probability, ProbabilitySeed, RelationEdge, RelationGraph, RelationId, SampleRate, Source,
    SourceGraph, SourceId, Step, StepIndex, Tempo, TrackId,
};
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

mod song;

pub use song::{
    CableDefinition, CuratedControlDefinition, DspDefinition, ModuleDefinition, ModuleKind,
    NoteEventDefinition, NotePatternDefinition, ParameterInterpolation, ParameterLaneDefinition,
    ParameterOwner, ParameterPatternDefinition, ParameterPointDefinition,
    ParameterTargetDefinition, PatchInput, PerformanceDefinition, SignalType, SongDiagnostic,
    SongLoadError, SynthDefinition, TrackDefinition, ValidatedSong, load_song_directory,
};

#[derive(Clone, Debug, PartialEq)]
pub struct ValidatedProject {
    name: String,
    tempo: Tempo,
    probability_seed: ProbabilitySeed,
    samples: Vec<SampleRef>,
    patterns: Vec<Pattern>,
    relations: Vec<RelationRef>,
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

    #[must_use]
    pub fn relations(&self) -> &[RelationRef] {
        &self.relations
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SampleRef {
    note: u8,
    path: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RelationRef {
    from: RelationEndpoint,
    to: RelationEndpoint,
    kind: RelationKind,
}

impl RelationRef {
    #[must_use]
    pub const fn new(from: RelationEndpoint, to: RelationEndpoint, kind: RelationKind) -> Self {
        Self { from, to, kind }
    }

    #[must_use]
    pub const fn from(&self) -> RelationEndpoint {
        self.from
    }

    #[must_use]
    pub const fn to(&self) -> RelationEndpoint {
        self.to
    }

    #[must_use]
    pub const fn kind(&self) -> RelationKind {
        self.kind
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RelationEndpoint {
    SampleNote(u8),
    Pattern(PatternId),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RelationKind {
    Audio,
    Control,
    Sidechain,
}

#[derive(Clone, Debug)]
pub struct CompiledProject {
    sources: SourceGraph,
    relations: RelationGraph,
    source_bindings: Vec<SourceBinding>,
    relation_bindings: Vec<RelationBinding>,
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

    #[must_use]
    pub fn relation_bindings(&self) -> &[RelationBinding] {
        &self.relation_bindings
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RelationBinding {
    relation: RelationId,
    from: NodeId,
    to: NodeId,
    kind: RelationBindingKind,
}

impl RelationBinding {
    #[must_use]
    pub const fn relation(&self) -> RelationId {
        self.relation
    }

    #[must_use]
    pub const fn from(&self) -> NodeId {
        self.from
    }

    #[must_use]
    pub const fn to(&self) -> NodeId {
        self.to
    }

    #[must_use]
    pub const fn kind(&self) -> &RelationBindingKind {
        &self.kind
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RelationBindingKind {
    SampleToPattern {
        note: u8,
        pattern: PatternId,
    },
    PatternControlsPattern {
        from_pattern: PatternId,
        to_pattern: PatternId,
    },
    PatternSidechainsPattern {
        from_pattern: PatternId,
        to_pattern: PatternId,
    },
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
    let mut graph = CompiledGraphBuilder::new();

    for sample in project.samples() {
        let source = sample_source_id(sample.note());
        let node = NodeId::new(source.raw());
        graph.sources.insert(Source::new(source, node));
        graph
            .relations
            .insert_node(node, NodeProperties::new(Linearity::Linear));
        graph.sample_nodes_by_note.insert(sample.note(), node);
        graph.source_bindings.push(SourceBinding {
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
                graph.sources.insert(Source::new(source, node));
                graph
                    .relations
                    .insert_node(node, NodeProperties::new(Linearity::Linear));
                graph.pattern_nodes_by_id.insert(pattern.id().raw(), node);
                graph.source_bindings.push(SourceBinding {
                    source,
                    node,
                    kind: SourceBindingKind::Pattern {
                        pattern: pattern.id(),
                    },
                });

                for note in pattern.used_notes() {
                    graph.insert_sample_pattern_relation(note, pattern.id(), &mut diagnostics);
                }
            }
            None => diagnostics.push(Diagnostic::new(format!(
                "pattern {} cannot be compiled to a source id",
                pattern.id().raw()
            ))),
        }
    }

    for relation in project.relations() {
        match (relation.from(), relation.to(), relation.kind()) {
            (
                RelationEndpoint::SampleNote(note),
                RelationEndpoint::Pattern(pattern),
                RelationKind::Audio,
            ) => graph.insert_sample_pattern_relation(note, pattern, &mut diagnostics),
            (
                RelationEndpoint::Pattern(from_pattern),
                RelationEndpoint::Pattern(to_pattern),
                RelationKind::Control,
            ) => graph.insert_pattern_control_relation(from_pattern, to_pattern, &mut diagnostics),
            (
                RelationEndpoint::Pattern(from_pattern),
                RelationEndpoint::Pattern(to_pattern),
                RelationKind::Sidechain,
            ) => {
                graph.insert_pattern_sidechain_relation(from_pattern, to_pattern, &mut diagnostics)
            }
            _ => diagnostics.push(Diagnostic::new(
                "explicit relation shape is not supported by the compiler",
            )),
        }
    }

    if diagnostics.is_empty() {
        Ok(CompiledProject {
            sources: graph.sources,
            relations: graph.relations,
            source_bindings: graph.source_bindings,
            relation_bindings: graph.relation_bindings,
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

fn sample_to_pattern_relation_id(note: u8, pattern: PatternId) -> Option<RelationId> {
    2_000_000_000u64
        .checked_add(pattern.raw().checked_mul(1_000)?)
        .and_then(|id| id.checked_add(u64::from(note)))
        .map(RelationId::new)
}

fn pattern_control_relation_id(
    from_pattern: PatternId,
    to_pattern: PatternId,
) -> Option<RelationId> {
    3_000_000_000u64
        .checked_add(from_pattern.raw().checked_mul(1_000_000)?)
        .and_then(|id| id.checked_add(to_pattern.raw()))
        .map(RelationId::new)
}

fn pattern_sidechain_relation_id(
    from_pattern: PatternId,
    to_pattern: PatternId,
) -> Option<RelationId> {
    4_000_000_000u64
        .checked_add(from_pattern.raw().checked_mul(1_000_000)?)
        .and_then(|id| id.checked_add(to_pattern.raw()))
        .map(RelationId::new)
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum CompiledRelationKey {
    SampleAudio { note: u8, pattern: u64 },
    PatternControl { from_pattern: u64, to_pattern: u64 },
    PatternSidechain { from_pattern: u64, to_pattern: u64 },
}

struct CompiledGraphBuilder {
    sources: SourceGraph,
    relations: RelationGraph,
    source_bindings: Vec<SourceBinding>,
    relation_bindings: Vec<RelationBinding>,
    sample_nodes_by_note: BTreeMap<u8, NodeId>,
    pattern_nodes_by_id: BTreeMap<u64, NodeId>,
    relation_keys: BTreeSet<CompiledRelationKey>,
}

impl CompiledGraphBuilder {
    fn new() -> Self {
        Self {
            sources: SourceGraph::new(),
            relations: RelationGraph::new(),
            source_bindings: Vec::new(),
            relation_bindings: Vec::new(),
            sample_nodes_by_note: BTreeMap::new(),
            pattern_nodes_by_id: BTreeMap::new(),
            relation_keys: BTreeSet::new(),
        }
    }

    fn insert_sample_pattern_relation(
        &mut self,
        note: u8,
        pattern: PatternId,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        if !self.relation_keys.insert(CompiledRelationKey::SampleAudio {
            note,
            pattern: pattern.raw(),
        }) {
            return;
        }

        let Some(sample_node) = self.sample_nodes_by_note.get(&note).copied() else {
            diagnostics.push(Diagnostic::new(format!(
                "pattern {} uses note {} without a sample source",
                pattern.raw(),
                note
            )));
            return;
        };
        let Some(pattern_node) = self.pattern_nodes_by_id.get(&pattern.raw()).copied() else {
            diagnostics.push(Diagnostic::new(format!(
                "relation to pattern {} has no compiled pattern node",
                pattern.raw()
            )));
            return;
        };
        let Some(relation) = sample_to_pattern_relation_id(note, pattern) else {
            diagnostics.push(Diagnostic::new(format!(
                "pattern {} note {} cannot be compiled to a relation id",
                pattern.raw(),
                note
            )));
            return;
        };

        let edge = RelationEdge::new(relation, sample_node, pattern_node, EdgeKind::Audio);
        if let Err(err) = self.relations.insert_edge(edge) {
            diagnostics.push(Diagnostic::new(format!(
                "pattern {} note {} relation is invalid: {err}",
                pattern.raw(),
                note
            )));
        } else {
            self.relation_bindings.push(RelationBinding {
                relation,
                from: sample_node,
                to: pattern_node,
                kind: RelationBindingKind::SampleToPattern { note, pattern },
            });
        }
    }

    fn insert_pattern_control_relation(
        &mut self,
        from_pattern: PatternId,
        to_pattern: PatternId,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        if !self
            .relation_keys
            .insert(CompiledRelationKey::PatternControl {
                from_pattern: from_pattern.raw(),
                to_pattern: to_pattern.raw(),
            })
        {
            return;
        }

        let Some(from_node) = self.pattern_nodes_by_id.get(&from_pattern.raw()).copied() else {
            diagnostics.push(Diagnostic::new(format!(
                "control relation from pattern {} has no compiled pattern node",
                from_pattern.raw()
            )));
            return;
        };
        let Some(to_node) = self.pattern_nodes_by_id.get(&to_pattern.raw()).copied() else {
            diagnostics.push(Diagnostic::new(format!(
                "control relation to pattern {} has no compiled pattern node",
                to_pattern.raw()
            )));
            return;
        };
        let Some(relation) = pattern_control_relation_id(from_pattern, to_pattern) else {
            diagnostics.push(Diagnostic::new(format!(
                "pattern {} to pattern {} control relation cannot be compiled to a relation id",
                from_pattern.raw(),
                to_pattern.raw()
            )));
            return;
        };

        let edge = RelationEdge::new(relation, from_node, to_node, EdgeKind::Control);
        if let Err(err) = self.relations.insert_edge(edge) {
            diagnostics.push(Diagnostic::new(format!(
                "pattern {} to pattern {} control relation is invalid: {err}",
                from_pattern.raw(),
                to_pattern.raw()
            )));
        } else {
            self.relation_bindings.push(RelationBinding {
                relation,
                from: from_node,
                to: to_node,
                kind: RelationBindingKind::PatternControlsPattern {
                    from_pattern,
                    to_pattern,
                },
            });
        }
    }

    fn insert_pattern_sidechain_relation(
        &mut self,
        from_pattern: PatternId,
        to_pattern: PatternId,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        if !self
            .relation_keys
            .insert(CompiledRelationKey::PatternSidechain {
                from_pattern: from_pattern.raw(),
                to_pattern: to_pattern.raw(),
            })
        {
            return;
        }
        let Some(from_node) = self.pattern_nodes_by_id.get(&from_pattern.raw()).copied() else {
            diagnostics.push(Diagnostic::new(format!(
                "sidechain relation from pattern {} has no compiled pattern node",
                from_pattern.raw()
            )));
            return;
        };
        let Some(to_node) = self.pattern_nodes_by_id.get(&to_pattern.raw()).copied() else {
            diagnostics.push(Diagnostic::new(format!(
                "sidechain relation to pattern {} has no compiled pattern node",
                to_pattern.raw()
            )));
            return;
        };
        let Some(relation) = pattern_sidechain_relation_id(from_pattern, to_pattern) else {
            diagnostics.push(Diagnostic::new(format!(
                "pattern {} to pattern {} sidechain relation cannot be compiled",
                from_pattern.raw(),
                to_pattern.raw()
            )));
            return;
        };
        let edge = RelationEdge::new(relation, from_node, to_node, EdgeKind::Control);
        if let Err(err) = self.relations.insert_edge(edge) {
            diagnostics.push(Diagnostic::new(format!(
                "pattern {} to pattern {} sidechain relation is invalid: {err}",
                from_pattern.raw(),
                to_pattern.raw()
            )));
        } else {
            self.relation_bindings.push(RelationBinding {
                relation,
                from: from_node,
                to: to_node,
                kind: RelationBindingKind::PatternSidechainsPattern {
                    from_pattern,
                    to_pattern,
                },
            });
        }
    }
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
    let sample_notes = samples.iter().map(SampleRef::note).collect::<BTreeSet<_>>();

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
    let pattern_ids = patterns
        .iter()
        .map(Pattern::id)
        .map(PatternId::raw)
        .collect::<BTreeSet<_>>();
    let relations =
        validate_relations(raw.relations, &sample_notes, &pattern_ids, &mut diagnostics);

    if diagnostics.is_empty() {
        Ok(ValidatedProject {
            name: raw.project.name,
            tempo,
            probability_seed: ProbabilitySeed::new(raw.project.seed.unwrap_or(0)),
            samples,
            patterns,
            relations,
        })
    } else {
        Err(ProjectValidationError::new(diagnostics))
    }
}

fn validate_relations(
    raw_relations: Vec<RawRelation>,
    sample_notes: &BTreeSet<u8>,
    pattern_ids: &BTreeSet<u64>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<RelationRef> {
    let mut relations = Vec::new();

    for (index, raw_relation) in raw_relations.into_iter().enumerate() {
        let relation_number = index + 1;
        let Some(from) = validate_relation_endpoint(
            raw_relation.from,
            sample_notes,
            pattern_ids,
            relation_number,
            "from",
            diagnostics,
        ) else {
            continue;
        };
        let Some(to) = validate_relation_endpoint(
            raw_relation.to,
            sample_notes,
            pattern_ids,
            relation_number,
            "to",
            diagnostics,
        ) else {
            continue;
        };
        let Some(kind) = validate_relation_kind(&raw_relation.kind, relation_number, diagnostics)
        else {
            continue;
        };

        match (from, to, kind) {
            (
                RelationEndpoint::SampleNote(_),
                RelationEndpoint::Pattern(_),
                RelationKind::Audio,
            ) => relations.push(RelationRef::new(from, to, kind)),
            (
                RelationEndpoint::Pattern(_),
                RelationEndpoint::Pattern(_),
                RelationKind::Control,
            ) => relations.push(RelationRef::new(from, to, kind)),
            (
                RelationEndpoint::Pattern(_),
                RelationEndpoint::Pattern(_),
                RelationKind::Sidechain,
            ) => relations.push(RelationRef::new(from, to, kind)),
            _ => diagnostics.push(Diagnostic::new(format!(
                "relation {relation_number} must be sample_note -> pattern audio or pattern -> pattern control/sidechain"
            ))),
        }
    }

    relations
}

fn validate_relation_endpoint(
    raw: RawRelationEndpoint,
    sample_notes: &BTreeSet<u8>,
    pattern_ids: &BTreeSet<u64>,
    relation_number: usize,
    side: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<RelationEndpoint> {
    match (raw.sample_note, raw.pattern) {
        (Some(note), None) => {
            if sample_notes.contains(&note) {
                Some(RelationEndpoint::SampleNote(note))
            } else {
                diagnostics.push(Diagnostic::new(format!(
                    "relation {relation_number} {side} references unknown sample note {note}"
                )));
                None
            }
        }
        (None, Some(pattern)) => {
            if pattern_ids.contains(&pattern) {
                Some(RelationEndpoint::Pattern(PatternId::new(pattern)))
            } else {
                diagnostics.push(Diagnostic::new(format!(
                    "relation {relation_number} {side} references unknown pattern {pattern}"
                )));
                None
            }
        }
        (None, None) => {
            diagnostics.push(Diagnostic::new(format!(
                "relation {relation_number} {side} endpoint is empty"
            )));
            None
        }
        (Some(_), Some(_)) => {
            diagnostics.push(Diagnostic::new(format!(
                "relation {relation_number} {side} endpoint must specify exactly one target"
            )));
            None
        }
    }
}

fn validate_relation_kind(
    kind: &str,
    relation_number: usize,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<RelationKind> {
    match kind {
        "audio" => Some(RelationKind::Audio),
        "control" => Some(RelationKind::Control),
        "sidechain" => Some(RelationKind::Sidechain),
        _ => {
            diagnostics.push(Diagnostic::new(format!(
                "relation {relation_number} has unknown kind '{kind}'"
            )));
            None
        }
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
    #[serde(default)]
    relations: Vec<RawRelation>,
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

#[derive(Debug, Deserialize)]
struct RawRelation {
    from: RawRelationEndpoint,
    to: RawRelationEndpoint,
    kind: String,
}

#[derive(Debug, Deserialize)]
struct RawRelationEndpoint {
    sample_note: Option<u8>,
    pattern: Option<u64>,
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
        assert!(project.relations().is_empty());

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
        assert_eq!(compiled.relations().len_edges(), 2);
        assert_eq!(compiled.source_bindings().len(), 3);
        assert_eq!(compiled.relation_bindings().len(), 2);
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
        assert_eq!(
            compiled.relation_bindings()[0].kind(),
            &RelationBindingKind::SampleToPattern {
                note: 36,
                pattern: PatternId::new(1)
            }
        );
        assert_eq!(compiled.relation_bindings()[0].from(), NodeId::new(1_036));
        assert_eq!(
            compiled.relation_bindings()[0].to(),
            NodeId::new(1_000_000_001)
        );
    }

    #[test]
    fn parses_explicit_sample_to_pattern_relations() {
        let input = MINIMAL_PROJECT.to_owned()
            + r#"

[[relations]]
from = { sample_note = 36 }
to = { pattern = 1 }
kind = "audio"
"#;
        let project = parse_project(&input).unwrap();

        assert_eq!(
            project.relations(),
            &[RelationRef::new(
                RelationEndpoint::SampleNote(36),
                RelationEndpoint::Pattern(PatternId::new(1)),
                RelationKind::Audio
            )]
        );

        let compiled = compile_project(&project).unwrap();
        assert_eq!(compiled.relation_bindings().len(), 2);
    }

    #[test]
    fn parses_explicit_pattern_control_relations() {
        let input = MINIMAL_PROJECT.to_owned()
            + r#"

[[patterns]]
id = 2
length_steps = 16
steps_per_beat = 4

[[patterns.tracks]]
id = 1

[[patterns.tracks.steps]]
step = 8
note = 36
velocity = 0.5

[[relations]]
from = { pattern = 1 }
to = { pattern = 2 }
kind = "control"
"#;
        let project = parse_project(&input).unwrap();

        assert_eq!(
            project.relations(),
            &[RelationRef::new(
                RelationEndpoint::Pattern(PatternId::new(1)),
                RelationEndpoint::Pattern(PatternId::new(2)),
                RelationKind::Control
            )]
        );

        let compiled = compile_project(&project).unwrap();
        assert_eq!(compiled.relation_bindings().len(), 4);
        assert_eq!(
            compiled.relation_bindings()[3].kind(),
            &RelationBindingKind::PatternControlsPattern {
                from_pattern: PatternId::new(1),
                to_pattern: PatternId::new(2)
            }
        );
        assert_eq!(
            compiled.relation_bindings()[3].from(),
            NodeId::new(1_000_000_001)
        );
        assert_eq!(
            compiled.relation_bindings()[3].to(),
            NodeId::new(1_000_000_002)
        );
    }

    #[test]
    fn sidechain_relations_compile_to_control_edges_and_propagate_dirty_ranges() {
        let input = MINIMAL_PROJECT.to_owned()
            + r#"

[[patterns]]
id = 2
length_steps = 16
steps_per_beat = 4

[[patterns.tracks]]
id = 4

[[patterns.tracks.steps]]
step = 0
note = 36

[[relations]]
from = { pattern = 1 }
to = { pattern = 2 }
kind = "sidechain"
"#;
        let project = parse_project(&input).unwrap();
        assert_eq!(project.relations()[0].kind(), RelationKind::Sidechain);
        let compiled = compile_project(&project).unwrap();
        assert!(matches!(
            compiled.relation_bindings().last().unwrap().kind(),
            RelationBindingKind::PatternSidechainsPattern {
                from_pattern,
                to_pattern,
            } if *from_pattern == PatternId::new(1) && *to_pattern == PatternId::new(2)
        ));
        let range = FrameRange::new(12_000, 18_000).unwrap();
        let dirty = compiled
            .relations()
            .invalidate_from(NodeId::new(1_000_000_001), range);
        assert!(dirty.iter().any(|dirty| {
            dirty.entity() == meldritch_core::EntityId::Node(NodeId::new(1_000_000_002))
                && dirty.range() == range
        }));
    }

    #[test]
    fn relation_validation_reports_unknown_targets() {
        let input = MINIMAL_PROJECT.to_owned()
            + r#"

[[relations]]
from = { sample_note = 99 }
to = { pattern = 2 }
kind = "audio"
"#;
        let err = parse_project(&input).unwrap_err();
        let messages = err
            .diagnostics()
            .into_iter()
            .map(|diagnostic| diagnostic.message().to_owned())
            .collect::<Vec<_>>();

        assert_eq!(
            messages,
            vec!["relation 1 from references unknown sample note 99"]
        );
    }

    #[test]
    fn relation_validation_reports_unknown_kinds() {
        let input = MINIMAL_PROJECT.to_owned()
            + r#"

[[relations]]
from = { sample_note = 36 }
to = { pattern = 1 }
kind = "mystery"
"#;
        let err = parse_project(&input).unwrap_err();
        let messages = err
            .diagnostics()
            .into_iter()
            .map(|diagnostic| diagnostic.message().to_owned())
            .collect::<Vec<_>>();

        assert_eq!(messages, vec!["relation 1 has unknown kind 'mystery'"]);
    }

    #[test]
    fn compile_reports_pattern_notes_without_samples() {
        let input = MINIMAL_PROJECT.replace(
            "note = 38\npath = \"audio/snare.wav\"",
            "note = 39\npath = \"audio/clap.wav\"",
        );
        let project = parse_project(&input).unwrap();
        let err = compile_project(&project).unwrap_err();
        let messages = err
            .diagnostics()
            .iter()
            .map(|diagnostic| diagnostic.message().to_owned())
            .collect::<Vec<_>>();

        assert_eq!(
            messages,
            vec!["pattern 1 uses note 38 without a sample source"]
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
