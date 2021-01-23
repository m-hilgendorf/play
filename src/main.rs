mod audio_file;
mod audio_stream;
mod utils;
use audio_file::AudioFile;
use audio_stream::audio_stream;
use ringbuf::RingBuffer;
use std::sync::Arc;
use utils::Flag;

struct AudioThreadState {
    content: AudioFile,
}

fn main() {
    // get program input... 
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 2 {
        println!("usage is: `play <path>`");
        std::process::exit(1);
    }
    let path = &args[1];

    // set up synchronization...
    let rb: RingBuffer<Arc<AudioThreadState>> = RingBuffer::new(2);
    let (mut tx, mut rx) = rb.split();
    let stop = Flag::new();
    let audio_stop = stop.clone();

    // initialize state and begin the stream...
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

    // load hte audio file..
    let state = Arc::new(AudioThreadState {
        content: AudioFile::open(path).expect("failed to read file"),
    });
    
    // if this fails, keep trying
    while let Err(_) = tx.push(state.clone()) {}

    // spin until we're done.
    while !stop.is_set() {
        std::thread::sleep(std::time::Duration::from_millis(16));
    }
}
