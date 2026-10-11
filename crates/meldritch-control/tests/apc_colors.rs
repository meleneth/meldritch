use meldritch_control::led_demo::ApcLedDemo;
use std::path::PathBuf;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../songs/examples/20-apc-lua-colors")
}

#[test]
fn lua_animation_has_deterministic_protocol_messages_and_final_playground_hints() {
    let source = std::fs::read_to_string(root().join("colors.lua")).unwrap();
    let plan = ApcLedDemo::load(&source).unwrap();
    assert_eq!(plan.frames.len(), 9);
    assert_eq!(plan.duration_ms(), 1440);
    for frame in &plan.frames {
        assert_eq!(frame.messages().len(), 80);
    }
    assert_eq!(plan.frames[0].messages()[0], [0x92, 0, 5]);
    assert_eq!(plan.frames[1].messages()[0], [0x92, 0, 9]);
    assert_eq!(plan.frames[0].messages()[64], [0x90, 100, 1]);
    assert_eq!(plan.frames[0].messages()[65], [0x90, 112, 1]);
    let final_frame = plan.frames.last().unwrap().messages();
    assert_eq!(final_frame[0], [0x92, 0, 9]);
    assert_eq!(final_frame[32], [0x92, 32, 5]);
    assert_eq!(final_frame[40], [0x92, 40, 13]);
    assert_eq!(final_frame[63], [0x92, 63, 0]);
    assert_eq!(plan, ApcLedDemo::load(&source).unwrap());
    assert_eq!(ApcLedDemo::clear_messages().len(), 80);
}

#[test]
fn invalid_plans_and_unrestricted_lua_are_rejected_before_transport() {
    for (name, diagnostic) in [
        ("duplicate-pad", "duplicate pad"),
        ("invalid-button", "button note"),
    ] {
        let source = std::fs::read_to_string(root().join(format!("invalid/{name}.lua"))).unwrap();
        assert!(ApcLedDemo::load(&source).unwrap_err().contains(diagnostic));
    }
    for source in [
        "return {version=1, frames={{hold_ms=1001,pads={},buttons={}}}}",
        "return {version=1, frames={{hold_ms=0,pads={{cell=64,color=5}},buttons={}}}}",
        "return {version=1, frames={{hold_ms=0,pads={{cell=0,color=128}},buttons={}}}}",
        "return {version=1, frames={{hold_ms=0,pads={{cell=0,color=5,brightness=7}},buttons={}}}}",
        "return {version=1, frames={{hold_ms=0,pads={},buttons={},raw_midi={144,0,5}}}}",
        "return {version=1, frames={}}",
        "return {version=1, frames={bad={hold_ms=0,pads={},buttons={}}}}",
        "return {version=1, frames={{hold_ms=0,pads={bad={cell=0,color=5}},buttons={}}}}",
        "return {version=2, frames={}}",
        "return os.execute('true')",
        "return require('io')",
        "while true do end",
        "local f={}; for i=1,11 do f[i]={hold_ms=1000,pads={},buttons={}} end; return {version=1,frames=f}",
    ] {
        assert!(ApcLedDemo::load(source).is_err(), "must reject: {source}");
    }
}
