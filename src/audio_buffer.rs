use arrayvec::ArrayString;
use thiserror::Error as ThisError;
const MAX_DELAY_SIZE: usize = 4096;

/// There are a variety of errors that can occur at runtime due to developer
/// neglect or invalid configuration by a user. Most errors are recoverable,
/// but they are made explicit here to prevent panics and crashes.
#[derive(ThisError, Debug, PartialEq, Eq)]
pub enum Error {
    #[error("Invalid channel index {0}.")]
    InvalidChannel(usize),
    #[error("Missing or insufficient storage to create buffer")]
    StorageRequired,
}

/// A channel configuration is a pair of values indicating how an audio buffer
/// or port should be interpreted (typed).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct ChannelConfiguration {
    count: usize,
    description: ArrayString<[u8; 32]>,
}

/// The engine uses a variety of different audio buffering types internally. Any function
/// that may require one should be generic over the trait.
pub trait AudioBuffer {
    /// Get a channel slice that can be read from.
    fn get_channel(&self, channel_index: usize) -> Result<&'_ [f32], Error>;
    /// Get a channel slice that can be written to.
    fn get_channel_mut(&mut self, channel_index: usize) -> Result<&'_ mut [f32], Error>;
    /// Get the channel configuration of the buffer.
    fn get_channel_config(&self) -> ChannelConfiguration;
    /// Some buffers need to process internally between being used as an input or output buffer.
    /// This method is used to prepare for the next cycle.
    ///
    /// For example, [DelBuffer] will write its internal scratch memory to the delay line and
    /// copy the delayed signal back in. [SumBuffer] will sum the scratch buffer with its internal
    /// accumulator, then zero the contents of the scratch buffer.
    fn prepare(&mut self);
    /// clear the buffer (set to zeros)
    fn clear(&mut self);
    /// get the number of samples in the buffer
    fn num_samples(&self) -> usize;
}

/// A SimpleBuffer is a block of memory and associated channel configuration.
pub struct SimpleBuffer {
    channel_config: ChannelConfiguration,
    memory: Vec<f32>,
}

/// A RefBuffer is an audio buffer that doesn't own its own memory.
pub struct RefBuffer<'a> {
    channel_config: ChannelConfiguration,
    memory: &'a mut [f32],
}

/// A SumBuffer is used to implement mixing.
pub struct SumBuffer {
    channel_config: ChannelConfiguration,
    max_buffer_size: usize,
    memory: Vec<f32>,
}

/// A DelBuffer has its write buffers delayed from its read buffers.
pub struct DelBuffer {
    channel_config: ChannelConfiguration,
    memory: Vec<f32>,
    max_buffer_size: usize,
    record_head: usize,
    delay: usize,
}

impl ChannelConfiguration {
    /// Returns the number of channels required by this configuration.
    pub fn count(&self) -> usize {
        self.count
    }
    /// A short description of how the channels should be interpreted. Max 32 characters.
    pub fn description(&self) -> &'_ str {
        &self.description
    }
    /// Check if an index is in bounds.
    pub fn check_channel(&self, idx: usize) -> Result<(), Error> {
        if idx >= self.count {
            Err(Error::InvalidChannel(idx))
        } else {
            Ok(())
        }
    }
}

impl SimpleBuffer {
    /// Create a new SimpleBuffer. Will allocate memory.
    pub fn new(channel_config: ChannelConfiguration, max_buffer_size: usize) -> Self {
        Self {
            channel_config,
            memory: vec![0.0; max_buffer_size * channel_config.count()],
        }
    }
}

impl AudioBuffer for SimpleBuffer {
    fn get_channel_config(&self) -> ChannelConfiguration {
        self.channel_config
    }
    fn get_channel(&self, channel_index: usize) -> Result<&'_ [f32], Error> {
        self.channel_config.check_channel(channel_index)?;
        let start = channel_index * self.num_samples();
        let end = (start + self.num_samples()).min(self.memory.len());
        Ok(&self.memory[start..end])
    }
    fn get_channel_mut(&mut self, channel_index: usize) -> Result<&'_ mut [f32], Error> {
        self.channel_config.check_channel(channel_index)?;
        let start = channel_index * self.num_samples();
        let end = (start + self.num_samples()).min(self.memory.len());
        Ok(&mut self.memory[start..end])
    }
    fn num_samples(&self) -> usize {
        self.memory.len() / self.channel_config.count()
    }
    fn clear(&mut self) {
        for sample in &mut self.memory {
            *sample = 0.0;
        }
    }
    fn prepare(&mut self) {}
}

impl<'a> RefBuffer<'a> {
    /// Construct a new reference buffer. Returns an error if the slice is not large enough for the channel
    /// configuration and max buffer size.
    pub fn new(
        channel_config: ChannelConfiguration,
        max_buffer_size: usize,
        memory: &'a mut [f32],
    ) -> Result<Self, Error> {
        if memory.len() < max_buffer_size * channel_config.count() {
            Err(Error::StorageRequired)
        } else {
            Ok(Self {
                channel_config,
                memory,
            })
        }
    }
}

