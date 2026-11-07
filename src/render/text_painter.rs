// SPDX-FileCopyrightText: 2026 Daniel
// SPDX-License-Identifier: GPL-3.0-or-later

//! Text and decoration painter.

use vello::Scene;
use vello::kurbo::{Affine, Line, Stroke};
use vello::peniko::Fill;

use crate::css::properties::{BorderStyle, TextAlign};
use crate::css::{Axis, Size};
use crate::scene::Node;
use crate::text::{ShapeRequest, TextEngine};

pub fn paint_text(
    node: &Node,
    transform: &Affine,
    canvas: &mut Scene,
    engine: &mut TextEngine,
    inline_shadows: bool,
) {
    let NodeKindText::Text(content) = NodeKindText::from(&node.kind) else {
        return;
    };
    let Some(shaped) = shape(node, engine, &content.text) else {
        return;
    };

    let content_x = node.layout.content_x;
    let content_y = node.layout.content_y;
    let origin = *transform * Affine::translate((content_x as f64, content_y as f64));
    let color = node.filter_color(node.text_color());

    if inline_shadows {
        paint_text_shadows(node, &shaped, &origin, canvas);
    }
    draw_shaped(canvas, &shaped, &origin, color);
    paint_decoration(node, &shaped, &origin, canvas);
}

/// Draw the glyphs as opaque white coverage for the shadow mask.
pub fn paint_text_mask(
    node: &Node,
    transform: &Affine,
    canvas: &mut Scene,
    engine: &mut TextEngine,
) {
    let NodeKindText::Text(content) = NodeKindText::from(&node.kind) else {
        return;
    };
    let Some(shaped) = shape(node, engine, &content.text) else {
        return;
    };
    let origin = *transform
        * Affine::translate((node.layout.content_x as f64, node.layout.content_y as f64));
    draw_shaped(
        canvas,
        &shaped,
        &origin,
        vello::peniko::Color::new([1.0, 1.0, 1.0, 1.0]),
    );
}

/// Approximate a Gaussian text shadow by sampling a small disk around the
/// shadow offset. Vello has no arbitrary glyph blur, so a handful of low-alpha
/// glyph passes at increasing radii produces a much softer result than a single
/// offset copy. The number of passes is capped to keep animated scenes cheap.
fn paint_text_shadows(
    node: &Node,
    shaped: &crate::text::ShapedText,
    origin: &Affine,
    canvas: &mut Scene,
) {
    let context = node.style.resolve_context(
        crate::css::Viewport::default(),
        node.layout.width,
        node.layout.height,
    );
    for shadow in node.text_shadows() {
        let dx = shadow.offset_x.resolve(&context, Axis::Horizontal);
        let dy = shadow.offset_y.resolve(&context, Axis::Vertical);
        let blur = shadow.blur.resolve(&context, Axis::Horizontal).max(0.0);
        let base = shadow.color.components;

        if blur <= 0.5 {
            let offset = Affine::translate((dx as f64, dy as f64));
            draw_shaped(canvas, shaped, &(*origin * offset), shadow.color);
            continue;
        }

        // Sample the disk with a golden-angle spiral instead of concentric
        // rings. Equal-area samples avoid visible duplicated glyph contours and
        // read as a soft glow while keeping the draw count bounded.
        let samples = 16usize;
        // Glyphs are vector shapes, not a raster alpha mask. Keep the sampled
        // halo close to the outline so it brightens the edge instead of making
        // readable offset copies of the whole word.
        let alpha = base[3] * 0.28 / samples as f32;
        let radius = (blur * 0.15).clamp(0.5, 1.5);
        for sample in 0..samples {
            let angle = std::f32::consts::TAU * sample as f32 / samples as f32;
            let offset = Affine::translate((
                (dx + angle.cos() * radius) as f64,
                (dy + angle.sin() * radius) as f64,
            ));
            draw_shaped(
                canvas,
                shaped,
                &(*origin * offset),
                vello::peniko::Color::new([base[0], base[1], base[2], alpha]),
            );
        }
    }
}

enum NodeKindText<'a> {
    Text(&'a crate::scene::TextContent),
    Other,
}

