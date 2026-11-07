// SPDX-FileCopyrightText: 2026 Daniel
// SPDX-License-Identifier: GPL-3.0-or-later

//! CSS color parsing.
//!
//! Named colors, `#rgb`, `#rrggbb`, `#rrggbbaa`, `rgb()`, `rgba()`, `hsl()`,
//! `hsla()`, `transparent`, `currentColor` and the historical
//! `r, g, b, a` syntax used by older Widdgyy stylesheets are supported.

pub use vello::peniko::Color;
use vello::peniko::color::{Srgb, parse_color as peniko_parse_color};

pub fn parse(input: &str, current_color: Color) -> Option<Color> {
    let text = input.trim();

    if text.eq_ignore_ascii_case("transparent") {
        return Some(Color::new([0.0, 0.0, 0.0, 0.0]));
    }
    if text.eq_ignore_ascii_case("currentcolor") {
        return Some(current_color);
    }

    // Accept CSS-like quoted values: `"#88F54C"`.
    let unquoted = text.trim_matches(|c| c == '"' || c == '\'');
    if unquoted.len() < text.len() {
        return parse(unquoted, current_color);
    }

    if lower_starts_with(text, "color-mix(") {
        return parse_color_mix(text, current_color);
    }

    if let Ok(color) = peniko_parse_color(text) {
        return Some(color.to_alpha_color::<Srgb>());
    }

    // Legacy Widdgyy syntax: `255, 255, 255, 1` (the alpha channel may be
    // either 0..1 or 0..255).
    if text.contains(',') {
        let parts: Vec<&str> = text.split(',').map(str::trim).collect();
        if parts.len() == 3 || parts.len() == 4 {
            let mut channels = [0.0_f32; 4];
            channels[3] = 1.0;
            let mut valid = true;
            for (index, part) in parts.iter().enumerate() {
                match part.trim_end_matches('%').parse::<f32>() {
                    Ok(value) => {
                        let normalized = if part.ends_with('%') {
                            value / 100.0
                        } else if value > 1.0 {
                            value / 255.0
                        } else {
                            value
                        };
                        channels[index] = normalized.clamp(0.0, 1.0);
                    }
                    Err(_) => valid = false,
                }
            }
            if valid {
                return Some(Color::new(channels));
            }
        }
    }

    None
}

fn lower_starts_with(text: &str, prefix: &str) -> bool {
    text.len() >= prefix.len() && text[..prefix.len()].eq_ignore_ascii_case(prefix)
}

/// Parse `color-mix(in <space>, <color> [<pct>], <color> [<pct>])`. Colours are
/// mixed in sRGB; the space name is accepted for compatibility.
fn parse_color_mix(input: &str, current: Color) -> Option<Color> {
    let inner = input.strip_prefix("color-mix(").or_else(|| {
        let index = input.find('(')?;
        input.get(index + 1..)
    })?;
    let inner = inner.strip_suffix(')')?;
    let parts = split_top_level(inner);
    if parts.len() != 3 {
        return None;
    }
    let space = parts[0].trim().to_ascii_lowercase();
    if !space.starts_with("in ") {
        return None;
    }
    let (a, weight_a) = parse_weighted_color(parts[1], current)?;
    let (b, weight_b) = parse_weighted_color(parts[2], current)?;
    let (weight_a, weight_b) = match (weight_a, weight_b) {
        (Some(a), Some(b)) if a + b > 0.0 => (a / (a + b), b / (a + b)),
        (Some(a), None) => (a, 1.0 - a),
        (None, Some(b)) => (1.0 - b, b),
        _ => (0.5, 0.5),
    };
    let [ar, ag, ab, aa] = a.components;
    let [br, bg, bb, ba] = b.components;
    Some(Color::new([
        ar * weight_a + br * weight_b,
        ag * weight_a + bg * weight_b,
        ab * weight_a + bb * weight_b,
        aa * weight_a + ba * weight_b,
    ]))
}

fn parse_weighted_color(input: &str, current: Color) -> Option<(Color, Option<f32>)> {
    let input = input.trim();
    let mut color_text = input;
    let mut weight = None;
    if input.ends_with('%')
        && let Some(index) = input.rfind(|character: char| character.is_whitespace())
    {
        let percentage = input[index + 1..].trim_end_matches('%');
        if let Ok(value) = percentage.parse::<f32>() {
            weight = Some((value / 100.0).clamp(0.0, 1.0));
            color_text = input[..index].trim();
        }
    }
    parse(color_text, current).map(|color| (color, weight))
}

fn split_top_level(input: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0usize;
    let mut start = 0usize;
    for (index, character) in input.char_indices() {
        match character {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                parts.push(&input[start..index]);
                start = index + 1;
            }
            _ => {}
        }
    }
    parts.push(&input[start..]);
    parts
}

pub fn with_opacity(color: Color, opacity: f32) -> Color {
    let [r, g, b, a] = color.components;
    Color::new([r, g, b, a * opacity.clamp(0.0, 1.0)])
}

pub fn mix(a: Color, b: Color, t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);
    let [ar, ag, ab, aa] = a.components;
    let [br, bg, bb, ba] = b.components;
    Color::new([
        ar + (br - ar) * t,
        ag + (bg - ag) * t,
        ab + (bb - ab) * t,
        aa + (ba - aa) * t,
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(color: Color, rgba: [f32; 4]) {
        let c = color.components;
        for index in 0..4 {
            assert!(
                (c[index] - rgba[index]).abs() < 0.01,
                "channel {index}: {:?} != {:?}",
                c,
                rgba
            );
        }
    }

    #[test]
    fn parses_hex_and_named() {
        approx(
            parse("#ff0000", Color::new([0.0; 4])).unwrap(),
            [1.0, 0.0, 0.0, 1.0],
        );
        approx(
            parse("red", Color::new([0.0; 4])).unwrap(),
            [1.0, 0.0, 0.0, 1.0],
        );
    }

    #[test]
    fn parses_legacy_numeric() {
        approx(
            parse("255, 128, 0, 0.5", Color::new([0.0; 4])).unwrap(),
            [1.0, 128.0 / 255.0, 0.0, 0.5],
        );
    }

    #[test]
    fn parses_rgba_and_transparent() {
        approx(
            parse("rgba(0, 255, 0, 0.25)", Color::new([0.0; 4])).unwrap(),
            [0.0, 1.0, 0.0, 0.25],
        );
        approx(
            parse("transparent", Color::new([0.0; 4])).unwrap(),
            [0.0, 0.0, 0.0, 0.0],
        );
    }
}

#[cfg(test)]
mod mix_tests {
    use super::*;

    #[test]
    fn mixes_srgb_colours_with_percentages() {
        let red = Color::new([1.0, 0.0, 0.0, 1.0]);
        let blue = Color::new([0.0, 0.0, 1.0, 1.0]);
        let mixed = parse("color-mix(in srgb, red 25%, blue 75%)", red).unwrap();
        assert!((mixed.components[0] - 0.25).abs() < 0.01);
        assert!((mixed.components[2] - 0.75).abs() < 0.01);

        let even = parse("color-mix(in srgb, red, blue)", red).unwrap();
        assert!((even.components[0] - 0.5).abs() < 0.01);
        assert!((even.components[2] - 0.5).abs() < 0.01);
        let _ = blue;
    }
}
