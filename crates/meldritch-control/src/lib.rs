//! Single-owner, non-realtime Lua composition and control dispatch.
//! Application state and command semantics remain in `meldritch-app`.

use meldritch_app::{AppCommand, AppController, AppViewModel, CockpitMode};
use meldritch_dsl::{ValidatedSong, load_song_directory};
use mlua::{Function, HookTriggers, Lua, LuaOptions, LuaSerdeExt, StdLib, Table, Value, VmState};
use serde::{Deserialize, Serialize};
use std::cell::Cell;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::Path;
use std::rc::Rc;

pub type Result<T> = std::result::Result<T, String>;
pub mod assignments;
pub mod instrument;
pub mod led_demo;
pub mod pattern;
pub mod performance;
pub mod pickup;
pub mod reconnect;
pub mod session;
pub mod song_host;
const MAX_DEFINITIONS: usize = 64;
pub const QUEUE_CAPACITY: usize = 64;
pub const JOURNAL_CAPACITY: usize = 256;
pub const TRACE_CAPACITY: usize = 256;
const MAX_CAUSAL_DEPTH: u8 = 16;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SurfaceKind {
    ApcMiniMk2,
    StreamDeckXl,
    Tui,
}

impl SurfaceKind {
    pub fn accepts_cell(self, cell: u8) -> bool {
        cell < self.cells()
            || match self {
                Self::ApcMiniMk2 => {
                    (100..=107).contains(&cell) || (112..=119).contains(&cell) || cell == 122
                }
                Self::Tui => cell < 128,
                Self::StreamDeckXl => false,
            }
    }
    pub fn cells(self) -> u8 {
        match self {
            Self::ApcMiniMk2 => 64,
            Self::StreamDeckXl => 32,
            Self::Tui => 64,
        }
    }
    fn name(self) -> &'static str {
        match self {
            Self::ApcMiniMk2 => "apc_mini_mk2",
            Self::StreamDeckXl => "stream_deck_xl",
            Self::Tui => "tui",
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SurfaceDefinition {
    id: String,
    kind: SurfaceKind,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Binding {
    surface: String,
    cell: u8,
    widget: Option<String>,
    event: Option<String>,
}

/// Wire descriptions are translated immediately to the existing AppCommand.
/// This is a versioned scripting vocabulary, not another command processor.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(tag = "name", rename_all = "snake_case", deny_unknown_fields)]
pub enum CommandRequest {
    EditParameter {
        edit: meldritch_app::ParameterEdit,
    },
    AssignFader {
        id: String,
        target: Option<String>,
    },
    AssignmentPage {
        id: String,
    },
    SaveAssignments,
    CreateNotePattern {
        track: String,
        id: String,
        length_ticks: u64,
        looped: bool,
    },
    EditNote {
        pattern: String,
        edit: meldritch_dsl::NoteEdit,
    },
    SelectNote {
        pattern: String,
        id: String,
    },
    SavePattern,
    EditInstrument {
        edit: meldritch_dsl::InstrumentEdit,
    },
    SetMode {
        mode: Mode,
    },
    Play,
    Stop,
    Rewind,
    SelectPage {
        id: String,
    },
    SelectLaneVariation {
        lane: String,
        variation: String,
    },
    ToggleLaneMute {
        lane: String,
    },
    ToggleLaneSolo {
        lane: String,
    },
    SetControl {
        id: String,
        value: f64,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    Performance,
    AllParameters,
}

impl CommandRequest {
    pub fn command(&self) -> Result<AppCommand> {
        match self {
            Self::SelectPage { id } => identity(id)?,
            Self::SetControl { id, .. } => identity(id.strip_prefix("assignment:").unwrap_or(id))?,
            Self::ToggleLaneMute { lane } | Self::ToggleLaneSolo { lane } => identity(lane)?,
            Self::SelectLaneVariation { lane, variation } => {
                identity(lane)?;
                identity(variation)?;
            }
            _ => {}
        }
        Ok(match self {
            Self::EditParameter { edit } => AppCommand::EditParameter(edit.clone()),
            Self::AssignFader { id, target } => AppCommand::AssignFader {
                id: id.clone(),
                target: target.clone(),
            },
            Self::AssignmentPage { id } => AppCommand::AssignmentPage(id.clone()),
            Self::SaveAssignments => AppCommand::SaveAssignments,
            Self::CreateNotePattern {
                track,
                id,
                length_ticks,
                looped,
            } => AppCommand::CreateNotePattern {
                track: track.clone(),
                id: id.clone(),
                length_ticks: *length_ticks,
                looped: *looped,
            },
            Self::EditNote { pattern, edit } => AppCommand::EditNote {
                pattern: pattern.clone(),
                edit: edit.clone(),
            },
            Self::SelectNote { pattern, id } => AppCommand::SelectNote {
                pattern: pattern.clone(),
                id: id.clone(),
            },
            Self::SavePattern => AppCommand::SavePattern,
            Self::EditInstrument { edit } => AppCommand::EditInstrument(edit.clone()),
            Self::SetMode { mode } => AppCommand::SetCockpitMode(match mode {
                Mode::Performance => CockpitMode::Performance,
                Mode::AllParameters => CockpitMode::AllParameters,
            }),
            Self::Play => AppCommand::Play,
            Self::Stop => AppCommand::Stop,
            Self::Rewind => AppCommand::Rewind,
            Self::SelectPage { id } => AppCommand::SelectPerformancePage(id.clone()),
            Self::SelectLaneVariation { lane, variation } => AppCommand::SelectLaneVariation {
                lane_id: lane.clone(),
                variation_id: variation.clone(),
            },
            Self::ToggleLaneMute { lane } => AppCommand::ToggleLaneMute(lane.clone()),
            Self::ToggleLaneSolo { lane } => AppCommand::ToggleLaneSolo(lane.clone()),
            Self::SetControl { id, value } => {
                if !value.is_finite() || !(0.0..=1.0).contains(value) {
                    return Err("control value must be finite and normalized".into());
                }
                AppCommand::SetCuratedControlNormalized {
                    id: id.clone(),
                    value: *value,
                }
            }
        })
    }
}

struct Widget {
    events: bool,
    dependencies: BTreeSet<String>,
    interact: Function,
    present: Function,
}

struct State {
    parent: Option<String>,
    transitions: BTreeSet<String>,
    bindings: Vec<Binding>,
    handle: Option<Function>,
    entry: Option<Function>,
    exit: Option<Function>,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct Decision {
    command: Option<CommandRequest>,
    transition: Option<String>,
    reject: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Presentation {
    pub label: String,
    pub color: String,
    pub style: String,
}

/// Refreshable semantic output. A transport must replace the complete frame,
/// clearing cells no longer present. It never owns musical state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SurfaceFrame {
    pub revision: u64,
    pub page: String,
    pub cells: BTreeMap<u8, Presentation>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SavedProject {
    pub version: u32,
    pub id: String,
    pub song_fingerprint: String,
    pub control_fingerprint: String,
    pub lua_source: String,
}

/// Owns a fresh VM and validated song. No Send/Sync feature is enabled for Lua.
pub struct LuaProject {
    pub assignments: Option<meldritch_app::AssignmentDefinition>,
    lua: Lua,
    budget: Rc<Cell<u32>>,
    source: String,
    pub id: String,
    pub song: ValidatedSong,
    initial: String,
    states: BTreeMap<String, State>,
    widgets: BTreeMap<String, Widget>,
    surfaces: BTreeMap<String, SurfaceDefinition>,
}

fn error(e: impl std::fmt::Display) -> String {
    e.to_string()
}
fn identity(id: &str) -> Result<()> {
    if id.is_empty()
        || id.len() > 64
        || !id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
    {
        Err(format!("invalid identity '{id}'"))
    } else {
        Ok(())
    }
}
fn tables(table: Table) -> Result<Vec<Table>> {
    tables_limit(table, MAX_DEFINITIONS)
}
fn tables_limit(table: Table, maximum: usize) -> Result<Vec<Table>> {
    if table.raw_len() > maximum {
        return Err("too many definitions".into());
    }
    let count = table.clone().pairs::<Value, Value>().count();
    if count != table.raw_len() {
        return Err("definitions must be contiguous arrays".into());
    }
    table
        .sequence_values::<Table>()
        .map(|v| v.map_err(error))
        .collect()
}
fn insert<T>(map: &mut BTreeMap<String, T>, id: String, value: T) -> Result<()> {
    identity(&id)?;
    if map.insert(id.clone(), value).is_some() {
        return Err(format!("duplicate identity '{id}'"));
    }
    Ok(())
}

fn constrained_vm(source: &str) -> Result<(Lua, Rc<Cell<u32>>)> {
    if source.len() > 256 * 1024 {
        return Err("Lua source too large".into());
    }
    let lua = Lua::new_with(
        StdLib::TABLE | StdLib::STRING | StdLib::UTF8,
        LuaOptions::default(),
    )
    .map_err(error)?;
    lua.set_memory_limit(4 * 1024 * 1024).map_err(error)?;
    // Base-library functions can otherwise load files/bytecode or catch the
    // instruction-limit error repeatedly. No coroutine library is loaded.
    for name in [
        "dofile",
        "loadfile",
        "load",
        "require",
        "print",
        "collectgarbage",
        "pcall",
        "xpcall",
        "rawset",
    ] {
        lua.globals().set(name, Value::Nil).map_err(error)?;
    }
    let budget = Rc::new(Cell::new(0u32));
    let hook_budget = Rc::clone(&budget);
    lua.set_hook(
        HookTriggers::new().every_nth_instruction(1000),
        move |_, _| {
            let count = hook_budget.get() + 1;
            hook_budget.set(count);
            if count > 100 {
                return Err(mlua::Error::RuntimeError(
                    "Lua instruction budget exceeded".into(),
                ));
            }
            Ok(VmState::Continue)
        },
    )
    .map_err(error)?;
    Ok((lua, budget))
}

impl LuaProject {
    /// Immutable transport ownership across all pages; releases never fall through.
    pub fn surface_cells(&self, surface: &str) -> BTreeSet<u8> {
        self.states
            .values()
            .flat_map(|state| &state.bindings)
            .filter(|binding| binding.surface == surface)
            .map(|binding| binding.cell)
            .collect()
    }

    pub fn surfaces(&self) -> impl Iterator<Item = (&str, SurfaceKind)> {
        self.surfaces
            .values()
            .map(|surface| (surface.id.as_str(), surface.kind))
    }
    pub fn load(root: impl AsRef<Path>, source: &str) -> Result<Self> {
        let (lua, budget) = constrained_vm(source)?;
        let definition: Table = lua
            .load(source)
            .set_name("main.lua")
            .set_mode(mlua::chunk::ChunkMode::Text)
            .eval()
            .map_err(error)?;
        let version: u32 = definition.get("version").map_err(error)?;
        if version != 1 {
            return Err(format!("unsupported Lua API version {version}"));
        }
        let id: String = definition.get("id").map_err(error)?;
        identity(&id)?;
        let root = root.as_ref().canonicalize().map_err(error)?;
        let reference: String = definition.get("song").map_err(error)?;
        let song_root = root.join(reference).canonicalize().map_err(error)?;
        if !song_root.starts_with(&root) {
            return Err("project reference escapes root".into());
        }
        let song = load_song_directory(&song_root).map_err(error)?;
        let initial: String = definition.get("initial").map_err(error)?;
        let mut surfaces = BTreeMap::new();
        for table in tables(definition.get("surfaces").map_err(error)?)? {
            let def: SurfaceDefinition = lua.from_value(Value::Table(table)).map_err(error)?;
            insert(&mut surfaces, def.id.clone(), def)?;
        }
        if surfaces.is_empty() {
            return Err("at least one surface is required".into());
        }
        let mut widgets = BTreeMap::new();
        for table in tables(definition.get("widgets").map_err(error)?)? {
            let id: String = table.get("id").map_err(error)?;
            let dependencies: Vec<String> = lua
                .from_value(table.get("dependencies").map_err(error)?)
                .map_err(error)?;
            if dependencies.len() > MAX_DEFINITIONS
                || dependencies.iter().any(|d| {
                    ![
                        "mode",
                        "page",
                        "controls",
                        "lanes",
                        "transport",
                        "audio",
                        "instrument",
                        "pattern",
                        "assignments",
                    ]
                    .contains(&d.as_str())
                })
            {
                return Err(format!("unknown dependency for widget '{id}'"));
            }
            insert(
                &mut widgets,
                id,
                Widget {
                    dependencies: dependencies.into_iter().collect(),
                    interact: table.get("interact").map_err(error)?,
                    events: table
                        .get::<Option<bool>>("events")
                        .map_err(error)?
                        .unwrap_or(false),
                    present: table.get("present").map_err(error)?,
                },
            )?;
        }
        let mut states = BTreeMap::new();
        for table in tables(definition.get("states").map_err(error)?)? {
            let id: String = table.get("id").map_err(error)?;
            let transitions: Table = table.get("transitions").map_err(error)?;
            if transitions.raw_len() > MAX_DEFINITIONS {
                return Err("too many transitions".into());
            }
            let bindings = match table.get::<Option<Table>>("bindings").map_err(error)? {
                Some(table) => tables_limit(table, 256)?
                    .into_iter()
                    .map(|t| lua.from_value(Value::Table(t)).map_err(error))
                    .collect::<Result<Vec<Binding>>>()?,
                None => Vec::new(),
            };
            insert(
                &mut states,
                id,
                State {
                    parent: table.get("parent").map_err(error)?,
                    transitions: transitions
                        .sequence_values::<String>()
                        .map(|v| v.map_err(error))
                        .collect::<Result<_>>()?,
                    bindings,
                    handle: table.get("handle").map_err(error)?,
                    entry: table.get("entry").map_err(error)?,
                    exit: table.get("exit").map_err(error)?,
                },
            )?;
        }
        if !states.contains_key(&initial) {
            return Err("unknown initial state".into());
        }
        let project = Self {
            assignments: match definition.get::<Value>("assignments").map_err(error)? {
                Value::Nil => None,
                v => {
                    let a: meldritch_app::AssignmentDefinition =
                        lua.from_value(v).map_err(error)?;
                    a.validate(&song)?;
                    Some(a)
                }
            },
            lua,
            budget,
            source: source.into(),
            id,
            song,
            initial,
            states,
            widgets,
            surfaces,
        };
        for (id, state) in &project.states {
            project.path(id)?;
            for target in &state.transitions {
                if !project.states.contains_key(target) {
                    return Err(format!("unknown transition '{target}'"));
                }
            }
            let mut cells = BTreeSet::new();
            for binding in &state.bindings {
                let surface = project
                    .surfaces
                    .get(&binding.surface)
                    .ok_or_else(|| format!("unknown surface '{}'", binding.surface))?;
                if !surface.kind.accepts_cell(binding.cell)
                    || !cells.insert((&binding.surface, binding.cell))
                {
                    return Err(format!("invalid/duplicate binding in '{id}'"));
                }
                if binding.widget.is_some() == binding.event.is_some() {
                    return Err("binding requires exactly one widget or event".into());
                }
                if let Some(widget) = &binding.widget
                    && !project.widgets.contains_key(widget)
                {
                    return Err(format!("unknown widget '{widget}'"));
                }
                if let Some(event) = &binding.event {
                    identity(event)?;
                    if event == "command_accepted" {
                        return Err("bindings cannot forge domain facts".into());
                    }
                }
            }
        }
        Ok(project)
    }

    fn path(&self, state: &str) -> Result<Vec<String>> {
        let mut path = Vec::new();
        let mut next = Some(state);
        while let Some(id) = next {
            if path.iter().any(|s| s == id) {
                return Err("state parent cycle".into());
            }
            let state = self
                .states
                .get(id)
                .ok_or_else(|| format!("unknown state '{id}'"))?;
            path.push(id.to_owned());
            next = state.parent.as_deref();
        }
        path.reverse();
        Ok(path)
    }

    fn arm(&self) {
        self.budget.set(0);
    }

    fn projection(&self, view: &AppViewModel) -> Result<Table> {
        let values = self.lua.create_table().map_err(error)?;
        if let Some(a) = &view.assignments {
            let data = self.lua.create_table().map_err(error)?;
            data.set("page", a.page.clone()).map_err(error)?;
            data.set(
                "definition",
                self.lua.to_value(&a.definition).map_err(error)?,
            )
            .map_err(error)?;
            let observations = self.lua.create_table().map_err(error)?;
            for (id, (raw, acquired)) in &a.observations {
                let o = self.lua.create_table().map_err(error)?;
                o.set("raw", *raw).map_err(error)?;
                o.set("acquired", *acquired).map_err(error)?;
                observations.set(id.clone(), o).map_err(error)?;
            }
            data.set("observations", observations).map_err(error)?;
            values
                .set("assignments", self.readonly_tree(data)?)
                .map_err(error)?;
        }
        if let Some(editor) = &view.pattern_editor {
            let pattern = self
                .lua
                .to_value(&editor.pattern)
                .map_err(error)?
                .as_table()
                .unwrap()
                .clone();
            pattern
                .set("selected_note", editor.selected_note.clone())
                .map_err(error)?;
            pattern
                .set(
                    "fingerprint",
                    format!("{:016x}", editor.source.fingerprint()),
                )
                .map_err(error)?;
            let position = u64::from(view.transport.position);
            let frame = if editor.pattern.looped {
                position % editor.source.note_patterns()[&editor.pattern.id].length_frames()
            } else {
                position
            };
            let tick = (frame as f64
                * editor.source.performance().bpm()
                * meldritch_dsl::AUTHORING_PPQ as f64
                / (f64::from(editor.source.performance().sample_rate()) * 60.0))
                .floor() as u64;
            pattern.set("playhead_tick", tick).map_err(error)?;
            values
                .set("pattern", self.readonly_tree(pattern)?)
                .map_err(error)?;
        }
        if let Some(editor) = &view.instrument_editor {
            let instrument = self.lua.create_table().map_err(error)?;
            instrument.set("id", editor.synth.clone()).map_err(error)?;
            instrument
                .set(
                    "fingerprint",
                    format!("{:016x}", editor.source.fingerprint()),
                )
                .map_err(error)?;
            instrument
                .set("selected_parameter", editor.selected_parameter + 1)
                .map_err(error)?;
            // Recursively readonly proxies, using the same typed parameter projection.
            instrument
                .set(
                    "parameters",
                    self.readonly_tree(
                        self.lua
                            .to_value(&editor.parameters)
                            .map_err(error)?
                            .as_table()
                            .unwrap()
                            .clone(),
                    )?,
                )
                .map_err(error)?;
            instrument
                .set(
                    "defaults",
                    self.readonly_tree(
                        self.lua
                            .to_value(&editor.defaults)
                            .map_err(error)?
                            .as_table()
                            .unwrap()
                            .clone(),
                    )?,
                )
                .map_err(error)?;
            values
                .set("instrument", self.readonly(instrument)?)
                .map_err(error)?;
        }
        values
            .set(
                "mode",
                match view.cockpit_mode {
                    CockpitMode::Performance => "performance",
                    CockpitMode::AllParameters => "all_parameters",
                },
            )
            .map_err(error)?;
        values
            .set("frame", u64::from(view.transport.position))
            .map_err(error)?;
        values
            .set(
                "playing",
                view.transport.state == meldritch_audio::transport::TransportState::Playing,
            )
            .map_err(error)?;
        let controls = self.lua.create_table().map_err(error)?;
        for control in &view.curated_controls {
            controls
                .set(control.id.as_str(), control.value)
                .map_err(error)?;
        }
        values
            .set("controls", self.readonly(controls)?)
            .map_err(error)?;
        let lanes = self.lua.create_table().map_err(error)?;
        let included = song_host::included_lanes_from_view(view);
        for page in &view.performance.pages {
            for strip in &page.strips {
                let lane = self.lua.create_table().map_err(error)?;
                lane.set("muted", strip.muted).map_err(error)?;
                lane.set("soloed", strip.soloed).map_err(error)?;
                lane.set("audible", included.contains(&strip.lane_id))
                    .map_err(error)?;
                lane.set("variation", strip.active_variation_id.clone())
                    .map_err(error)?;
                lanes
                    .set(strip.lane_id.as_str(), self.readonly(lane)?)
                    .map_err(error)?;
            }
        }
        values.set("lanes", self.readonly(lanes)?).map_err(error)?;
        if let Some(status) = &view.song_audio {
            let audio = self.lua.create_table().map_err(error)?;
            audio
                .set("requested_generation", status.requested_generation)
                .map_err(error)?;
            audio
                .set("published_generation", status.published_generation)
                .map_err(error)?;
            audio
                .set("callback_generation", status.callback_generation)
                .map_err(error)?;
            audio.set("pending", status.pending).map_err(error)?;
            audio.set("error", status.error.clone()).map_err(error)?;
            let lanes = self.lua.create_table().map_err(error)?;
            for (id, state) in &status.lanes {
                let lane = self.lua.create_table().map_err(error)?;
                lane.set("published_variation", state.published_variation.clone())
                    .map_err(error)?;
                lane.set("playing_variation", state.playing_variation.clone())
                    .map_err(error)?;
                lanes
                    .set(id.as_str(), self.readonly(lane)?)
                    .map_err(error)?;
            }
            audio.set("lanes", self.readonly(lanes)?).map_err(error)?;
            values.set("audio", self.readonly(audio)?).map_err(error)?;
        }
        values
            .set(
                "page",
                view.performance
                    .active_page
                    .and_then(|i| view.performance.pages.get(i))
                    .map(|p| p.id.clone()),
            )
            .map_err(error)?;
        self.readonly(values)
    }

    fn readonly_tree(&self, values: Table) -> Result<Table> {
        for pair in values.clone().pairs::<Value, Value>() {
            let (key, value) = pair.map_err(error)?;
            if let Value::Table(nested) = value {
                values
                    .set(key, self.readonly_tree(nested)?)
                    .map_err(error)?;
            }
        }
        self.readonly(values)
    }

    fn readonly(&self, values: Table) -> Result<Table> {
        let proxy = self.lua.create_table().map_err(error)?;
        let meta = self.lua.create_table().map_err(error)?;
        let length = values.raw_len();
        meta.set(
            "__len",
            self.lua
                .create_function(move |_, _: Value| Ok(length))
                .map_err(error)?,
        )
        .map_err(error)?;
        meta.set("__index", values).map_err(error)?;
        meta.set(
            "__newindex",
            self.lua
                .create_function(|_, _: (Value, Value, Value)| -> mlua::Result<()> {
                    Err(mlua::Error::RuntimeError(
                        "state projection is read-only".into(),
                    ))
                })
                .map_err(error)?,
        )
        .map_err(error)?;
        meta.set("__metatable", false).map_err(error)?;
        proxy.set_metatable(Some(meta)).map_err(error)?;
        Ok(proxy)
    }

    pub fn save(&self) -> Result<String> {
        toml::to_string_pretty(&SavedProject {
            version: 1,
            id: self.id.clone(),
            song_fingerprint: format!("{:016x}", self.song.fingerprint()),
            control_fingerprint: format!("{:016x}", self.fingerprint()),
            lua_source: self.source.clone(),
        })
        .map_err(error)
    }

    pub fn restore(root: impl AsRef<Path>, saved: &str) -> Result<Self> {
        let saved: SavedProject = toml::from_str(saved).map_err(error)?;
        if saved.version != 1 {
            return Err("unsupported saved Lua version".into());
        }
        let project = Self::load(root, &saved.lua_source)?;
        if project.id != saved.id
            || format!("{:016x}", project.song.fingerprint()) != saved.song_fingerprint
            || format!("{:016x}", project.fingerprint()) != saved.control_fingerprint
        {
            return Err("saved project identity/fingerprint mismatch".into());
        }
        Ok(project)
    }

    /// Stable FNV-1a identity of API version, exact Lua source and typed song.
    /// Source-sensitive by design; not a security/authentication hash.
    pub fn fingerprint(&self) -> u64 {
        let mut hash = 0xcbf29ce484222325u64;
        for byte in b"meldritch-lua54-api-v1"
            .iter()
            .chain(self.source.as_bytes())
            .chain(self.song.fingerprint().to_le_bytes().iter())
        {
            hash = (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3);
        }
        hash
    }
}

#[derive(Clone, Debug)]
pub enum Input {
    Pad { cell: u8, pressed: bool },
    Cancel,
    Press(u8),
    Named(String),
    Connection(bool),
}

#[derive(Clone, Debug)]
struct Event {
    source: String,
    frame: u64,
    payload: EventPayload,
    depth: u8,
}

/// Reserved domain vocabulary. These facts are produced only after dispatch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DomainEvent {
    CommandAccepted,
}

#[derive(Clone, Debug)]
enum EventPayload {
    Observed(Input),
    Fact(DomainEvent),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Input,
    Handler,
    Exit,
    Entry,
    Requested,
    Accepted,
    Rejected,
    Domain,
    Projection,
    Diagnostic,
}

#[derive(Clone, Debug)]
pub struct Trace {
    pub sequence: u64,
    pub frame: u64,
    pub source: String,
    pub phase: Phase,
    pub detail: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct AcceptedCommand {
    pub sequence: u64,
    pub frame: u64,
    pub source: String,
    pub request: CommandRequest,
    pub result: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Journal {
    pub version: u32,
    pub project_id: String,
    pub song_fingerprint: String,
    pub control_fingerprint: String,
    pub commands: Vec<AcceptedCommand>,
}

impl Journal {
    pub fn encode(&self) -> Result<String> {
        toml::to_string_pretty(self).map_err(error)
    }
    pub fn decode(source: &str) -> Result<Self> {
        let journal: Self = toml::from_str(source).map_err(error)?;
        if journal.version != 1 {
            return Err("unsupported journal version".into());
        }
        for pair in journal.commands.windows(2) {
            if pair[0].sequence >= pair[1].sequence || pair[0].frame > pair[1].frame {
                return Err("unordered journal".into());
            }
        }
        Ok(journal)
    }
    /// Apply at the recorded immediate control-frame boundary. The host must
    /// advance audio to each frame for sample-timed playback; this API reports
    /// frames, it does not pretend to drive a physical audio clock.
    pub fn replay(&self, project: &LuaProject, app: &mut AppController) -> Result<Vec<u64>> {
        if self.version != 1
            || self.project_id != project.id
            || self.song_fingerprint != format!("{:016x}", project.song.fingerprint())
            || self.control_fingerprint != format!("{:016x}", project.fingerprint())
        {
            return Err("replay identity/fingerprint mismatch".into());
        }
        let mut frames = Vec::new();
        for event in &self.commands {
            app.dispatch(event.request.command()?)
                .map_err(|e| format!("replay rejected at {}: {e:?}", event.sequence))?;
            frames.push(event.frame);
        }
        Ok(frames)
    }

    /// Persist outside realtime. Never overwrites an authored file or session.
    /// A failed write leaves the caller owning this journal for retry.
    pub fn write_session(&self, song_root: impl AsRef<Path>) -> Result<std::path::PathBuf> {
        use std::io::Write;
        use std::time::{SystemTime, UNIX_EPOCH};
        #[derive(Serialize)]
        struct Header {
            kind: &'static str,
            version: u32,
        }
        #[derive(Serialize)]
        struct Session<'a> {
            meldritch: Header,
            lua_control: &'a Journal,
        }
        let directory = song_root.as_ref().join("performances");
        std::fs::create_dir_all(&directory).map_err(error)?;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(error)?;
        let timestamp = utc_timestamp(now.as_secs());
        let contents = toml::to_string_pretty(&Session {
            meldritch: Header {
                kind: "session",
                version: 1,
            },
            lua_control: self,
        })
        .map_err(error)?;
        for collision in 0..1000 {
            let path = directory.join(format!(
                "{timestamp}-{:09}-{collision}.mlperformance",
                now.subsec_nanos()
            ));
            match std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
            {
                Ok(mut file) => {
                    file.write_all(contents.as_bytes()).map_err(error)?;
                    file.sync_all().map_err(error)?;
                    return Ok(path);
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(error(e)),
            }
        }
        Err("unable to reserve unique session path".into())
    }
}

// Gregorian civil date from days since Unix epoch. No locale/timezone dependency.
fn utc_timestamp(seconds: u64) -> String {
    let days = (seconds / 86400) as i64 + 719468;
    let era = days / 146097;
    let doe = days - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let mut year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    format!(
        "{year:04}{month:02}{day:02}T{:02}{:02}{:02}Z",
        seconds / 3600 % 24,
        seconds / 60 % 60,
        seconds % 60
    )
}

/// One control-thread owner, bounded FIFO and lossless accepted-command outbox.
/// Call `take_journal` before it fills. Audio never references this object.
pub struct ControlPlane {
    observed_audio: Option<meldritch_app::SongAudioView>,
    pub project: LuaProject,
    active: String,
    connected: BTreeSet<String>,
    queue: VecDeque<Event>,
    trace: VecDeque<Trace>,
    dropped_trace: u64,
    journal: Vec<AcceptedCommand>,
    next_command: u64,
    next_trace: u64,
    last_frame: u64,
    outputs: BTreeMap<String, SurfaceFrame>,
    revision: u64,
}

impl ControlPlane {
    /// Missing release/overflow invalidates queued pad observations, not accepted facts.
    pub fn interrupt_surface(&mut self, source: &str, frame: u64) -> Result<()> {
        self.queue.retain(|e| {
            !(e.source == source
                && matches!(
                    e.payload,
                    EventPayload::Observed(Input::Pad { .. } | Input::Press(_))
                ))
        });
        self.enqueue(source, frame, Input::Cancel)
    }
    pub fn new(project: LuaProject, app: &AppController) -> Result<Self> {
        let active = project.initial.clone();
        let connected = project.surfaces.keys().cloned().collect();
        let mut plane = Self {
            observed_audio: None,
            project,
            active,
            connected,
            queue: VecDeque::new(),
            trace: VecDeque::new(),
            dropped_trace: 0,
            journal: Vec::new(),
            next_command: 0,
            next_trace: 0,
            last_frame: 0,
            outputs: BTreeMap::new(),
            revision: 0,
        };
        let event = Event {
            source: "project".into(),
            frame: 0,
            payload: EventPayload::Observed(Input::Named("load".into())),
            depth: 0,
        };
        for state in plane.project.path(&plane.active)? {
            plane.lifecycle(&state, true, &event, app)?;
        }
        plane.refresh(app)?;
        Ok(plane)
    }
    pub fn active_state(&self) -> &str {
        &self.active
    }
    pub fn trace(&self) -> &VecDeque<Trace> {
        &self.trace
    }
    pub fn dropped_trace(&self) -> u64 {
        self.dropped_trace
    }
    pub fn output(&self, surface: &str) -> Option<&SurfaceFrame> {
        self.outputs.get(surface)
    }
    pub fn pending(&self) -> usize {
        self.queue.len()
    }
    pub fn enqueue(&mut self, source: &str, frame: u64, input: Input) -> Result<()> {
        if let Input::Named(name) = &input {
            identity(name)?;
            if name == "command_accepted" {
                return Err("domain facts cannot be injected as inputs".into());
            }
        }
        if !self.project.surfaces.contains_key(source) {
            return Err(format!("unknown input source '{source}'"));
        }
        if self.queue.len() >= QUEUE_CAPACITY {
            return Err("control input queue full; retry required".into());
        }
        // Availability is not a scheduled musical action. A rewound/stopped
        // playhead must not prevent a disconnect from removing its surface.
        if frame < self.last_frame
            && !matches!(
                &input,
                Input::Connection(_) | Input::Pad { .. } | Input::Cancel
            )
        {
            return Err("input frame moved backwards".into());
        }
        self.last_frame = self.last_frame.max(frame);
        self.queue.push_back(Event {
            source: source.into(),
            frame,
            payload: EventPayload::Observed(input),
            depth: 0,
        });
        Ok(())
    }
    fn log(&mut self, event: &Event, phase: Phase, detail: impl Into<String>) {
        if self.trace.len() == TRACE_CAPACITY {
            self.trace.pop_front();
            self.dropped_trace += 1;
        }
        let mut detail = detail.into();
        if detail.len() > 512 {
            let mut end = 512 - " [truncated]".len();
            while !detail.is_char_boundary(end) {
                end -= 1;
            }
            detail.truncate(end);
            detail.push_str(" [truncated]");
        }
        self.trace.push_back(Trace {
            sequence: self.next_trace,
            frame: event.frame,
            source: event.source.clone(),
            phase,
            detail,
        });
        self.next_trace += 1;
    }
    /// At most 64 events per pump, including queued domain follow-ups.
    pub fn pump(&mut self, app: &mut AppController) -> usize {
        self.pump_with_observer(app, |_, _, _, _, _| Ok(None))
    }

    /// Called immediately after acceptance, before any causal follow-up command.
    /// Observer errors are diagnostics: accepted facts are never rolled back.
    pub fn pump_with_observer(
        &mut self,
        app: &mut AppController,
        mut observer: impl FnMut(
            &AppController,
            &CommandRequest,
            &meldritch_app::AppCommandResult,
            &str,
            u64,
        ) -> Result<Option<String>>,
    ) -> usize {
        let mut processed = 0;
        while processed < QUEUE_CAPACITY {
            let Some(event) = self.queue.pop_front() else {
                break;
            };
            match &event.payload {
                EventPayload::Observed(input) => {
                    self.log(&event, Phase::Input, format!("{input:?}"))
                }
                EventPayload::Fact(fact) => {
                    self.log(&event, Phase::Domain, format!("dispatch {fact:?}"))
                }
            }
            if let Err(message) = self.process(&event, app, &mut observer) {
                self.log(&event, Phase::Rejected, message);
            }
            processed += 1;
        }
        processed
    }
    fn process(
        &mut self,
        event: &Event,
        app: &mut AppController,
        observer: &mut impl FnMut(
            &AppController,
            &CommandRequest,
            &meldritch_app::AppCommandResult,
            &str,
            u64,
        ) -> Result<Option<String>>,
    ) -> Result<()> {
        if matches!(
            event.payload,
            EventPayload::Observed(Input::Connection(false) | Input::Cancel)
        ) {
            // Physical availability is authoritative even if a Lua cancel
            // handler fails. Bad presentation must not retain a disconnected surface.
            if matches!(
                event.payload,
                EventPayload::Observed(Input::Connection(false))
            ) {
                self.connected.remove(&event.source);
                self.outputs.remove(&event.source);
            }
            let projection = self.project.projection(&app.view_model())?;
            for widget in self.project.widgets.values().filter(|w| w.events) {
                let input = self.project.lua.create_table().map_err(error)?;
                input.set("kind", "cancel").map_err(error)?;
                input.set("source", event.source.as_str()).map_err(error)?;
                self.project.arm();
                let result: Value = widget
                    .interact
                    .call((projection.clone(), input))
                    .map_err(error)?;
                if !result.is_nil() {
                    return Err("cancel handlers cannot mutate musical state".into());
                }
            }
            if matches!(event.payload, EventPayload::Observed(Input::Cancel)) {
                return self.refresh_at(app, event.frame);
            }
        }
        if let EventPayload::Observed(Input::Connection(connected)) = event.payload {
            if connected {
                self.connected.insert(event.source.clone());
            } else {
                self.connected.remove(&event.source);
                self.outputs.remove(&event.source);
            }
            self.log(
                event,
                Phase::Domain,
                if connected {
                    "device connected"
                } else {
                    "device disconnected"
                },
            );
            return self.refresh_at(app, event.frame);
        }
        if !self.connected.contains(&event.source) {
            return Err("device disconnected".into());
        }
        if event.depth >= MAX_CAUSAL_DEPTH {
            return Err("causal event depth exceeded".into());
        }
        let projection = self.project.projection(&app.view_model())?;
        let (decision, handler) = match &event.payload {
            EventPayload::Observed(Input::Press(cell) | Input::Pad { cell, .. }) => {
                let binding = self.project.states[&self.active]
                    .bindings
                    .iter()
                    .find(|b| b.surface == event.source && b.cell == *cell)
                    .ok_or("unbound surface cell")?
                    .clone();
                if let Some(widget) = binding.widget {
                    let pressed = !matches!(
                        event.payload,
                        EventPayload::Observed(Input::Pad { pressed: false, .. })
                    );
                    if !pressed && !self.project.widgets[&widget].events {
                        return self.refresh_at(app, event.frame);
                    }
                    self.log(event, Phase::Handler, format!("widget {widget}"));
                    self.project.arm();
                    let input = self.project.lua.create_table().map_err(error)?;
                    input
                        .set("kind", if pressed { "press" } else { "release" })
                        .map_err(error)?;
                    input.set("cell", *cell).map_err(error)?;
                    input.set("source", event.source.as_str()).map_err(error)?;
                    input.set("frame", event.frame.to_string()).map_err(error)?;
                    let value: Value = self.project.widgets[&widget]
                        .interact
                        .call((projection, input))
                        .map_err(error)?;
                    let command = if value.is_nil() {
                        None
                    } else {
                        Some(self.project.lua.from_value(value).map_err(error)?)
                    };
                    (
                        Decision {
                            command,
                            ..Decision::default()
                        },
                        None,
                    )
                } else {
                    if matches!(
                        event.payload,
                        EventPayload::Observed(Input::Pad { pressed: false, .. })
                    ) {
                        return self.refresh_at(app, event.frame);
                    }
                    self.handle_named(binding.event.as_deref().unwrap(), projection, event)?
                }
            }
            EventPayload::Observed(Input::Named(name)) => {
                self.handle_named(name, projection, event)?
            }
            EventPayload::Observed(Input::Connection(_)) => unreachable!(),
            EventPayload::Observed(Input::Cancel) => unreachable!(),
            EventPayload::Fact(DomainEvent::CommandAccepted) => {
                self.handle_named("command_accepted", projection, event)?
            }
        };
        if let Some(reject) = decision.reject {
            return Err(reject);
        }
        if let Some(target) = &decision.transition {
            let handler = handler
                .as_ref()
                .ok_or("widget transitions are unsupported")?;
            if !self.project.states[handler].transitions.contains(target) {
                return Err(format!("undeclared transition '{target}' from '{handler}'"));
            }
        }
        if let Some(request) = decision.command {
            self.log(event, Phase::Requested, format!("{request:?}"));
            // Reserve authoritative journal and follow-up space before mutation.
            if self.journal.len() >= JOURNAL_CAPACITY {
                return Err("accepted-command journal full; persist/drain and retry".into());
            }
            if self.queue.len() >= QUEUE_CAPACITY {
                return Err("domain event queue full; retry".into());
            }
            let result = app
                .dispatch(request.command()?)
                .map_err(|e| format!("command rejected: {e:?}"))?;
            self.log(event, Phase::Accepted, format!("{result:?}"));
            let diagnostic = match observer(app, &request, &result, &event.source, event.frame) {
                Err(message) => format!("accepted-command observer failed: {message}"),
                Ok(Some(message)) => message,
                Ok(None) => match &result {
                    meldritch_app::AppCommandResult::TransportQueued => {
                        "audio transport queued; completion belongs to the audio engine"
                    }
                    meldritch_app::AppCommandResult::LaneVariationSelected { .. }
                    | meldritch_app::AppCommandResult::LaneMuteToggled { .. }
                    | meldritch_app::AppCommandResult::CuratedControlAdjusted { .. } => {
                        "app state accepted; song host must submit existing rerender work"
                    }
                    _ => "presentation change; no audio or graph work scheduled",
                }
                .to_owned(),
            };
            self.log(event, Phase::Diagnostic, diagnostic);
            self.journal.push(AcceptedCommand {
                sequence: self.next_command,
                frame: event.frame,
                source: event.source.clone(),
                request,
                result: format!("{result:?}"),
            });
            self.next_command += 1;
            self.log(
                event,
                Phase::Domain,
                "command accepted; app view/history authoritative",
            );
            self.queue.push_front(Event {
                source: event.source.clone(),
                frame: event.frame,
                payload: EventPayload::Fact(DomainEvent::CommandAccepted),
                depth: event.depth + 1,
            });
        }
        if let Some(target) = decision.transition {
            self.transition(&target, event, app)?;
        }
        // Commands remain accepted if presentation fails. Diagnostics describe
        // stale telemetry; reconnect/refresh can reconstruct from the app.
        if let Err(message) = self.refresh_at(app, event.frame) {
            self.log(event, Phase::Diagnostic, message);
        }
        Ok(())
    }
    fn handle_named(
        &mut self,
        name: &str,
        projection: Table,
        event: &Event,
    ) -> Result<(Decision, Option<String>)> {
        let mut path = self.project.path(&self.active)?;
        path.reverse();
        for state in path {
            let Some(callback) = self.project.states[&state].handle.clone() else {
                continue;
            };
            self.log(event, Phase::Handler, format!("state {state}: {name}"));
            let input = self.project.lua.create_table().map_err(error)?;
            input.set("name", name).map_err(error)?;
            input.set("source", event.source.as_str()).map_err(error)?;
            // Lua integers are signed i64; decimal text preserves the entire
            // Rust u64 frame domain without silently rounding via Lua numbers.
            input.set("frame", event.frame.to_string()).map_err(error)?;
            self.project.arm();
            let result: Value = callback.call((projection.clone(), input)).map_err(error)?;
            if !result.is_nil() {
                return Ok((
                    self.project.lua.from_value(result).map_err(error)?,
                    Some(state),
                ));
            }
        }
        if name == "command_accepted" {
            return Ok((Decision::default(), None));
        }
        Err(format!("unhandled event '{name}'"))
    }
    fn lifecycle(
        &mut self,
        id: &str,
        entering: bool,
        event: &Event,
        app: &AppController,
    ) -> Result<()> {
        self.log(event, if entering { Phase::Entry } else { Phase::Exit }, id);
        let state = &self.project.states[id];
        let callback = if entering { &state.entry } else { &state.exit };
        if let Some(callback) = callback {
            self.project.arm();
            let message: Option<String> = callback
                .call(self.project.projection(&app.view_model())?)
                .map_err(error)?;
            if let Some(message) = message {
                self.log(event, Phase::Diagnostic, message);
            }
        }
        Ok(())
    }
    fn transition(&mut self, target: &str, event: &Event, app: &AppController) -> Result<()> {
        let old = self.project.path(&self.active)?;
        let new = self.project.path(target)?;
        let common = old.iter().zip(&new).take_while(|(a, b)| a == b).count();
        // Lifecycle callbacks cannot emit commands. Commit the explicit state
        // even on lifecycle errors; report them and continue reconstruction.
        for id in old[common..].iter().rev() {
            if let Err(message) = self.lifecycle(id, false, event, app) {
                self.log(event, Phase::Diagnostic, message);
            }
        }
        self.active = target.to_owned();
        for id in &new[common..] {
            if let Err(message) = self.lifecycle(id, true, event, app) {
                self.log(event, Phase::Diagnostic, message);
            }
        }
        self.log(
            event,
            Phase::Domain,
            format!("page transitioned to {target}"),
        );
        Ok(())
    }
    /// Also call after non-Lua/TUI/MIDI commands to project their accepted state.
    pub fn refresh(&mut self, app: &AppController) -> Result<()> {
        self.refresh_at(app, self.last_frame)
    }

    fn refresh_at(&mut self, app: &AppController, absolute_frame: u64) -> Result<()> {
        let audio = app.view_model().song_audio;
        if self.observed_audio != audio {
            let event = Event {
                source: "audio".into(),
                frame: absolute_frame,
                payload: EventPayload::Observed(Input::Named("audio_progress".into())),
                depth: 0,
            };
            if let Some(status) = &audio {
                self.log(
                    &event,
                    Phase::Diagnostic,
                    format!(
                        "audio requested={} published={} callback={:?} pending={} error={:?}",
                        status.requested_generation,
                        status.published_generation,
                        status.callback_generation,
                        status.pending,
                        status.error
                    ),
                );
            }
            self.observed_audio = audio;
        }
        let projection = self.project.projection(&app.view_model())?;
        let mut updates = BTreeMap::new();
        for surface in &self.connected {
            let kind = self.project.surfaces[surface].kind;
            let mut cells = BTreeMap::new();
            for binding in &self.project.states[&self.active].bindings {
                if &binding.surface != surface {
                    continue;
                }
                if let Some(id) = &binding.widget {
                    let widget = &self.project.widgets[id];
                    // Dependencies are validated API declarations. v1 refreshes
                    // at command/page/host boundaries; no subscription graph.
                    let _dependencies = &widget.dependencies;
                    self.project.arm();
                    let result: Value = widget
                        .present
                        .call((
                            projection.clone(),
                            kind.name(),
                            self.active.as_str(),
                            binding.cell,
                        ))
                        .map_err(error)?;
                    let presentation: Presentation =
                        self.project.lua.from_value(result).map_err(error)?;
                    if presentation.label.len() > 256
                        || presentation.color.len() > 32
                        || presentation.style.len() > 32
                    {
                        return Err("presentation exceeds bounds".into());
                    }
                    cells.insert(binding.cell, presentation);
                }
            }
            updates.insert(
                surface.clone(),
                SurfaceFrame {
                    revision: self.revision,
                    page: self.active.clone(),
                    cells,
                },
            );
        }
        // Publish all projections together. Failed Lua presentation cannot leave
        // one device on a new frame and another on the previous frame.
        for (source, frame) in updates {
            if self
                .outputs
                .get(&source)
                .is_none_or(|old| old.page != frame.page || old.cells != frame.cells)
            {
                let event = Event {
                    source: source.clone(),
                    frame: absolute_frame,
                    payload: EventPayload::Observed(Input::Named("refresh".into())),
                    depth: 0,
                };
                self.log(
                    &event,
                    Phase::Projection,
                    format!(
                        "page {} cells {:?}; audio work only via AppController result",
                        frame.page,
                        frame.cells.keys()
                    ),
                );
                self.outputs.insert(source, frame);
            }
        }
        self.revision += 1;
        Ok(())
    }
    pub fn take_journal(&mut self) -> Journal {
        Journal {
            version: 1,
            project_id: self.project.id.clone(),
            song_fingerprint: format!("{:016x}", self.project.song.fingerprint()),
            control_fingerprint: format!("{:016x}", self.project.fingerprint()),
            commands: std::mem::take(&mut self.journal),
        }
    }
}

pub mod devices;