impl<'a> AudioBuffer for RefBuffer<'a> {
    fn get_channel_config(&self) -> ChannelConfiguration {
        self.channel_config
    }
    fn get_channel(&self, channel_index: usize) -> Result<&'_ [f32], Error> {
        self.channel_config.check_channel(channel_index)?;
        let start = channel_index * self.num_samples();
        let end = (start + self.num_samples()).min(self.memory.len());
        Ok(&self.memory[start..end])
    }
    fn get_channel_mut(&mut self, channel_index: usize) -> Result<&'_ mut [f32], Error> {
        self.channel_config.check_channel(channel_index)?;
        let start = channel_index * self.num_samples();
        let end = (start + self.num_samples()).min(self.memory.len());
        Ok(&mut self.memory[start..end])
    }
    fn num_samples(&self) -> usize {
        self.memory.len() / self.channel_config.count()
    }
    fn clear(&mut self) {
        for sample in self.memory.iter_mut() {
            *sample = 0.0;
        }
    }
    fn prepare(&mut self) {}
}

impl DelBuffer {
    fn new(channel_config: ChannelConfiguration, max_buffer_size: usize, delay: usize) -> Self {
        Self {
            channel_config,
            record_head: 0,
            max_buffer_size,
            delay,
            memory: vec![0.0; (max_buffer_size + MAX_DELAY_SIZE) * channel_config.count()],
        }
    }
}

impl AudioBuffer for DelBuffer {
    fn get_channel(&self, channel_index: usize) -> Result<&'_ [f32], Error> {
        self.channel_config.check_channel(channel_index)?;
        let num_samples = self.max_buffer_size;
        Ok(&self.memory[channel_index * num_samples..(channel_index + 1) * num_samples])
    }
    fn get_channel_mut(&mut self, channel_index: usize) -> Result<&'_ mut [f32], Error> {
        self.channel_config.check_channel(channel_index)?;
        let num_samples = self.max_buffer_size;
        Ok(&mut self.memory[channel_index * num_samples..(channel_index + 1) * num_samples])
    }
    fn get_channel_config(&self) -> ChannelConfiguration {
        self.channel_config
    }
    fn prepare(&mut self) {
        let (num_samples, num_channels, head, delay_samples) = (
            self.num_samples(),
            self.channel_config.count(),
            self.record_head,
            self.delay,
        );
        let (scratch, delay) = self.memory.split_at_mut(num_samples * num_channels);
        for ch in 0..self.channel_config.count() {
            let (scratch, delay) = (
                &mut scratch[ch * num_samples..(ch + 1) * num_samples],
                &mut delay[ch * MAX_DELAY_SIZE..(ch + 1) * MAX_DELAY_SIZE],
            );
            let write = (0..num_samples).map(|n| (n + head + delay_samples) % MAX_DELAY_SIZE);
            let read = (0..num_samples).map(|n| (n + head) % MAX_DELAY_SIZE);
            for (sample, (read, write)) in scratch.iter_mut().zip(read.zip(write)) {
                let tmp = *sample;
                *sample = delay[read];
                delay[write] = tmp;
            }
        }
        self.record_head = (self.record_head + num_samples) % MAX_DELAY_SIZE;
    }
    fn clear(&mut self) {
        let num_samples = self.num_samples();
        for sample in &mut self.memory[0..num_samples] {
            *sample = 0.0;
        }
    }
    fn num_samples(&self) -> usize {
        self.max_buffer_size
    }
}

impl SumBuffer {
    pub fn new(channel_config: ChannelConfiguration, max_buffer_size: usize) -> Self {
        Self {
            channel_config,
            max_buffer_size,
            memory: vec![0.0; 2 * max_buffer_size * channel_config.count()],
        }
    }
}

impl AudioBuffer for SumBuffer {
    fn get_channel(&self, channel_index: usize) -> Result<&'_ [f32], Error> {
        self.channel_config.check_channel(channel_index)?;
        let num_samples = self.num_samples();
        let offset = num_samples * self.channel_config.count();
        Ok(&self.memory
            [offset + channel_index * num_samples..(offset + (channel_index + 1) * num_samples)])
    }
    fn get_channel_mut(&mut self, channel_index: usize) -> Result<&'_ mut [f32], Error> {
        self.channel_config.check_channel(channel_index)?;
        let num_samples = self.max_buffer_size;
        Ok(&mut self.memory[channel_index * num_samples..(channel_index + 1) * num_samples])
    }
    fn get_channel_config(&self) -> ChannelConfiguration {
        self.channel_config
    }
    fn prepare(&mut self) {
        let num_channels = self.channel_config.count();
        let num_samples = self.num_samples();
        let (scratch, acc) = self
            .memory
            .split_at_mut(self.max_buffer_size * num_channels);
        for ch in 0..num_channels {
            let (scratch, acc) = (
                &mut scratch[ch * num_samples..(ch + 1) * num_samples],
                &mut acc[ch * num_samples..(ch + 1) * num_samples],
            );
            for (scratch, acc) in scratch.iter_mut().zip(acc.iter_mut()) {
                *acc += *scratch;
                *scratch = 0.0;
            }
        }
    }
    fn clear(&mut self) {
        let num_samples = self.num_samples();
        for sample in &mut self.memory[0..num_samples] {
            *sample = 0.0;
        }
    }
    fn num_samples(&self) -> usize {
        self.max_buffer_size
    }
}

