use super::*;
pub use meldritch_dsl::{EditableTarget, ParameterEdit};
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AssignmentTarget {
    pub id: String,
    pub label: String,
    pub target: EditableTarget,
    pub minimum: f64,
    pub maximum: f64,
    pub step: f64,
}
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FaderAssignment {
    pub id: String,
    pub device: String,
    pub channel: u8,
    pub cc: u8,
    pub page: String,
    pub target: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AssignmentDefinition {
    pub pages: Vec<String>,
    pub targets: Vec<AssignmentTarget>,
    pub faders: Vec<FaderAssignment>,
}
#[derive(Clone, Debug)]
pub struct AssignmentEditorView {
    pub definition: AssignmentDefinition,
    pub page: String,
    pub selected_fader: usize,
    pub selected_target: usize,
    pub visible: bool,
    pub observations: BTreeMap<String, (u8, bool)>,
    pub source: Arc<ValidatedSong>,
    pub base: Arc<ValidatedSong>,
    pub save_status: Option<String>,
}
pub(crate) struct AssignmentEditor {
    definition: AssignmentDefinition,
    page: String,
    selected_fader: usize,
    selected_target: usize,
    pub(crate) visible: bool,
    observations: BTreeMap<String, (u8, bool)>,
    save_status: Option<String>,
}
impl AssignmentDefinition {
    pub fn validate(&self, song: &ValidatedSong) -> Result<(), String> {
        if self.pages.is_empty()
            || self.pages.len() > 32
            || self.targets.is_empty()
            || self.targets.len() > 64
            || self.faders.len() > 128
        {
            return Err(
                "assignments require 1..32 pages, 1..64 targets and at most 128 faders".into(),
            );
        }
        fn unique<'a>(values: impl Iterator<Item = &'a str>) -> bool {
            let mut seen = BTreeSet::new();
            values
                .into_iter()
                .all(|s| !s.trim().is_empty() && s.len() <= 128 && seen.insert(s))
        }
        if !unique(self.pages.iter().map(String::as_str))
            || !unique(self.targets.iter().map(|t| t.id.as_str()))
            || !unique(self.faders.iter().map(|f| f.id.as_str()))
        {
            return Err("assignment/page/target IDs must be unique and nonempty".into());
        }
        let mut identities = BTreeSet::new();
        for t in &self.targets {
            let p = song.editable_parameter(&t.target)?;
            if !identities.insert(t.target.identity()) {
                return Err("ambiguous duplicate parameter targets in catalog".into());
            }
            if !t.minimum.is_finite()
                || !t.maximum.is_finite()
                || !t.step.is_finite()
                || t.minimum >= t.maximum
                || t.step <= 0.0
                || t.step > t.maximum - t.minimum
                || t.minimum < p.minimum
                || t.maximum > p.maximum
            {
                return Err(format!("invalid numeric binding range for {}", t.id));
            }
        }
        let mut physical = BTreeSet::new();
        if song
            .performance()
            .controls()
            .iter()
            .any(|c| c.id().starts_with("assignment:"))
        {
            return Err(
                "TOML control IDs beginning 'assignment:' are reserved for authored faders".into(),
            );
        }
        for f in &self.faders {
            let legacy = song
                .performance()
                .controls()
                .iter()
                .flat_map(|c| c.bindings())
                .chain(
                    song.performance()
                        .actions()
                        .iter()
                        .flat_map(|c| c.bindings()),
                )
                .chain(
                    song.performance()
                        .modifier_controls()
                        .iter()
                        .flat_map(|c| c.bindings()),
                )
                .chain(
                    song.performance()
                        .modifiers()
                        .iter()
                        .flat_map(|c| c.bindings()),
                );
            if legacy.into_iter().any(|b| matches!(b,meldritch_dsl::ControlBindingDefinition::MidiCc {device,channel,cc,..} if device==&f.device && *cc==f.cc && channel.is_none_or(|ch|ch==f.channel))) {
                return Err(format!("fader '{}' conflicts with an existing TOML MIDI binding; remove that duplicate declaration",f.id));
            }
            if !self.pages.contains(&f.page)
                || !(1..=16).contains(&f.channel)
                || f.cc > 127
                || !song
                    .performance()
                    .midi_devices()
                    .iter()
                    .any(|d| d.id() == f.device)
            {
                return Err(format!(
                    "fader '{}' references an undeclared page/device or invalid channel/CC",
                    f.id
                ));
            }
            if !physical.insert((&f.device, f.channel, f.cc, &f.page)) {
                return Err(format!(
                    "ambiguous physical fader '{}' on page '{}'",
                    f.id, f.page
                ));
            }
            if f.target
                .as_ref()
                .is_some_and(|id| !self.targets.iter().any(|t| t.id == *id))
            {
                return Err(format!("fader '{}' references missing target", f.id));
            }
        }
        Ok(())
    }
}
pub fn normalized_parameter_value(minimum: f64, maximum: f64, step: f64, normalized: f64) -> f64 {
    let raw = minimum + normalized.clamp(0.0, 1.0) * (maximum - minimum);
    (minimum + step * ((raw - minimum) / step).round()).clamp(minimum, maximum)
}
impl AppController {
    /// Install a composed snapshot by reconstructing its supported semantic scope.
    /// The validated Rust source remains the only musical owner.
    pub fn enable_project_authoring_from(
        &mut self,
        base: ValidatedSong,
        source: ValidatedSong,
        synth: &str,
        pattern: &str,
        definition: AssignmentDefinition,
    ) -> Result<(), String> {
        definition.validate(&source)?;
        let graph = source
            .synths()
            .get(synth)
            .ok_or("missing project synth")?
            .clone();
        let mut reconstructed = base.replace_patch(graph)?;
        reconstructed =
            reconstructed.restore_authored_pattern(&source.authored_pattern(pattern)?)?;
        for t in &definition.targets {
            if matches!(&t.target, EditableTarget::Track {id,parameter:meldritch_dsl::MixerParameter::Pan}
                if source.performance().tracks().iter().find(|t|t.id()==id).unwrap().pan().is_none())
            {
                continue;
            }
            reconstructed = reconstructed.edit_parameter(&ParameterEdit {
                target: t.target.clone(),
                value: source.editable_parameter(&t.target)?.value,
            })?;
        }
        reconstructed = reconstructed.canonicalize_instrument_defaults(&source)?;
        if reconstructed.fingerprint() != source.fingerprint() {
            return Err("project reload changed unsupported/uncaptured source semantics".into());
        }
        meldritch_render::song_host::CompiledSongPatch::compile_initial(&source)?;
        self.enable_instrument_editor(base)?;
        self.project_source.as_mut().unwrap().source = Arc::new(source);
        let editor = self.instrument_editor.as_mut().unwrap();
        editor.synth = synth.into();
        self.enable_pattern_editor(pattern)?;
        self.enable_assignments(definition)
    }
    /// Existing curated widgets observing a catalog target use the same source
    /// command as physical faders, rather than maintaining a competing override.
    pub(crate) fn catalog_control_command(
        &self,
        command: &AppCommand,
    ) -> Result<Option<AppCommand>, AppCommandError> {
        let id = match command {
            AppCommand::AdjustCuratedControl { id, .. }
            | AppCommand::SetCuratedControlNormalized { id, .. } => id,
            _ => return Ok(None),
        };
        let Some(a) = &self.assignment_editor else {
            return Ok(None);
        };
        let Some(control) = self.curated_controls.iter().find(|c| &c.id == id) else {
            return Ok(None);
        };
        let Some(target) = a
            .definition
            .targets
            .iter()
            .find(|t| t.target.identity() == control.target)
        else {
            return Ok(None);
        };
        let owner = self.project_source.as_ref().unwrap();
        let current = owner
            .source
            .editable_parameter(&target.target)
            .map_err(AppCommandError::Instrument)?
            .value;
        let value = match command {
            AppCommand::AdjustCuratedControl { steps, .. } => {
                let index =
                    ((current - control.minimum) / control.step).round() + f64::from(*steps);
                (control.minimum + control.step * index).clamp(control.minimum, control.maximum)
            }
            AppCommand::SetCuratedControlNormalized { value, .. } => {
                if !value.is_finite() || !(0.0..=1.0).contains(value) {
                    return Err(AppCommandError::Instrument(
                        "normalized value must be finite in 0..1".into(),
                    ));
                }
                normalized_parameter_value(control.minimum, control.maximum, control.step, *value)
            }
            _ => unreachable!(),
        };
        Ok(Some(AppCommand::EditParameter(ParameterEdit {
            target: target.target.clone(),
            value,
        })))
    }
    pub fn enable_assignment_authoring_from(
        &mut self,
        base: ValidatedSong,
        source: ValidatedSong,
        definition: AssignmentDefinition,
    ) -> Result<(), String> {
        definition.validate(&source)?;
        let mut reconstructed = base.clone();
        for t in &definition.targets {
            if matches!(&t.target, EditableTarget::Track {id,parameter:meldritch_dsl::MixerParameter::Pan} if source.performance().tracks().iter().find(|t|t.id()==id).unwrap().pan().is_none())
            {
                continue;
            }
            reconstructed = reconstructed.edit_parameter(&ParameterEdit {
                target: t.target.clone(),
                value: source.editable_parameter(&t.target)?.value,
            })?;
        }
        reconstructed = reconstructed.canonicalize_instrument_defaults(&base)?;
        if reconstructed.fingerprint() != source.fingerprint() {
            return Err("controls reload changed unsupported/uncatalogued source semantics".into());
        }
        meldritch_render::song_host::CompiledSongPatch::compile_initial(&source)?;
        self.enable_instrument_editor(base)?;
        self.project_source.as_mut().unwrap().source = Arc::new(source);
        self.enable_assignments(definition)
    }
    pub fn enable_assignments(&mut self, definition: AssignmentDefinition) -> Result<(), String> {
        let owner = self
            .project_source
            .as_ref()
            .ok_or("no authoring song source")?;
        definition.validate(&owner.source)?;
        self.assignment_editor = Some(AssignmentEditor {
            page: definition.pages[0].clone(),
            definition,
            selected_fader: 0,
            selected_target: 0,
            visible: false,
            observations: BTreeMap::new(),
            save_status: None,
        });
        Ok(())
    }
    pub fn assignment_view(&self) -> Option<AssignmentEditorView> {
        let a = self.assignment_editor.as_ref()?;
        let owner = self.project_source.as_ref()?;
        Some(AssignmentEditorView {
            definition: a.definition.clone(),
            page: a.page.clone(),
            selected_fader: a.selected_fader,
            selected_target: a.selected_target,
            visible: a.visible,
            observations: a.observations.clone(),
            source: owner.source.clone(),
            base: owner.base.clone(),
            save_status: a.save_status.clone(),
        })
    }
    pub fn observe_assignment(&mut self, id: &str, raw: u8, acquired: bool) {
        if let Some(a) = self.assignment_editor.as_mut() {
            a.observations.insert(id.into(), (raw, acquired));
        }
    }
    pub fn observe_assignment_save(&mut self, status: String) {
        if let Some(a) = self.assignment_editor.as_mut() {
            a.save_status = Some(status);
        }
    }
    pub fn disconnect_assignments(&mut self, device: &str) {
        if let Some(a) = self.assignment_editor.as_mut() {
            for f in &a.definition.faders {
                if f.device == device
                    && let Some(o) = a.observations.get_mut(&f.id)
                {
                    o.1 = false;
                }
            }
        }
    }
    pub(crate) fn rearm_assignment_target(&mut self, target: &EditableTarget) {
        if let Some(a) = self.assignment_editor.as_mut() {
            for f in &a.definition.faders {
                if a.definition
                    .targets
                    .iter()
                    .any(|t| Some(&t.id) == f.target.as_ref() && &t.target == target)
                    && let Some(o) = a.observations.get_mut(&f.id)
                {
                    o.1 = false;
                }
            }
        }
    }
    pub(crate) fn assignment_controls(&self) -> Vec<CuratedControlView> {
        let Some(a) = self.assignment_view() else {
            return vec![];
        };
        a.definition
            .faders
            .iter()
            .filter_map(|f| {
                let t = a
                    .definition
                    .targets
                    .iter()
                    .find(|t| Some(&t.id) == f.target.as_ref())?;
                let p = a.source.editable_parameter(&t.target).ok()?;
                Some(CuratedControlView {
                    id: format!("assignment:{}", f.id),
                    label: f.id.clone(),
                    target: t.target.identity(),
                    minimum: t.minimum,
                    maximum: t.maximum,
                    step: t.step,
                    binding: format!("{} CC{} ({})", f.device, f.cc, f.page),
                    value: Some(p.value),
                })
            })
            .collect()
    }
    pub(crate) fn assignment_command(
        &mut self,
        command: &AppCommand,
    ) -> Result<AppCommandResult, AppCommandError> {
        let err = |s: String| AppCommandError::Instrument(s);
        if let AppCommand::EditParameter(edit) = command {
            let owner = self
                .project_source
                .as_mut()
                .ok_or_else(|| err("no authoring source".into()))?;
            let next = owner
                .source
                .edit_parameter(edit)
                .map_err(err)?
                .canonicalize_instrument_defaults(&owner.base)
                .map_err(err)?;
            meldritch_render::song_host::CompiledSongPatch::compile_initial(&next).map_err(err)?;
            let changed = next.fingerprint() != owner.source.fingerprint();
            owner.source = Arc::new(next);
            for control in &mut self.curated_controls {
                if control.target == edit.target.identity() {
                    control.value = Some(edit.value);
                }
            }
            if changed && let Some(a) = self.assignment_editor.as_mut() {
                a.save_status = None;
            }
            if changed {
                self.rearm_assignment_target(&edit.target);
            }
            return Ok(AppCommandResult::ParameterEdited {
                target: edit.target.clone(),
                changed,
            });
        }
        let a = self
            .assignment_editor
            .as_mut()
            .ok_or_else(|| err("no Lua assignment definition loaded".into()))?;
        let mut changed = false;
        match command {
            AppCommand::AssignFader { id, target } => {
                if target
                    .as_ref()
                    .is_some_and(|id| !a.definition.targets.iter().any(|t| t.id == *id))
                {
                    return Err(err(
                        "missing assignment target; choose a declared catalog target".into(),
                    ));
                }
                let f = a
                    .definition
                    .faders
                    .iter_mut()
                    .find(|f| f.id == *id)
                    .ok_or_else(|| err("missing declared fader identity".into()))?;
                changed = f.target != *target;
                f.target = target.clone();
                if changed {
                    for o in a.observations.values_mut() {
                        o.1 = false;
                    }
                    a.save_status = None;
                }
            }
            AppCommand::AssignmentPage(page) => {
                if !a.definition.pages.contains(page) {
                    return Err(err("unknown assignment page".into()));
                }
                changed = a.page != *page;
                a.page = page.clone();
                a.selected_fader = 0;
                if changed {
                    for o in a.observations.values_mut() {
                        o.1 = false;
                    }
                }
            }
            AppCommand::ToggleAssignmentEditor => {
                a.visible = !a.visible;
                if a.visible {
                    self.instrument_editor.as_mut().unwrap().visible = false;
                    if let Some(p) = self.pattern_selection.as_mut() {
                        p.visible = false;
                    }
                }
            }
            AppCommand::NavigateAssignment {
                fader_delta,
                target_delta,
            } => {
                let count = a
                    .definition
                    .faders
                    .iter()
                    .filter(|f| f.page == a.page)
                    .count()
                    .max(1);
                a.selected_fader =
                    (a.selected_fader as i32 + fader_delta).rem_euclid(count as i32) as usize;
                a.selected_target = (a.selected_target as i32 + target_delta)
                    .rem_euclid(a.definition.targets.len() as i32)
                    as usize;
            }
            AppCommand::SaveAssignments => return Ok(AppCommandResult::AssignmentSaveRequested),
            _ => unreachable!(),
        }
        Ok(AppCommandResult::AssignmentsChanged { changed })
    }
    pub(crate) fn assignment_input(
        &self,
        id: &str,
        value: f64,
    ) -> Result<AppCommand, AppCommandError> {
        let view = self
            .assignment_view()
            .ok_or_else(|| AppCommandError::Instrument("no authoring assignments".into()))?;
        let f = view
            .definition
            .faders
            .iter()
            .find(|f| format!("assignment:{}", f.id) == id && f.page == view.page)
            .ok_or_else(|| {
                AppCommandError::Instrument("inactive or missing fader assignment".into())
            })?;
        let t = view
            .definition
            .targets
            .iter()
            .find(|t| Some(&t.id) == f.target.as_ref())
            .ok_or_else(|| AppCommandError::Instrument("fader has no target".into()))?;
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return Err(AppCommandError::Instrument(
                "normalized value must be finite in 0..1".into(),
            ));
        }
        Ok(AppCommand::EditParameter(ParameterEdit {
            target: t.target.clone(),
            value: normalized_parameter_value(t.minimum, t.maximum, t.step, value),
        }))
    }
}
