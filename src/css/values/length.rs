// SPDX-FileCopyrightText: 2026 Daniel
// SPDX-License-Identifier: GPL-3.0-or-later

//! CSS lengths, percentages, `calc()` expressions and edge helpers.

use std::fmt;

/// Axis used when resolving a percentage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    Horizontal,
    Vertical,
}

/// Everything needed to turn a relative [`Length`] into logical pixels.
#[derive(Clone, Copy, Debug)]
pub struct ResolveContext {
    pub viewport_width: f32,
    pub viewport_height: f32,
    pub font_size: f32,
    pub root_font_size: f32,
    pub basis_width: f32,
    pub basis_height: f32,
}

impl Default for ResolveContext {
    fn default() -> Self {
        Self {
            viewport_width: 1920.0,
            viewport_height: 1080.0,
            font_size: 16.0,
            root_font_size: 16.0,
            basis_width: 1920.0,
            basis_height: 1080.0,
        }
    }
}

impl ResolveContext {
    pub fn basis(&self, axis: Axis) -> f32 {
        match axis {
            Axis::Horizontal => self.basis_width,
            Axis::Vertical => self.basis_height,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unit {
    Px,
    Percent,
    Em,
    Rem,
    Ex,
    Ch,
    Vw,
    Vh,
    Vmin,
    Vmax,
    Pt,
    Pc,
    In,
    Cm,
    Mm,
    Fr,
}

impl Unit {
    pub fn parse(value: &str) -> Option<Self> {
        Some(match value.trim().to_ascii_lowercase().as_str() {
            "px" | "" => Self::Px,
            "%" => Self::Percent,
            "em" => Self::Em,
            "rem" => Self::Rem,
            "ex" => Self::Ex,
            "ch" => Self::Ch,
            "vw" => Self::Vw,
            "vh" => Self::Vh,
            "vmin" => Self::Vmin,
            "vmax" => Self::Vmax,
            "pt" => Self::Pt,
            "pc" => Self::Pc,
            "in" => Self::In,
            "cm" => Self::Cm,
            "mm" => Self::Mm,
            "fr" => Self::Fr,
            _ => return None,
        })
    }
}

/// A `calc()` expression. Kept as an AST so it can be resolved against the
/// final containing block instead of being flattened too early.
#[derive(Clone, Debug, PartialEq)]
pub enum Calc {
    Value(Length),
    Add(Box<Calc>, Box<Calc>),
    Sub(Box<Calc>, Box<Calc>),
    Mul(Box<Calc>, f32),
    Div(Box<Calc>, f32),
}

impl Calc {
    pub fn resolve(&self, ctx: &ResolveContext, axis: Axis) -> f32 {
        match self {
            Self::Value(length) => length.resolve(ctx, axis),
            Self::Add(a, b) => a.resolve(ctx, axis) + b.resolve(ctx, axis),
            Self::Sub(a, b) => a.resolve(ctx, axis) - b.resolve(ctx, axis),
            Self::Mul(a, b) => a.resolve(ctx, axis) * b,
            Self::Div(a, b) => {
                if *b == 0.0 {
                    0.0
                } else {
                    a.resolve(ctx, axis) / b
                }
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum LengthFunc {
    Min(Vec<Length>),
    Max(Vec<Length>),
    Clamp(Length, Length, Length),
}

/// A CSS length. `Auto` is accepted for margins, insets and sizes.
#[derive(Clone, Debug, PartialEq, Default)]
pub enum Length {
    Px(f32),
    Percent(f32),
    Em(f32),
    Rem(f32),
    Ex(f32),
    Ch(f32),
    Vw(f32),
    Vh(f32),
    Vmin(f32),
    Vmax(f32),
    Pt(f32),
    Pc(f32),
    In(f32),
    Cm(f32),
    Mm(f32),
    Fr(f32),
    Auto,
    Calc(Box<Calc>),
    Func(Box<LengthFunc>),
    #[default]
    Zero,
}

impl Length {
    pub const fn px(value: f32) -> Self {
        Self::Px(value)
    }

    pub const fn percent(value: f32) -> Self {
        Self::Percent(value)
    }

    pub fn is_auto(&self) -> bool {
        matches!(self, Self::Auto)
    }

    pub fn is_fr(&self) -> bool {
        matches!(self, Self::Fr(_))
    }

    /// Resolve to logical pixels. `auto` resolves to `0.0`; callers that need
    /// to distinguish auto must check [`Self::is_auto`] first.
    pub fn resolve(&self, ctx: &ResolveContext, axis: Axis) -> f32 {
        let basis = ctx.basis(axis);
        match self {
            Self::Px(v) | Self::Pt(v) => *v,
            Self::Percent(v) => basis * (*v / 100.0),
            Self::Em(v) => ctx.font_size * *v,
            Self::Rem(v) => ctx.root_font_size * *v,
            Self::Ex(v) | Self::Ch(v) => ctx.font_size * 0.5 * *v,
            Self::Vw(v) => ctx.viewport_width * (*v / 100.0),
            Self::Vh(v) => ctx.viewport_height * (*v / 100.0),
            Self::Vmin(v) => ctx.viewport_width.min(ctx.viewport_height) * (*v / 100.0),
            Self::Vmax(v) => ctx.viewport_width.max(ctx.viewport_height) * (*v / 100.0),
            Self::Pc(v) => 16.0 * *v,
            Self::In(v) => 96.0 * *v,
            Self::Cm(v) => 96.0 / 2.54 * *v,
            Self::Mm(v) => 96.0 / 25.4 * *v,
            Self::Fr(v) => *v,
            Self::Auto | Self::Zero => 0.0,
            Self::Calc(calc) => calc.resolve(ctx, axis),
            Self::Func(func) => match func.as_ref() {
                LengthFunc::Min(values) => values
                    .iter()
                    .map(|value| value.resolve(ctx, axis))
                    .fold(f32::INFINITY, f32::min),
                LengthFunc::Max(values) => values
                    .iter()
                    .map(|value| value.resolve(ctx, axis))
                    .fold(f32::NEG_INFINITY, f32::max),
                LengthFunc::Clamp(min, value, max) => value
                    .resolve(ctx, axis)
                    .clamp(min.resolve(ctx, axis), max.resolve(ctx, axis)),
            },
        }
    }

    pub fn parse(input: &str) -> Result<Self, String> {
        let text = input.trim();
        if text.is_empty() {
            return Err("empty length".to_string());
        }
        if text.eq_ignore_ascii_case("auto") {
            return Ok(Self::Auto);
        }
        let lower = text.to_ascii_lowercase();
        if lower.starts_with("calc(") {
            return parse_calc(text).map(|c| Self::Calc(Box::new(c)));
        }
        if let Some((name, args)) = parse_function(text) {
            let values = split_args(args)
                .into_iter()
                .map(Length::parse)
                .collect::<Result<Vec<_>, _>>()?;
            return match name.as_str() {
                "min" if !values.is_empty() => Ok(Self::Func(Box::new(LengthFunc::Min(values)))),
                "max" if !values.is_empty() => Ok(Self::Func(Box::new(LengthFunc::Max(values)))),
                "clamp" if values.len() == 3 => Ok(Self::Func(Box::new(LengthFunc::Clamp(
                    values[0].clone(),
                    values[1].clone(),
                    values[2].clone(),
                )))),
                _ => parse_number_unit(text),
            };
        }
        parse_number_unit(text)
    }
}

fn parse_number_unit(text: &str) -> Result<Length, String> {
    let text = text.trim();
    if text == "0" {
        return Ok(Length::Zero);
    }
    let (number, unit) = split_number_unit(text)?;
    let value = number
        .parse::<f32>()
        .map_err(|_| format!("invalid number in length: {text}"))?;
    Ok(match unit {
        Unit::Px => Length::Px(value),
        Unit::Percent => Length::Percent(value),
        Unit::Em => Length::Em(value),
        Unit::Rem => Length::Rem(value),
        Unit::Ex => Length::Ex(value),
        Unit::Ch => Length::Ch(value),
        Unit::Vw => Length::Vw(value),
        Unit::Vh => Length::Vh(value),
        Unit::Vmin => Length::Vmin(value),
        Unit::Vmax => Length::Vmax(value),
        Unit::Pt => Length::Pt(value * 96.0 / 72.0),
        Unit::Pc => Length::Pc(value),
        Unit::In => Length::In(value),
        Unit::Cm => Length::Cm(value),
        Unit::Mm => Length::Mm(value),
        Unit::Fr => Length::Fr(value),
    })
}

fn split_number_unit(text: &str) -> Result<(&str, Unit), String> {
    let text = text.trim();
    let mut split = text.len();
    for (index, ch) in text.char_indices() {
        if ch.is_ascii_alphabetic() || ch == '%' {
            split = index;
            break;
        }
    }
    let (number, unit) = text.split_at(split);
    let unit = Unit::parse(unit).ok_or_else(|| format!("unsupported unit in: {text}"))?;
    Ok((number.trim(), unit))
}

/// Tiny recursive-descent parser for the common `calc()` forms.
fn parse_calc(text: &str) -> Result<Calc, String> {
    let inner = text
        .trim()
        .strip_prefix("calc(")
        .or_else(|| text.trim().strip_prefix("CALC("))
        .ok_or_else(|| format!("invalid calc(): {text}"))?;
    let inner = inner
        .strip_suffix(')')
        .ok_or_else(|| format!("unbalanced calc(): {text}"))?;
    let mut parser = CalcParser {
        input: inner.as_bytes(),
        position: 0,
    };
    let value = parser.expression()?;
    parser.skip_ws();
    if parser.position != parser.input.len() {
        return Err(format!("trailing data in calc(): {text}"));
    }
    Ok(value)
}

struct CalcParser<'a> {
    input: &'a [u8],
    position: usize,
}

impl CalcParser<'_> {
    fn skip_ws(&mut self) {
        while self
            .input
            .get(self.position)
            .is_some_and(|b| b.is_ascii_whitespace())
        {
            self.position += 1;
        }
    }

    fn peek(&mut self) -> Option<u8> {
        self.skip_ws();
        self.input.get(self.position).copied()
    }

    fn expression(&mut self) -> Result<Calc, String> {
        let mut left = self.term()?;
        loop {
            match self.peek() {
                Some(b'+') => {
                    self.position += 1;
                    let right = self.term()?;
                    left = Calc::Add(Box::new(left), Box::new(right));
                }
                Some(b'-') => {
                    self.position += 1;
                    let right = self.term()?;
                    left = Calc::Sub(Box::new(left), Box::new(right));
                }
                _ => break,
            }
        }
        Ok(left)
    }

    fn term(&mut self) -> Result<Calc, String> {
        let mut left = self.factor()?;
        loop {
            match self.peek() {
                Some(b'*') => {
                    self.position += 1;
                    let right = self.factor()?;
                    let right = match right {
                        Calc::Value(Length::Px(v)) => v,
                        Calc::Value(Length::Zero) => 0.0,
                        _ => return Err("calc() multiplication requires a number".to_string()),
                    };
                    left = Calc::Mul(Box::new(left), right);
                }
                Some(b'/') => {
                    self.position += 1;
                    let right = self.factor()?;
                    let right = match right {
                        Calc::Value(Length::Px(v)) => v,
                        Calc::Value(Length::Zero) => 0.0,
                        _ => return Err("calc() division requires a number".to_string()),
                    };
                    left = Calc::Div(Box::new(left), right);
                }
                _ => break,
            }
        }
        Ok(left)
    }

    fn factor(&mut self) -> Result<Calc, String> {
        self.skip_ws();
        match self.peek() {
            Some(b'(') => {
                self.position += 1;
                let value = self.expression()?;
                if self.peek() != Some(b')') {
                    return Err("missing ) in calc()".to_string());
                }
                self.position += 1;
                Ok(value)
            }
            Some(b'-') => {
                self.position += 1;
                Ok(Calc::Mul(Box::new(self.factor()?), -1.0))
            }
            _ => {
                let start = self.position;
                while let Some(byte) = self.input.get(self.position) {
                    if byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'%' | b'+' | b'-') {
                        // A sign is only part of the number at the start.
                        if matches!(byte, b'+' | b'-') && self.position != start {
                            break;
                        }
                        self.position += 1;
                    } else {
                        break;
                    }
                }
                let token = std::str::from_utf8(&self.input[start..self.position])
                    .map_err(|_| "invalid utf-8 in calc()".to_string())?;
                if token.is_empty() {
                    return Err("expected value in calc()".to_string());
                }
                parse_number_unit(token).map(Calc::Value)
            }
        }
    }
}

/// A `width`, `height`, `min-*` or `max-*` value.
#[derive(Clone, Debug, PartialEq, Default)]
pub enum Size {
    #[default]
    Auto,
    None,
    Length(Length),
    MinContent,
    MaxContent,
    FitContent(Length),
    Stretch,
}

impl Size {
    pub fn parse(input: &str) -> Result<Self, String> {
        let text = input.trim();
        match text.to_ascii_lowercase().as_str() {
            "auto" => Ok(Self::Auto),
            "none" => Ok(Self::None),
            "min-content" => Ok(Self::MinContent),
            "max-content" => Ok(Self::MaxContent),
            "stretch" | "-webkit-fill-available" => Ok(Self::Stretch),
            _ => {
                if let Some(inner) = text
                    .strip_prefix("fit-content(")
                    .and_then(|value| value.strip_suffix(')'))
                {
                    return Ok(Self::FitContent(Length::parse(inner)?));
                }
                Ok(Self::Length(Length::parse(text)?))
            }
        }
    }

    pub fn is_auto(&self) -> bool {
        matches!(self, Self::Auto)
    }

    /// Resolve a definite size. Percentages use `axis`; intrinsic keywords
    /// resolve to `None` so the caller can run intrinsic measurement.
    pub fn resolve(&self, ctx: &ResolveContext, axis: Axis) -> Option<f32> {
        match self {
            Self::Length(length) if !length.is_auto() => Some(length.resolve(ctx, axis).max(0.0)),
            Self::FitContent(length) => Some(length.resolve(ctx, axis).max(0.0)),
            _ => None,
        }
    }
}

/// Top/right/bottom/left values.
#[derive(Clone, Debug, PartialEq)]
pub struct Edges<T> {
    pub top: T,
    pub right: T,
    pub bottom: T,
    pub left: T,
}

impl<T: Clone> Edges<T> {
    pub fn all(value: T) -> Self {
        Self {
            top: value.clone(),
            right: value.clone(),
            bottom: value.clone(),
            left: value,
        }
    }

    pub fn from_1_to_4(values: &[T]) -> Option<Self> {
        match values {
            [a] => Some(Self::all(a.clone())),
            [a, b] => Some(Self {
                top: a.clone(),
                right: b.clone(),
                bottom: a.clone(),
                left: b.clone(),
            }),
            [a, b, c] => Some(Self {
                top: a.clone(),
                right: b.clone(),
                bottom: c.clone(),
                left: b.clone(),
            }),
            [a, b, c, d] => Some(Self {
                top: a.clone(),
                right: b.clone(),
                bottom: c.clone(),
                left: d.clone(),
            }),
            _ => None,
        }
    }

    pub fn as_array(&self) -> [&T; 4] {
        [&self.top, &self.right, &self.bottom, &self.left]
    }
}

impl Edges<Length> {
    pub fn resolve(&self, ctx: &ResolveContext, axis: Axis) -> Edges<f32> {
        Edges {
            top: self.top.resolve(ctx, axis),
            right: self.right.resolve(ctx, axis),
            bottom: self.bottom.resolve(ctx, axis),
            left: self.left.resolve(ctx, axis),
        }
    }
}

impl Edges<f32> {
    pub fn horizontal(&self) -> f32 {
        self.left + self.right
    }

    pub fn vertical(&self) -> f32 {
        self.top + self.bottom
    }
}

impl<T: Default + Clone> Default for Edges<T> {
    fn default() -> Self {
        Self::all(T::default())
    }
}

impl fmt::Display for Length {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Px(v) | Self::Pt(v) => write!(f, "{v}px"),
            Self::Percent(v) => write!(f, "{v}%"),
            Self::Em(v) => write!(f, "{v}em"),
            Self::Rem(v) => write!(f, "{v}rem"),
            Self::Ex(v) => write!(f, "{v}ex"),
            Self::Ch(v) => write!(f, "{v}ch"),
            Self::Vw(v) => write!(f, "{v}vw"),
            Self::Vh(v) => write!(f, "{v}vh"),
            Self::Vmin(v) => write!(f, "{v}vmin"),
            Self::Vmax(v) => write!(f, "{v}vmax"),
            Self::Pc(v) => write!(f, "{v}pc"),
            Self::In(v) => write!(f, "{v}in"),
            Self::Cm(v) => write!(f, "{v}cm"),
            Self::Mm(v) => write!(f, "{v}mm"),
            Self::Fr(v) => write!(f, "{v}fr"),
            Self::Auto => write!(f, "auto"),
            Self::Zero => write!(f, "0"),
            Self::Calc(_) => write!(f, "calc(...)"),
            Self::Func(_) => write!(f, "min/max/clamp(...)"),
        }
    }
}

fn parse_function(text: &str) -> Option<(String, &str)> {
    let open = text.find('(')?;
    let name = text[..open].trim().to_ascii_lowercase();
    if !text.trim_end().ends_with(')') {
        return None;
    }
    let close = text.rfind(')')?;
    if close <= open {
        return None;
    }
    Some((name, &text[open + 1..close]))
}

fn split_args(input: &str) -> Vec<&str> {
    let mut result = Vec::new();
    let mut start = 0;
    let mut depth = 0_i32;
    for (index, ch) in input.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => depth -= 1,
            ',' if depth == 0 => {
                result.push(input[start..index].trim());
                start = index + 1;
            }
            _ => {}
        }
    }
    result.push(input[start..].trim());
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_units() {
        assert_eq!(Length::parse("12px").unwrap(), Length::Px(12.0));
        assert_eq!(Length::parse("50%").unwrap(), Length::Percent(50.0));
        assert_eq!(Length::parse("-2em").unwrap(), Length::Em(-2.0));
        assert_eq!(Length::parse("auto").unwrap(), Length::Auto);
        assert_eq!(Length::parse("0").unwrap(), Length::Zero);
    }

    #[test]
    fn resolves_percent_and_em() {
        let ctx = ResolveContext {
            basis_width: 1000.0,
            font_size: 20.0,
            ..ResolveContext::default()
        };
        assert_eq!(Length::Percent(25.0).resolve(&ctx, Axis::Horizontal), 250.0);
        assert_eq!(Length::Em(1.5).resolve(&ctx, Axis::Horizontal), 30.0);
    }

    #[test]
    fn resolves_calc() {
        let ctx = ResolveContext {
            basis_width: 1000.0,
            ..ResolveContext::default()
        };
        let length = Length::parse("calc(100% - 20px)").unwrap();
        assert_eq!(length.resolve(&ctx, Axis::Horizontal), 980.0);
        let length = Length::parse("calc((100% - 20px) / 2)").unwrap();
        assert_eq!(length.resolve(&ctx, Axis::Horizontal), 490.0);
    }

    #[test]
    fn resolves_edges() {
        let values = vec![Length::Px(1.0), Length::Px(2.0)];
        let edges = Edges::from_1_to_4(&values).unwrap();
        assert_eq!(edges.top, Length::Px(1.0));
        assert_eq!(edges.right, Length::Px(2.0));
    }
}
