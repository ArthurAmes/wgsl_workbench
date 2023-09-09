// Much of this code is boilerplate shamelessly stolen from https://sotrh.github.io/learn-wgpu

use parking_lot::RwLock;
use std::sync::Arc;

use wgpu::{
    self, include_wgsl, util::DeviceExt, BindGroupLayoutDescriptor, Buffer, Extent3d,
    ImageCopyTexture, Texture, TextureFormat, TextureView,
};

// lib.rs
use winit::{event::WindowEvent, window::Window};

pub struct ValidationError {
    description: String,
}

pub struct RenderPipelineContext {
    pub device: wgpu::Device,
    pub pipeline: wgpu::RenderPipeline,
    pub pipeline_layout: wgpu::PipelineLayout,
    pub surface_config: wgpu::SurfaceConfiguration,
    pub validation_errors: Arc<RwLock<Vec<ValidationError>>>,
}

impl RenderPipelineContext {
    pub async fn rebuild_pipeline(lock: Arc<RwLock<Self>>, frag_path: &str) {
        let read = lock.read();
        let vert = read
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("Vertex Shader"),
                source: wgpu::ShaderSource::Wgsl(include_str!("vert_default.wgsl").into()),
            });

        if let Ok(frag_str) = std::fs::read_to_string(frag_path) {
            const PRELUDE: &str = "@group(0) @binding(0)
var<uniform> res: vec2<f32>;
@group(0) @binding(1)
var<uniform> frame: u32;
@group(0) @binding(2)
var videoBuffer: texture_2d<f32>;
@group(0) @binding(3)
var videoSampler: sampler;
@group(1) @binding(0)
var backBuffer: texture_2d<f32>;
@group(1) @binding(1)
var backSampler: sampler;";

            let frag_str = [PRELUDE, &frag_str].join("\n");

            let frag = unsafe {
                read.device
                    .create_shader_module_unchecked(wgpu::ShaderModuleDescriptor {
                        label: Some("Fragment Shader"),
                        source: wgpu::ShaderSource::Wgsl(frag_str.into()),
                    })
            };

            if read.validation_errors.read().len() > 0 {
                let mut ve_wr = read.validation_errors.write();
                while ve_wr.len() > 0 {
                    println!("Validation Error: {:}", ve_wr.pop().unwrap().description);
                }
                return;
            }

            let render_pipeline =
                read.device
                    .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                        label: Some("Render Pipeline"),
                        layout: Some(&read.pipeline_layout),
                        vertex: wgpu::VertexState {
                            module: &vert,
                            entry_point: "vs_main", // 1.
                            buffers: &[],           // 2.
                        },
                        fragment: Some(wgpu::FragmentState {
                            // 3.
                            module: &frag,
                            entry_point: "main",
                            targets: &[Some(wgpu::ColorTargetState {
                                // 4.
                                format: read.surface_config.format,
                                blend: Some(wgpu::BlendState::REPLACE),
                                write_mask: wgpu::ColorWrites::ALL,
                            })],
                        }),
                        primitive: wgpu::PrimitiveState {
                            topology: wgpu::PrimitiveTopology::TriangleList, // 1.
                            strip_index_format: None,
                            front_face: wgpu::FrontFace::Ccw, // 2.
                            cull_mode: Some(wgpu::Face::Back),
                            // Setting this to anything other than Fill requires Features::NON_FILL_POLYGON_MODE
                            polygon_mode: wgpu::PolygonMode::Fill,
                            // Requires Features::DEPTH_CLIP_CONTROL
                            unclipped_depth: false,
                            // Requires Features::CONSERVATIVE_RASTERIZATION
                            conservative: false,
                        },
                        depth_stencil: None, // 1.
                        multisample: wgpu::MultisampleState {
                            count: 1,                         // 2.
                            mask: !0,                         // 3.
                            alpha_to_coverage_enabled: false, // 4.
                        },
                        multiview: None, // 5.
                    });

            drop(read);
            let mut write = lock.write();
            write.pipeline = render_pipeline;
        }
    }
}

pub struct BackBuffer {
    bind_group: wgpu::BindGroup,
    bind_group_layout: wgpu::BindGroupLayout,
    sample_texture: Texture,
    sample_texture_view: TextureView,
    format: TextureFormat,
}

impl BackBuffer {}

pub struct App {
    pub surface: wgpu::Surface,
    pub queue: wgpu::Queue,
    pub rpcontext: Arc<RwLock<RenderPipelineContext>>,
    pub size: winit::dpi::PhysicalSize<u32>,
    pub window: Window,
    pub unif_bind_group: wgpu::BindGroup,
    pub backbuffer: BackBuffer,
    pub res_buffer_unif: Buffer,
    pub frame_unif: Buffer,
    pub frame: u32,
    pub camera_texture: Texture,
    pub camera_dims: (u32, u32),
}

