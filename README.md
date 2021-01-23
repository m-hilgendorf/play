# Audio Playback Demo

Usage: 

```bash
cargo run -- path/to/file.wav
```

This app: 

- starts the audio stream
- loads a .wav file into memory
- deinterleaves it into `f32` samples
- passes it to the audio thread 
- exits once the file has been played to completion

Synchronization is handled by passing an `Arc` to the audio thread using a ring buffer. The only mutable state on the audio thread is the playhead (`AudioFile::read_offset`) which is implemented using an atomic. 
