use std::sync::atomic::{AtomicBool, Ordering};

use linstr::*;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use rand::Rng;

struct MyControl<'a> {
    signal: &'a AtomicBool,
    stream: [NoteCommand<MidiNote>; 1],
}

impl<'a> MyControl<'a> {
    fn new(signal: &'a AtomicBool) -> Self {
        Self {
            signal,
            stream: [NoteCommand {
                command_type: NoteCommandType::Noop,
                velocity: 0,
                note: 0,
            }],
        }
    }
}

impl ControlStreamSource<MidiNote> for MyControl<'_> {
    fn get_control_stream(&self) -> &[NoteCommand<MidiNote>] {
        &self.stream
    }

    fn fetch_next_stream(&mut self) {
        self.stream[0] = if self.signal.load(Ordering::SeqCst) {
            println!("NoteOn");

            NoteCommand {
                command_type: NoteCommandType::NoteOn,
                velocity: 255,
                note: 69,
            }
        } else {
            NoteCommand {
                command_type: NoteCommandType::NoteOff,
                velocity: 0,
                note: 69,
            }
        };

        self.signal.store(false, Ordering::SeqCst);
    }
}

static SIGNAL: AtomicBool = AtomicBool::new(false);

fn main() {
    let host = cpal::default_host();
    let device = host.default_output_device().unwrap();

    let mut supported_configs_range = device.supported_output_configs().unwrap();
    let supported_config = supported_configs_range.next().unwrap().with_max_sample_rate();

    let config = supported_config.config();

    let sampling_rate = config.sample_rate.0 as usize;

    let signal_ref = &SIGNAL;

    let mut rng = rand::rng();

    let f: u16 = rng.random_range(440..1760);

    let f = f as f32;
    let f2 = f * 1.3;

    let mut graph = graph::InstrumentGraph::<16>::new();

    graph.add_instrument(leak(container(instrument::oscillators::SineOscillator::<MidiNote>::new(sampling_rate))));
    graph.add_instrument(leak(container(instrument::envelope::LinearEnvelope::<1, MidiNote>::new([0], [1.0], sampling_rate / 2))));
    graph.add_instrument(leak(container(instrument::Amplifier::<MidiNote>::new())));
    graph.add_instrument(leak(container(instrument::Constant::<MidiNote>::new(f))));
    graph.add_instrument(leak(container(instrument::Constant::<MidiNote>::new(0.0))));
    graph.add_instrument(leak(container(instrument::Constant::<MidiNote>::new(f2))));
    graph.add_instrument(leak(container(instrument::oscillators::SineOscillator::<MidiNote>::new(sampling_rate))));
    graph.add_instrument(leak(container(instrument::Amplifier::<MidiNote>::new())));
    graph.add_instrument(leak(container(instrument::envelope::LinearEnvelope::<1, MidiNote>::new([0], [0.125], sampling_rate / 2))));

    graph.add_control_source(leak(MyControl::new(signal_ref)));

    graph.connect_control_source(0, 1);
    graph.connect_control_source(0, 8);
    graph.connect_destination(0, 2, 0);
    graph.connect_value_stream(0, 0, 2, 0);
    graph.connect_value_stream(1, 0, 2, 1);
    graph.connect_value_stream(3, 0, 0, 0);
    // graph.connect_value_stream(4, 0, 0, 1);
    graph.connect_value_stream(5, 0, 6, 0);
    graph.connect_value_stream(4, 0, 6, 1);
    graph.connect_value_stream(7, 0, 0, 1);
    graph.connect_value_stream(6, 0, 7, 0);
    graph.connect_value_stream(8, 0, 7, 1);

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
                    }
                    j = 0;
                    data[i] = remains[j];
                }
                if i % config.channels as usize == config.channels as usize - 1 {
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

    let stdin_handle = std::io::stdin();
    let mut line = String::new();
    loop {
        stdin_handle.read_line(&mut line).unwrap();
        SIGNAL.store(true, Ordering::SeqCst);
        line.clear();
    }
}

fn leak<'a, T: 'a>(value: T) -> &'a mut T {
    Box::leak(Box::new(value))
}
