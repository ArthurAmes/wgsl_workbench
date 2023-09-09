use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

pub fn start_audio_capture() {
    let host = cpal::default_host();

    let device = host.default_input_device().unwrap();

    let mut supported_configs_range = device
        .supported_output_configs()
        .expect("error while querying configs");
    let supported_config = supported_configs_range
        .next()
        .expect("no supported config?!");
    let sample_format = supported_config.sample_format();
    let config: cpal::StreamConfig = supported_config
        .with_sample_rate(cpal::SampleRate(48000))
        .into();

    let stream = match sample_format {
        cpal::SampleFormat::I8 => device
            .build_input_stream(
                &config,
                |data: &[i8], _: &cpal::InputCallbackInfo| {
                    println!("{:?}", data);
                    println!("{:?}", data.len());
                },
                |err| {
                    println!("{:?}", err);
                },
                None,
            )
            .expect("failed to create input stream"),
        _ => todo!()
    }.play().unwrap();
}
