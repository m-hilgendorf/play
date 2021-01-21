use hound::{SampleFormat, WavReader};

pub struct AudioFile {
    pub data: Vec<f32>,
    pub sample_rate: f64,
    pub num_channels: usize,
}

impl AudioFile {
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
        Ok(Self {
            data,
            sample_rate: spec.sample_rate as f64,
            num_channels: spec.channels as usize,
        })
    }
}
