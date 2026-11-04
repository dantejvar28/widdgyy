// SPDX-FileCopyrightText: 2026 Daniel
// SPDX-License-Identifier: GPL-3.0-or-later

//! Background properties: color, images, gradients, position, size, repeat.

use vello::peniko::Mix;

use super::{
    BackgroundPosition, BackgroundRepeat, BackgroundSize, BorderBox, Declarations, parse_color,
    split_commas, split_ws,
};
use crate::css::values::{Color, Gradient, Length};

#[derive(Clone, Debug, PartialEq)]
pub enum BackgroundImage {
    Url(String),
    Gradient(Gradient),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum BackgroundAttachment {
    #[default]
    Scroll,
    Fixed,
    Local,
}

pub fn apply(decl: &mut Declarations, name: &str, value: &str, current: Color) -> bool {
    match name {
        "background-color" => {
            if let Some(color) = parse_color(value, current) {
                decl.background_color = Some(color);
            }
            true
        }
        "background-image" => {
            if value.eq_ignore_ascii_case("none") {
                decl.background_image = Some(Vec::new());
            } else {
                decl.background_image = Some(parse_image_list(value, current));
            }
            true
        }
        "background-position" => {
            decl.background_position = Some(parse_position_list(value));
            true
        }
        "background-size" => {
            decl.background_size = Some(parse_size_list(value));
            true
        }
        "background-repeat" => {
            decl.background_repeat = Some(parse_repeat_list(value));
            true
        }
        "background-clip" => {
            if let Some(clip) = parse_box(value) {
                decl.background_clip = Some(clip);
            }
            true
        }
        "background-origin" => {
            if let Some(origin) = parse_box(value) {
                decl.background_origin = Some(origin);
            }
            true
        }
        "background-blend-mode" => {
            decl.background_blend_mode = parse_blend_list(value).into_iter().next();
            true
        }
        "background-attachment" => {
            decl.background_attachment = Some(match value.to_ascii_lowercase().as_str() {
                "fixed" => BackgroundAttachment::Fixed,
                "local" => BackgroundAttachment::Local,
                _ => BackgroundAttachment::Scroll,
            });
            true
        }
        "background" => {
            apply_shorthand(decl, value, current);
            true
        }
        _ => false,
    }
}

fn parse_image_list(value: &str, current: Color) -> Vec<BackgroundImage> {
    let mut images = Vec::new();
    for part in split_commas(value) {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        if let Some(url) = parse_url(part) {
            images.push(BackgroundImage::Url(url));
        } else if part.to_ascii_lowercase().contains("gradient(")
            && let Ok(gradient) = Gradient::parse(part, current)
        {
            images.push(BackgroundImage::Gradient(gradient));
        }
    }
    images
}

fn parse_url(value: &str) -> Option<String> {
    let text = value.trim();
    let lower = text.to_ascii_lowercase();
    if !lower.starts_with("url(") {
        return None;
    }
    let inner = text[4..].strip_suffix(')')?.trim();
    Some(
        inner
            .trim_matches('"')
            .trim_matches('\'')
            .trim()
            .to_string(),
    )
}

fn parse_position(value: &str) -> BackgroundPosition {
    let tokens = split_ws(value);
    let mut x = None;
    let mut y = None;
    for token in tokens {
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
    BackgroundPosition {
        x: x.unwrap_or(Length::Percent(0.0)),
        y: y.unwrap_or(Length::Percent(0.0)),
    }
}

fn parse_position_list(value: &str) -> Vec<BackgroundPosition> {
    split_commas(value)
        .into_iter()
        .map(|part| parse_position(&part))
        .collect()
}

fn parse_size_list(value: &str) -> Vec<BackgroundSize> {
    split_commas(value)
        .into_iter()
        .map(|part| {
            let tokens = split_ws(&part);
            match tokens.first().map(|token| token.to_ascii_lowercase()) {
                Some(token) if token == "cover" => BackgroundSize::Cover,
                Some(token) if token == "contain" => BackgroundSize::Contain,
                Some(token) if token == "auto" && tokens.len() == 1 => BackgroundSize::Auto,
                _ => {
                    let x = tokens
                        .first()
                        .and_then(|token| Length::parse(token).ok())
                        .unwrap_or(Length::Auto);
                    let y = tokens
                        .get(1)
                        .and_then(|token| Length::parse(token).ok())
                        .unwrap_or(Length::Auto);
                    BackgroundSize::Explicit(x, y)
                }
            }
        })
        .collect()
}

fn parse_repeat_list(value: &str) -> Vec<BackgroundRepeat> {
    split_commas(value)
        .into_iter()
        .map(|part| match part.trim().to_ascii_lowercase().as_str() {
            "repeat-x" => BackgroundRepeat::RepeatX,
            "repeat-y" => BackgroundRepeat::RepeatY,
            "no-repeat" => BackgroundRepeat::NoRepeat,
            "space" => BackgroundRepeat::Space,
            "round" => BackgroundRepeat::Round,
            _ => BackgroundRepeat::Repeat,
        })
        .collect()
}

fn parse_box(value: &str) -> Option<BorderBox> {
    Some(match value.trim().to_ascii_lowercase().as_str() {
        "border-box" | "border" => BorderBox::BorderBox,
        "padding-box" | "padding" => BorderBox::PaddingBox,
        "content-box" | "content" => BorderBox::ContentBox,
        "text" => BorderBox::Text,
        _ => return None,
    })
}

fn parse_blend_list(value: &str) -> Vec<Mix> {
    split_commas(value)
        .into_iter()
        .filter_map(|part| parse_blend(part.trim()))
        .collect()
}

pub fn parse_blend(value: &str) -> Option<Mix> {
    Some(match value.to_ascii_lowercase().as_str() {
        "normal" => Mix::Normal,
        "multiply" => Mix::Multiply,
        "screen" => Mix::Screen,
        "overlay" => Mix::Overlay,
        "darken" => Mix::Darken,
        "lighten" => Mix::Lighten,
        "color-dodge" => Mix::ColorDodge,
        "color-burn" => Mix::ColorBurn,
        "hard-light" => Mix::HardLight,
        "soft-light" => Mix::SoftLight,
        "difference" => Mix::Difference,
        "exclusion" => Mix::Exclusion,
        "hue" => Mix::Hue,
        "saturation" => Mix::Saturation,
        "color" => Mix::Color,
        "luminosity" => Mix::Luminosity,
        "plus-lighter" => Mix::Normal,
        _ => return None,
    })
}

fn apply_shorthand(decl: &mut Declarations, value: &str, current: Color) {
    // `background` is layer based; support the common single layer form and
    // apply the same parsing to each comma separated layer.
    for layer in split_commas(value) {
        let layer = layer.trim();
        if layer.is_empty() {
            continue;
        }
        let mut color = None;
        let mut images = Vec::new();
        let mut repeat = Vec::new();
        let mut position = None;
        let mut size = None;
        let mut origin = None;
        let mut clip = None;
        let mut attachment = BackgroundAttachment::Scroll;

        let tokens = split_ws(layer);
        let mut index = 0;
        while index < tokens.len() {
            let token = &tokens[index];
            let lower = token.to_ascii_lowercase();
            if lower == "url(" || lower.starts_with("url(") || lower.contains("gradient(") {
                if let Some(url) = parse_url(token) {
                    images.push(BackgroundImage::Url(url));
                } else if let Ok(gradient) = Gradient::parse(token, current) {
                    images.push(BackgroundImage::Gradient(gradient));
                }
            } else if let Some(parsed) = parse_color(token, current) {
                color = Some(parsed);
            } else if matches!(
                lower.as_str(),
                "repeat" | "repeat-x" | "repeat-y" | "no-repeat" | "space" | "round"
            ) {
                repeat.push(match lower.as_str() {
                    "repeat-x" => BackgroundRepeat::RepeatX,
                    "repeat-y" => BackgroundRepeat::RepeatY,
                    "no-repeat" => BackgroundRepeat::NoRepeat,
                    "space" => BackgroundRepeat::Space,
                    "round" => BackgroundRepeat::Round,
                    _ => BackgroundRepeat::Repeat,
                });
            } else if matches!(lower.as_str(), "scroll" | "fixed" | "local") {
                attachment = match lower.as_str() {
                    "fixed" => BackgroundAttachment::Fixed,
                    "local" => BackgroundAttachment::Local,
                    _ => BackgroundAttachment::Scroll,
                };
            } else if matches!(
                lower.as_str(),
                "border-box" | "padding-box" | "content-box" | "text"
            ) {
                if origin.is_none() {
                    origin = parse_box(&lower);
                } else {
                    clip = parse_box(&lower);
                }
            } else if lower == "/" {
                if index + 1 < tokens.len() {
                    size = Some(
                        parse_size_list(&tokens[index + 1])
                            .into_iter()
                            .next()
                            .unwrap_or_default(),
                    );
                    index += 1;
                }
            } else if lower == "cover" || lower == "contain" {
                size = Some(if lower == "cover" {
                    BackgroundSize::Cover
                } else {
                    BackgroundSize::Contain
                });
            } else if matches!(
                lower.as_str(),
                "left" | "right" | "top" | "bottom" | "center"
            ) || Length::parse(token).is_ok()
            {
                let mut position_tokens = Vec::new();
                while index < tokens.len() {
                    let candidate = tokens[index].to_ascii_lowercase();
                    if matches!(
                        candidate.as_str(),
                        "left" | "right" | "top" | "bottom" | "center"
                    ) || Length::parse(&tokens[index]).is_ok()
                    {
                        position_tokens.push(tokens[index].clone());
                        index += 1;
                    } else {
                        break;
                    }
                }
                if !position_tokens.is_empty() {
                    position = Some(parse_position(&position_tokens.join(" ")));
                }
                continue;
            }
            index += 1;
        }

        if let Some(color) = color {
            decl.background_color = Some(color);
        }
        if !images.is_empty() {
            decl.background_image = Some(images);
        }
        if !repeat.is_empty() {
            decl.background_repeat = Some(repeat);
        }
        if let Some(position) = position {
            decl.background_position = Some(vec![position]);
        }
        if let Some(size) = size {
            decl.background_size = Some(vec![size]);
        }
        if let Some(origin) = origin {
            decl.background_origin = Some(origin);
        }
        if let Some(clip) = clip {
            decl.background_clip = Some(clip);
        }
        decl.background_attachment = Some(attachment);
    }
}
