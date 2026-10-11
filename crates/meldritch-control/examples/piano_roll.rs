//! Run reproducible pad scripts without hardware or writes to authored fixtures.
#[path = "support/piano_roll.rs"]
mod support;
fn main() {
    let mut host = support::Harness::new();
    let script = std::env::args()
        .nth(1)
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| support::fixture().join("scripts/compose.pads"));
    host.run_script(&std::fs::read_to_string(script).unwrap());
    let view = host.app.view_model().pattern_editor.unwrap();
    let text = meldritch_control::pattern::encode_pattern(&view).unwrap();
    let (reloaded, _) = meldritch_control::pattern::load_pattern(&host.root, &text).unwrap();
    assert_eq!(view.source.fingerprint(), reloaded.fingerprint());
    print!("{text}");
    eprintln!(
        "Headless round trip: {:016x}; physical acceptance pending",
        reloaded.fingerprint()
    );
}
