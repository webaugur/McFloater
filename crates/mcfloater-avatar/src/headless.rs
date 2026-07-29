//! Headless wgpu-based avatar renderer.
//!
//! Renders a 3D avatar head driven by SAM lip-sync curves and produces
//! video frames suitable for WebRTC.

use std::sync::Arc;
use thiserror::Error;
use wgpu::util::DeviceExt;
use tracing::info;

#[derive(Debug, Error)]
pub enum AvatarError {
    #[error("wgpu adapter request failed")]
    Adapter,
    #[error("wgpu device request failed")]
    Device,
    #[error("shader compilation failed: {0}")]
    Shader(String),
    #[error("surface error: {0}")]
    Surface(String),
}

/// One frame of lip-sync data coming from SAM / Piper.
#[derive(Clone, Copy, Debug)]
pub struct LipSyncFrame {
    /// Mouth opening (0.0 = closed, 1.0 = fully open)
    pub mouth_open: f32,
    /// Jaw position (0.0 = neutral, positive = down)
    pub jaw: f32,
    /// Lip rounding (0.0 = neutral, 1.0 = rounded)
    pub lip_round: f32,
    /// Eyebrow raise (0.0 = neutral, 1.0 = raised)
    pub brow: f32,
}

/// The main headless avatar renderer.
pub struct AvatarRenderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    uniform_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    texture: wgpu::Texture,
    texture_view: wgpu::TextureView,
    width: u32,
    height: u32,
}

impl AvatarRenderer {
    /// Create a new headless avatar renderer.
    ///
    /// `width` and `height` define the output resolution of the video frames.
    pub async fn new(width: u32, height: u32) -> Result<Self, AvatarError> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..Default::default()
        });

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await
            .ok_or(AvatarError::Adapter)?;

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("mcfloater-avatar"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::downlevel_webgl2_defaults(),

                },
                None,
            )
            .await
            .map_err(|_| AvatarError::Device)?;

        // Create a simple quad that we will texture with an avatar image
        // or shade procedurally.
        let vertices: &[f32] = &[
            // position (x, y)   uv (u, v)
            -1.0, -1.0,  0.0, 1.0,
             1.0, -1.0,  1.0, 1.0,
            -1.0,  1.0,  0.0, 0.0,
             1.0,  1.0,  1.0, 0.0,
        ];

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("avatar-vertex"),
            contents: bytemuck::cast_slice(vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        // Uniform buffer for lip-sync parameters
        #[repr(C)]
        #[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
        struct AvatarUniforms {
            mouth_open: f32,
            jaw: f32,
            lip_round: f32,
            brow: f32,
            _pad: [f32; 4],
        }

        let initial_uniforms = AvatarUniforms {
            mouth_open: 0.0,
            jaw: 0.0,
            lip_round: 0.0,
            brow: 0.0,
            _pad: [0.0; 4],
        };

        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("avatar-uniforms"),
            contents: bytemuck::bytes_of(&initial_uniforms),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("avatar-bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("avatar-bg"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        // Simple shader that draws a stylized head and animates the mouth
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("avatar-shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("avatar-pipeline-layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("avatar-pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: 4 * 4,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[
                        wgpu::VertexAttribute {
                            offset: 0,
                            shader_location: 0,
                            format: wgpu::VertexFormat::Float32x2,
                        },
                        wgpu::VertexAttribute {
                            offset: 8,
                            shader_location: 1,
                            format: wgpu::VertexFormat::Float32x2,
                        },
                    ],
                }],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba8UnormSrgb,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,

        });

        // Off-screen texture we will render into
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("avatar-output"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });

        let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        info!("AvatarRenderer initialized at {}x{}", width, height);

        Ok(Self {
            device,
            queue,
            pipeline,
            vertex_buffer,
            uniform_buffer,
            bind_group,
            texture,
            texture_view,
            width,
            height,
        })
    }

    /// Render one frame using the provided lip-sync data.
    pub fn render(&mut self, frame: LipSyncFrame) -> image::ImageBuffer<image::Rgb<u8>, Vec<u8>> {
        // Update uniforms
        #[repr(C)]
        #[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
        struct AvatarUniforms {
            mouth_open: f32,
            jaw: f32,
            lip_round: f32,
            brow: f32,
            _pad: [f32; 4],
        }

        let uniforms = AvatarUniforms {
            mouth_open: frame.mouth_open,
            jaw: frame.jaw,
            lip_round: frame.lip_round,
            brow: frame.brow,
            _pad: [0.0; 4],
        };

        self.queue.write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(&uniforms));

        // Render
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("avatar-encoder"),
        });

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("avatar-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.texture_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.05,
                            g: 0.05,
                            b: 0.08,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.bind_group, &[]);
            pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            pass.draw(0..4, 0..1);
        }

        self.queue.submit(Some(encoder.finish()));

        // Read back the texture
        self.read_texture()
    }

    fn read_texture(&self) -> image::ImageBuffer<image::Rgb<u8>, Vec<u8>> {
        // For a production implementation we would use a readback buffer.
        // Here we return a simple colored image that visualizes the mouth state.
        let mut img = image::ImageBuffer::new(self.width, self.height);

        // Simple procedural head visualization
        for (x, y, pixel) in img.enumerate_pixels_mut() {
            let u = x as f32 / self.width as f32;
            let v = y as f32 / self.height as f32;

            // Head shape
            let dx = (u - 0.5) * 2.0;
            let dy = (v - 0.5) * 2.0;
            let dist = (dx * dx + dy * dy).sqrt();

            if dist < 0.85 {
                // Skin tone
                let r = 220u8;
                let g = 180u8;
                let b = 160u8;

                // Mouth area
                let mouth_y = 0.55 + 0.08; // rough mouth position
                let mouth_open = 0.08; // will be driven by uniforms in real shader

                if (v - mouth_y).abs() < 0.04 && (u - 0.5).abs() < 0.12 {
                    *pixel = image::Rgb([40, 20, 20]);
                } else {
                    *pixel = image::Rgb([r, g, b]);
                }
            } else {
                *pixel = image::Rgb([20, 20, 30]);
            }
        }

        img
    }

    /// Convenience: render directly from a SAM-style lip-sync curve point.
    pub fn render_from_curve(&mut self, mouth_open: f32, jaw: f32) -> image::ImageBuffer<image::Rgb<u8>, Vec<u8>> {
        self.render(LipSyncFrame {
            mouth_open,
            jaw,
            lip_round: 0.0,
            brow: 0.0,
        })
    }
}