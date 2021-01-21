use cpal::traits::{DeviceTrait, HostTrait};
use cpal::Stream;

pub struct PlaybackContext<'a, 'b> {
    pub buffer_size:usize,
    pub sample_rate:f64,
    pub num_channels:usize,
    output_buffer:&'a mut [f32], 
    input_buffer:&'b [f32],
}

pub fn audio_stream(
    f: impl FnMut(&mut [f32], &cpal::OutputCallbackInfo) + Send + 'static,
) -> Stream {
    let host = cpal::default_host();
    let output_device = host.default_output_device().expect("no output found");
    let config = output_device
        .default_output_config()
        .expect("no default output config")
        .config();

    let sample_rate = config.sample_rate.0 as f64;
    let num_channels = config.channels as usize;
    let mut output_buffer = vec![]; 
    output_buffer.resize_with(1 << 16, 0.0);
    let callback = move |data, _| {
        let buffer_size = data.len() / num_channels;
        let input_buffer = data;
        let output_buffer = &mut output_buffer;
        let context = PlaybackContext {
            buffer_size, 
            num_channels, 
            sample_rate, 
            output_buffer, 
            input_buffer
        };
        
    };
    output_device
        .build_output_stream(&config, f, |err| eprintln!("{}", err))
        .expect("failed to open stream")
}
