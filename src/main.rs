mod audio_file;
mod audio_stream;
mod utils;
use audio_file::AudioFile;
use audio_stream::audio_stream;
use ringbuf::RingBuffer;
use utils::Flag;
use std::sync::Arc;

struct AudioThreadState {
    content: AudioFile,
}

fn main() {
    let rb: RingBuffer<Arc<AudioThreadState>> = RingBuffer::new(2);
    let (mut tx, mut rx) = rb.split();
    let stop = Flag::new();
    let audio_stop = stop.clone();

    let mut state = None;
    let _stream = audio_stream(move |mut context| {
        if state.is_none() {
            state = rx.pop();
        }

        if let Some(state) = &state {
            if !state.content.finished() {
                for channel in 0..context.num_channels {
                    let content = state.content.get_channel(channel, context.buffer_size);
                    context.get_output(channel).copy_from_slice(content);
                }
                state.content.advance(context.buffer_size);
                if state.content.finished() {
                    audio_stop.set();
                }
            }
        }
    });

    let content = AudioFile::open("test-data/loop.wav").expect("failed to read file");
    let state = Arc::new(AudioThreadState { content });
    println!("pushing content");
    while let Err(_) = tx.push(state.clone()) {
        println!("failed to push content");
    }
    
    while !stop.is_set() {
        std::thread::sleep(std::time::Duration::from_millis(16));
    }
}
