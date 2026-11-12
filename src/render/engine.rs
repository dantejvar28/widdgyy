// SPDX-FileCopyrightText: 2026 Daniel
// SPDX-License-Identifier: GPL-3.0-or-later

//! wgpu + Vello presentation.
//!
//! Vello renders to an intermediate `Rgba8Unorm` texture, which is then blitted
//! to the swapchain. This avoids binding the surface texture directly to the
//! compute pipeline and works with the layer-shell's transparent surfaces.

use anyhow::{Context, Result};
use vello::peniko::Color;
use vello::{AaConfig, RenderParams, Renderer as VelloRenderer, RendererOptions, Scene};
use wgpu::util::TextureBlitter;
use wgpu::{
    CompositeAlphaMode, Device, Instance, PresentMode, Queue, RenderPassColorAttachment,
    RenderPassDescriptor, Surface, SurfaceConfiguration, Texture, TextureFormat, TextureUsages,
    TextureView,
};

use super::ShadowDraw;
use super::compositor::{Compositor, MAX_SHADOW_ITEMS, MAX_SHADOW_SLOTS, ShadowItem, VideoFrame};
use super::filter::{FilterCompositor, FilterLayer, LayerItem, LayerSpec, Scissor, SpatialFilter};

pub struct GpuRenderer {
    pub device: Device,
    pub queue: Queue,
    pub surface: Surface<'static>,
    pub config: SurfaceConfiguration,
    pub format: TextureFormat,
    vello: VelloRenderer,
    compositor: Compositor,
    filter: FilterCompositor,
    shadow_filter: FilterCompositor,
    /// Quantized sigma currently stored in each shadow blur slot. The mask is
    /// re-blurred only when a slot's value changes.
    shadow_sigmas: Vec<Option<f32>>,
    shadow_scissor: Option<Scissor>,
    backdrop_filter: FilterCompositor,
    backdrop_source: Texture,
    backdrop_source_view: TextureView,
    backdrop_mask: Texture,
    backdrop_mask_view: TextureView,
    backdrop_blur: Texture,
    backdrop_blur_view: TextureView,
    target: Texture,
    target_view: TextureView,
    /// Accumulation textures for nested layer groups, allocated lazily per
    /// nesting depth and reused across frames.
    accum: Vec<TextureView>,
    /// Scratch mask for group clip paths, allocated on first use.
    clip_mask: Option<TextureView>,
    blitter: TextureBlitter,
    width: u32,
    height: u32,
}

