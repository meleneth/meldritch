#[path = "support/assignments.rs"]
mod support;
fn main() {
    let mut h = support::Harness::new();
    h.script(include_str!(
        "../../../songs/examples/28-author-fader-mappings/scripts/compose.faders"
    ));
    let text =
        meldritch_control::assignments::encode_controls(&h.host.app.assignment_view().unwrap())
            .unwrap();
    print!("{text}");
    eprintln!("Headless assignment/mixer acceptance; physical acceptance pending");
}
