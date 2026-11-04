// SPDX-FileCopyrightText: 2026 Daniel
// SPDX-License-Identifier: GPL-3.0-or-later

//! Opacity, transform, blend-mode and clipping effects.

use vello::Scene;
use vello::kurbo::{Affine, BezPath, Ellipse, Point, Rect, RoundedRect, Shape};
use vello::peniko::{BlendMode, Compose, Fill, Mix};

use crate::css::properties::ClipPath;
use crate::css::values::ResolveContext;
use crate::css::{Length, Overflow};
use crate::scene::Node;

#[derive(Default)]
pub struct Layer {
    active: bool,
}

pub fn node_transform(node: &Node, context: &ResolveContext) -> Affine {
    if node.transform().is_empty()
        && node.style.effects.translate.is_none()
        && node.style.effects.rotate.is_none()
        && node.style.effects.scale.is_none()
    {
        return Affine::IDENTITY;
    }
    let width = node.layout.width;
    let height = node.layout.height;
    let (origin_x, origin_y) = node.style.effects.transform_origin.clone();
    let origin_x = origin_x
        .resolve(context, crate::css::Axis::Horizontal)
        .clamp(0.0, width);
    let origin_y = origin_y
        .resolve(context, crate::css::Axis::Vertical)
        .clamp(0.0, height);
    let resolve = |length: &Length| length.resolve(context, crate::css::Axis::Horizontal);
    let mut matrix = Affine::IDENTITY;
    for operation in node.transform() {
        matrix *= operation.to_affine(resolve);
    }
    Affine::translate((origin_x as f64, origin_y as f64))
        * matrix
        * Affine::translate((-(origin_x as f64), -(origin_y as f64)))
}

pub fn begin_effects(
    node: &Node,
    transform: &Affine,
    box_rect: &Rect,
    canvas: &mut Scene,
) -> Layer {
    let opacity = node.opacity().clamp(0.0, 1.0);
    let blend = node.style.effects.mix_blend_mode;
    let clip = clip_shape(node, box_rect);

    let isolate = node.style.effects.isolation;
    if opacity >= 0.999 && blend == Mix::Normal && !isolate && clip.is_none() {
        return Layer { active: false };
    }

    let shape = clip.unwrap_or_else(|| {
        BezPath::from_iter(Rect::new(-1.0e6, -1.0e6, 1.0e6, 1.0e6).path_elements(0.1))
    });
    let clip_style = Fill::NonZero;
    canvas.push_layer(
        clip_style,
        BlendMode::new(blend, Compose::SrcOver),
        opacity,
        *transform,
        &shape,
    );
    Layer { active: true }
}

pub fn end_effects(layer: Layer, canvas: &mut Scene) {
    if layer.active {
        canvas.pop_layer();
    }
}

fn clip_shape(node: &Node, box_rect: &Rect) -> Option<BezPath> {
    let context = node.style.resolve_context(
        crate::css::Viewport::default(),
        node.layout.width,
        node.layout.height,
    );
    let resolve = |length: &Length, axis| length.resolve(&context, axis);

    match &node.style.effects.clip_path {
        ClipPath::None => {}
        ClipPath::Inset(edges, radius) => {
            let left = resolve(&edges.left, crate::css::Axis::Horizontal);
            let right = resolve(&edges.right, crate::css::Axis::Horizontal);
            let top = resolve(&edges.top, crate::css::Axis::Vertical);
            let bottom = resolve(&edges.bottom, crate::css::Axis::Vertical);
            let rect = Rect::new(
                box_rect.x0 + left as f64,
                box_rect.y0 + top as f64,
                (box_rect.x1 - right as f64).max(box_rect.x0),
                (box_rect.y1 - bottom as f64).max(box_rect.y0),
            );
            let radius = radius
                .as_ref()
                .map(|length| resolve(length, crate::css::Axis::Horizontal))
                .unwrap_or(0.0) as f64;
            let shape = RoundedRect::from_rect(rect, radius);
            return Some(BezPath::from_iter(shape.path_elements(0.1)));
        }
        ClipPath::Circle(radius, x, y) => {
            let radius = resolve(radius, crate::css::Axis::Horizontal).max(0.0) as f64;
            let center = Point::new(
                box_rect.x0 + resolve(x, crate::css::Axis::Horizontal) as f64,
                box_rect.y0 + resolve(y, crate::css::Axis::Vertical) as f64,
            );
            return Some(BezPath::from_iter(
                kurbo_circle(center, radius).path_elements(0.1),
            ));
        }
        ClipPath::Ellipse(rx, ry, x, y) => {
            let rx = resolve(rx, crate::css::Axis::Horizontal).max(0.0) as f64;
            let ry = resolve(ry, crate::css::Axis::Vertical).max(0.0) as f64;
            let center = Point::new(
                box_rect.x0 + resolve(x, crate::css::Axis::Horizontal) as f64,
                box_rect.y0 + resolve(y, crate::css::Axis::Vertical) as f64,
            );
            let ellipse = Ellipse::new(center, (rx, ry), 0.0);
            return Some(BezPath::from_iter(ellipse.path_elements(0.1)));
        }
        ClipPath::Polygon(points) => {
            let mut path = BezPath::new();
            for (index, (x, y)) in points.iter().enumerate() {
                let point = Point::new(
                    box_rect.x0 + resolve(x, crate::css::Axis::Horizontal) as f64,
                    box_rect.y0 + resolve(y, crate::css::Axis::Vertical) as f64,
                );
                if index == 0 {
                    path.move_to(point);
                } else {
                    path.line_to(point);
                }
            }
            path.close_path();
            return Some(path);
        }
        ClipPath::Path(data) => {
            if let Some(path) = parse_svg_path(data) {
                return Some(path);
            }
        }
        ClipPath::Url(_) => {
            // External SVG clip paths are not fetched; fall back to no clip.
        }
    }

    let hidden = matches!(
        node.style.overflow_x,
        Overflow::Hidden | Overflow::Clip | Overflow::Scroll | Overflow::Auto
    ) || matches!(
        node.style.overflow_y,
        Overflow::Hidden | Overflow::Clip | Overflow::Scroll | Overflow::Auto
    );
    if hidden {
        let shape = RoundedRect::from_rect(*box_rect, 0.0);
        return Some(BezPath::from_iter(shape.path_elements(0.1)));
    }
    None
}

