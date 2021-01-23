use crate::utils::deinterleave;
use std::sync::atomic::{AtomicUsize, Ordering};
use hound::{SampleFormat, WavReader};

pub struct AudioFile {
    pub data: Vec<f32>,
    pub sample_rate: f64,
    pub num_channels: usize,
    pub num_samples: usize,
    pub read_offset: AtomicUsize,
}

impl AudioFile {
    pub fn finished(&self) -> bool {
        self.read_offset.load(Ordering::SeqCst) >= self.num_samples
    }

    pub fn advance(&self, count: usize) {
        self.read_offset.fetch_add(count, Ordering::SeqCst);
    }

    pub fn get_channel(&self, idx: usize, size: usize) -> &'_ [f32] {
        let sample_start = self.read_offset.load(Ordering::SeqCst);
        let sample_end = (sample_start + size).min(self.num_samples);
        let buffer_start = self.num_samples * idx + sample_start;
        let buffer_end = self.num_samples * idx + sample_end;
        &self.data[buffer_start..buffer_end]
    }

    pub fn open(path: &str) -> Result<Self, hound::Error> {
        let mut reader = WavReader::open(path)?;
        let spec = reader.spec();
        let mut data = Vec::with_capacity((spec.channels as usize) * (reader.duration() as usize));
        match (spec.bits_per_sample, spec.sample_format) {
            (16, SampleFormat::Int) => {
                for sample in reader.samples::<i16>() {
                    data.push((sample? as f32) / (0x7fffi32 as f32));
                }
            }
            (24, SampleFormat::Int) => {
                for sample in reader.samples::<i32>() {
                    let val = (sample? as f32) / (0x00ff_ffffi32 as f32);
                    data.push(val);
                }
            }
            (32, SampleFormat::Int) => {
                for sample in reader.samples::<i32>() {
                    data.push((sample? as f32) / (0x7fff_ffffi32 as f32));
                }
            }
            (32, SampleFormat::Float) => {
                for sample in reader.samples::<f32>() {
                    data.push(sample?);
                }
            }
            _ => return Err(hound::Error::Unsupported),
        }

        let mut deinterleaved = vec![0.0; data.len()];
        let num_channels = spec.channels as usize;
        let num_samples = deinterleaved.len() / num_channels;
        deinterleave(&data, &mut deinterleaved, num_channels);
        Ok(Self {
            data: deinterleaved,
            sample_rate: spec.sample_rate as f64,
            num_channels: num_channels,
            num_samples: num_samples,
            read_offset: AtomicUsize::new(0),
        })
    }
}
