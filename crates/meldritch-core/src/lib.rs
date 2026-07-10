//! Core model types for Meldritch.
//!
//! This crate is intentionally headless: it owns identifiers, timeline ranges,
//! command results, graph structure, typed ports, and invalidation behavior.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt;

pub type Sample = f64;
pub type Param = f64;
pub type Coeff = f64;
pub type Frame = u64;
pub type Frames = u32;
pub type SampleRate = u32;

macro_rules! id_type {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name(u64);

        impl $name {
            #[must_use]
            pub const fn new(raw: u64) -> Self {
                Self(raw)
            }

            #[must_use]
            pub const fn raw(self) -> u64 {
                self.0
            }
        }
    };
}

id_type!(CommandId);
id_type!(NodeId);
id_type!(PortId);
id_type!(RelationId);
id_type!(SourceId);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum EntityId {
    Node(NodeId),
    Port(PortId),
    Relation(RelationId),
    Source(SourceId),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FrameRange {
    start: Frame,
    end: Frame,
}

impl FrameRange {
    pub const ZERO: Self = Self { start: 0, end: 0 };

    pub fn new(start: Frame, end: Frame) -> Result<Self, FrameRangeError> {
        if start > end {
            return Err(FrameRangeError::StartAfterEnd { start, end });
        }

        Ok(Self { start, end })
    }

    #[must_use]
    pub const fn start(self) -> Frame {
        self.start
    }

    #[must_use]
    pub const fn end(self) -> Frame {
        self.end
    }

    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.start == self.end
    }

    #[must_use]
    pub const fn contains_frame(self, frame: Frame) -> bool {
        self.start <= frame && frame < self.end
    }

    #[must_use]
    pub const fn contains_range(self, other: Self) -> bool {
        self.start <= other.start && other.end <= self.end
    }

    #[must_use]
    pub const fn overlaps(self, other: Self) -> bool {
        self.start < other.end && other.start < self.end
    }

    #[must_use]
    pub const fn expand_saturating(self, lookahead_frames: Frames, tail_frames: Frames) -> Self {
        Self {
            start: self.start.saturating_sub(lookahead_frames as Frame),
            end: self.end.saturating_add(tail_frames as Frame),
        }
    }

    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self {
            start: if self.start < other.start {
                self.start
            } else {
                other.start
            },
            end: if self.end > other.end {
                self.end
            } else {
                other.end
            },
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FrameRangeError {
    StartAfterEnd { start: Frame, end: Frame },
}

impl fmt::Display for FrameRangeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StartAfterEnd { start, end } => {
                write!(f, "frame range start {start} is after end {end}")
            }
        }
    }
}

