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
