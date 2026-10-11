//! Source-owned note authoring. No controller grid or gesture policy lives here.
use super::*;
use serde::{Deserialize, Serialize};

pub const AUTHORING_PPQ: u64 = 960;

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum NoteEdit {
    Insert {
        start_tick: u64,
        duration_ticks: u64,
        pitch: u8,
        velocity: f64,
    },
    InsertSample {
        start_tick: u64,
        duration_ticks: u64,
        pitch: u8,
        velocity: f64,
        slot: String,
    },
    Duration {
        id: String,
        duration_ticks: u64,
    },
    Velocity {
        id: String,
        velocity: f64,
    },
    Delete {
        id: String,
    },
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AuthoredNote {
    pub id: String,
    pub note: String,
    pub start_tick: u64,
    pub duration_ticks: u64,
    pub velocity: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slot: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AuthoredPattern {
    pub id: String,
    pub track: String,
    pub length_ticks: u64,
    pub looped: bool,
    pub next_note_id: u64,
    pub notes: Vec<AuthoredNote>,
}

impl ValidatedSong {
    fn validate_authoring_slot(&self, pattern: &str, slot: Option<&str>) -> Result<(), String> {
        let owners: Vec<_> = self
            .performance
            .tracks
            .iter()
            .filter(|t| t.pattern_ids.iter().any(|p| p == pattern))
            .collect();
        let [owner] = owners.as_slice() else {
            return Err("authoring requires one track owner for the pattern".into());
        };
        if owner.sampler_id.is_none() {
            return if slot.is_some() {
                Err("sample slot edit requires a sampler track".into())
            } else {
                Ok(())
            };
        }
        let slot = slot.ok_or("sampler note authoring requires an explicit sample slot")?;
        let bank = owner
            .sample_bank_id
            .as_ref()
            .and_then(|id| self.sample_banks.get(id))
            .ok_or("sampler track has no sample bank")?;
        let definition = bank
            .slots
            .iter()
            .find(|s| s.id == slot)
            .ok_or_else(|| format!("unknown sample slot '{slot}'"))?;
        if definition.mode != SamplePlaybackMode::OneShot {
            return Err("sample authoring currently requires one-shot slots".into());
        }
        Ok(())
    }
    pub fn authoring_frame(&self, ticks: u64) -> Result<u64, String> {
        if ticks > (1_u64 << 53) - 1 {
            return Err("musical ticks exceed exact numeric range".into());
        }
        frames_from_beats(
            Path::new("authored pattern"),
            ticks as f64 / AUTHORING_PPQ as f64,
            f64::from(self.performance.sample_rate) * 60.0 / self.performance.bpm,
        )
        .map_err(|e| e.to_string())
    }
    pub fn authoring_tick(&self, frame: u64) -> Result<u64, String> {
        let ticks = (frame as f64 * self.performance.bpm * AUTHORING_PPQ as f64
            / (f64::from(self.performance.sample_rate) * 60.0))
            .round() as u64;
        if self.authoring_frame(ticks)? != frame {
            return Err("pattern time cannot be losslessly represented at 960 PPQ".into());
        }
        Ok(ticks)
    }
    pub fn authored_pattern(&self, id: &str) -> Result<AuthoredPattern, String> {
        let pattern = self
            .note_patterns
            .get(id)
            .ok_or_else(|| format!("unknown note pattern '{id}'"))?;
        let tracks: Vec<_> = self
            .performance
            .tracks
            .iter()
            .filter(|t| t.pattern_ids.iter().any(|p| p == id))
            .collect();
        if tracks.len() != 1 {
            return Err("authoring requires one track owner for the pattern".into());
        }
        let mut notes = Vec::new();
        for event in &pattern.events {
            self.validate_authoring_slot(id, event.sample_slot.as_deref())?;
            if event.sample_slice.is_some()
                || event.root_note.is_some()
                || event.pitch_semitones != 0.0
            {
                return Err(
                    "sample slice/root/transpose authoring is unsupported; refusing lossy export"
                        .into(),
                );
            }
            notes.push(AuthoredNote {
                id: event.id.clone(),
                note: note_name(event.note),
                start_tick: self.authoring_tick(event.start_frame)?,
                duration_ticks: self.authoring_tick(event.duration_frames)?,
                velocity: event.velocity,
                slot: event.sample_slot.clone(),
            });
        }
        Ok(AuthoredPattern {
            id: id.into(),
            track: tracks[0].id.clone(),
            length_ticks: self.authoring_tick(pattern.length_frames)?,
            looped: pattern.looped,
            next_note_id: pattern.next_note_id,
            notes,
        })
    }
    pub fn create_note_pattern(
        &self,
        track: &str,
        id: &str,
        length_ticks: u64,
        looped: bool,
    ) -> Result<Self, String> {
        if id.trim().is_empty()
            || id.len() > 128
            || self.note_patterns.contains_key(id)
            || self.parameter_patterns.contains_key(id)
        {
            return Err(format!("invalid or duplicate pattern identity '{id}'"));
        }
        let _owner = self
            .performance
            .tracks
            .iter()
            .find(|t| t.id == track)
            .ok_or_else(|| format!("unknown track '{track}'"))?;
        if !self.performance.lanes.is_empty() {
            return Err(
                "new pattern assignment currently requires a track without performance lanes"
                    .into(),
            );
        }
        let length_frames = self.authoring_frame(length_ticks)?;
        if length_frames == 0 {
            return Err("pattern length must be positive".into());
        }
        let mut next = self.clone();
        next.note_patterns.insert(
            id.into(),
            NotePatternDefinition {
                id: id.into(),
                length_frames,
                looped,
                next_note_id: 1,
                events: Vec::new(),
            },
        );
        let owner = next
            .performance
            .tracks
            .iter_mut()
            .find(|t| t.id == track)
            .unwrap();
        owner.pattern_ids.push(id.into());
        owner.initial_pattern = Some(id.into());
        next.refresh_authoring_fingerprint();
        Ok(next)
    }
    pub fn edit_note(
        &self,
        pattern: &str,
        edit: &NoteEdit,
    ) -> Result<(Self, Option<String>, u64), String> {
        self.authored_pattern(pattern)?; // rejects unsupported ownership/sample translations
        let mut next = self.clone();
        let definition = next.note_patterns.get_mut(pattern).unwrap();
        let (selected, earliest) = match edit {
            NoteEdit::Insert {
                start_tick,
                duration_ticks,
                pitch,
                velocity,
            }
            | NoteEdit::InsertSample {
                start_tick,
                duration_ticks,
                pitch,
                velocity,
                ..
            } => {
                let slot = match edit {
                    NoteEdit::InsertSample { slot, .. } => Some(slot.as_str()),
                    _ => None,
                };
                self.validate_authoring_slot(pattern, slot)?;
                if *pitch > 127 {
                    return Err("note pitch must be MIDI 0..127".into());
                }
                let start = self.authoring_frame(*start_tick)?;
                let duration = self.authoring_frame(*duration_ticks)?;
                validate_note(start, duration, *velocity, definition.length_frames)?;
                let id = format!("note-{}", definition.next_note_id);
                definition.next_note_id = definition
                    .next_note_id
                    .checked_add(1)
                    .filter(|next| *next <= i64::MAX as u64)
                    .ok_or("note identity exhausted")?;
                definition.events.push(NoteEventDefinition {
                    id: id.clone(),
                    start_frame: start,
                    duration_frames: duration,
                    note: *pitch,
                    velocity: *velocity,
                    sample_slot: slot.map(str::to_owned),
                    sample_slice: None,
                    root_note: None,
                    pitch_semitones: 0.0,
                });
                // Preserve authored order at equal onsets: voice allocation is semantic.
                definition.events.sort_by_key(|event| event.start_frame);
                (Some(id), start)
            }
            NoteEdit::Duration { id, .. }
            | NoteEdit::Velocity { id, .. }
            | NoteEdit::Delete { id } => {
                let index = definition
                    .events
                    .iter()
                    .position(|n| &n.id == id)
                    .ok_or_else(|| format!("unknown note identity '{id}' in '{pattern}'"))?;
                let event = &mut definition.events[index];
                let earliest = event.start_frame;
                match edit {
                    NoteEdit::Duration { duration_ticks, .. } => {
                        event.duration_frames = self.authoring_frame(*duration_ticks)?
                    }
                    NoteEdit::Velocity { velocity, .. } => event.velocity = *velocity,
                    _ => {}
                }
                validate_note(
                    event.start_frame,
                    event.duration_frames,
                    event.velocity,
                    definition.length_frames,
                )?;
                if matches!(edit, NoteEdit::Delete { .. }) {
                    definition.events.remove(index);
                    (None, earliest)
                } else {
                    (Some(id.clone()), earliest)
                }
            }
        };
        next.refresh_authoring_fingerprint();
        Ok((next, selected, earliest))
    }
    pub fn restore_authored_pattern(&self, pattern: &AuthoredPattern) -> Result<Self, String> {
        let mut next = if self.note_patterns.contains_key(&pattern.id) {
            self.clone()
        } else {
            self.create_note_pattern(
                &pattern.track,
                &pattern.id,
                pattern.length_ticks,
                pattern.looped,
            )?
        };
        let prior = next.authored_pattern(&pattern.id)?;
        if prior.track != pattern.track
            || prior.length_ticks != pattern.length_ticks
            || prior.looped != pattern.looped
        {
            return Err("unsupported pattern owner/length/loop edit".into());
        }
        let length = next.authoring_frame(pattern.length_ticks)?;
        let mut events = Vec::new();
        let mut seen = std::collections::BTreeSet::new();
        for note in &pattern.notes {
            next.validate_authoring_slot(&pattern.id, note.slot.as_deref())?;
            let serial = note
                .id
                .strip_prefix("note-")
                .and_then(|s| s.parse::<u64>().ok())
                .ok_or("invalid note identity")?;
            if serial == 0
                || serial >= pattern.next_note_id
                || !seen.insert(note.id.clone())
                || note.id != format!("note-{serial}")
            {
                return Err("duplicate note or invalid next identity".into());
            }
            let start = next.authoring_frame(note.start_tick)?;
            let duration = next.authoring_frame(note.duration_ticks)?;
            validate_note(start, duration, note.velocity, length)?;
            if events
                .last()
                .is_some_and(|e: &NoteEventDefinition| e.start_frame > start)
            {
                return Err("notes must be ordered by onset".into());
            }
            events.push(NoteEventDefinition {
                id: note.id.clone(),
                start_frame: start,
                duration_frames: duration,
                note: parse_note(Path::new("authored note"), &note.note)
                    .map_err(|e| e.to_string())?,
                velocity: note.velocity,
                sample_slot: note.slot.clone(),
                sample_slice: None,
                root_note: None,
                pitch_semitones: 0.0,
            });
        }
        if pattern.next_note_id == 0 || pattern.next_note_id > i64::MAX as u64 {
            return Err("invalid next note identity".into());
        }
        let definition = next.note_patterns.get_mut(&pattern.id).unwrap();
        definition.events = events;
        definition.next_note_id = pattern.next_note_id;
        next.refresh_authoring_fingerprint();
        Ok(next)
    }
    /// Preserve pattern edits while canonicalizing instrument defaults over import provenance.
    pub fn canonicalize_instrument_defaults(&self, base: &Self) -> Result<Self, String> {
        let mut canonical = base.clone();
        for id in self.synths.keys() {
            let graph = &self.synths[id];
            let original = base
                .synths
                .get(id)
                .ok_or_else(|| format!("unknown base synth '{id}'"))?;
            let same_topology = graph.inputs == original.inputs
                && graph.cables == original.cables
                && graph
                    .modules
                    .iter()
                    .map(|m| (&m.id, m.kind))
                    .eq(original.modules.iter().map(|m| (&m.id, m.kind)));
            let same_structural_values =
                graph.modules.iter().zip(&original.modules).all(|(a, b)| {
                    a.frequency_hz == b.frequency_hz
                        && a.channels == b.channels
                        && a.time == b.time
                        && a.feedback == b.feedback
                        && a.mix == b.mix
                });
            if !same_topology || !same_structural_values {
                canonical.synths.insert(id.clone(), graph.clone());
                continue;
            }
            for p in self.instrument_parameters(id)? {
                canonical = canonical.edit_instrument(&InstrumentEdit {
                    target: p.id,
                    value: p.value,
                })?;
            }
        }
        let mut next = self.clone();
        next.synths = canonical.synths;
        next.refresh_authoring_fingerprint();
        Ok(next)
    }
    pub(super) fn refresh_authoring_fingerprint(&mut self) {
        self.fingerprint = fingerprint_song(
            &self.performance,
            &self.synths,
            &self.samplers,
            &self.note_patterns,
            &self.parameter_patterns,
            &self.dsps,
            &self.sample_banks,
        );
    }
}

fn validate_note(start: u64, duration: u64, velocity: f64, length: u64) -> Result<(), String> {
    if duration == 0 || start.checked_add(duration).is_none_or(|end| end > length) {
        return Err("note must have positive duration and end within the pattern".into());
    }
    if !velocity.is_finite() || !(0.0..=1.0).contains(&velocity) {
        return Err("note velocity must be finite in 0..1".into());
    }
    Ok(())
}
pub fn note_name(pitch: u8) -> String {
    let names = [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];
    format!(
        "{}{}",
        names[usize::from(pitch % 12)],
        i16::from(pitch / 12) - 1
    )
}
