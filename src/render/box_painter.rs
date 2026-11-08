// SPDX-FileCopyrightText: 2026 Daniel
// SPDX-License-Identifier: GPL-3.0-or-later

//! Background, border, outline and shadow painters.

use vello::Scene;
use vello::kurbo::{Affine, Rect, RoundedRect, RoundedRectRadii, Stroke};
use vello::peniko::{
    BlendMode, ColorStop, Compose, Extend, Gradient, ImageBrushRef, ImageQuality, ImageSampler, Mix,
};

use crate::css::Axis;
use crate::css::properties::background::BackgroundImage;
use crate::css::properties::border::BorderImageRepeat;
use crate::css::properties::{BackgroundRepeat, BackgroundSize, BorderBox, BorderStyle};
use crate::css::values::{Length, ResolveContext};
use crate::scene::Node;

use super::media::MediaStore;

pub fn paint_shadows(node: &Node, transform: &Affine, canvas: &mut Scene) {
    if node.style.effects.box_shadows.is_empty() {
        return;
    }
    let width = node.layout.width;
    let height = node.layout.height;
    let context = node
        .style
        .resolve_context(crate::css::Viewport::default(), width, height);
    let radius = radius(&node.style.border.radius, &context, width, height);

    for shadow in &node.style.effects.box_shadows {
        let offset_x = shadow.offset_x.resolve(&context, Axis::Horizontal);
        let offset_y = shadow.offset_y.resolve(&context, Axis::Vertical);
        let blur = shadow.blur.resolve(&context, Axis::Horizontal).max(0.0);
        let spread = shadow.spread.resolve(&context, Axis::Horizontal);
        let rect = Rect::new(
            offset_x as f64 - spread as f64,
            offset_y as f64 - spread as f64,
            (width + offset_x + spread) as f64,
            (height + offset_y + spread) as f64,
        );
        let std_dev = (blur / 2.0) as f64;
        if shadow.inset {
            let clip =
                RoundedRect::from_rect(Rect::new(0.0, 0.0, width as f64, height as f64), radius);
            canvas.push_clip_layer(vello::peniko::Fill::NonZero, *transform, &clip);
            let thickness = ((blur / 2.0) + spread.abs()).max(1.0) as f64;
            let edges = [
                Rect::new(0.0, 0.0, width as f64, thickness),
                Rect::new(
                    0.0,
                    (height as f64 - thickness).max(0.0),
                    width as f64,
                    height as f64,
                ),
                Rect::new(0.0, 0.0, thickness, height as f64),
                Rect::new(
                    (width as f64 - thickness).max(0.0),
                    0.0,
                    width as f64,
                    height as f64,
                ),
            ];
            for edge in edges {
                if std_dev > 0.01 {
                    canvas.draw_blurred_rounded_rect(*transform, edge, shadow.color, 0.0, std_dev);
                } else {
                    canvas.fill(
                        vello::peniko::Fill::NonZero,
                        *transform,
                        shadow.color,
                        None,
                        &edge,
                    );
                }
            }
            canvas.pop_layer();
            continue;
        }
        if std_dev > 0.01 {
            canvas.draw_blurred_rounded_rect(
                *transform,
                rect,
                node.filter_color(shadow.color),
                (radius + spread as f64).max(0.0),
                std_dev,
            );
        } else {
            let shape = RoundedRect::from_rect(rect, (radius + spread as f64).max(0.0));
            canvas.fill(
                vello::peniko::Fill::NonZero,
                *transform,
                node.filter_color(shadow.color),
                None,
                &shape,
            );
        }
    }
}

/// Paint a `mask-image` gradient as an alpha mask. Gradient colours are
/// converted to white with the stop's alpha so the luminance mask behaves like
/// the CSS alpha mask.
pub fn paint_mask_image(
    node: &Node,
    transform: &Affine,
    canvas: &mut Scene,
    media: &mut MediaStore,
) {
    let width = node.layout.width;
    let height = node.layout.height;
    if width <= 0.0 || height <= 0.0 {
        return;
    }
    let effects = &node.style.effects;
    if effects.masks.is_empty() {
        return;
    }
    let context = node
        .style
        .resolve_context(crate::css::Viewport::default(), width, height);
    let box_rect = Rect::new(0.0, 0.0, width as f64, height as f64);
    // Multiple mask layers use the default `mask-composite: add`, so each
    // layer is accumulated additively inside the luminance mask.
    let additive = effects.masks.len() > 1;
    for mask in &effects.masks {
        if additive {
            canvas.push_layer(
                vello::peniko::Fill::NonZero,
                BlendMode::new(Mix::Normal, Compose::Plus),
                1.0,
                *transform,
                &box_rect,
            );
        }
        paint_single_mask(node, mask, transform, canvas, media, &context);
        if additive {
            canvas.pop_layer();
        }
    }
}

