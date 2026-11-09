//! Bounded asynchronous HTTP/WebSocket transport for the UI-thread Lua owner.

use std::{
    cell::{Cell, RefCell},
    collections::{BTreeMap, BTreeSet},
    net::{IpAddr, SocketAddr},
    rc::Rc,
    sync::{
        Arc, RwLock,
        atomic::{AtomicU64, AtomicUsize, Ordering},
        mpsc::{self, SyncSender, TrySendError},
    },
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use axum::{
    Router,
    body::{Body, to_bytes},
    extract::{
        ConnectInfo, MatchedPath, Path, State, WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    http::{HeaderMap, HeaderName, HeaderValue, Method, Request, StatusCode},
    response::{IntoResponse, Response},
    routing::{MethodRouter, get},
};
use futures_util::{SinkExt, StreamExt};
use mlua::Function;
use serde::Serialize;
use serde_json::{Value, json};
use tokio::sync::{broadcast, oneshot};

use crate::compiler::ControlHandle;
use crate::sequencer::{NoteEvent, SequencerControl};

pub const DEFAULT_QUEUE_CAPACITY: usize = 128;
pub const DEFAULT_BODY_LIMIT: usize = 64 * 1024;
pub const DEFAULT_MESSAGE_LIMIT: usize = 64 * 1024;
pub const DEFAULT_MAX_CONNECTIONS: usize = 64;
pub const DEFAULT_TIMEOUT_MS: u64 = 2_000;

#[derive(Clone, Debug)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub queue_capacity: usize,
    pub body_limit: usize,
    pub message_limit: usize,
    pub max_connections: usize,
    pub timeout: Duration,
    pub allowed_origins: BTreeSet<String>,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".into(),
            port: 8080,
            queue_capacity: DEFAULT_QUEUE_CAPACITY,
            body_limit: DEFAULT_BODY_LIMIT,
            message_limit: DEFAULT_MESSAGE_LIMIT,
            max_connections: DEFAULT_MAX_CONNECTIONS,
            timeout: Duration::from_millis(DEFAULT_TIMEOUT_MS),
            allowed_origins: BTreeSet::new(),
        }
    }
}

#[derive(Clone)]
pub struct Route {
    pub method: Method,
    pub path: String,
    pub handler: Function,
}

#[derive(Clone)]
pub struct WebSocketCallbacks {
    pub path: String,
    pub connected: Option<Function>,
    pub message: Option<Function>,
    pub disconnected: Option<Function>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ControlKind {
    Continuous,
    Toggle,
    Trigger,
    Choice,
}

#[derive(Clone)]
pub struct PerformanceControl {
    pub name: String,
    pub label: String,
    pub group: Option<String>,
    pub kind: ControlKind,
    pub target: String,
    pub min: f64,
    pub max: f64,
    pub choices: Vec<String>,
    pub changed: Option<Function>,
}

#[derive(Clone, Debug)]
pub struct Role {
    pub controls: Option<BTreeSet<String>>,
}

#[derive(Clone)]
pub struct ControlPlane {
    pub id: String,
    pub label: String,
    pub layout: Option<String>,
    pub roles: BTreeSet<String>,
    pub read_only_roles: BTreeSet<String>,
    pub controls: Vec<PlaneControl>,
}
#[derive(Clone)]
pub enum PlaneControl {
    Range {
        id: String,
        label: String,
        target: String,
        kind: &'static str,
        min: f64,
        max: f64,
        scale: String,
        unit: Option<String>,
    },
    Steps {
        id: String,
        label: String,
        sequencer: Arc<SequencerControl>,
        track: usize,
        pattern: usize,
        steps: usize,
    },
    Trigger {
        id: String,
        label: String,
        callback: Function,
        serial: Rc<Cell<u64>>,
    },
    PianoRoll {
        id: String,
        label: String,
        sequencer: Arc<SequencerControl>,
        tracks: Vec<usize>,
        steps: usize,
        pitch_min: u8,
        pitch_max: u8,
    },
    DrumGrid {
        id: String,
        label: String,
        kit_name: String,
        sequencer: Arc<SequencerControl>,
        lanes: Vec<DrumLane>,
        steps: usize,
    },
    Adsr {
        id: String,
        label: String,
        parameters: Vec<AdsrParameter>,
    },
    Patterns {
        id: String,
        label: String,
        sequencer: Arc<SequencerControl>,
        default_boundary: String,
    },
    Presets {
        id: String,
        label: String,
        target: String,
        names: Vec<String>,
        entries: Vec<PresetEntry>,
        status: Option<Arc<crate::vst3::PresetStatus>>,
    },
}

#[derive(Clone, Debug, Serialize)]
pub struct PresetEntry {
    pub id: String,
    pub name: String,
    pub bank: String,
    pub category: String,
}

#[derive(Clone)]
pub struct DrumLane {
    pub id: String,
    pub label: String,
    pub track: usize,
    pub note: u8,
}

#[derive(Clone)]
pub struct AdsrParameter {
    pub name: &'static str,
    pub control: String,
    pub target: String,
    pub min: f64,
    pub max: f64,
    pub scale: String,
    pub unit: &'static str,
}

impl PlaneControl {
    pub fn id(&self) -> &str {
        match self {
            Self::Range { id, .. }
            | Self::Steps { id, .. }
            | Self::Trigger { id, .. }
            | Self::PianoRoll { id, .. }
            | Self::DrumGrid { id, .. }
            | Self::Adsr { id, .. }
            | Self::Patterns { id, .. }
            | Self::Presets { id, .. } => id,
        }
    }
}

fn parse_note_command(
    message: &Value,
    id: u32,
    pitch_min: u8,
    pitch_max: u8,
) -> Result<NoteEvent, String> {
    let byte = |name: &str| {
        message
            .get(name)
            .and_then(Value::as_u64)
            .and_then(|value| u8::try_from(value).ok())
            .ok_or_else(|| format!("note command requires byte field '{name}'"))
    };
    let pitch = byte("pitch")?;
    if !(pitch_min..=pitch_max).contains(&pitch) {
        return Err(format!(
            "note pitch must be in the control's {pitch_min}..={pitch_max} range"
        ));
    }
    let velocity = byte("velocity")?;
    let duration = byte("duration")?;
    if velocity == 0 || duration == 0 {
        return Err("note velocity and duration must be positive".into());
    }
    Ok(NoteEvent {
        id,
        step: byte("step")?,
        pitch,
        velocity,
        duration,
        legato: message
            .get("legato")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    })
}

#[derive(Clone)]
pub struct Application {
    pub config: ServerConfig,
    pub routes: Vec<Route>,
    pub websocket: Option<WebSocketCallbacks>,
    pub name: String,
    pub controls: BTreeMap<String, PerformanceControl>,
    pub roles: Rc<RefCell<BTreeMap<String, Role>>>,
    pub credentials: BTreeMap<String, String>,
    pub anonymous_role: Option<String>,
    pub planes: Rc<RefCell<BTreeMap<String, ControlPlane>>>,
    audio_telemetry: Option<crate::device::AudioTelemetry>,
    runtime: Rc<RefCell<PerformanceRuntime>>,
}

#[derive(Default)]
struct PerformanceRuntime {
    handles: BTreeMap<String, ControlHandle>,
    observed: BTreeMap<String, f64>,
    observed_planes: BTreeMap<String, u64>,
}

impl Application {
    pub fn new(config: ServerConfig) -> Self {
        Self {
            config,
            routes: Vec::new(),
            websocket: None,
            name: "Meldritch Performance".into(),
            controls: BTreeMap::new(),
            roles: Rc::new(RefCell::new(BTreeMap::new())),
            credentials: BTreeMap::new(),
            anonymous_role: None,
            planes: Rc::new(RefCell::new(BTreeMap::new())),
            audio_telemetry: None,
            runtime: Rc::new(RefCell::new(PerformanceRuntime::default())),
        }
    }

    pub fn set_audio_telemetry(&mut self, telemetry: crate::device::AudioTelemetry) {
        self.audio_telemetry = Some(telemetry);
    }

    pub fn attach_controls(&mut self, controls: Vec<(ControlHandle, f64)>) -> Result<(), String> {
        let mut runtime = self.runtime.borrow_mut();
        runtime.handles = controls
            .into_iter()
            .map(|(handle, _)| (handle.id().to_owned(), handle))
            .collect();
        for control in self.controls.values() {
            let Some(handle) = runtime.handles.get(&control.target) else {
                return Err(format!(
                    "network performance control '{}' targets unknown Meldritch control '{}'",
                    control.name, control.target
                ));
            };
            let (min, max) = handle.range();
            if control.min < min || control.max > max {
                return Err(format!(
                    "network performance control '{}' range {}..{} exceeds target '{}' range {min}..{max}",
                    control.name, control.min, control.max, control.target
                ));
            }
        }
        runtime.observed = self
            .controls
            .iter()
            .map(|(name, definition)| {
                (
                    name.clone(),
                    runtime.handles[&definition.target].effective_value(),
                )
            })
            .collect();
        Ok(())
    }

    fn permitted(&self, role: Option<&str>, control: &str) -> bool {
        role.and_then(|name| self.roles.borrow().get(name).cloned())
            .is_some_and(|role| {
                role.controls
                    .as_ref()
                    .is_none_or(|controls| controls.contains(control))
            })
    }

