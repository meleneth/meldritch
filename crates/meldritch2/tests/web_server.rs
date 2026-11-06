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
server:credential("viewer-secret-token", "viewer")
server:post("/lua-set", function(request)
  performance:set("level", request.json.value)
  return { value = performance:value("level") }
end)
server:post("/slow-set", function(request)
  local deadline = os.clock() + 0.55
  while os.clock() < deadline do end
  performance:set("level", request.json.value)
  return { value = performance:value("level") }
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

    program.events_mut().stop_server();
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
    assert!(statuses.iter().any(|status| *status == 200));
    assert!(statuses.iter().any(|status| *status == 503));
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
    assert!(html.contains("id=\"controls\""));
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
