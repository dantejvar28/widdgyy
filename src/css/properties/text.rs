// SPDX-FileCopyrightText: 2026 Daniel
// SPDX-License-Identifier: GPL-3.0-or-later

//! Typography and text layout properties.

use super::{
    Declarations, FontStyle, LineHeight, TextAlign, TextDecoration, TextOverflow, TextTransform,
    WhiteSpace, parse_color, split_commas, split_ws,
};
use crate::css::values::{Color, Length, Shadow};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum FontVariantCaps {
    #[default]
    Normal,
    SmallCaps,
    AllSmallCaps,
    PetiteCaps,
    AllPetiteCaps,
    Unicase,
    TitlingCaps,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum OverflowWrap {
    #[default]
    Normal,
    BreakWord,
    Anywhere,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum WordBreak {
    #[default]
    Normal,
    BreakAll,
    KeepAll,
    BreakWord,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Direction {
    #[default]
    Ltr,
    Rtl,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum VerticalAlign {
    #[default]
    Baseline,
    Top,
    Middle,
    Bottom,
    TextTop,
    TextBottom,
    Sub,
    Super,
}

pub fn apply(decl: &mut Declarations, name: &str, value: &str, current: Color) -> bool {
    match name {
        "color" => {
            decl.color = parse_color(value, current);
            true
        }
        "font-family" => {
            decl.font_family = Some(parse_font_families(value));
            true
        }
        "font-size" => {
            if let Some(length) = parse_font_size(value) {
                decl.font_size = Some(length);
            }
            true
        }
        "font-weight" => {
            decl.font_weight = parse_font_weight(value);
            true
        }
        "font-style" => {
            decl.font_style = Some(parse_font_style(value));
            true
        }
        "font-stretch" => {
            decl.font_stretch = parse_font_stretch(value);
            true
        }
        "font-feature-settings" => {
            decl.font_features = Some(parse_font_features(value));
            true
        }
        "font-variant" | "font-variant-caps" => {
            decl.font_variant_caps = Some(match value.to_ascii_lowercase().as_str() {
                "small-caps" => FontVariantCaps::SmallCaps,
                "all-small-caps" => FontVariantCaps::AllSmallCaps,
                "petite-caps" => FontVariantCaps::PetiteCaps,
                "all-petite-caps" => FontVariantCaps::AllPetiteCaps,
                "unicase" => FontVariantCaps::Unicase,
                "titling-caps" => FontVariantCaps::TitlingCaps,
                _ => FontVariantCaps::Normal,
            });
            true
        }
        "line-height" => {
            decl.line_height = Some(parse_line_height(value));
            true
        }
        "letter-spacing" => {
            decl.letter_spacing = if value.eq_ignore_ascii_case("normal") {
                Some(Length::Zero)
            } else {
                Length::parse(value).ok()
            };
            true
        }
        "word-spacing" => {
            decl.word_spacing = if value.eq_ignore_ascii_case("normal") {
                Some(Length::Zero)
            } else {
                Length::parse(value).ok()
            };
            true
        }
        "text-align" => {
            decl.text_align = Some(parse_text_align(value));
            true
        }
        "text-align-last" => {
            decl.text_align_last = Some(parse_text_align(value));
            true
        }
        "line-clamp" | "-webkit-line-clamp" => {
            decl.line_clamp = value
                .trim()
                .parse::<usize>()
                .ok()
                .filter(|lines| *lines > 0);
            true
        }
        "-webkit-box-orient" | "-webkit-box-pack" | "-webkit-box-align" => true,
        "text-decoration" => {
            let decoration = decl
                .text_decoration
                .get_or_insert_with(TextDecoration::default);
            for token in split_ws(value) {
                match token.to_ascii_lowercase().as_str() {
                    "none" => {
                        decoration.underline = false;
                        decoration.overline = false;
                        decoration.line_through = false;
                    }
                    "underline" => decoration.underline = true,
                    "overline" => decoration.overline = true,
                    "line-through" => decoration.line_through = true,
                    "solid" => decoration.style = super::BorderStyle::Solid,
                    "dashed" => decoration.style = super::BorderStyle::Dashed,
                    "dotted" => decoration.style = super::BorderStyle::Dotted,
                    "double" => decoration.style = super::BorderStyle::Double,
                    other => {
                        if let Some(color) = parse_color(other, current) {
                            decoration.color = Some(color);
                        } else if let Ok(length) = Length::parse(other) {
                            decoration.thickness = Some(length);
                        }
                    }
                }
            }
            true
        }
        "text-decoration-line" => {
            let decoration = decl
                .text_decoration
                .get_or_insert_with(TextDecoration::default);
            decoration.underline = false;
            decoration.overline = false;
            decoration.line_through = false;
            for token in split_ws(value) {
                match token.to_ascii_lowercase().as_str() {
                    "underline" => decoration.underline = true,
                    "overline" => decoration.overline = true,
                    "line-through" => decoration.line_through = true,
                    _ => {}
                }
            }
            true
        }
        "text-decoration-color" => {
            let decoration = decl
                .text_decoration
                .get_or_insert_with(TextDecoration::default);
            decoration.color = parse_color(value, current);
            true
        }
        "text-decoration-style" => {
            let decoration = decl
                .text_decoration
                .get_or_insert_with(TextDecoration::default);
            decoration.style =
                super::border::parse_style(value).unwrap_or(super::BorderStyle::Solid);
            true
        }
        "text-decoration-thickness" => {
            let decoration = decl
                .text_decoration
                .get_or_insert_with(TextDecoration::default);
            decoration.thickness = Length::parse(value).ok();
            true
        }
        "text-transform" => {
            decl.text_transform = Some(match value.to_ascii_lowercase().as_str() {
                "uppercase" => TextTransform::Uppercase,
                "lowercase" => TextTransform::Lowercase,
                "capitalize" => TextTransform::Capitalize,
                _ => TextTransform::None,
            });
            true
        }
        "text-indent" => {
            decl.text_indent = Length::parse(value).ok();
            true
        }
        "white-space" => {
            decl.white_space = Some(parse_white_space(value));
            true
        }
        "text-wrap" => {
            decl.white_space = Some(if value.eq_ignore_ascii_case("nowrap") {
                WhiteSpace::NoWrap
            } else {
                WhiteSpace::Normal
            });
            true
        }
        "overflow-wrap" | "word-wrap" => {
            decl.overflow_wrap = Some(match value.to_ascii_lowercase().as_str() {
                "break-word" => OverflowWrap::BreakWord,
                "anywhere" => OverflowWrap::Anywhere,
                _ => OverflowWrap::Normal,
            });
            true
        }
        "word-break" => {
            decl.word_break = Some(match value.to_ascii_lowercase().as_str() {
                "break-all" => WordBreak::BreakAll,
                "keep-all" => WordBreak::KeepAll,
                "break-word" => WordBreak::BreakWord,
                _ => WordBreak::Normal,
            });
            true
        }
        "text-overflow" => {
            decl.text_overflow = Some(if value.eq_ignore_ascii_case("ellipsis") {
                TextOverflow::Ellipsis
            } else {
                TextOverflow::Clip
            });
            true
        }
        "text-shadow" => {
            decl.text_shadow = Shadow::parse_list(value, current).ok();
            true
        }
        "direction" => {
            decl.direction = Some(if value.eq_ignore_ascii_case("rtl") {
                Direction::Rtl
            } else {
                Direction::Ltr
            });
            true
        }
        "vertical-align" => {
            decl.vertical_align = Some(match value.to_ascii_lowercase().as_str() {
                "top" => VerticalAlign::Top,
                "middle" => VerticalAlign::Middle,
                "bottom" => VerticalAlign::Bottom,
                "text-top" => VerticalAlign::TextTop,
                "text-bottom" => VerticalAlign::TextBottom,
                "sub" => VerticalAlign::Sub,
                "super" => VerticalAlign::Super,
                _ => VerticalAlign::Baseline,
            });
            true
        }
        "font" => {
            apply_font_shorthand(decl, value, current);
            true
        }
        "text-rendering" | "font-variation-settings" => true,
        _ => false,
    }
}

fn parse_font_families(value: &str) -> Vec<String> {
    split_commas(value)
        .into_iter()
        .map(|part| {
            part.trim()
                .trim_matches('"')
                .trim_matches('\'')
                .trim()
                .to_string()
        })
        .filter(|part| !part.is_empty())
        .collect()
}

fn parse_font_size(value: &str) -> Option<Length> {
    let text = value.trim().to_ascii_lowercase();
    let keyword = match text.as_str() {
        "xx-small" => Some(9.0),
        "x-small" => Some(10.0),
        "small" => Some(13.0),
        "medium" => Some(16.0),
        "large" => Some(18.0),
        "x-large" => Some(24.0),
        "xx-large" => Some(32.0),
        "xxx-large" => Some(48.0),
        "smaller" => Some(13.0),
        "larger" => Some(19.0),
        _ => None,
    };
    keyword
        .map(Length::Px)
        .or_else(|| Length::parse(value).ok())
}

fn parse_font_weight(value: &str) -> Option<u16> {
    let value = value.trim().trim_matches('"').trim_matches('\'');
    Some(match value.to_ascii_lowercase().as_str() {
        "thin" | "hairline" => 100,
        "extralight" | "ultralight" => 200,
        "light" => 300,
        "normal" | "regular" | "book" => 400,
        "medium" => 500,
        "semibold" | "demibold" => 600,
        "bold" => 700,
        "extrabold" | "ultrabold" => 800,
        "black" | "heavy" => 900,
        "bolder" => 800,
        "lighter" => 200,
        other => other.parse::<u16>().ok()?.clamp(1, 1000),
    })
}

fn parse_font_style(value: &str) -> FontStyle {
    let value = value.trim().trim_matches('"').trim_matches('\'');
    match value.to_ascii_lowercase().as_str() {
        "italic" => FontStyle::Italic,
        "oblique" => FontStyle::Oblique,
        _ => FontStyle::Normal,
    }
}

fn parse_font_stretch(value: &str) -> Option<f32> {
    let keyword = match value.trim().to_ascii_lowercase().as_str() {
        "ultra-condensed" => Some(0.50),
        "extra-condensed" => Some(0.625),
        "condensed" => Some(0.75),
        "semi-condensed" => Some(0.875),
        "normal" => Some(1.0),
        "semi-expanded" => Some(1.125),
        "expanded" => Some(1.25),
        "extra-expanded" => Some(1.5),
        "ultra-expanded" => Some(2.0),
        _ => None,
    };
    if keyword.is_some() {
        return keyword;
    }
    let text = value.trim();
    if let Some(percent) = text.strip_suffix('%') {
        return percent.parse::<f32>().ok().map(|value| value / 100.0);
    }
    text.parse::<f32>()
        .ok()
        .map(|value| if value > 10.0 { value / 100.0 } else { value })
}

fn parse_font_features(value: &str) -> Vec<(String, u32)> {
    if value.trim().eq_ignore_ascii_case("normal") {
        return Vec::new();
    }
    let mut features = Vec::new();
    for part in super::split_commas(value) {
        let tokens = super::split_ws(&part);
        let Some(tag) = tokens.first() else {
            continue;
        };
        let tag = tag.trim().trim_matches('"').trim_matches('\'');
        if tag.len() != 4 {
            continue;
        }
        let enabled = tokens
            .get(1)
            .and_then(|token| token.parse::<u32>().ok())
            .unwrap_or(1);
        features.push((tag.to_string(), enabled));
    }
    features
}

fn parse_line_height(value: &str) -> LineHeight {
    if value.eq_ignore_ascii_case("normal") {
        return LineHeight::Normal;
    }
    if let Ok(number) = value.trim().parse::<f32>() {
        return LineHeight::Number(number);
    }
    Length::parse(value)
        .map(LineHeight::Length)
        .unwrap_or(LineHeight::Normal)
}

fn parse_text_align(value: &str) -> TextAlign {
    match value.trim().to_ascii_lowercase().as_str() {
        "right" => TextAlign::Right,
        "center" => TextAlign::Center,
        "justify" => TextAlign::Justify,
        "start" => TextAlign::Start,
        "end" => TextAlign::End,
        "auto" => TextAlign::Auto,
        _ => TextAlign::Left,
    }
}

fn parse_white_space(value: &str) -> WhiteSpace {
    match value.trim().to_ascii_lowercase().as_str() {
        "nowrap" => WhiteSpace::NoWrap,
        "pre" => WhiteSpace::Pre,
        "pre-wrap" => WhiteSpace::PreWrap,
        "pre-line" => WhiteSpace::PreLine,
        "break-spaces" => WhiteSpace::BreakSpaces,
        _ => WhiteSpace::Normal,
    }
}

fn apply_font_shorthand(decl: &mut Declarations, value: &str, current: Color) {
    let tokens = split_ws(value);
    let mut index = 0;
    let mut size = None;
    let mut line_height = None;

    while index < tokens.len() {
        let token = &tokens[index];
        let lower = token.to_ascii_lowercase();
        if matches!(lower.as_str(), "italic" | "oblique" | "normal") {
            if lower != "normal" {
                decl.font_style = Some(parse_font_style(&lower));
            }
        } else if matches!(
            lower.as_str(),
            "bold"
                | "bolder"
                | "lighter"
                | "100"
                | "200"
                | "300"
                | "400"
                | "500"
                | "600"
                | "700"
                | "800"
                | "900"
        ) {
            decl.font_weight = parse_font_weight(&lower);
        } else if lower.starts_with("small-caps") {
            decl.font_variant_caps = Some(FontVariantCaps::SmallCaps);
        } else if let Some(parsed) = parse_font_size(token) {
            size = Some(parsed);
            index += 1;
            if tokens.get(index).is_some_and(|token| token.contains('/')) {
                let parts: Vec<&str> = tokens[index].split('/').collect();
                if let Some(first) = parts.first().and_then(|part| parse_font_size(part)) {
                    size = Some(first);
                }
                if let Some(second) = parts.get(1) {
                    line_height = Some(parse_line_height(second));
                }
                index += 1;
            } else if index < tokens.len() && tokens[index] == "/" {
                index += 1;
                if let Some(token) = tokens.get(index) {
                    line_height = Some(parse_line_height(token));
                    index += 1;
                }
            }
            break;
        }
        index += 1;
    }

    if let Some(size) = size {
        decl.font_size = Some(size);
    }
    if let Some(line_height) = line_height {
        decl.line_height = Some(line_height);
    }
    let families = tokens[index..].join(" ");
    if !families.trim().is_empty() {
        decl.font_family = Some(parse_font_families(&families));
    }
    let _ = current;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_keyword_weights() {
        let color = Color::new([1.0, 1.0, 1.0, 1.0]);
        for (value, expected) in [
            ("light", 300),
            ("semibold", 600),
            ("heavy", 900),
            ("bold", 700),
            ("normal", 400),
            ("\"light\"", 300),
        ] {
            let mut declarations = Declarations::default();
            assert!(declarations.apply("font-weight", value, color));
            assert_eq!(declarations.font_weight, Some(expected), "{value}");
        }
    }
}
