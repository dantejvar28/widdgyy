// SPDX-FileCopyrightText: 2026 Daniel
// SPDX-License-Identifier: GPL-3.0-or-later

//! Fast video compositor.
//!
//! The normal Vello path rasterizes the whole scene (video included) every
//! time a frame arrives. That is unnecessary for the common wallpaper layout:
//! a background video plus a small static overlay. This compositor uploads the
//! newest NV12 planes to two textures, converts YUV to RGB in a fragment
//! shader, then blends a cached Vello overlay texture on top. Vello only runs
//! when the overlay changes (clock/API/resize), not at video frame rate.

use std::sync::Arc;

use crate::media::VideoFrameData;
use wgpu::{
    BindGroup, BindGroupEntry, BindGroupLayout, BindGroupLayoutEntry, BindingResource, BindingType,
    BlendComponent, BlendFactor, BlendOperation, BlendState, Buffer, ColorTargetState, ColorWrites,
    CommandEncoder, Device, Extent3d, FragmentState, MultisampleState, PipelineLayoutDescriptor,
    PrimitiveState, Queue, RenderPassColorAttachment, RenderPassDescriptor, RenderPipeline,
    RenderPipelineDescriptor, Sampler, SamplerBindingType, SamplerDescriptor,
    ShaderModuleDescriptor, ShaderSource, ShaderStages, Texture, TextureFormat, TextureSampleType,
    TextureUsages, TextureView, TextureViewDescriptor, TextureViewDimension, VertexState,
};

