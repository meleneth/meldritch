#[path = "piano_roll.rs"]
mod piano;
use meldritch_control::devices::{MidiInputQueue, MidiPickup};
pub struct Harness {
    pub host: piano::Harness,
    pub pickup: MidiPickup,
    frame: u64,
}
pub fn fixture() -> std::path::PathBuf {
    piano::fixture()
        .parent()
        .unwrap()
        .join("28-author-fader-mappings")
}
impl Harness {
    pub fn new() -> Self {
        Self::from_fixture(&fixture())
    }
    pub fn from_fixture(fixture: &std::path::Path) -> Self {
        Self {
            host: piano::Harness::from_fixture(fixture),
            pickup: MidiPickup::default(),
            frame: 0,
        }
    }
    pub fn cc(&mut self, cc: u8, value: u8) -> bool {
        let queue = MidiInputQueue::default();
        let route = queue.route("apc-mini");
        assert!(route.observe(&[0xb0, cc, value]));
        let observed = queue.next().unwrap();
        assert!(observed.is_current());
        let h = &mut self.host;
        let result = h
            .performance
            .fader(
                &observed.device,
                &observed.bytes,
                self.frame,
                &mut h.app,
                &mut self.pickup,
                &mut h.host,
                &mut h.capture,
            )
            .unwrap()
            .unwrap();
        self.frame += 16;
        result
    }
    pub fn script(&mut self, source: &str) {
        for (index, line) in source
            .lines()
            .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
            .enumerate()
        {
            let args: Vec<_> = line.split_whitespace().collect();
            if args[0] == "cc" {
                assert_eq!(
                    self.cc(args[1].parse().unwrap(), args[2].parse().unwrap()),
                    args[3].parse::<bool>().unwrap(),
                    "event {index}: {line}"
                );
            } else {
                self.host.run_script(line)
            }
        }
    }
}