fn paint_single_mask(
    node: &Node,
    mask: &crate::css::computed::MaskLayer,
    transform: &Affine,
    canvas: &mut Scene,
    media: &mut MediaStore,
    context: &ResolveContext,
) {
    let width = node.layout.width;
    let height = node.layout.height;
    let (mask_width, mask_height) = match &mask.image {
        BackgroundImage::Gradient(_) => background_size(&mask.size, width, height, width, height),
        BackgroundImage::Url(url) => {
            let Some(loaded) = media.image_for(std::path::Path::new(url)) else {
                return;
            };
            background_size(
                &mask.size,
                loaded.width as f32,
                loaded.height as f32,
                width,
                height,
            )
        }
    };
    if mask_width <= 0.0 || mask_height <= 0.0 {
        return;
    }
    let free_x = (width - mask_width).max(0.0);
    let free_y = (height - mask_height).max(0.0);
    let position_context = ResolveContext {
        basis_width: free_x,
        basis_height: free_y,
        ..*context
    };
    let origin_x = mask.position.x.resolve(&position_context, Axis::Horizontal);
    let origin_y = mask.position.y.resolve(&position_context, Axis::Vertical);

    match &mask.image {
        BackgroundImage::Gradient(gradient) => {
            let stops = gradient
                .stops
                .iter()
                .map(|stop| crate::css::values::gradient::ColorStop {
                    color: vello::peniko::Color::new([1.0, 1.0, 1.0, stop.color.components[3]]),
                    position: stop.position,
                })
                .collect();
            let mask_gradient = crate::css::values::Gradient {
                kind: gradient.kind.clone(),
                stops,
            };
            if let Some(brush) = gradient_brush(&mask_gradient, mask_width, mask_height, context) {
                let local = Rect::new(0.0, 0.0, mask_width as f64, mask_height as f64);
                let repeat = mask.repeat;
                let repeat_x = matches!(
                    repeat,
                    BackgroundRepeat::Repeat
                        | BackgroundRepeat::RepeatX
                        | BackgroundRepeat::Space
                        | BackgroundRepeat::Round
                );
                let repeat_y = matches!(
                    repeat,
                    BackgroundRepeat::Repeat
                        | BackgroundRepeat::RepeatY
                        | BackgroundRepeat::Space
                        | BackgroundRepeat::Round
                );
                let mut y = origin_y;
                let mut rows = 0;
                while y < height && rows < 256 {
                    let mut x = origin_x;
                    let mut columns = 0;
                    while x < width && columns < 256 {
                        canvas.fill(
                            vello::peniko::Fill::NonZero,
                            *transform * Affine::translate((x as f64, y as f64)),
                            &brush,
                            None,
                            &local,
                        );
                        if !repeat_x {
                            break;
                        }
                        x += mask_width;
                        columns += 1;
                    }
                    if !repeat_y {
                        break;
                    }
                    y += mask_height;
                    rows += 1;
                }
            }
        }
        BackgroundImage::Url(url) => {
            let path = std::path::Path::new(url);
            let loaded = match mask.mode {
                crate::css::properties::MaskMode::Luminance => media.image_for(path),
                // `match-source` resolves to alpha for CSS images.
                _ => media.alpha_mask_image_for(path),
            };
            let Some(loaded) = loaded else {
                return;
            };
            let sampler = ImageSampler::default().with_quality(ImageQuality::Medium);
            let brush = ImageBrushRef {
                image: &loaded,
                sampler,
            };
            draw_tiles(
                canvas,
                transform,
                &brush,
                (origin_x, origin_y),
                (mask_width, mask_height),
                (width, height),
                mask.repeat,
            );
        }
    }
}

