use std::{fs, path::Path};

use meldritch2::{compiler::compile_at_rate, event_bridge::EventDiagnostics, lua, model::Widget};

const EXAMPLE: &str = include_str!("../../../examples/instruments/event_plaits.lua");

#[test]
fn independent_plaits_example_uses_the_same_mounted_and_persistent_event_bridge() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/instruments");
    let mut program = lua::read_program_with_root(EXAMPLE, &root).unwrap();
    let mut machine = compile_at_rate(program.graph(), 16.0).unwrap();
    let source = machine.sequencer_controls()[0].event_source().clone();
    let Widget::EventActivity { state, .. } = &program.graph().screens[0].widgets[0] else {
        panic!("event example must author the reusable activity widget")
    };
    let state = state.clone();

    // One persistent observer remains after the example disconnects its second
    // handle. Mounting adds independent step and note screen subscriptions.
    assert_eq!(program.events_mut().active_subscriptions(), 1);
    program.events_mut().mount_screen("activity").unwrap();
    assert_eq!(program.events_mut().active_subscriptions(), 3);
    machine.process_stereo(&mut [0.0], &mut [0.0]);
    assert_eq!(source.diagnostics().publications, 2);
    assert_eq!(program.events_mut().dispatch(), 3);
    assert_eq!(state.step(), 0);
    assert!((state.activity(0).unwrap().1 - 130.812_78).abs() < 0.1);

    let generation = state.activity(0).unwrap().0;
    let callbacks = program.events_mut().callback_count();
    program.events_mut().unmount_screen("activity");
    assert_eq!(program.events_mut().active_subscriptions(), 1);
    let mut left = [0.0; 12];
    let mut right = [0.0; 12];
    machine.process_stereo(&mut left, &mut right);
    program.events_mut().dispatch();
    assert_eq!(state.activity(0).unwrap().0, generation);
    assert!(program.events_mut().callback_count() > callbacks);
    assert!(left.iter().chain(&right).all(|sample| sample.is_finite()));
}

#[test]
fn invalid_event_registration_fails_during_real_lua_authoring() {
    let root = std::env::temp_dir().join(format!("meldritch-event-invalid-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    let script = r#"
      local i=meldritch.instrument("invalid event")
      local tone=i:constant {value=0};i:connect(tone.out,i.output.left);i:connect(tone.out,i.output.right)
      local q=i:sequencer {bpm=120,steps=16,steps_per_beat=4};q:track("one")
      q:on("sample_evaluated",function(_) end)
      return i
    "#;
    let error = match lua::read_program_with_root(script, &root) {
        Ok(_) => panic!("invalid event unexpectedly loaded"),
        Err(error) => error,
    };
    assert!(
        error.contains("unknown event 'sample_evaluated'"),
        "{error}"
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn ordinary_graph_loading_drops_authored_observers_and_restores_zero_interest() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/instruments");
    let graph = lua::read_with_root(EXAMPLE, &root).unwrap();
    let mut unsubscribed = compile_at_rate(&graph, 16.0).unwrap();
    let source = unsubscribed.sequencer_controls()[0].event_source().clone();
    let mut plain_left = [0.0; 64];
    let mut plain_right = [0.0; 64];
    unsubscribed.process_stereo(&mut plain_left, &mut plain_right);
    assert_eq!(source.diagnostics(), EventDiagnostics::default());

    let program = lua::read_program_with_root(EXAMPLE, &root).unwrap();
    let mut subscribed = compile_at_rate(program.graph(), 16.0).unwrap();
    let mut event_left = [0.0; 64];
    let mut event_right = [0.0; 64];
    subscribed.process_stereo(&mut event_left, &mut event_right);
    assert_eq!(plain_left, event_left);
    assert_eq!(plain_right, event_right);
}
