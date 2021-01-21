mod audio_file;
mod audio_stream;
mod utils;
use audio_file::AudioFile;
use audio_stream::audio_stream;
use utils::Flag;
use ringbuf::RingBuffer;

struct AudioThreadState {
    content: Vec<f32>,
    count: usize,
}

fn deinterleave<T>(buf:&[T], nch:usize) -> Vec<T> {
    let nsm = buf.len() / nch; 
    let mut v = Vec::with_capacity(buf.len());
    for sm in 0..nsm {
        for ch in 0..nch {
            v.push(buf[sm + ch * nch]);
        }
    }
    v
}

fn main() {
    // initialize the garbage collector 
    let rb: RingBuffer<AudioThreadState> = RingBuffer::new(2);
    let (mut tx, mut rx) = rb.split();
    let stop = Flag::new();
    let audio_stop = stop.clone();

    let mut state = None;
    let _stream = audio_stream(move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
        for sample in data.iter_mut() {
            *sample = 0.0;
        }

        if state.is_none() {
            state = rx.pop();
        }

        if let Some(state) = state.as_mut() {
            for sample in data {
                if state.count < state.content.len() {
                    *sample = state.content[state.count];
                    state.count += 1;
                } else {
                    audio_stop.set();
                }
            }
        }
    });

    let content = AudioFile::open("test-data/loop.wav").expect("failed to read file");
    let mut state = AudioThreadState { content: content.data, count: 0 };
    while let Err(e) = tx.push(state) {
        state = e;
    }
    while !stop.is_set() {
        std::thread::sleep(std::time::Duration::from_millis(16));
    }
}
