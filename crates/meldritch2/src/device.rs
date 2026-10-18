use std::sync::{
    Arc,
    atomic::{AtomicU8, AtomicU64, Ordering},
};

use cpal::{
    FromSample, Sample, SampleFormat,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};

use crate::{
    compiler::{ControlHandle, compile_at_rate},
    live::{LiveControl, LiveRuntime},
    live_notes::{LiveNoteConsumer, live_note_queue},
    midi::{MidiRouter, TimedMidiRouter},
    model::Graph,
};

const CHUNK_FRAMES: usize = 256;

fn stream_error_code(kind: cpal::ErrorKind) -> u8 {
    use cpal::ErrorKind;
    match kind {
        ErrorKind::DeviceBusy => 1,
        ErrorKind::DeviceChanged => 2,
        ErrorKind::DeviceNotAvailable => 3,
        ErrorKind::HostUnavailable => 4,
        ErrorKind::InvalidInput => 5,
        ErrorKind::PermissionDenied => 6,
        ErrorKind::RealtimeDenied => 7,
        ErrorKind::ResourceExhausted => 8,
        ErrorKind::StreamInvalidated => 9,
        ErrorKind::UnsupportedConfig => 10,
        ErrorKind::UnsupportedOperation => 11,
        ErrorKind::Xrun => 12,
        ErrorKind::BackendError => 13,
        ErrorKind::Other => 14,
        _ => 14,
    }
}

fn stream_error_label(code: u8) -> Option<&'static str> {
    match code {
        0 => None,
        1 => Some("audio device busy"),
        2 => Some("audio route changed"),
        3 => Some("audio device unavailable"),
        4 => Some("audio host unavailable"),
        5 => Some("invalid audio input"),
        6 => Some("audio device permission denied"),
        7 => Some("real-time scheduling denied"),
        8 => Some("audio resource exhausted"),
        9 => Some("audio stream invalidated"),
        10 => Some("unsupported audio configuration"),
        11 => Some("unsupported audio operation"),
        12 => Some("audio buffer underrun or overrun"),
        13 => Some("audio backend error"),
        _ => Some("other audio stream error"),
    }
}

fn record_stream_error(kind: cpal::ErrorKind, errors: &AtomicU64, first_error: &AtomicU8) {
    let _ = first_error.compare_exchange(
        0,
        stream_error_code(kind),
        Ordering::Release,
        Ordering::Relaxed,
    );
    errors.fetch_add(1, Ordering::Relaxed);
}

/// Copy mono graph output to every device channel without allocating.
pub fn fill_interleaved<T: Sample + FromSample<f64>>(
    runtime: &mut LiveRuntime,
    output: &mut [T],
    channels: usize,
    scratch: &mut [f64; CHUNK_FRAMES],
) {
    if channels == 0 || output.is_empty() {
        return;
    }
    runtime.begin_buffer();
    for chunk in output.chunks_mut(CHUNK_FRAMES * channels) {
        let frames = chunk.len().div_ceil(channels);
        runtime.process_prepared(&mut scratch[..frames]);
        for (frame, value) in chunk.chunks_mut(channels).zip(&scratch[..frames]) {
            let converted = T::from_sample(value.clamp(-1.0, 1.0));
            frame.fill(converted);
        }
    }
}

/// Render queued notes through one device callback snapshot, including chunks.
pub fn fill_interleaved_timed<T: Sample + FromSample<f64>>(
    runtime: &mut LiveRuntime,
    consumer: &mut LiveNoteConsumer,
    active: &mut bool,
    output: &mut [T],
    channels: usize,
    scratch: &mut [f64; CHUNK_FRAMES],
    errors: &AtomicU64,
) {
    if channels == 0 || output.is_empty() {
        return;
    }
    if runtime.begin_buffer() {
        // This queue is scoped to the original revision. A replacement uses
        // ordinary rendering until the output and MIDI route are reopened.
        *active = false;
    }
    for chunk in output.chunks_mut(CHUNK_FRAMES * channels) {
        let frames = chunk.len().div_ceil(channels);
        if *active {
            if consumer
                .render_prepared(runtime.current_mut(), &mut scratch[..frames])
                .is_err()
            {
                *active = false;
                errors.fetch_add(1, Ordering::Relaxed);
                runtime.process_prepared(&mut scratch[..frames]);
            }
        } else {
            runtime.process_prepared(&mut scratch[..frames]);
        }
        for (frame, value) in chunk.chunks_mut(channels).zip(&scratch[..frames]) {
            frame.fill(T::from_sample(value.clamp(-1.0, 1.0)));
        }
    }
}

