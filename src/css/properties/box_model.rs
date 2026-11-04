// SPDX-FileCopyrightText: 2026 Daniel
// SPDX-License-Identifier: GPL-3.0-or-later

//! Box model properties: display, position, sizing, margin/padding, overflow.

use super::{BoxSizing, Declarations, Display, Overflow, Position, parse_lengths, split_ws};
use crate::css::values::{Color, Edges, Length, Size};

pub fn apply(decl: &mut Declarations, name: &str, value: &str, _current: Color) -> bool {
    match name {
        "display" => {
            decl.display = Some(match value.to_ascii_lowercase().as_str() {
                "none" => Display::None,
                "block" => Display::Block,
                "inline" => Display::Inline,
                "inline-block" => Display::InlineBlock,
                "flex" | "inline-flex" => Display::Flex,
                "grid" | "inline-grid" => Display::Grid,
                _ => return false,
            });
            true
        }
        "position" => {
            decl.position = Some(match value.to_ascii_lowercase().as_str() {
                "static" => Position::Static,
                "relative" => Position::Relative,
                "absolute" => Position::Absolute,
                "fixed" => Position::Fixed,
                "sticky" => Position::Sticky,
                _ => return false,
            });
            true
        }
        "top" | "right" | "bottom" | "left" => {
            if let Ok(length) = Length::parse(value) {
                match name {
                    "top" => decl.top = Some(length),
                    "right" => decl.right = Some(length),
                    "bottom" => decl.bottom = Some(length),
                    _ => decl.left = Some(length),
                }
            }
            true
        }
        "inset" => {
            if let Ok(values) = parse_lengths(value)
                && let Some(edges) = Edges::from_1_to_4(&values)
            {
                decl.top = Some(edges.top);
                decl.right = Some(edges.right);
                decl.bottom = Some(edges.bottom);
                decl.left = Some(edges.left);
            }
            true
        }
        "width" | "height" | "min-width" | "min-height" | "max-width" | "max-height" => {
            if let Ok(size) = Size::parse(value) {
                match name {
                    "width" => decl.width = Some(size),
                    "height" => decl.height = Some(size),
                    "min-width" => decl.min_width = Some(size),
                    "min-height" => decl.min_height = Some(size),
                    "max-width" => decl.max_width = Some(size),
                    _ => decl.max_height = Some(size),
                }
            }
            true
        }
        "margin" => {
            if let Ok(values) = parse_lengths(value)
                && let Some(edges) = Edges::from_1_to_4(&values)
            {
                decl.margin = Some(edges);
            }
            true
        }
        "margin-top" | "margin-right" | "margin-bottom" | "margin-left" => {
            if let Ok(length) = Length::parse(value) {
                let edges = decl.margin.get_or_insert_with(Edges::default);
                match name {
                    "margin-top" => edges.top = length,
                    "margin-right" => edges.right = length,
                    "margin-bottom" => edges.bottom = length,
                    _ => edges.left = length,
                }
            }
            true
        }
        "padding" => {
            if let Ok(values) = parse_lengths(value)
                && let Some(edges) = Edges::from_1_to_4(&values)
            {
                decl.padding = Some(edges);
            }
            true
        }
        "padding-top" | "padding-right" | "padding-bottom" | "padding-left" => {
            if let Ok(length) = Length::parse(value) {
                let edges = decl.padding.get_or_insert_with(Edges::default);
                match name {
                    "padding-top" => edges.top = length,
                    "padding-right" => edges.right = length,
                    "padding-bottom" => edges.bottom = length,
                    _ => edges.left = length,
                }
            }
            true
        }
        "box-sizing" => {
            decl.box_sizing = Some(match value.to_ascii_lowercase().as_str() {
                "border-box" => BoxSizing::BorderBox,
                "content-box" => BoxSizing::ContentBox,
                _ => return false,
            });
            true
        }
        "overflow" => {
            let values = split_ws(value);
            let x = values.first().and_then(|value| parse_overflow(value));
            let y = values.get(1).and_then(|value| parse_overflow(value)).or(x);
            if let Some(x) = x {
                decl.overflow_x = Some(x);
                decl.overflow_y = Some(y.unwrap_or(x));
            }
            true
        }
        "overflow-x" | "overflow-y" => {
            if let Some(overflow) = parse_overflow(value) {
                if name == "overflow-x" {
                    decl.overflow_x = Some(overflow);
                } else {
                    decl.overflow_y = Some(overflow);
                }
            }
            true
        }
        "aspect-ratio" => {
            let text = value.trim();
            let ratio = if let Some((a, b)) = text.split_once('/') {
                a.trim()
                    .parse::<f32>()
                    .ok()
                    .zip(b.trim().parse::<f32>().ok())
                    .and_then(|(a, b)| (b != 0.0).then_some(a / b))
            } else {
                text.parse::<f32>().ok()
            };
            if let Some(ratio) = ratio {
                decl.aspect_ratio = Some(ratio);
            }
            true
        }
        _ => false,
    }
}

fn parse_overflow(value: &str) -> Option<Overflow> {
    Some(match value.to_ascii_lowercase().as_str() {
        "visible" => Overflow::Visible,
        "hidden" => Overflow::Hidden,
        "clip" => Overflow::Clip,
        "scroll" => Overflow::Scroll,
        "auto" | "overlay" => Overflow::Auto,
        _ => return None,
    })
}
