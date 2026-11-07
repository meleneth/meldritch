use std::{
    thread,
    time::{Duration, Instant},
};

use futures_util::{SinkExt, StreamExt};
use meldritch2::{compiler::compile, lua};
use serde_json::{Value, json};
use tokio_tungstenite::{connect_async, tungstenite::Message};

const APP: &str = r#"
local graph = meldritch.graph()
local source = graph:module("source", "constant", { value = 1.0 })
local gain = graph:module("gain", "gain", { gain = 0.0 })
local output = graph:module("output", "output")
graph:connect(source.out, gain.input)
graph:connect(gain.out, output.input)
graph:control("level", gain, "gain", { min = 0.0, max = 1.0 })

local server = meldritch.server {
  host = "127.0.0.1", port = 0, anonymous_role = "performer",
  queue_capacity = 8, body_limit = 128, message_limit = 256,
  timeout_ms = 500,
}
server:get("/hello/:name", function(request)
  return { hello = request.params.name, query = request.query.q, method = request.method }
end)
server:post("/echo", function(request)
  return { value = request.json.value }, 201, { ["x-meldritch"] = "lua" }
end)
server:get("/explode", function() error("private programmer detail") end)
server:websocket("/performance", {
  message = function(client, message)
    if message.type == "ping" then return { type = "pong", client = client.id } end
  end,
})
local performance = server:performance { name = "Network Test" }
performance:control("level", {
  type = "continuous", target = "level", min = 0.0, max = 1.0,
})
performance:role("performer", { controls = { "level" } })
performance:role("viewer", { controls = {} })
local plane = performance:plane {
  id = "main", label = "Main", roles = { "performer" },
  read_only_roles = { "viewer" },
}
plane:fader {
  id = "plane_level", label = "Level", target = "level",
  min = 0.0, max = 1.0,
}
plane:preset_selector {
  id = "patch", label = "Patch", target = "level", presets = { "Clean", "Driven" },
}
performance:assign("performer", { "level", "plane_level", "patch" })
server:post("/promote-plane", function()
  performance:assign("viewer", { "plane_level" })
  performance:assign_plane("main", { roles = { "performer", "viewer" } })
  return { role = "viewer" }
end)
server:credential("viewer-secret-token", "viewer")
server:post("/lua-set", function(request)
  performance:set("level", request.json.value)
  return { value = performance:value("level") }
end)
server:post("/slow-set", function(request)
  local deadline = os.clock() + 1.5
  while os.clock() < deadline do end
  performance:set("level", request.json.value)
  return { value = performance:value("level") }
end)
server:post("/promote-viewer", function()
  performance:assign("viewer", "*")
  return { role = "viewer" }
end)
return graph
"#;

fn poll_thread<T>(program: &mut lua::Program, request: thread::JoinHandle<T>) -> T {
    let deadline = Instant::now() + Duration::from_secs(3);
    while !request.is_finished() {
        program.events_mut().dispatch_server();
        assert!(Instant::now() < deadline, "network test timed out");
        thread::yield_now();
    }
    program.events_mut().dispatch_server();
    request.join().unwrap()
}

fn response_json(response: &mut ureq::http::Response<ureq::Body>) -> Value {
    serde_json::from_str(&response.body_mut().read_to_string().unwrap()).unwrap()
}

fn agent() -> ureq::Agent {
    ureq::config::Config::builder()
        .http_status_as_error(false)
        .build()
        .new_agent()
}

fn start() -> (
    lua::Program,
    meldritch2::compiler::Machine,
    std::net::SocketAddr,
) {
    let mut program =
        lua::read_program_with_root(APP, std::path::Path::new(env!("CARGO_MANIFEST_DIR"))).unwrap();
    let machine = compile(program.graph()).unwrap();
    let controls = machine
        .control_ids()
        .map(|id| {
            let handle = machine.control_handle(id).unwrap();
            let value = handle.effective_value();
            (handle, value)
        })
        .collect();
    let address = program
        .events_mut()
        .start_server(controls)
        .unwrap()
        .unwrap();
    (program, machine, address)
}

fn get_json(program: &mut lua::Program, url: String) -> (u16, Value) {
    let request = thread::spawn(move || {
        let mut response = ureq::get(&url).call().unwrap();
        let status = response.status().as_u16();
        let body = response_json(&mut response);
        (status, body)
    });
    poll_thread(program, request)
}