pub fn paint_background(
    node: &Node,
    transform: &Affine,
    canvas: &mut Scene,
    media: &mut MediaStore,
) {
    let width = node.layout.width;
    let height = node.layout.height;
    let context = node
        .style
        .resolve_context(crate::css::Viewport::default(), width, height);
    let clip_rect = clip_rect(node, width, height, &context);
    let radii = radius(&node.style.border.radius, &context, width, height);
    let shape = RoundedRect::from_rect(clip_rect, radii);

    let background = &node.style.background;
    let origin_rect = background_origin_rect(node, width, height, &context);
    let (origin_x, origin_y) = (origin_rect.x0 as f32, origin_rect.y0 as f32);
    let (origin_width, origin_height) = (origin_rect.width() as f32, origin_rect.height() as f32);
    let background_color = node.background_color();
    if background_color.components[3] > 0.0 {
        canvas.fill(
            vello::peniko::Fill::NonZero,
            *transform,
            background_color,
            None,
            &shape,
        );
    }

    if background.images.is_empty() {
        return;
    }

    canvas.push_clip_layer(vello::peniko::Fill::NonZero, *transform, &shape);

    for (index, image) in background.images.iter().enumerate() {
        match image {
            BackgroundImage::Gradient(gradient) => {
                if let Some(brush) = gradient_brush(gradient, width, height, &context) {
                    let layer_shape = Rect::new(0.0, 0.0, width as f64, height as f64);
                    let blend = match background.blend_mode {
                        Mix::Normal => None,
                        mix => Some(mix),
                    };
                    if let Some(mix) = blend {
                        canvas.push_layer(
                            vello::peniko::Fill::NonZero,
                            mix,
                            1.0,
                            *transform,
                            &layer_shape,
                        );
                    }
                    canvas.fill(
                        vello::peniko::Fill::NonZero,
                        *transform,
                        &brush,
                        None,
                        &layer_shape,
                    );
                    if blend.is_some() {
                        canvas.pop_layer();
                    }
                }
            }
            BackgroundImage::Url(url) => {
                let path = std::path::PathBuf::from(url);
                let Some(image) = media.image_for(&path) else {
                    continue;
                };
                let size = background
                    .sizes
                    .get(index)
                    .or_else(|| background.sizes.first())
                    .cloned()
                    .unwrap_or(BackgroundSize::Auto);
                let position = background
                    .positions
                    .get(index)
                    .or_else(|| background.positions.first())
                    .cloned()
                    .unwrap_or_default();
                let repeat = background
                    .repeats
                    .get(index)
                    .or_else(|| background.repeats.first())
                    .copied()
                    .unwrap_or(BackgroundRepeat::Repeat);
                let (draw_width, draw_height) = background_size(
                    &size,
                    image.width as f32,
                    image.height as f32,
                    origin_width,
                    origin_height,
                );
                let free_x = (origin_width - draw_width).max(0.0);
                let free_y = (origin_height - draw_height).max(0.0);
                let position_context = ResolveContext {
                    basis_width: free_x,
                    basis_height: free_y,
                    ..context
                };
                let x = origin_x + position.x.resolve(&position_context, Axis::Horizontal);
                let y = origin_y + position.y.resolve(&position_context, Axis::Vertical);
                let sampler = ImageSampler::default().with_quality(ImageQuality::Medium);
                let brush = ImageBrushRef {
                    image: &image,
                    sampler,
                };
                draw_tiles(
                    canvas,
                    transform,
                    &brush,
                    (x, y),
                    (draw_width, draw_height),
                    (origin_x + origin_width, origin_y + origin_height),
                    repeat,
                );
            }
        }
    }
    canvas.pop_layer();
}

fn resolve_slice(length: &Length, basis: f32) -> f32 {
    match length {
        Length::Percent(value) => basis * value / 100.0,
        Length::Px(value) => *value,
        _ => 0.0,
    }
}

