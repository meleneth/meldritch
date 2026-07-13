//! CPAL-backed realtime output boundary.
//!
//! Everything expensive happens before the stream starts. The device callback
//! owns immutable, pre-rendered audio and only advances a cursor, applies a
//! safety clamp, converts sample formats, and updates atomics.

use crate::audio_publication::{AudioSnapshotReader, audio_publication};
use crate::published_audio::PublishedAudio;
use crate::realtime_queue::{
    QueueDiagnostics, QueueFull, QueueMonitor, TransportCommand, TransportCommandConsumer,
    TransportCommandProducer, transport_command_queue,
};
use crate::realtime_status::{
    RealtimeStatusMonitor, RealtimeStatusPublisher, StreamErrorReporter, realtime_status,
};
use crate::transport::PlaybackTransport;
use crate::{AudioBlock, Sample, SampleRate};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, Sample as DeviceSample, SampleFormat};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlaybackReport {
    pub device_name: String,
    pub sample_format: SampleFormat,
    pub sample_rate: SampleRate,
    pub channels: u16,
    pub callbacks: u64,
    pub stream_errors: u64,
    pub commands_applied: u64,
    pub commands_dropped: u64,
    pub underruns: u64,
    pub missed_artifacts: u64,
    pub final_position: u32,
}

#[derive(Default)]
struct PlaybackCounters {
    finished: AtomicBool,
}

pub struct PlaybackControl {
    commands: TransportCommandProducer,
    command_monitor: QueueMonitor,
    status_monitor: RealtimeStatusMonitor,
}

pub struct PlaybackEngine {
    commands: TransportCommandConsumer,
    status: RealtimeStatusPublisher,
    status_monitor: RealtimeStatusMonitor,
    error_reporter: StreamErrorReporter,
}

pub fn playback_session_parts(
    command_capacity: usize,
) -> Result<(PlaybackControl, PlaybackEngine), String> {
    let (commands, command_consumer) = transport_command_queue(command_capacity)?;
    let command_monitor = commands.monitor();
    let (status, status_monitor, error_reporter) = realtime_status();
    Ok((
        PlaybackControl {
            commands,
            command_monitor: command_monitor.clone(),
            status_monitor: status_monitor.clone(),
        },
        PlaybackEngine {
            commands: command_consumer,
            status,
            status_monitor,
            error_reporter,
        },
    ))
}

impl PlaybackControl {
    pub fn play(&self) -> Result<(), QueueFull> {
        self.commands.try_push(TransportCommand::Play)
    }

    pub fn stop(&self) -> Result<(), QueueFull> {
        self.commands.try_push(TransportCommand::Stop)
    }

    pub fn rewind(&self) -> Result<(), QueueFull> {
        self.commands.try_push(TransportCommand::Rewind)
    }

    #[must_use]
    pub fn status_monitor(&self) -> RealtimeStatusMonitor {
        self.status_monitor.clone()
    }

    #[must_use]
    pub fn command_diagnostics(&self) -> QueueDiagnostics {
        self.command_monitor.diagnostics()
    }
}

struct LoopingAudio {
    audio: AudioSnapshotReader,
    device_channels: usize,
    transport: PlaybackTransport,
    commands: TransportCommandConsumer,
    status: RealtimeStatusPublisher,
    #[cfg(test)]
    status_monitor: RealtimeStatusMonitor,
    error_reporter: StreamErrorReporter,
    last_missing_chunk: Option<usize>,
}

impl LoopingAudio {
    #[cfg(test)]
    fn new(block: &AudioBlock, device_channels: u16, loops: u32) -> Result<Self, String> {
        let audio = PublishedAudio::from_block(block, 4096)
            .map_err(|err| format!("cannot publish playback audio: {err:?}"))?;
        Self::from_published(audio, device_channels, loops)
    }

    #[cfg(test)]
    fn from_published(
        audio: PublishedAudio,
        device_channels: u16,
        loops: u32,
    ) -> Result<Self, String> {
        let (_, reader) = audio_publication(audio);
        Self::from_reader(reader, device_channels, loops)
    }

