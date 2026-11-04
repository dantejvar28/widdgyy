// SPDX-FileCopyrightText: 2026 Daniel
// SPDX-License-Identifier: GPL-3.0-or-later

//! Offscreen GPU smoke test.
//!
//! It builds a small scene (background gradient, rounded box, border, shadow and
//! shaped text), paints it with Vello and reads the pixels back. If no GPU
//! adapter is available the test is skipped instead of failing CI.

use widdgyy::css::{Stylesheet, Viewport};
use widdgyy::layout::LayoutEngine;
use widdgyy::render::{MediaStore, Painter};
use widdgyy::scene::{Anchor, Node, Scene};
use widdgyy::text::TextEngine;

const WIDTH: u32 = 640;
const HEIGHT: u32 = 360;

#[test]
fn paints_scene_to_texture() {
    let css = Stylesheet::parse(
        r#"
        .root {
            display: flex;
            flex-direction: column;
            width: 100%;
            height: 100%;
            background: linear-gradient(135deg, #1a0b2e, #4a1d6e 55%, #ff6b9d);
            padding: 40px;
            gap: 18px;
            justify-content: center;
            align-items: center;
        }
        .card {
            display: flex;
            flex-direction: column;
            gap: 6px;
            background-color: rgba(10, 10, 18, 0.55);
            border-radius: 28px;
            border: 2px solid rgba(255, 255, 255, 0.25);
            box-shadow: 0 18px 40px rgba(0, 0, 0, 0.55);
            padding: 26px 34px;
        }
        .title {
            font-family: Sans;
            font-size: 42px;
            font-weight: bold;
            color: #f7f2ff;
            text-shadow: 0 3px 12px rgba(0, 0, 0, 0.8);
        }
        .subtitle {
            font-size: 18px;
            color: #c7a7ff;
            letter-spacing: 1px;
        }
        "#,
    );

    let mut scene = Scene::new(Viewport {
        width: WIDTH as f32,
        height: HEIGHT as f32,
        scale_factor: 1.0,
    });
    let mut root = Node::container("vbox", vec!["root".to_string()]);
    root.anchor = Anchor::TopLeft;
    let mut card = Node::container("vbox", vec!["card".to_string()]);
    card.anchor = Anchor::Center;
    card.children
        .push(Node::text("text", vec!["title".to_string()], "Widdgyy"));
    card.children.push(Node::text(
        "text",
        vec!["subtitle".to_string()],
        "Rust + Vello + CSS",
    ));
    root.children.push(card);
    scene.roots.push(root);

    let mut text = TextEngine::new();
    scene.resolve_styles(&css);
    scene.layout(&mut LayoutEngine::new(&mut text));
    assert!(scene.roots[0].layout.width > 0.0);

    let mut media = MediaStore::new();
    let mut canvas = vello::Scene::new();
    Painter::new(&mut text, &mut media).paint(&scene, &mut canvas);

    let Some(pixels) = render(&canvas) else {
        eprintln!("no GPU adapter available; skipping offscreen render assertion");
        return;
    };

    let mut opaque = 0usize;
    let mut bright = 0usize;
    for pixel in pixels.as_chunks::<4>().0 {
        if pixel[3] > 0 {
            opaque += 1;
        }
        if pixel[0] as u32 + pixel[1] as u32 + pixel[2] as u32 > 180 {
            bright += 1;
        }
    }
    assert!(
        opaque > (WIDTH * HEIGHT / 2) as usize,
        "scene was not painted"
    );
    assert!(
        bright > 1000,
        "expected visible text/details, got {bright} bright pixels"
    );

    let _ = image::save_buffer(
        "/tmp/widdgyy-offscreen.png",
        &pixels,
        WIDTH,
        HEIGHT,
        image::ColorType::Rgba8,
    );
}

fn render(scene: &vello::Scene) -> Option<Vec<u8>> {
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
        label: Some("widdgyy-offscreen"),
        required_features: wgpu::Features::empty(),
        required_limits: wgpu::Limits::default(),
        ..Default::default()
    }))
    .ok()?;

    let mut renderer = vello::Renderer::new(
        &device,
        vello::RendererOptions {
            use_cpu: false,
            antialiasing_support: vello::AaSupport::area_only(),
            num_init_threads: None,
            pipeline_cache: None,
        },
    )
    .ok()?;

    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("target"),
        size: wgpu::Extent3d {
            width: WIDTH,
            height: HEIGHT,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::STORAGE_BINDING
            | wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    renderer
        .render_to_texture(
            &device,
            &queue,
            scene,
            &view,
            &vello::RenderParams {
                base_color: vello::peniko::Color::new([0.0, 0.0, 0.0, 0.0]),
                width: WIDTH,
                height: HEIGHT,
                antialiasing_method: vello::AaConfig::Area,
            },
        )
        .ok()?;

    let padded_row = (WIDTH * 4).div_ceil(256) * 256;
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("readback"),
        size: (padded_row * HEIGHT) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("readback"),
    });
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
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
    device.poll(wgpu::PollType::wait_indefinitely()).ok()?;
    receiver.recv().ok()?.ok()?;

    let data = slice.get_mapped_range().ok()?;
    let mut pixels = Vec::with_capacity((WIDTH * HEIGHT * 4) as usize);
    for row in 0..HEIGHT as usize {
        let start = row * padded_row as usize;
        pixels.extend_from_slice(&data[start..start + (WIDTH * 4) as usize]);
    }
    drop(data);
    buffer.unmap();
    Some(pixels)
}