fn paint_border_image(node: &Node, transform: &Affine, canvas: &mut Scene, media: &mut MediaStore) {
    let Some(spec) = &node.style.border.image else {
        return;
    };
    let path = std::path::PathBuf::from(&spec.source);
    let Some(image) = media.image_for(&path) else {
        return;
    };
    let width = node.layout.width as f64;
    let height = node.layout.height as f64;
    if width <= 0.0 || height <= 0.0 {
        return;
    }
    let image_width = image.width as f32;
    let image_height = image.height as f32;
    let slice_left = resolve_slice(&spec.slice.left, image_width).clamp(0.0, image_width) as f64;
    let slice_right = resolve_slice(&spec.slice.right, image_width).clamp(0.0, image_width) as f64;
    let slice_top = resolve_slice(&spec.slice.top, image_height).clamp(0.0, image_height) as f64;
    let slice_bottom =
        resolve_slice(&spec.slice.bottom, image_height).clamp(0.0, image_height) as f64;
    let dest_left =
        resolve_slice(&spec.width.left, width as f32).clamp(0.0, width as f32 / 2.0) as f64;
    let dest_right =
        resolve_slice(&spec.width.right, width as f32).clamp(0.0, width as f32 / 2.0) as f64;
    let dest_top =
        resolve_slice(&spec.width.top, height as f32).clamp(0.0, height as f32 / 2.0) as f64;
    let dest_bottom =
        resolve_slice(&spec.width.bottom, height as f32).clamp(0.0, height as f32 / 2.0) as f64;
    let source_x = [
        0.0,
        slice_left,
        (image_width as f64 - slice_right).max(slice_left),
        image_width as f64,
    ];
    let source_y = [
        0.0,
        slice_top,
        (image_height as f64 - slice_bottom).max(slice_top),
        image_height as f64,
    ];
    let outset_left = resolve_slice(&spec.outset.left, width as f32).max(0.0) as f64;
    let outset_right = resolve_slice(&spec.outset.right, width as f32).max(0.0) as f64;
    let outset_top = resolve_slice(&spec.outset.top, height as f32).max(0.0) as f64;
    let outset_bottom = resolve_slice(&spec.outset.bottom, height as f32).max(0.0) as f64;
    let dest_x = [
        -outset_left,
        dest_left,
        (width - dest_right).max(dest_left),
        width + outset_right,
    ];
    let dest_y = [
        -outset_top,
        dest_top,
        (height - dest_bottom).max(dest_top),
        height + outset_bottom,
    ];
    let sampler = ImageSampler::default().with_quality(ImageQuality::Medium);
    let brush = ImageBrushRef {
        image: &image,
        sampler,
    };

    // Corners are always stretched to their destination region.
    for row in [0usize, 2] {
        for column in [0usize, 2] {
            let source = Rect::new(
                source_x[column],
                source_y[row],
                source_x[column + 1],
                source_y[row + 1],
            );
            let destination = Rect::new(
                dest_x[column],
                dest_y[row],
                dest_x[column + 1],
                dest_y[row + 1],
            );
            draw_border_tile(canvas, transform, brush, source, destination);
        }
    }

    // Step 1: scale edges to the border image region thickness while keeping
    // their proportions. Zero or infinite factors fall back to no scaling.
    let top_scale = proportional_scale(dest_y[1] - dest_y[0], slice_top);
    let bottom_scale = proportional_scale(dest_y[3] - dest_y[2], slice_bottom);
    let left_scale = proportional_scale(dest_x[1] - dest_x[0], slice_left);
    let right_scale = proportional_scale(dest_x[3] - dest_x[2], slice_right);
    let inner_source_width = source_x[2] - source_x[1];
    let inner_source_height = source_y[2] - source_y[1];

    let top_region = Rect::new(dest_x[1], dest_y[0], dest_x[2], dest_y[1]);
    let bottom_region = Rect::new(dest_x[1], dest_y[2], dest_x[2], dest_y[3]);
    let left_region = Rect::new(dest_x[0], dest_y[1], dest_x[1], dest_y[2]);
    let right_region = Rect::new(dest_x[2], dest_y[1], dest_x[3], dest_y[2]);

    draw_border_cell(
        canvas,
        transform,
        brush,
        Rect::new(source_x[1], source_y[0], source_x[2], source_y[1]),
        top_region,
        inner_source_width * top_scale,
        top_region.height(),
        spec.repeat_x,
        BorderImageRepeat::Stretch,
    );
    draw_border_cell(
        canvas,
        transform,
        brush,
        Rect::new(source_x[1], source_y[2], source_x[2], source_y[3]),
        bottom_region,
        inner_source_width * bottom_scale,
        bottom_region.height(),
        spec.repeat_x,
        BorderImageRepeat::Stretch,
    );
    draw_border_cell(
        canvas,
        transform,
        brush,
        Rect::new(source_x[0], source_y[1], source_x[1], source_y[2]),
        left_region,
        left_region.width(),
        inner_source_height * left_scale,
        BorderImageRepeat::Stretch,
        spec.repeat_y,
    );
    draw_border_cell(
        canvas,
        transform,
        brush,
        Rect::new(source_x[2], source_y[1], source_x[3], source_y[2]),
        right_region,
        right_region.width(),
        inner_source_height * right_scale,
        BorderImageRepeat::Stretch,
        spec.repeat_y,
    );

    if spec.fill {
        // The middle image uses the top/left scale factors, with the bottom
        // and right as fallbacks, as specified.
        let middle_scale_x = safe_factor(top_scale).or_else(|| safe_factor(bottom_scale));
        let middle_scale_y = safe_factor(left_scale).or_else(|| safe_factor(right_scale));
        let middle_width = middle_scale_x
            .map(|scale| inner_source_width * scale)
            .unwrap_or(inner_source_width);
        let middle_height = middle_scale_y
            .map(|scale| inner_source_height * scale)
            .unwrap_or(inner_source_height);
        draw_border_cell(
            canvas,
            transform,
            brush,
            Rect::new(source_x[1], source_y[1], source_x[2], source_y[2]),
            Rect::new(dest_x[1], dest_y[1], dest_x[2], dest_y[2]),
            middle_width,
            middle_height,
            spec.repeat_x,
            spec.repeat_y,
        );
    }
}

fn proportional_scale(destination: f64, source: f64) -> f64 {
    if source > 0.0 && destination.is_finite() && destination > 0.0 {
        destination / source
    } else {
        1.0
    }
}

fn safe_factor(scale: f64) -> Option<f64> {
    (scale.is_finite() && scale > 0.0).then_some(scale)
}

#[allow(clippy::too_many_arguments)]
fn draw_border_cell(
    canvas: &mut Scene,
    transform: &Affine,
    brush: ImageBrushRef<'_>,
    source: Rect,
    region: Rect,
    natural_width: f64,
    natural_height: f64,
    mode_x: BorderImageRepeat,
    mode_y: BorderImageRepeat,
) {
    if source.width() <= 0.0 || source.height() <= 0.0 {
        return;
    }
    if region.width() <= 0.0 || region.height() <= 0.0 {
        return;
    }
    let columns = border_axis_tiles(region.x0, region.width(), natural_width, mode_x);
    let rows = border_axis_tiles(region.y0, region.height(), natural_height, mode_y);
    for &(x, tile_width) in &columns {
        for &(y, tile_height) in &rows {
            draw_border_tile(
                canvas,
                transform,
                brush,
                source,
                Rect::new(x, y, x + tile_width, y + tile_height),
            );
        }
    }
}