fn kurbo_circle(center: Point, radius: f64) -> vello::kurbo::Circle {
    vello::kurbo::Circle::new(center, radius.max(0.0))
}

/// Minimal SVG path parser for `clip-path: path(...)`. Supports M/L/H/V/C/Q/Z
/// in absolute and relative forms. Units are treated as pixels.
fn parse_svg_path(data: &str) -> Option<BezPath> {
    use vello::kurbo::Point;

    #[derive(Clone, Copy)]
    enum Command {
        M,
        L,
        H,
        V,
        C,
        Q,
        Z,
    }

    let mut tokens = Vec::new();
    let mut current = String::new();
    for ch in data.chars() {
        if ch.is_ascii_alphabetic() {
            if !current.is_empty() {
                tokens.push(std::mem::take(&mut current));
            }
            tokens.push(ch.to_string());
        } else if ch == ',' || ch.is_whitespace() {
            if !current.is_empty() {
                tokens.push(std::mem::take(&mut current));
            }
        } else {
            current.push(ch);
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }

    let mut path = BezPath::new();
    let mut index = 0;
    let mut command = Command::M;
    let mut relative = false;
    let mut point = Point::ZERO;
    let mut start = Point::ZERO;

    let number = |tokens: &Vec<String>, index: &mut usize| -> Option<f64> {
        let value = tokens.get(*index)?.parse::<f64>().ok()?;
        *index += 1;
        Some(value)
    };

    while index < tokens.len() {
        let token = &tokens[index];
        if token.len() == 1
            && token
                .chars()
                .next()
                .is_some_and(|ch| ch.is_ascii_alphabetic())
        {
            let ch = token.chars().next().unwrap();
            relative = ch.is_ascii_lowercase();
            command = match ch.to_ascii_uppercase() {
                'M' => Command::M,
                'L' => Command::L,
                'H' => Command::H,
                'V' => Command::V,
                'C' => Command::C,
                'Q' => Command::Q,
                'Z' => Command::Z,
                _ => return None,
            };
            index += 1;
            if matches!(command, Command::Z) {
                path.close_path();
                point = start;
                continue;
            }
        }

        let base = if relative { point } else { Point::ZERO };
        match command {
            Command::M => {
                let x = number(&tokens, &mut index)?;
                let y = number(&tokens, &mut index)?;
                point = Point::new(base.x + x, base.y + y);
                start = point;
                path.move_to(point);
                command = Command::L;
            }
            Command::L => {
                let x = number(&tokens, &mut index)?;
                let y = number(&tokens, &mut index)?;
                point = Point::new(base.x + x, base.y + y);
                path.line_to(point);
            }
            Command::H => {
                let x = number(&tokens, &mut index)?;
                point = Point::new(base.x + x, point.y);
                path.line_to(point);
            }
            Command::V => {
                let y = number(&tokens, &mut index)?;
                point = Point::new(point.x, base.y + y);
                path.line_to(point);
            }
            Command::C => {
                let x1 = number(&tokens, &mut index)?;
                let y1 = number(&tokens, &mut index)?;
                let x2 = number(&tokens, &mut index)?;
                let y2 = number(&tokens, &mut index)?;
                let x = number(&tokens, &mut index)?;
                let y = number(&tokens, &mut index)?;
                let c1 = Point::new(base.x + x1, base.y + y1);
                let c2 = Point::new(base.x + x2, base.y + y2);
                point = Point::new(base.x + x, base.y + y);
                path.curve_to(c1, c2, point);
            }
            Command::Q => {
                let x1 = number(&tokens, &mut index)?;
                let y1 = number(&tokens, &mut index)?;
                let x = number(&tokens, &mut index)?;
                let y = number(&tokens, &mut index)?;
                let control = Point::new(base.x + x1, base.y + y1);
                point = Point::new(base.x + x, base.y + y);
                path.quad_to(control, point);
            }
            Command::Z => {}
        }
    }
    Some(path)
}
