// SPDX-FileCopyrightText: 2026 Daniel
// SPDX-License-Identifier: GPL-3.0-or-later

//! `transform` functions.

use vello::kurbo::Affine;

use super::angle::Angle;
use super::length::Length;

#[derive(Clone, Debug, PartialEq)]
pub enum TransformOp {
    Translate(Length, Length),
    TranslateX(Length),
    TranslateY(Length),
    Scale(f32, f32),
    ScaleX(f32),
    ScaleY(f32),
    Rotate(Angle),
    Skew(Angle, Angle),
    SkewX(Angle),
    SkewY(Angle),
    Matrix([f32; 6]),
}

impl TransformOp {
    /// Convert to a kurbo transform. Lengths are resolved by the caller with
    /// the element's containing block.
    pub fn to_affine(&self, resolve: impl Fn(&Length) -> f32) -> Affine {
        match self {
            Self::Translate(x, y) => Affine::translate((resolve(x) as f64, resolve(y) as f64)),
            Self::TranslateX(x) => Affine::translate((resolve(x) as f64, 0.0)),
            Self::TranslateY(y) => Affine::translate((0.0, resolve(y) as f64)),
            Self::Scale(x, y) => Affine::scale_non_uniform(*x as f64, *y as f64),
            Self::ScaleX(x) => Affine::scale_non_uniform(*x as f64, 1.0),
            Self::ScaleY(y) => Affine::scale_non_uniform(1.0, *y as f64),
            Self::Rotate(angle) => Affine::rotate(angle.to_radians() as f64),
            Self::Skew(x, y) => Affine::skew(x.to_radians() as f64, y.to_radians() as f64),
            Self::SkewX(x) => Affine::skew(x.to_radians() as f64, 0.0),
            Self::SkewY(y) => Affine::skew(0.0, y.to_radians() as f64),
            Self::Matrix([a, b, c, d, e, f]) => Affine::new([
                *a as f64, *b as f64, *c as f64, *d as f64, *e as f64, *f as f64,
            ]),
        }
    }

    pub fn parse(input: &str) -> Result<Vec<Self>, String> {
        let mut ops = Vec::new();
        for function in split_functions(input) {
            ops.push(parse_function(&function)?);
        }
        Ok(ops)
    }
}

fn split_functions(input: &str) -> Vec<String> {
    let mut result = Vec::new();
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
                if depth == 0 {
                    result.push(std::mem::take(&mut current));
                }
            }
            c if c.is_whitespace() && depth == 0 => {
                if !current.is_empty() {
                    result.push(std::mem::take(&mut current));
                }
            }
            c => current.push(c),
        }
    }
    if !current.is_empty() {
        result.push(current);
    }
    result
}

fn parse_function(function: &str) -> Result<TransformOp, String> {
    let open = function.find('(').ok_or_else(|| "missing (".to_string())?;
    let name = function[..open].trim().to_ascii_lowercase();
    let inner = function[open + 1..]
        .strip_suffix(')')
        .ok_or_else(|| "missing )".to_string())?;
    let values: Vec<&str> = inner
        .split(|c: char| c == ',' || c.is_whitespace())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .collect();

    let length = |value: &str| Length::parse(value).map_err(|error| error.to_string());
    let number = |value: &str| {
        value
            .trim_end_matches('%')
            .parse::<f32>()
            .map(|value| {
                if value.is_nan() {
                    value
                } else if inner.contains('%') && value.abs() > 2.0 {
                    value / 100.0
                } else {
                    value
                }
            })
            .map_err(|_| format!("invalid number: {value}"))
    };

    Ok(match name.as_str() {
        "translate" => TransformOp::Translate(
            length(values.first().copied().unwrap_or("0"))?,
            length(values.get(1).copied().unwrap_or("0"))?,
        ),
        "translatex" => TransformOp::TranslateX(length(values.first().copied().unwrap_or("0"))?),
        "translatey" => TransformOp::TranslateY(length(values.first().copied().unwrap_or("0"))?),
        "scale" => {
            let x = number(values.first().copied().unwrap_or("1"))?;
            let y = number(
                values
                    .get(1)
                    .copied()
                    .unwrap_or_else(|| values.first().copied().unwrap_or("1")),
            )?;
            TransformOp::Scale(x, y)
        }
        "scalex" => TransformOp::ScaleX(number(values.first().copied().unwrap_or("1"))?),
        "scaley" => TransformOp::ScaleY(number(values.first().copied().unwrap_or("1"))?),
        "rotate" => TransformOp::Rotate(Angle::parse(values.first().copied().unwrap_or("0deg"))?),
        "skew" => TransformOp::Skew(
            Angle::parse(values.first().copied().unwrap_or("0deg"))?,
            Angle::parse(values.get(1).copied().unwrap_or("0deg"))?,
        ),
        "skewx" => TransformOp::SkewX(Angle::parse(values.first().copied().unwrap_or("0deg"))?),
        "skewy" => TransformOp::SkewY(Angle::parse(values.first().copied().unwrap_or("0deg"))?),
        "matrix" => {
            if values.len() != 6 {
                return Err("matrix() requires 6 values".to_string());
            }
            let mut matrix = [0.0; 6];
            for (index, value) in values.iter().enumerate() {
                matrix[index] = value
                    .parse::<f32>()
                    .map_err(|_| format!("invalid matrix value: {value}"))?;
            }
            TransformOp::Matrix(matrix)
        }
        "none" => return Err("none is not a transform function".to_string()),
        other => return Err(format!("unsupported transform: {other}")),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_transform_functions() {
        let ops = TransformOp::parse("translate(10px, 20px) rotate(45deg) scale(2)").unwrap();
        assert_eq!(ops.len(), 3);
        assert!(matches!(ops[0], TransformOp::Translate(..)));
        assert!(matches!(ops[1], TransformOp::Rotate(..)));
        assert!(matches!(ops[2], TransformOp::Scale(2.0, 2.0)));
    }
}
