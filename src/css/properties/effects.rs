// SPDX-FileCopyrightText: 2026 Daniel
// SPDX-License-Identifier: GPL-3.0-or-later

//! Visual effects: opacity, shadows, filters, transforms, clipping.

use super::background::parse_blend;
use super::media_object_fit::ObjectFit;
use super::{BackgroundPosition, ClipPath, Declarations, Filter, split_ws};
use crate::css::values::{Angle, Color, Edges, Length, Shadow, TransformOp};

pub fn apply(decl: &mut Declarations, name: &str, value: &str, current: Color) -> bool {
    match name {
        "opacity" => {
            decl.opacity = parse_opacity(value);
            true
        }
        "box-shadow" => {
            if value.eq_ignore_ascii_case("none") {
                decl.box_shadow = Some(Vec::new());
            } else {
                decl.box_shadow = Shadow::parse_list(value, current).ok();
            }
            true
        }
        "filter" => {
            decl.filter = Some(parse_filter_list(value, current));
            true
        }
        "backdrop-filter" | "-webkit-backdrop-filter" => {
            decl.backdrop_filter = Some(parse_filter_list(value, current));
            true
        }
        "mix-blend-mode" => {
            decl.mix_blend_mode = parse_blend(value);
            true
        }
        "transform" => {
            if value.eq_ignore_ascii_case("none") {
                decl.transform = Some(Vec::new());
            } else {
                decl.transform = TransformOp::parse(value).ok();
            }
            true
        }
        "transform-origin" => {
            decl.transform_origin = Some(parse_position(value));
            true
        }
        "clip-path" | "mask" | "-webkit-clip-path" => {
            decl.clip_path = parse_clip_path(value);
            true
        }
        "isolation" => {
            decl.isolation = Some(value.eq_ignore_ascii_case("isolate"));
            true
        }
        "object-fit" => {
            decl.object_fit = Some(match value.to_ascii_lowercase().as_str() {
                "contain" => ObjectFit::Contain,
                "cover" => ObjectFit::Cover,
                "none" => ObjectFit::None,
                "scale-down" => ObjectFit::ScaleDown,
                _ => ObjectFit::Fill,
            });
            true
        }
        "object-position" => {
            let (x, y) = parse_position(value);
            decl.object_position = Some(BackgroundPosition { x, y });
            true
        }
        _ => false,
    }
}

fn parse_opacity(value: &str) -> Option<f32> {
    let text = value.trim();
    if let Some(percent) = text.strip_suffix('%') {
        return percent
            .parse::<f32>()
            .ok()
            .map(|value| (value / 100.0).clamp(0.0, 1.0));
    }
    text.parse::<f32>().ok().map(|value| value.clamp(0.0, 1.0))
}

fn parse_position(value: &str) -> (Length, Length) {
    let mut x = None;
    let mut y = None;
    for token in split_ws(value) {
        match token.to_ascii_lowercase().as_str() {
            "left" => x = Some(Length::Percent(0.0)),
            "right" => x = Some(Length::Percent(100.0)),
            "top" => y = Some(Length::Percent(0.0)),
            "bottom" => y = Some(Length::Percent(100.0)),
            "center" => {
                if x.is_none() {
                    x = Some(Length::Percent(50.0));
                } else {
                    y = Some(Length::Percent(50.0));
                }
            }
            _ => {
                if let Ok(length) = Length::parse(&token) {
                    if x.is_none() {
                        x = Some(length);
                    } else if y.is_none() {
                        y = Some(length);
                    }
                }
            }
        }
    }
    (
        x.unwrap_or(Length::Percent(50.0)),
        y.unwrap_or(Length::Percent(50.0)),
    )
}

