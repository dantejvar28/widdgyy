// SPDX-FileCopyrightText: 2026 Daniel
// SPDX-License-Identifier: GPL-3.0-or-later

//! Measures the offscreen Gaussian against the exact blur of a rectangle.
//!
//! Run with `cargo test --test blur_quality -- --nocapture`. Skips when no GPU
//! adapter is available. Set `WIDDGYY_BLUR_DUMP=<dir>` to write the alpha
//! planes for offline inspection.

use widdgyy::render::filter::{FilterCompositor, SpatialFilter};

const SIZE: u32 = 192;
const RECT: (f64, f64, f64, f64) = (72.0, 72.0, 120.0, 120.0);

#[test]
fn blur_matches_exact_gaussian() {
    let Some((device, queue)) = device() else {
        eprintln!("no GPU adapter available; skipping blur quality measurement");
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
        vello::peniko::Color::new([1.0, 1.0, 1.0, 1.0]),
        None,
        &vello::kurbo::Rect::new(RECT.0, RECT.1, RECT.2, RECT.3),
    );

    let mut filters = FilterCompositor::new(&device, wgpu::TextureFormat::Rgba8Unorm, SIZE, SIZE);
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("blur quality target"),
        size: wgpu::Extent3d {
            width: SIZE,
            height: SIZE,
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
    let dump = std::env::var("WIDDGYY_BLUR_DUMP").ok();

    for radius in [3.0_f32, 6.0, 9.0, 15.0] {
        for iterations in [1_u32] {
            renderer
                .render_to_texture(
                    &device,
                    &queue,
                    &scene,
                    filters.layer_view(),
                    &vello::RenderParams {
                        base_color: vello::peniko::Color::new([0.0, 0.0, 0.0, 0.0]),
                        width: SIZE,
                        height: SIZE,
                        antialiasing_method: vello::AaConfig::Area,
                    },
                )
                .expect("rendering rectangle");
            let mut encoder =
                device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
            filters.encode_layer(
                &queue,
                &mut encoder,
                &target_view,
                Some(SpatialFilter::Blur { radius }),
                true,
            );
            queue.submit(Some(encoder.finish()));
            let pixels = read_back(&device, &queue, &target);

            let sigma = radius as f64;
            let mut sum_sq = 0.0;
            let mut max_err: f64 = 0.0;
            let mut count = 0.0;
            let mut samples = Vec::with_capacity((SIZE * SIZE) as usize);
            for y in 0..SIZE as usize {
                for x in 0..SIZE as usize {
                    let ideal = gaussian_rect(x as f64 + 0.5, y as f64 + 0.5, sigma);
                    let got = pixels[(y * SIZE as usize + x) * 4 + 3] as f64 / 255.0;
                    samples.push(got);
                    // Ignore the shell where the kernel itself is truncated.
                    if ideal > 0.002 || got > 0.002 {
                        let err = got - ideal;
                        sum_sq += err * err;
                        max_err = max_err.max(err.abs());
                        count += 1.0;
                    }
                }
            }
            let rmse = (sum_sq / count).sqrt();
            println!(
                "radius={radius:>5.1} sigma={sigma:>5.1} iterations={iterations} rmse={rmse:.4} max_err={max_err:.3}"
            );
            if let Some(dir) = &dump {
                let text: String = samples
                    .iter()
                    .map(|value| format!("{value:.5}\n"))
                    .collect();
                let _ = std::fs::write(format!("{dir}/blur_r{radius}_i{iterations}.txt"), text);
            }
        }
    }
}

/// Exact Gaussian blur of the test rectangle, sigma in pixels.
fn gaussian_rect(x: f64, y: f64, sigma: f64) -> f64 {
    let (x0, y0, x1, y1) = RECT;
    let cdf = |v: f64| 0.5 * (1.0 + erf(v / (sigma * std::f64::consts::SQRT_2)));
    let fx = cdf(x - x0) - cdf(x - x1);
    let fy = cdf(y - y0) - cdf(y - y1);
    fx * fy
}

/// Abramowitz-Stegun 7.1.26 approximation, accurate to ~1.5e-7.
fn erf(x: f64) -> f64 {
    let sign = if x < 0.0 { -1.0 } else { 1.0 };
    let x = x.abs();
    let t = 1.0 / (1.0 + 0.3275911 * x);
    let poly = t
        * (0.254829592
            + t * (-0.284496736 + t * (1.421413741 + t * (-1.453152027 + t * 1.061405429))));
    sign * (1.0 - poly * (-x * x).exp())
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
        label: Some("widdgyy-blur-quality"),
        required_features: wgpu::Features::empty(),
        required_limits: wgpu::Limits::default(),
        ..Default::default()
    }))
    .ok()?;
    Some((device, queue))
}

fn read_back(device: &wgpu::Device, queue: &wgpu::Queue, texture: &wgpu::Texture) -> Vec<u8> {
    let padded_row = (SIZE * 4).div_ceil(256) * 256;
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("blur quality readback"),
        size: (padded_row * SIZE) as u64,
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
                rows_per_image: Some(SIZE),
            },
        },
        wgpu::Extent3d {
            width: SIZE,
            height: SIZE,
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
    let data = slice.get_mapped_range().expect("mapping readback");
    let mut pixels = Vec::with_capacity((SIZE * SIZE * 4) as usize);
    for row in 0..SIZE as usize {
        let start = row * padded_row as usize;
        pixels.extend_from_slice(&data[start..start + (SIZE * 4) as usize]);
    }
    drop(data);
    buffer.unmap();
    pixels
}