#[test]
fn preset_selector_is_typed_authoritative_and_server_validated() {
    let (mut program, machine, address) = start();
    let client = thread::spawn(move || {
        tokio::runtime::Runtime::new().unwrap().block_on(async move {
            let (mut socket, _) = connect_async(format!("ws://{address}/performance"))
                .await
                .unwrap();
            let snapshot: Value = serde_json::from_str(
                socket.next().await.unwrap().unwrap().to_text().unwrap(),
            )
            .unwrap();
            assert_eq!(snapshot["schema_version"], 3);
            let patch = snapshot["state"]["planes"][0]["controls"]
                .as_array()
                .unwrap()
                .iter()
                .find(|control| control["id"] == "patch")
                .unwrap();
            assert_eq!(patch["kind"], "preset_selector");
            assert_eq!(patch["presets"], json!(["Clean", "Driven"]));
            socket
                .send(Message::Text(
                    json!({"type":"preset.select","plane":"main","control":"patch","preset":"Driven"})
                        .to_string()
                        .into(),
                ))
                .await
                .unwrap();
            let updated: Value = serde_json::from_str(
                socket.next().await.unwrap().unwrap().to_text().unwrap(),
            )
            .unwrap();
            assert_eq!(updated["state"]["values"]["main/patch"]["active"], 1);
            socket
                .send(Message::Text(
                    json!({"type":"preset.select","plane":"main","control":"patch","preset":"/tmp/not-allowed"})
                        .to_string()
                        .into(),
                ))
                .await
                .unwrap();
            let rejected: Value = serde_json::from_str(
                socket.next().await.unwrap().unwrap().to_text().unwrap(),
            )
            .unwrap();
            assert_eq!(rejected["type"], "error");
        })
    });
    poll_thread(&mut program, client);
    assert_eq!(
        machine.control_handle("level").unwrap().override_value(),
        Some(1.0)
    );
}

#[test]
fn actual_http_server_dispatches_structured_requests_to_retained_lua() {
    let (mut program, _machine, address) = start();
    let (status, body) = get_json(&mut program, format!("http://{address}/hello/Ada?q=music"));
    assert_eq!(status, 200);
    assert_eq!(
        body,
        json!({"hello":"Ada", "query":"music", "method":"GET"})
    );

    let url = format!("http://{address}/echo");
    let request = thread::spawn(move || {
        let mut response = ureq::post(&url)
            .content_type("application/json")
            .send(serde_json::to_vec(&json!({"value": 42})).unwrap())
            .unwrap();
        let body = response_json(&mut response);
        (
            response.status().as_u16(),
            response
                .headers()
                .get("x-meldritch")
                .unwrap()
                .to_str()
                .unwrap()
                .to_owned(),
            body,
        )
    });
    let (status, header, body) = poll_thread(&mut program, request);
    assert_eq!(
        (status, header.as_str(), body),
        (201, "lua", json!({"value":42}))
    );

    let url = format!("http://{address}/lua-set");
    let request = thread::spawn(move || {
        let mut response = ureq::post(&url)
            .content_type("application/json")
            .send("{\"value\":0.625}")
            .unwrap();
        response_json(&mut response)
    });
    assert_eq!(poll_thread(&mut program, request), json!({"value":0.625}));
    assert_eq!(
        program.events_mut().server_diagnostics().unwrap().revision,
        1
    );

    let url = format!("http://{address}/explode");
    let request = thread::spawn(move || {
        let mut response = agent().get(&url).call().unwrap();
        let body = response_json(&mut response);
        (response.status().as_u16(), body)
    });
    let (status, body) = poll_thread(&mut program, request);
    assert_eq!(status, 500);
    assert_eq!(body, json!({"error":"Lua request handler failed"}));
    assert!(!body.to_string().contains("private programmer detail"));
}