/// Tile positions along one axis for `border-image-repeat`.
fn border_axis_tiles(
    region_start: f64,
    region_length: f64,
    natural: f64,
    mode: BorderImageRepeat,
) -> Vec<(f64, f64)> {
    if region_length <= 0.0 {
        return Vec::new();
    }
    let tile = if natural.is_finite() && natural > 0.0 {
        natural
    } else {
        region_length
    };
    const MAX_TILES: i64 = 512;
    match mode {
        BorderImageRepeat::Stretch => vec![(region_start, region_length)],
        BorderImageRepeat::Repeat => {
            // The first tile is centred in the region, then the pattern is
            // tiled in both directions and clipped at the region edges.
            let origin = region_start + (region_length - tile) / 2.0;
            let first = ((region_start - origin) / tile).floor() as i64;
            let last = ((region_start + region_length - origin) / tile).ceil() as i64;
            let mut tiles = Vec::new();
            for index in first..last {
                if tiles.len() as i64 >= MAX_TILES {
                    break;
                }
                tiles.push((origin + index as f64 * tile, tile));
            }
            tiles
        }
        BorderImageRepeat::Round => {
            let count = (region_length / tile).round().max(1.0) as i64;
            let count = count.min(MAX_TILES);
            let size = region_length / count as f64;
            (0..count)
                .map(|index| (region_start + index as f64 * size, size))
                .collect()
        }
        BorderImageRepeat::Space => {
            let count = (region_length / tile).floor() as i64;
            if count <= 0 {
                return Vec::new();
            }
            let count = count.min(MAX_TILES);
            let gap = (region_length - count as f64 * tile) / (count + 1) as f64;
            (0..count)
                .map(|index| {
                    let x = region_start + gap + index as f64 * (tile + gap);
                    (x, tile)
                })
                .collect()
        }
    }
}

fn draw_border_tile(
    canvas: &mut Scene,
    transform: &Affine,
    brush: ImageBrushRef<'_>,
    source: Rect,
    destination: Rect,
) {
    if source.width() <= 0.0
        || source.height() <= 0.0
        || destination.width() <= 0.0
        || destination.height() <= 0.0
    {
        return;
    }
    let scale_x = destination.width() / source.width();
    let scale_y = destination.height() / source.height();
    let mapping = Affine::translate((
        destination.x0 - source.x0 * scale_x,
        destination.y0 - source.y0 * scale_y,
    )) * Affine::scale_non_uniform(scale_x, scale_y);
    canvas.push_clip_layer(vello::peniko::Fill::NonZero, *transform, &destination);
    canvas.draw_image(brush, *transform * mapping);
    canvas.pop_layer();
}

