//! Pinned historical corpus, independent of new Lua examples and their loader.
//! Do not remove a row when it fails or rewrite the historical source fixture.
use meldritch_core::FrameRange;
use meldritch_dsl::{compile_project, load_song_directory, parse_project};
use meldritch_render::song_render::*;
use meldritch_render::{RenderSettings, render_pattern_clicks, render_pattern_samples_chunk};
use std::collections::BTreeMap;
use std::path::PathBuf;

fn repository() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn historical_song_directories_load_compile_and_retain_range_identical_audio() {
    // Preserve every original row and grow the corpus with accepted projects.
    // Design fixtures 05-08, 10, 12-14 do not claim current loader compatibility.
    let fixtures = [
        ("00-minimal-synth", "drone"),
        ("01-synth-note-pattern", "note"),
        ("02-synth-parameter-pattern", "filter"),
        ("03-dsp-chain", "delay"),
        ("04-dsp-parameter-pattern", "automated_delay"),
        ("09-curated-performance-controls", "delay"),
        ("11-session-capture", "delay"),
        ("15-launch-control-xl-input", "delay"),
        ("16-launch-control-xl-playground", "delay"),
        ("17-launch-control-xl-ensemble", "mix"),
        ("18-lua-control-plane", "drone"),
        ("19-apc-mini-mk2-playground", "mix"),
        ("21-lua-apc-performance", "mix"),
        ("22-midi-soft-takeover", "mix"),
        ("24-audio-aware-widgets", "mix"),
        ("25-edit-existing-synth", "mix"),
    ];
    for (fixture, compiler) in fixtures {
        let path = repository().join("songs/examples").join(fixture);
        let song = load_song_directory(&path).unwrap_or_else(|e| panic!("{fixture}: {e}"));
        if fixture == "17-launch-control-xl-ensemble" {
            // Captured from the production loader before adding song_audio.
            assert_eq!(format!("{:016x}", song.fingerprint()), "2010d44585037a44");
        }
        assert_eq!(
            song.fingerprint(),
            load_song_directory(&path).unwrap().fingerprint()
        );
        let render: Box<dyn Fn(FrameRange) -> meldritch_audio::AudioBlock> = match compiler {
            "drone" => {
                let p = compile_drone_song(&song).unwrap();
                Box::new(move |r| p.render(r).unwrap())
            }
            "note" => {
                let p = compile_note_song(&song).unwrap();
                Box::new(move |r| p.render(r).unwrap())
            }
            "filter" => {
                let p = compile_filtered_note_song(&song).unwrap();
                Box::new(move |r| p.render(r).unwrap())
            }
            "delay" => {
                let p = compile_delayed_note_song(&song).unwrap();
                Box::new(move |r| p.render(r).unwrap())
            }
            "automated_delay" => {
                let p = compile_automated_delayed_note_song(&song).unwrap();
                Box::new(move |r| p.render(r).unwrap())
            }
            "mix" => {
                let p = compile_mixed_note_song(&song).unwrap();
                Box::new(move |r| p.render(r).unwrap())
            }
            _ => unreachable!(),
        };
        let whole = render(FrameRange::new(0, 24000).unwrap());
        let left = render(FrameRange::new(0, 7331).unwrap());
        let right = render(FrameRange::new(7331, 24000).unwrap());
        assert!(
            whole.peak_abs() > 0.0,
            "{fixture}: previously playable audio is silent"
        );
        assert!(whole.samples().iter().all(|s| s.is_finite()), "{fixture}");
        assert_eq!(
            left.samples()
                .iter()
                .chain(right.samples())
                .copied()
                .collect::<Vec<_>>(),
            whole.samples(),
            "{fixture}: range determinism regressed"
        );
    }
}

#[test]
fn historical_legacy_projects_validate_compile_and_render_event_timing() {
    for fixture in [
        "minimal_kick",
        "basic_drums",
        "explicit_relations",
        "control_relations",
        "sidechain_relations",
        "showcase",
        "warehouse",
    ] {
        let source =
            std::fs::read_to_string(repository().join(format!("fixtures/{fixture}.toml"))).unwrap();
        let project = parse_project(&source).unwrap_or_else(|e| panic!("{fixture}: {e}"));
        let compiled = compile_project(&project).unwrap_or_else(|e| panic!("{fixture}: {e}"));
        assert!(!compiled.source_bindings().is_empty());
        let samples = project
            .samples()
            .iter()
            .map(|sample| {
                (
                    sample.note(),
                    meldritch_audio::read_wav(repository().join("fixtures").join(sample.path()))
                        .unwrap(),
                )
            })
            .collect::<BTreeMap<_, _>>();
        // Preserve both the click/event path and real fixture WAV playback.
        for pattern in project.patterns() {
            let range = FrameRange::new(0, 24000).unwrap();
            let a = render_pattern_clicks(
                pattern,
                project.tempo(),
                range,
                project.probability_seed(),
                RenderSettings::new(1).unwrap(),
            );
            let b = render_pattern_clicks(
                pattern,
                project.tempo(),
                range,
                project.probability_seed(),
                RenderSettings::new(1).unwrap(),
            );
            assert_eq!(a, b, "{fixture}");
            assert!(a.samples().iter().all(|s| s.is_finite()));
            let render = |range| {
                render_pattern_samples_chunk(
                    pattern,
                    project.tempo(),
                    range,
                    project.probability_seed(),
                    RenderSettings::new(1).unwrap(),
                    &samples,
                )
            };
            let whole = render(range);
            let left = render(FrameRange::new(0, 7331).unwrap());
            let right = render(FrameRange::new(7331, 24000).unwrap());
            assert_eq!(
                left.samples()
                    .iter()
                    .chain(right.samples())
                    .copied()
                    .collect::<Vec<_>>(),
                whole.samples(),
                "{fixture}: sample tails changed"
            );
            assert!(whole.samples().iter().all(|s| s.is_finite()));
        }
    }
}
