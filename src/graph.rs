
use crate::{ControlStreamSource, InstrumentContainer, MidiNote, MusicalValue, STANDARD_BLOCK_SIZE};

#[derive(Debug, Clone)]
pub(crate) struct ControlStreamConnection {
    pub source_index: usize,
}

#[derive(Debug, Clone)]
pub(crate) struct ValueStreamConnection {
    pub source_index: usize,
    pub source_stream_index: usize,
    pub destination_stream_index: usize,
}

pub(crate) struct DestinationConnection {
    pub(crate) source_index: usize,
    pub(crate) source_stream_index: usize,
}

pub struct InstrumentGraph<'a, const SIZE: usize, const CONTROL_SIZE: usize = 16usize, const CONNECTION_SIZE: usize = 16usize, const OUTPUT_CHANNELS: usize = 1usize, Note: Sized + Default + Copy = MidiNote> {
    /// The instruments in the graph
    pub instruments: [Option<&'a mut dyn InstrumentContainer<Note>>; SIZE],

    /// The control sources in the graph
    pub control_sources: [Option<&'a mut dyn ControlStreamSource<Note>>; CONTROL_SIZE],

    /// The connections between control sources and instruments, for each instrument
    pub(crate) instruments_control_sources: [Option<ControlStreamConnection>; SIZE],

    /// The connections between value streams, for each instrument
    pub(crate) value_stream_connections: [[Option<ValueStreamConnection>; CONNECTION_SIZE]; SIZE],

    pub(crate) destination_connections: [[Option<DestinationConnection>; CONNECTION_SIZE]; OUTPUT_CHANNELS],

    pub(crate) output_channels: [[MusicalValue; STANDARD_BLOCK_SIZE]; OUTPUT_CHANNELS],
}

unsafe impl<'a, const SIZE: usize, const CONTROL_SIZE: usize, const CONNECTION_SIZE: usize, const OUTPUT_CHANNELS: usize, Note: Sized + Default + Copy> Send for InstrumentGraph<'a, SIZE, CONTROL_SIZE, CONNECTION_SIZE, OUTPUT_CHANNELS, Note> {}

impl<'a, const SIZE: usize, const CONTROL_SIZE: usize, const CONNECTION_SIZE: usize, const OUTPUT_CHANNELS: usize, Note: Sized + Default + Copy> InstrumentGraph<'a, SIZE, CONTROL_SIZE, CONNECTION_SIZE, OUTPUT_CHANNELS, Note> {
    pub fn new() -> Self {
        let mut instance: Self = unsafe {
            core::mem::zeroed()
        };

        for i in 0..SIZE {
            instance.instruments[i] = None;
            instance.instruments_control_sources[i] = None;
            for j in 0..CONNECTION_SIZE {
                instance.value_stream_connections[i][j] = None;
            }
        }

        for i in 0..CONTROL_SIZE {
            instance.control_sources[i] = None;
        }

        for i in 0..OUTPUT_CHANNELS {
            for j in 0..CONNECTION_SIZE {
                instance.destination_connections[i][j] = None;
            }
        }

        instance
    }

    pub fn add_instrument(&mut self, instrument: &'a mut dyn InstrumentContainer<Note>) -> usize {
        for i in 0..SIZE {
            if self.instruments[i].is_none() {
                self.instruments[i] = Some(instrument);
                return i;
            }
        }
        panic!("No more space for instruments");
    }

    pub fn add_control_source(&mut self, control_source: &'a mut dyn ControlStreamSource<Note>) -> usize {
        for i in 0..CONTROL_SIZE {
            if self.control_sources[i].is_none() {
                self.control_sources[i] = Some(control_source);
                return i;
            }
        }
        panic!("No more space for control sources");
    }

    pub fn connect_control_source(&mut self, control_source_index: usize, instrument_index: usize) {
        if instrument_index >= SIZE {
            panic!("Instrument index out of bounds");
        }

        self.instruments_control_sources[instrument_index] = Some(ControlStreamConnection {
            source_index: control_source_index,
        });
    }

    pub fn connect_value_stream(&mut self, source_index: usize, source_stream_index: usize, destination_index: usize, destination_stream_index: usize) {
        if source_index >= SIZE {
            panic!("Source index out of bounds");
        }

        if destination_index >= SIZE {
            panic!("Destination index out of bounds");
        }

        for i in 0..CONNECTION_SIZE {
            if self.value_stream_connections[destination_index][i].is_none() {
                self.value_stream_connections[destination_index][i] = Some(ValueStreamConnection {
                    source_index,
                    source_stream_index,
                    destination_stream_index,
                });
                return;
            }
        }
        panic!("No more space for value stream connections");
    }

    pub fn connect_destination(&mut self, output_channel_index: usize, source_index: usize, source_stream_index: usize) {
        for i in 0..CONNECTION_SIZE {
            if self.destination_connections[output_channel_index][i].is_none() {
                self.destination_connections[output_channel_index][i] = Some(DestinationConnection {
                    source_index,
                    source_stream_index,
                });
                return;
            }
        }
        panic!("No more space for destination connections");
    }

    fn clear_output(&mut self) {
        for i in 0..OUTPUT_CHANNELS {
            for j in 0..STANDARD_BLOCK_SIZE {
                self.output_channels[i][j] = 0.0;
            }
        }
    }

    /// Resolving dependencies, returns the order in which instruments should be processed, in instrument indexes
    pub fn get_instrument_process_order(&self) -> [usize; SIZE] {
        let mut order = [usize::MAX; SIZE];
        let mut order_index = 0;
        let mut processed = [false; SIZE];

        loop {
            for i in 0..SIZE {
                if processed[i] {
                    continue;
                }

                let mut all_processed = true;
                for j in 0..self.value_stream_connections[i].len() {
                    if let Some(value_stream_connection) = &self.value_stream_connections[i][j] {
                        if !processed[value_stream_connection.source_index] {
                            all_processed = false;
                        }
                    }
                }

                if all_processed {
                    processed[i] = true;
                    order[order_index] = i;
                    order_index += 1;

                    if order_index == SIZE {
                        return order;
                    }
                }
            }
        }
    }

    pub fn process_next(&mut self) {
        self.clear_output();

        let order = self.get_instrument_process_order();

        for i in 0..CONTROL_SIZE {
            if let Some(control_source) = &mut self.control_sources[i] {
                control_source.fetch_next_stream();
            }
        }
        for i in 0..SIZE {
            let instrument_index = order[i];
            if instrument_index == usize::MAX {
                break;
            }

            for j in 0..CONNECTION_SIZE {
                if let Some(value_stream_connection) = &self.value_stream_connections[instrument_index][j] {
                    let source_index = value_stream_connection.source_index;
                    let source_stream_index = value_stream_connection.source_stream_index;
                    let destination_stream_index = value_stream_connection.destination_stream_index;

                    let source_stream = if let Some(source_instrument) = &self.instruments[source_index] {
                        Some(source_instrument.get_output(source_stream_index).clone())
                    } else {
                        None
                    };
                    if let Some(instrument) = &mut self.instruments[instrument_index] {
                        instrument.feed_value_stream(destination_stream_index, &source_stream.unwrap());
                    }
                }
            }

            if let Some(instrument) = &mut self.instruments[instrument_index] {
                for j in 0..instrument.in_control_streams() {
                    if let Some(connection) = &self.instruments_control_sources[instrument_index] {
                        if let Some(control_source) = &self.control_sources[connection.source_index] {
                            instrument.feed_control_stream(j, control_source.get_control_stream());
                        }
                    }
                }
                instrument.process_next();
            }
        }

        for i in 0..OUTPUT_CHANNELS {
            for j in 0..CONNECTION_SIZE {
                if let Some(destination_connection) = &self.destination_connections[i][j] {
                    let source_index = destination_connection.source_index;
                    let source_stream_index = destination_connection.source_stream_index;

                    if let Some(source_instrument) = &self.instruments[source_index] {
                        let source_stream = source_instrument.get_output(source_stream_index);
                        for k in 0..STANDARD_BLOCK_SIZE {
                            self.output_channels[i][k] += source_stream[k];
                        }
                    }
                }
            }
        }
    }

    pub fn get_output(&self, index: usize) -> &[MusicalValue; STANDARD_BLOCK_SIZE] {
        &self.output_channels[index]
    }
}