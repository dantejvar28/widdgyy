// SPDX-FileCopyrightText: 2026 Daniel
// SPDX-License-Identifier: GPL-3.0-or-later

//! Background, border, outline and shadow painters.

use vello::Scene;
use vello::kurbo::{Affine, Rect, RoundedRect, RoundedRectRadii, Stroke};
use vello::peniko::{ColorStop, Extend, Gradient, ImageBrushRef, ImageQuality, ImageSampler, Mix};

use crate::css::Axis;
use crate::css::properties::background::BackgroundImage;
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
pub fn paint_mask_image(node: &Node, transform: &Affine, canvas: &mut Scene) {
    let Some(crate::css::properties::background::BackgroundImage::Gradient(gradient)) =
        node.mask_image()
    else {
        return;
    };
    let width = node.layout.width;
    let height = node.layout.height;
    let context = node
        .style
        .resolve_context(crate::css::Viewport::default(), width, height);
    let stops = gradient
        .stops
        .iter()
        .map(|stop| crate::css::values::gradient::ColorStop {
            color: vello::peniko::Color::new([1.0, 1.0, 1.0, stop.color.components[3]]),
            position: stop.position,
        })
        .collect();
    let mask = crate::css::values::Gradient {
        kind: gradient.kind.clone(),
        stops,
    };
    if let Some(brush) = gradient_brush(&mask, width, height, &context) {
        let rect = Rect::new(0.0, 0.0, width as f64, height as f64);
        canvas.fill(
            vello::peniko::Fill::NonZero,
            *transform,
            &brush,
            None,
            &rect,
        );
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

pub fn paint_border(node: &Node, transform: &Affine, canvas: &mut Scene) {
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

    let stops: Vec<ColorStop> = gradient
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
    brush.stops = stops.as_slice().into();
    Some(brush)
}