impl App {
    fn bb_refresh(&mut self, size: (u32, u32)) {
        let bb_texture_size = wgpu::Extent3d {
            width: size.0,
            height: size.1,
            depth_or_array_layers: 1,
        };

        let bb_sample_texture =
            self.rpcontext
                .write()
                .device
                .create_texture(&wgpu::TextureDescriptor {
                    size: bb_texture_size,
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: self.backbuffer.format,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                    label: Some("Back-Buffer Sample Texture"),
                    view_formats: &[],
                });

        let bb_sample_texture_view =
            bb_sample_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let bb_sampler = self
            .rpcontext
            .write()
            .device
            .create_sampler(&wgpu::SamplerDescriptor {
                address_mode_u: wgpu::AddressMode::ClampToEdge,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                address_mode_w: wgpu::AddressMode::ClampToEdge,
                mag_filter: wgpu::FilterMode::Nearest,
                min_filter: wgpu::FilterMode::Nearest,
                mipmap_filter: wgpu::FilterMode::Nearest,
                ..Default::default()
            });

        let bb_bind_group =
            self.rpcontext
                .write()
                .device
                .create_bind_group(&wgpu::BindGroupDescriptor {
                    layout: &self.backbuffer.bind_group_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(&bb_sample_texture_view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::Sampler(&bb_sampler),
                        },
                    ],
                    label: Some("bb_bind_group"),
                });

