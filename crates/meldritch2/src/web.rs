//! Bounded asynchronous HTTP/WebSocket transport for the UI-thread Lua owner.

use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
    net::{IpAddr, SocketAddr},
    rc::Rc,
    sync::{
        Arc, RwLock,
        atomic::{AtomicU64, AtomicUsize, Ordering},
        mpsc::{self, SyncSender, TrySendError},
    },
    thread,
    time::{Duration, Instant},
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
pub struct Application {
    pub config: ServerConfig,
    pub routes: Vec<Route>,
    pub websocket: Option<WebSocketCallbacks>,
    pub name: String,
    pub controls: BTreeMap<String, PerformanceControl>,
    pub roles: Rc<RefCell<BTreeMap<String, Role>>>,
    pub credentials: BTreeMap<String, String>,
    pub anonymous_role: Option<String>,
    runtime: Rc<RefCell<PerformanceRuntime>>,
}

#[derive(Default)]
struct PerformanceRuntime {
    handles: BTreeMap<String, ControlHandle>,
    observed: BTreeMap<String, f64>,
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
            runtime: Rc::new(RefCell::new(PerformanceRuntime::default())),
        }
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
        self.runtime
            .borrow_mut()
            .observed
            .insert(name.to_owned(), resolved);
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

    fn snapshot(&self, revision: u64) -> Value {
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
        json!({
            "type": "state.snapshot",
            "revision": revision,
            "state": { "name": self.name, "controls": controls }
        })
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
        changed
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
}

impl ControlExecutor {
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
        if self.application.poll_external_changes() {
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
            return HttpResponse::json(200, (**self.shared.snapshot.read().unwrap()).clone());
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
        let revision = self
            .shared
            .diagnostics
            .revision
            .fetch_add(1, Ordering::AcqRel)
            + 1;
        let snapshot = Arc::new(self.application.snapshot(revision));
        *self.shared.snapshot.write().unwrap() = Arc::clone(&snapshot);
        let _ = self.shared.updates.send(Outbound {
            target: None,
            value: snapshot,
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
    let diagnostics = Arc::new(AtomicDiagnostics::default());
    let initial = Arc::new(application.snapshot(0));
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
        },
        ControlExecutor {
            application,
            receiver,
            shared,
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
        .route("/meldritch.css", get(css))
        .route("/meldritch.js", get(javascript));
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
        [("content-type", "text/html; charset=utf-8")],
        include_str!("../web/index.html"),
    )
}
async fn browser_config(State(shared): State<Shared>) -> impl IntoResponse {
    axum::Json(json!({"websocket": shared.websocket_path}))
}
async fn css() -> impl IntoResponse {
    (
        [("content-type", "text/css; charset=utf-8")],
        include_str!("../web/meldritch.css"),
    )
}
async fn javascript() -> impl IntoResponse {
    (
        [("content-type", "text/javascript; charset=utf-8")],
        include_str!("../web/meldritch.js"),
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
    let initial = Arc::clone(&shared.snapshot.read().unwrap());
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
            update = updates.recv() => match update {
                Ok(update) => {
                    if update.target.is_none_or(|target| target == id) {
                        let next = update.value.get("revision").and_then(Value::as_u64);
                        if next.is_none_or(|next| next > revision) {
                            if sink.send(Message::Text(update.value.to_string().into())).await.is_err() { break; }
                            if let Some(next) = next { revision = next; }
                        }
                    }
                }
                Err(broadcast::error::RecvError::Lagged(_)) => {
                    shared.diagnostics.dropped_messages.fetch_add(1, Ordering::Relaxed);
                    let snapshot = Arc::clone(&shared.snapshot.read().unwrap());
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
    let mut routes = BTreeSet::new();
    for route in &application.routes {
        let key = (route.method.clone(), normalize_path(&route.path));
        if !routes.insert(key.clone()) {
            return Err(format!("duplicate HTTP route {} {}", key.0, key.1));
        }
        if matches!(
            key.1.as_str(),
            "/" | "/meldritch.css"
                | "/meldritch.js"
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