    #[cfg(test)]
    fn from_reader(
        audio: AudioSnapshotReader,
        device_channels: u16,
        loops: u32,
    ) -> Result<Self, String> {
        let (control, engine) = playback_session_parts(16)?;
        control
            .play()
            .map_err(|_| "failed to enqueue initial play command".to_owned())?;
        Self::from_engine(audio, device_channels, loops, engine)
    }

    fn from_engine(
        audio: AudioSnapshotReader,
        device_channels: u16,
        loops: u32,
        engine: PlaybackEngine,
    ) -> Result<Self, String> {
        if loops == 0 {
            return Err("playback loop count must be at least one".to_owned());
        }

        let frames = audio.snapshot().frames();
        let transport = PlaybackTransport::new(0, frames, loops).map_err(|err| err.to_string())?;
        let PlaybackEngine {
            commands,
            mut status,
            status_monitor,
            error_reporter,
        } = engine;
        #[cfg(not(test))]
        let _ = status_monitor;
        status.publish_transport(&transport);

        Ok(Self {
            audio,
            device_channels: usize::from(device_channels),
            transport,
            commands,
            status,
            #[cfg(test)]
            status_monitor,
            error_reporter,
            last_missing_chunk: None,
        })
    }

    fn fill<T>(&mut self, output: &mut [T]) -> bool
    where
        T: DeviceSample + FromSample<Sample>,
    {
        self.apply_commands();
        let audio = self.audio.snapshot();
        let mut finished = false;
        let mut callback_underrun = false;
        for output_frame in output.chunks_mut(self.device_channels) {
            let Some(frame) = self.transport.next_frame() else {
                for sample in output_frame {
                    *sample = T::from_sample(0.0);
                }
                finished = self.transport.is_finished();
                continue;
            };

            match audio.frame(frame) {
                Ok(source_frame) => {
                    self.last_missing_chunk = None;
                    for (channel, output_sample) in output_frame.iter_mut().enumerate() {
                        let source_channel = channel.min(source_frame.len() - 1);
                        let sample = source_frame[source_channel].clamp(-1.0, 1.0);
                        *output_sample = T::from_sample(sample);
                    }
                }
                Err(chunk_index) => {
                    for sample in output_frame {
                        *sample = T::from_sample(0.0);
                    }
                    callback_underrun = true;
                    if self.last_missing_chunk != Some(chunk_index) {
                        self.status.record_missed_artifact();
                        self.last_missing_chunk = Some(chunk_index);
                    }
                }
            }
            finished = self.transport.is_finished();
        }
        if callback_underrun {
            self.status.record_underrun();
        }
        self.status.publish_transport(&self.transport);
        self.status.callback_completed();
        finished
    }

    fn apply_commands(&mut self) {
        while let Some(command) = self.commands.try_pop() {
            match command {
                TransportCommand::Play => self.transport.play(),
                TransportCommand::Stop => self.transport.stop(),
                TransportCommand::Rewind => self.transport.rewind(),
            }
        }
    }

    #[cfg(test)]
    fn queue_diagnostics(&self) -> QueueDiagnostics {
        self.commands.diagnostics()
    }

    #[cfg(test)]
    fn status_monitor(&self) -> RealtimeStatusMonitor {
        self.status_monitor.clone()
    }

    fn error_reporter(&self) -> StreamErrorReporter {
        self.error_reporter.clone()
    }
}

pub struct RealtimePlaybackSession {
    _stream: cpal::Stream,
    command_monitor: QueueMonitor,
    status_monitor: RealtimeStatusMonitor,
    counters: Arc<PlaybackCounters>,
    device_name: String,
    sample_format: SampleFormat,
    sample_rate: SampleRate,
    channels: u16,
}

