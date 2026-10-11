use super::*;
pub use meldritch_dsl::{AuthoredPattern, NoteEdit};

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
    pub pattern: String,
    pub note: Option<String>,
    pub visible: bool,
    pub save_status: Option<String>,
}
impl AppController {
    pub fn enable_pattern_editor(&mut self, pattern: &str) -> Result<(), String> {
        self.instrument_editor
            .as_ref()
            .ok_or("no authored song source")?
            .source
            .authored_pattern(pattern)?;
        self.pattern_selection = Some(PatternSelection {
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
        self.instrument_editor.as_mut().unwrap().source = Arc::new(source);
        self.enable_pattern_editor(pattern)
    }
    pub(crate) fn pattern_view(&self) -> Option<PatternEditorView> {
        let selection = self.pattern_selection.as_ref()?;
        let source = self.instrument_editor.as_ref()?;
        Some(PatternEditorView {
            pattern: source.source.authored_pattern(&selection.pattern).ok()?,
            selected_note: selection.note.clone(),
            source: source.source.clone(),
            base_fingerprint: source.base.fingerprint(),
            visible: selection.visible,
            save_status: selection.save_status.clone(),
        })
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
            .instrument_editor
            .as_mut()
            .ok_or_else(|| AppCommandError::Instrument("no authored song source".into()))?;
        let source = owner
            .source
            .create_note_pattern(track, id, length_ticks, looped)
            .map_err(AppCommandError::Instrument)?;
        meldritch_render::song_host::CompiledSongPatch::compile_initial(&source)
            .map_err(AppCommandError::Instrument)?;
        owner.source = Arc::new(source);
        owner.visible = false;
        self.pattern_selection = Some(PatternSelection {
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
            .instrument_editor
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
        owner.visible = false;
        self.pattern_selection = Some(PatternSelection {
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
            .instrument_editor
            .as_mut()
            .ok_or_else(|| AppCommandError::Instrument("no authored song source".into()))?;
        if !owner
            .source
            .authored_pattern(pattern)
            .map_err(AppCommandError::Instrument)?
            .notes
            .iter()
            .any(|n| n.id == id)
        {
            return Err(AppCommandError::Instrument(format!(
                "unknown note '{id}' in '{pattern}'"
            )));
        }
        owner.visible = false;
        self.pattern_selection = Some(PatternSelection {
            pattern: pattern.into(),
            note: Some(id.into()),
            visible: true,
            save_status: None,
        });
        Ok(AppCommandResult::PatternSelectionChanged)
    }
}
