// SPDX-FileCopyrightText: 2026 Daniel
// SPDX-License-Identifier: GPL-3.0-or-later

//! CSS angles.

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Angle(pub f32);

impl Angle {
    pub fn parse(input: &str) -> Result<Self, String> {
        let text = input.trim().to_ascii_lowercase();
        let parse = |suffix: &str, factor: f32| {
            text.strip_suffix(suffix)
                .and_then(|value| value.trim().parse::<f32>().ok())
                .map(|value| Self(value * factor))
        };
        if let Some(angle) = parse("deg", 1.0) {
            return Ok(angle);
        }
        if let Some(angle) = parse("rad", 180.0 / std::f32::consts::PI) {
            return Ok(angle);
        }
        if let Some(angle) = parse("grad", 0.9) {
            return Ok(angle);
        }
        if let Some(angle) = parse("turn", 360.0) {
            return Ok(angle);
        }
        if text == "0" {
            return Ok(Self(0.0));
        }
        Err(format!("invalid angle: {input}"))
    }

    pub fn to_radians(self) -> f32 {
        self.0.to_radians()
    }
}