pub struct VideoFrame {
    pub data: Arc<VideoFrameData>,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

pub struct Compositor {
    width: u32,
    height: u32,
    overlay_bind_group_layout: BindGroupLayout,
    video_bind_group_layout: BindGroupLayout,
    sampler: Sampler,
    video_pipeline: RenderPipeline,
    overlay_pipeline: RenderPipeline,
    glow_buffer: Buffer,
    glow_start: std::time::Instant,
    glow_view: TextureView,
    overlay_view: TextureView,
    overlay_bind_group: BindGroup,
    video_y_texture: Option<Texture>,
    video_uv_texture: Option<Texture>,
    video_bind_group: Option<BindGroup>,
}

impl Compositor {
    pub fn new(device: &Device, format: TextureFormat, width: u32, height: u32) -> Self {
        let overlay_shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("widdgyy overlay shader"),
            source: ShaderSource::Wgsl(include_str!("compositor.wgsl").into()),
        });
        let video_shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("widdgyy NV12 video shader"),
            source: ShaderSource::Wgsl(include_str!("video.wgsl").into()),
        });

        let overlay_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("widdgyy overlay bind group layout"),
                entries: &[
                    BindGroupLayoutEntry {
                        binding: 0,
                        visibility: ShaderStages::FRAGMENT,
                        ty: BindingType::Texture {
                            sample_type: TextureSampleType::Float { filterable: true },
                            view_dimension: TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    BindGroupLayoutEntry {
                        binding: 1,
                        visibility: ShaderStages::FRAGMENT,
                        ty: BindingType::Sampler(SamplerBindingType::Filtering),
                        count: None,
                    },
                    BindGroupLayoutEntry {
                        binding: 2,
                        visibility: ShaderStages::FRAGMENT,
                        ty: BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: wgpu::BufferSize::new(16),
                        },
                        count: None,
                    },
                    BindGroupLayoutEntry {
                        binding: 3,
                        visibility: ShaderStages::FRAGMENT,
                        ty: BindingType::Texture {
                            sample_type: TextureSampleType::Float { filterable: true },
                            view_dimension: TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                ],
            });
        let video_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("widdgyy video bind group layout"),
                entries: &[
                    BindGroupLayoutEntry {
                        binding: 0,
                        visibility: ShaderStages::FRAGMENT,
                        ty: BindingType::Texture {
                            sample_type: TextureSampleType::Float { filterable: true },
                            view_dimension: TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    BindGroupLayoutEntry {
                        binding: 1,
                        visibility: ShaderStages::FRAGMENT,
                        ty: BindingType::Texture {
                            sample_type: TextureSampleType::Float { filterable: true },
                            view_dimension: TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    BindGroupLayoutEntry {
                        binding: 2,
                        visibility: ShaderStages::FRAGMENT,
                        ty: BindingType::Sampler(SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });

        let overlay_pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("widdgyy overlay pipeline layout"),
            bind_group_layouts: &[Some(&overlay_bind_group_layout)],
            immediate_size: 0,
        });
        let video_pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("widdgyy video pipeline layout"),
            bind_group_layouts: &[Some(&video_bind_group_layout)],
            immediate_size: 0,
        });

        let video_pipeline = make_pipeline(
            device,
            &video_pipeline_layout,
            &video_shader,
            format,
            BlendState::REPLACE,
            "widdgyy NV12 video quad",
        );
        let overlay_pipeline = make_pipeline(
            device,
            &overlay_pipeline_layout,
            &overlay_shader,
            format,
            BlendState {
                color: BlendComponent {
                    src_factor: BlendFactor::SrcAlpha,
                    dst_factor: BlendFactor::OneMinusSrcAlpha,
                    operation: BlendOperation::Add,
                },
                alpha: BlendComponent {
                    src_factor: BlendFactor::One,
                    dst_factor: BlendFactor::OneMinusSrcAlpha,
                    operation: BlendOperation::Add,
                },
            },
            "widdgyy overlay quad",
        );
        let sampler = device.create_sampler(&SamplerDescriptor {
            label: Some("widdgyy compositor sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });

        let glow_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("widdgyy glow uniform"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let (_overlay_texture, overlay_view) = create_overlay_texture(device, width, height);
        let (_glow_texture, glow_view) = create_glow_texture(device, width, height);
        let overlay_bind_group = create_overlay_bind_group(
            device,
            &overlay_bind_group_layout,
            &overlay_view,
            &glow_view,
            &sampler,
            &glow_buffer,
        );

        Self {
            width,
            height,
            overlay_bind_group_layout,
            video_bind_group_layout,
            sampler,
            video_pipeline,
            overlay_pipeline,
            glow_buffer,
            glow_start: std::time::Instant::now(),
            glow_view,
            overlay_view,
            overlay_bind_group,
            video_y_texture: None,
            video_uv_texture: None,
            video_bind_group: None,
        }
    }

    pub fn resize(&mut self, device: &Device, width: u32, height: u32) {
        if width == self.width && height == self.height {
            return;
        }
        self.width = width;
        self.height = height;
        let (_texture, view) = create_overlay_texture(device, width, height);
        self.overlay_view = view;
        let (_glow_texture, glow_view) = create_glow_texture(device, width, height);
        self.glow_view = glow_view;
        self.overlay_bind_group = create_overlay_bind_group(
            device,
            &self.overlay_bind_group_layout,
            &self.overlay_view,
            &self.glow_view,
            &self.sampler,
            &self.glow_buffer,
        );
    }

    pub fn overlay_view(&self) -> &TextureView {
        &self.overlay_view
    }

    pub fn glow_view(&self) -> &TextureView {
        &self.glow_view
    }

    /// Clear the pre-blurred glow layer before it is rebuilt.
    pub fn clear_glow(&self, device: &Device, queue: &Queue) {
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("widdgyy clear glow"),
        });
        {
            let _pass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: Some("widdgyy clear glow pass"),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: &self.glow_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
        }
        queue.submit(Some(encoder.finish()));
    }

    pub fn encode(
        &mut self,
        device: &Device,
        queue: &Queue,
        encoder: &mut CommandEncoder,
        target: &TextureView,
        video: Option<&VideoFrame>,
        glow_speed: f32,
    ) {
        if let Some(video) = video {
            self.upload_video(device, queue, video);
        }
        let params = [
            self.glow_start.elapsed().as_secs_f32(),
            glow_speed.max(0.0),
            0.0,
            0.0,
        ];
        queue.write_buffer(&self.glow_buffer, 0, &f32_bytes(&params));

        let mut pass = encoder.begin_render_pass(&RenderPassDescriptor {
            label: Some("widdgyy composite"),
            color_attachments: &[Some(RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });

        if let Some(video) = video
            && let Some(bind_group) = &self.video_bind_group
        {
            let x = video.x.max(0.0).min(self.width as f32);
            let y = video.y.max(0.0).min(self.height as f32);
            let width = video.width.min(self.width as f32 - x).max(0.0);
            let height = video.height.min(self.height as f32 - y).max(0.0);
            if width > 0.0 && height > 0.0 {
                pass.set_pipeline(&self.video_pipeline);
                pass.set_bind_group(0, bind_group, &[]);
                pass.set_viewport(x, y, width, height, 0.0, 1.0);
                pass.set_scissor_rect(x as u32, y as u32, width as u32, height as u32);
                pass.draw(0..3, 0..1);
            }
        }

        pass.set_pipeline(&self.overlay_pipeline);
        pass.set_bind_group(0, &self.overlay_bind_group, &[]);
        pass.set_viewport(0.0, 0.0, self.width as f32, self.height as f32, 0.0, 1.0);
        pass.set_scissor_rect(0, 0, self.width, self.height);
        pass.draw(0..3, 0..1);
    }

    fn upload_video(&mut self, device: &Device, queue: &Queue, video: &VideoFrame) {
        let data = &video.data;
        let width = data.width.max(1);
        let height = data.height.max(1);
        let uv_width = width.div_ceil(2);
        let uv_height = height.div_ceil(2);
        let needs_texture = self
            .video_y_texture
            .as_ref()
            .is_none_or(|texture| texture.width() != width || texture.height() != height);
        if needs_texture {
            let y_texture = create_plane_texture(
                device,
                "widdgyy video Y plane",
                width,
                height,
                TextureFormat::R8Unorm,
            );
            let uv_texture = create_plane_texture(
                device,
                "widdgyy video UV plane",
                uv_width,
                uv_height,
                TextureFormat::Rg8Unorm,
            );
            let y_view = y_texture.create_view(&TextureViewDescriptor::default());
            let uv_view = uv_texture.create_view(&TextureViewDescriptor::default());
            let bind_group = create_video_bind_group(
                device,
                &self.video_bind_group_layout,
                &y_view,
                &uv_view,
                &self.sampler,
            );
            self.video_y_texture = Some(y_texture);
            self.video_uv_texture = Some(uv_texture);
            self.video_bind_group = Some(bind_group);
        }

        if let (Some(y_texture), Some(uv_texture)) = (&self.video_y_texture, &self.video_uv_texture)
        {
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: y_texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &data.buffer[data.y_offset..],
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(data.y_stride),
                    rows_per_image: Some(height),
                },
                Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
            );
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: uv_texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &data.buffer[data.uv_offset..],
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(data.uv_stride),
                    rows_per_image: Some(uv_height),
                },
                Extent3d {
                    width: uv_width,
                    height: uv_height,
                    depth_or_array_layers: 1,
                },
            );
        }
    }
}