    fn set_control(
        &mut self,
        role: Option<&str>,
        name: &str,
        value: Value,
    ) -> Result<(), DispatchError> {
        let definition = self
            .controls
            .get(name)
            .cloned()
            .ok_or_else(|| DispatchError::not_found("unknown performance control"))?;
        if role != Some("__meldritch_trusted") && !self.permitted(role, name) {
            return Err(DispatchError::forbidden(
                "control is not assigned to this performer",
            ));
        }
        let handle = self
            .runtime
            .borrow()
            .handles
            .get(&definition.target)
            .cloned()
            .ok_or_else(|| DispatchError::internal("performance control is not attached"))?;
        let current = handle.effective_value();
        let resolved = match definition.kind {
            ControlKind::Continuous => value
                .as_f64()
                .filter(|value| value.is_finite())
                .ok_or_else(|| {
                    DispatchError::bad_request("continuous control requires a number")
                })?,
            ControlKind::Toggle => value
                .as_bool()
                .map(|value| {
                    if value {
                        definition.max
                    } else {
                        definition.min
                    }
                })
                .ok_or_else(|| DispatchError::bad_request("toggle control requires a boolean"))?,
            ControlKind::Trigger => {
                if !value.as_bool().unwrap_or(true) {
                    return Err(DispatchError::bad_request("trigger value must be true"));
                }
                if (current - definition.min).abs() <= (current - definition.max).abs() {
                    definition.max
                } else {
                    definition.min
                }
            }
            ControlKind::Choice => {
                let choice = value.as_str().ok_or_else(|| {
                    DispatchError::bad_request("choice control requires a string")
                })?;
                let index = definition
                    .choices
                    .iter()
                    .position(|candidate| candidate == choice)
                    .ok_or_else(|| DispatchError::bad_request("unknown control choice"))?;
                if definition.choices.len() <= 1 {
                    definition.min
                } else {
                    definition.min
                        + (definition.max - definition.min) * index as f64
                            / (definition.choices.len() - 1) as f64
                }
            }
        };
        if resolved < definition.min || resolved > definition.max {
            return Err(DispatchError::bad_request(
                "control value is outside its declared range",
            ));
        }
        if let Some(callback) = definition.changed {
            callback
                .call::<()>((resolved, current))
                .map_err(lua_error)?;
        }
        handle
            .publish(resolved)
            .map_err(DispatchError::bad_request)?;
        let aliases = self
            .controls
            .iter()
            .filter(|(_, control)| control.target == definition.target)
            .map(|(name, _)| name.clone())
            .collect::<Vec<_>>();
        let mut runtime = self.runtime.borrow_mut();
        for alias in aliases {
            runtime.observed.insert(alias, resolved);
        }
        Ok(())
    }

    pub fn set_control_from_lua(&mut self, name: &str, value: Value) -> Result<(), String> {
        self.set_control(Some("__meldritch_trusted"), name, value)
            .map_err(|error| error.message)?;
        self.runtime
            .borrow_mut()
            .observed
            .insert(name.to_owned(), f64::NAN);
        Ok(())
    }

    pub fn control_value(&self, name: &str) -> Option<f64> {
        let definition = self.controls.get(name)?;
        self.runtime
            .borrow()
            .handles
            .get(&definition.target)
            .map(ControlHandle::effective_value)
    }

    fn schema_fingerprint(&self) -> String {
        Value::Array(self.plane_definitions()).to_string()
    }

    fn plane_definitions(&self) -> Vec<Value> {
        self.planes
            .borrow()
            .values()
            .map(|plane| {
                let controls = plane
                    .controls
                    .iter()
                    .map(|control| match control {
                        PlaneControl::Range {
                            id,
                            label,
                            kind,
                            min,
                            max,
                            scale,
                            unit,
                            ..
                        } => json!({"id":id,"label":label,"kind":kind,"min":min,"max":max,"scale":scale,"unit":unit}),
                        PlaneControl::Steps { id,label,steps,.. } => json!({"id":id,"label":label,"kind":"step_sequencer","step_count":steps}),
                        PlaneControl::Trigger { id,label,.. } => json!({"id":id,"label":label,"kind":"trigger"}),
                        PlaneControl::PianoRoll { id,label,steps,pitch_min,pitch_max,tracks,sequencer } => {
                            let step_count=sequencer.pattern_length(sequencer.active_pattern(),tracks[0]).unwrap_or(*steps);
                            json!({"id":id,"label":label,"kind":"piano_roll","step_count":step_count,"pitch_min":pitch_min,"pitch_max":pitch_max,"voices":tracks.len()})
                        },
                        PlaneControl::DrumGrid { id,label,kit_name,steps,lanes,.. } => {
                            let rows=lanes.iter().map(|lane| json!({"id":lane.id,"label":lane.label,"note":lane.note})).collect::<Vec<_>>();
                            json!({"id":id,"label":label,"kind":"drum_grid","kit":kit_name,"step_count":steps,"rows":rows})
                        }
                        PlaneControl::Adsr { id,label,parameters } => {
                            let parameters=parameters.iter().map(|parameter| json!({"name":parameter.name,"control":parameter.control,"min":parameter.min,"max":parameter.max,"scale":parameter.scale,"unit":parameter.unit})).collect::<Vec<_>>();
                            json!({"id":id,"label":label,"kind":"adsr","parameters":parameters})
                        }
                        PlaneControl::Patterns { id,label,sequencer,default_boundary } => json!({"id":id,"label":label,"kind":"pattern_selector","patterns":sequencer.pattern_names(),"boundaries":["immediate","beat","bar","cycle"],"default_boundary":default_boundary}),
                        PlaneControl::Presets { id,label,names,entries,.. } => json!({"id":id,"label":label,"kind":"preset_selector","presets":names,"entries":entries}),
                    })
                    .collect::<Vec<_>>();
                json!({"id":plane.id,"label":plane.label,"layout":plane.layout,"roles":plane.roles,"read_only_roles":plane.read_only_roles,"controls":controls})
            })
            .collect()
    }

