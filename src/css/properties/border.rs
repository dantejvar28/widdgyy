// SPDX-FileCopyrightText: 2026 Daniel
// SPDX-License-Identifier: GPL-3.0-or-later

//! Border and outline properties.

use super::{BorderStyle, Declarations, parse_color, parse_lengths, split_ws};
use crate::css::values::{Color, Edges, Length};

#[derive(Clone, Debug, Default)]
pub struct BorderImage {
    pub source: String,
    pub slice: Edges<Length>,
    pub width: Edges<Length>,
    pub outset: Edges<Length>,
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
        "border-image" => true,
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

pub fn parse_image_url(value: &str) -> Option<String> {
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