fn make_pipeline(
    device: &Device,
    layout: &wgpu::PipelineLayout,
    shader: &wgpu::ShaderModule,
    format: TextureFormat,
    blend: BlendState,
    label: &'static str,
) -> RenderPipeline {
    device.create_render_pipeline(&RenderPipelineDescriptor {
        label: Some(label),
        layout: Some(layout),
        vertex: VertexState {
            module: shader,
            entry_point: Some("vs_main"),
            buffers: &[],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        },
        primitive: PrimitiveState::default(),
        depth_stencil: None,
        multisample: MultisampleState::default(),
        fragment: Some(FragmentState {
            module: shader,
            entry_point: Some("fs_main"),
            targets: &[Some(ColorTargetState {
                format,
                blend: Some(blend),
                write_mask: ColorWrites::ALL,
            })],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        }),
        multiview_mask: None,
        cache: None,
    })
}

fn create_glow_texture(device: &Device, width: u32, height: u32) -> (Texture, TextureView) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("widdgyy glow target"),
        size: Extent3d {
            width: width.max(1),
            height: height.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: TextureFormat::Rgba8Unorm,
        usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let view = texture.create_view(&TextureViewDescriptor::default());
    (texture, view)
}

fn create_overlay_texture(device: &Device, width: u32, height: u32) -> (Texture, TextureView) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("widdgyy overlay target"),
        size: Extent3d {
            width: width.max(1),
            height: height.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: TextureFormat::Rgba8Unorm,
        usage: TextureUsages::STORAGE_BINDING | TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let view = texture.create_view(&TextureViewDescriptor::default());
    (texture, view)
}

fn create_plane_texture(
    device: &Device,
    label: &str,
    width: u32,
    height: u32,
    format: TextureFormat,
) -> Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: Extent3d {
            width: width.max(1),
            height: height.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
        view_formats: &[],
    })
}

fn create_overlay_bind_group(
    device: &Device,
    layout: &BindGroupLayout,
    view: &TextureView,
    glow_view: &TextureView,
    sampler: &Sampler,
    glow: &Buffer,
) -> BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("widdgyy overlay bind group"),
        layout,
        entries: &[
            BindGroupEntry {
                binding: 0,
                resource: BindingResource::TextureView(view),
            },
            BindGroupEntry {
                binding: 1,
                resource: BindingResource::Sampler(sampler),
            },
            BindGroupEntry {
                binding: 2,
                resource: glow.as_entire_binding(),
            },
            BindGroupEntry {
                binding: 3,
                resource: BindingResource::TextureView(glow_view),
            },
        ],
    })
}

fn f32_bytes(values: &[f32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(values.len() * 4);
    for value in values {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes
}

fn create_video_bind_group(
    device: &Device,
    layout: &BindGroupLayout,
    y_view: &TextureView,
    uv_view: &TextureView,
    sampler: &Sampler,
) -> BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("widdgyy video bind group"),
        layout,
        entries: &[
            BindGroupEntry {
                binding: 0,
                resource: BindingResource::TextureView(y_view),
            },
            BindGroupEntry {
                binding: 1,
                resource: BindingResource::TextureView(uv_view),
            },
            BindGroupEntry {
                binding: 2,
                resource: BindingResource::Sampler(sampler),
            },
        ],
    })
}
