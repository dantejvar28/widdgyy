// SPDX-FileCopyrightText: 2026 Daniel
// SPDX-License-Identifier: GPL-3.0-or-later

//! Border and outline properties.

use super::{BorderStyle, Declarations, parse_color, parse_lengths, split_ws};
use crate::css::values::{Color, Edges, Length};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BorderImageRepeat {
    #[default]
    Stretch,
    Repeat,
    Round,
    Space,
}

impl BorderImageRepeat {
    fn parse(token: &str) -> Option<Self> {
        Some(match token.to_ascii_lowercase().as_str() {
            "stretch" => Self::Stretch,
            "repeat" => Self::Repeat,
            "round" => Self::Round,
            "space" => Self::Space,
            _ => return None,
        })
    }
}

#[derive(Clone, Debug, Default)]
pub struct BorderImage {
    pub source: String,
    pub slice: Edges<Length>,
    pub width: Edges<Length>,
    pub outset: Edges<Length>,
    pub repeat_x: BorderImageRepeat,
    pub repeat_y: BorderImageRepeat,
    pub fill: bool,
}

pub fn apply(decl: &mut Declarations, name: &str, value: &str, current: Color) -> bool {
    match name {
        "border-width" => {
            if let Ok(values) = parse_lengths(value)
                && let Some(edges) = Edges::from_1_to_4(&values)
            {
                decl.border_width = Some(edges);
            }
            true
        }
        "border-style" => {
            let values: Vec<BorderStyle> = split_ws(value)
                .iter()
                .filter_map(|token| parse_style(token))
                .collect();
            if let Some(edges) = Edges::from_1_to_4(&values) {
                decl.border_style = Some(edges);
            }
            true
        }
        "border-color" => {
            let values: Vec<Color> = split_ws(value)
                .iter()
                .filter_map(|token| parse_color(token, current))
                .collect();
            if let Some(edges) = Edges::from_1_to_4(&values) {
                decl.border_color = Some(edges);
            }
            true
        }
        "border-radius" => {
            // The slash form (`10px / 20px`) describes elliptical corners.
            // Vello paints circular corners; use the horizontal radii.
            let horizontal = value.split('/').next().unwrap_or(value);
            if let Ok(values) = parse_lengths(horizontal)
                && let Some(edges) = Edges::from_1_to_4(&values)
            {
                decl.border_radius = Some(edges);
            }
            true
        }
        "border-image-source" => {
            decl.border_image_source = parse_image_url(value);
            true
        }
        "border-image-slice" => {
            let mut tokens = split_ws(value);
            let fill = tokens
                .iter()
                .any(|token| token.eq_ignore_ascii_case("fill"));
            tokens.retain(|token| !token.eq_ignore_ascii_case("fill"));
            let lengths: Vec<Length> = tokens
                .iter()
                .filter_map(|token| Length::parse(token).ok())
                .collect();
            if let Some(slice) = Edges::from_1_to_4(&lengths) {
                decl.border_image_slice = Some((slice, fill));
            }
            true
        }
        "border-image-outset" => {
            if let Ok(values) = parse_lengths(value)
                && let Some(outset) = Edges::from_1_to_4(&values)
            {
                decl.border_image_outset = Some(outset);
            }
            true
        }
        "border-image-width" => {
            if let Ok(values) = parse_lengths(value)
                && let Some(width) = Edges::from_1_to_4(&values)
            {
                decl.border_image_width = Some(width);
            }
            true
        }
        "border-image-repeat" => {
            let values: Vec<BorderImageRepeat> = split_ws(value)
                .iter()
                .filter_map(|token| BorderImageRepeat::parse(token))
                .collect();
            let repeats = match values.as_slice() {
                [both] => Some((*both, *both)),
                [x, y, ..] => Some((*x, *y)),
                [] => None,
            };
            if let Some(repeats) = repeats {
                decl.border_image_repeat = Some(repeats);
            }
            true
        }
        "border-image" => {
            apply_border_image_shorthand(decl, value);
            true
        }
        "border" | "border-top" | "border-right" | "border-bottom" | "border-left" => {
            apply_border_shorthand(decl, name, value, current);
            true
        }
        "border-top-width" | "border-right-width" | "border-bottom-width" | "border-left-width" => {
            if let Some(length) = parse_border_width(value) {
                let edges = decl.border_width.get_or_insert_with(Edges::default);
                match name {
                    "border-top-width" => edges.top = length,
                    "border-right-width" => edges.right = length,
                    "border-bottom-width" => edges.bottom = length,
                    _ => edges.left = length,
                }
            }
            true
        }
        "border-top-style" | "border-right-style" | "border-bottom-style" | "border-left-style" => {
            if let Some(style) = parse_style(value) {
                let edges = decl.border_style.get_or_insert_with(Edges::default);
                match name {
                    "border-top-style" => edges.top = style,
                    "border-right-style" => edges.right = style,
                    "border-bottom-style" => edges.bottom = style,
                    _ => edges.left = style,
                }
            }
            true
        }
        "border-top-color" | "border-right-color" | "border-bottom-color" | "border-left-color" => {
            if let Some(color) = parse_color(value, current) {
                let edges = decl.border_color.get_or_insert_with(|| Edges::all(current));
                match name {
                    "border-top-color" => edges.top = color,
                    "border-right-color" => edges.right = color,
                    "border-bottom-color" => edges.bottom = color,
                    _ => edges.left = color,
                }
            }
            true
        }
        "border-top-left-radius"
        | "border-top-right-radius"
        | "border-bottom-right-radius"
        | "border-bottom-left-radius" => {
            if let Ok(length) = Length::parse(value) {
                let edges = decl.border_radius.get_or_insert_with(Edges::default);
                match name {
                    "border-top-left-radius" => edges.top = length,
                    "border-top-right-radius" => edges.right = length,
                    "border-bottom-right-radius" => edges.bottom = length,
                    _ => edges.left = length,
                }
            }
            true
        }
        "outline" => {
            for token in split_ws(value) {
                if let Some(color) = parse_color(&token, current) {
                    decl.outline_color = Some(color);
                } else if let Some(style) = parse_style(&token) {
                    decl.outline_style = Some(style);
                } else if let Some(width) = parse_border_width(&token) {
                    decl.outline_width = Some(width);
                }
            }
            true
        }
        "outline-width" => {
            decl.outline_width = parse_border_width(value);
            true
        }
        "outline-style" => {
            decl.outline_style = parse_style(value);
            true
        }
        "outline-color" => {
            decl.outline_color = parse_color(value, current);
            true
        }
        "outline-offset" => {
            decl.outline_offset = Length::parse(value).ok();
            true
        }
        _ => false,
    }
}