impl std::error::Error for FrameRangeError {}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum EdgeKind {
    Audio,
    Control,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Linearity {
    Linear,
    Nonlinear,
    TimeVariant,
    Feedback,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NodeProperties {
    linearity: Linearity,
    latency_frames: Frames,
    lookahead_frames: Frames,
    tail_frames: Frames,
    can_cache: bool,
    needs_group_signal: bool,
}

impl NodeProperties {
    #[must_use]
    pub const fn new(linearity: Linearity) -> Self {
        Self {
            linearity,
            latency_frames: 0,
            lookahead_frames: 0,
            tail_frames: 0,
            can_cache: true,
            needs_group_signal: false,
        }
    }

    #[must_use]
    pub const fn linearity(self) -> Linearity {
        self.linearity
    }

    #[must_use]
    pub const fn latency_frames(self) -> Frames {
        self.latency_frames
    }

    #[must_use]
    pub const fn lookahead_frames(self) -> Frames {
        self.lookahead_frames
    }

    #[must_use]
    pub const fn tail_frames(self) -> Frames {
        self.tail_frames
    }

    #[must_use]
    pub const fn can_cache(self) -> bool {
        self.can_cache
    }

    #[must_use]
    pub const fn needs_group_signal(self) -> bool {
        self.needs_group_signal
    }

    #[must_use]
    pub const fn with_latency_frames(mut self, frames: Frames) -> Self {
        self.latency_frames = frames;
        self
    }

    #[must_use]
    pub const fn with_lookahead_frames(mut self, frames: Frames) -> Self {
        self.lookahead_frames = frames;
        self
    }

    #[must_use]
    pub const fn with_tail_frames(mut self, frames: Frames) -> Self {
        self.tail_frames = frames;
        self
    }

    #[must_use]
    pub const fn with_can_cache(mut self, can_cache: bool) -> Self {
        self.can_cache = can_cache;
        self
    }

    #[must_use]
    pub const fn with_needs_group_signal(mut self, needs_group_signal: bool) -> Self {
        self.needs_group_signal = needs_group_signal;
        self
    }
}

impl Default for NodeProperties {
    fn default() -> Self {
        Self::new(Linearity::Linear)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ModuleProperties {
    node: NodeProperties,
    realtime_safe: bool,
    deterministic: bool,
}

impl ModuleProperties {
    #[must_use]
    pub const fn new(node: NodeProperties) -> Self {
        Self {
            node,
            realtime_safe: true,
            deterministic: true,
        }
    }

    #[must_use]
    pub const fn node(self) -> NodeProperties {
        self.node
    }

    #[must_use]
    pub const fn realtime_safe(self) -> bool {
        self.realtime_safe
    }

    #[must_use]
    pub const fn deterministic(self) -> bool {
        self.deterministic
    }

    #[must_use]
    pub const fn with_realtime_safe(mut self, realtime_safe: bool) -> Self {
        self.realtime_safe = realtime_safe;
        self
    }

    #[must_use]
    pub const fn with_deterministic(mut self, deterministic: bool) -> Self {
        self.deterministic = deterministic;
        self
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PortKind {
    AudioIn,
    AudioOut,
    GateIn,
    GateOut,
    TriggerIn,
    TriggerOut,
    ControlIn,
    ControlOut,
    EventIn,
    EventOut,
    FeatureIn,
    FeatureOut,
    MetadataIn,
    MetadataOut,
}

impl PortKind {
    #[must_use]
    pub const fn is_input(self) -> bool {
        matches!(
            self,
            Self::AudioIn
                | Self::GateIn
                | Self::TriggerIn
                | Self::ControlIn
                | Self::EventIn
                | Self::FeatureIn
                | Self::MetadataIn
        )
    }

    #[must_use]
    pub const fn is_output(self) -> bool {
        !self.is_input()
    }

    #[must_use]
    pub const fn accepts_from(self, source: Self) -> bool {
        matches!(
            (source, self),
            (Self::AudioOut, Self::AudioIn)
                | (Self::GateOut, Self::GateIn)
                | (Self::TriggerOut, Self::TriggerIn)
                | (Self::ControlOut, Self::ControlIn)
                | (Self::EventOut, Self::EventIn)
                | (Self::FeatureOut, Self::FeatureIn)
                | (Self::MetadataOut, Self::MetadataIn)
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PortRate {
    Audio,
    Block,
    Step,
    Beat,
    Event,
    Scene,
    Manual,
    Offline,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Port {
    id: PortId,
    node: NodeId,
    kind: PortKind,
    rate: PortRate,
}

impl Port {
    #[must_use]
    pub const fn new(id: PortId, node: NodeId, kind: PortKind, rate: PortRate) -> Self {
        Self {
            id,
            node,
            kind,
            rate,
        }
    }

    #[must_use]
    pub const fn id(self) -> PortId {
        self.id
    }

    #[must_use]
    pub const fn node(self) -> NodeId {
        self.node
    }

    #[must_use]
    pub const fn kind(self) -> PortKind {
        self.kind
    }

    #[must_use]
    pub const fn rate(self) -> PortRate {
        self.rate
    }

    #[must_use]
    pub const fn can_connect_to(self, target: Self) -> bool {
        self.kind.is_output() && target.kind.accepts_from(self.kind)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Diagnostic {
    message: String,
}

impl Diagnostic {
    #[must_use]
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirtyRange {
    entity: EntityId,
    range: FrameRange,
}

impl DirtyRange {
    #[must_use]
    pub const fn new(entity: EntityId, range: FrameRange) -> Self {
        Self { entity, range }
    }

    #[must_use]
    pub const fn entity(&self) -> EntityId {
        self.entity
    }

    #[must_use]
    pub const fn range(&self) -> FrameRange {
        self.range
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Command {
    AddSource {
        source: SourceId,
    },
    AddNode {
        node: NodeId,
        properties: NodeProperties,
    },
    SetRelation {
        relation: RelationEdge,
    },
    ClearRelation {
        relation: RelationId,
    },
    MarkDirty {
        node: NodeId,
        range: FrameRange,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Default)]
pub struct CommandResult {
    changed_entities: Vec<EntityId>,
    dirty_ranges: Vec<DirtyRange>,
    diagnostics: Vec<Diagnostic>,
}

impl CommandResult {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn changed_entities(&self) -> &[EntityId] {
        &self.changed_entities
    }

    #[must_use]
    pub fn dirty_ranges(&self) -> &[DirtyRange] {
        &self.dirty_ranges
    }

    #[must_use]
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    pub fn push_changed_entity(&mut self, entity: EntityId) {
        self.changed_entities.push(entity);
    }

    pub fn push_dirty_range(&mut self, dirty_range: DirtyRange) {
        self.dirty_ranges.push(dirty_range);
    }

    pub fn push_diagnostic(&mut self, diagnostic: Diagnostic) {
        self.diagnostics.push(diagnostic);
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Source {
    id: SourceId,
    node: NodeId,
}

impl Source {
    #[must_use]
    pub const fn new(id: SourceId, node: NodeId) -> Self {
        Self { id, node }
    }

    #[must_use]
    pub const fn id(&self) -> SourceId {
        self.id
    }

    #[must_use]
    pub const fn node(&self) -> NodeId {
        self.node
    }
}

#[derive(Clone, Debug, Default)]
pub struct SourceGraph {
    sources: BTreeMap<SourceId, Source>,
}

impl SourceGraph {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, source: Source) -> Option<Source> {
        self.sources.insert(source.id(), source)
    }

    #[must_use]
    pub fn get(&self, source: SourceId) -> Option<&Source> {
        self.sources.get(&source)
    }

    #[must_use]
    pub fn contains(&self, source: SourceId) -> bool {
        self.sources.contains_key(&source)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.sources.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.sources.is_empty()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RelationEdge {
    id: RelationId,
    from: NodeId,
    to: NodeId,
    kind: EdgeKind,
}

impl RelationEdge {
    #[must_use]
    pub const fn new(id: RelationId, from: NodeId, to: NodeId, kind: EdgeKind) -> Self {
        Self { id, from, to, kind }
    }

    #[must_use]
    pub const fn id(&self) -> RelationId {
        self.id
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
    pub const fn kind(&self) -> EdgeKind {
        self.kind
    }
}

#[derive(Clone, Debug, Default)]
pub struct RelationGraph {
    nodes: BTreeMap<NodeId, NodeProperties>,
    ports: BTreeMap<PortId, Port>,
    edges: BTreeMap<RelationId, RelationEdge>,
    outgoing: BTreeMap<NodeId, BTreeSet<RelationId>>,
}

impl RelationGraph {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert_node(
        &mut self,
        node: NodeId,
        properties: NodeProperties,
    ) -> Option<NodeProperties> {
        self.nodes.insert(node, properties)
    }

    pub fn insert_port(&mut self, port: Port) -> Result<Option<Port>, RelationGraphError> {
        if !self.nodes.contains_key(&port.node()) {
            return Err(RelationGraphError::UnknownNode(port.node()));
        }

        Ok(self.ports.insert(port.id(), port))
    }

    pub fn insert_edge(
        &mut self,
        edge: RelationEdge,
    ) -> Result<Option<RelationEdge>, RelationGraphError> {
        if !self.nodes.contains_key(&edge.from()) {
            return Err(RelationGraphError::UnknownNode(edge.from()));
        }
        if !self.nodes.contains_key(&edge.to()) {
            return Err(RelationGraphError::UnknownNode(edge.to()));
        }
        if self.path_exists(edge.to(), edge.from()) {
            return Err(RelationGraphError::CycleDetected {
                from: edge.from(),
                to: edge.to(),
            });
        }

        let previous = self.edges.insert(edge.id(), edge.clone());
        if let Some(previous) = &previous {
            if let Some(ids) = self.outgoing.get_mut(&previous.from()) {
                ids.remove(&previous.id());
            }
        }
        self.outgoing
            .entry(edge.from())
            .or_default()
            .insert(edge.id());

        Ok(previous)
    }

    #[must_use]
    pub fn node_properties(&self, node: NodeId) -> Option<NodeProperties> {
        self.nodes.get(&node).copied()
    }

    #[must_use]
    pub fn port(&self, port: PortId) -> Option<Port> {
        self.ports.get(&port).copied()
    }

    #[must_use]
    pub fn edge(&self, relation: RelationId) -> Option<&RelationEdge> {
        self.edges.get(&relation)
    }

    #[must_use]
    pub fn len_nodes(&self) -> usize {
        self.nodes.len()
    }

    #[must_use]
    pub fn len_edges(&self) -> usize {
        self.edges.len()
    }

    #[must_use]
    pub fn invalidate_from(&self, changed_node: NodeId, range: FrameRange) -> Vec<DirtyRange> {
        self.invalidate_many([(changed_node, range)])
    }

    #[must_use]
    pub fn invalidate_many(
        &self,
        changed: impl IntoIterator<Item = (NodeId, FrameRange)>,
    ) -> Vec<DirtyRange> {
        let mut dirty_by_node = BTreeMap::<NodeId, FrameRange>::new();
        let mut queue = VecDeque::<NodeId>::new();

        for (node, range) in changed {
            let inserted = merge_dirty_range(&mut dirty_by_node, node, range);
            if inserted {
                queue.push_back(node);
            }
        }

        while let Some(node) = queue.pop_front() {
            let Some(range) = dirty_by_node.get(&node).copied() else {
                continue;
            };
            let Some(edge_ids) = self.outgoing.get(&node) else {
                continue;
            };

            for edge_id in edge_ids {
                let edge = &self.edges[edge_id];
                let target_properties = self.node_properties(edge.to()).unwrap_or_default();
                let target_range = range.expand_saturating(
                    target_properties.lookahead_frames(),
                    target_properties.tail_frames(),
                );

                if merge_dirty_range(&mut dirty_by_node, edge.to(), target_range) {
                    queue.push_back(edge.to());
                }
            }
        }

        dirty_by_node
            .into_iter()
            .map(|(node, range)| DirtyRange::new(EntityId::Node(node), range))
            .collect()
    }

    fn path_exists(&self, start: NodeId, target: NodeId) -> bool {
        let mut seen = BTreeSet::<NodeId>::new();
        let mut queue = VecDeque::from([start]);

        while let Some(node) = queue.pop_front() {
            if node == target {
                return true;
            }
            if !seen.insert(node) {
                continue;
            }
            if let Some(edge_ids) = self.outgoing.get(&node) {
                for edge_id in edge_ids {
                    queue.push_back(self.edges[edge_id].to());
                }
            }
        }

        false
    }
}

fn merge_dirty_range(
    dirty_by_node: &mut BTreeMap<NodeId, FrameRange>,
    node: NodeId,
    range: FrameRange,
) -> bool {
    match dirty_by_node.get_mut(&node) {
        Some(existing) if existing.contains_range(range) => false,
        Some(existing) => {
            *existing = existing.union(range);
            true
        }
        None => {
            dirty_by_node.insert(node, range);
            true
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RelationGraphError {
    CycleDetected { from: NodeId, to: NodeId },
    UnknownNode(NodeId),
}

impl fmt::Display for RelationGraphError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CycleDetected { from, to } => {
                write!(
                    f,
                    "relation from node {} to node {} would create a cycle",
                    from.raw(),
                    to.raw()
                )
            }
            Self::UnknownNode(node) => write!(f, "unknown node {}", node.raw()),
        }
    }
}

impl std::error::Error for RelationGraphError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn range(start: Frame, end: Frame) -> FrameRange {
        FrameRange::new(start, end).unwrap()
    }

    fn linear_node() -> NodeProperties {
        NodeProperties::new(Linearity::Linear)
    }

    #[test]
    fn typed_ids_are_distinct_and_ordered() {
        let source = SourceId::new(7);
        let node = NodeId::new(7);

        assert_eq!(source.raw(), node.raw());
        assert_eq!(NodeId::new(1), NodeId::new(1));
        assert!(NodeId::new(1) < NodeId::new(2));
    }

    #[test]
    fn frame_range_rejects_start_after_end() {
        let err = FrameRange::new(12, 8).unwrap_err();

        assert_eq!(err, FrameRangeError::StartAfterEnd { start: 12, end: 8 });
    }

    #[test]
    fn frame_range_uses_half_open_bounds() {
        let frames = range(10, 20);

        assert!(!frames.contains_frame(9));
        assert!(frames.contains_frame(10));
        assert!(frames.contains_frame(19));
        assert!(!frames.contains_frame(20));
        assert!(frames.overlaps(range(19, 25)));
        assert!(!frames.overlaps(range(20, 25)));
    }

    #[test]
    fn frame_range_expands_without_underflow() {
        assert_eq!(
            range(3, 10).expand_saturating(8, 4),
            FrameRange::new(0, 14).unwrap()
        );
    }

    #[test]
    fn ports_require_matching_output_to_input_kinds() {
        let node_a = NodeId::new(1);
        let node_b = NodeId::new(2);
        let audio_out = Port::new(PortId::new(1), node_a, PortKind::AudioOut, PortRate::Audio);
        let audio_in = Port::new(PortId::new(2), node_b, PortKind::AudioIn, PortRate::Audio);
        let control_in = Port::new(PortId::new(3), node_b, PortKind::ControlIn, PortRate::Block);

        assert!(audio_out.can_connect_to(audio_in));
        assert!(!audio_out.can_connect_to(control_in));
        assert!(!audio_in.can_connect_to(audio_out));
    }

    #[test]
    fn source_graph_stores_sources_by_typed_id() {
        let mut graph = SourceGraph::new();
        let source = Source::new(SourceId::new(4), NodeId::new(9));

        assert!(graph.is_empty());
        assert_eq!(graph.insert(source.clone()), None);
        assert_eq!(graph.get(source.id()), Some(&source));
        assert!(graph.contains(source.id()));
        assert_eq!(graph.len(), 1);
    }

    #[test]
    fn audio_edge_invalidation_reaches_downstream_node() {
        let mut graph = RelationGraph::new();
        graph.insert_node(NodeId::new(1), linear_node());
        graph.insert_node(NodeId::new(2), linear_node());
        graph
            .insert_edge(RelationEdge::new(
                RelationId::new(1),
                NodeId::new(1),
                NodeId::new(2),
                EdgeKind::Audio,
            ))
            .unwrap();

        let dirty = graph.invalidate_from(NodeId::new(1), range(100, 200));

        assert_eq!(
            dirty,
            vec![
                DirtyRange::new(EntityId::Node(NodeId::new(1)), range(100, 200)),
                DirtyRange::new(EntityId::Node(NodeId::new(2)), range(100, 200)),
            ]
        );
    }

    #[test]
    fn control_edge_invalidation_reaches_controlled_target() {
        let mut graph = RelationGraph::new();
        let kick = NodeId::new(1);
        let bass = NodeId::new(2);
        graph.insert_node(kick, linear_node());
        graph.insert_node(bass, linear_node());
        graph
            .insert_edge(RelationEdge::new(
                RelationId::new(1),
                kick,
                bass,
                EdgeKind::Control,
            ))
            .unwrap();

        let dirty = graph.invalidate_from(kick, range(64, 128));

        assert_eq!(
            dirty,
            vec![
                DirtyRange::new(EntityId::Node(kick), range(64, 128)),
                DirtyRange::new(EntityId::Node(bass), range(64, 128)),
            ]
        );
    }

    #[test]
    fn unrelated_branches_remain_clean() {
        let mut graph = RelationGraph::new();
        let changed = NodeId::new(1);
        let downstream = NodeId::new(2);
        let unrelated = NodeId::new(3);
        graph.insert_node(changed, linear_node());
        graph.insert_node(downstream, linear_node());
        graph.insert_node(unrelated, linear_node());
        graph
            .insert_edge(RelationEdge::new(
                RelationId::new(1),
                changed,
                downstream,
                EdgeKind::Audio,
            ))
            .unwrap();

        let dirty = graph.invalidate_from(changed, range(0, 16));

        assert_eq!(dirty.len(), 2);
        assert!(dirty
            .iter()
            .all(|dirty| dirty.entity() != EntityId::Node(unrelated)));
    }

    #[test]
    fn downstream_tail_expands_dirty_end() {
        let mut graph = RelationGraph::new();
        let source = NodeId::new(1);
        let reverb = NodeId::new(2);
        graph.insert_node(source, linear_node());
        graph.insert_node(reverb, linear_node().with_tail_frames(48));
        graph
            .insert_edge(RelationEdge::new(
                RelationId::new(1),
                source,
                reverb,
                EdgeKind::Audio,
            ))
            .unwrap();

        let dirty = graph.invalidate_from(source, range(100, 200));

        assert_eq!(
            dirty,
            vec![
                DirtyRange::new(EntityId::Node(source), range(100, 200)),
                DirtyRange::new(EntityId::Node(reverb), range(100, 248)),
            ]
        );
    }

    #[test]
    fn downstream_lookahead_expands_dirty_start() {
        let mut graph = RelationGraph::new();
        let source = NodeId::new(1);
        let lookahead_limiter = NodeId::new(2);
        graph.insert_node(source, linear_node());
        graph.insert_node(lookahead_limiter, linear_node().with_lookahead_frames(24));
        graph
            .insert_edge(RelationEdge::new(
                RelationId::new(1),
                source,
                lookahead_limiter,
                EdgeKind::Audio,
            ))
            .unwrap();

        let dirty = graph.invalidate_from(source, range(10, 40));

        assert_eq!(
            dirty,
            vec![
                DirtyRange::new(EntityId::Node(source), range(10, 40)),
                DirtyRange::new(EntityId::Node(lookahead_limiter), range(0, 40)),
            ]
        );
    }

    #[test]
    fn invalidation_is_deterministic_with_multiple_paths() {
        let mut graph = RelationGraph::new();
        let root = NodeId::new(1);
        let left = NodeId::new(2);
        let right = NodeId::new(3);
        let join = NodeId::new(4);
        for node in [root, left, right, join] {
            graph.insert_node(node, linear_node());
        }
        graph
            .insert_edge(RelationEdge::new(
                RelationId::new(1),
                root,
                right,
                EdgeKind::Audio,
            ))
            .unwrap();
        graph
            .insert_edge(RelationEdge::new(
                RelationId::new(2),
                root,
                left,
                EdgeKind::Control,
            ))
            .unwrap();
        graph
            .insert_edge(RelationEdge::new(
                RelationId::new(3),
                right,
                join,
                EdgeKind::Audio,
            ))
            .unwrap();
        graph
            .insert_edge(RelationEdge::new(
                RelationId::new(4),
                left,
                join,
                EdgeKind::Control,
            ))
            .unwrap();

        let first = graph.invalidate_from(root, range(10, 20));
        let second = graph.invalidate_from(root, range(10, 20));

        assert_eq!(first, second);
        assert_eq!(
            first,
            vec![
                DirtyRange::new(EntityId::Node(root), range(10, 20)),
                DirtyRange::new(EntityId::Node(left), range(10, 20)),
                DirtyRange::new(EntityId::Node(right), range(10, 20)),
                DirtyRange::new(EntityId::Node(join), range(10, 20)),
            ]
        );
    }

    #[test]
    fn graph_rejects_edges_with_unknown_nodes() {
        let mut graph = RelationGraph::new();
        graph.insert_node(NodeId::new(1), linear_node());

        let err = graph
            .insert_edge(RelationEdge::new(
                RelationId::new(1),
                NodeId::new(1),
                NodeId::new(2),
                EdgeKind::Audio,
            ))
            .unwrap_err();

        assert_eq!(err, RelationGraphError::UnknownNode(NodeId::new(2)));
    }

    #[test]
    fn graph_rejects_cycles() {
        let mut graph = RelationGraph::new();
        let a = NodeId::new(1);
        let b = NodeId::new(2);
        graph.insert_node(a, linear_node());
        graph.insert_node(b, linear_node());
        graph
            .insert_edge(RelationEdge::new(RelationId::new(1), a, b, EdgeKind::Audio))
            .unwrap();

        let err = graph
            .insert_edge(RelationEdge::new(
                RelationId::new(2),
                b,
                a,
                EdgeKind::Control,
            ))
            .unwrap_err();

        assert_eq!(err, RelationGraphError::CycleDetected { from: b, to: a });
    }
}