pub struct DeviceOutput {
    stream: cpal::Stream,
    control: LiveControl,
    controls: Vec<(ControlHandle, f64)>,
    midi_router: MidiRouter,
    timed_midi_router: Option<TimedMidiRouter>,
    errors: Arc<AtomicU64>,
    timed_note_errors: Arc<AtomicU64>,
    first_error: Arc<AtomicU8>,
    pub device_name: String,
    pub sample_rate: u32,
    pub channels: u16,
}

pub fn output_device_names() -> Result<Vec<String>, String> {
    let host = cpal::default_host();
    let devices = host
        .output_devices()
        .map_err(|error| format!("failed to list output devices: {error}"))?;
    Ok(devices
        .map(|device| {
            device
                .description()
                .map(|description| description.name().to_owned())
                .unwrap_or_else(|_| "unknown output device".into())
        })
        .collect())
}

impl DeviceOutput {
    pub fn open_default(graph: &Graph) -> Result<Self, String> {
        Self::open(graph, None)
    }

    pub fn open_index(graph: &Graph, index: usize) -> Result<Self, String> {
        Self::open(graph, Some(index))
    }

    fn open(graph: &Graph, index: Option<usize>) -> Result<Self, String> {
        let host = cpal::default_host();
        let device = match index {
            Some(index) => host
                .output_devices()
                .map_err(|error| format!("failed to list output devices: {error}"))?
                .nth(index)
                .ok_or_else(|| format!("output device index {index} does not exist"))?,
            None => host
                .default_output_device()
                .ok_or("no default audio output device is available")?,
        };
        let device_name = device
            .description()
            .map(|description| description.name().to_owned())
            .unwrap_or_else(|_| "unknown output device".into());
        let supported = device
            .default_output_config()
            .map_err(|error| format!("failed to get output configuration: {error}"))?;
        let format = supported.sample_format();
        let config: cpal::StreamConfig = supported.into();
        if config.channels == 0 {
            return Err("output device reported zero channels".into());
        }
        let machine = compile_at_rate(graph, f64::from(config.sample_rate))?;
        let controls = machine
            .control_ids()
            .map(|id| {
                let handle = machine.control_handle(id).expect("compiled control");
                let value = machine
                    .effective_value(handle.target())
                    .expect("compiled parameter");
                (handle, value)
            })
            .collect();
        let midi_router = MidiRouter::from_machine(&machine);
        let (timed_midi_router, mut note_consumer) =
            if machine.midi_notes().is_empty() && !machine.has_poly_notes() {
                (None, None)
            } else {
                let (producer, consumer) = live_note_queue(&machine, 0);
                let router = TimedMidiRouter::new(&machine, producer, config.sample_rate, 256)
                    .map_err(|error| format!("invalid MIDI note clock: {error:?}"))?;
                (Some(router), Some(consumer))
            };
        let (runtime, control) = LiveRuntime::new(machine);
        let errors = Arc::new(AtomicU64::new(0));
        let timed_note_errors = Arc::new(AtomicU64::new(0));
        let first_error = Arc::new(AtomicU8::new(0));
        let channels = usize::from(config.channels);
        macro_rules! build {
            ($sample:ty) => {{
                let mut runtime = runtime;
                let mut scratch = [0.0; CHUNK_FRAMES];
                let errors = Arc::clone(&errors);
                let first_error = Arc::clone(&first_error);
                let note_errors = Arc::clone(&timed_note_errors);
                let mut note_active = true;
                device.build_output_stream(
                    config,
                    move |output: &mut [$sample], _| {
                        if let Some(consumer) = &mut note_consumer {
                            fill_interleaved_timed(
                                &mut runtime,
                                consumer,
                                &mut note_active,
                                output,
                                channels,
                                &mut scratch,
                                &note_errors,
                            );
                        } else {
                            fill_interleaved(&mut runtime, output, channels, &mut scratch);
                        }
                    },
                    move |error| {
                        record_stream_error(error.kind(), &errors, &first_error);
                    },
                    None,
                )
            }};
        }
        let result = match format {
            SampleFormat::F32 => build!(f32),
            SampleFormat::F64 => build!(f64),
            SampleFormat::I8 => build!(i8),
            SampleFormat::I16 => build!(i16),
            SampleFormat::I32 => build!(i32),
            SampleFormat::I64 => build!(i64),
            SampleFormat::U8 => build!(u8),
            SampleFormat::U16 => build!(u16),
            SampleFormat::U32 => build!(u32),
            SampleFormat::U64 => build!(u64),
            _ => return Err(format!("unsupported output sample format: {format}")),
        };
        let stream = result.map_err(|error| format!("failed to build output stream: {error}"))?;
        Ok(Self {
            stream,
            control,
            controls,
            midi_router,
            timed_midi_router,
            errors,
            timed_note_errors,
            first_error,
            device_name,
            sample_rate: config.sample_rate,
            channels: config.channels,
        })
    }

