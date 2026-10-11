//! Run the sample-backed drum policy without physical hardware.
#[path = "support/piano_roll.rs"]
mod support;
fn main() {
    let fixture = support::fixture()
        .parent()
        .unwrap()
        .join("31-apc-drum-editor");
    let mut host = support::Harness::from_fixture(&fixture);
    let script = std::env::args()
        .nth(1)
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| fixture.join("scripts/compose.pads"));
    host.run_script(&std::fs::read_to_string(script).unwrap());
    let view = host.app.view_model().pattern_editor.unwrap();
    let text = meldritch_control::pattern::encode_pattern(&view).unwrap();
    let (loaded, _) = meldritch_control::pattern::load_pattern(&host.root, &text).unwrap();
    assert_eq!(loaded.fingerprint(), view.source.fingerprint());
    print!("{text}");
    eprintln!(
        "Headless drum round trip: {:016x}; physical acceptance pending",
        loaded.fingerprint()
    );
}