#[test]
fn transport_limits_errors_permissions_port_conflicts_and_restart_are_explicit() {
    let (mut program, machine, address) = start();

    let invalid_url = format!("http://{address}/echo");
    let invalid = thread::spawn(move || {
        agent()
            .post(&invalid_url)
            .content_type("application/json")
            .send("{")
            .unwrap()
            .status()
            .as_u16()
    });
    assert_eq!(poll_thread(&mut program, invalid), 400);

    let large_url = format!("http://{address}/echo");
    let large = thread::spawn(move || {
        agent()
            .post(&large_url)
            .content_type("application/json")
            .send(format!("{{\"value\":\"{}\"}}", "x".repeat(256)))
            .unwrap()
            .status()
            .as_u16()
    });
    assert_eq!(poll_thread(&mut program, large), 413);

    let missing_url = format!("http://{address}/missing");
    let missing =
        thread::spawn(move || agent().get(&missing_url).call().unwrap().status().as_u16());
    assert_eq!(poll_thread(&mut program, missing), 404);

    let forbidden_url = format!("http://{address}/api/control/level");
    let forbidden = thread::spawn(move || {
        agent()
            .post(&forbidden_url)
            .header("authorization", "Bearer viewer-secret-token")
            .content_type("application/json")
            .send("{\"value\":0.9}")
            .unwrap()
            .status()
            .as_u16()
    });
    assert_eq!(poll_thread(&mut program, forbidden), 403);
    assert_eq!(
        machine.control_handle("level").unwrap().override_value(),
        None
    );

    let promote_url = format!("http://{address}/promote-viewer");
    let promote = thread::spawn(move || {
        agent()
            .post(&promote_url)
            .send_empty()
            .unwrap()
            .status()
            .as_u16()
    });
    assert_eq!(poll_thread(&mut program, promote), 200);
    let allowed_url = format!("http://{address}/api/control/level");
    let allowed = thread::spawn(move || {
        agent()
            .post(&allowed_url)
            .header("authorization", "Bearer viewer-secret-token")
            .content_type("application/json")
            .send("{\"value\":0.9}")
            .unwrap()
            .status()
            .as_u16()
    });
    assert_eq!(poll_thread(&mut program, allowed), 200);

    let occupied = APP.replace("port = 0", &format!("port = {}", address.port()));
    let mut conflict =
        lua::read_program_with_root(&occupied, std::path::Path::new(env!("CARGO_MANIFEST_DIR")))
            .unwrap();
    let other = compile(conflict.graph()).unwrap();
    let controls = other
        .control_ids()
        .map(|id| {
            let handle = other.control_handle(id).unwrap();
            let value = handle.effective_value();
            (handle, value)
        })
        .collect();
    assert!(
        conflict
            .events_mut()
            .start_server(controls)
            .unwrap_err()
            .contains("failed to bind")
    );

    let (connected, ready) = std::sync::mpsc::sync_channel(1);
    let websocket_url = format!("ws://{address}/performance");
    let active_client = thread::spawn(move || {
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(async move {
                let (mut socket, _) = connect_async(websocket_url).await.unwrap();
                let _snapshot = socket.next().await.unwrap().unwrap();
                connected.send(()).unwrap();
                let closing = socket.next().await;
                assert!(
                    closing.is_none()
                        || matches!(closing, Some(Ok(Message::Close(_))) | Some(Err(_)))
                );
            });
    });
    ready.recv_timeout(Duration::from_secs(2)).unwrap();
    program.events_mut().stop_server();
    active_client.join().unwrap();
    let controls = machine
        .control_ids()
        .map(|id| {
            let handle = machine.control_handle(id).unwrap();
            let value = handle.effective_value();
            (handle, value)
        })
        .collect();
    assert!(
        program
            .events_mut()
            .start_server(controls)
            .unwrap()
            .is_some()
    );
}

#[test]
fn bounded_queue_rejects_overload_and_started_timeout_has_defined_commit_policy() {
    let (mut program, machine, address) = start();
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(13));
    let requests = (0..12)
        .map(|index| {
            let barrier = std::sync::Arc::clone(&barrier);
            let url = format!("http://{address}/hello/client-{index}");
            thread::spawn(move || {
                barrier.wait();
                agent().get(&url).call().unwrap().status().as_u16()
            })
        })
        .collect::<Vec<_>>();
    barrier.wait();
    let deadline = Instant::now() + Duration::from_secs(2);
    while program.events_mut().server_diagnostics().unwrap().rejected == 0 {
        assert!(
            Instant::now() < deadline,
            "request overload was not observed"
        );
        thread::yield_now();
    }
    program.events_mut().dispatch_server();
    let statuses = requests
        .into_iter()
        .map(|request| request.join().unwrap())
        .collect::<Vec<_>>();
    assert!(statuses.contains(&200));
    assert!(statuses.contains(&503));
    assert!(program.events_mut().server_diagnostics().unwrap().rejected > 0);

    let url = format!("http://{address}/slow-set");
    let request = thread::spawn(move || {
        agent()
            .post(&url)
            .content_type("application/json")
            .send("{\"value\":0.33}")
            .unwrap()
            .status()
            .as_u16()
    });
    let status = poll_thread(&mut program, request);
    assert_eq!(status, 504);
    assert_eq!(
        machine.control_handle("level").unwrap().override_value(),
        Some(0.33),
        "once Lua dispatch begins it commits atomically even if the waiting client times out"
    );
}