impl<'a> From<&'a crate::scene::NodeKind> for NodeKindText<'a> {
    fn from(kind: &'a crate::scene::NodeKind) -> Self {
        match kind {
            crate::scene::NodeKind::Text(content) => Self::Text(content),
            _ => Self::Other,
        }
    }
}

fn shape(
    node: &Node,
    engine: &mut TextEngine,
    text: &str,
) -> Option<std::sync::Arc<crate::text::ShapedText>> {
    let max_width = match &node.style.width {
        Size::Length(length) => Some(length.resolve(
            &node.style.resolve_context(
                crate::css::Viewport::default(),
                node.layout.width,
                node.layout.height,
            ),
            Axis::Horizontal,
        )),
        _ => node.style.max_width.resolve(
            &node.style.resolve_context(
                crate::css::Viewport::default(),
                node.layout.width,
                node.layout.height,
            ),
            Axis::Horizontal,
        ),
    };
    let request = ShapeRequest {
        text,
        font_size: node.style.text.font_size_px,
        line_height: node.style.text.line_height_px,
        families: &node.style.text.families,
        weight: node.style.text.weight,
        style: node.style.text.style,
        letter_spacing: node.style.text.letter_spacing_px,
        word_spacing: node.style.text.word_spacing_px,
        max_width,
        align: node.style.text.align,
        white_space: node.style.text.white_space,
        overflow_wrap: node.style.text.overflow_wrap,
        word_break: node.style.text.word_break,
        transform: node.style.text.transform,
        text_overflow: node.style.text.overflow,
    };
    Some(engine.shape(request))
}

fn draw_shaped(
    canvas: &mut Scene,
    shaped: &crate::text::ShapedText,
    transform: &Affine,
    color: vello::peniko::Color,
) {
    for run in &shaped.runs {
        canvas
            .draw_glyphs(&run.font)
            .font_size(run.font_size)
            .brush(color)
            .transform(*transform * Affine::translate((0.0, run.baseline as f64)))
            .draw(Fill::NonZero, run.glyphs.iter().copied());
    }
}

fn paint_decoration(
    node: &Node,
    shaped: &crate::text::ShapedText,
    origin: &Affine,
    canvas: &mut Scene,
) {
    let decoration = &node.style.text.decoration;
    if !decoration.underline && !decoration.overline && !decoration.line_through {
        return;
    }
    let context = node.style.resolve_context(
        crate::css::Viewport::default(),
        node.layout.width,
        node.layout.height,
    );
    let thickness = decoration
        .thickness
        .as_ref()
        .map(|length| length.resolve(&context, Axis::Vertical))
        .unwrap_or_else(|| (node.style.text.font_size_px / 14.0).max(1.0));
    let color = node.filter_color(decoration.color.unwrap_or(node.text_color()));
    let stroke = match decoration.style {
        BorderStyle::Dashed => Stroke::new(thickness as f64)
            .with_dashes(0.0, [thickness as f64 * 3.0, thickness as f64 * 2.0]),
        BorderStyle::Dotted => Stroke::new(thickness as f64)
            .with_dashes(0.0, [thickness as f64, thickness as f64 * 1.6]),
        _ => Stroke::new(thickness as f64),
    };

    for line in &shaped.lines {
        let y = match node.style.text.align {
            TextAlign::Left | TextAlign::Start => line.baseline,
            _ => line.baseline,
        };
        let width = line.width;
        if decoration.underline {
            let y = y + node.style.text.font_size_px * 0.12;
            canvas.stroke(
                &stroke,
                *origin,
                color,
                None,
                &Line::new((0.0, y as f64), (width as f64, y as f64)),
            );
        }
        if decoration.overline {
            let y = line.top;
            canvas.stroke(
                &stroke,
                *origin,
                color,
                None,
                &Line::new((0.0, y as f64), (width as f64, y as f64)),
            );
        }
        if decoration.line_through {
            let y = line.baseline - node.style.text.font_size_px * 0.3;
            canvas.stroke(
                &stroke,
                *origin,
                color,
                None,
                &Line::new((0.0, y as f64), (width as f64, y as f64)),
            );
        }
    }
}