fn apply_border_shorthand(decl: &mut Declarations, name: &str, value: &str, current: Color) {
    let mut width = None;
    let mut style = None;
    let mut color = None;
    for token in split_ws(value) {
        if let Some(parsed) = parse_color(&token, current) {
            color = Some(parsed);
        } else if let Some(parsed) = parse_style(&token) {
            style = Some(parsed);
        } else if let Some(parsed) = parse_border_width(&token) {
            width = Some(parsed);
        }
    }

    let width = width.unwrap_or(Length::Px(0.0));
    let style = style.unwrap_or(BorderStyle::None);
    let color = color.unwrap_or(current);

    if name == "border" {
        decl.border_width = Some(Edges::all(width));
        decl.border_style = Some(Edges::all(style));
        decl.border_color = Some(Edges::all(color));
    } else {
        let widths = decl.border_width.get_or_insert_with(Edges::default);
        let styles = decl.border_style.get_or_insert_with(Edges::default);
        let colors = decl.border_color.get_or_insert_with(|| Edges::all(current));
        match name {
            "border-top" => {
                widths.top = width;
                styles.top = style;
                colors.top = color;
            }
            "border-right" => {
                widths.right = width;
                styles.right = style;
                colors.right = color;
            }
            "border-bottom" => {
                widths.bottom = width;
                styles.bottom = style;
                colors.bottom = color;
            }
            _ => {
                widths.left = width;
                styles.left = style;
                colors.left = color;
            }
        }
    }
}

