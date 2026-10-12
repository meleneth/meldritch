use super::*;
pub use meldritch_dsl::{AuthoredPattern, NoteEdit, PatternMaterial, note_name};

#[derive(Clone, Debug)]
pub struct PalettePatternEditorView {
    pub pattern: PatternMaterial,
    pub selected_note: Option<String>,
    pub source: Arc<ValidatedSong>,
    pub visible: bool,
    pub cursor_tick: u64,
    pub cursor_pitch: u8,
}

#[derive(Clone, Debug)]
pub struct PatternEditorView {
    pub pattern: AuthoredPattern,
    pub selected_note: Option<String>,
    pub source: Arc<ValidatedSong>,
    pub base_fingerprint: u64,
    pub visible: bool,
    pub save_status: Option<String>,
}
pub(crate) struct PatternSelection {
    pub cursor_tick: u64,
    pub cursor_pitch: u8,
    pub pattern: String,
    pub note: Option<String>,
    pub visible: bool,
    pub save_status: Option<String>,
}
impl AppController {
    pub(crate) fn pattern_dirty_tracks(
        &self,
        pattern: &str,
        ranges: &[FrameRange],
    ) -> Vec<DirtyRange> {
        let owner = self.project_source.as_ref().unwrap();
        let mut dirty: Vec<_> = owner
            .source
            .performance()
            .tracks()
            .iter()
            .enumerate()
            .filter(|(_, track)| track.pattern_ids().iter().any(|id| id == pattern))
            .flat_map(|(index, _)| {
                ranges.iter().map(move |range| {
                    DirtyRange::new(EntityId::Track(TrackId::new(index as u64 + 1)), *range)
                })
            })
            .collect();
        let offset = owner.source.performance().tracks().len();
        for (index, voice) in owner.source.pattern_voices().values().enumerate() {
            if voice.pitch_pattern.as_deref() == Some(pattern)
                || voice.gate_pattern.as_deref() == Some(pattern)
            {
                dirty.extend(ranges.iter().map(|range| {
                    DirtyRange::new(
                        EntityId::Track(TrackId::new((offset + index + 1) as u64)),
                        *range,
                    )
                }));
            }
        }
        dirty
    }

    pub fn enable_pattern_editor(&mut self, pattern: &str) -> Result<(), String> {
        self.project_source
            .as_ref()
            .ok_or("no authored song source")?
            .source
            .pattern_material(pattern)?;
        self.pattern_selection = Some(PatternSelection {
            cursor_tick: 0,
            cursor_pitch: 48,
            pattern: pattern.into(),
            note: None,
            visible: false,
            save_status: None,
        });
        Ok(())
    }
    pub fn enable_pattern_authoring_from(
        &mut self,
        base: ValidatedSong,
        source: ValidatedSong,
        pattern: &str,
    ) -> Result<(), String> {
        let definition = source.authored_pattern(pattern)?;
        if base.restore_authored_pattern(&definition)?.fingerprint() != source.fingerprint() {
            return Err("pattern reload changed unrelated definitions".into());
        }
        self.enable_instrument_editor(base)?;
        meldritch_render::song_host::CompiledSongPatch::compile_initial(&source)?;
        self.project_source.as_mut().unwrap().source = Arc::new(source);
        self.enable_pattern_editor(pattern)
    }
    pub(crate) fn pattern_view(&self) -> Option<PatternEditorView> {
        let selection = self.pattern_selection.as_ref()?;
        let source = self.project_source.as_ref()?;
        Some(PatternEditorView {
            pattern: source.source.authored_pattern(&selection.pattern).ok()?,
            selected_note: selection.note.clone(),
            source: source.source.clone(),
            base_fingerprint: source.base.fingerprint(),
            visible: selection.visible,
            save_status: selection.save_status.clone(),
        })
    }