impl RealtimePlaybackSession {
    pub fn open_default(
        audio: AudioSnapshotReader,
        sample_rate: SampleRate,
        loops: u32,
        control: &PlaybackControl,
        engine: PlaybackEngine,
    ) -> Result<Self, String> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or_else(|| "no default audio output device is available".to_owned())?;
        let device_name = device
            .description()
            .map(|description| description.name().to_owned())
            .unwrap_or_else(|_| "unknown output device".to_owned());
        let source_channels = audio.snapshot().channels();
        let supported = select_config(&device, sample_rate, source_channels)?;
        let sample_format = supported.sample_format();
        let config: cpal::StreamConfig = supported.into();
        let counters = Arc::new(PlaybackCounters::default());
        let command_monitor = control.command_monitor.clone();
        let status_monitor = control.status_monitor.clone();
        let state = LoopingAudio::from_engine(audio, config.channels, loops, engine)?;
        let error_reporter = state.error_reporter();
        let stream = build_stream(
            &device,
            &config,
            sample_format,
            state,
            Arc::clone(&counters),
            error_reporter,
        )?;
        stream
            .play()
            .map_err(|err| format!("failed to start output stream: {err}"))?;
        Ok(Self {
            _stream: stream,
            command_monitor,
            status_monitor,
            counters,
            device_name,
            sample_format,
            sample_rate: config.sample_rate,
            channels: config.channels,
        })
    }

    #[must_use]
    pub fn status_monitor(&self) -> RealtimeStatusMonitor {
        self.status_monitor.clone()
    }

    #[must_use]
    pub fn is_finished(&self) -> bool {
        self.counters.finished.load(Ordering::Acquire)
    }

    #[must_use]
    pub fn report(&self) -> PlaybackReport {
        let status = self.status_monitor.snapshot();
        let commands = self.command_monitor.diagnostics();
        PlaybackReport {
            device_name: self.device_name.clone(),
            sample_format: self.sample_format,
            sample_rate: self.sample_rate,
            channels: self.channels,
            callbacks: status.callbacks,
            stream_errors: status.stream_errors,
            commands_applied: commands.applied,
            commands_dropped: commands.dropped,
            underruns: status.underruns,
            missed_artifacts: status.missed_artifacts,
            final_position: status.position,
        }
    }
}