/// some predefined strings for [ChannelConfig] descriptions.
pub mod channel_description {
    use super::ChannelConfiguration;
    use arrayvec::ArrayString;

    pub const MONO: &'static str = "mono";
    pub const LEFT: &'static str = "left";
    pub const RIGHT: &'static str = "right";
    pub const MID: &'static str = "mid";
    pub const SIDE: &'static str = "side";
    pub const STEREO: &'static str = "stereo";
    pub const MID_SIDE: &'static str = "mid-side";
    pub const MULTI_MONO: &'static str = "multi-mono";

    /// Create a multimono channel configuration. Multimono
    /// refers to a set of discrete channels that should be processed
    /// together, but are independent of each other. Contrast to
    /// a mid/side or stereo channel set, whose signals may
    /// require being processed and mixed together internally.
    pub fn multi_mono(channels: usize) -> ChannelConfiguration {
        ChannelConfiguration {
            count: channels,
            description: ArrayString::from(MULTI_MONO).unwrap(),
        }
    }
    /// A single channel of audio.
    pub fn mono() -> ChannelConfiguration {
        ChannelConfiguration {
            count: 1,
            description: ArrayString::from(MONO).unwrap(),
        }
    }
    /// A pair of channels, left/right.
    pub fn stereo() -> ChannelConfiguration {
        ChannelConfiguration {
            count: 2,
            description: ArrayString::from(STEREO).unwrap(),
        }
    }
    /// A pair of channels, one representing the middle of the signal (mid)
    /// and the other representing its sides (side).
    pub fn mid_side() -> ChannelConfiguration {
        ChannelConfiguration {
            count: 2,
            description: ArrayString::from(MID_SIDE).unwrap(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use channel_description::{mono, multi_mono, stereo};

    fn basic_tests<B: AudioBuffer>(mut buffer: B) {
        assert_eq!(buffer.get_channel_config(), stereo());
        buffer.prepare();
        {
            let left = buffer.get_channel_mut(0).expect("could not get channel");
            assert_eq!(left.len(), 16);
            for sample in left {
                *sample = 0.5;
            }
            let right = buffer.get_channel_mut(1).expect("could not get channel");
            assert_eq!(right.len(), 16);
            assert_eq!(Err(Error::InvalidChannel(2)), buffer.get_channel_mut(2));
        }
        {
            let left = buffer.get_channel(0).expect("could not get channel");
            let right = buffer.get_channel(1).expect("could not get channel");
            for sample in left {
                assert_eq!(*sample, 0.5);
            }
            for sample in right {
                assert_eq!(*sample, 0.0);
            }
            assert_eq!(left.len(), 16);
            assert_eq!(right.len(), 16);
            assert_eq!(Err(Error::InvalidChannel(2)), buffer.get_channel(2));
        }
        buffer.clear();
        for sample in buffer.get_channel(0).unwrap() {
            assert_eq!(*sample, 0.0);
        }
    }

    #[test]
    fn simple_buffer() {
        let buffer = SimpleBuffer::new(stereo(), 16);
        basic_tests(buffer);
    }

    #[test]
    fn ref_buffer() {
        let mut mem = [0.0; 32];
        assert_eq!(
            Error::StorageRequired,
            RefBuffer::new(multi_mono(3), 16, &mut mem).err().unwrap()
        );
        let buffer = RefBuffer::new(stereo(), 16, &mut mem).expect("could not create buffer");
        basic_tests(buffer);
    }

    #[test]
    fn del_buffer() {
        let buffer = DelBuffer::new(stereo(), 16, 0);
        basic_tests(buffer);
        let d = 3;
        let mut buffer = DelBuffer::new(mono(), 16, d);
        {
            let channel = buffer.get_channel_mut(0).unwrap();
            for (n, x) in (0..).zip(channel.iter_mut()) {
                *x = n as f32;
            }
        }
        buffer.prepare();
        {
            let channel = buffer.get_channel(0).unwrap();
            for (n, x) in (0..).zip(channel.iter()) {
                if n >= d {
                    assert_eq!(*x, (n - d) as f32);
                }
            }
        }
    }

    #[test]
    fn sum_buffer() {
        //let buffer = SumBuffer::new(stereo(), 16);
        //basic_tests(buffer);
        let mut buffer = SumBuffer::new(mono(), 16);
        {
            let channel = buffer.get_channel_mut(0).unwrap();
            for sample in channel.iter_mut() {
                *sample = 1.0;
            }
        }
        buffer.prepare();
        {
            let channel = buffer.get_channel_mut(0).unwrap();
            for sample in channel.iter_mut() {
                *sample = 2.0;
            }
        }
        buffer.prepare();
        {
            let channel = buffer.get_channel(0).unwrap();
            for sample in channel.iter() {
                assert_eq!(*sample, 3.0);
            }
        }
    }
}
