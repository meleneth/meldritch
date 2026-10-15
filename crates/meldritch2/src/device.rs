use std::sync::{
    Arc, OnceLock,
    atomic::{AtomicU64, Ordering},
};

use cpal::{
    FromSample, Sample, SampleFormat,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};

use crate::{
    compiler::{ControlHandle, compile_at_rate},
    live::{LiveControl, LiveRuntime},
    model::Graph,
};

const CHUNK_FRAMES: usize = 256;

/// Copy mono graph output to every device channel without allocating.
pub fn fill_interleaved<T: Sample + FromSample<f64>>(
    runtime: &mut LiveRuntime,
    output: &mut [T],
    channels: usize,
    scratch: &mut [f64; CHUNK_FRAMES],
) {
    if channels == 0 {
        return;
    }
    for chunk in output.chunks_mut(CHUNK_FRAMES * channels) {
        let frames = chunk.len().div_ceil(channels);
        runtime.process(&mut scratch[..frames]);
        for (frame, value) in chunk.chunks_mut(channels).zip(&scratch[..frames]) {
            let converted = T::from_sample(value.clamp(-1.0, 1.0));
            frame.fill(converted);
        }
    }
}

pub struct DeviceOutput {
    stream: cpal::Stream,
    control: LiveControl,
    controls: Vec<(ControlHandle, f64)>,
    errors: Arc<AtomicU64>,
    first_error: Arc<OnceLock<String>>,
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
        let (runtime, control) = LiveRuntime::new(machine);
        let errors = Arc::new(AtomicU64::new(0));
        let first_error = Arc::new(OnceLock::new());
        let channels = usize::from(config.channels);
        macro_rules! build {
            ($sample:ty) => {{
                let mut runtime = runtime;
                let mut scratch = [0.0; CHUNK_FRAMES];
                let errors = Arc::clone(&errors);
                let first_error = Arc::clone(&first_error);
                device.build_output_stream(
                    config,
                    move |output: &mut [$sample], _| {
                        fill_interleaved(&mut runtime, output, channels, &mut scratch);
                    },
                    move |error| {
                        if first_error.get().is_none() {
                            let _ = first_error.set(error.to_string());
                        }
                        errors.fetch_add(1, Ordering::Relaxed);
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
            errors,
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
    pub fn stream_errors(&self) -> u64 {
        self.errors.load(Ordering::Relaxed)
    }

    pub fn first_stream_error(&self) -> Option<&str> {
        self.first_error.get().map(String::as_str)
    }
}

#[cfg(test)]
mod tests {
    use super::{CHUNK_FRAMES, fill_interleaved};
    use crate::{compiler::compile, live::LiveRuntime, lua};

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
}