#[test]
fn two_websockets_share_ordered_authoritative_state_and_change_audio_control() {
    let (mut program, mut machine, address) = start();
    let external_control = machine.control_handle("level").unwrap();
    let client = thread::spawn(move || {
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(async move {
                let url = format!("ws://{address}/performance");
                let (mut a, _) = connect_async(&url).await.unwrap();
                let (mut b, _) = connect_async(&url).await.unwrap();
                let first_a = a.next().await.unwrap().unwrap();
                let first_b = b.next().await.unwrap().unwrap();
                let initial_a: Value = serde_json::from_str(first_a.to_text().unwrap()).unwrap();
                let initial_b: Value = serde_json::from_str(first_b.to_text().unwrap()).unwrap();
                assert_eq!(initial_a, initial_b);
                assert!(!initial_a["epoch"].as_str().unwrap().is_empty());
                assert_eq!(initial_a["state"]["planes"][0]["id"], "main");
                a.send(Message::Text(
                    json!({"type":"control.set","name":"level","value":0.75})
                        .to_string()
                        .into(),
                ))
                .await
                .unwrap();
                let update_a: Value =
                    serde_json::from_str(a.next().await.unwrap().unwrap().to_text().unwrap())
                        .unwrap();
                let update_b: Value =
                    serde_json::from_str(b.next().await.unwrap().unwrap().to_text().unwrap())
                        .unwrap();
                assert_eq!(update_a, update_b);
                assert_eq!(update_a["revision"], 1);
                assert!(update_a["state"].get("planes").is_none());
                assert_eq!(update_a["state"]["controls"]["level"]["value"], 0.75);
                external_control.publish(0.25).unwrap();
                let external_a: Value =
                    serde_json::from_str(a.next().await.unwrap().unwrap().to_text().unwrap())
                        .unwrap();
                let external_b: Value =
                    serde_json::from_str(b.next().await.unwrap().unwrap().to_text().unwrap())
                        .unwrap();
                assert_eq!(external_a, external_b);
                assert_eq!(external_a["revision"], 2);
                assert_eq!(external_a["state"]["controls"]["level"]["value"], 0.25);
                b.send(Message::Text("{\"type\":\"ping\"}".into()))
                    .await
                    .unwrap();
                let targeted: Value =
                    serde_json::from_str(b.next().await.unwrap().unwrap().to_text().unwrap())
                        .unwrap();
                assert_eq!(targeted["type"], "pong");
                drop(a);
                let (mut reconnected, _) = connect_async(&url).await.unwrap();
                let recovered: Value = serde_json::from_str(
                    reconnected
                        .next()
                        .await
                        .unwrap()
                        .unwrap()
                        .to_text()
                        .unwrap(),
                )
                .unwrap();
                assert_eq!(recovered["revision"], 2);
                assert_eq!(recovered["state"]["controls"]["level"]["value"], 0.25);
                reconnected.send(Message::Text("{".into())).await.unwrap();
                let malformed: Value = serde_json::from_str(
                    reconnected
                        .next()
                        .await
                        .unwrap()
                        .unwrap()
                        .to_text()
                        .unwrap(),
                )
                .unwrap();
                assert_eq!(
                    malformed,
                    json!({"type":"error","error":"invalid JSON message"})
                );
                (initial_a, external_a)
            })
    });
    let (_initial, _updated) = poll_thread(&mut program, client);
    let level = machine.control_handle("level").unwrap();
    assert_eq!(level.override_value(), Some(0.25));
    let mut samples = [0.0; 8];
    machine.process(&mut samples);
    assert!(samples.iter().all(|sample| (*sample - 0.25).abs() < 1e-12));
    let diagnostics = program.events_mut().server_diagnostics().unwrap();
    assert_eq!(diagnostics.revision, 2);
    assert!(diagnostics.broadcasts >= 1);
}