pub fn paint_border(node: &Node, transform: &Affine, canvas: &mut Scene, media: &mut MediaStore) {
    if node.style.border.image.is_some() {
        paint_border_image(node, transform, canvas, media);
        return;
    }
    let width = node.layout.width;
    let height = node.layout.height;
    let context = node
        .style
        .resolve_context(crate::css::Viewport::default(), width, height);
    let widths = node.style.border.widths.resolve(&context, Axis::Horizontal);
    let radius = radius(&node.style.border.radius, &context, width, height);
    let uniform = (widths.top - widths.right).abs() < 0.01
        && (widths.top - widths.bottom).abs() < 0.01
        && (widths.top - widths.left).abs() < 0.01;

    if uniform && widths.top > 0.0 {
        // Browsers require an explicit border style. This keeps containers such
        // as the clock/weather, which only set `border-width`, borderless.
        let style = node.style.border.styles.top;
        if style != BorderStyle::None {
            let inset = widths.top / 2.0;
            let rect = Rect::new(
                inset as f64,
                inset as f64,
                (width - inset).max(inset) as f64,
                (height - inset).max(inset) as f64,
            );
            let shape = RoundedRect::from_rect(rect, (radius - inset as f64).max(0.0));
            let mut stroke = Stroke::new(widths.top as f64);
            match style {
                BorderStyle::Dashed => {
                    stroke =
                        stroke.with_dashes(0.0, [widths.top as f64 * 3.0, widths.top as f64 * 2.0]);
                }
                BorderStyle::Dotted => {
                    stroke = stroke.with_dashes(0.0, [widths.top as f64, widths.top as f64 * 1.6]);
                }
                _ => {}
            }
            canvas.stroke(
                &stroke,
                *transform,
                node.filter_color(node.style.border.colors.top),
                None,
                &shape,
            );
        }
    } else {
        paint_edge(
            canvas,
            transform,
            widths.top,
            node.style.border.styles.top,
            node.filter_color(node.style.border.colors.top),
            Rect::new(0.0, 0.0, width as f64, widths.top as f64),
        );
        paint_edge(
            canvas,
            transform,
            widths.right,
            node.style.border.styles.right,
            node.filter_color(node.style.border.colors.right),
            Rect::new(
                (width - widths.right) as f64,
                0.0,
                width as f64,
                height as f64,
            ),
        );
        paint_edge(
            canvas,
            transform,
            widths.bottom,
            node.style.border.styles.bottom,
            node.filter_color(node.style.border.colors.bottom),
            Rect::new(
                0.0,
                (height - widths.bottom) as f64,
                width as f64,
                height as f64,
            ),
        );
        paint_edge(
            canvas,
            transform,
            widths.left,
            node.style.border.styles.left,
            node.filter_color(node.style.border.colors.left),
            Rect::new(0.0, 0.0, widths.left as f64, height as f64),
        );
    }

    if node
        .style
        .border
        .outline_width
        .resolve(&context, Axis::Horizontal)
        > 0.0
        && node.style.border.outline_style != BorderStyle::None
    {
        let outline_width = node
            .style
            .border
            .outline_width
            .resolve(&context, Axis::Horizontal);
        let offset = node
            .style
            .border
            .outline_offset
            .resolve(&context, Axis::Horizontal);
        let rect = Rect::new(
            -(outline_width / 2.0 + offset) as f64,
            -(outline_width / 2.0 + offset) as f64,
            (width + outline_width / 2.0 + offset) as f64,
            (height + outline_width / 2.0 + offset) as f64,
        );
        let shape = RoundedRect::from_rect(rect, radius + outline_width as f64);
        canvas.stroke(
            &Stroke::new(outline_width as f64),
            *transform,
            node.filter_color(node.style.border.outline_color),
            None,
            &shape,
        );
    }
}

fn paint_edge(
    canvas: &mut Scene,
    transform: &Affine,
    width: f32,
    style: BorderStyle,
    color: vello::peniko::Color,
    rect: Rect,
) {
    if style == BorderStyle::None
        || width <= 0.0
        || color.components[3] <= 0.0
        || rect.width() <= 0.0
        || rect.height() <= 0.0
    {
        return;
    }
    canvas.fill(vello::peniko::Fill::NonZero, *transform, color, None, &rect);
}

pub(crate) fn radius(
    edges: &crate::css::Edges<Length>,
    context: &ResolveContext,
    width: f32,
    height: f32,
) -> f64 {
    let horizontal = ResolveContext {
        basis_width: width,
        basis_height: height,
        ..*context
    };
    let clamp = |value: f32| value.max(0.0) as f64;
    let radii = RoundedRectRadii::new(
        clamp(edges.top.resolve(&horizontal, Axis::Horizontal)),
        clamp(edges.right.resolve(&horizontal, Axis::Vertical)),
        clamp(edges.bottom.resolve(&horizontal, Axis::Horizontal)),
        clamp(edges.left.resolve(&horizontal, Axis::Vertical)),
    );
    radii
        .top_left
        .max(radii.top_right)
        .max(radii.bottom_right)
        .max(radii.bottom_left)
        .min((width.min(height) / 2.0) as f64)
}

fn clip_rect(node: &Node, width: f32, height: f32, context: &ResolveContext) -> Rect {
    let border = node.style.border.widths.resolve(context, Axis::Horizontal);
    match node.style.background.clip {
        BorderBox::BorderBox => Rect::new(0.0, 0.0, width as f64, height as f64),
        BorderBox::PaddingBox => Rect::new(
            border.left as f64,
            border.top as f64,
            (width - border.right) as f64,
            (height - border.bottom) as f64,
        ),
        BorderBox::ContentBox | BorderBox::Text => Rect::new(
            node.layout.content_x as f64,
            node.layout.content_y as f64,
            (node.layout.content_x + node.layout.content_width) as f64,
            (node.layout.content_y + node.layout.content_height) as f64,
        ),
    }
}

fn background_size(
    size: &BackgroundSize,
    image_width: f32,
    image_height: f32,
    box_width: f32,
    box_height: f32,
) -> (f32, f32) {
    match size {
        BackgroundSize::Auto => (image_width, image_height),
        BackgroundSize::Cover => {
            let scale = (box_width / image_width).max(box_height / image_height);
            (image_width * scale, image_height * scale)
        }
        BackgroundSize::Contain => {
            let scale = (box_width / image_width).min(box_height / image_height);
            (image_width * scale, image_height * scale)
        }
        BackgroundSize::Explicit(width, height) => {
            let context = ResolveContext {
                basis_width: box_width,
                basis_height: box_height,
                ..ResolveContext::default()
            };
            match (width.is_auto(), height.is_auto()) {
                (true, true) => (image_width, image_height),
                (false, true) => {
                    let width = width.resolve(&context, Axis::Horizontal);
                    (width, image_height * width / image_width)
                }
                (true, false) => {
                    let height = height.resolve(&context, Axis::Vertical);
                    (image_width * height / image_height, height)
                }
                (false, false) => (
                    width.resolve(&context, Axis::Horizontal),
                    height.resolve(&context, Axis::Vertical),
                ),
            }
        }
    }
}

