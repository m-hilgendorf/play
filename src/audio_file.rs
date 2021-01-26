use crate::utils::deinterleave;
use druid::kurbo::BezPath;
use druid::Color;
use hound::{SampleFormat, WavReader};
use rustfft::{num_complex::Complex, FftPlanner};

pub struct PeakView {
    pub segments: Vec<(BezPath, Color)>,
}

pub struct Peaks {
    pub peaks: Vec<(usize, PeakView)>,
}

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
    fn peak(&self, channel: usize, block_size: usize) -> PeakView {
        let mut planner = FftPlanner::<f32>::new();
        let mut fft_buf = vec![Complex::new(0.0f32, 0.0f32); block_size];
        let fft = planner.plan_fft_forward(block_size);
        let time = &self.get_channel(channel)[0..self.num_samples];
        let mut result = vec![];
        let mut x0 = 0.0;
        let mut min0 = 0.5;
        let mut max0 = 0.5;

        for (chunk, n) in time
            .chunks(block_size)
            .zip((0..time.len()).step_by(block_size))
        {
            let x1 = (n as f64) / (time.len() as f64);
            for (x, fx) in chunk.iter().zip(fft_buf.iter_mut()) {
                fx.re = *x;
                fx.im = 0.0;
            }
            fft.process(&mut fft_buf);
            let (min, max, energy) =
                chunk
                    .iter()
                    .fold((0.0f32, 0.0f32, 0.0f32), |(min, max, energy), sample| {
                        (min.min(*sample), max.max(*sample), energy + sample.abs())
                    });

            let centroid = (&fft_buf[0..block_size / 2])
                .iter()
                .zip(0..fft_buf.len())
                .fold(0.0f32, |c, (x, k)| {
                    c + (x.re.powi(2) + x.im.powi(2)).sqrt() * (2.0 * k as f32)
                        / (block_size as f32)
                })
                / energy;

            let centroid = centroid as f64;
            let min = min as f64;
            let max = max as f64;
            let mut path = BezPath::new();

            path.move_to((x0, max0));
            path.line_to((x0, min0));
            path.line_to((x1, min));
            path.line_to((x1, max));
            path.line_to((x0, max0));

            x0 = x1;
            max0 = max;
            min0 = min;

            let color = Color::hlc(180.0 + 10.0 * centroid, 50.0 + 40.0 * centroid, 127.0);
            result.push((path, color))
        }
        PeakView { segments: result }
    }

    pub fn spectral_peaks(&self, channel: usize) -> Peaks {
        Peaks {
            peaks: (&[128, 256, 512, 1024, 2048, 4096])
                .iter()
                .map(|n| (*n, self.peak(channel, *n)))
                .collect(),
        }
    }

    /// return a buffer of samples corresponding to a channel in the audio file
    #[allow(dead_code)]
    pub fn get_channel(&self, idx: usize) -> &'_ [f32] {
        debug_assert!(idx < self.num_channels);
        let start = self.num_samples * idx;
        &self.data[start..(start + self.num_samples)]
    }

    /// plots the "stair step" pattern when the stepsize is 1.
    pub fn plot(
        &self,
        step_size: usize,
        channel: usize,
        start: usize,
        end: usize,
    ) -> (BezPath, BezPath) {
        debug_assert!(step_size > 0, "step size must be at least 1");
        debug_assert!(
            end <= self.num_samples,
            "end must be less than the number of samples available"
        );

        let channel = &self.get_channel(channel)[start..end];
        let mut peak = BezPath::new();
        let mut peak0 = BezPath::new();
        let mut peak1 = BezPath::new();
        let mut avg0 = BezPath::new();
        let mut avg1 = BezPath::new();

        let mut avg = BezPath::new();

        let len = channel.len() as f64;
        if step_size == 1 {
            let mut x0 = 0.0;
            let mut y0 = 0.0;
            for (x1, y1) in (0..self.num_samples)
                .map(|n| (n as f64) / len)
                .zip(channel.iter().map(|f| *f as f64))
            {
                peak.move_to((x0, y0));
                peak.line_to((x0, y1));
                peak.line_to((x0, y1));
                peak.line_to((x1, y1));
                x0 = x1;
                y0 = y1;
            }
        } else {
            peak0.move_to((0.0, 0.5));
            peak1.move_to((0.0, 0.5));
            avg0.move_to((0.0, 0.5));
            avg1.move_to((0.0, 0.5));

            for n in (0..channel.len()).step_by(step_size) {
                let (n0, n1) = (n, (n + step_size).min(self.num_samples));
                let x = (n as f64) / len;
                let (y_min, y_peak, y_rms) = (channel[n0..n1])
                    .iter()
                    .fold((0.0f32, 0.0f32, 0.0f32), |(m, p, r), y| {
                        (m.min(*y), p.max(*y), r + y.abs())
                    });
                let y_peak = y_peak as f64;
                let y_rms = y_rms as f64;
                let y_min = y_min as f64;

                peak0.line_to((x, 0.5 - y_peak));
                peak1.line_to((x, 0.5 - y_min));
                avg0.line_to((x, 0.5 + y_rms / (step_size as f64)));
                avg1.line_to((x, 0.5 - y_rms / (step_size as f64)));
            }

            peak0.line_to((1.0, 0.5));
            peak1.line_to((1.0, 0.5));

            avg0.line_to((1.0, 0.5));
            avg1.line_to((1.0, 0.5));

            peak.extend(peak0);
            peak.extend(peak1);
            peak.close_path();
            avg.extend(avg0);
            avg.extend(avg1);
            avg.close_path();
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