#[test]
fn lua_planes_project_by_role_and_edit_the_real_sequencer() {
    let source =
        include_str!("../examples/71-control-planes.lua").replace("port = 8080", "port = 0");
    let mut program =
        lua::read_program_with_root(&source, std::path::Path::new(env!("CARGO_MANIFEST_DIR")))
            .unwrap();
    let mut machine = compile(program.graph()).unwrap();
    let controls = machine
        .control_ids()
        .map(|id| {
            let handle = machine.control_handle(id).unwrap();
            let value = handle.effective_value();
            (handle, value)
        })
        .collect();
    let address = program
        .events_mut()
        .start_server(controls)
        .unwrap()
        .unwrap();

    let client = thread::spawn(move || {
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(async move {
                let (mut socket, _) = connect_async(format!("ws://{address}/performance"))
                    .await
                    .unwrap();
                let guest: Value =
                    serde_json::from_str(socket.next().await.unwrap().unwrap().to_text().unwrap())
                        .unwrap();
                assert_eq!(guest["state"]["planes"], json!([]));
                socket
                    .send(Message::Text(
                        json!({"type":"authenticate","token":"drummer-demo-token"})
                            .to_string()
                            .into(),
                    ))
                    .await
                    .unwrap();
                let session: Value =
                    serde_json::from_str(socket.next().await.unwrap().unwrap().to_text().unwrap())
                        .unwrap();
                assert_eq!(session, json!({"type":"session","role":"drummer"}));
                let snapshot: Value =
                    serde_json::from_str(socket.next().await.unwrap().unwrap().to_text().unwrap())
                        .unwrap();
                assert_eq!(snapshot["state"]["planes"].as_array().unwrap().len(), 1);
                assert_eq!(snapshot["state"]["planes"][0]["id"], "rhythm");
                assert_eq!(snapshot["state"]["planes"][0]["read_only"], false);

                socket
                .send(Message::Text(
                    json!({"type":"step.toggle","plane":"rhythm","control":"kick_pattern","step":1})
                        .to_string()
                        .into(),
                ))
                .await
                .unwrap();
                let toggled: Value =
                    serde_json::from_str(socket.next().await.unwrap().unwrap().to_text().unwrap())
                        .unwrap();
                assert_eq!(toggled["revision"], 1);
                assert_eq!(
                    toggled["state"]["values"]["rhythm/kick_pattern"]["steps"][1],
                    true
                );

                socket
                    .send(Message::Text(
                        json!({"type":"trigger.fire","plane":"rhythm","control":"kick_fill"})
                            .to_string()
                            .into(),
                    ))
                    .await
                    .unwrap();
                let triggered: Value =
                    serde_json::from_str(socket.next().await.unwrap().unwrap().to_text().unwrap())
                        .unwrap();
                assert_eq!(triggered["revision"], 2);
                assert_eq!(
                    triggered["state"]["values"]["rhythm/kick_fill"]["serial"],
                    1
                );

                let (mut synth, _) = connect_async(format!("ws://{address}/performance"))
                    .await
                    .unwrap();
                synth.next().await.unwrap().unwrap();
                synth
                    .send(Message::Text(
                        json!({"type":"authenticate","token":"synth-demo-token"})
                            .to_string()
                            .into(),
                    ))
                    .await
                    .unwrap();
                synth.next().await.unwrap().unwrap();
                let synth_schema: Value =
                    serde_json::from_str(synth.next().await.unwrap().unwrap().to_text().unwrap())
                        .unwrap();
                assert_eq!(synth_schema["state"]["planes"][0]["id"], "synthesis");
                synth
                    .send(Message::Text(
                        json!({"type":"control.set","name":"filter_cutoff","value":2400.0})
                            .to_string()
                            .into(),
                    ))
                    .await
                    .unwrap();
                synth.next().await.unwrap().unwrap();
                synth
                    .send(Message::Text(
                        json!({"type":"control.set","name":"filter_resonance","value":0.8})
                            .to_string()
                            .into(),
                    ))
                    .await
                    .unwrap();
                synth.next().await.unwrap().unwrap();
            })
    });
    poll_thread(&mut program, client);
    let sequencer = &machine.sequencer_controls()[0];
    let main = sequencer
        .pattern_names()
        .iter()
        .position(|name| name == "Main")
        .unwrap();
    let pattern = sequencer.pattern_at(main).unwrap();
    assert_eq!(
        pattern.velocities[0][3], 1.0,
        "FILL must mutate authored music"
    );
    assert_eq!(
        machine
            .control_handle("filter_cutoff")
            .unwrap()
            .override_value(),
        Some(2400.0)
    );
    assert_eq!(
        machine
            .control_handle("filter_resonance")
            .unwrap()
            .override_value(),
        Some(0.8)
    );
    let mut audio = [0.0; 4096];
    machine.process(&mut audio);
    assert!(audio.iter().any(|sample| sample.abs() > 1e-5));
    assert_eq!(
        program.events_mut().server_diagnostics().unwrap().revision,
        4
    );
}

