// SPDX-FileCopyrightText: 2026 Daniel
// SPDX-License-Identifier: GPL-3.0-or-later

//! Offscreen CSS spatial filters (`blur()` and `drop-shadow()`).
//!
//! The painter emits a tree of [`FilterLayer`] groups. Every group is
//! rasterized into its own accumulation texture, its child groups are
//! composited recursively, and then the group's filter, clip mask and opacity
//! are applied when it is composited into its parent. That keeps filters
//! working under `opacity`, `clip-path`, `overflow: hidden`, `isolation` and
//! inside another filtered subtree.
//!
//! Limits, kept intentionally small:
//! * `mix-blend-mode` and `mask-image` wrap their subtree in one Vello layer,
//!   so filters below them are painted inline (logged once).
//! * When a node has several spatial filters, a `blur()` wins over
//!   `drop-shadow()` and multiple blur radii collapse to the largest one.
//! * The blur radius is approximate: a fixed nine-tap Gaussian per axis.
//! * Group nesting is capped by the renderer (eight levels) to bound texture
//!   memory; deeper groups are composited without their effects.

use vello::Scene as VelloScene;
use wgpu::util::DeviceExt;
use wgpu::{
    BindGroup, BindGroupEntry, BindGroupLayout, BindGroupLayoutEntry, BindingResource, BindingType,
    BlendComponent, BlendFactor, BlendOperation, BlendState, Buffer, BufferBindingType,
    BufferUsages, ColorTargetState, ColorWrites, CommandEncoder, Device, FragmentState,
    MultisampleState, PipelineLayoutDescriptor, PrimitiveState, Queue, RenderPassColorAttachment,
    RenderPassDescriptor, RenderPipeline, RenderPipelineDescriptor, Sampler, SamplerBindingType,
    SamplerDescriptor, ShaderModuleDescriptor, ShaderSource, ShaderStages, Texture, TextureFormat,
    TextureSampleType, TextureUsages, TextureView, TextureViewDescriptor, TextureViewDimension,
    VertexState,
};

use crate::css::Axis;
use crate::css::properties::Filter;
use crate::css::values::ResolveContext;
use crate::scene::Node;

/// A spatial filter resolved to pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SpatialFilter {
    Blur {
        radius: f32,
    },
    DropShadow {
        blur: f32,
        offset: (f32, f32),
        /// Straight-alpha shadow colour.
        color: [f32; 4],
    },
}

/// One compositing operation: a source texture, the filter applied to it and
/// the group effects resolved after that filter.
pub struct LayerSpec<'a> {
    pub source: &'a TextureView,
    pub filter: Option<SpatialFilter>,
    pub opacity: f32,
    /// Clip mask painted white on transparent, in screen coordinates.
    pub mask: Option<&'a TextureView>,
    /// True when the source already stores premultiplied alpha (layer groups);
    /// false for Vello scenes and video frames.
    pub source_premultiplied: bool,
}

/// One item of a layer group, in paint order.
pub enum LayerItem {
    /// A run of ordinary Vello paint commands.
    Scene(VelloScene),
    /// A nested group with its own filter, opacity or clip mask.
    Layer(FilterLayer),
}

/// A compositing layer, in paint order.
///
/// Nodes with a spatial filter, or nodes with opacity/clip/isolation over a
/// filtered subtree, become groups. Their content is painted into `items` and
/// the effects are applied when the group is composited, so nested filters
/// keep working under those effects.
pub struct FilterLayer {
    pub items: Vec<LayerItem>,
    pub filter: Option<SpatialFilter>,
    pub opacity: f32,
    pub mask: Option<VelloScene>,
}

impl FilterLayer {
    pub fn plain(scene: VelloScene) -> Self {
        Self {
            items: vec![LayerItem::Scene(scene)],
            filter: None,
            opacity: 1.0,
            mask: None,
        }
    }