    pub(crate) fn palette_pattern_view(&self) -> Option<PalettePatternEditorView> {
        let selection = self.pattern_selection.as_ref()?;
        let owner = self.project_source.as_ref()?;
        if owner
            .source
            .performance()
            .tracks()
            .iter()
            .any(|t| t.pattern_ids().contains(&selection.pattern))
        {
            return None;
        }
        Some(PalettePatternEditorView {
            pattern: owner.source.pattern_material(&selection.pattern).ok()?,
            selected_note: selection.note.clone(),
            source: owner.source.clone(),
            visible: selection.visible,
            cursor_tick: selection.cursor_tick,
            cursor_pitch: selection.cursor_pitch,
        })
    }

    pub(crate) fn create_palette_pattern(
        &mut self,
        id: &str,
        length_ticks: u64,
        looped: bool,
    ) -> Result<AppCommandResult, AppCommandError> {
        let owner = self
            .project_source
            .as_ref()
            .ok_or_else(|| AppCommandError::Instrument("no authored project source".into()))?;
        let next = owner
            .source
            .create_palette_pattern(id, length_ticks, looped)
            .map_err(AppCommandError::Instrument)?;
        meldritch_render::song_host::CompiledSongPatch::compile_initial(&next)
            .map_err(AppCommandError::Instrument)?;
        self.project_source.as_mut().unwrap().source = Arc::new(next);
        self.enable_pattern_editor(id)
            .map_err(AppCommandError::Instrument)?;
        self.pattern_selection.as_mut().unwrap().visible = true;
        if let Some(editor) = self.instrument_editor.as_mut() {
            editor.visible = false;
        }
        Ok(AppCommandResult::PatternEdited {
            pattern: id.into(),
            selected_note: None,
            changed: true,
            dirty_ranges: vec![],
        })
    }

    pub(crate) fn select_palette_pattern(
        &mut self,
        id: &str,
    ) -> Result<AppCommandResult, AppCommandError> {
        let source = self
            .project_source
            .as_ref()
            .ok_or_else(|| AppCommandError::Instrument("no authored project source".into()))?;
        source
            .source
            .pattern_material(id)
            .map_err(AppCommandError::Instrument)?;
        if source
            .source
            .performance()
            .tracks()
            .iter()
            .any(|track| track.pattern_ids().iter().any(|pattern| pattern == id))
        {
            return Err(AppCommandError::Instrument(
                "palette pattern required".into(),
            ));
        }
        if self
            .pattern_selection
            .as_ref()
            .is_some_and(|selection| selection.pattern == id)
        {
            return Ok(AppCommandResult::PatternSelectionChanged);
        }
        let visible = self
            .pattern_selection
            .as_ref()
            .is_some_and(|selection| selection.visible);
        self.enable_pattern_editor(id)
            .map_err(AppCommandError::Instrument)?;
        self.pattern_selection.as_mut().unwrap().visible = visible;
        Ok(AppCommandResult::PatternSelectionChanged)
    }