    pub fn start(&self) -> Result<(), String> {
        self.stream
            .play()
            .map_err(|error| format!("failed to start output stream: {error}"))
    }

    pub fn control(&self) -> &LiveControl {
        &self.control
    }

    pub fn controls(&self) -> Vec<(ControlHandle, f64)> {
        self.controls.clone()
    }
    pub fn midi_router(&self) -> MidiRouter {
        self.midi_router.clone()
    }
    pub fn take_timed_midi_router(&mut self) -> Option<TimedMidiRouter> {
        self.timed_midi_router.take()
    }
    pub fn timed_note_errors(&self) -> u64 {
        self.timed_note_errors.load(Ordering::Relaxed)
    }
    pub fn stream_errors(&self) -> u64 {
        self.errors.load(Ordering::Relaxed)
    }

    pub fn first_stream_error(&self) -> Option<&'static str> {
        stream_error_label(self.first_error.load(Ordering::Acquire))
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CHUNK_FRAMES, fill_interleaved, fill_interleaved_timed, record_stream_error,
        stream_error_label,
    };
    use crate::{
        compiler::compile,
        live::LiveRuntime,
        live_notes::live_note_queue,
        lua,
        midi::TimedMidiRouter,
        timed::{TimedNotePlan, TimedNoteState},
    };
    use std::sync::atomic::{AtomicU8, AtomicU64, Ordering};

    #[test]
    fn stream_error_recording_keeps_first_kind_without_formatting() {
        let errors = AtomicU64::new(0);
        let first = AtomicU8::new(0);
        record_stream_error(cpal::ErrorKind::Xrun, &errors, &first);
        record_stream_error(cpal::ErrorKind::DeviceNotAvailable, &errors, &first);
        assert_eq!(errors.load(Ordering::Relaxed), 2);
        assert_eq!(
            stream_error_label(first.load(Ordering::Acquire)),
            Some("audio buffer underrun or overrun")
        );
    }

    #[test]
    fn mono_values_fill_all_channels_across_chunks() {
        let graph = lua::read(include_str!("../examples/01-basic.lua")).unwrap();
        let (mut runtime, _) = LiveRuntime::new(compile(&graph).unwrap());
        let mut scratch = [0.0; CHUNK_FRAMES];
        let mut output = vec![0.0_f32; 301 * 2];
        fill_interleaved(&mut runtime, &mut output, 2, &mut scratch);
        assert!(output.iter().all(|sample| *sample == 0.125));
    }

    #[test]
    fn integer_conversion_and_sine_phase_cross_chunk_boundary() {
        let graph = lua::read(include_str!("../examples/08-sine.lua")).unwrap();
        let (mut runtime, _) = LiveRuntime::new(compile(&graph).unwrap());
        let mut scratch = [0.0; CHUNK_FRAMES];
        let mut output = vec![0_i16; 260];
        fill_interleaved(&mut runtime, &mut output, 1, &mut scratch);
        assert_eq!(output[0], 0);
        assert!((i32::from(output[1]) - 8192).abs() <= 1);
        assert_eq!(output[256], 0);
        assert!((i32::from(output[257]) - 8192).abs() <= 1);
    }

    #[test]
    fn timestamped_midi_renders_through_real_chunked_output_boundary() {
        let graph = lua::read(include_str!("../examples/47-device-timed-midi.lua")).unwrap();
        let machine = compile(&graph).unwrap();
        let (producer, mut consumer) = live_note_queue(&machine, 0);
        let mut midi = TimedMidiRouter::new(&machine, producer, 48_000, 3).unwrap();
        for (time, message) in [
            (0, [0x90, 69, 100]),
            (5_355, [0x90, 81, 100]),
            (5_980, [0x80, 81, 0]),
            (6_300, [0x80, 69, 0]),
        ] {
            assert_eq!(midi.dispatch(time, &message), Ok(true));
        }
        let (mut runtime, _) = LiveRuntime::new(machine);
        let mut reference = compile(&graph).unwrap();
        let mut plan = TimedNotePlan::prepare(
            &reference,
            0,
            &[
                TimedNoteState {
                    frame: 3,
                    channel: 1,
                    pitch_hz: 440.0,
                    gate: true,
                },
                TimedNoteState {
                    frame: 260,
                    channel: 1,
                    pitch_hz: 880.0,
                    gate: true,
                },
                TimedNoteState {
                    frame: 290,
                    channel: 1,
                    pitch_hz: 440.0,
                    gate: true,
                },
                TimedNoteState {
                    frame: 305,
                    channel: 1,
                    pitch_hz: 440.0,
                    gate: false,
                },
            ],
        )
        .unwrap();
        let mut expected = [0.0; 300];
        plan.render_next(&mut reference, &mut expected).unwrap();
        let mut output = [f64::NAN; 600];
        let mut scratch = [0.0; CHUNK_FRAMES];
        let mut active = true;
        let errors = AtomicU64::new(0);
        fill_interleaved_timed(
            &mut runtime,
            &mut consumer,
            &mut active,
            &mut output,
            2,
            &mut scratch,
            &errors,
        );
        for (frame, pair) in output.chunks_exact(2).enumerate() {
            assert!((pair[0] - expected[frame]).abs() < 1e-12, "frame {frame}");
            assert_eq!(pair[0], pair[1]);
        }
        assert_eq!(midi.first_unrendered_frame(), 300);
        assert_eq!(errors.load(Ordering::Relaxed), 0);

        assert_eq!(midi.dispatch(7_000, &[0xB0, 7, 127]), Ok(true));
        reference
            .control_handle("level")
            .unwrap()
            .publish(1.0)
            .unwrap();
        let mut next_expected = [0.0; 16];
        plan.render_next(&mut reference, &mut next_expected)
            .unwrap();
        let mut next = [f64::NAN; 16];
        fill_interleaved_timed(
            &mut runtime,
            &mut consumer,
            &mut active,
            &mut next,
            1,
            &mut scratch,
            &errors,
        );
        for (frame, actual) in next.iter().enumerate() {
            assert!(
                (actual - next_expected[frame]).abs() < 1e-12,
                "next frame {frame}"
            );
        }
        assert_eq!(midi.first_unrendered_frame(), 316);
        assert_eq!(errors.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn graph_replacement_disables_old_timed_queue_without_callback_drop() {
        let graph = lua::read(include_str!("../examples/47-device-timed-midi.lua")).unwrap();
        let machine = compile(&graph).unwrap();
        let (_, mut consumer) = live_note_queue(&machine, 0);
        let (mut runtime, control) = LiveRuntime::new(machine);
        let replacement = lua::read(include_str!("../examples/01-basic.lua")).unwrap();
        control.submit(&replacement).unwrap();
        let mut active = true;
        let mut scratch = [0.0; CHUNK_FRAMES];
        let mut output = [f64::NAN; 16];
        let errors = AtomicU64::new(0);
        fill_interleaved_timed(
            &mut runtime,
            &mut consumer,
            &mut active,
            &mut output,
            1,
            &mut scratch,
            &errors,
        );
        assert_eq!(output, [0.125; 16]);
        assert!(!active);
        assert_eq!(errors.load(Ordering::Relaxed), 0);
        assert_eq!(consumer.cursor(), 0);
    }

    #[test]
    fn timed_midi_velocity_routes_through_explicit_graph_modulation() {
        let graph = lua::read(include_str!("../examples/50-midi-velocity.lua")).unwrap();
        let messages = [
            (0, [0x90, 69, 64]),
            (42, [0x90, 69, 32]),
            (84, [0x90, 81, 127]),
            (167, [0x80, 81, 0]),
            (250, [0x80, 69, 0]),
        ];
        let machine = compile(&graph).unwrap();
        let (producer, mut consumer) = live_note_queue(&machine, 0);
        let mut midi = TimedMidiRouter::new(&machine, producer, 48_000, 3).unwrap();
        for (time, message) in messages {
            assert_eq!(midi.dispatch(time, &message), Ok(true));
        }
        let (mut runtime, _) = LiveRuntime::new(machine);
        let mut whole = [f64::NAN; 20];
        let mut scratch = [0.0; CHUNK_FRAMES];
        let mut active = true;
        let errors = AtomicU64::new(0);
        fill_interleaved_timed(
            &mut runtime,
            &mut consumer,
            &mut active,
            &mut whole,
            1,
            &mut scratch,
            &errors,
        );
        assert_eq!(errors.load(Ordering::Relaxed), 0);
        let mut envelope = 0.0;
        let mut slewed_velocity = 0.0;
        for (frame, sample) in whole.iter().enumerate() {
            let cycles = if frame < 7 {
                440.0 * frame as f64
            } else if frame < 11 {
                440.0 * 7.0 + 880.0 * (frame - 7) as f64
            } else {
                440.0 * 7.0 + 880.0 * 4.0 + 440.0 * (frame - 11) as f64
            };
            let velocity: f64 = match frame {
                0..=2 => 1.0,
                3..=4 => 64.0 / 127.0,
                5..=6 | 11.. => 32.0 / 127.0,
                7..=10 => 1.0,
            };
            let gate_open = (3..15).contains(&frame);
            envelope += if gate_open { 0.01 } else { 0.005 } * (f64::from(gate_open) - envelope);
            slewed_velocity += 0.01 * (velocity - slewed_velocity);
            let expected = 0.25
                * envelope
                * slewed_velocity
                * (std::f64::consts::TAU * cycles / 48_000.0).sin();
            assert!(
                (sample - expected).abs() < 1e-12,
                "frame {frame}: {sample} != {expected}"
            );
        }

        let mut split_machine = compile(&graph).unwrap();
        let (producer, mut split_consumer) = live_note_queue(&split_machine, 0);
        let mut split_midi = TimedMidiRouter::new(&split_machine, producer, 48_000, 3).unwrap();
        for (time, message) in messages {
            assert_eq!(split_midi.dispatch(time, &message), Ok(true));
        }
        let mut a = [0.0; 6];
        let mut b = [0.0; 6];
        let mut c = [0.0; 8];
        split_consumer
            .render_next(&mut split_machine, &mut a)
            .unwrap();
        split_consumer
            .render_next(&mut split_machine, &mut b)
            .unwrap();
        split_consumer
            .render_next(&mut split_machine, &mut c)
            .unwrap();
        assert_eq!(whole[..6], a);
        assert_eq!(whole[6..12], b);
        assert_eq!(whole[12..], c);
    }

    #[test]
    fn playable_note_edges_do_not_jump_at_gate_changes() {
        for script in [
            include_str!("../examples/50-midi-velocity.lua"),
            include_str!("../examples/53-tui-keys.lua"),
        ] {
            let graph = lua::read(script).unwrap();
            let mut machine = compile(&graph).unwrap();
            let (producer, mut consumer) = live_note_queue(&machine, 0);
            let mut midi = TimedMidiRouter::new(&machine, producer, 48_000, 3).unwrap();
            assert_eq!(midi.dispatch(0, &[0x90, 69, 127]), Ok(true));
            assert_eq!(midi.dispatch(50_000, &[0x80, 69, 0]), Ok(true));

            let mut samples = [0.0; 4096];
            for chunk in samples.chunks_mut(1024) {
                consumer.render_next(&mut machine, chunk).unwrap();
            }
            let release_frame = 2403;
            assert!(
                samples[release_frame - 100..release_frame]
                    .iter()
                    .any(|sample| sample.abs() > 0.2)
            );
            for edge in [3, release_frame] {
                let step = (samples[edge] - samples[edge - 1]).abs();
                assert!(step < 0.02, "frame {edge}: edge jump {step}");
            }
            assert!(samples[4095].abs() < 0.001);
        }
    }
}