#[test]
fn note_planes_authorize_edits_project_envelopes_and_schedule_patterns() {
    let source = include_str!("../examples/72-note-performance.lua").replace("port=8080", "port=0");
    let mut program =
        lua::read_program_with_root(&source, std::path::Path::new(env!("CARGO_MANIFEST_DIR")))
            .unwrap();
    let machine = compile(program.graph()).unwrap();
    let controls = machine
        .control_ids()
        .map(|id| {
            let handle = machine.control_handle(id).unwrap();
            let value = handle.effective_value();
            (handle, value)
        })
        .collect();
    let address = program
        .events_mut()
        .start_server(controls)
        .unwrap()
        .unwrap();

    let client = thread::spawn(move || {
        tokio::runtime::Runtime::new().unwrap().block_on(async move {
            let (mut drummer, _) = connect_async(format!("ws://{address}/performance"))
                .await
                .unwrap();
            let guest: Value = serde_json::from_str(
                drummer.next().await.unwrap().unwrap().to_text().unwrap(),
            )
            .unwrap();
            assert_eq!(guest["state"]["planes"], json!([]));
            assert_eq!(guest["state"]["controls"], json!({}));
            drummer
                .send(Message::Text(
                    json!({"type":"authenticate","token":"drummer-demo-token"})
                        .to_string()
                        .into(),
                ))
                .await
                .unwrap();
            drummer.next().await.unwrap().unwrap();
            let snapshot: Value = serde_json::from_str(
                drummer.next().await.unwrap().unwrap().to_text().unwrap(),
            )
            .unwrap();
            let kinds = snapshot["state"]["planes"][0]["controls"]
                .as_array()
                .unwrap()
                .iter()
                .map(|control| control["kind"].as_str().unwrap())
                .collect::<Vec<_>>();
            assert!(kinds.contains(&"piano_roll"));
            assert!(kinds.contains(&"pattern_selector"));
            assert_eq!(snapshot["state"]["planes"][0]["read_only"], false);

            drummer
                .send(Message::Text(
                    json!({"type":"note.create","plane":"rhythm","control":"bass_notes","pattern":0,"step":15,"pitch":48,"velocity":77,"duration":1})
                        .to_string().into(),
                ))
                .await
                .unwrap();
            let edited: Value = serde_json::from_str(
                drummer.next().await.unwrap().unwrap().to_text().unwrap(),
            )
            .unwrap();
            assert!(edited["state"]["values"]["rhythm/bass_notes"]["notes"]
                .as_array()
                .unwrap()
                .iter()
                .any(|note| note["step"] == 15 && note["velocity"] == 77));

            drummer
                .send(Message::Text(
                    json!({"type":"pattern.select","plane":"rhythm","control":"bass_patterns","pattern":1,"boundary":"bar"})
                        .to_string().into(),
                ))
                .await
                .unwrap();
            let pending: Value = serde_json::from_str(
                drummer.next().await.unwrap().unwrap().to_text().unwrap(),
            )
            .unwrap();
            assert_eq!(pending["state"]["values"]["rhythm/bass_patterns"]["pending"], 1);
            assert_eq!(pending["state"]["values"]["rhythm/bass_patterns"]["boundary"], "bar");

            let (mut observer, _) = connect_async(format!("ws://{address}/performance"))
                .await
                .unwrap();
            observer.next().await.unwrap().unwrap();
            observer
                .send(Message::Text(
                    json!({"type":"authenticate","token":"observer-demo-token"})
                        .to_string()
                        .into(),
                ))
                .await
                .unwrap();
            observer.next().await.unwrap().unwrap();
            let observer_snapshot: Value = serde_json::from_str(
                observer.next().await.unwrap().unwrap().to_text().unwrap(),
            )
            .unwrap();
            assert!(observer_snapshot["state"]["planes"]
                .as_array()
                .unwrap()
                .iter()
                .all(|plane| plane["read_only"] == true));
            observer
                .send(Message::Text(
                    json!({"type":"note.delete","plane":"rhythm","control":"bass_notes","pattern":0,"id":1})
                        .to_string().into(),
                ))
                .await
                .unwrap();
            let rejected: Value = serde_json::from_str(
                observer.next().await.unwrap().unwrap().to_text().unwrap(),
            )
            .unwrap();
            assert_eq!(rejected["type"], "error");

            let (mut synth, _) = connect_async(format!("ws://{address}/performance"))
                .await
                .unwrap();
            synth.next().await.unwrap().unwrap();
            synth
                .send(Message::Text(
                    json!({"type":"authenticate","token":"synth-demo-token"})
                        .to_string()
                        .into(),
                ))
                .await
                .unwrap();
            synth.next().await.unwrap().unwrap();
            let synth_snapshot: Value = serde_json::from_str(
                synth.next().await.unwrap().unwrap().to_text().unwrap(),
            )
            .unwrap();
            assert!(synth_snapshot["state"]["planes"][0]["controls"]
                .as_array()
                .unwrap()
                .iter()
                .any(|control| control["kind"] == "adsr"));
            synth
                .send(Message::Text(
                    json!({"type":"adsr.set","plane":"synthesis","control":"amp_envelope","parameter":"attack","value":0.08})
                        .to_string().into(),
                ))
                .await
                .unwrap();
            synth.next().await.unwrap().unwrap();
        })
    });
    poll_thread(&mut program, client);

    let sequencer = &machine.sequencer_controls()[0];
    assert!(
        sequencer
            .note_events(0, 1)
            .iter()
            .any(|note| note.step == 15 && note.velocity == 77)
    );
    assert_eq!(sequencer.queued_pattern(), Some(1));
    assert_eq!(sequencer.queued_pattern_quantization(), 16);
    assert_eq!(
        machine
            .control_handle("amp_attack")
            .unwrap()
            .override_value(),
        Some(0.08)
    );
}

