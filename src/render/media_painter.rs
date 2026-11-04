// SPDX-FileCopyrightText: 2026 Daniel
// SPDX-License-Identifier: GPL-3.0-or-later

//! Image/GIF/video painter.

use vello::Scene;
use vello::kurbo::Affine;
use vello::peniko::{ImageBrushRef, ImageQuality, ImageSampler};

use crate::scene::{MediaContent, MediaKind, Node};

use super::media::MediaStore;

pub fn paint_media(
    node: &Node,
    media: &MediaContent,
    transform: &Affine,
    canvas: &mut Scene,
    store: &mut MediaStore,
) {
    let Some(image) = store.image(media) else {
        if media.kind == MediaKind::Video {
            // Video needs the GStreamer backend; draw a subtle placeholder so
            // the layout is still visible instead of silently disappearing.
            let rect = vello::kurbo::Rect::new(
                node.layout.content_x as f64,
                node.layout.content_y as f64,
                (node.layout.content_x + node.layout.content_width) as f64,
                (node.layout.content_y + node.layout.content_height) as f64,
            );
            canvas.fill(
                vello::peniko::Fill::NonZero,
                *transform,
                vello::peniko::Color::new([0.05, 0.05, 0.05, 0.6]),
                None,
                &rect,
            );
        }
        return;
    };

    let image_width = image.width as f32;
    let image_height = image.height as f32;
    if image_width <= 0.0 || image_height <= 0.0 {
        return;
    }
    let draw_width = node.layout.draw_width.max(1.0);
    let draw_height = node.layout.draw_height.max(1.0);
    let free_x = (node.layout.content_width - draw_width).max(0.0);
    let free_y = (node.layout.content_height - draw_height).max(0.0);
    let position = &node.style.effects.object_position;
    let context = crate::css::values::ResolveContext {
        basis_width: free_x,
        basis_height: free_y,
        ..node.style.resolve_context(
            crate::css::Viewport::default(),
            node.layout.width,
            node.layout.height,
        )
    };
    let x = node.layout.content_x
        + position
            .x
            .resolve(&context, crate::css::Axis::Horizontal)
            .clamp(0.0, free_x);
    let y = node.layout.content_y
        + position
            .y
            .resolve(&context, crate::css::Axis::Vertical)
            .clamp(0.0, free_y);
    let scale_x = draw_width / image_width;
    let scale_y = draw_height / image_height;
    let brush = ImageBrushRef {
        image: &image,
        sampler: ImageSampler::default().with_quality(ImageQuality::Medium),
    };
    let tile = *transform
        * Affine::translate((x as f64, y as f64))
        * Affine::scale_non_uniform(scale_x as f64, scale_y as f64);
    canvas.draw_image(brush, tile);
}