    fn snapshot(
        &self,
        revision: u64,
        epoch: &str,
        schema_revision: u64,
        include_schema: bool,
    ) -> Value {
        let runtime = self.runtime.borrow();
        let controls = self
            .controls
            .iter()
            .map(|(name, definition)| {
                let raw = runtime
                    .handles
                    .get(&definition.target)
                    .map_or(definition.min, ControlHandle::effective_value);
                let value = match definition.kind {
                    ControlKind::Toggle => {
                        Value::Bool((raw - definition.max).abs() < (raw - definition.min).abs())
                    }
                    ControlKind::Choice => {
                        let index = if definition.choices.len() <= 1 {
                            0
                        } else {
                            (((raw - definition.min) / (definition.max - definition.min)
                                * (definition.choices.len() - 1) as f64)
                                .round() as usize)
                                .min(definition.choices.len() - 1)
                        };
                        Value::String(definition.choices.get(index).cloned().unwrap_or_default())
                    }
                    _ => json!(raw),
                };
                (
                    name.clone(),
                    json!({
                        "name": name,
                        "label": definition.label,
                        "group": definition.group,
                        "type": definition.kind,
                        "min": definition.min,
                        "max": definition.max,
                        "choices": definition.choices,
                        "value": value,
                    }),
                )
            })
            .collect::<serde_json::Map<_, _>>();
        let definitions = self.plane_definitions();
        let planes = self.planes.borrow();
        let mut values = serde_json::Map::new();
        for plane in planes.values() {
            for control in &plane.controls {
                let (id, value) = match control {
                    PlaneControl::Range {
                        id,
                        target,
                        kind,
                        min,
                        max,
                        ..
                    } => {
                        let raw = runtime
                            .handles
                            .get(target)
                            .map_or(*min, ControlHandle::effective_value);
                        let value = if *kind == "toggle" {
                            Value::Bool((raw - max).abs() < (raw - min).abs())
                        } else {
                            json!(raw)
                        };
                        (id, value)
                    }
                    PlaneControl::Steps {
                        id,
                        sequencer,
                        track,
                        pattern,
                        steps,
                        ..
                    } => {
                        let authored = sequencer.pattern_at(*pattern).unwrap_or_default();
                        let enabled = (0..*steps)
                            .map(|step| authored.velocities[*track][step] > 0.0)
                            .collect::<Vec<_>>();
                        (
                            id,
                            json!({"steps":enabled,"position":sequencer.current_step()}),
                        )
                    }
                    PlaneControl::Trigger { id, serial, .. } => {
                        (id, json!({"serial":serial.get()}))
                    }
                    PlaneControl::PianoRoll {
                        id,
                        sequencer,
                        tracks,
                        ..
                    } => {
                        let pattern = sequencer.active_pattern();
                        let step_count = sequencer.pattern_length(pattern, tracks[0]).unwrap_or(16);
                        let notes = sequencer
                            .note_events(pattern, tracks[0])
                            .into_iter()
                            .map(|note| json!({"id":note.id,"step":note.step,"pitch":note.pitch,"velocity":note.velocity,"duration":note.duration,"legato":note.legato}))
                            .collect::<Vec<_>>();
                        (
                            id,
                            json!({"pattern":pattern,"step_count":step_count,"notes":notes,"position":sequencer.current_step()}),
                        )
                    }
                    PlaneControl::DrumGrid {
                        id,
                        kit_name,
                        sequencer,
                        lanes,
                        ..
                    } => {
                        let pattern = sequencer.active_pattern();
                        let rows = lanes.iter().map(|lane| {
                            let hits = sequencer.note_events(pattern, lane.track).into_iter()
                                .map(|hit| json!({"id":hit.id,"step":hit.step,"velocity":hit.velocity,"duration":hit.duration}))
                                .collect::<Vec<_>>();
                            json!({"id":lane.id,"muted":sequencer.muted(lane.track),"hits":hits})
                        }).collect::<Vec<_>>();
                        (
                            id,
                            json!({"current_kit":kit_name,"pattern":pattern,"rows":rows,"position":sequencer.current_step()}),
                        )
                    }
                    PlaneControl::Adsr { id, parameters, .. } => {
                        let values = parameters
                            .iter()
                            .map(|parameter| {
                                let value = runtime
                                    .handles
                                    .get(&parameter.target)
                                    .map_or(parameter.min, ControlHandle::effective_value);
                                (parameter.name.to_owned(), json!(value))
                            })
                            .collect::<serde_json::Map<_, _>>();
                        (id, Value::Object(values))
                    }
                    PlaneControl::Patterns { id, sequencer, .. } => {
                        let names = sequencer.pattern_names();
                        let active = sequencer.active_pattern();
                        let pending = sequencer.queued_pattern();
                        let quantization = sequencer.queued_pattern_quantization();
                        let boundary = if quantization == 0 {
                            "cycle"
                        } else if quantization == 1 {
                            "immediate"
                        } else if quantization == sequencer.steps_per_beat as usize {
                            "beat"
                        } else {
                            "bar"
                        };
                        (
                            id,
                            json!({"active":active,"active_name":names.get(active),"pending":pending,"pending_name":pending.and_then(|index|names.get(index)),"boundary":boundary}),
                        )
                    }
                    PlaneControl::Presets {
                        id,
                        target,
                        names,
                        status,
                        ..
                    } => {
                        let active = status.as_ref().map_or_else(
                            || {
                                runtime
                                    .handles
                                    .get(target)
                                    .map_or(0.0, ControlHandle::effective_value)
                                    .round()
                                    .clamp(0.0, names.len().saturating_sub(1) as f64)
                                    as usize
                            },
                            |status| status.active(),
                        );
                        let pending = status
                            .as_ref()
                            .filter(|status| status.pending())
                            .map(|status| status.requested());
                        let failed = status.as_ref().and_then(|status| status.failure());
                        (
                            id,
                            json!({"active":active,"active_name":names.get(active),"pending":pending,"failed":failed}),
                        )
                    }
                };
                values.insert(format!("{}/{}", plane.id, id), value);
            }
        }
        let role_controls = self
            .roles
            .borrow()
            .iter()
            .map(|(role, definition)| {
                let controls = definition.controls.as_ref().map(|controls| {
                    Value::Array(controls.iter().cloned().map(Value::String).collect())
                });
                (role.clone(), controls.unwrap_or(Value::Null))
            })
            .collect::<serde_json::Map<_, _>>();
        let mut state = json!({ "name": self.name, "controls": controls, "values": values, "audio": self.audio_telemetry, "_role_controls": role_controls });
        if include_schema {
            state["planes"] = Value::Array(definitions);
        }
        json!({
            "type": if include_schema { "state.snapshot" } else { "state.updated" },
            "epoch": epoch,
            "revision": revision,
            "schema_version": 3,
            "schema_revision": schema_revision,
            "state": state
        })
    }

    fn plane_action(
        &mut self,
        _lua: &mlua::Lua,
        role: Option<&str>,
        message: &Value,
    ) -> Result<(), DispatchError> {
        let plane_id = message
            .get("plane")
            .and_then(Value::as_str)
            .ok_or_else(|| DispatchError::bad_request("plane command requires plane"))?;
        let control_id = message
            .get("control")
            .and_then(Value::as_str)
            .ok_or_else(|| DispatchError::bad_request("plane command requires control"))?;
        let planes = self.planes.borrow();
        let plane = planes
            .get(plane_id)
            .ok_or_else(|| DispatchError::not_found("unknown control plane"))?;
        if !role.is_some_and(|role| plane.roles.contains(role)) {
            return Err(DispatchError::forbidden(
                "control plane is not assigned to this performer",
            ));
        }
        let control = plane
            .controls
            .iter()
            .find(|control| match control {
                PlaneControl::Range { id, .. }
                | PlaneControl::Steps { id, .. }
                | PlaneControl::Trigger { id, .. }
                | PlaneControl::PianoRoll { id, .. }
                | PlaneControl::DrumGrid { id, .. }
                | PlaneControl::Adsr { id, .. }
                | PlaneControl::Patterns { id, .. }
                | PlaneControl::Presets { id, .. } => id == control_id,
            })
            .cloned()
            .ok_or_else(|| DispatchError::not_found("unknown plane control"))?;
        drop(planes);
        match (message.get("type").and_then(Value::as_str), control) {
            (
                Some("step.toggle"),
                PlaneControl::Steps {
                    sequencer,
                    track,
                    pattern,
                    steps,
                    ..
                },
            ) => {
                let step = message
                    .get("step")
                    .and_then(Value::as_u64)
                    .and_then(|v| usize::try_from(v).ok())
                    .filter(|v| *v < steps)
                    .ok_or_else(|| DispatchError::bad_request("invalid sequencer step"))?;
                sequencer
                    .toggle_pattern(pattern, track, step)
                    .map_err(DispatchError::bad_request)
            }
            (
                Some("trigger.fire"),
                PlaneControl::Trigger {
                    callback, serial, ..
                },
            ) => {
                callback.call::<()>(()).map_err(lua_error)?;
                serial.set(serial.get().wrapping_add(1));
                Ok(())
            }
            (
                Some(kind @ ("note.create" | "note.update" | "note.delete")),
                PlaneControl::PianoRoll {
                    sequencer,
                    tracks,
                    pitch_min,
                    pitch_max,
                    ..
                },
            ) => {
                let pattern = message
                    .get("pattern")
                    .and_then(Value::as_u64)
                    .and_then(|value| usize::try_from(value).ok())
                    .unwrap_or_else(|| sequencer.active_pattern());
                if pattern >= sequencer.pattern_names().len() {
                    return Err(DispatchError::bad_request("unknown piano-roll pattern"));
                }
                let steps = sequencer
                    .pattern_length(pattern, tracks[0])
                    .ok_or_else(|| {
                        DispatchError::bad_request("unknown piano-roll pattern length")
                    })?;
                let id = message
                    .get("id")
                    .and_then(Value::as_u64)
                    .and_then(|value| u32::try_from(value).ok());
                sequencer
                    .edit_note_events(pattern, &tracks, steps, |notes| match kind {
                        "note.delete" => {
                            let id = id.ok_or_else(|| "note.delete requires an ID".to_owned())?;
                            let before = notes.len();
                            notes.retain(|note| note.id != id);
                            (notes.len() != before)
                                .then_some(())
                                .ok_or_else(|| "unknown note ID".to_owned())
                        }
                        "note.create" => {
                            let next = notes
                                .iter()
                                .map(|note| note.id)
                                .max()
                                .unwrap_or(0)
                                .checked_add(1)
                                .ok_or_else(|| "note ID space exhausted".to_owned())?;
                            notes.push(parse_note_command(message, next, pitch_min, pitch_max)?);
                            Ok(())
                        }
                        "note.update" => {
                            let id = id.ok_or_else(|| "note.update requires an ID".to_owned())?;
                            let note = notes
                                .iter_mut()
                                .find(|note| note.id == id)
                                .ok_or_else(|| "unknown note ID".to_owned())?;
                            *note = parse_note_command(message, id, pitch_min, pitch_max)?;
                            Ok(())
                        }
                        _ => unreachable!(),
                    })
                    .map_err(DispatchError::bad_request)
            }
            (
                Some(kind @ ("drum.hit.toggle" | "drum.hit.velocity" | "drum.mute")),
                PlaneControl::DrumGrid {
                    sequencer,
                    lanes,
                    steps,
                    ..
                },
            ) => {
                let row = message
                    .get("row")
                    .and_then(Value::as_str)
                    .ok_or_else(|| DispatchError::bad_request("drum command requires row"))?;
                let lane = lanes
                    .iter()
                    .find(|lane| lane.id == row)
                    .ok_or_else(|| DispatchError::bad_request("unknown drum row"))?;
                if kind == "drum.mute" {
                    let muted = message
                        .get("muted")
                        .and_then(Value::as_bool)
                        .ok_or_else(|| DispatchError::bad_request("drum mute requires muted"))?;
                    return sequencer
                        .set_muted(lane.track, muted)
                        .map_err(DispatchError::bad_request);
                }
                let step = message
                    .get("step")
                    .and_then(Value::as_u64)
                    .and_then(|value| u8::try_from(value).ok())
                    .filter(|step| usize::from(*step) < steps)
                    .ok_or_else(|| DispatchError::bad_request("invalid drum step"))?;
                let pattern = message
                    .get("pattern")
                    .and_then(Value::as_u64)
                    .and_then(|value| usize::try_from(value).ok())
                    .unwrap_or_else(|| sequencer.active_pattern());
                sequencer
                    .edit_note_events(pattern, &[lane.track], steps, |events| {
                        if kind == "drum.hit.toggle" {
                            if let Some(index) = events.iter().position(|hit| hit.step == step) {
                                events.remove(index);
                            } else {
                                let id = events
                                    .iter()
                                    .map(|hit| hit.id)
                                    .max()
                                    .unwrap_or(0)
                                    .saturating_add(1);
                                events.push(NoteEvent {
                                    id,
                                    step,
                                    pitch: lane.note,
                                    velocity: 100,
                                    duration: 1,
                                    legato: false,
                                });
                            }
                        } else {
                            let velocity = message
                                .get("velocity")
                                .and_then(Value::as_u64)
                                .and_then(|value| u8::try_from(value).ok())
                                .filter(|value| *value > 0 && *value <= 127)
                                .ok_or_else(|| "drum velocity must be in 1..127".to_string())?;
                            let hit = events.iter_mut().find(|hit| hit.step == step).ok_or_else(
                                || "cannot set velocity for an empty drum step".to_string(),
                            )?;
                            hit.velocity = velocity;
                        }
                        Ok(())
                    })
                    .map_err(DispatchError::bad_request)
            }
            (
                Some("pattern.select"),
                PlaneControl::Patterns {
                    sequencer,
                    default_boundary,
                    ..
                },
            ) => {
                let pattern = message
                    .get("pattern")
                    .and_then(Value::as_u64)
                    .and_then(|value| usize::try_from(value).ok())
                    .ok_or_else(|| {
                        DispatchError::bad_request("pattern.select requires a pattern index")
                    })?;
                let boundary = message
                    .get("boundary")
                    .and_then(Value::as_str)
                    .unwrap_or(&default_boundary);
                let quantization = match boundary {
                    "immediate" => 1,
                    "beat" => sequencer.steps_per_beat as usize,
                    "bar" => sequencer.steps_per_beat as usize * 4,
                    "cycle" => 0,
                    _ => return Err(DispatchError::bad_request("unknown pattern boundary")),
                };
                sequencer
                    .queue_pattern_quantized(pattern, quantization)
                    .map_err(DispatchError::bad_request)
            }
            (Some("adsr.set"), PlaneControl::Adsr { parameters, .. }) => {
                let name = message
                    .get("parameter")
                    .and_then(Value::as_str)
                    .ok_or_else(|| DispatchError::bad_request("adsr.set requires a parameter"))?;
                let parameter = parameters
                    .iter()
                    .find(|parameter| parameter.name == name)
                    .ok_or_else(|| DispatchError::bad_request("unknown ADSR parameter"))?;
                self.set_control(
                    Some("__meldritch_trusted"),
                    &parameter.control,
                    message.get("value").cloned().unwrap_or(Value::Null),
                )
            }
            (
                Some("preset.select"),
                PlaneControl::Presets {
                    id,
                    names,
                    entries,
                    status,
                    ..
                },
            ) => {
                let index = match message.get("preset") {
                    Some(Value::String(name)) => entries
                        .iter()
                        .position(|candidate| candidate.id == *name)
                        .or_else(|| names.iter().position(|candidate| candidate == name))
                        .ok_or_else(|| DispatchError::bad_request("unknown configured preset"))?,
                    Some(value) => value
                        .as_u64()
                        .and_then(|value| usize::try_from(value).ok())
                        .filter(|value| *value < names.len())
                        .ok_or_else(|| DispatchError::bad_request("invalid preset index"))?,
                    None => {
                        return Err(DispatchError::bad_request("preset.select requires preset"));
                    }
                };
                if let Some(status) = status {
                    status.request(index);
                }
                self.set_control(
                    Some("__meldritch_trusted"),
                    &id,
                    Value::String(names[index].clone()),
                )
            }
            _ => Err(DispatchError::bad_request(
                "command does not match plane control type",
            )),
        }
    }