#[test]
fn read_only_projection_is_enforced_and_live_role_reassignment_is_revisioned() {
    let (mut program, machine, address) = start();
    let client = thread::spawn(move || {
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(async move {
                let (mut socket, _) = connect_async(format!("ws://{address}/performance"))
                    .await
                    .unwrap();
                socket.next().await.unwrap().unwrap();
                socket
                    .send(Message::Text(
                        json!({"type":"authenticate","token":"viewer-secret-token"})
                            .to_string()
                            .into(),
                    ))
                    .await
                    .unwrap();
                socket.next().await.unwrap().unwrap();
                let projected: Value =
                    serde_json::from_str(socket.next().await.unwrap().unwrap().to_text().unwrap())
                        .unwrap();
                assert_eq!(projected["state"]["planes"][0]["read_only"], true);
                socket
                    .send(Message::Text(
                        json!({"type":"control.set","name":"plane_level","value":0.4})
                            .to_string()
                            .into(),
                    ))
                    .await
                    .unwrap();
                let rejected: Value =
                    serde_json::from_str(socket.next().await.unwrap().unwrap().to_text().unwrap())
                        .unwrap();
                assert_eq!(rejected["type"], "error");

                let status = tokio::task::spawn_blocking(move || {
                    agent()
                        .post(format!("http://{address}/promote-plane"))
                        .send_empty()
                        .unwrap()
                        .status()
                        .as_u16()
                })
                .await
                .unwrap();
                assert_eq!(status, 200);
                let reassigned: Value =
                    serde_json::from_str(socket.next().await.unwrap().unwrap().to_text().unwrap())
                        .unwrap();
                assert_eq!(reassigned["schema_revision"], 2);
                assert_eq!(reassigned["state"]["planes"][0]["read_only"], false);
                socket
                    .send(Message::Text(
                        json!({"type":"control.set","name":"plane_level","value":0.4})
                            .to_string()
                            .into(),
                    ))
                    .await
                    .unwrap();
                let accepted: Value =
                    serde_json::from_str(socket.next().await.unwrap().unwrap().to_text().unwrap())
                        .unwrap();
                assert_eq!(accepted["state"]["values"]["main/plane_level"], 0.4);
            })
    });
    poll_thread(&mut program, client);
    assert_eq!(
        machine.control_handle("level").unwrap().override_value(),
        Some(0.4)
    );
}

