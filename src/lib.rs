#![no_std]

pub mod instrument;
pub mod graph;

/// The type of command to be sent to an instrument
#[repr(u8)]
#[derive(Debug, Copy, Clone)]
pub enum NoteCommandType {
    Noop = 0,
    NoteOn = 1,
    NoteOff = 2,
}

/// A command to be sent to an instrument
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct NoteCommand<N: Sized> {
    /// The type of command
    pub command_type: NoteCommandType,

    /// Full range of 0-255
    pub velocity: u8,

    /// The note to be played
    pub note: N,
}

pub type MidiNote = u8;

#[derive(Debug, Copy, Clone)]
pub struct VoidNote;

impl Default for VoidNote {
    fn default() -> Self {
        Self
    }
}

pub type MidiNoteCommand = NoteCommand<MidiNote>;

/// The type to be transmitted in value streams
pub type MusicalValue = f32;

pub const STANDARD_BLOCK_SIZE: usize = 128;
pub const STANDARD_ELEMENT_COUNT: usize = 128;

/// The input block for an instrument
/// The meanings of the streams are defined by the instrument
#[repr(C)]
#[derive(Debug, Clone)]
pub struct InstrumentInput<
    const VALUE_STREAMS: usize,
    const CONTROL_STREAMS: usize,
    Note: Sized = MidiNote,
    const VALUE_BLOCK: usize = STANDARD_BLOCK_SIZE,
    const CONTROL_ELEMENTS: usize = STANDARD_ELEMENT_COUNT,
> {
    /// Control streams are used for sending commands to the instrument
    /// The first dimension is the stream number, the second is the element index
    /// 
    /// Regardless of the element indexes, all control events are defined to happen at the same time at the start of the block
    pub control_streams: [[NoteCommand<Note>; CONTROL_ELEMENTS]; CONTROL_STREAMS],

    /// Value streams are used for sending musical values to the instrument
    /// The first dimension is the stream number, the second is the block number
    pub value_streams: [[MusicalValue; VALUE_BLOCK]; VALUE_STREAMS],
}

/// The output block for an instrument
/// The meanings of the streams are defined by the instrument, commonly used for audio channels
#[repr(C)]
#[derive(Debug, Clone)]
pub struct InstrumentOutput<const VALUE_STREAMS: usize, const VALUE_BLOCK: usize> {
    /// Value streams are used for receiving musical values from the instrument
    /// The first dimension is the stream number, the second is the block number
    pub value_streams: [[MusicalValue; VALUE_BLOCK]; VALUE_STREAMS],
}

/// The trait for an instrument
pub trait Instrument<
    const IN_VALUE_STREAMS: usize,
    const IN_CONTROL_STREAMS: usize,
    const OUT_VALUE_STREAMS: usize,
    Note: Sized = MidiNote,
> {
    /// Processes a block of input data and produces a block of output data
    fn process_block<const BLOCK_SIZE: usize, const CONTROL_ELEMENTS: usize>(
        &mut self,
        input: &InstrumentInput<IN_VALUE_STREAMS, IN_CONTROL_STREAMS, Note, BLOCK_SIZE, CONTROL_ELEMENTS>,
        output: &mut InstrumentOutput<OUT_VALUE_STREAMS, BLOCK_SIZE>,
    );
}

#[repr(C)]
#[derive(Debug, Clone)]
pub(crate) struct InstrumentContainerImpl<
    I: Instrument<IN_VALUE_STREAMS, IN_CONTROL_STREAMS, OUT_VALUE_STREAMS, Note>,
    const IN_VALUE_STREAMS: usize,
    const IN_CONTROL_STREAMS: usize,
    const OUT_VALUE_STREAMS: usize,
    Note: Sized,
> {
    pub instrument: I,
    pub input: InstrumentInput<IN_VALUE_STREAMS, IN_CONTROL_STREAMS, Note, STANDARD_BLOCK_SIZE, STANDARD_ELEMENT_COUNT>,
    pub output: InstrumentOutput<OUT_VALUE_STREAMS, STANDARD_BLOCK_SIZE>,
}

impl<
    I: Instrument<IN_VALUE_STREAMS, IN_CONTROL_STREAMS, OUT_VALUE_STREAMS, Note>,
    const IN_VALUE_STREAMS: usize,
    const IN_CONTROL_STREAMS: usize,
    const OUT_VALUE_STREAMS: usize,
    Note: Sized + Default + Copy,