fn draw_tiles(
    canvas: &mut Scene,
    transform: &Affine,
    brush: &ImageBrushRef<'_>,
    origin: (f32, f32),
    size: (f32, f32),
    box_end: (f32, f32),
    repeat: BackgroundRepeat,
) {
    if size.0 <= 0.0 || size.1 <= 0.0 {
        return;
    }
    let repeat_x = matches!(
        repeat,
        BackgroundRepeat::Repeat
            | BackgroundRepeat::RepeatX
            | BackgroundRepeat::Space
            | BackgroundRepeat::Round
    );
    let repeat_y = matches!(
        repeat,
        BackgroundRepeat::Repeat
            | BackgroundRepeat::RepeatY
            | BackgroundRepeat::Space
            | BackgroundRepeat::Round
    );
    let space = repeat == BackgroundRepeat::Space;
    let round = repeat == BackgroundRepeat::Round;
    let (start_x, step_x, tile_w) = tile_axis(
        repeat_x,
        space,
        round,
        origin.0,
        size.0,
        box_end.0 - origin.0,
    );
    let (start_y, step_y, tile_h) = tile_axis(
        repeat_y,
        space,
        round,
        origin.1,
        size.1,
        box_end.1 - origin.1,
    );
    let images = brush.image;
    let scale_x = tile_w / images.width as f32;
    let scale_y = tile_h / images.height as f32;
    let mut x = start_x;
    let mut columns = 0;
    while x < box_end.0 && columns < 256 {
        let mut y = start_y;
        let mut rows = 0;
        while y < box_end.1 && rows < 256 {
            let tile = *transform
                * Affine::translate((x as f64, y as f64))
                * Affine::scale_non_uniform(scale_x as f64, scale_y as f64);
            canvas.draw_image(*brush, tile);
            if !repeat_y {
                break;
            }
            y += step_y;
            rows += 1;
        }
        if !repeat_x {
            break;
        }
        x += step_x;
        columns += 1;
    }
}

fn tile_axis(
    repeat: bool,
    space: bool,
    round: bool,
    origin: f32,
    size: f32,
    box_size: f32,
) -> (f32, f32, f32) {
    if !repeat {
        return (origin, size, size);
    }
    if space {
        let count = (box_size / size).floor().max(1.0);
        let gap = (box_size - count * size) / count;
        return (origin + gap / 2.0, size + gap, size);
    }
    if round {
        let count = (box_size / size).round().max(1.0);
        let tile = box_size / count;
        return (origin, tile, tile);
    }
    let start = origin - ((origin / size).ceil() * size);
    (start, size, size)
}

fn background_origin_rect(node: &Node, width: f32, height: f32, context: &ResolveContext) -> Rect {
    let border = node.style.border.widths.resolve(context, Axis::Horizontal);
    match node.style.background.origin {
        BorderBox::BorderBox => Rect::new(0.0, 0.0, width as f64, height as f64),
        BorderBox::PaddingBox => Rect::new(
            border.left as f64,
            border.top as f64,
            (width - border.right) as f64,
            (height - border.bottom) as f64,
        ),
        BorderBox::ContentBox | BorderBox::Text => Rect::new(
            node.layout.content_x as f64,
            node.layout.content_y as f64,
            (node.layout.content_x + node.layout.content_width) as f64,
            (node.layout.content_y + node.layout.content_height) as f64,
        ),
    }
}