    fn poll_external_changes(&mut self) -> bool {
        let mut changed = false;
        let mut runtime = self.runtime.borrow_mut();
        for (name, definition) in &self.controls {
            let value = runtime.handles[&definition.target].effective_value();
            let previous = runtime.observed.entry(name.clone()).or_insert(value);
            if value.to_bits() != previous.to_bits() {
                *previous = value;
                changed = true;
            }
        }
        for value in self.plane_observations() {
            match runtime.observed_planes.entry(value.0) {
                std::collections::btree_map::Entry::Vacant(entry) => {
                    entry.insert(value.1);
                }
                std::collections::btree_map::Entry::Occupied(mut entry)
                    if *entry.get() != value.1 =>
                {
                    entry.insert(value.1);
                    changed = true;
                }
                _ => {}
            }
        }
        changed
    }

    fn plane_observations(&self) -> Vec<(String, u64)> {
        let mut values = Vec::new();
        for plane in self.planes.borrow().values() {
            for control in &plane.controls {
                let value = match control {
                    PlaneControl::Steps {
                        id,
                        sequencer,
                        track,
                        pattern,
                        steps,
                        ..
                    } => {
                        let authored = sequencer.pattern_at(*pattern).unwrap_or_default();
                        let mut value = sequencer.current_step() as u64;
                        for step in 0..(*steps).min(56) {
                            if authored.velocities[*track][step] > 0.0 {
                                value |= 1_u64 << (step + 8);
                            }
                        }
                        (format!("{}/{}", plane.id, id), value)
                    }
                    PlaneControl::DrumGrid {
                        id,
                        sequencer,
                        lanes,
                        ..
                    } => {
                        let pattern = sequencer.active_pattern();
                        let mut value = (pattern as u64) << 56 | sequencer.current_step() as u64;
                        for lane in lanes {
                            value = value.wrapping_mul(1_099_511_628_211)
                                ^ (lane.track as u64) << 40
                                ^ (sequencer.muted(lane.track) as u64) << 55;
                            for hit in sequencer.note_events(pattern, lane.track) {
                                value = value
                                    .wrapping_mul(1_099_511_628_211)
                                    .wrapping_add(u64::from(hit.id))
                                    ^ (u64::from(hit.step) << 8)
                                    ^ (u64::from(hit.velocity) << 24);
                            }
                        }
                        (format!("{}/{}", plane.id, id), value)
                    }
                    PlaneControl::Trigger { id, serial, .. } => {
                        (format!("{}/{}", plane.id, id), serial.get())
                    }
                    PlaneControl::PianoRoll {
                        id,
                        sequencer,
                        tracks,
                        ..
                    } => {
                        let pattern = sequencer.active_pattern();
                        let length = sequencer.pattern_length(pattern, tracks[0]).unwrap_or(0);
                        let mut value = (pattern as u64) << 56
                            | (length as u64) << 48
                            | sequencer.current_step() as u64;
                        for note in sequencer.note_events(pattern, tracks[0]) {
                            value = value
                                .wrapping_mul(1_099_511_628_211)
                                .wrapping_add(u64::from(note.id))
                                ^ (u64::from(note.step) << 8)
                                ^ (u64::from(note.pitch) << 16)
                                ^ (u64::from(note.velocity) << 24)
                                ^ (u64::from(note.duration) << 32);
                        }
                        (format!("{}/{}", plane.id, id), value)
                    }
                    PlaneControl::Patterns { id, sequencer, .. } => {
                        let value = sequencer.active_pattern() as u64
                            | (sequencer.queued_pattern().unwrap_or(255) as u64) << 16
                            | (sequencer.queued_pattern_quantization() as u64) << 32;
                        (format!("{}/{}", plane.id, id), value)
                    }
                    PlaneControl::Presets {
                        id,
                        status: Some(status),
                        ..
                    } => {
                        let value = status.active() as u64
                            ^ (status.requested() as u64).rotate_left(16)
                            ^ status.generation().rotate_left(32)
                            ^ status.request_epoch().rotate_left(48)
                            ^ u64::from(status.pending()).rotate_left(63);
                        (format!("{}/{}", plane.id, id), value)
                    }
                    PlaneControl::Range { .. }
                    | PlaneControl::Adsr { .. }
                    | PlaneControl::Presets { .. } => continue,
                };
                values.push(value);
            }
        }
        values
    }