> InstrumentContainerImpl<I, IN_VALUE_STREAMS, IN_CONTROL_STREAMS, OUT_VALUE_STREAMS, Note> {
    pub fn new(instrument: I) -> Self {
        Self {
            instrument,
            input: InstrumentInput {
                control_streams: [[NoteCommand {
                    command_type: NoteCommandType::Noop,
                    velocity: 0,
                    note: Note::default(),
                }; STANDARD_ELEMENT_COUNT]; IN_CONTROL_STREAMS],
                value_streams: [[0.0; STANDARD_BLOCK_SIZE]; IN_VALUE_STREAMS],
            },
            output: InstrumentOutput {
                value_streams: [[0.0; STANDARD_BLOCK_SIZE]; OUT_VALUE_STREAMS],
            },
        }
    }

    fn clear_input(&mut self) {
        for i in 0..IN_CONTROL_STREAMS {
            for j in 0..STANDARD_ELEMENT_COUNT {
                self.input.control_streams[i][j] = NoteCommand {
                    command_type: NoteCommandType::Noop,
                    velocity: 0,
                    note: Note::default(),
                };
            }
        }

        for i in 0..IN_VALUE_STREAMS {
            for j in 0..STANDARD_BLOCK_SIZE {
                self.input.value_streams[i][j] = 0.0;
            }
        }
    }

    fn process_block(&mut self) {
        self.instrument.process_block(&self.input, &mut self.output);
        self.clear_input();
    }
}

/// A container for an instrument, its input, and its output
pub trait InstrumentContainer<Note: Sized + Default + Copy = MidiNote> {
    fn in_value_streams(&self) -> usize;

    fn in_control_streams(&self) -> usize;

    fn out_value_streams(&self) -> usize;

    /// Processes the next block of data
    fn process_next(&mut self);

    /// Gets the output stream at the given index
    /// 
    /// Out of bounds stream indexes may panic.
    fn get_output(&self, index: usize) -> &[MusicalValue; STANDARD_BLOCK_SIZE];

    /// Feeds a control stream to the instrument. Last call to this function before `process_next` will be used.
    /// 
    /// Out of bounds stream indexes may panic.
    fn feed_control_stream(&mut self, stream_index: usize, stream: &[NoteCommand<Note>]);

    /// Feeds a value stream to the instrument. Multiple calls to this function are additive.
    /// 
    /// Out of bounds stream indexes may panic.
    fn feed_value_stream(&mut self, stream_index: usize, stream: &[MusicalValue]);
}

impl<
    I: Instrument<IN_VALUE_STREAMS, IN_CONTROL_STREAMS, OUT_VALUE_STREAMS, Note>,
    const IN_VALUE_STREAMS: usize,
    const IN_CONTROL_STREAMS: usize,
    const OUT_VALUE_STREAMS: usize,
    Note: Sized + Default + Copy,
> InstrumentContainer<Note> for InstrumentContainerImpl<I, IN_VALUE_STREAMS, IN_CONTROL_STREAMS, OUT_VALUE_STREAMS, Note> {
    fn in_value_streams(&self) -> usize {
        IN_VALUE_STREAMS
    }

    fn in_control_streams(&self) -> usize {
        IN_CONTROL_STREAMS
    }

    fn out_value_streams(&self) -> usize {
        OUT_VALUE_STREAMS
    }

    fn process_next(&mut self) {
        self.process_block();
    }

    fn get_output(&self, index: usize) -> &[MusicalValue; STANDARD_BLOCK_SIZE] {
        &self.output.value_streams[index]
    }
    
    fn feed_control_stream(&mut self, stream_index: usize, stream: &[NoteCommand<Note>]) {
        let len = stream.len().min(STANDARD_ELEMENT_COUNT);
        for i in 0..len {
            self.input.control_streams[stream_index][i] = stream[i];
        }
    }

    fn feed_value_stream(&mut self, stream_index: usize, stream: &[MusicalValue]) {
        let len = stream.len().min(STANDARD_BLOCK_SIZE);
        for i in 0..len {
            self.input.value_streams[stream_index][i] += stream[i];
        }
    }
}

pub fn container<
    I: Instrument<IN_VALUE_STREAMS, IN_CONTROL_STREAMS, OUT_VALUE_STREAMS, Note>,
    const IN_VALUE_STREAMS: usize,
    const IN_CONTROL_STREAMS: usize,
    const OUT_VALUE_STREAMS: usize,
    Note: Sized + Default + Copy,
>(
    instrument: I,
) -> impl InstrumentContainer<Note> {
    InstrumentContainerImpl::new(instrument)
}

pub trait ControlStreamSource<Note: Sized> {
    fn get_control_stream(&self) -> &[NoteCommand<Note>];
    fn fetch_next_stream(&mut self);
}

pub fn leak<'a, T>(mut value: T) -> &'a mut T {
    unsafe {
        let value = &mut value as *mut T;
        &mut *value
    }
}
