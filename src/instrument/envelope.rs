
use crate::{Instrument, InstrumentInput, InstrumentOutput, NoteCommandType, MidiNote};

/// A linear envelope with a fixed number of points.
/// 
/// The envelope is defined by a series of points, each with a time and a gain.
/// 
/// The envelope has a release time, which is the time it takes to go from the last point to zero.
/// 
/// The envelope has a current time, which is the time since the last note on event.
/// 
/// The envelope is triggered by a note on event, and goes through the points in order.
/// 
/// When the envelope reaches the last point, it stays at the last point until a note off event.
/// 
/// When the envelope receives a note off event, it goes to the release phase, which is a linear ramp from the last point to zero.
/// 
/// The envelope is a state machine with the following states:
/// - Off: The envelope is not active
/// - Playing: The envelope is playing the points
/// - Releasing: The envelope is releasing to zero
/// 
/// This instrument accepts one control stream:
/// - The first control stream is the note on/off event, with linear velocity
/// 
/// This instrument accepts no value streams.
/// 
/// The output stream is the value of the envelope at the given time.
pub struct LinearEnvelope<const POINTS: usize, Note: Sized = MidiNote> {
    /// The time in samples for each point.
    /// 
    /// The first point is the attack time
    pub point_times: [usize; POINTS],

    /// The gain for each point.
    pub point_gains: [f32; POINTS],

    /// Release time in samples
    pub release_time: usize,

    /// The current state of the envelope
    /// 
    /// 0 = off, 1-POINTS = playing, POINTS+1 = releasing
    pub current_point: usize,

    /// Current time inside the current point, in samples
    pub current_time: usize,

    /// Current note's base gain, derived from the note velocity
    pub current_note_gain: f32,

    /// Current gain of the envelope
    pub current_gain: f32,

    _phantom: core::marker::PhantomData<Note>,
}

impl<const POINTS: usize, Note: Sized> LinearEnvelope<POINTS, Note> {
    pub const fn new(
        point_times: [usize; POINTS],
        point_gains: [f32; POINTS],
        release_time: usize,
    ) -> Self {
        Self {
            point_times,
            point_gains,
            release_time,
            current_point: 0,
            current_time: 0,
            current_note_gain: 0.0,
            current_gain: 0.0,
            _phantom: core::marker::PhantomData,
        }
    }
}

impl<const POINTS: usize, Note: Sized> Instrument<0, 1, 1, Note> for LinearEnvelope<POINTS, Note> {
    fn process_block<const VALUE_BLOCK: usize, const CONTROL_ELEMENTS: usize>(
        &mut self,
        input: &InstrumentInput<0, 1, Note, VALUE_BLOCK, CONTROL_ELEMENTS>,
        output: &mut InstrumentOutput<1, VALUE_BLOCK>,
    ) {
        for i in 0..CONTROL_ELEMENTS {
            let note_command = &input.control_streams[0][i];
            match note_command.command_type {
                NoteCommandType::NoteOn => {
                    self.current_time = 0;
                    self.current_note_gain = note_command.velocity as f32 / 255.0;
                    self.current_gain = 0.0;

                    let mut i: usize = 1;
                    while POINTS > i && self.point_times[i - 1] == 0 {
                        i += 1;
                    }

                    self.current_point = i;
                    if i > 1 {
                        self.current_gain = self.point_gains[i - 2];
                    }
                },
                NoteCommandType::NoteOff => {
                    if self.current_point > 0 && self.current_point <= POINTS {
                        self.current_point = POINTS + 1;
                        self.current_time = 0;
                    }
                },

                _ => {},
            }
        }

        for i in 0..VALUE_BLOCK {
            if self.current_point == 0 {
                output.value_streams[0][i] = 0.0;
            } else if self.current_point <= POINTS {
                let prev_point_gain = if self.current_point > 1 {
                    self.point_gains[self.current_point - 2]
                } else {
                    0.0
                };

                let point_time = self.point_times[self.current_point - 1];

                let point_gain = self.point_gains[self.current_point - 1];
                let gain_diff = point_gain - prev_point_gain;

                let point_time_f32 = point_time as f32;
                let current_time_f32 = self.current_time.min(point_time) as f32;

                let gain = if point_time == 0 { point_gain } else { prev_point_gain + gain_diff * (current_time_f32 / point_time_f32) };
                output.value_streams[0][i] = gain * self.current_note_gain;
                self.current_gain = gain;

                self.current_time += 1;
                if self.current_time >= point_time {
                    if self.current_point < POINTS {
                        self.current_point += 1;
                        self.current_time = 0;
                    }

                    while self.current_point < POINTS && self.point_times[self.current_point] == 0 {
                        self.current_point += 1;
                    }
                }
            } else {
                if POINTS == 0 {
                    self.current_gain = 1.0;
                }
                let release_time_f32 = self.release_time as f32;
                let current_time_f32 = self.current_time as f32;
                let gain = if self.release_time == 0 { 0.0 } else { self.current_gain * (1.0 - current_time_f32 / release_time_f32) };
                output.value_streams[0][i] = gain * self.current_note_gain;
                self.current_time += 1;
                if self.current_time >= self.release_time {
                    self.current_point = 0;
                    self.current_time = 0;
                }
            }
        }
    }
}
