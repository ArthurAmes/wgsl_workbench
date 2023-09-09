use audio::start_audio_capture;
use nokhwa::{
    pixel_format::RgbAFormat,
    utils::{CameraIndex, RequestedFormat},
};
use parking_lot::RwLock;
use std::sync::Arc;

use appstate::App;
use hotwatch::{EventKind, Hotwatch};
use winit::{
    event::{ElementState, Event, KeyboardInput, VirtualKeyCode, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

use crate::appstate::RenderPipelineContext;

mod appstate;
mod audio;

pub async fn run() {
    let index = CameraIndex::Index(0);
    let frame_fmt = RequestedFormat::new::<RgbAFormat>(
        nokhwa::utils::RequestedFormatType::AbsoluteHighestFrameRate,
    );

    nokhwa::nokhwa_initialize(|_| {});
    while !nokhwa::nokhwa_check() {}

    let mut camera = nokhwa::Camera::new(index, frame_fmt).expect("Failed to open camera!");
    let _ = camera.open_stream();

    let camera_dim = (camera.resolution().x(), camera.resolution().y());

    start_audio_capture();

    let event_loop = EventLoop::new();
    let window = WindowBuilder::new().build(&event_loop).unwrap();

    let app = Arc::new(RwLock::new(App::new(window, camera_dim).await));
    let rpctx = app.read().rpcontext.clone();

    let mut watch = Hotwatch::new().expect("Hotwatch failed to init!");
    let _ = watch.watch("frag.wgsl", move |event: hotwatch::notify::Event| {
        if let EventKind::Modify(_) = event.kind {
            println!("File Changed");
            pollster::block_on(RenderPipelineContext::rebuild_pipeline(
                rpctx.clone(),
                "frag.wgsl",
            ));
        }
    });

    event_loop.run(move |event, _, control_flow| {
        let read = app.read();
        match event {
            Event::WindowEvent {
                ref event,
                window_id,
            } if window_id == read.window().id() => {
                if !read.input(event) {
                    match event {
                        WindowEvent::CloseRequested
                        | WindowEvent::KeyboardInput {
                            input:
                                KeyboardInput {
                                    state: ElementState::Pressed,
                                    virtual_keycode: Some(VirtualKeyCode::Escape),
                                    ..
                                },
                            ..
                        } => *control_flow = ControlFlow::Exit,
                        WindowEvent::Resized(size) => {
                            drop(read);
                            let mut write = app.write();
                            write.resize(*size);
                        }
                        WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
                            drop(read);
                            let mut write = app.write();
                            write.resize(**new_inner_size);
                        }
                        _ => {}
                    }
                }
            }
            Event::RedrawRequested(window_id) if window_id == read.window().id() => {
                drop(read);
                let mut write = app.write();
                let frame = &camera
                    .frame()
                    .unwrap()
                    .decode_image::<RgbAFormat>()
                    .unwrap();

                write.update_camera(frame);
                write.update();
                let s = write.size;
                match write.render() {
                    Ok(_) => {}
                    // Reconfigure the surface if lost
                    Err(wgpu::SurfaceError::Lost) => write.resize(s),
                    // The system is out of memory, we should probably quit
                    Err(wgpu::SurfaceError::OutOfMemory) => *control_flow = ControlFlow::Exit,
                    // All other errors (Outdated, Timeout) should be resolved by the next frame
                    Err(e) => eprintln!("{:?}", e),
                }
            }
            Event::MainEventsCleared => {
                // RedrawRequested will only trigger once, unless we manually
                // request it.
                read.window().request_redraw();
            }
            _ => {}
        }
    });
}
