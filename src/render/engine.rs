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
    CompositeAlphaMode, Device, Instance, PresentMode, Queue, Surface, SurfaceConfiguration,
    Texture, TextureFormat, TextureUsages, TextureView,
};

use super::GlowLayer;
use super::compositor::{Compositor, VideoFrame};
use super::filter::{FilterCompositor, FilterLayer, SpatialFilter};

pub struct GpuRenderer {
    pub device: Device,
    pub queue: Queue,
    pub surface: Surface<'static>,
    pub config: SurfaceConfiguration,
    pub format: TextureFormat,
    vello: VelloRenderer,
    compositor: Compositor,
    filter: FilterCompositor,
    target: Texture,
    target_view: TextureView,
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

        Ok(Self {
            device,
            queue,
            surface,
            config,
            format,
            vello,
            compositor,
            filter,
            target,
            target_view,
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

    /// Rasterize the CSS text-shadow masks and blur them into the compositor
    /// texture. Each layer carries its own radius, so different `text-shadow`
    /// blur values get their own offscreen pass.
    pub fn render_glow(&mut self, layers: &[GlowLayer]) -> Result<()> {
        if layers.is_empty() {
            self.compositor.clear_glow(&self.device, &self.queue);
            return Ok(());
        }
        for (index, layer) in layers.iter().enumerate() {
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
                    &layer.scene,
                    self.filter.layer_view(),
                    &params,
                )
                .context("Vello glow rasterization")?;
            let mut encoder = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("widdgyy glow blur"),
                });
            self.filter.encode_layer(
                &self.queue,
                &mut encoder,
                self.compositor.glow_view(),
                Some(SpatialFilter::Blur {
                    radius: layer.radius,
                }),
                index == 0,
            );
            self.queue.submit(Some(encoder.finish()));
        }
        Ok(())
    }

    /// Composite the newest video frame and the cached overlay to the surface.
    pub fn render_composite(&mut self, video: Option<&VideoFrame>) -> Result<()> {
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

    /// Rasterize root layers in paint order. Filtered layers go through the
    /// offscreen blur/drop-shadow passes; unfiltered runs are composited
    /// directly. The accumulated target is then blitted to the surface.
    pub fn render_layers(&mut self, layers: &[FilterLayer]) -> Result<()> {
        if layers.is_empty() {
            return Ok(());
        }
        let params = RenderParams {
            base_color: Color::new([0.0, 0.0, 0.0, 0.0]),
            width: self.width,
            height: self.height,
            antialiasing_method: AaConfig::Area,
        };
        for (index, layer) in layers.iter().enumerate() {
            self.vello
                .render_to_texture(
                    &self.device,
                    &self.queue,
                    &layer.scene,
                    self.filter.layer_view(),
                    &params,
                )
                .context("Vello filter layer rasterization")?;
            let mut encoder = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("widdgyy filter composite"),
                });
            self.filter.encode_layer(
                &self.queue,
                &mut encoder,
                &self.target_view,
                layer.filter,
                index == 0,
            );
            self.queue.submit(Some(encoder.finish()));
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