    pub fn is_plain(&self) -> bool {
        self.filter.is_none() && self.opacity >= 0.999 && self.mask.is_none()
    }
}

/// Cheap classification used for logging and fast-path decisions.
pub fn has_spatial_filter(filters: &[Filter]) -> bool {
    filters
        .iter()
        .any(|filter| matches!(filter, Filter::Blur(_) | Filter::DropShadow(_)))
}

/// Resolve the spatial filter for a root node.
///
/// Only the root's own filter list is considered. Colour filters are applied
/// while painting colours, so they intentionally do not appear here.
pub fn root_spatial_filter(node: &Node) -> Option<SpatialFilter> {
    let width = node.layout.width.max(0.0);
    let height = node.layout.height.max(0.0);
    let context = node
        .style
        .resolve_context(crate::css::Viewport::default(), width, height);
    spatial_filter(&node.style.effects.filters, &context)
}

/// Resolve a filter list. `blur()` takes priority when both spatial filters
/// are present; the drop shadow is then ignored (documented approximation).
pub fn spatial_filter(filters: &[Filter], context: &ResolveContext) -> Option<SpatialFilter> {
    let mut blur_radius: Option<f32> = None;
    let mut shadow = None;

    for filter in filters {
        match filter {
            Filter::Blur(length) => {
                let radius = length.resolve(context, Axis::Horizontal).max(0.0);
                blur_radius = Some(blur_radius.map_or(radius, |current| current.max(radius)));
            }
            Filter::DropShadow(value) if shadow.is_none() => {
                shadow = Some(SpatialFilter::DropShadow {
                    blur: value.blur.resolve(context, Axis::Horizontal).max(0.0),
                    offset: (
                        value.offset_x.resolve(context, Axis::Horizontal),
                        value.offset_y.resolve(context, Axis::Vertical),
                    ),
                    color: value.color.components,
                });
            }
            _ => {}
        }
    }

    if let Some(radius) = blur_radius {
        Some(SpatialFilter::Blur { radius })
    } else {
        shadow
    }
}

/// GPU resources for the offscreen filter path.
///
/// Three full-size textures are kept for the lifetime of the renderer (layer,
/// horizontal blur, vertical blur) and reused for every layer and frame. This
/// keeps memory independent of the number of filtered roots.
/// Separable passes per Gaussian. Each pass is a nine-tap kernel with sigma
/// of three taps; four passes keep the residual sampling pattern invisible.
pub const BLUR_PASSES: u32 = 4;

pub struct FilterCompositor {
    device: Device,
    width: u32,
    height: u32,
    blur_bind_group_layout: BindGroupLayout,
    composite_bind_group_layout: BindGroupLayout,
    sampler: Sampler,
    premultiply_pipeline: RenderPipeline,
    blur_pipeline: RenderPipeline,
    composite_pipeline: RenderPipeline,
    /// Scratch target for the most recent Vello scene. Nested groups
    /// rasterize their scenes here before compositing them into their
    /// accumulation texture.
    layer_view: TextureView,
    blur_a_view: TextureView,
    blur_b_view: TextureView,
    blur_v_bind_group: BindGroup,
    blur_h2_bind_group: BindGroup,
    blur_h_uniform: Buffer,
    blur_v_uniform: Buffer,
    composite_uniform: Buffer,
}

