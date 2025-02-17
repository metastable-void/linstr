use std::sync::atomic::{AtomicIsize, Ordering};

use linstr::*;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use rand::Rng;

struct MyControl<'a> {
    signal: &'a AtomicIsize,
    stream: [NoteCommand<MidiNote>; 1],
    last_note: u8,
}

impl<'a> MyControl<'a> {
    fn new(signal: &'a AtomicIsize) -> Self {
        Self {
            signal,
            stream: [NoteCommand {
                command_type: NoteCommandType::Noop,
                velocity: 0,
                note: 0,
            }],
            last_note: 128,
        }
    }
}

impl ControlStreamSource<MidiNote> for MyControl<'_> {
    fn get_control_stream(&self) -> &[NoteCommand<MidiNote>] {
        &self.stream
    }

    fn fetch_next_stream(&mut self) {
        let value = self.signal.load(Ordering::SeqCst);
        self.stream[0] = if value >= 0 {
            println!("NoteOn({})", value);

            self.last_note = value as u8;

            NoteCommand {
                command_type: NoteCommandType::NoteOn,
                velocity: 255,
                note: self.last_note,
            }
        } else if self.last_note == 128 {
            NoteCommand {
                command_type: NoteCommandType::Noop,
                velocity: 0,
                note: 0,
            }
        } else {
            let note = self.last_note;
            self.last_note = 128;

            NoteCommand {
                command_type: NoteCommandType::NoteOff,
                velocity: 0,
                note,
            }
        };

        self.signal.store(-1, Ordering::SeqCst);
    }
}

struct BellInstrumentUnit {
    note: usize,
    envelope: Box<dyn InstrumentContainer<MidiNote>>,
    oscillator: Box<dyn InstrumentContainer<MidiNote>>,
    amplifier: Box<dyn InstrumentContainer<MidiNote>>,
}

impl BellInstrumentUnit {
    fn new(sampling_rate: usize, note: usize) -> Self {
        Self {
            note,
            envelope: Box::new(container(instrument::envelope::LinearEnvelope::<1, MidiNote>::new([0], [0.25], sampling_rate / 2))),
            oscillator: Box::new(container(instrument::oscillators::SineOscillator::<MidiNote>::new(sampling_rate))),
            amplifier: Box::new(container(instrument::Amplifier::<MidiNote>::new())),
        }
    }
}

impl Instrument<0, 1, 1, MidiNote> for BellInstrumentUnit {
    fn process_block<const VALUE_BLOCK: usize, const CONTROL_ELEMENTS: usize>(
        &mut self,
        input: &InstrumentInput<0, 1, MidiNote, VALUE_BLOCK, CONTROL_ELEMENTS>,
        output: &mut InstrumentOutput<1, VALUE_BLOCK>,
    ) {
        let freq = 440.0 * 2.0f32.powf((self.note as f32 - 69.0) / 12.0);
        let mut control_stream = input.control_streams[0].clone();
        for i in 0..CONTROL_ELEMENTS {
            if control_stream[i].note != self.note as u8 {
                control_stream[i].command_type = NoteCommandType::Noop;
            } 
        }
        self.envelope.as_mut().feed_control_stream(0, &control_stream[0..1]);
        self.envelope.as_mut().process_next();
        self.oscillator.as_mut().feed_value_stream(0, &[freq; VALUE_BLOCK]);
        self.oscillator.as_mut().feed_value_stream(1, &[0.0; VALUE_BLOCK]);
        self.oscillator.as_mut().process_next();
        self.amplifier.as_mut().feed_value_stream(0, self.oscillator.as_ref().get_output(0));
        self.amplifier.as_mut().feed_value_stream(1, self.envelope.as_ref().get_output(0));
        self.amplifier.as_mut().process_next();
        output.value_streams[0].copy_from_slice(self.amplifier.as_ref().get_output(0));
    }
}

struct BellInstrument {
    units: [Box<dyn InstrumentContainer<MidiNote>>; 128],
}

impl BellInstrument {
    fn new(sampling_rate: usize) -> Self {
        let mut units: Vec<Box<dyn InstrumentContainer<MidiNote>>> = Vec::with_capacity(128);
        for i in 0..128 {
            units.push(Box::new(container(BellInstrumentUnit::new(sampling_rate, i))));
        }
        Self { units: units.try_into().map_err(|_| ()).expect("Failed") }
    }
}

impl Instrument<0, 1, 1, MidiNote> for BellInstrument {
    fn process_block<const VALUE_BLOCK: usize, const CONTROL_ELEMENTS: usize>(
        &mut self,
        input: &InstrumentInput<0, 1, MidiNote, VALUE_BLOCK, CONTROL_ELEMENTS>,
        output: &mut InstrumentOutput<1, VALUE_BLOCK>,
    ) {
        for unit in self.units.iter_mut() {
            unit.as_mut().feed_control_stream(0, &input.control_streams[0]);
            unit.as_mut().process_next();
            let output_stream = unit.as_ref().get_output(0);
            for i in 0..VALUE_BLOCK {
                output.value_streams[0][i] += output_stream[i];
            }
        }
    }
}

static SIGNAL: AtomicIsize = AtomicIsize::new(-1);

fn main() {
    let host = cpal::default_host();
    let device = host.default_output_device().unwrap();

    let mut supported_configs_range = device.supported_output_configs().unwrap();
    let supported_config = supported_configs_range.next().unwrap().with_max_sample_rate();

    let config = supported_config.config();

    let sampling_rate = config.sample_rate.0 as usize;

    let signal_ref = &SIGNAL;

    let mut rng = rand::rng();

    let mut graph = graph::InstrumentGraph::<1>::new();

    graph.add_instrument(leak(container(BellInstrument::new(sampling_rate))));

    println!("initiated");
    graph.add_control_source(leak(MyControl::new(signal_ref)));

    graph.connect_control_source(0, 0);
    graph.connect_destination(0, 0, 0);

    println!("Order: {:?}", graph.get_instrument_process_order());

    let mut remains: Vec<f32> = Vec::with_capacity(128);
    let stream = device.build_output_stream(
        &config,
        move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {

            let mut j = 0;
            for i in 0..data.len() {
                if j < remains.len() {
                    data[i] = remains[j];
                } else {
                    if i % config.channels as usize == 0 {
                        graph.process_next();
                        remains.clear();
                        remains.extend_from_slice(graph.get_output(0));
                        j = 0;
                    }
                    data[i] = remains[j];
                }
                if (i % config.channels as usize) == (config.channels as usize - 1) {
                    j += 1;
                }
            }

            remains.drain(0..j);
        },
        move |err| {
            eprintln!("an error occurred on stream: {}", err);
        },
        None,
    ).unwrap();

    stream.play().unwrap();

    let prob = 0.05;

    loop {
        std::thread::sleep(std::time::Duration::from_millis(100));
        let rand: f64 = rng.random_range(0.0..1.0);
        if rand < prob {
            let note: i32 = rng.random_range(0..128);
            SIGNAL.store(note as isize, Ordering::SeqCst);
        }
    }
}

fn leak<'a, T: 'a>(value: T) -> &'a mut T {
    Box::leak(Box::new(value))
}
