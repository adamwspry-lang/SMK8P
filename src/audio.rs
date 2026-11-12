use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

const MAX_QUEUED_SAMPLES: usize = 48_000 / 2;

struct Resampler {
    from_rate: f64,
    to_rate: f64,
    
    phase: f64,
    
    last_frame: (i16, i16),
}

impl Resampler {
    fn new(from_rate: f64, to_rate: f64) -> Self {
        Self { from_rate, to_rate, phase: 0.0, last_frame: (0, 0) }
    }

    fn process(&mut self, input: &[i16]) -> Vec<i16> {
        if self.from_rate == self.to_rate {
            return input.to_vec();
        }

        let in_frames: Vec<(i16, i16)> = input
            .chunks_exact(2)
            .map(|c| (c[0], c[1]))
            .collect();
        if in_frames.is_empty() {
            return Vec::new();
        }

        let ratio = self.from_rate / self.to_rate;
        let mut out = Vec::new();

        loop {
            let idx = self.phase.floor() as isize;
            let frac = self.phase - self.phase.floor();

            let (l0, r0) = if idx < 0 {
                self.last_frame
            } else if (idx as usize) < in_frames.len() {
                in_frames[idx as usize]
            } else {
                break;
            };
            let (l1, r1) = if idx + 1 < 0 {
                self.last_frame
            } else if ((idx + 1) as usize) < in_frames.len() {
                in_frames[(idx + 1) as usize]
            } else {
                break;
            };

            let l = lerp_i16(l0, l1, frac);
            let r = lerp_i16(r0, r1, frac);
            out.push(l);
            out.push(r);

            self.phase += ratio;
        }

        self.phase -= in_frames.len() as f64;
        self.last_frame = *in_frames.last().unwrap();

        out
    }
}

fn lerp_i16(a: i16, b: i16, t: f64) -> i16 {
    let a = a as f64;
    let b = b as f64;
    (a + (b - a) * t).round().clamp(i16::MIN as f64, i16::MAX as f64) as i16
}

pub struct AudioOutput {
    queue: Arc<Mutex<VecDeque<i16>>>,
    resampler: Mutex<Resampler>,
    
    volume_bits: Arc<AtomicU32>,
    
    _stream: cpal::Stream,
}

impl AudioOutput {
    
    pub fn new(source_rate_hz: f64) -> Result<Self, String> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or_else(|| "no default audio output device found".to_string())?;

        let supported = device
            .default_output_config()
            .map_err(|e| format!("couldn't query default output config: {e}"))?;

        let device_sample_rate = supported.sample_rate().0 as f64;
        let device_channels = supported.channels();
        if device_channels < 2 {
            return Err(format!(
                "default output device only supports {device_channels} channel(s); stereo required"
            ));
        }

        let config = cpal::StreamConfig {
            channels: 2,
            sample_rate: cpal::SampleRate(device_sample_rate as u32),
            buffer_size: cpal::BufferSize::Default,
        };

        let queue: Arc<Mutex<VecDeque<i16>>> = Arc::new(Mutex::new(VecDeque::new()));
        let queue_for_callback = Arc::clone(&queue);

        let volume_bits: Arc<AtomicU32> = Arc::new(AtomicU32::new(1.0f32.to_bits()));
        let volume_for_callback = Arc::clone(&volume_bits);

        let sample_format = supported.sample_format();
        let error_fn = |err: cpal::StreamError| eprintln!("audio stream error: {err}");

        let stream = match sample_format {
            cpal::SampleFormat::I16 => device
                .build_output_stream(
                    &config,
                    move |output: &mut [i16], _info: &cpal::OutputCallbackInfo| {
                        let volume = f32::from_bits(volume_for_callback.load(Ordering::Relaxed));
                        let mut q = queue_for_callback.lock().unwrap();
                        for sample_slot in output.iter_mut() {
                            let raw = q.pop_front().unwrap_or(0);
                            *sample_slot = apply_gain_i16(raw, volume);
                        }
                    },
                    error_fn,
                    None,
                )
                .map_err(|e| format!("failed to build i16 audio output stream: {e}"))?,
            cpal::SampleFormat::F32 => device
                .build_output_stream(
                    &config,
                    move |output: &mut [f32], _info: &cpal::OutputCallbackInfo| {
                        let volume = f32::from_bits(volume_for_callback.load(Ordering::Relaxed));
                        let mut q = queue_for_callback.lock().unwrap();
                        for sample_slot in output.iter_mut() {
                            let sample_i16 = q.pop_front().unwrap_or(0);
                            *sample_slot = (sample_i16 as f32 / i16::MAX as f32) * volume;
                        }
                    },
                    error_fn,
                    None,
                )
                .map_err(|e| format!("failed to build f32 audio output stream: {e}"))?,
            other => {
                
                return Err(format!("unsupported device sample format: {other:?}"));
            }
        };

        stream
            .play()
            .map_err(|e| format!("failed to start audio output stream: {e}"))?;

        eprintln!(
            "audio: emulator rate {source_rate_hz:.1}Hz -> device rate {device_sample_rate:.1}Hz (resampling)"
        );

        Ok(Self {
            queue,
            resampler: Mutex::new(Resampler::new(source_rate_hz, device_sample_rate)),
            volume_bits,
            _stream: stream,
        })
    }

    pub fn push_samples(&self, samples: &[i16]) {
        let resampled = self.resampler.lock().unwrap().process(samples);

        let mut q = self.queue.lock().unwrap();
        q.extend(resampled);

        while q.len() > MAX_QUEUED_SAMPLES {
            q.pop_front();
        }
    }

    pub fn set_volume(&self, volume: f32) {
        let clamped = volume.clamp(0.0, 1.0);
        self.volume_bits.store(clamped.to_bits(), Ordering::Relaxed);
    }
}

fn apply_gain_i16(sample: i16, gain: f32) -> i16 {
    ((sample as f32) * gain).round().clamp(i16::MIN as f32, i16::MAX as f32) as i16
}