impl GpuRenderer {
    pub fn new(
        instance: &Instance,
        surface: Surface<'static>,
        width: u32,
        height: u32,
    ) -> Result<Self> {
        let width = width.max(1);
        let height = height.max(1);
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
            ..Default::default()
        }))
        .context("no compatible GPU adapter")?;

        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("widdgyy"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            ..Default::default()
        }))
        .context("creating wgpu device")?;

        let capabilities = surface.get_capabilities(&adapter);
        let format = capabilities
            .formats
            .iter()
            .copied()
            .find(|format| {
                matches!(
                    format,
                    TextureFormat::Rgba8Unorm | TextureFormat::Bgra8Unorm
                )
            })
            .or_else(|| capabilities.formats.first().copied())
            .context("surface has no supported format")?;

        let alpha_mode = if capabilities
            .alpha_modes
            .contains(&CompositeAlphaMode::PreMultiplied)
        {
            CompositeAlphaMode::PreMultiplied
        } else {
            capabilities
                .alpha_modes
                .first()
                .copied()
                .unwrap_or(CompositeAlphaMode::Auto)
        };

        let present_mode = if capabilities.present_modes.contains(&PresentMode::Mailbox) {
            PresentMode::Mailbox
        } else {
            PresentMode::Fifo
        };
        let config = SurfaceConfiguration {
            usage: TextureUsages::RENDER_ATTACHMENT,
            format,
            width,
            height,
            present_mode,
            desired_maximum_frame_latency: 2,
            alpha_mode,
            view_formats: vec![],
            color_space: wgpu::SurfaceColorSpace::Auto,
        };
        surface.configure(&device, &config);

        let vello = VelloRenderer::new(&device, RendererOptions::default())
            .context("creating Vello renderer")?;
        let (target, target_view) = create_target(&device, width, height);
        let blitter = TextureBlitter::new(&device, format);
        let compositor = Compositor::new(&device, format, width, height);
        let filter = FilterCompositor::new(&device, TextureFormat::Rgba8Unorm, width, height);
        let (shadow_width, shadow_height) = shadow_extent(width, height);
        let shadow_filter = FilterCompositor::new(
            &device,
            TextureFormat::Rgba8Unorm,
            shadow_width,
            shadow_height,
        );
        let (backdrop_width, backdrop_height) = backdrop_extent(width, height);
        let backdrop_filter = FilterCompositor::new_with_passes(
            &device,
            TextureFormat::Rgba8Unorm,
            backdrop_width,
            backdrop_height,
            2,
        );
        let (backdrop_source, backdrop_source_view) = create_backdrop_texture(
            &device,
            "widdgyy backdrop source",
            backdrop_width,
            backdrop_height,
            format,
            TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
        );
        let (backdrop_mask, backdrop_mask_view) = create_backdrop_texture(
            &device,
            "widdgyy backdrop mask",
            backdrop_width,
            backdrop_height,
            TextureFormat::Rgba8Unorm,
            TextureUsages::STORAGE_BINDING | TextureUsages::TEXTURE_BINDING,
        );
        let (backdrop_blur, backdrop_blur_view) = create_backdrop_texture(
            &device,
            "widdgyy backdrop blur",
            backdrop_width,
            backdrop_height,
            TextureFormat::Rgba8Unorm,
            TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
        );

        Ok(Self {
            device,
            queue,
            surface,
            config,
            format,
            vello,
            compositor,
            filter,
            shadow_filter,
            shadow_sigmas: vec![None; MAX_SHADOW_SLOTS],
            shadow_scissor: None,
            backdrop_filter,
            backdrop_source,
            backdrop_source_view,
            backdrop_mask,
            backdrop_mask_view,
            backdrop_blur,
            backdrop_blur_view,
            target,
            target_view,
            accum: Vec::new(),
            clip_mask: None,
            blitter,
            width,
            height,
        })
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        let width = width.max(1);
        let height = height.max(1);
        if width == self.width && height == self.height {
            return;
        }
        self.width = width;
        self.height = height;
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);
        let (target, view) = create_target(&self.device, width, height);
        self.target = target;
        self.target_view = view;
        self.compositor.resize(&self.device, width, height);
        self.filter.resize(&self.device, width, height);
        let (shadow_width, shadow_height) = shadow_extent(width, height);
        self.shadow_filter
            .resize(&self.device, shadow_width, shadow_height);
        self.shadow_sigmas.fill(None);
        self.shadow_scissor = None;
        let (backdrop_width, backdrop_height) = backdrop_extent(width, height);
        self.backdrop_filter
            .resize(&self.device, backdrop_width, backdrop_height);
        let (source, source_view) = create_backdrop_texture(
            &self.device,
            "widdgyy backdrop source",
            backdrop_width,
            backdrop_height,
            self.format,
            TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
        );
        self.backdrop_source = source;
        self.backdrop_source_view = source_view;
        let (mask, mask_view) = create_backdrop_texture(
            &self.device,
            "widdgyy backdrop mask",
            backdrop_width,
            backdrop_height,
            TextureFormat::Rgba8Unorm,
            TextureUsages::STORAGE_BINDING | TextureUsages::TEXTURE_BINDING,
        );
        self.backdrop_mask = mask;
        self.backdrop_mask_view = mask_view;
        let (blur, blur_view) = create_backdrop_texture(
            &self.device,
            "widdgyy backdrop blur",
            backdrop_width,
            backdrop_height,
            TextureFormat::Rgba8Unorm,
            TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
        );
        self.backdrop_blur = blur;
        self.backdrop_blur_view = blur_view;
        self.accum.clear();
        self.clip_mask = None;
    }

    pub fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// Rasterize the static overlay with Vello into the compositor texture.
    pub fn render_overlay(&mut self, scene: &Scene) -> Result<()> {
        let params = RenderParams {
            base_color: Color::new([0.0, 0.0, 0.0, 0.0]),
            width: self.width,
            height: self.height,
            antialiasing_method: AaConfig::Area,
        };
        self.vello
            .render_to_texture(
                &self.device,
                &self.queue,
                scene,
                self.compositor.overlay_view(),
                &params,
            )
            .context("Vello overlay rasterization")
    }

    /// Rasterize the glyph coverage of every text shadow once. The result stays
    /// in the filter layer until the text changes; blur, offset and colour are
    /// applied per frame by `render_shadows` without touching Vello again.
    pub fn render_shadow_mask(&mut self, scene: &Scene) -> Result<()> {
        self.shadow_sigmas.fill(None);
        self.shadow_scissor = None;
        let (width, height) = shadow_extent(self.width, self.height);
        let params = RenderParams {
            base_color: Color::new([0.0, 0.0, 0.0, 0.0]),
            width,
            height,
            antialiasing_method: AaConfig::Area,
        };
        self.vello
            .render_to_texture(
                &self.device,
                &self.queue,
                scene,
                self.shadow_filter.layer_view(),
                &params,
            )
            .context("Vello shadow mask rasterization")
    }

    /// Blur the cached shadow mask once per distinct radius and upload the
    /// shadow list. Radii beyond the slot budget reuse the nearest slot.
    pub fn render_shadows(
        &mut self,
        shadows: &[ShadowDraw],
        region: Option<[f32; 4]>,
    ) -> Result<()> {
        let (width, height) = shadow_extent(self.width, self.height);
        let scissor = region.and_then(|region| {
            scaled_scissor(region, crate::app::SHADOW_MASK_SCALE, width, height)
        });
        // Shadow slots are full resolution even though the blur mask is half
        // resolution, so the final composite needs a separately scaled scissor.
        let output_scissor =
            region.and_then(|region| scaled_scissor(region, 1.0, self.width, self.height));
        if self.shadow_scissor != scissor {
            self.shadow_sigmas.fill(None);
            self.shadow_scissor = scissor;
        }
        let mut sigmas: Vec<f32> = Vec::new();
        let mut items = Vec::with_capacity(shadows.len().min(MAX_SHADOW_ITEMS));
        for shadow in shadows.iter().take(MAX_SHADOW_ITEMS) {
            let sigma = quantize_shadow_sigma(shadow.sigma);
            let slot = match sigmas
                .iter()
                .position(|current| (current - sigma).abs() < 1e-4)
            {
                Some(slot) => slot,
                None if sigmas.len() < MAX_SHADOW_SLOTS => {
                    sigmas.push(sigma);
                    sigmas.len() - 1
                }
                None => nearest_slot(&sigmas, sigma),
            };
            items.push(ShadowItem {
                slot,
                offset: shadow.offset,
                color: shadow.color,
            });
        }

        for (slot, sigma) in sigmas.iter().enumerate() {
            if self.shadow_sigmas.get(slot).copied().flatten() == Some(*sigma) {
                continue;
            }
            // Hard shadows skip the blur; the others run at mask resolution,
            // so their sigma is halved to match.
            let filter = (*sigma >= HARD_SHADOW_SIGMA).then(|| SpatialFilter::Blur {
                radius: sigma * crate::app::SHADOW_MASK_SCALE as f32,
            });
            let mut encoder = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("widdgyy shadow blur"),
                });
            self.shadow_filter.encode_layer_scissored(
                &self.queue,
                &mut encoder,
                self.compositor.shadow_slot_view(slot),
                filter,
                true,
                scissor,
                output_scissor,
            );
            self.queue.submit(Some(encoder.finish()));
            self.shadow_sigmas[slot] = Some(*sigma);
        }
        self.compositor.set_shadows(&self.queue, &items);
        Ok(())
    }

    /// Rasterize the panel mask used by the backdrop pass.
    pub fn render_backdrop_mask(&mut self, scene: &Scene) -> Result<()> {
        let (width, height) = backdrop_extent(self.width, self.height);
        let params = RenderParams {
            base_color: Color::new([0.0, 0.0, 0.0, 0.0]),
            width,
            height,
            antialiasing_method: AaConfig::Area,
        };
        self.vello
            .render_to_texture(
                &self.device,
                &self.queue,
                scene,
                &self.backdrop_mask_view,
                &params,
            )
            .context("Vello backdrop mask rasterization")
    }

    /// Composite the newest video frame and the cached overlay to the surface.
    pub fn render_composite(
        &mut self,
        video: Option<&VideoFrame>,
        backdrop_radius: Option<f32>,
        backdrop_region: Option<[f32; 4]>,
    ) -> Result<()> {
        self.compositor
            .prepare_video(&self.device, &self.queue, video);
        if let (Some(video), Some(radius)) = (video, backdrop_radius) {
            let (backdrop_width, backdrop_height) = backdrop_extent(self.width, self.height);
            let scissor = backdrop_region.and_then(|region| {
                scaled_scissor(
                    region,
                    crate::app::BACKDROP_MASK_SCALE,
                    backdrop_width,
                    backdrop_height,
                )
            });
            self.compositor.render_video_to(
                &self.device,
                &self.queue,
                &self.backdrop_source_view,
                video,
                crate::app::BACKDROP_MASK_SCALE as f32,
                scissor,
            );
            let mut encoder = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("widdgyy backdrop blur"),
                });
            self.backdrop_filter.encode_composite(
                &self.queue,
                &mut encoder,
                &self.backdrop_blur_view,
                LayerSpec {
                    source: &self.backdrop_source_view,
                    filter: Some(SpatialFilter::Blur {
                        radius: radius * crate::app::BACKDROP_MASK_SCALE as f32,
                    }),
                    opacity: 1.0,
                    mask: None,
                    source_premultiplied: false,
                    scissor,
                },
                true,
            );
            self.queue.submit(Some(encoder.finish()));
            self.compositor.set_backdrop(
                &self.device,
                &self.backdrop_blur_view,
                &self.backdrop_mask_view,
            );
        } else {
            self.compositor.clear_backdrop();
        }

        let Some(frame) = self.acquire_surface() else {
            return Ok(());
        };
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("widdgyy composite"),
            });
        self.compositor
            .encode(&self.device, &self.queue, &mut encoder, &view, video);
        self.queue.submit(Some(encoder.finish()));
        self.queue.present(frame);
        Ok(())
    }

    pub fn render(&mut self, scene: &Scene) -> Result<()> {
        let params = RenderParams {
            base_color: Color::new([0.0, 0.0, 0.0, 0.0]),
            width: self.width,
            height: self.height,
            antialiasing_method: AaConfig::Area,
        };
        self.vello
            .render_to_texture(&self.device, &self.queue, scene, &self.target_view, &params)
            .context("Vello rasterization")?;

        let Some(frame) = self.acquire_surface() else {
            return Ok(());
        };

        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("widdgyy blit"),
            });
        self.blitter
            .copy(&self.device, &mut encoder, &self.target_view, &view);
        self.queue.submit(Some(encoder.finish()));
        self.queue.present(frame);
        Ok(())
    }

    /// Rasterize root layers in paint order. Nested groups recurse into
    /// per-depth accumulation textures, so filters keep working under
    /// opacity, clip-path, isolation and other filters.
    pub fn render_layers(&mut self, layers: &[FilterLayer]) -> Result<()> {
        if layers.is_empty() {
            return Ok(());
        }
        let target = self.target_view.clone();
        let mut clear = true;
        for layer in layers {
            self.render_layer(layer, 0, &target, clear)?;
            clear = false;
        }

        let Some(frame) = self.acquire_surface() else {
            return Ok(());
        };
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("widdgyy filtered blit"),
            });
        self.blitter
            .copy(&self.device, &mut encoder, &self.target_view, &view);
        self.queue.submit(Some(encoder.finish()));
        self.queue.present(frame);
        Ok(())
    }

    /// Composite one layer into `target`. A layer with a single scene uses the
    /// shared scratch texture; anything with nested items accumulates into a
    /// depth-specific texture first.
    fn render_layer(
        &mut self,
        layer: &FilterLayer,
        depth: usize,
        target: &TextureView,
        clear: bool,
    ) -> Result<()> {
        if layer.items.is_empty() {
            if clear {
                self.clear_view(target);
            }
            return Ok(());
        }
        if depth >= MAX_FILTER_DEPTH {
            warn_depth_limit();
            return self.render_layer_flat(layer, target, clear);
        }
        if layer.items.len() == 1
            && let LayerItem::Scene(scene) = &layer.items[0]
        {
            let scratch = self.filter.layer_view().clone();
            self.render_scene(scene, &scratch)?;
            return self.composite_layer(&scratch, target, layer, false, clear);
        }

        let accum = self.accum_view(depth);
        let mut first = true;
        for item in &layer.items {
            match item {
                LayerItem::Scene(scene) => {
                    let scratch = self.filter.layer_view().clone();
                    self.render_scene(scene, &scratch)?;
                    self.composite_plain(&scratch, &accum, false, first)?;
                }
                LayerItem::Layer(child) => {
                    self.render_layer(child, depth + 1, &accum, first)?;
                }
            }
            first = false;
        }
        self.composite_layer(&accum, target, layer, true, clear)
    }

    /// Last-resort painter for trees deeper than the accumulation pool. It
    /// keeps content and paint order but drops the group effects.
    fn render_layer_flat(
        &mut self,
        layer: &FilterLayer,
        target: &TextureView,
        clear: bool,
    ) -> Result<()> {
        if layer.items.is_empty() {
            if clear {
                self.clear_view(target);
            }
            return Ok(());
        }
        let mut first = clear;
        for item in &layer.items {
            match item {
                LayerItem::Scene(scene) => {
                    let scratch = self.filter.layer_view().clone();
                    self.render_scene(scene, &scratch)?;
                    self.composite_plain(&scratch, target, false, first)?;
                }
                LayerItem::Layer(child) => {
                    self.render_layer_flat(child, target, first)?;
                }
            }
            first = false;
        }
        Ok(())
    }

    fn composite_layer(
        &mut self,
        source: &TextureView,
        target: &TextureView,
        layer: &FilterLayer,
        source_premultiplied: bool,
        clear: bool,
    ) -> Result<()> {
        let mask = match &layer.mask {
            Some(scene) => {
                let view = self.clip_mask_view();
                self.render_scene(scene, &view)?;
                Some(view)
            }
            None => None,
        };
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("widdgyy layer composite"),
            });
        self.filter.encode_composite(
            &self.queue,
            &mut encoder,
            target,
            LayerSpec {
                source,
                filter: layer.filter,
                opacity: layer.opacity,
                mask: mask.as_ref(),
                source_premultiplied,
                scissor: None,
            },
            clear,
        );
        self.queue.submit(Some(encoder.finish()));
        Ok(())
    }

    fn composite_plain(
        &mut self,
        source: &TextureView,
        target: &TextureView,
        source_premultiplied: bool,
        clear: bool,
    ) -> Result<()> {
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("widdgyy layer item"),
            });
        self.filter.encode_composite(
            &self.queue,
            &mut encoder,
            target,
            LayerSpec {
                source,
                filter: None,
                opacity: 1.0,
                mask: None,
                source_premultiplied,
                scissor: None,
            },
            clear,
        );
        self.queue.submit(Some(encoder.finish()));
        Ok(())
    }

    fn render_scene(&mut self, scene: &Scene, target: &TextureView) -> Result<()> {
        let params = RenderParams {
            base_color: Color::new([0.0, 0.0, 0.0, 0.0]),
            width: self.width,
            height: self.height,
            antialiasing_method: AaConfig::Area,
        };
        self.vello
            .render_to_texture(&self.device, &self.queue, scene, target, &params)
            .context("Vello layer rasterization")
    }

    fn accum_view(&mut self, depth: usize) -> TextureView {
        while self.accum.len() <= depth {
            let (texture, view) = create_backdrop_texture(
                &self.device,
                "widdgyy filter group",
                self.width,
                self.height,
                TextureFormat::Rgba8Unorm,
                TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
            );
            // The view keeps the texture alive; this drops the extra handle.
            drop(texture);
            self.accum.push(view);
        }
        self.accum[depth].clone()
    }

    fn clip_mask_view(&mut self) -> TextureView {
        if self.clip_mask.is_none() {
            let (texture, view) = create_backdrop_texture(
                &self.device,
                "widdgyy clip mask",
                self.width,
                self.height,
                TextureFormat::Rgba8Unorm,
                TextureUsages::STORAGE_BINDING | TextureUsages::TEXTURE_BINDING,
            );
            drop(texture);
            self.clip_mask = Some(view);
        }
        self.clip_mask.clone().expect("clip mask initialized")
    }

    fn clear_view(&self, target: &TextureView) {
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("widdgyy layer clear"),
            });
        let _ = encoder.begin_render_pass(&RenderPassDescriptor {
            label: Some("widdgyy layer clear"),
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
        self.queue.submit(Some(encoder.finish()));
    }

    fn acquire_surface(&mut self) -> Option<wgpu::SurfaceTexture> {
        match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame)
            | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => Some(frame),
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                self.surface.configure(&self.device, &self.config);
                match self.surface.get_current_texture() {
                    wgpu::CurrentSurfaceTexture::Success(frame)
                    | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => Some(frame),
                    other => {
                        log::debug!("surface still unavailable after reconfigure: {other:?}");
                        None
                    }
                }
            }
            other => {
                log::debug!("surface unavailable: {other:?}");
                None
            }
        }
    }

    pub fn target(&self) -> &Texture {
        &self.target
    }
}

