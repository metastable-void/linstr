
use crate::{Instrument, InstrumentInput, InstrumentOutput, MidiNote};

/// Sine oscillator.
/// 
/// This instrument accepts two value streams:
/// - The first stream is the frequency of the oscillator
/// - The second stream is the phase of the oscillator
/// 
/// The output stream is the value of the oscillator at the given phase.
/// The phase is in the range [0, 1), representing a full cycle of the oscillator.
pub struct SineOscillator<Note: Sized = MidiNote> {
    /// The sampling rate of the instrument
    pub sampling_rate: usize,

    /// The current phase of the oscillator
    pub phase: f32,

    _phantom: core::marker::PhantomData<Note>,
}

impl<Note: Sized> SineOscillator<Note> {
    pub const fn new(sampling_rate: usize) -> Self {
        Self {
            sampling_rate,
            phase: 0.0,
            _phantom: core::marker::PhantomData,
        }
    }
}

impl<Note: Sized> Instrument<2, 0, 1, Note> for SineOscillator<Note> {
    fn process_block<const VALUE_BLOCK: usize, const CONTROL_ELEMENTS: usize>(
        &mut self,
        input: &InstrumentInput<2, 0, Note, VALUE_BLOCK, CONTROL_ELEMENTS>,
        output: &mut InstrumentOutput<1, VALUE_BLOCK>,
    ) {

        for i in 0..VALUE_BLOCK {
            let frequency = input.value_streams[0][i];
            let phase_increment = frequency / self.sampling_rate as f32;
            self.phase += phase_increment + input.value_streams[1][i];
            while self.phase >= 1.0 {
                self.phase -= 1.0;
            }
            while self.phase < 0.0 {
                self.phase += 1.0;
            }

            output.value_streams[0][i] = libm::sinf(2.0 * core::f32::consts::PI * self.phase);
        }
    }
}