pub fn parse_style(value: &str) -> Option<BorderStyle> {
    Some(match value.trim().to_ascii_lowercase().as_str() {
        "none" => BorderStyle::None,
        "solid" => BorderStyle::Solid,
        "dashed" => BorderStyle::Dashed,
        "dotted" => BorderStyle::Dotted,
        "double" => BorderStyle::Double,
        "groove" => BorderStyle::Groove,
        "ridge" => BorderStyle::Ridge,
        "inset" => BorderStyle::Inset,
        "outset" => BorderStyle::Outset,
        _ => return None,
    })
}

pub fn apply_border_image_shorthand(decl: &mut Declarations, value: &str) {
    let tokens = split_image_shorthand(value);
    let mut source: Option<String> = None;
    let mut slice: Vec<Length> = Vec::new();
    let mut fill = false;
    let mut width: Vec<Length> = Vec::new();
    let mut outset: Vec<Length> = Vec::new();
    let mut repeats: Vec<BorderImageRepeat> = Vec::new();
    let mut stage = 0u8;

    for token in tokens {
        let lower = token.to_ascii_lowercase();
        if let Some(repeat) = BorderImageRepeat::parse(&token) {
            if repeats.len() < 2 {
                repeats.push(repeat);
            }
            continue;
        }
        match lower.as_str() {
            "/" => {
                stage = stage.saturating_add(1);
                continue;
            }
            "fill" => {
                fill = true;
                continue;
            }
            "none" => {
                source = None;
                continue;
            }
            _ => {}
        }
        if lower.starts_with("url(") {
            source = parse_image_url(&token);
            continue;
        }
        if let Ok(length) = Length::parse(&token) {
            match stage {
                0 => slice.push(length),
                1 => width.push(length),
                _ => outset.push(length),
            }
        }
    }

    decl.border_image_source = source;
    let slice = Edges::from_1_to_4(&slice).unwrap_or_else(|| Edges::all(Length::Percent(100.0)));
    decl.border_image_slice = Some((slice, fill));
    decl.border_image_width = Edges::from_1_to_4(&width);
    decl.border_image_outset = Edges::from_1_to_4(&outset);
    let (repeat_x, repeat_y) = match repeats.as_slice() {
        [both] => (*both, *both),
        [x, y, ..] => (*x, *y),
        [] => (BorderImageRepeat::Stretch, BorderImageRepeat::Stretch),
    };
    decl.border_image_repeat = Some((repeat_x, repeat_y));
}

/// Split a shorthand into whitespace-separated tokens and slashes while keeping
/// function arguments such as `url(...)` together.
fn split_image_shorthand(value: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut depth = 0i32;
    for ch in value.chars() {
        if ch == '(' {
            depth += 1;
            current.push(ch);
        } else if ch == ')' {
            depth = (depth - 1).max(0);
            current.push(ch);
        } else if depth == 0 && (ch.is_whitespace() || ch == '/') {
            if !current.is_empty() {
                tokens.push(std::mem::take(&mut current));
            }
            if ch == '/' {
                tokens.push("/".to_string());
            }
        } else {
            current.push(ch);
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

fn parse_image_url(value: &str) -> Option<String> {
    let text = value.trim();
    if text.eq_ignore_ascii_case("none") {
        return None;
    }
    let inner = text
        .strip_prefix("url(")
        .or_else(|| text.strip_prefix("URL("))?
        .strip_suffix(')')?
        .trim()
        .trim_matches(|c| c == '"' || c == '\'');
    (!inner.is_empty()).then(|| inner.to_string())
}

fn parse_border_width(value: &str) -> Option<Length> {
    match value.trim().to_ascii_lowercase().as_str() {
        "thin" => Some(Length::Px(1.0)),
        "medium" => Some(Length::Px(3.0)),
        "thick" => Some(Length::Px(5.0)),
        other => Length::parse(other).ok(),
    }
}
