//! Standard instruments.
//! 

pub mod oscillators;
pub mod envelope;

use crate::{Instrument, InstrumentInput, InstrumentOutput, MidiNote};

/// The amplifier instrument.
/// 
/// This instrument accepts two value streams:
/// - The first stream is the input signal
/// - The second stream is the gain of the amplifier
/// 
/// The output stream is the input signal multiplied by the gain.
pub struct Amplifier<Note: Sized = MidiNote> {
    _phantom: core::marker::PhantomData<Note>,
}

impl<Note: Sized> Amplifier<Note> {
    pub const fn new() -> Self {
        Self {
            _phantom: core::marker::PhantomData,
        }
    }
}

impl<Note: Sized> Instrument<2, 0, 1, Note> for Amplifier {
    fn process_block<const VALUE_BLOCK: usize, const CONTROL_ELEMENTS: usize>(
        &mut self,
        input: &InstrumentInput<2, 0, Note, VALUE_BLOCK, CONTROL_ELEMENTS>,
        output: &mut InstrumentOutput<1, VALUE_BLOCK>,
    ) {

        for i in 0..VALUE_BLOCK {
            output.value_streams[0][i] = input.value_streams[0][i] * input.value_streams[1][i];
        }
    }
}

/// The mixer instrument.
/// 
/// This instrument accepts `INPUT_STREAMS` value streams.
/// 
/// The output stream is the sum of the input streams.
pub struct Mixer<const INPUT_STREAMS: usize = 2usize, Note: Sized = MidiNote> {
    _phantom: core::marker::PhantomData<Note>,
}

impl<const INPUT_STREAMS: usize, Note: Sized> Mixer<INPUT_STREAMS, Note> {
    pub const fn new() -> Self {
        Self {
            _phantom: core::marker::PhantomData,
        }
    }
}

impl<const INPUT_STREAMS: usize, Note: Sized> Instrument<INPUT_STREAMS, 0, 1, Note> for Mixer<INPUT_STREAMS> {
    fn process_block<const VALUE_BLOCK: usize, const CONTROL_ELEMENTS: usize>(
        &mut self,
        input: &InstrumentInput<INPUT_STREAMS, 0, Note, VALUE_BLOCK, CONTROL_ELEMENTS>,
        output: &mut InstrumentOutput<1, VALUE_BLOCK>,
    ) {

        for i in 0..VALUE_BLOCK {
            output.value_streams[0][i] = 0.0;
            for j in 0..INPUT_STREAMS {
                output.value_streams[0][i] += input.value_streams[j][i];
            }
        }
    }
}

pub struct Delay<Note: Sized = MidiNote> {
    /// The delay time in samples
    pub delay: u16,

    pub buffer: [f32; 65536],
    pub buffer_index: usize,

    _phantom: core::marker::PhantomData<Note>,
}

impl<Note: Sized> Delay<Note> {
    pub const fn new(delay: u16) -> Self {
        Self {
            delay,
            buffer: [0.0; 65536],
            buffer_index: 0,
            _phantom: core::marker::PhantomData,
        }
    }
}

impl<Note: Sized> Instrument<1, 0, 1, Note> for Delay<Note> {
    fn process_block<const VALUE_BLOCK: usize, const CONTROL_ELEMENTS: usize>(
        &mut self,
        input: &InstrumentInput<1, 0, Note, VALUE_BLOCK, CONTROL_ELEMENTS>,
        output: &mut InstrumentOutput<1, VALUE_BLOCK>,
    ) {

        for i in 0..VALUE_BLOCK {
            self.buffer[self.buffer_index] = input.value_streams[0][i];
            self.buffer_index += 1;
            if self.buffer_index > self.delay as usize {
                self.buffer_index = 0;
            }
            output.value_streams[0][i] = self.buffer[self.buffer_index];
        }
    }
}

pub struct Constant<Note: Sized = MidiNote> {
    pub value: f32,

    _phantom: core::marker::PhantomData<Note>,
}

impl<Note: Sized> Constant<Note> {
    pub const fn new(value: f32) -> Self {
        Self {
            value,
            _phantom: core::marker::PhantomData,
        }
    }
}

impl<Note: Sized> Instrument<0, 0, 1, Note> for Constant<Note> {
    fn process_block<const VALUE_BLOCK: usize, const CONTROL_ELEMENTS: usize>(
        &mut self,
        _input: &InstrumentInput<0, 0, Note, VALUE_BLOCK, CONTROL_ELEMENTS>,
        output: &mut InstrumentOutput<1, VALUE_BLOCK>,
    ) {

        for i in 0..VALUE_BLOCK {
            output.value_streams[0][i] = self.value;
        }
    }
}