fn create_target(device: &Device, width: u32, height: u32) -> (Texture, TextureView) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("widdgyy vello target"),
        size: wgpu::Extent3d {
            width: width.max(1),
            height: height.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: TextureFormat::Rgba8Unorm,
        usage: TextureUsages::STORAGE_BINDING
            | TextureUsages::TEXTURE_BINDING
            | TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    (texture, view)
}

/// Shadow sigma below which the mask is composited without blur.
const HARD_SHADOW_SIGMA: f32 = 0.5;

/// Maximum nesting depth for offscreen layer groups.
const MAX_FILTER_DEPTH: usize = 8;

static DEPTH_LIMIT_WARNED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

fn warn_depth_limit() {
    if !DEPTH_LIMIT_WARNED.swap(true, std::sync::atomic::Ordering::Relaxed) {
        log::warn!(
            "layer nesting exceeds {MAX_FILTER_DEPTH}; deeper group effects are composited flat"
        );
    }
}

fn shadow_extent(width: u32, height: u32) -> (u32, u32) {
    (
        ((width as f64 * crate::app::SHADOW_MASK_SCALE).ceil() as u32).max(1),
        ((height as f64 * crate::app::SHADOW_MASK_SCALE).ceil() as u32).max(1),
    )
}

fn backdrop_extent(width: u32, height: u32) -> (u32, u32) {
    (
        ((width as f64 * crate::app::BACKDROP_MASK_SCALE).ceil() as u32).max(1),
        ((height as f64 * crate::app::BACKDROP_MASK_SCALE).ceil() as u32).max(1),
    )
}