#[test]
fn drum_grid_edits_velocity_and_mute_are_authoritative() {
    let source = r#"
local instrument=meldritch.instrument("Drum grid test")
local sequence=instrument:sequencer {bpm=120,steps=16,steps_per_beat=4}
local kit=sequence:drum_kit {name="Kit",voices={{id="kick",label="Kick",note=36},{id="snare",label="Snare",note=38}}}
local pattern=sequence:pattern("Main")
pattern:drums(kit,{kick={{id=1,step=1,velocity=100}},snare={{id=1,step=5,velocity=110}}},16)
local tone=instrument:oscillator {waveform="sine",frequency=60}
instrument:connect(tone.out,instrument.output.left);instrument:connect(tone.out,instrument.output.right)
local server=meldritch.server {host="127.0.0.1",port=0}
server:websocket("/performance",{})
local performance=server:performance {name="Drums"}
performance:role("drummer",{controls={}});performance:role("observer",{controls={}})
server:credential("drummer-token","drummer");server:credential("observer-token","observer")
local plane=performance:plane {id="drums",label="Drums",roles={"drummer"},read_only_roles={"observer"}}
plane:drum_grid {id="grid",label="Grid",kit=kit,steps=16}
return instrument
"#;
    let mut program =
        lua::read_program_with_root(source, std::path::Path::new(env!("CARGO_MANIFEST_DIR")))
            .unwrap();
    let machine = compile(program.graph()).unwrap();
    let controls = machine
        .control_ids()
        .map(|id| {
            let handle = machine.control_handle(id).unwrap();
            let value = handle.effective_value();
            (handle, value)
        })
        .collect();
    let address = program
        .events_mut()
        .start_server(controls)
        .unwrap()
        .unwrap();
    let client = thread::spawn(move || {
        tokio::runtime::Runtime::new().unwrap().block_on(async move {
        let (mut socket,_) = connect_async(format!("ws://{address}/performance")).await.unwrap();
        socket.next().await.unwrap().unwrap();
        socket.send(Message::Text(json!({"type":"authenticate","token":"drummer-token"}).to_string().into())).await.unwrap();
        socket.next().await.unwrap().unwrap();
        let snapshot: Value=serde_json::from_str(socket.next().await.unwrap().unwrap().to_text().unwrap()).unwrap();
        assert_eq!(snapshot["state"]["planes"][0]["controls"][0]["kind"],"drum_grid");
        socket.send(Message::Text(json!({"type":"drum.hit.toggle","plane":"drums","control":"grid","row":"kick","pattern":0,"step":2}).to_string().into())).await.unwrap();
        let added: Value=serde_json::from_str(socket.next().await.unwrap().unwrap().to_text().unwrap()).unwrap();
        assert!(added["state"]["values"]["drums/grid"]["rows"][0]["hits"].as_array().unwrap().iter().any(|hit|hit["step"]==2));
        socket.send(Message::Text(json!({"type":"drum.hit.velocity","plane":"drums","control":"grid","row":"kick","pattern":0,"step":2,"velocity":64}).to_string().into())).await.unwrap();
        let velocity: Value=serde_json::from_str(socket.next().await.unwrap().unwrap().to_text().unwrap()).unwrap();
        assert!(velocity["state"]["values"]["drums/grid"]["rows"][0]["hits"].as_array().unwrap().iter().any(|hit|hit["step"]==2&&hit["velocity"]==64));
        socket.send(Message::Text(json!({"type":"drum.mute","plane":"drums","control":"grid","row":"kick","muted":true}).to_string().into())).await.unwrap();
        let muted: Value=serde_json::from_str(socket.next().await.unwrap().unwrap().to_text().unwrap()).unwrap();
        assert_eq!(muted["state"]["values"]["drums/grid"]["rows"][0]["muted"],true);
        let (mut observer,_) = connect_async(format!("ws://{address}/performance")).await.unwrap();
        observer.next().await.unwrap().unwrap();
        observer.send(Message::Text(json!({"type":"authenticate","token":"observer-token"}).to_string().into())).await.unwrap();
        observer.next().await.unwrap().unwrap();observer.next().await.unwrap().unwrap();
        observer.send(Message::Text(json!({"type":"drum.mute","plane":"drums","control":"grid","row":"snare","muted":true}).to_string().into())).await.unwrap();
        let rejected: Value=serde_json::from_str(observer.next().await.unwrap().unwrap().to_text().unwrap()).unwrap();
        assert_eq!(rejected["type"],"error");
    })
    });
    poll_thread(&mut program, client);
    let sequencer = &machine.sequencer_controls()[0];
    assert!(
        sequencer
            .note_events(0, 0)
            .iter()
            .any(|hit| hit.step == 2 && hit.velocity == 64)
    );
    assert!(sequencer.muted(0));
    assert!(!sequencer.muted(1));
}

#[test]
fn drop_demo_is_browser_discovered_and_changes_the_real_audio_graph() {
    let source =
        include_str!("../examples/70-network-performance.lua").replace("port = 8080", "port = 0");
    let mut program =
        lua::read_program_with_root(&source, std::path::Path::new(env!("CARGO_MANIFEST_DIR")))
            .unwrap();
    let mut machine = compile(program.graph()).unwrap();
    let controls = machine
        .control_ids()
        .map(|id| {
            let handle = machine.control_handle(id).unwrap();
            let value = handle.effective_value();
            (handle, value)
        })
        .collect();
    let address = program
        .events_mut()
        .start_server(controls)
        .unwrap()
        .unwrap();

    let root_url = format!("http://{address}/");
    let root = thread::spawn(move || {
        agent()
            .get(&root_url)
            .call()
            .unwrap()
            .body_mut()
            .read_to_string()
            .unwrap()
    });
    let html = poll_thread(&mut program, root);
    assert!(html.contains("id=\"app\""));
    assert!(
        !html.contains("Bass Level"),
        "control model must come from the composition snapshot"
    );

    let mut before = [0.0; 64];
    machine.process(&mut before);
    let drop_url = format!("http://{address}/api/control/DROP");
    let request = thread::spawn(move || {
        agent()
            .post(&drop_url)
            .content_type("application/json")
            .send("{\"value\":true}")
            .unwrap()
            .status()
            .as_u16()
    });
    assert_eq!(poll_thread(&mut program, request), 200);
    assert_eq!(
        machine
            .control_handle("drop_layer")
            .unwrap()
            .override_value(),
        Some(0.35)
    );
    let mut after = [0.0; 64];
    machine.process(&mut after);
    assert_ne!(
        before, after,
        "DROP must alter Meldritch DSP rather than only browser state"
    );
}