impl FilterCompositor {
    pub fn new(device: &Device, format: TextureFormat, width: u32, height: u32) -> Self {
        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("widdgyy filter shader"),
            source: ShaderSource::Wgsl(include_str!("filter.wgsl").into()),
        });

        let blur_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("widdgyy filter blur bind group layout"),
                entries: &[texture_entry(0), sampler_entry(1), uniform_entry(2)],
            });
        let composite_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("widdgyy filter composite bind group layout"),
                entries: &[
                    texture_entry(0),
                    texture_entry(1),
                    sampler_entry(2),
                    uniform_entry(3),
                    texture_entry(4),
                ],
            });

        let blur_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("widdgyy filter blur pipeline layout"),
            bind_group_layouts: &[Some(&blur_bind_group_layout)],
            immediate_size: 0,
        });
        let composite_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("widdgyy filter composite pipeline layout"),
            bind_group_layouts: &[Some(&composite_bind_group_layout)],
            immediate_size: 0,
        });

        let premultiply_pipeline = make_pipeline(
            device,
            &blur_layout,
            &shader,
            "fs_premultiply",
            format,
            BlendState::REPLACE,
            "widdgyy filter premultiply",
        );
        let blur_pipeline = make_pipeline(
            device,
            &blur_layout,
            &shader,
            "fs_blur",
            format,
            BlendState::REPLACE,
            "widdgyy filter blur",
        );
        let composite_pipeline = make_pipeline(
            device,
            &composite_layout,
            &shader,
            "fs_composite",
            format,
            BlendState {
                color: BlendComponent {
                    src_factor: BlendFactor::One,
                    dst_factor: BlendFactor::OneMinusSrcAlpha,
                    operation: BlendOperation::Add,
                },
                alpha: BlendComponent {
                    src_factor: BlendFactor::One,
                    dst_factor: BlendFactor::OneMinusSrcAlpha,
                    operation: BlendOperation::Add,
                },
            },
            "widdgyy filter composite",
        );

        let sampler = device.create_sampler(&SamplerDescriptor {
            label: Some("widdgyy filter sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });

        let (layer_view, blur_a_view, blur_b_view) = create_filter_textures(device, width, height);

        let blur_h_uniform = create_uniform(device, "widdgyy filter blur h", &[0.0; 4]);
        let blur_v_uniform = create_uniform(device, "widdgyy filter blur v", &[0.0; 4]);
        let composite_uniform = create_uniform(
            device,
            "widdgyy filter composite",
            &composite_params(0.0, false, [0.0; 4], (0.0, 0.0), 1.0, false),
        );

        let blur_v_bind_group = create_blur_bind_group(
            device,
            &blur_bind_group_layout,
            &blur_a_view,
            &sampler,
            &blur_v_uniform,
            "widdgyy filter blur v bind group",
        );
        let blur_h2_bind_group = create_blur_bind_group(
            device,
            &blur_bind_group_layout,
            &blur_b_view,
            &sampler,
            &blur_h_uniform,
            "widdgyy filter blur h2 bind group",
        );

        Self {
            device: device.clone(),
            width: width.max(1),
            height: height.max(1),
            blur_bind_group_layout,
            composite_bind_group_layout,
            sampler,
            premultiply_pipeline,
            blur_pipeline,
            composite_pipeline,
            layer_view,
            blur_a_view,
            blur_b_view,
            blur_v_bind_group,
            blur_h2_bind_group,
            blur_h_uniform,
            blur_v_uniform,
            composite_uniform,
        }
    }

    pub fn resize(&mut self, device: &Device, width: u32, height: u32) {
        let width = width.max(1);
        let height = height.max(1);
        if width == self.width && height == self.height {
            return;
        }
        self.width = width;
        self.height = height;
        let (layer_view, blur_a_view, blur_b_view) = create_filter_textures(device, width, height);
        self.layer_view = layer_view;
        self.blur_a_view = blur_a_view;
        self.blur_b_view = blur_b_view;

        self.blur_v_bind_group = create_blur_bind_group(
            device,
            &self.blur_bind_group_layout,
            &self.blur_a_view,
            &self.sampler,
            &self.blur_v_uniform,
            "widdgyy filter blur v bind group",
        );
        self.blur_h2_bind_group = create_blur_bind_group(
            device,
            &self.blur_bind_group_layout,
            &self.blur_b_view,
            &self.sampler,
            &self.blur_h_uniform,
            "widdgyy filter blur h2 bind group",
        );
    }

    /// Scratch target used to rasterize Vello scenes before compositing.
    pub fn layer_view(&self) -> &TextureView {
        &self.layer_view
    }

    /// Compatible shorthand for compositing this compositor's scratch layer
    /// with no opacity or mask.
    pub fn encode_layer(
        &mut self,
        queue: &Queue,
        encoder: &mut CommandEncoder,
        target: &TextureView,
        filter: Option<SpatialFilter>,
        clear: bool,
    ) {
        let source = self.layer_view.clone();
        self.encode_composite(
            queue,
            encoder,
            target,
            LayerSpec {
                source: &source,
                filter,
                opacity: 1.0,
                mask: None,
                source_premultiplied: false,
            },
            clear,
        );
    }

    /// Record the passes for one compositing operation: optional spatial
    /// filter, then clip mask and opacity.
    ///
    /// Callers must submit the encoder before reusing the compositor, because
    /// the blur and composite parameters share small uniform buffers.
    pub fn encode_composite(
        &mut self,
        queue: &Queue,
        encoder: &mut CommandEncoder,
        target: &TextureView,
        spec: LayerSpec<'_>,
        clear: bool,
    ) {
        let LayerSpec {
            source,
            filter,
            opacity,
            mask,
            source_premultiplied,
        } = spec;

        let Some(filter) = filter else {
            self.composite(
                queue,
                encoder,
                source,
                source,
                target,
                CompositeMode::Plain,
                opacity,
                mask,
                source_premultiplied,
                (0.0, 0.0),
                [0.0; 4],
                clear,
            );
            return;
        };

        // CSS `blur()` uses the value as the standard deviation, while
        // box/text/drop shadows use half of their blur radius.
        let sigma = match filter {
            SpatialFilter::Blur { radius } => radius,
            SpatialFilter::DropShadow { blur, .. } => blur * 0.5,
        }
        .max(0.0);
        // Repeating the separable pass N times with spacing sigma / (3·√N)
        // gives the requested total sigma. Many small steps keep the taps
        // dense enough that the discrete sampling lattice does not show.
        let iterations = BLUR_PASSES;
        let step = sigma / (3.0 * (iterations as f32).sqrt());
        let step_x = step / self.width as f32;
        let step_y = step / self.height as f32;
        let straight = if source_premultiplied { 0.0 } else { 1.0 };
        write_uniform(queue, &self.blur_h_uniform, &[step_x, 0.0, straight, 0.0]);
        write_uniform(queue, &self.blur_v_uniform, &[0.0, step_y, straight, 0.0]);

        let source_bind_group = create_blur_bind_group(
            &self.device,
            &self.blur_bind_group_layout,
            source,
            &self.sampler,
            &self.blur_h_uniform,
            "widdgyy filter blur source bind group",
        );
        self.blur_pass(
            encoder,
            &self.blur_a_view,
            &self.premultiply_pipeline,
            &source_bind_group,
        );
        for iteration in 0..iterations {
            self.blur_pass(
                encoder,
                &self.blur_b_view,
                &self.blur_pipeline,
                &self.blur_v_bind_group,
            );
            if iteration + 1 < iterations {
                self.blur_pass(
                    encoder,
                    &self.blur_a_view,
                    &self.blur_pipeline,
                    &self.blur_h2_bind_group,
                );
            }
        }

        let (mode, offset, color) = match filter {
            SpatialFilter::Blur { .. } => (CompositeMode::Blurred, (0.0, 0.0), [0.0; 4]),
            SpatialFilter::DropShadow { offset, color, .. } => {
                let offset = (offset.0 / self.width as f32, offset.1 / self.height as f32);
                (CompositeMode::Shadow, offset, color)
            }
        };
        self.composite(
            queue,
            encoder,
            source,
            &self.blur_b_view,
            target,
            mode,
            opacity,
            mask,
            source_premultiplied,
            offset,
            color,
            clear,
        );
    }

    fn blur_pass(
        &self,
        encoder: &mut CommandEncoder,
        target: &TextureView,
        pipeline: &RenderPipeline,
        bind_group: &BindGroup,
    ) {
        let mut pass = encoder.begin_render_pass(&RenderPassDescriptor {
            label: Some("widdgyy filter blur"),
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
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, bind_group, &[]);
        pass.draw(0..3, 0..1);
    }

    #[allow(clippy::too_many_arguments)]
    fn composite(
        &self,
        queue: &Queue,
        encoder: &mut CommandEncoder,
        source: &TextureView,
        blur_result: &TextureView,
        target: &TextureView,
        mode: CompositeMode,
        opacity: f32,
        mask: Option<&TextureView>,
        premultiplied: bool,
        offset: (f32, f32),
        color: [f32; 4],
        clear: bool,
    ) {
        let params = composite_params(
            mode.code(),
            premultiplied,
            color,
            offset,
            opacity.clamp(0.0, 1.0),
            mask.is_some(),
        );
        write_uniform(queue, &self.composite_uniform, &params);
        let mask_view = mask.unwrap_or(source);
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("widdgyy filter composite bind group"),
            layout: &self.composite_bind_group_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::TextureView(source),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::TextureView(blur_result),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: BindingResource::Sampler(&self.sampler),
                },
                BindGroupEntry {
                    binding: 3,
                    resource: self.composite_uniform.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 4,
                    resource: BindingResource::TextureView(mask_view),
                },
            ],
        });
        let load = if clear {
            wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT)
        } else {
            wgpu::LoadOp::Load
        };
        let mut pass = encoder.begin_render_pass(&RenderPassDescriptor {
            label: Some("widdgyy filter composite"),
            color_attachments: &[Some(RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&self.composite_pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.draw(0..3, 0..1);
    }
}

#[derive(Clone, Copy)]
enum CompositeMode {
    Plain,
    Blurred,
    Shadow,
}

impl CompositeMode {
    fn code(self) -> f32 {
        match self {
            CompositeMode::Plain => 0.0,
            CompositeMode::Blurred => 1.0,
            CompositeMode::Shadow => 2.0,
        }
    }
}

fn texture_entry(binding: u32) -> BindGroupLayoutEntry {
    BindGroupLayoutEntry {
        binding,
        visibility: ShaderStages::FRAGMENT,
        ty: BindingType::Texture {
            sample_type: TextureSampleType::Float { filterable: true },
            view_dimension: TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    }
}

fn sampler_entry(binding: u32) -> BindGroupLayoutEntry {
    BindGroupLayoutEntry {
        binding,
        visibility: ShaderStages::FRAGMENT,
        ty: BindingType::Sampler(SamplerBindingType::Filtering),
        count: None,
    }
}

fn uniform_entry(binding: u32) -> BindGroupLayoutEntry {
    BindGroupLayoutEntry {
        binding,
        visibility: ShaderStages::FRAGMENT,
        ty: BindingType::Buffer {
            ty: BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

fn make_pipeline(
    device: &Device,
    layout: &wgpu::PipelineLayout,
    shader: &wgpu::ShaderModule,
    entry_point: &'static str,
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
            entry_point: Some(entry_point),
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

fn create_filter_textures(
    device: &Device,
    width: u32,
    height: u32,
) -> (TextureView, TextureView, TextureView) {
    let size = wgpu::Extent3d {
        width: width.max(1),
        height: height.max(1),
        depth_or_array_layers: 1,
    };
    let layer = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("widdgyy filter layer"),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: TextureFormat::Rgba8Unorm,
        usage: TextureUsages::STORAGE_BINDING | TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let blur_a = create_blur_texture(device, "widdgyy filter blur a", size);
    let blur_b = create_blur_texture(device, "widdgyy filter blur b", size);
    (
        layer.create_view(&TextureViewDescriptor::default()),
        blur_a.create_view(&TextureViewDescriptor::default()),
        blur_b.create_view(&TextureViewDescriptor::default()),
    )
}

fn create_blur_texture(device: &Device, label: &str, size: wgpu::Extent3d) -> Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: TextureFormat::Rgba8Unorm,
        usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    })
}

fn create_uniform(device: &Device, label: &str, values: &[f32]) -> Buffer {
    let bytes = f32_bytes(values);
    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some(label),
        contents: &bytes,
        usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
    })
}

fn write_uniform(queue: &Queue, buffer: &Buffer, values: &[f32]) {
    queue.write_buffer(buffer, 0, &f32_bytes(values));
}

fn f32_bytes(values: &[f32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(values.len() * 4);
    for value in values {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes
}

fn composite_params(
    mode: f32,
    premultiplied: bool,
    color: [f32; 4],
    offset: (f32, f32),
    opacity: f32,
    has_mask: bool,
) -> [f32; 12] {
    [
        color[0],
        color[1],
        color[2],
        color[3],
        offset.0,
        offset.1,
        mode,
        if premultiplied { 1.0 } else { 0.0 },
        opacity,
        if has_mask { 1.0 } else { 0.0 },
        0.0,
        0.0,
    ]
}

fn create_blur_bind_group(
    device: &Device,
    layout: &BindGroupLayout,
    source: &TextureView,
    sampler: &Sampler,
    uniform: &Buffer,
    label: &str,
) -> BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some(label),
        layout,
        entries: &[
            BindGroupEntry {
                binding: 0,
                resource: BindingResource::TextureView(source),
            },
            BindGroupEntry {
                binding: 1,
                resource: BindingResource::Sampler(sampler),
            },
            BindGroupEntry {
                binding: 2,
                resource: uniform.as_entire_binding(),
            },
        ],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::css::values::{Length, Shadow};

    fn shadow() -> Filter {
        Filter::DropShadow(Shadow {
            offset_x: Length::px(4.0),
            offset_y: Length::px(6.0),
            blur: Length::px(10.0),
            spread: Length::Zero,
            color: vello::peniko::Color::new([0.0, 0.0, 0.0, 0.5]),
            inset: false,
        })
    }

    #[test]
    fn classifies_blur_radius() {
        let context = ResolveContext::default();
        let filter = spatial_filter(&[Filter::Blur(Length::px(8.0))], &context);
        assert_eq!(filter, Some(SpatialFilter::Blur { radius: 8.0 }));
    }

    #[test]
    fn resolves_drop_shadow_geometry() {
        let context = ResolveContext::default();
        let filter = spatial_filter(&[shadow()], &context);
        assert_eq!(
            filter,
            Some(SpatialFilter::DropShadow {
                blur: 10.0,
                offset: (4.0, 6.0),
                color: [0.0, 0.0, 0.0, 0.5],
            })
        );
    }

    #[test]
    fn ignores_colour_filters() {
        let context = ResolveContext::default();
        assert_eq!(
            spatial_filter(
                &[
                    Filter::Brightness(1.2),
                    Filter::Grayscale(1.0),
                    Filter::HueRotate(crate::css::values::Angle(45.0)),
                ],
                &context,
            ),
            None
        );
        assert!(!has_spatial_filter(&[Filter::Saturate(0.5)]));
    }

    #[test]
    fn blur_wins_when_both_spatial_filters_are_present() {
        let context = ResolveContext::default();
        assert_eq!(
            spatial_filter(&[shadow(), Filter::Blur(Length::px(3.0))], &context),
            Some(SpatialFilter::Blur { radius: 3.0 })
        );
    }

    #[test]
    fn largest_blur_radius_wins() {
        let context = ResolveContext::default();
        assert_eq!(
            spatial_filter(
                &[Filter::Blur(Length::px(3.0)), Filter::Blur(Length::px(9.0)),],
                &context,
            ),
            Some(SpatialFilter::Blur { radius: 9.0 })
        );
    }
}
