#[path = "support/authoring.rs"]
mod support;
fn main() {
    let args: Vec<_> = std::env::args_os().collect();
    if args.get(1).is_some_and(|s| s == "--reload") {
        support::reload(
            std::path::Path::new(&args[2]),
            std::path::Path::new(&args[3]),
        );
        return;
    }
    assert_eq!(args.get(1).unwrap(), "--output");
    let output = std::path::Path::new(&args[2]);
    let song = support::run(output);
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .arg("--reload")
        .arg(song)
        .arg(output)
        .status()
        .unwrap();
    assert!(status.success(), "fresh process reload failed");
    for (before, after) in [
        ("saved-state.json", "reloaded-state.json"),
        ("saved.f64le", "reloaded.f64le"),
    ] {
        assert_eq!(
            std::fs::read(output.join(before)).unwrap(),
            std::fs::read(output.join(after)).unwrap()
        );
    }
    println!("PASS: composed music restored in a fresh process; f64 samples identical.");
    println!(
        "Artifacts: {}. Physical APC/Stream Deck acceptance pending.",
        output.display()
    );
}