    pub(crate) fn move_pattern_cursor(
        &mut self,
        tick_delta: i64,
        pitch_delta: i32,
    ) -> Result<AppCommandResult, AppCommandError> {
        let view = self
            .palette_pattern_view()
            .ok_or_else(|| AppCommandError::Instrument("no selected palette pattern".into()))?;
        let selection = self.pattern_selection.as_mut().unwrap();
        selection.cursor_tick = selection
            .cursor_tick
            .saturating_add_signed(tick_delta)
            .min(view.pattern.length_ticks.saturating_sub(1));
        selection.cursor_pitch =
            (i64::from(selection.cursor_pitch) + i64::from(pitch_delta)).clamp(0, 127) as u8;
        selection.note = None;
        Ok(AppCommandResult::PatternSelectionChanged)
    }
    pub fn observe_pattern_save(&mut self, message: String) {
        if let Some(selection) = self.pattern_selection.as_mut() {
            selection.save_status = Some(message);
        }
    }
    pub(crate) fn create_pattern(
        &mut self,
        track: &str,
        id: &str,
        length_ticks: u64,
        looped: bool,
    ) -> Result<AppCommandResult, AppCommandError> {
        let owner = self
            .project_source
            .as_mut()
            .ok_or_else(|| AppCommandError::Instrument("no authored song source".into()))?;
        let source = owner
            .source
            .create_note_pattern(track, id, length_ticks, looped)
            .map_err(AppCommandError::Instrument)?;
        meldritch_render::song_host::CompiledSongPatch::compile_initial(&source)
            .map_err(AppCommandError::Instrument)?;
        owner.source = Arc::new(source);
        if let Some(editor) = self.instrument_editor.as_mut() {
            editor.visible = false;
        }
        self.pattern_selection = Some(PatternSelection {
            cursor_tick: 0,
            cursor_pitch: 48,
            pattern: id.into(),
            note: None,
            visible: true,
            save_status: None,
        });
        Ok(AppCommandResult::PatternEdited {
            pattern: id.into(),
            selected_note: None,
            changed: true,
            dirty_ranges: vec![FrameRange::new(0, self.editor.timeline_frames()).unwrap()],
        })
    }
    pub(crate) fn edit_note(
        &mut self,
        pattern: &str,
        edit: &NoteEdit,
    ) -> Result<AppCommandResult, AppCommandError> {
        let owner = self
            .project_source
            .as_mut()
            .ok_or_else(|| AppCommandError::Instrument("no authored song source".into()))?;
        let (next, selected_note, earliest) = owner
            .source
            .edit_note(pattern, edit)
            .map_err(AppCommandError::Instrument)?;
        meldritch_render::song_host::CompiledSongPatch::compile_initial(&next)
            .map_err(AppCommandError::Instrument)?;
        let changed = next.fingerprint() != owner.source.fingerprint();
        let horizon = self.editor.timeline_frames();
        // Voice allocation, filter state, tails and later loops can affect all
        // following samples; no sample before the first changed onset is affected.
        let dirty_ranges = if changed && earliest < horizon {
            vec![FrameRange::new(earliest, horizon).unwrap()]
        } else {
            Vec::new()
        };
        owner.source = Arc::new(next);
        if let Some(editor) = self.instrument_editor.as_mut() {
            editor.visible = false;
        }
        self.pattern_selection = Some(PatternSelection {
            cursor_tick: self.pattern_selection.as_ref().map_or(0, |s| s.cursor_tick),
            cursor_pitch: self
                .pattern_selection
                .as_ref()
                .map_or(48, |s| s.cursor_pitch),
            pattern: pattern.into(),
            note: selected_note.clone(),
            visible: true,
            save_status: None,
        });
        Ok(AppCommandResult::PatternEdited {
            pattern: pattern.into(),
            selected_note,
            changed,
            dirty_ranges,
        })
    }
    pub(crate) fn select_note(
        &mut self,
        pattern: &str,
        id: &str,
    ) -> Result<AppCommandResult, AppCommandError> {
        let owner = self
            .project_source
            .as_mut()
            .ok_or_else(|| AppCommandError::Instrument("no authored song source".into()))?;
        if !owner
            .source
            .pattern_material(pattern)
            .map_err(AppCommandError::Instrument)?
            .notes
            .iter()
            .any(|n| n.id == id)
        {
            return Err(AppCommandError::Instrument(format!(
                "unknown note '{id}' in '{pattern}'"
            )));
        }
        if let Some(editor) = self.instrument_editor.as_mut() {
            editor.visible = false;
        }
        self.pattern_selection = Some(PatternSelection {
            cursor_tick: self.pattern_selection.as_ref().map_or(0, |s| s.cursor_tick),
            cursor_pitch: self
                .pattern_selection
                .as_ref()
                .map_or(48, |s| s.cursor_pitch),
            pattern: pattern.into(),
            note: Some(id.into()),
            visible: true,
            save_status: None,
        });
        Ok(AppCommandResult::PatternSelectionChanged)
    }
}