fn create_backdrop_texture(
    device: &Device,
    label: &str,
    width: u32,
    height: u32,
    format: TextureFormat,
    usage: TextureUsages,
) -> (Texture, TextureView) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    (texture, view)
}

/// Quantize shadow sigmas so small animation steps reuse the cached blur.
fn quantize_shadow_sigma(sigma: f32) -> f32 {
    let step = 0.5;
    ((sigma.max(0.0) / step).round() * step).max(0.0)
}

fn scaled_scissor(region: [f32; 4], scale: f64, width: u32, height: u32) -> Option<Scissor> {
    let x0 = (region[0] as f64 * scale).floor().max(0.0) as u32;
    let y0 = (region[1] as f64 * scale).floor().max(0.0) as u32;
    let x1 = ((region[0] + region[2]) as f64 * scale)
        .ceil()
        .clamp(0.0, width as f64) as u32;
    let y1 = ((region[1] + region[3]) as f64 * scale)
        .ceil()
        .clamp(0.0, height as f64) as u32;
    (x1 > x0 && y1 > y0).then_some(Scissor {
        x: x0.min(width),
        y: y0.min(height),
        width: x1.saturating_sub(x0),
        height: y1.saturating_sub(y0),
    })
}

fn nearest_slot(sigmas: &[f32], sigma: f32) -> usize {
    sigmas
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| (*a - sigma).abs().total_cmp(&(*b - sigma).abs()))
        .map(|(index, _)| index)
        .unwrap_or(0)
}