fn gradient_brush(
    gradient: &crate::css::values::Gradient,
    width: f32,
    height: f32,
    context: &ResolveContext,
) -> Option<Gradient> {
    use crate::css::values::GradientKind;
    let (w, h) = (width.max(1.0), height.max(1.0));
    let center = vello::kurbo::Point::new(w as f64 / 2.0, h as f64 / 2.0);
    let mut brush = match &gradient.kind {
        GradientKind::Linear { angle, repeating } => {
            let radians = angle.0.to_radians();
            let direction = (radians.sin(), -radians.cos());
            let extent = (w * direction.0.abs() + h * direction.1.abs()) as f64 / 2.0;
            let start =
                center - vello::kurbo::Vec2::new(direction.0 as f64, direction.1 as f64) * extent;
            let end =
                center + vello::kurbo::Vec2::new(direction.0 as f64, direction.1 as f64) * extent;
            Gradient::new_linear(start, end).with_extend(if *repeating {
                Extend::Repeat
            } else {
                Extend::Pad
            })
        }
        GradientKind::Radial {
            center_x,
            center_y,
            radius,
            repeating,
        } => {
            let center = vello::kurbo::Point::new(
                center_x.resolve(context, Axis::Horizontal) as f64,
                center_y.resolve(context, Axis::Vertical) as f64,
            );
            let radius = radius.resolve(context, Axis::Horizontal).max(1.0);
            Gradient::new_radial(center, radius).with_extend(if *repeating {
                Extend::Repeat
            } else {
                Extend::Pad
            })
        }
        GradientKind::Conic {
            angle,
            center_x,
            center_y,
        } => {
            let center = vello::kurbo::Point::new(
                center_x.resolve(context, Axis::Horizontal) as f64,
                center_y.resolve(context, Axis::Vertical) as f64,
            );
            let start = angle.0.to_radians();
            Gradient::new_sweep(center, start, start + std::f32::consts::TAU)
        }
    };

    let mut stops: Vec<ColorStop> = gradient
        .stops
        .iter()
        .enumerate()
        .map(|(index, stop)| {
            let offset = stop.position.unwrap_or_else(|| {
                if gradient.stops.len() <= 1 {
                    0.0
                } else {
                    index as f32 / (gradient.stops.len() - 1) as f32
                }
            });
            ColorStop {
                offset: offset.clamp(0.0, 1.0),
                color: vello::peniko::color::DynamicColor::from_alpha_color(stop.color),
            }
        })
        .collect();
    if stops.is_empty() {
        return None;
    }
    // Vello's ramp builder treats the first stop as if it were at 0 and
    // extrapolates past the last one, so pad explicit start/end stops. This is
    // also the CSS rule for the area before the first and after the last stop.
    if stops[0].offset > 0.0 {
        let first = stops[0];
        stops.insert(
            0,
            ColorStop {
                offset: 0.0,
                color: first.color,
            },
        );
    }
    let last = *stops.last().expect("non-empty stops");
    if last.offset < 1.0 {
        stops.push(ColorStop {
            offset: 1.0,
            color: last.color,
        });
    }
    brush.stops = stops.as_slice().into();
    Some(brush)
}

#[cfg(test)]
mod border_tiling_tests {
    use super::border_axis_tiles;
    use crate::css::properties::border::BorderImageRepeat::{Repeat, Round, Space, Stretch};

    fn approx(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-6
    }

    #[test]
    fn stretch_uses_one_tile() {
        let tiles = border_axis_tiles(10.0, 100.0, 30.0, Stretch);
        assert_eq!(tiles.len(), 1);
        assert!(approx(tiles[0].0, 10.0) && approx(tiles[0].1, 100.0));
    }

    #[test]
    fn round_fits_a_whole_number_of_tiles() {
        let tiles = border_axis_tiles(0.0, 100.0, 30.0, Round);
        assert_eq!(tiles.len(), 3);
        assert!(tiles.iter().all(|(_, size)| approx(*size, 100.0 / 3.0)));
        assert!(approx(tiles[2].0 + tiles[2].1, 100.0));
    }

    #[test]
    fn space_distributes_gaps_before_between_and_after() {
        let tiles = border_axis_tiles(0.0, 100.0, 30.0, Space);
        assert_eq!(tiles.len(), 3);
        assert!(approx(tiles[0].0, 2.5));
        assert!(approx(tiles[1].0, 35.0));
        assert!(approx(tiles[2].0, 67.5));
        assert!(approx(tiles[2].0 + tiles[2].1, 97.5));
    }

    #[test]
    fn repeat_centers_the_pattern_and_covers_the_region() {
        let tiles = border_axis_tiles(0.0, 100.0, 30.0, Repeat);
        assert!(tiles.iter().any(|(start, _)| *start < 0.0));
        assert!(tiles.iter().any(|(start, size)| start + size > 100.0));
        assert!(tiles.iter().any(|(start, _)| (start - 5.0).abs() < 1e-6));
    }

    #[test]
    fn space_is_empty_when_no_whole_tile_fits() {
        assert!(border_axis_tiles(0.0, 20.0, 30.0, Space).is_empty());
    }
}

#[cfg(test)]
mod gradient_offset_tests {
    use super::gradient_brush;
    use crate::css::values::{Gradient, ResolveContext};
    use vello::peniko::Color;

    #[test]
    fn brush_keeps_duplicate_stop_offsets() {
        let gradient = Gradient::parse(
            "linear-gradient(to right, #000 50%, transparent 50%)",
            Color::new([0.0, 0.0, 0.0, 0.0]),
        )
        .unwrap();
        let brush = gradient_brush(&gradient, 100.0, 100.0, &ResolveContext::default()).unwrap();
        let offsets: Vec<f32> = brush.stops.iter().map(|stop| stop.offset).collect();
        assert_eq!(offsets, vec![0.0, 0.5, 0.5, 1.0]);
    }
}
