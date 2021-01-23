use crate::utils::deinterleave;
use hound::{SampleFormat, WavReader};
use std::sync::atomic::{AtomicUsize, Ordering};

/// An audio file, loaded into memory
pub struct AudioFile {
    /// The sample data
    pub data: Vec<f32>,
    /// Sample rate of the audio file
    pub sample_rate: f64,
    /// number of channels in the audio file
    pub num_channels: usize,
    /// number of sample sin the audio file
    pub num_samples: usize,
    /// the current read offset (used during playback)
    pub read_offset: AtomicUsize,
}

impl AudioFile {
    /// returns true if the read_offset has passed the number of samples.
    pub fn finished(&self) -> bool {
        self.read_offset.load(Ordering::SeqCst) >= self.num_samples
    }

    /// advance the read_offset by a count.
    pub fn advance(&self, count: usize) {
        self.read_offset.fetch_add(count, Ordering::SeqCst);
    }

    /// return a buffer of samples corresponding to a channel in the audio file
    pub fn get_channel(&self, idx: usize, size: usize) -> &'_ [f32] {
        let sample_start = self.read_offset.load(Ordering::SeqCst);
        let sample_end = (sample_start + size).min(self.num_samples);
        let buffer_start = self.num_samples * idx + sample_start;
        let buffer_end = self.num_samples * idx + sample_end;
        &self.data[buffer_start..buffer_end]
    }

    /// open a file
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
