use linstr::*;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

fn main() {
    let host = cpal::default_host();
    let device = host.default_output_device().unwrap();

    let mut supported_configs_range = device.supported_output_configs().unwrap();
    let supported_config = supported_configs_range.next().unwrap().with_max_sample_rate();

    let config = supported_config.config();

    let sampling_rate = config.sample_rate.0 as usize;

    let mut container = container(instrument::oscillators::SineOscillator::<MidiNote>::new(sampling_rate));
    let mut remains: Vec<f32> = Vec::with_capacity(128);
    let stream = device.build_output_stream(
        &config,
        move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {

            let mut j = 0;
            for i in 0..data.len() {
                if j < remains.len() {
                    data[i] = remains[j];
                } else {
                    container.feed_value_stream(0, &[440.0; 128]);
                    container.feed_value_stream(1, &[0.0; 128]);
        
                    container.process_next();
                    remains.clear();
                    remains.extend_from_slice(container.get_output(0));
                    j = 0;
                    data[i] = remains[j];
                }
                j += 1;
            }

            remains.drain(0..j);
        },
        move |err| {
            eprintln!("an error occurred on stream: {}", err);
        },
        None,
    ).unwrap();

    stream.play().unwrap();

    loop {
        std::thread::park();
    }
}
