// SPDX-FileCopyrightText: 2026 Daniel
// SPDX-License-Identifier: GPL-3.0-or-later

//! `box-shadow` and `text-shadow` values.

use super::color::Color;
use super::length::Length;

#[derive(Clone, Debug, PartialEq)]
pub struct Shadow {
    pub offset_x: Length,
    pub offset_y: Length,
    pub blur: Length,
    pub spread: Length,
    pub color: Color,
    pub inset: bool,
}

impl Shadow {
    /// Parse one shadow. `current_color` is used when no color is specified.
    pub fn parse(input: &str, current_color: Color) -> Result<Self, String> {
        let mut inset = false;
        let mut lengths = Vec::new();
        let mut color = None;
        let mut tail = String::new();

        // Color may appear at the start or the end. Split into length-like
        // tokens while keeping function calls (`rgba(...)`) together.
        for token in split_ws(input) {
            let lower = token.to_ascii_lowercase();
            if lower == "inset" {
                inset = true;
            } else if lower.starts_with("rgb")
                || lower.starts_with("hsl")
                || lower.starts_with('#')
                || lower == "transparent"
                || lower == "currentcolor"
                || token.starts_with("var(")
                || is_named_color(&lower)
            {
                color = Some(token);
            } else if let Ok(length) = Length::parse(&token) {
                lengths.push(length);
            } else {
                tail.push_str(&token);
            }
        }

        if lengths.len() < 2 || lengths.len() > 4 {
            return Err(format!("invalid shadow: {input}"));
        }

        let color_text = color.unwrap_or_else(|| {
            if tail.is_empty() {
                "rgba(0,0,0,0.35)".to_string()
            } else {
                tail
            }
        });

        Ok(Self {
            offset_x: lengths[0].clone(),
            offset_y: lengths[1].clone(),
            blur: lengths.get(2).cloned().unwrap_or(Length::Zero),
            spread: lengths.get(3).cloned().unwrap_or(Length::Zero),
            color: super::color::parse(&color_text, current_color).unwrap_or(current_color),
            inset,
        })
    }

    pub fn parse_list(input: &str, current_color: Color) -> Result<Vec<Self>, String> {
        split_commas(input)
            .into_iter()
            .filter(|part| !part.trim().is_empty())
            .map(|part| Self::parse(&part, current_color))
            .collect()
    }
}

fn is_named_color(value: &str) -> bool {
    matches!(
        value,
        "black"
            | "white"
            | "red"
            | "green"
            | "blue"
            | "yellow"
            | "cyan"
            | "magenta"
            | "gray"
            | "grey"
            | "orange"
            | "purple"
            | "pink"
            | "lime"
            | "navy"
            | "teal"
            | "olive"
            | "maroon"
            | "silver"
    )
}

/// Split on whitespace outside of parentheses.
pub fn split_ws(input: &str) -> Vec<String> {
    let mut tokens = Vec::new();
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
            }
            c if c.is_whitespace() && depth == 0 => {
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
            }
            c => current.push(c),
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

/// Split a comma separated list at top level.
pub fn split_commas(input: &str) -> Vec<String> {
    let mut parts = Vec::new();
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
            }
            ',' if depth == 0 => {
                parts.push(std::mem::take(&mut current));
            }
            c => current.push(c),
        }
    }
    parts.push(current);
    parts
}

#[cfg(test)]
mod tests {
    use super::*;
    use vello::peniko::Color;

    #[test]
    fn parses_box_shadow_list() {
        let shadows = Shadow::parse_list(
            "0 10px 30px rgba(0,0,0,0.4), inset 0 0 2px #fff",
            Color::new([0.0, 0.0, 0.0, 0.0]),
        )
        .unwrap();
        assert_eq!(shadows.len(), 2);
        assert!(shadows[1].inset);
        assert_eq!(shadows[0].offset_y, Length::Px(10.0));
        assert_eq!(shadows[0].blur, Length::Px(30.0));
    }
}
