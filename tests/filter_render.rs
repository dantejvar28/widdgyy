// SPDX-FileCopyrightText: 2026 Daniel
// SPDX-License-Identifier: GPL-3.0-or-later

//! Headless smoke test for the offscreen blur / drop-shadow passes.
//!
//! Like `offscreen_render.rs`, this skips instead of failing when no GPU
//! adapter is available.

use widdgyy::render::filter::{FilterCompositor, SpatialFilter};

const WIDTH: u32 = 64;
const HEIGHT: u32 = 64;

#[test]
fn renders_blur_and_drop_shadow() {
    let Some((device, queue)) = device() else {
        eprintln!("no GPU adapter available; skipping offscreen filter assertion");
        return;
    };
    let mut renderer = vello::Renderer::new(
        &device,
        vello::RendererOptions {
            use_cpu: false,
            antialiasing_support: vello::AaSupport::area_only(),
            num_init_threads: None,
            pipeline_cache: None,
        },
    )
    .expect("creating Vello renderer");

    let mut scene = vello::Scene::new();
    scene.fill(
        vello::peniko::Fill::NonZero,
        vello::kurbo::Affine::IDENTITY,
        vello::peniko::Color::new([1.0, 0.0, 0.0, 1.0]),
        None,
        &vello::kurbo::Rect::new(20.0, 20.0, 44.0, 44.0),
    );

    let mut filters =
        FilterCompositor::new(&device, wgpu::TextureFormat::Rgba8Unorm, WIDTH, HEIGHT);
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("widdgyy filter test target"),
        size: wgpu::Extent3d {
            width: WIDTH,
            height: HEIGHT,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let target_view = target.create_view(&wgpu::TextureViewDescriptor::default());

    let cases = [
        (
            SpatialFilter::Blur { radius: 4.0 },
            false,
            "blurred layer produced no alpha",
        ),
        (
            SpatialFilter::DropShadow {
                blur: 2.0,
                offset: (8.0, 8.0),
                color: [0.0, 0.0, 0.0, 1.0],
            },
            true,
            "shadowed layer produced no alpha",
        ),
    ];

    for (filter, expect_shadow, alpha_message) in cases {
        renderer
            .render_to_texture(
                &device,
                &queue,
                &scene,
                filters.layer_view(),
                &vello::RenderParams {
                    base_color: vello::peniko::Color::new([0.0, 0.0, 0.0, 0.0]),
                    width: WIDTH,
                    height: HEIGHT,
                    antialiasing_method: vello::AaConfig::Area,
                },
            )
            .expect("rendering filter layer");
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        filters.encode_layer(&queue, &mut encoder, &target_view, Some(filter), true, 2);
        queue.submit(Some(encoder.finish()));

        let pixels = read_back(&device, &queue, &target);
        let center = (32 * WIDTH as usize + 32) * 4;
        assert!(pixels[center + 3] > 0, "{alpha_message}");
        assert!(pixels[center] > 0, "filtered layer lost its colour");
        if expect_shadow {
            let shadow = (50 * WIDTH as usize + 50) * 4;
            assert!(
                pixels[shadow + 3] > 0,
                "drop shadow did not extend beyond the source rect"
            );
        }
    }
}

fn device() -> Option<(wgpu::Device, wgpu::Queue)> {
    let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
    descriptor.backends = wgpu::Backends::VULKAN | wgpu::Backends::GL;
    let instance = wgpu::Instance::new(descriptor);
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::LowPower,
        compatible_surface: None,
        force_fallback_adapter: false,
        ..Default::default()
    }))
    .ok()?;
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("widdgyy-filter-test"),
        required_features: wgpu::Features::empty(),
        required_limits: wgpu::Limits::default(),
        ..Default::default()
    }))
    .ok()?;
    Some((device, queue))
}

fn read_back(device: &wgpu::Device, queue: &wgpu::Queue, texture: &wgpu::Texture) -> Vec<u8> {
    let padded_row = (WIDTH * 4).div_ceil(256) * 256;
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("widdgyy filter readback"),
        size: (padded_row * HEIGHT) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded_row),
                rows_per_image: Some(HEIGHT),
            },
        },
        wgpu::Extent3d {
            width: WIDTH,
            height: HEIGHT,
            depth_or_array_layers: 1,
        },
    );
    queue.submit(Some(encoder.finish()));

    let slice = buffer.slice(..);
    let (sender, receiver) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = sender.send(result);
    });
    device.poll(wgpu::PollType::wait_indefinitely()).ok();
    receiver.recv().ok().and_then(Result::ok);
    let data = slice.get_mapped_range().expect("mapping filter readback");
    let mut pixels = Vec::with_capacity((WIDTH * HEIGHT * 4) as usize);
    for row in 0..HEIGHT as usize {
        let start = row * padded_row as usize;
        pixels.extend_from_slice(&data[start..start + (WIDTH * 4) as usize]);
    }
    drop(data);
    buffer.unmap();
    pixels
}