#[cfg(test)]
mod shadow_cache_tests {
    use super::{quantize_shadow_sigma, scaled_scissor};

    #[test]
    fn scales_and_clamps_scissors() {
        let scissor = scaled_scissor([100.0, 50.0, 200.0, 100.0], 0.5, 960, 540).unwrap();
        assert_eq!(
            (scissor.x, scissor.y, scissor.width, scissor.height),
            (50, 25, 100, 50)
        );
        let clamped = scaled_scissor([-10.0, -10.0, 20.0, 20.0], 1.0, 1920, 1080).unwrap();
        assert_eq!(
            (clamped.x, clamped.y, clamped.width, clamped.height),
            (0, 0, 10, 10)
        );
        assert!(scaled_scissor([2000.0, 0.0, 10.0, 10.0], 1.0, 1920, 1080).is_none());
    }

    #[test]
    fn quantizes_shadow_sigmas_to_half_pixels() {
        assert!((quantize_shadow_sigma(4.1) - 4.0).abs() < 1e-6);
        assert!((quantize_shadow_sigma(4.26) - 4.5).abs() < 1e-6);
        assert!((quantize_shadow_sigma(0.2) - 0.0).abs() < 1e-6);
        assert!((quantize_shadow_sigma(-1.0) - 0.0).abs() < 1e-6);
    }
}
