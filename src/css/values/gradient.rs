// SPDX-FileCopyrightText: 2026 Daniel
// SPDX-License-Identifier: GPL-3.0-or-later

//! Linear/radial gradients.

use super::angle::Angle;
use super::color::Color;
use super::length::Length;
use super::shadow::split_commas;

#[derive(Clone, Debug, PartialEq)]
pub struct ColorStop {
    pub color: Color,
    /// `None` means evenly distributed by the renderer.
    pub position: Option<f32>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum GradientKind {
    Linear {
        angle: Angle,
        repeating: bool,
    },
    Radial {
        center_x: Length,
        center_y: Length,
        radius: Length,
        repeating: bool,
    },
    Conic {
        angle: Angle,
        center_x: Length,
        center_y: Length,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Gradient {
    pub kind: GradientKind,
    pub stops: Vec<ColorStop>,
}

impl Gradient {
    pub fn parse(input: &str, current_color: Color) -> Result<Self, String> {
        let text = input.trim();
        let lower = text.to_ascii_lowercase();
        let (kind, body_start) = if let Some(body) = strip_function(&lower, "linear-gradient") {
            let offset = text.len() - body.len();
            let (angle, consumed) = parse_linear_prefix(&text[offset..])?;
            (
                GradientKind::Linear {
                    angle,
                    repeating: false,
                },
                offset + consumed,
            )
        } else if let Some(body) = strip_function(&lower, "repeating-linear-gradient") {
            let offset = text.len() - body.len();
            let (angle, consumed) = parse_linear_prefix(&text[offset..])?;
            (
                GradientKind::Linear {
                    angle,
                    repeating: true,
                },
                offset + consumed,
            )
        } else if let Some(body) = strip_function(&lower, "radial-gradient") {
            let offset = text.len() - body.len();
            let (center_x, center_y, radius, consumed) = parse_radial_prefix(&text[offset..])?;
            (
                GradientKind::Radial {
                    center_x,
                    center_y,
                    radius,
                    repeating: false,
                },
                offset + consumed,
            )
        } else if let Some(body) = strip_function(&lower, "repeating-radial-gradient") {
            let offset = text.len() - body.len();
            let (center_x, center_y, radius, consumed) = parse_radial_prefix(&text[offset..])?;
            (
                GradientKind::Radial {
                    center_x,
                    center_y,
                    radius,
                    repeating: true,
                },
                offset + consumed,
            )
        } else if let Some(body) = strip_function(&lower, "conic-gradient") {
            let offset = text.len() - body.len();
            let (angle, consumed) = parse_linear_prefix(&text[offset..])?;
            (
                GradientKind::Conic {
                    angle,
                    center_x: Length::Percent(50.0),
                    center_y: Length::Percent(50.0),
                },
                offset + consumed,
            )
        } else {
            return Err(format!("unsupported gradient: {input}"));
        };

        let stops_text = text[body_start..].trim();
        let stops_text = stops_text
            .trim_start_matches(',')
            .trim_end_matches(')')
            .trim();
        let mut stops = Vec::new();
        for part in split_commas(stops_text) {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }
            let mut color = None;
            let mut position = None;
            for token in part.split_whitespace() {
                let position_token = token.trim_end_matches(')');
                if position_token.ends_with('%') {
                    position = position_token
                        .trim_end_matches('%')
                        .parse::<f32>()
                        .ok()
                        .map(|value| value / 100.0);
                } else if let Some(parsed) = super::color::parse(token, current_color) {
                    color = Some(parsed);
                }
            }
            if let Some(color) = color {
                stops.push(ColorStop { color, position });
            }
        }
        if stops.is_empty() {
            return Err(format!("gradient without color stops: {input}"));
        }
        Ok(Self { kind, stops })
    }
}

fn strip_function<'a>(input: &'a str, name: &str) -> Option<&'a str> {
    let prefix = format!("{name}(");
    input.strip_prefix(&prefix)
}

fn parse_linear_prefix(input: &str) -> Result<(Angle, usize), String> {
    let trimmed = input.trim_start();
    let consumed_ws = input.len() - trimmed.len();
    let lower = trimmed.to_ascii_lowercase();
    for (name, angle) in [
        ("to top right", Angle(45.0)),
        ("to right top", Angle(45.0)),
        ("to bottom right", Angle(135.0)),
        ("to right bottom", Angle(135.0)),
        ("to bottom left", Angle(225.0)),
        ("to left bottom", Angle(225.0)),
        ("to top left", Angle(315.0)),
        ("to left top", Angle(315.0)),
        ("to top", Angle(0.0)),
        ("to right", Angle(90.0)),
        ("to bottom", Angle(180.0)),
        ("to left", Angle(270.0)),
    ] {
        if lower.starts_with(name) {
            return Ok((angle, consumed_ws + name.len()));
        }
    }
    let end = trimmed.find(',').unwrap_or(trimmed.len());
    let candidate = trimmed[..end].trim();
    if let Ok(angle) = Angle::parse(candidate) {
        return Ok((angle, consumed_ws + end));
    }
    Ok((Angle(180.0), consumed_ws))
}

fn parse_radial_prefix(input: &str) -> Result<(Length, Length, Length, usize), String> {
    let trimmed = input.trim_start();
    let consumed_ws = input.len() - trimmed.len();
    let end = trimmed.find(',').unwrap_or(trimmed.len());
    let prefix = trimmed[..end].trim();
    let lower = prefix.to_ascii_lowercase();

    let mut center_x = Length::Percent(50.0);
    let mut center_y = Length::Percent(50.0);
    let mut radius = Length::Percent(50.0);

    if let Some(position) = lower.find("at ") {
        let coords = prefix[position + 3..].trim();
        let parts: Vec<&str> = coords.split_whitespace().collect();
        if let Some(x) = parts.first().and_then(|value| parse_position(value)) {
            center_x = x;
        }
        if let Some(y) = parts.get(1).and_then(|value| parse_position(value)) {
            center_y = y;
        }
    }

    if let Some(shape_end) = lower.find(" at ") {
        let shape = prefix[..shape_end].trim();
        for token in shape.split_whitespace() {
            if let Ok(length) = Length::parse(token) {
                radius = length;
            }
        }
        if lower.contains("closest-side") {
            radius = Length::Percent(50.0);
        }
    }

    Ok((center_x, center_y, radius, consumed_ws + end))
}

fn parse_position(value: &str) -> Option<Length> {
    match value.to_ascii_lowercase().as_str() {
        "left" => Some(Length::Percent(0.0)),
        "right" => Some(Length::Percent(100.0)),
        "top" => Some(Length::Percent(0.0)),
        "bottom" => Some(Length::Percent(100.0)),
        "center" => Some(Length::Percent(50.0)),
        _ => Length::parse(value).ok(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vello::peniko::Color;

    #[test]
    fn parses_linear_gradient() {
        let gradient = Gradient::parse(
            "linear-gradient(135deg, #ff0000 0%, rgba(0,0,255,0.5) 100%)",
            Color::new([0.0, 0.0, 0.0, 0.0]),
        )
        .unwrap();
        assert!(matches!(gradient.kind, GradientKind::Linear { .. }));
        assert_eq!(gradient.stops.len(), 2);
        assert_eq!(gradient.stops[1].position, Some(1.0));
    }

    #[test]
    fn parses_radial_gradient() {
        let gradient = Gradient::parse(
            "radial-gradient(circle at 20% 30%, red, blue)",
            Color::new([0.0, 0.0, 0.0, 0.0]),
        )
        .unwrap();
        assert!(matches!(gradient.kind, GradientKind::Radial { .. }));
    }
}
