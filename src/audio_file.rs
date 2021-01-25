use crate::utils::deinterleave;
use hound::{SampleFormat, WavReader};
use druid::kurbo::BezPath;

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
}

impl AudioFile {
    /// return a buffer of samples corresponding to a channel in the audio file
    #[allow(dead_code)]
    pub fn get_channel(&self, idx: usize) -> &'_ [f32] {
        debug_assert!(idx < self.num_channels);
        let start = self.num_samples * idx;
        &self.data[start..(start + self.num_samples)]
    }

        // for n in (0..file.num_samples).step_by(step) {
        //     let (n0, n1) = (
        //         n, (n + step).min(file.num_samples)
        //     );
        //     let (peak, avg) = (&file.data[n0..n1]).iter()
        //         .fold((0.0f32, 0.0f32), |(peak, avg), y| 
        //             (peak.max(y.abs()), avg + y.abs()));
            
        //     let (peak, avg) = (peak as f64, (avg as f64) / (step as f64));
        //     let x = (n as f64) / len;
        //     peak_path.move_to((x, 1.0));
        //     peak_path.line_to((x, peak + 1.0));
        //     peak_path.line_to((x, -peak + 1.0));
        //     avg_path.move_to((x, 1.0));
        //     avg_path.line_to((x, avg + 1.0));
        //     avg_path.line_to((x, -avg + 1.0));
        // }
    /// returns the peak and average of the waveform as bezier paths for plotting.
    /// 
    /// plots the "stair step" pattern when the stepsize is 1. 
    pub fn plot (&self, step_size:usize, channel:usize, start:usize, end:usize) -> (BezPath, BezPath) {
        debug_assert!(step_size > 0, "step size must be at least 1");
        debug_assert!(end < self.num_samples, "end must be less than the number of samples available");

        let channel = &self.get_channel(channel)[start..end];
        let mut peak = BezPath::new();
        let mut avg = BezPath::new(); 
        let len = channel.len() as f64;
        if step_size == 1 {
            let mut x0 = 0.0;
            let mut y0 = 0.0;    
            for (x1, y1) in (0..self.num_samples).map(|n| (n as f64) / len).zip(channel.iter().map(|f| *f as f64)) {
                peak.move_to((x0, y0));
                peak.line_to((x0, y1));
                peak.line_to((x0, y1));
                peak.line_to((x1, y1));
                x0 = x1;
                y0 = y1;
            }
        } else {
            for n in (0..channel.len()).step_by(step_size) {
                let (n0, n1) = (n, (n + step_size).min(self.num_samples));
                let x = (n as f64) / len;
                let (y_peak, y_rms) = (channel[n0..n1]).iter()
                    .fold((0.0f32, 0.0f32), |(p, r), y| {
                        (p.max(y.abs()), r + y.abs())
                    });
                let y_peak = y_peak as f64;
                let y_rms = y_rms as f64;
                peak.move_to((x, 0.5 + y_peak));
                peak.line_to((x, 0.5 - y_peak));
                avg.move_to((x, 0.5 + y_rms / (step_size as f64)));
                avg.line_to((x, 0.5 - y_rms / (step_size as f64)));
            }
        }

        (peak, avg)
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
            num_channels,
            num_samples,
        })
    }
}