        self.backbuffer.bind_group = bb_bind_group;
        self.backbuffer.sample_texture = bb_sample_texture;
        self.backbuffer.sample_texture_view = bb_sample_texture_view;
    }

    pub fn update_camera(&mut self, pix: &[u8]) {
        let image_cpy = ImageCopyTexture {
            texture: &self.camera_texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        };
        self.queue.write_texture(
            image_cpy,
            pix,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(4 * self.camera_dims.0),
                rows_per_image: Some(self.camera_dims.1),
            },
            Extent3d {
                width: self.camera_dims.0,
                height: self.camera_dims.1,
                depth_or_array_layers: 1,
            },
        )
    }

    // Creating some of the wgpu types requires async code
    pub async fn new(window: Window, camera_dim: (u32, u32)) -> Self {
        let size = window.inner_size();

        // The instance is a handle to our GPU
        // Backends::all => Vulkan + Metal + DX12 + Browser WebGPU
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            dx12_shader_compiler: Default::default(),
        });

        // # Safety
        //
        // The surface needs to live as long as the window that created it.
        // State owns the window so this should be safe.
        let surface = unsafe { instance.create_surface(&window) }.unwrap();

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .unwrap();

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    features: wgpu::Features::empty(),
                    // WebGL doesn't support all of wgpu's features, so if
                    // we're building for the web we'll have to disable some.
                    limits: if cfg!(target_arch = "wasm32") {
                        wgpu::Limits::downlevel_webgl2_defaults()
                    } else {
                        wgpu::Limits::default()
                    },
                    label: None,
                },
                None, // Trace path
            )
            .await
            .unwrap();

        let surface_caps = surface.get_capabilities(&adapter);
        // Shader code in this tutorial assumes an sRGB surface texture. Using a different
        // one will result all the colors coming out darker. If you want to support non
        // sRGB surfaces, you'll need to account for that when drawing to the frame.
        let surface_format = surface_caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(surface_caps.formats[0]);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: surface_caps.present_modes[0],
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
        };
        surface.configure(&device, &config);

        // Set up camera texture

        let camera_texture_size = wgpu::Extent3d {
            width: camera_dim.0,
            height: camera_dim.1,
            depth_or_array_layers: 1,
        };

        let camera_texture = device.create_texture(&wgpu::TextureDescriptor {
            size: camera_texture_size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            label: Some("Camera Texture"),
            view_formats: &[],
        });

        let camera_texture_view =
            camera_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let camera_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        // Set up back buffer texture

        let bb_sample_texture = device.create_texture(&wgpu::TextureDescriptor {
            size: Extent3d {
                width: size.width,
                height: size.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: surface_format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            label: Some("Back-Buffer Texture"),
            view_formats: &[],
        });

        let bb_sample_texture_view =
            bb_sample_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let bb_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        // Set up uniforms (resolution, framecount, etc)

        let res_unif = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Resolution Uniform"),
            contents: bytemuck::cast_slice(&[0f32; 2]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let frame_unif = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Frame Count Uniform"),
            contents: bytemuck::cast_slice(&[0u32, 1]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let unif_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
                label: Some("unif_bind_group_layout"),
            });

        let unif_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &unif_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: res_unif.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: frame_unif.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&camera_texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&camera_sampler),
                },
            ],
            label: Some("unif_bind_group"),
        });

        let bb_bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
            label: Some("bb_bind_group_layout"),
        });

        let bb_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &bb_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&bb_sample_texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&bb_sampler),
                },
            ],
            label: Some("bb_bind_group"),
        });

        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Render Pipeline Layout"),
                bind_group_layouts: &[&unif_bind_group_layout, &bb_bind_group_layout],
                push_constant_ranges: &[],
            });

        let vert = device.create_shader_module(include_wgsl!("vert_default.wgsl"));
        let frag = device.create_shader_module(include_wgsl!("frag_default.wgsl"));

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &vert,
                entry_point: "vs_main", // 1.
                buffers: &[],           // 2.
            },
            fragment: Some(wgpu::FragmentState {
                // 3.
                module: &frag,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    // 4.
                    format: surface_format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList, // 1.
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw, // 2.
                cull_mode: Some(wgpu::Face::Back),
                // Setting this to anything other than Fill requires Features::NON_FILL_POLYGON_MODE
                polygon_mode: wgpu::PolygonMode::Fill,
                // Requires Features::DEPTH_CLIP_CONTROL
                unclipped_depth: false,
                // Requires Features::CONSERVATIVE_RASTERIZATION
                conservative: false,
            },
            depth_stencil: None, // 1.
            multisample: wgpu::MultisampleState {
                count: 1,                         // 2.
                mask: !0,                         // 3.
                alpha_to_coverage_enabled: false, // 4.
            },
            multiview: None, // 5.
        });

        let validation_errors = Arc::new(RwLock::new(vec![]));
        let c_validation_errors = validation_errors.clone();

        device.on_uncaptured_error(Box::new(move |e| match e {
            wgpu::Error::OutOfMemory { .. } => panic!("Device out of memory!"),
            wgpu::Error::Validation { description, .. } => {
                println!("validation error! {:}", description);
                c_validation_errors
                    .write()
                    .push(ValidationError { description });
            }
        }));

        let rpctx = Arc::new(RwLock::new(RenderPipelineContext {
            device,
            pipeline: render_pipeline,
            pipeline_layout: render_pipeline_layout,
            surface_config: config,
            validation_errors,
        }));

        Self {
            window,
            surface,
            queue,
            size,
            unif_bind_group,
            res_buffer_unif: res_unif,
            frame_unif,
            frame: 0,
            camera_texture,
            camera_dims: camera_dim,
            rpcontext: rpctx,
            backbuffer: BackBuffer {
                bind_group: bb_bind_group,
                bind_group_layout: bb_bind_group_layout,
                sample_texture: bb_sample_texture,
                sample_texture_view: bb_sample_texture_view,
                format: surface_format,
            },
        }
    }

    pub fn window(&self) -> &Window {
        &self.window
    }

    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.rpcontext.write().surface_config.width = new_size.width;
            self.rpcontext.write().surface_config.height = new_size.height;
            self.surface.configure(
                &self.rpcontext.read().device,
                &self.rpcontext.read().surface_config,
            );

            self.bb_refresh((new_size.width, new_size.height));
        }
    }

    pub fn input(&self, _event: &WindowEvent) -> bool {
        false
    }

    pub fn update(&mut self) {}

    pub fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        let output = self.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder =
            self.rpcontext
                .read()
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Render Encoder"),
                });

        self.queue.write_buffer(
            &self.res_buffer_unif,
            0,
            bytemuck::cast_slice(&[self.size.width as f32, self.size.height as f32]),
        );

        self.queue
            .write_buffer(&self.frame_unif, 0, bytemuck::cast_slice(&[self.frame]));

        self.frame += 1;

        let rpctx = self.rpcontext.read();

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 1.0,
                            g: 1.0,
                            b: 1.0,
                            a: 1.0,
                        }),
                        store: true,
                    },
                })],
                depth_stencil_attachment: None,
            });

            render_pass.set_pipeline(&rpctx.pipeline);
            render_pass.set_bind_group(0, &self.unif_bind_group, &[]);
            render_pass.set_bind_group(1, &self.backbuffer.bind_group, &[]);
            render_pass.draw(0..6, 0..1);
        }

        encoder.copy_texture_to_texture(
            ImageCopyTexture {
                texture: &output.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            ImageCopyTexture {
                texture: &self.backbuffer.sample_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            output.texture.size(),
        );

        // submit will accept anything that implements IntoIter
        self.queue.submit([encoder.finish()]);
        output.present();

        Ok(())
    }
}