fn parse_filter_list(value: &str, current: Color) -> Vec<Filter> {
    let mut filters = Vec::new();
    for function in split_functions(value) {
        let Some(open) = function.find('(') else {
            continue;
        };
        let name = function[..open].trim().to_ascii_lowercase();
        let inner = function[open + 1..]
            .strip_suffix(')')
            .unwrap_or(&function[open + 1..])
            .trim();

        let amount = |default: f32| -> f32 {
            if inner.is_empty() {
                return default;
            }
            if let Some(percent) = inner.strip_suffix('%') {
                percent.parse::<f32>().map(|v| v / 100.0).unwrap_or(default)
            } else {
                inner.parse::<f32>().unwrap_or(default)
            }
        };

        filters.push(match name.as_str() {
            "blur" => Filter::Blur(Length::parse(inner).unwrap_or(Length::Zero)),
            "brightness" => Filter::Brightness(amount(1.0)),
            "contrast" => Filter::Contrast(amount(1.0)),
            "grayscale" => Filter::Grayscale(amount(1.0)),
            "saturate" => Filter::Saturate(amount(1.0)),
            "sepia" => Filter::Sepia(amount(1.0)),
            "invert" => Filter::Invert(amount(1.0)),
            "opacity" => Filter::Opacity(amount(1.0)),
            "hue-rotate" => Filter::HueRotate(Angle::parse(inner).unwrap_or(Angle(0.0))),
            "drop-shadow" => {
                if let Ok(shadow) = Shadow::parse(inner, current) {
                    Filter::DropShadow(shadow)
                } else {
                    continue;
                }
            }
            _ => continue,
        });
    }
    filters
}

fn split_functions(input: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut current = String::new();
    let mut depth = 0_i32;
    for ch in input.chars() {
        match ch {
            '(' => {
                depth += 1;
                current.push(ch);
            }
            ')' => {
                depth -= 1;
                current.push(ch);
                if depth == 0 {
                    result.push(std::mem::take(&mut current));
                }
            }
            c if c.is_whitespace() && depth == 0 => {
                if !current.is_empty() {
                    result.push(std::mem::take(&mut current));
                }
            }
            c => current.push(c),
        }
    }
    if !current.is_empty() {
        result.push(current);
    }
    result
}

fn parse_clip_path(value: &str) -> Option<ClipPath> {
    let text = value.trim();
    if text.eq_ignore_ascii_case("none") {
        return Some(ClipPath::None);
    }
    let open = text.find('(')?;
    let name = text[..open].trim().to_ascii_lowercase();
    let inner = text[open + 1..].strip_suffix(')')?.trim();
    Some(match name.as_str() {
        "inset" => {
            let (edges_text, round) = inner.split_once(" round ").unwrap_or((inner, ""));
            let values: Vec<Length> = split_ws(edges_text)
                .iter()
                .filter_map(|token| Length::parse(token).ok())
                .collect();
            let edges = Edges::from_1_to_4(&values).unwrap_or_default();
            let radius = Length::parse(round.trim()).ok();
            ClipPath::Inset(edges, radius)
        }
        "circle" => {
            let mut radius = Length::Percent(50.0);
            let mut center = (Length::Percent(50.0), Length::Percent(50.0));
            if let Some((before, after)) = inner.split_once(" at ") {
                if let Ok(length) = Length::parse(before.trim()) {
                    radius = length;
                }
                let (x, y) = parse_position(after);
                center = (x, y);
            } else if !inner.is_empty()
                && let Ok(length) = Length::parse(inner)
            {
                radius = length;
            }
            ClipPath::Circle(radius, center.0, center.1)
        }
        "ellipse" => {
            let mut radii = (Length::Percent(50.0), Length::Percent(50.0));
            let mut center = (Length::Percent(50.0), Length::Percent(50.0));
            if let Some((before, after)) = inner.split_once(" at ") {
                let values = split_ws(before);
                if let Some(x) = values.first().and_then(|v| Length::parse(v).ok()) {
                    radii.0 = x;
                }
                if let Some(y) = values.get(1).and_then(|v| Length::parse(v).ok()) {
                    radii.1 = y;
                }
                center = parse_position(after);
            }
            ClipPath::Ellipse(radii.0, radii.1, center.0, center.1)
        }
        "polygon" => {
            let mut points = Vec::new();
            for pair in inner.split(',') {
                let values = split_ws(pair);
                let x = values
                    .first()
                    .and_then(|v| Length::parse(v).ok())
                    .unwrap_or(Length::Percent(0.0));
                let y = values
                    .get(1)
                    .and_then(|v| Length::parse(v).ok())
                    .unwrap_or(Length::Percent(0.0));
                points.push((x, y));
            }
            ClipPath::Polygon(points)
        }
        "url" => ClipPath::Url(inner.trim_matches('"').trim_matches('\'').to_string()),
        _ => return None,
    })
}
