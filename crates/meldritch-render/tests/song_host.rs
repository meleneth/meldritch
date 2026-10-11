use meldritch_core::FrameRange;
use meldritch_dsl::load_song_directory;
use meldritch_render::song_host::{CompiledSongPatch, SongLiveOverrides, SongRerenderWorker};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

#[test]
fn shared_worker_renders_latest_state_and_reports_invalid_overrides() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../songs/examples/19-apc-mini-mk2-playground");
    let song = load_song_directory(root).unwrap();
    let patch = CompiledSongPatch::compile_initial(&song).unwrap();
    let overrides = SongLiveOverrides::default();
    let expected = patch
        .render(FrameRange::new(0, 24000).unwrap(), &overrides)
        .unwrap();
    let mut worker = SongRerenderWorker::new(24000);
    for generation in 1..=64 {
        worker.submit(generation, patch.clone(), overrides.clone());
    }
    let deadline = Instant::now() + Duration::from_secs(5);
    let actual = loop {
        if let Some((generation, block, _)) = worker.latest_completed() {
            if generation == 64 {
                break block.unwrap();
            }
        }
        assert!(
            Instant::now() < deadline,
            "worker did not complete newest generation"
        );
        std::thread::yield_now();
    };
    assert_eq!(actual.samples(), expected.samples());
    let mut invalid = SongRerenderWorker::new(24000);
    let mut invalid_overrides = overrides;
    invalid_overrides
        .lane_levels
        .insert("bass-a".to_owned(), f64::NAN);
    invalid.submit(1, patch, invalid_overrides);
    let failure = loop {
        if let Some((_, result, _)) = invalid.latest_completed() {
            break result;
        }
        assert!(Instant::now() < deadline);
        std::thread::yield_now();
    };
    assert!(failure.is_err());
}