    fn sync_plane_observations(&self) {
        self.runtime.borrow_mut().observed_planes = self.plane_observations().into_iter().collect();
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct ClientInfo {
    pub id: u64,
    pub remote: String,
    pub role: Option<String>,
    pub performer: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ParsedRequest {
    pub id: u64,
    pub method: String,
    pub route: String,
    pub path: String,
    pub params: BTreeMap<String, String>,
    pub query: BTreeMap<String, String>,
    pub headers: BTreeMap<String, String>,
    pub json: Option<Value>,
    pub raw: String,
    pub remote: String,
    pub role: Option<String>,
}

#[derive(Clone, Debug)]
pub struct HttpResponse {
    pub status: u16,
    pub headers: BTreeMap<String, String>,
    pub body: ResponseBody,
}

#[derive(Clone, Debug)]
pub enum ResponseBody {
    Json(Value),
    Text(String),
    Empty,
}

impl HttpResponse {
    pub fn json(status: u16, value: Value) -> Self {
        Self {
            status,
            headers: BTreeMap::new(),
            body: ResponseBody::Json(value),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Serialize)]
pub struct Diagnostics {
    pub active_http_requests: usize,
    pub active_websockets: usize,
    pub queue_depth: usize,
    pub dispatches: u64,
    pub handler_errors: u64,
    pub broadcasts: u64,
    pub rejected: u64,
    pub dropped_messages: u64,
    pub revision: u64,
    pub last_dispatch_micros: u64,
    pub last_handler_micros: u64,
}

#[derive(Default)]
struct AtomicDiagnostics {
    active_http_requests: AtomicUsize,
    active_websockets: AtomicUsize,
    queue_depth: AtomicUsize,
    dispatches: AtomicU64,
    handler_errors: AtomicU64,
    broadcasts: AtomicU64,
    rejected: AtomicU64,
    dropped_messages: AtomicU64,
    revision: AtomicU64,
    last_dispatch_micros: AtomicU64,
    last_handler_micros: AtomicU64,
}

impl AtomicDiagnostics {
    fn snapshot(&self) -> Diagnostics {
        Diagnostics {
            active_http_requests: self.active_http_requests.load(Ordering::Relaxed),
            active_websockets: self.active_websockets.load(Ordering::Relaxed),
            queue_depth: self.queue_depth.load(Ordering::Relaxed),
            dispatches: self.dispatches.load(Ordering::Relaxed),
            handler_errors: self.handler_errors.load(Ordering::Relaxed),
            broadcasts: self.broadcasts.load(Ordering::Relaxed),
            rejected: self.rejected.load(Ordering::Relaxed),
            dropped_messages: self.dropped_messages.load(Ordering::Relaxed),
            revision: self.revision.load(Ordering::Acquire),
            last_dispatch_micros: self.last_dispatch_micros.load(Ordering::Relaxed),
            last_handler_micros: self.last_handler_micros.load(Ordering::Relaxed),
        }
    }
}

enum Dispatch {
    Http {
        request: ParsedRequest,
        response: oneshot::Sender<HttpResponse>,
    },
    Connected {
        client: ClientInfo,
    },
    Message {
        client: ClientInfo,
        message: Value,
    },
    Disconnected {
        client: ClientInfo,
    },
}

#[derive(Clone)]
struct Shared {
    queue: SyncSender<Dispatch>,
    snapshot: Arc<RwLock<Arc<Value>>>,
    updates: broadcast::Sender<Outbound>,
    diagnostics: Arc<AtomicDiagnostics>,
    config: ServerConfig,
    credentials: Arc<BTreeMap<String, String>>,
    anonymous_role: Option<String>,
    next_request: Arc<AtomicU64>,
    next_client: Arc<AtomicU64>,
    websocket_path: Option<String>,
    connection_shutdown: broadcast::Sender<()>,
    epoch: Arc<str>,
}

#[derive(Clone)]
struct Outbound {
    target: Option<u64>,
    value: Arc<Value>,
}

pub struct ControlExecutor {
    application: Application,
    receiver: mpsc::Receiver<Dispatch>,
    shared: Shared,
    schema_fingerprint: String,
    schema_revision: u64,
}

impl ControlExecutor {
    pub fn update_audio_telemetry(&mut self, telemetry: crate::device::AudioTelemetry) {
        if self.application.audio_telemetry.as_ref() != Some(&telemetry) {
            self.application.audio_telemetry = Some(telemetry);
            self.publish();
        }
    }

    pub fn poll(&mut self, lua: &mlua::Lua, limit: usize) {
        let started = Instant::now();
        let mut processed = 0;
        while processed < limit {
            let Ok(dispatch) = self.receiver.try_recv() else {
                break;
            };
            self.shared
                .diagnostics
                .queue_depth
                .fetch_sub(1, Ordering::Relaxed);
            self.dispatch(lua, dispatch);
            processed += 1;
        }
        if self.application.poll_external_changes()
            || self.application.schema_fingerprint() != self.schema_fingerprint
        {
            self.publish();
        }
        if processed > 0 {
            self.shared.diagnostics.last_dispatch_micros.store(
                started.elapsed().as_micros().min(u128::from(u64::MAX)) as u64,
                Ordering::Relaxed,
            );
        }
    }

    fn dispatch(&mut self, lua: &mlua::Lua, dispatch: Dispatch) {
        self.shared
            .diagnostics
            .dispatches
            .fetch_add(1, Ordering::Relaxed);
        let started = Instant::now();
        match dispatch {
            Dispatch::Http { request, response } => {
                if response.is_closed() {
                    return;
                }
                let result = self.call_http(lua, &request);
                if result.status >= 500 {
                    self.shared
                        .diagnostics
                        .handler_errors
                        .fetch_add(1, Ordering::Relaxed);
                }
                let _ = response.send(result);
            }
            Dispatch::Connected { client } => {
                if let Some(callback) = self
                    .application
                    .websocket
                    .as_ref()
                    .and_then(|ws| ws.connected.clone())
                {
                    match callback.call::<mlua::Value>(json_to_lua(lua, &json!(client))) {
                        Ok(value) => self.send_lua_value(lua, client.id, value),
                        Err(error) => self.log_callback_error("WebSocket connected", error),
                    }
                }
            }
            Dispatch::Message {
                mut client,
                message,
            } => {
                if message.get("type").and_then(Value::as_str) == Some("authenticate") {
                    client.role = message
                        .get("token")
                        .and_then(Value::as_str)
                        .and_then(|token| self.application.credentials.get(token).cloned());
                } else if message.get("type").and_then(Value::as_str) == Some("control.set") {
                    let result = message
                        .get("name")
                        .and_then(Value::as_str)
                        .ok_or_else(|| DispatchError::bad_request("control.set requires a name"))
                        .and_then(|name| {
                            self.application.set_control(
                                client.role.as_deref(),
                                name,
                                message.get("value").cloned().unwrap_or(Value::Bool(true)),
                            )
                        });
                    match result {
                        Ok(()) => self.publish(),
                        Err(error) => {
                            let message = error.message.clone();
                            self.log_dispatch_error("WebSocket control", error);
                            let _ = self.shared.updates.send(Outbound {
                                target: Some(client.id),
                                value: Arc::new(json!({"type":"error", "error":message})),
                            });
                        }
                    }
                } else if matches!(
                    message.get("type").and_then(Value::as_str),
                    Some(
                        "step.toggle"
                            | "trigger.fire"
                            | "note.create"
                            | "note.update"
                            | "note.delete"
                            | "pattern.select"
                            | "adsr.set"
                            | "preset.select"
                            | "drum.hit.toggle"
                            | "drum.hit.velocity"
                            | "drum.mute"
                    )
                ) {
                    match self
                        .application
                        .plane_action(lua, client.role.as_deref(), &message)
                    {
                        Ok(()) => self.publish(),
                        Err(error) => {
                            let message = error.message.clone();
                            self.log_dispatch_error("WebSocket plane command", error);
                            let _ = self.shared.updates.send(Outbound {
                                target: Some(client.id),
                                value: Arc::new(json!({"type":"error","error":message})),
                            });
                        }
                    }
                } else if let Some(callback) = self
                    .application
                    .websocket
                    .as_ref()
                    .and_then(|ws| ws.message.clone())
                {
                    let args = (json_to_lua(lua, &json!(client)), json_to_lua(lua, &message));
                    match callback.call::<mlua::Value>(args) {
                        Ok(value) => self.send_lua_value(lua, client.id, value),
                        Err(error) => self.log_callback_error("WebSocket message", error),
                    }
                    if self.application.poll_external_changes() {
                        self.publish();
                    }
                }
            }
            Dispatch::Disconnected { client } => {
                if let Some(callback) = self
                    .application
                    .websocket
                    .as_ref()
                    .and_then(|ws| ws.disconnected.clone())
                    && let Err(error) = callback.call::<()>(json_to_lua(lua, &json!(client)))
                {
                    self.log_callback_error("WebSocket disconnected", error);
                }
            }
        }
        self.shared.diagnostics.last_handler_micros.store(
            started.elapsed().as_micros().min(u128::from(u64::MAX)) as u64,
            Ordering::Relaxed,
        );
    }

    fn call_http(&mut self, lua: &mlua::Lua, request: &ParsedRequest) -> HttpResponse {
        if request.route == "/api/state" {
            let snapshot = Arc::clone(&self.shared.snapshot.read().unwrap());
            return HttpResponse::json(
                200,
                project_snapshot(&snapshot, request.role.as_deref(), &snapshot),
            );
        }
        if request.route == "/api/control/{name}" && request.method == "POST" {
            let Some(name) = request.params.get("name") else {
                return DispatchError::bad_request("missing control name").response();
            };
            let value = request
                .json
                .as_ref()
                .and_then(|body| body.get("value"))
                .cloned()
                .unwrap_or(Value::Bool(true));
            return match self
                .application
                .set_control(request.role.as_deref(), name, value)
            {
                Ok(()) => {
                    self.publish();
                    HttpResponse::json(
                        200,
                        json!({"control": name, "revision": self.shared.diagnostics.revision.load(Ordering::Acquire)}),
                    )
                }
                Err(error) => error.response(),
            };
        }
        let Some(route) = self.application.routes.iter().find(|route| {
            route.method.as_str() == request.method && normalize_path(&route.path) == request.route
        }) else {
            return DispatchError::not_found("route not found").response();
        };
        let request_value = serde_json::to_value(request).unwrap_or(Value::Null);
        match route
            .handler
            .call::<mlua::MultiValue>(json_to_lua(lua, &request_value))
        {
            Ok(values) => {
                let response = lua_response(lua, values);
                if self.application.poll_external_changes() {
                    self.publish();
                }
                response
            }
            Err(error) => {
                eprintln!(
                    "network Lua handler failed for request {}: {}",
                    request.id,
                    sanitize_lua_error(&error)
                );
                DispatchError::internal("Lua request handler failed").response()
            }
        }
    }

    fn publish(&mut self) {
        self.application.sync_plane_observations();
        let revision = self
            .shared
            .diagnostics
            .revision
            .fetch_add(1, Ordering::AcqRel)
            + 1;
        let fingerprint = self.application.schema_fingerprint();
        let schema_changed = fingerprint != self.schema_fingerprint;
        if schema_changed {
            self.schema_fingerprint = fingerprint;
            self.schema_revision += 1;
        }
        let snapshot = Arc::new(self.application.snapshot(
            revision,
            &self.shared.epoch,
            self.schema_revision,
            true,
        ));
        *self.shared.snapshot.write().unwrap() = Arc::clone(&snapshot);
        let update = if schema_changed {
            snapshot
        } else {
            Arc::new(self.application.snapshot(
                revision,
                &self.shared.epoch,
                self.schema_revision,
                false,
            ))
        };
        let _ = self.shared.updates.send(Outbound {
            target: None,
            value: update,
        });
        self.shared
            .diagnostics
            .broadcasts
            .fetch_add(1, Ordering::Relaxed);
    }

    fn send_lua_value(&self, lua: &mlua::Lua, client: u64, value: mlua::Value) {
        use mlua::LuaSerdeExt;
        if matches!(value, mlua::Value::Nil) {
            return;
        }
        match lua.from_value::<Value>(value) {
            Ok(value) => {
                let _ = self.shared.updates.send(Outbound {
                    target: Some(client),
                    value: Arc::new(value),
                });
            }
            Err(error) => self.log_callback_error("WebSocket response serialization", error),
        }
    }

    fn log_callback_error(&self, context: &str, error: mlua::Error) {
        self.shared
            .diagnostics
            .handler_errors
            .fetch_add(1, Ordering::Relaxed);
        eprintln!("{context} callback failed: {}", sanitize_lua_error(&error));
    }

    fn log_dispatch_error(&self, context: &str, error: DispatchError) {
        self.shared
            .diagnostics
            .handler_errors
            .fetch_add(1, Ordering::Relaxed);
        eprintln!("{context} rejected: {}", error.message);
    }

    pub fn diagnostics(&self) -> Diagnostics {
        self.shared.diagnostics.snapshot()
    }
}

pub struct ServerSession {
    address: SocketAddr,
    shutdown: Option<oneshot::Sender<()>>,
    thread: Option<thread::JoinHandle<Result<(), String>>>,
    diagnostics: Arc<AtomicDiagnostics>,
    connection_shutdown: broadcast::Sender<()>,
}

impl ServerSession {
    pub fn address(&self) -> SocketAddr {
        self.address
    }
    pub fn diagnostics(&self) -> Diagnostics {
        self.diagnostics.snapshot()
    }
    pub fn shutdown(mut self) -> Result<(), String> {
        self.finish()
    }
    fn finish(&mut self) -> Result<(), String> {
        let _ = self.connection_shutdown.send(());
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        self.thread.take().map_or(Ok(()), |thread| {
            thread
                .join()
                .map_err(|_| "network server thread panicked".to_owned())?
        })
    }
}

impl Drop for ServerSession {
    fn drop(&mut self) {
        let _ = self.finish();
    }
}

pub fn start(application: Application) -> Result<(ServerSession, ControlExecutor), String> {
    validate_config(&application.config)?;
    validate_application(&application)?;
    let (sender, receiver) = mpsc::sync_channel(application.config.queue_capacity);
    let (updates, _) = broadcast::channel(64);
    let (connection_shutdown, _) = broadcast::channel(1);
    let diagnostics = Arc::new(AtomicDiagnostics::default());
    let epoch: Arc<str> = format!(
        "{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    )
    .into();
    let schema_fingerprint = application.schema_fingerprint();
    let initial = Arc::new(application.snapshot(0, &epoch, 1, true));
    let shared = Shared {
        queue: sender,
        snapshot: Arc::new(RwLock::new(initial)),
        updates,
        diagnostics: Arc::clone(&diagnostics),
        config: application.config.clone(),
        credentials: Arc::new(application.credentials.clone()),
        anonymous_role: application.anonymous_role.clone(),
        next_request: Arc::new(AtomicU64::new(0)),
        next_client: Arc::new(AtomicU64::new(0)),
        websocket_path: application
            .websocket
            .as_ref()
            .map(|websocket| normalize_path(&websocket.path)),
        connection_shutdown: connection_shutdown.clone(),
        epoch,
    };
    let network = shared.clone();
    let network_spec = NetworkSpec::from_application(&application);
    let (ready_tx, ready_rx) = mpsc::sync_channel(1);
    let startup_errors = ready_tx.clone();
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let thread = thread::Builder::new()
        .name("meldritch-http".into())
        .spawn(move || {
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .worker_threads(2)
                .thread_name("meldritch-http-worker")
                .build()
                .map_err(|error| error.to_string())?;
            let result = runtime.block_on(run_server(network, network_spec, shutdown_rx, ready_tx));
            if let Err(error) = &result {
                let _ = startup_errors.try_send(Err(error.clone()));
            }
            result
        })
        .map_err(|error| error.to_string())?;
    let address = ready_rx
        .recv_timeout(Duration::from_secs(5))
        .map_err(|_| "timed out while starting HTTP server".to_owned())??;
    {
        let mut runtime = application.runtime.borrow_mut();
        runtime.observed = application
            .controls
            .iter()
            .filter_map(|(name, definition)| {
                runtime
                    .handles
                    .get(&definition.target)
                    .map(|handle| (name.clone(), handle.effective_value()))
            })
            .collect();
    }
    Ok((
        ServerSession {
            address,
            shutdown: Some(shutdown_tx),
            thread: Some(thread),
            diagnostics,
            connection_shutdown,
        },
        ControlExecutor {
            application,
            receiver,
            shared,
            schema_fingerprint,
            schema_revision: 1,
        },
    ))
}

#[derive(Clone)]
struct NetworkSpec {
    routes: Vec<(Method, String)>,
    websocket_path: Option<String>,
}

impl NetworkSpec {
    fn from_application(application: &Application) -> Self {
        let mut routes = application
            .routes
            .iter()
            .map(|route| (route.method.clone(), normalize_path(&route.path)))
            .collect::<Vec<_>>();
        routes.push((Method::GET, "/api/state".into()));
        routes.push((Method::POST, "/api/control/{name}".into()));
        Self {
            routes,
            websocket_path: application
                .websocket
                .as_ref()
                .map(|ws| normalize_path(&ws.path)),
        }
    }
}

async fn run_server(
    shared: Shared,
    spec: NetworkSpec,
    shutdown: oneshot::Receiver<()>,
    ready: SyncSender<Result<SocketAddr, String>>,
) -> Result<(), String> {
    let ip: IpAddr = shared
        .config
        .host
        .parse()
        .map_err(|_| "server host must be an IP address".to_owned())?;
    let listener =
        match tokio::net::TcpListener::bind(SocketAddr::new(ip, shared.config.port)).await {
            Ok(listener) => listener,
            Err(error) => {
                let message = format!(
                    "failed to bind HTTP server on {ip}:{}: {error}",
                    shared.config.port
                );
                let _ = ready.send(Err(message.clone()));
                return Err(message);
            }
        };
    let address = listener.local_addr().map_err(|error| error.to_string())?;
    let mut router = Router::new()
        .route("/", get(index))
        .route("/api/browser-config", get(browser_config))
        .route("/assets/app.css", get(css))
        .route("/assets/app.js", get(javascript));
    for (method, path) in spec.routes {
        let handler = match method {
            Method::GET => MethodRouter::new().get(http),
            Method::POST => MethodRouter::new().post(http),
            Method::PUT => MethodRouter::new().put(http),
            Method::PATCH => MethodRouter::new().patch(http),
            Method::DELETE => MethodRouter::new().delete(http),
            _ => return Err(format!("unsupported HTTP route method {method}")),
        };
        router = router.route(&path, handler);
    }
    if let Some(path) = spec.websocket_path {
        router = router.route(&path, get(websocket));
    }
    let app = router.with_state(shared);
    let _ = ready.send(Ok(address));
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(async {
        let _ = shutdown.await;
    })
    .await
    .map_err(|error| error.to_string())
}

async fn index() -> impl IntoResponse {
    (
        [
            ("content-type", "text/html; charset=utf-8"),
            ("cache-control", "no-store"),
        ],
        include_str!("../web/dist/index.html"),
    )
}
async fn browser_config(State(shared): State<Shared>) -> impl IntoResponse {
    axum::Json(json!({"websocket": shared.websocket_path}))
}
async fn css() -> impl IntoResponse {
    (
        [
            ("content-type", "text/css; charset=utf-8"),
            ("cache-control", "no-store"),
        ],
        include_str!("../web/dist/assets/app.css"),
    )
}
async fn javascript() -> impl IntoResponse {
    (
        [
            ("content-type", "text/javascript; charset=utf-8"),
            ("cache-control", "no-store"),
        ],
        include_str!("../web/dist/assets/app.js"),
    )
}

async fn http(
    State(shared): State<Shared>,
    ConnectInfo(remote): ConnectInfo<SocketAddr>,
    matched: MatchedPath,
    Path(params): Path<BTreeMap<String, String>>,
    request: Request<Body>,
) -> Response {
    if !origin_allowed(&shared.config, request.headers()) {
        return error_response(403, "origin is not allowed");
    }
    shared
        .diagnostics
        .active_http_requests
        .fetch_add(1, Ordering::Relaxed);
    let result = dispatch_http(shared.clone(), remote, matched.as_str(), params, request).await;
    shared
        .diagnostics
        .active_http_requests
        .fetch_sub(1, Ordering::Relaxed);
    result
}

async fn dispatch_http(
    shared: Shared,
    remote: SocketAddr,
    route: &str,
    params: BTreeMap<String, String>,
    request: Request<Body>,
) -> Response {
    let id = shared.next_request.fetch_add(1, Ordering::Relaxed) + 1;
    let method = request.method().to_string();
    let uri = request.uri().clone();
    let headers = request.headers().clone();
    let role = bearer(&headers)
        .and_then(|token| shared.credentials.get(token).cloned())
        .or_else(|| shared.anonymous_role.clone());
    let body = match to_bytes(request.into_body(), shared.config.body_limit).await {
        Ok(body) => body,
        Err(_) => return error_response(413, "request body exceeds configured limit"),
    };
    let json = if body.is_empty() {
        None
    } else {
        match serde_json::from_slice(&body) {
            Ok(value) => Some(value),
            Err(_) => return error_response(400, "request body is not valid JSON"),
        }
    };
    let parsed = ParsedRequest {
        id,
        method,
        route: route.into(),
        path: uri.path().into(),
        params,
        query: url::form_urlencoded::parse(uri.query().unwrap_or_default().as_bytes())
            .into_owned()
            .collect(),
        headers: selected_headers(&headers),
        json,
        raw: String::from_utf8_lossy(&body).into_owned(),
        remote: remote.to_string(),
        role,
    };
    let (tx, rx) = oneshot::channel();
    if enqueue(
        &shared,
        Dispatch::Http {
            request: parsed,
            response: tx,
        },
    )
    .is_err()
    {
        return error_response(503, "Lua request queue is full");
    }
    match tokio::time::timeout(shared.config.timeout, rx).await {
        Ok(Ok(response)) => response.into_response(),
        Ok(Err(_)) => error_response(503, "Lua control executor stopped"),
        Err(_) => error_response(504, "Lua request handler timed out"),
    }
}

async fn websocket(
    State(shared): State<Shared>,
    ConnectInfo(remote): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
) -> Response {
    if !origin_allowed(&shared.config, &headers) {
        return error_response(403, "origin is not allowed");
    }
    if shared.diagnostics.active_websockets.load(Ordering::Acquire) >= shared.config.max_connections
    {
        return error_response(503, "WebSocket connection limit reached");
    }
    ws.max_message_size(shared.config.message_limit)
        .on_upgrade(move |socket| websocket_loop(socket, shared, remote))
        .into_response()
}

async fn websocket_loop(socket: WebSocket, shared: Shared, remote: SocketAddr) {
    shared
        .diagnostics
        .active_websockets
        .fetch_add(1, Ordering::Relaxed);
    let id = shared.next_client.fetch_add(1, Ordering::Relaxed) + 1;
    let mut client = ClientInfo {
        id,
        remote: remote.to_string(),
        role: shared.anonymous_role.clone(),
        performer: None,
    };
    let _ = enqueue(
        &shared,
        Dispatch::Connected {
            client: client.clone(),
        },
    );
    let mut updates = shared.updates.subscribe();
    let mut shutdown = shared.connection_shutdown.subscribe();
    let initial_full = Arc::clone(&shared.snapshot.read().unwrap());
    let initial = Arc::new(project_snapshot(
        &initial_full,
        client.role.as_deref(),
        &initial_full,
    ));
    let mut revision = initial.get("revision").and_then(Value::as_u64).unwrap_or(0);
    let (mut sink, mut stream) = socket.split();
    if sink
        .send(Message::Text(initial.to_string().into()))
        .await
        .is_err()
    {
        shared
            .diagnostics
            .active_websockets
            .fetch_sub(1, Ordering::Relaxed);
        return;
    }
    let mut heartbeat = tokio::time::interval(Duration::from_secs(20));
    heartbeat.tick().await;
    loop {
        tokio::select! {
            _ = shutdown.recv() => {
                let _ = sink.send(Message::Close(None)).await;
                break;
            }
            update = updates.recv() => match update {
                Ok(update) => {
                    if update.target.is_none_or(|target| target == id) {
                        let next = update.value.get("revision").and_then(Value::as_u64);
                        if next.is_none_or(|next| next > revision) {
                            let schema = Arc::clone(&shared.snapshot.read().unwrap());
                            let projected = project_snapshot(&update.value, client.role.as_deref(), &schema);
                            if sink.send(Message::Text(projected.to_string().into())).await.is_err() { break; }
                            if let Some(next) = next { revision = next; }
                        }
                    }
                }
                Err(broadcast::error::RecvError::Lagged(_)) => {
                    shared.diagnostics.dropped_messages.fetch_add(1, Ordering::Relaxed);
                    let schema = Arc::clone(&shared.snapshot.read().unwrap());
                    let snapshot = Arc::new(project_snapshot(&schema, client.role.as_deref(), &schema));
                    revision = snapshot.get("revision").and_then(Value::as_u64).unwrap_or(revision);
                    if sink.send(Message::Text(snapshot.to_string().into())).await.is_err() { break; }
                }
                Err(broadcast::error::RecvError::Closed) => break,
            },
            incoming = stream.next() => match incoming {
                Some(Ok(Message::Text(text))) => {
                    let Ok(message) = serde_json::from_str::<Value>(&text) else {
                        let _ = sink.send(Message::Text(json!({"type":"error","error":"invalid JSON message"}).to_string().into())).await;
                        continue;
                    };
                    if message.get("type").and_then(Value::as_str) == Some("authenticate") {
                        client.role = message.get("token").and_then(Value::as_str).and_then(|token| shared.credentials.get(token).cloned());
                        let _ = sink.send(Message::Text(json!({"type":"session","role":client.role}).to_string().into())).await;
                        let schema = Arc::clone(&shared.snapshot.read().unwrap());
                        let snapshot = project_snapshot(&schema, client.role.as_deref(), &schema);
                        let _ = sink.send(Message::Text(snapshot.to_string().into())).await;
                    } else if enqueue(&shared, Dispatch::Message { client: client.clone(), message }).is_err() {
                        let _ = sink.send(Message::Text(json!({"type":"error","error":"Lua request queue is full"}).to_string().into())).await;
                        break;
                    }
                }
                Some(Ok(Message::Ping(value))) => { if sink.send(Message::Pong(value)).await.is_err() { break; } }
                Some(Ok(Message::Close(_))) | None | Some(Err(_)) => break,
                _ => {}
            },
            _ = heartbeat.tick() => { if sink.send(Message::Ping(Vec::new().into())).await.is_err() { break; } }
        }
    }
    let _ = enqueue(&shared, Dispatch::Disconnected { client });
    shared
        .diagnostics
        .active_websockets
        .fetch_sub(1, Ordering::Relaxed);
}

fn project_snapshot(snapshot: &Value, role: Option<&str>, schema: &Value) -> Value {
    let mut projected = snapshot.clone();
    let global_controls = role
        .and_then(|role| {
            schema
                .pointer("/state/_role_controls")
                .and_then(|roles| roles.get(role))
        })
        .map(|controls| {
            controls.as_array().map(|controls| {
                controls
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect::<BTreeSet<_>>()
            })
        });
    let allowed = schema
        .pointer("/state/planes")
        .and_then(Value::as_array)
        .map(|planes| {
            planes
                .iter()
                .filter(|plane| {
                    ["roles", "read_only_roles"].iter().any(|field| {
                        plane
                            .get(field)
                            .and_then(Value::as_array)
                            .is_some_and(|roles| {
                                roles.iter().any(|candidate| candidate.as_str() == role)
                            })
                    })
                })
                .filter_map(|plane| plane.get("id").and_then(Value::as_str).map(str::to_owned))
                .collect::<BTreeSet<_>>()
        })
        .unwrap_or_default();
    if let Some(planes) = projected
        .pointer_mut("/state/planes")
        .and_then(Value::as_array_mut)
    {
        planes.retain(|plane| {
            plane
                .get("id")
                .and_then(Value::as_str)
                .is_some_and(|id| allowed.contains(id))
        });
        for plane in planes {
            let writable = plane
                .get("roles")
                .and_then(Value::as_array)
                .is_some_and(|roles| roles.iter().any(|candidate| candidate.as_str() == role));
            plane["read_only"] = Value::Bool(!writable);
            if let Some(controls) = plane.get_mut("controls").and_then(Value::as_array_mut) {
                for control in controls {
                    control["read_only"] = Value::Bool(!writable);
                }
            }
            if let Some(plane) = plane.as_object_mut() {
                plane.remove("read_only_roles");
            }
        }
    }
    if let Some(values) = projected
        .pointer_mut("/state/values")
        .and_then(Value::as_object_mut)
    {
        values.retain(|key, _| {
            key.split_once('/')
                .is_some_and(|(plane, _)| allowed.contains(plane))
        });
    }
    if let Some(controls) = projected
        .pointer_mut("/state/controls")
        .and_then(Value::as_object_mut)
    {
        match global_controls {
            Some(None) => {}
            Some(Some(allowed)) => controls.retain(|name, _| allowed.contains(name)),
            None => controls.clear(),
        }
    }
    if let Some(state) = projected.get_mut("state").and_then(Value::as_object_mut) {
        state.remove("_role_controls");
    }
    projected
}

fn enqueue(shared: &Shared, dispatch: Dispatch) -> Result<(), ()> {
    shared
        .diagnostics
        .queue_depth
        .fetch_add(1, Ordering::Relaxed);
    match shared.queue.try_send(dispatch) {
        Ok(()) => Ok(()),
        Err(TrySendError::Full(_)) => {
            shared
                .diagnostics
                .queue_depth
                .fetch_sub(1, Ordering::Relaxed);
            shared.diagnostics.rejected.fetch_add(1, Ordering::Relaxed);
            Err(())
        }
        Err(TrySendError::Disconnected(_)) => {
            shared
                .diagnostics
                .queue_depth
                .fetch_sub(1, Ordering::Relaxed);
            Err(())
        }
    }
}

fn normalize_path(path: &str) -> String {
    let mut result = path
        .split('/')
        .map(|segment| {
            segment
                .strip_prefix(':')
                .map_or_else(|| segment.to_owned(), |name| format!("{{{name}}}"))
        })
        .collect::<Vec<_>>()
        .join("/");
    if !result.starts_with('/') {
        result.insert(0, '/');
    }
    result
}

fn selected_headers(headers: &HeaderMap) -> BTreeMap<String, String> {
    headers
        .iter()
        .filter_map(|(name, value)| {
            matches!(
                name.as_str(),
                "content-type" | "accept" | "user-agent" | "origin"
            )
            .then(|| {
                value
                    .to_str()
                    .ok()
                    .map(|value| (name.to_string(), value.to_owned()))
            })
            .flatten()
        })
        .collect()
}

fn bearer(headers: &HeaderMap) -> Option<&str> {
    headers
        .get("authorization")?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")
}

fn origin_allowed(config: &ServerConfig, headers: &HeaderMap) -> bool {
    let Some(origin) = headers.get("origin").and_then(|value| value.to_str().ok()) else {
        return true;
    };
    if !config.allowed_origins.is_empty() {
        return config.allowed_origins.contains(origin);
    }
    origin == format!("http://{}:{}", config.host, config.port)
        || (config.port == 0
            && (origin.starts_with("http://127.0.0.1:") || origin.starts_with("http://localhost:")))
}

fn validate_config(config: &ServerConfig) -> Result<(), String> {
    let ip: IpAddr = config
        .host
        .parse()
        .map_err(|_| "server host must be an IP address".to_owned())?;
    if !ip.is_loopback() && config.allowed_origins.is_empty() {
        return Err("LAN server binding requires at least one explicit allowed_origin".into());
    }
    if config.queue_capacity == 0
        || config.body_limit == 0
        || config.message_limit == 0
        || config.max_connections == 0
    {
        return Err("server limits must be positive".into());
    }
    Ok(())
}

fn validate_application(application: &Application) -> Result<(), String> {
    let ip: IpAddr = application
        .config
        .host
        .parse()
        .expect("validated server IP");
    if !ip.is_loopback()
        && (application.credentials.is_empty() || application.anonymous_role.is_some())
    {
        return Err(
            "LAN server control requires credentials and may not assign an anonymous role".into(),
        );
    }
    if let Some(role) = &application.anonymous_role
        && !application.roles.borrow().contains_key(role)
    {
        return Err(format!("anonymous_role references unknown role '{role}'"));
    }
    for role in application.credentials.values() {
        if !application.roles.borrow().contains_key(role) {
            return Err(format!(
                "performer credential references unknown role '{role}'"
            ));
        }
    }
    for (name, role) in application.roles.borrow().iter() {
        if let Some(controls) = &role.controls {
            for control in controls {
                if !application.controls.contains_key(control) {
                    return Err(format!(
                        "role '{name}' references unknown performance control '{control}'"
                    ));
                }
            }
        }
    }
    for plane in application.planes.borrow().values() {
        for role in plane.roles.iter().chain(&plane.read_only_roles) {
            if !application.roles.borrow().contains_key(role) {
                return Err(format!(
                    "control plane '{}' references unknown performer role '{role}'",
                    plane.id
                ));
            }
        }
        let mut ids = BTreeSet::new();
        for control in &plane.controls {
            if !ids.insert(control.id()) {
                return Err(format!(
                    "control plane '{}' contains duplicate control ID '{}'",
                    plane.id,
                    control.id()
                ));
            }
        }
    }
    let mut routes = BTreeSet::new();
    for route in &application.routes {
        let key = (route.method.clone(), normalize_path(&route.path));
        if !routes.insert(key.clone()) {
            return Err(format!("duplicate HTTP route {} {}", key.0, key.1));
        }
        if matches!(
            key.1.as_str(),
            "/" | "/assets/app.css"
                | "/assets/app.js"
                | "/api/browser-config"
                | "/api/state"
                | "/api/control/{name}"
        ) {
            return Err(format!(
                "HTTP route {} {} is reserved by Meldritch",
                key.0, key.1
            ));
        }
    }
    Ok(())
}

impl IntoResponse for HttpResponse {
    fn into_response(self) -> Response {
        let status = StatusCode::from_u16(self.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        let mut response = match self.body {
            ResponseBody::Json(value) => (status, axum::Json(value)).into_response(),
            ResponseBody::Text(text) => (status, text).into_response(),
            ResponseBody::Empty => status.into_response(),
        };
        for (name, value) in self.headers {
            if let (Ok(name), Ok(value)) =
                (HeaderName::try_from(name), HeaderValue::try_from(value))
            {
                response.headers_mut().insert(name, value);
            }
        }
        response
    }
}

#[derive(Debug)]
struct DispatchError {
    status: u16,
    message: String,
}
impl DispatchError {
    fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: 400,
            message: message.into(),
        }
    }
    fn forbidden(message: impl Into<String>) -> Self {
        Self {
            status: 403,
            message: message.into(),
        }
    }
    fn not_found(message: impl Into<String>) -> Self {
        Self {
            status: 404,
            message: message.into(),
        }
    }
    fn internal(message: impl Into<String>) -> Self {
        Self {
            status: 500,
            message: message.into(),
        }
    }
    fn response(self) -> HttpResponse {
        HttpResponse::json(self.status, json!({"error": self.message}))
    }
}

fn error_response(status: u16, message: &str) -> Response {
    HttpResponse::json(status, json!({"error": message})).into_response()
}

fn json_to_lua(lua: &mlua::Lua, value: &Value) -> mlua::Value {
    use mlua::LuaSerdeExt;
    lua.to_value(value).unwrap_or(mlua::Value::Nil)
}

fn lua_response(lua: &mlua::Lua, mut values: mlua::MultiValue) -> HttpResponse {
    use mlua::LuaSerdeExt;
    let body = values.pop_front().unwrap_or(mlua::Value::Nil);
    let status = values
        .pop_front()
        .and_then(|value| match value {
            mlua::Value::Integer(value) => u16::try_from(value).ok(),
            _ => None,
        })
        .unwrap_or(200);
    let headers = values
        .pop_front()
        .and_then(|value| match value {
            mlua::Value::Table(table) => Some(table),
            _ => None,
        })
        .map(|table| {
            table
                .pairs::<String, String>()
                .filter_map(Result::ok)
                .collect()
        })
        .unwrap_or_default();
    let body = match body {
        mlua::Value::Nil => ResponseBody::Empty,
        mlua::Value::String(text) => ResponseBody::Text(text.to_string_lossy()),
        value => match lua.from_value::<Value>(value) {
            Ok(value) => ResponseBody::Json(value),
            Err(_) => {
                return DispatchError::internal(
                    "Lua handler returned an unsupported response value",
                )
                .response();
            }
        },
    };
    HttpResponse {
        status,
        headers,
        body,
    }
}

fn lua_error(error: mlua::Error) -> DispatchError {
    eprintln!(
        "network Lua callback failed: {}",
        sanitize_lua_error(&error)
    );
    DispatchError::internal("Lua application callback failed")
}

fn sanitize_lua_error(error: &mlua::Error) -> String {
    error
        .to_string()
        .lines()
        .next()
        .unwrap_or("Lua error")
        .replace('\\', "/")
}
