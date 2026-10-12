use meldritch_audio::{
    AudioBlock,
    audio_publication::audio_publication,
    device_output::{PublishedAudioRenderer, playback_session_parts},
    published_audio::PublishedAudio,
    streaming_audio::streaming_audio,
};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
};
thread_local! { static TRACK: Cell<bool> = const { Cell::new(false) }; static OPERATIONS: Cell<usize> = const { Cell::new(0) }; }
struct Allocator;
fn allocation() {
    if TRACK.try_with(Cell::get).unwrap_or(false) {
        OPERATIONS.with(|n| n.set(n.get() + 1));
    }
}
unsafe impl GlobalAlloc for Allocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        allocation();
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, p: *mut u8, layout: Layout) {
        allocation();
        unsafe { System.dealloc(p, layout) }
    }
    unsafe fn realloc(&self, p: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        allocation();
        unsafe { System.realloc(p, layout, size) }
    }
}
#[global_allocator]
static ALLOCATOR: Allocator = Allocator;

#[test]
fn timed_ring_atomic_negatives_epoch_gaps_and_u64_clock() {
    assert!(streaming_audio(0, 0).is_err());
    assert!(streaming_audio(4097, 0).is_err());
    let origin = 5_000_000_000;
    let (mut producer, mut consumer, control) = streaming_audio(4, origin).unwrap();
    let epoch = control.activate().unwrap();
    for (frame, values) in [
        (origin, vec![]),
        (origin, vec![0.5, f64::NAN]),
        (u64::MAX, vec![0.5]),
        (origin, vec![0.5; 5]),
    ] {
        assert!(producer.publish(frame, epoch, &values).is_err());
        assert_eq!(producer.available(), 4);
    }
    producer
        .publish(origin, epoch, &[0.1, 0.2, 0.3, 0.4])
        .unwrap();
    assert!(producer.publish(origin + 4, epoch, &[0.9]).is_err());
    consumer.begin_callback();
    assert_eq!(consumer.next_sample(), 0.1);
    assert!(producer.publish(origin + 2, epoch, &[0.9]).is_err());
    assert_eq!(consumer.next_sample(), 0.2);
    let next = control.activate().unwrap();
    producer.publish(origin + 4, next, &[0.5]).unwrap();
    // Stale buffered samples cannot become audible after activation. Gaps are silence.
    consumer.begin_callback();
    assert_eq!(consumer.next_sample(), 0.0);
    assert_eq!(consumer.next_sample(), 0.0);
    assert_eq!(consumer.next_sample(), 0.5);
    assert_eq!(consumer.next_sample(), 0.0);
    assert_eq!(control.clock(), origin + 6);
    producer.publish(origin + 5, next, &[0.8, 0.9]).unwrap();
    consumer.begin_callback();
    assert_eq!(consumer.next_sample(), 0.9); // Late sample discarded, never replayed.
    control.deactivate();
    producer.publish(origin + 7, next, &[0.7]).unwrap();
    consumer.begin_callback();
    assert_eq!(consumer.next_sample(), 0.0);
    assert!(!control.enabled());
    assert!(control.underruns() > 0);
}

#[test]
fn production_callback_streams_while_stopped_clamps_and_cleans_without_allocating() {
    let mut base = AudioBlock::silent(2, 1);
    base.samples_mut().copy_from_slice(&[0.75, -0.75]);
    let (_, reader) = audio_publication(PublishedAudio::from_block(&base, 1).unwrap());
    let (control, engine) = playback_session_parts(16).unwrap();
    let (mut producer, consumer, stream) = streaming_audio(16, 0).unwrap();
    let mut callback = PublishedAudioRenderer::from_engine(reader, 2, u32::MAX, engine)
        .unwrap()
        .with_streaming(consumer);
    let mut out = [9.0; 8];
    callback.render_f64(&mut out); // Warm existing ArcSwap callback path.
    let epoch = stream.activate().unwrap();
    producer.publish(4, epoch, &[0.2, -0.3, 2.0, -2.0]).unwrap();
    OPERATIONS.with(|count| count.set(0));
    TRACK.with(|track| track.set(true));
    callback.render_f64(&mut out);
    TRACK.with(|track| track.set(false));
    assert_eq!(OPERATIONS.with(Cell::get), 0);
    assert_eq!(out, [0.2, 0.2, -0.3, -0.3, 1.0, 1.0, -1.0, -1.0]);
    // Every explicit transport command disables audition before copying PCM.
    for action in [0, 1, 2] {
        let epoch = stream.activate().unwrap();
        producer.publish(stream.clock(), epoch, &[0.5; 4]).unwrap();
        match action {
            0 => control.play().unwrap(),
            1 => control.stop().unwrap(),
            _ => control.rewind().unwrap(),
        }
        TRACK.with(|track| track.set(true));
        callback.render_f64(&mut out);
        TRACK.with(|track| track.set(false));
        assert_eq!(OPERATIONS.with(Cell::get), 0);
        assert!(!stream.enabled());
        if action == 0 {
            assert_eq!(out, [0.75, -0.75, 0.75, -0.75, 0.75, -0.75, 0.75, -0.75]);
        } else {
            assert!(out.iter().all(|s| *s == 0.0));
        }
    }
}
