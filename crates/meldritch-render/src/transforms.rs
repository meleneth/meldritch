//! Cacheable transforms over finite audio chunks.

use crate::{Fingerprint, FingerprintBuilder};
use meldritch_audio::AudioBlock;
use meldritch_core::{
    EdgeKind, Linearity, NodeId, NodeProperties, RelationEdge, RelationGraph, RelationGraphError,
    RelationId, Source, SourceGraph, SourceId,
};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ChunkTransform {
    Reverse,
    Reslice { order: Vec<usize> },
    Freeze { frame: u32 },
    Smear { radius_frames: u32 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChunkTransformError {
    EmptyAudio,
    EmptySliceOrder,
    UnevenSlices,
    InvalidSliceOrder,
    FrameOutOfRange,
    ZeroSmearRadius,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TransformArtifactKey {
    pub fingerprint: Fingerprint,
    pub channels: u16,
    pub frames: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DerivedTransformSource {
    pub source: SourceId,
    pub node: NodeId,
    pub parent_node: NodeId,
    pub relation: RelationId,
    pub transform: ChunkTransform,
    pub artifact: TransformArtifactKey,
}

impl DerivedTransformSource {
    #[allow(clippy::too_many_arguments)]
    pub fn attach(
        sources: &mut SourceGraph,
        relations: &mut RelationGraph,
        source: SourceId,
        node: NodeId,
        parent_node: NodeId,
        relation: RelationId,
        transform: ChunkTransform,
        artifact: TransformArtifactKey,
    ) -> Result<Self, RelationGraphError> {
        relations.insert_node(node, NodeProperties::new(Linearity::TimeVariant));
        relations.insert_edge(RelationEdge::new(
            relation,
            parent_node,
            node,
            EdgeKind::Audio,
        ))?;
        sources.insert(Source::new(source, node));
        Ok(Self {
            source,
            node,
            parent_node,
            relation,
            transform,
            artifact,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransformCacheStatus {
    Hit,
    Miss,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CachedTransform {
    pub key: TransformArtifactKey,
    pub block: AudioBlock,
    pub status: TransformCacheStatus,
}

#[derive(Default)]
pub struct TransformArtifactCache {
    artifacts: BTreeMap<TransformArtifactKey, AudioBlock>,
}

impl TransformArtifactCache {
    #[must_use]
    pub fn len(&self) -> usize {
        self.artifacts.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.artifacts.is_empty()
    }

    pub fn render(
        &mut self,
        input: &AudioBlock,
        transform: &ChunkTransform,
    ) -> Result<CachedTransform, ChunkTransformError> {
        let key = transform_artifact_key(input, transform);
        if let Some(block) = self.artifacts.get(&key) {
            return Ok(CachedTransform {
                key,
                block: block.clone(),
                status: TransformCacheStatus::Hit,
            });
        }
        let block = transform_chunk(input, transform)?;
        self.artifacts.insert(key, block.clone());
        Ok(CachedTransform {
            key,
            block,
            status: TransformCacheStatus::Miss,
        })
    }
}

pub fn transform_chunk(
    input: &AudioBlock,
    transform: &ChunkTransform,
) -> Result<AudioBlock, ChunkTransformError> {
    if input.frames() == 0 {
        return Err(ChunkTransformError::EmptyAudio);
    }
    match transform {
        ChunkTransform::Reverse => Ok(reverse(input)),
        ChunkTransform::Reslice { order } => reslice(input, order),
        ChunkTransform::Freeze { frame } => freeze(input, *frame),
        ChunkTransform::Smear { radius_frames } => smear(input, *radius_frames),
    }
}

#[must_use]
pub fn transform_artifact_key(
    input: &AudioBlock,
    transform: &ChunkTransform,
) -> TransformArtifactKey {
    let mut state = FingerprintBuilder::new();
    state.write_u64(u64::from(input.channels()));
    state.write_u64(u64::from(input.frames()));
    for sample in input.samples() {
        state.write_u64(sample.to_bits());
    }
    match transform {
        ChunkTransform::Reverse => state.write_u64(0),
        ChunkTransform::Reslice { order } => {
            state.write_u64(1);
            state.write_u64(order.len() as u64);
            for index in order {
                state.write_u64(*index as u64);
            }
        }
        ChunkTransform::Freeze { frame } => {
            state.write_u64(2);
            state.write_u64(u64::from(*frame));
        }
        ChunkTransform::Smear { radius_frames } => {
            state.write_u64(3);
            state.write_u64(u64::from(*radius_frames));
        }
    }
    TransformArtifactKey {
        fingerprint: state.finish(),
        channels: input.channels(),
        frames: input.frames(),
    }
}

fn reverse(input: &AudioBlock) -> AudioBlock {
    let channels = usize::from(input.channels());
    let mut output = AudioBlock::silent(input.channels(), input.frames());
    for frame in 0..input.frames() {
        let source = (input.frames() - 1 - frame) as usize * channels;
        let target = frame as usize * channels;
        output.samples_mut()[target..target + channels]
            .copy_from_slice(&input.samples()[source..source + channels]);
    }
    output
}

fn reslice(input: &AudioBlock, order: &[usize]) -> Result<AudioBlock, ChunkTransformError> {
    if order.is_empty() {
        return Err(ChunkTransformError::EmptySliceOrder);
    }
    if !(input.frames() as usize).is_multiple_of(order.len()) {
        return Err(ChunkTransformError::UnevenSlices);
    }
    let mut sorted = order.to_vec();
    sorted.sort_unstable();
    if sorted != (0..order.len()).collect::<Vec<_>>() {
        return Err(ChunkTransformError::InvalidSliceOrder);
    }
    let channels = usize::from(input.channels());
    let slice_frames = input.frames() as usize / order.len();
    let slice_samples = slice_frames * channels;
    let mut output = AudioBlock::silent(input.channels(), input.frames());
    for (target_slice, source_slice) in order.iter().copied().enumerate() {
        let source = source_slice * slice_samples;
        let target = target_slice * slice_samples;
        output.samples_mut()[target..target + slice_samples]
            .copy_from_slice(&input.samples()[source..source + slice_samples]);
    }
    Ok(output)
}

fn freeze(input: &AudioBlock, frame: u32) -> Result<AudioBlock, ChunkTransformError> {
    if frame >= input.frames() {
        return Err(ChunkTransformError::FrameOutOfRange);
    }
    let channels = usize::from(input.channels());
    let source = frame as usize * channels;
    let frozen = &input.samples()[source..source + channels];
    let mut output = AudioBlock::silent(input.channels(), input.frames());
    for target in output.samples_mut().chunks_exact_mut(channels) {
        target.copy_from_slice(frozen);
    }
    Ok(output)
}

fn smear(input: &AudioBlock, radius: u32) -> Result<AudioBlock, ChunkTransformError> {
    if radius == 0 {
        return Err(ChunkTransformError::ZeroSmearRadius);
    }
    let channels = usize::from(input.channels());
    let mut output = AudioBlock::silent(input.channels(), input.frames());
    for frame in 0..input.frames() {
        let start = frame.saturating_sub(radius);
        let end = frame
            .saturating_add(radius)
            .saturating_add(1)
            .min(input.frames());
        let count = f64::from(end - start);
        for channel in 0..channels {
            let sum = (start..end).fold(0.0, |sum, source| {
                sum + input.samples()[source as usize * channels + channel]
            });
            output.samples_mut()[frame as usize * channels + channel] = sum / count;
        }
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use meldritch_core::{EntityId, FrameRange};

    fn block() -> AudioBlock {
        let mut block = AudioBlock::silent(1, 4);
        block.samples_mut().copy_from_slice(&[1.0, 2.0, 3.0, 4.0]);
        block
    }

    #[test]
    fn reverse_and_reslice_reorder_complete_frames() {
        assert_eq!(
            transform_chunk(&block(), &ChunkTransform::Reverse)
                .unwrap()
                .samples(),
            &[4.0, 3.0, 2.0, 1.0]
        );
        assert_eq!(
            transform_chunk(&block(), &ChunkTransform::Reslice { order: vec![1, 0] })
                .unwrap()
                .samples(),
            &[3.0, 4.0, 1.0, 2.0]
        );
    }

    #[test]
    fn freeze_and_smear_create_finite_derived_audio() {
        assert_eq!(
            transform_chunk(&block(), &ChunkTransform::Freeze { frame: 2 })
                .unwrap()
                .samples(),
            &[3.0; 4]
        );
        assert_eq!(
            transform_chunk(&block(), &ChunkTransform::Smear { radius_frames: 1 })
                .unwrap()
                .samples(),
            &[1.5, 2.0, 3.0, 3.5]
        );
    }

    #[test]
    fn invalid_specs_are_rejected() {
        assert_eq!(
            transform_chunk(&block(), &ChunkTransform::Reslice { order: vec![0, 0] }),
            Err(ChunkTransformError::InvalidSliceOrder)
        );
        assert_eq!(
            transform_chunk(&block(), &ChunkTransform::Freeze { frame: 4 }),
            Err(ChunkTransformError::FrameOutOfRange)
        );
    }

    #[test]
    fn artifact_keys_track_source_audio_and_transform_parameters() {
        let input = block();
        let reverse = transform_artifact_key(&input, &ChunkTransform::Reverse);
        let freeze = transform_artifact_key(&input, &ChunkTransform::Freeze { frame: 1 });
        let mut changed = input.clone();
        changed.samples_mut()[0] = 9.0;
        assert_ne!(reverse, freeze);
        assert_ne!(
            reverse,
            transform_artifact_key(&changed, &ChunkTransform::Reverse)
        );
        assert_eq!(
            reverse,
            transform_artifact_key(&input, &ChunkTransform::Reverse)
        );
    }

    #[test]
    fn transformed_artifacts_cache_and_attach_as_derived_sources() {
        let input = block();
        let transform = ChunkTransform::Reverse;
        let mut cache = TransformArtifactCache::default();
        let first = cache.render(&input, &transform).unwrap();
        let second = cache.render(&input, &transform).unwrap();
        assert_eq!(first.status, TransformCacheStatus::Miss);
        assert_eq!(second.status, TransformCacheStatus::Hit);
        assert_eq!(cache.len(), 1);

        let parent = NodeId::new(1);
        let derived = NodeId::new(2);
        let mut sources = SourceGraph::new();
        let mut relations = RelationGraph::new();
        relations.insert_node(parent, NodeProperties::default());
        let source = DerivedTransformSource::attach(
            &mut sources,
            &mut relations,
            SourceId::new(2),
            derived,
            parent,
            RelationId::new(1),
            transform,
            first.key,
        )
        .unwrap();
        assert!(sources.contains(source.source));
        assert_eq!(relations.len_edges(), 1);
        let dirty = relations.invalidate_from(parent, FrameRange::new(0, 4).unwrap());
        assert!(dirty.iter().any(|dirty| {
            dirty.entity() == EntityId::Node(derived)
                && dirty.range() == FrameRange::new(0, 4).unwrap()
        }));
    }
}