/// Play a pre-rendered block on the default output device and return after the
/// requested loop count. Parsing, rendering, and file I/O must happen before
/// calling this function.
pub fn play_blocking(
    block: &AudioBlock,
    sample_rate: SampleRate,
    loops: u32,
) -> Result<PlaybackReport, String> {
    let audio = PublishedAudio::from_block(block, 4096)
        .map_err(|err| format!("cannot publish playback audio: {err:?}"))?;
    let (_, reader) = audio_publication(audio);
    let (control, engine) = playback_session_parts(16)?;
    control
        .play()
        .map_err(|_| "failed to enqueue initial play command".to_owned())?;
    let session =
        RealtimePlaybackSession::open_default(reader, sample_rate, loops, &control, engine)?;

    let audio_duration = Duration::from_secs_f64(
        f64::from(block.frames()) * f64::from(loops) / f64::from(sample_rate),
    );
    let deadline = Instant::now() + audio_duration + Duration::from_secs(5);
    while !session.is_finished() {
        if Instant::now() >= deadline {
            return Err("audio device timed out before playback completed".to_owned());
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    Ok(session.report())
}

fn select_config(
    device: &cpal::Device,
    sample_rate: SampleRate,
    preferred_channels: u16,
) -> Result<cpal::SupportedStreamConfig, String> {
    let configs = device
        .supported_output_configs()
        .map_err(|err| format!("failed to query output configurations: {err}"))?;
    let mut candidates = configs
        .filter(|config| {
            config.min_sample_rate() <= sample_rate
                && sample_rate <= config.max_sample_rate()
                && is_supported_format(config.sample_format())
        })
        .collect::<Vec<_>>();
    candidates.sort_by_key(|config| {
        let channel_penalty = u16::abs_diff(config.channels(), preferred_channels);
        (channel_penalty, format_rank(config.sample_format()))
    });
    candidates
        .into_iter()
        .next()
        .map(|config| config.with_sample_rate(sample_rate))
        .ok_or_else(|| format!("default output device does not support {sample_rate} Hz"))
}

fn is_supported_format(format: SampleFormat) -> bool {
    matches!(
        format,
        SampleFormat::F32
            | SampleFormat::F64
            | SampleFormat::I8
            | SampleFormat::I16
            | SampleFormat::I32
            | SampleFormat::I64
            | SampleFormat::U8
            | SampleFormat::U16
            | SampleFormat::U32
            | SampleFormat::U64
    )
}

fn format_rank(format: SampleFormat) -> u8 {
    match format {
        SampleFormat::F32 => 0,
        SampleFormat::F64 => 1,
        SampleFormat::I16 => 2,
        _ => 3,
    }
}

fn build_stream(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    format: SampleFormat,
    state: LoopingAudio,
    counters: Arc<PlaybackCounters>,
    error_reporter: StreamErrorReporter,
) -> Result<cpal::Stream, String> {
    macro_rules! stream_for {
        ($sample:ty) => {{
            let mut state = state;
            let callback_counters = Arc::clone(&counters);
            let error_reporter = error_reporter.clone();
            device.build_output_stream(
                *config,
                move |output: &mut [$sample], _| {
                    let finished = state.fill(output);
                    callback_counters
                        .finished
                        .store(finished, Ordering::Release);
                },
                move |_| {
                    error_reporter.record_error();
                },
                None,
            )
        }};
    }

    let result = match format {
        SampleFormat::F32 => stream_for!(f32),
        SampleFormat::F64 => stream_for!(f64),
        SampleFormat::I8 => stream_for!(i8),
        SampleFormat::I16 => stream_for!(i16),
        SampleFormat::I32 => stream_for!(i32),
        SampleFormat::I64 => stream_for!(i64),
        SampleFormat::U8 => stream_for!(u8),
        SampleFormat::U16 => stream_for!(u16),
        SampleFormat::U32 => stream_for!(u32),
        SampleFormat::U64 => stream_for!(u64),
        _ => return Err(format!("unsupported output sample format: {format}")),
    };
    result.map_err(|err| format!("failed to build output stream: {err}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn looping_callback_finishes_without_allocating_new_audio() {
        let mut block = AudioBlock::silent(1, 2);
        block.samples_mut().copy_from_slice(&[0.25, -0.5]);
        let mut state = LoopingAudio::new(&block, 2, 2).unwrap();
        let mut output = [0.0_f64; 10];

        assert!(state.fill(&mut output));
        assert_eq!(
            output,
            [0.25, 0.25, -0.5, -0.5, 0.25, 0.25, -0.5, -0.5, 0.0, 0.0]
        );
    }

    #[test]
    fn callback_clamps_only_at_device_boundary() {
        let mut block = AudioBlock::silent(2, 1);
        block.samples_mut().copy_from_slice(&[2.0, -2.0]);
        let mut state = LoopingAudio::new(&block, 2, 1).unwrap();
        let mut output = [0.0_f32; 2];

        assert!(state.fill(&mut output));
        assert_eq!(output, [1.0, -1.0]);
        assert_eq!(block.samples(), [2.0, -2.0]);
    }

    #[test]
    fn callback_applies_initial_play_command() {
        let block = AudioBlock::silent(1, 1);
        let mut state = LoopingAudio::new(&block, 1, 1).unwrap();
        let status = state.status_monitor();
        let mut output = [1.0_f64; 1];

        assert!(state.fill(&mut output));
        assert_eq!(output, [0.0]);
        assert_eq!(
            state.queue_diagnostics(),
            QueueDiagnostics {
                applied: 1,
                dropped: 0
            }
        );
        let snapshot = status.snapshot();
        assert_eq!(snapshot.state, crate::transport::TransportState::Stopped);
        assert_eq!(snapshot.position, 1);
        assert_eq!(snapshot.callbacks, 1);
    }

    #[test]
    fn missing_chunk_falls_back_to_silence_and_counts_one_underrun() {
        let audio =
            PublishedAudio::from_chunks(1, 4, 2, vec![Some(Arc::from([0.5, 0.5])), None]).unwrap();
        let mut state = LoopingAudio::from_published(audio, 1, 1).unwrap();
        let status = state.status_monitor();
        let mut output = [0.0_f64; 4];

        assert!(state.fill(&mut output));
        assert_eq!(output, [0.5, 0.5, 0.0, 0.0]);
        let snapshot = status.snapshot();
        assert_eq!(snapshot.underruns, 1);
        assert_eq!(snapshot.missed_artifacts, 1);
    }

    #[test]
    fn newly_published_chunk_is_acquired_at_the_next_callback() {
        let partial =
            PublishedAudio::from_chunks(1, 4, 2, vec![Some(Arc::from([0.5, 0.5])), None]).unwrap();
        let (publisher, reader) = audio_publication(partial);
        let mut state = LoopingAudio::from_reader(reader, 1, 2).unwrap();
        let status = state.status_monitor();
        let mut first_output = [0.0_f64; 4];

        assert!(!state.fill(&mut first_output));
        assert_eq!(first_output, [0.5, 0.5, 0.0, 0.0]);

        let ready = PublishedAudio::from_chunks(
            1,
            4,
            2,
            vec![Some(Arc::from([0.5, 0.5])), Some(Arc::from([0.75, 0.75]))],
        )
        .unwrap();
        publisher.publish(ready).unwrap();
        let mut second_output = [0.0_f64; 4];

        assert!(state.fill(&mut second_output));
        assert_eq!(second_output, [0.5, 0.5, 0.75, 0.75]);
        let snapshot = status.snapshot();
        assert_eq!(snapshot.callbacks, 2);
        assert_eq!(snapshot.underruns, 1);
        assert_eq!(snapshot.missed_artifacts, 1);
    }

    #[test]
    fn max_loop_playback_continues_across_publications() {
        let mut first = AudioBlock::silent(1, 2);
        first.samples_mut().copy_from_slice(&[0.25, 0.5]);
        let initial = PublishedAudio::from_block(&first, 1).unwrap();
        let (publisher, reader) = audio_publication(initial);
        let (control, engine) = playback_session_parts(4).unwrap();
        let status = control.status_monitor();
        let mut state = LoopingAudio::from_engine(reader, 1, u32::MAX, engine).unwrap();
        control.play().unwrap();

        let mut first_output = [0.0_f64; 5];
        assert!(!state.fill(&mut first_output));
        assert_eq!(first_output, [0.25, 0.5, 0.25, 0.5, 0.25]);

        let mut second = AudioBlock::silent(1, 2);
        second.samples_mut().copy_from_slice(&[0.75, -0.25]);
        publisher
            .publish(PublishedAudio::from_block(&second, 1).unwrap())
            .unwrap();

        let mut second_output = [0.0_f64; 5];
        assert!(!state.fill(&mut second_output));
        assert_eq!(second_output, [-0.25, 0.75, -0.25, 0.75, -0.25]);
        let snapshot = status.snapshot();
        assert_eq!(snapshot.state, crate::transport::TransportState::Playing);
        assert_eq!(snapshot.underruns, 0);
        assert_eq!(snapshot.missed_artifacts, 0);
    }

    #[test]
    fn split_control_drives_engine_without_treating_stop_as_completion() {
        let block = AudioBlock::silent(1, 2);
        let audio = PublishedAudio::from_block(&block, 1).unwrap();
        let (_, reader) = audio_publication(audio);
        let (control, engine) = playback_session_parts(4).unwrap();
        let status = control.status_monitor();
        let mut state = LoopingAudio::from_engine(reader, 1, 1, engine).unwrap();
        let mut output = [1.0_f64; 1];

        control.play().unwrap();
        assert!(!state.fill(&mut output));
        control.stop().unwrap();
        assert!(!state.fill(&mut output));
        assert_eq!(
            status.snapshot().state,
            crate::transport::TransportState::Stopped
        );
        control.play().unwrap();
        assert!(state.fill(&mut output));
        assert_eq!(control.command_diagnostics().applied, 3);
    }
}
