// SPDX-FileCopyrightText: 2026 Daniel
// SPDX-License-Identifier: GPL-3.0-or-later

//! CSS engine: values, selectors, cascade and typed computed styles.
//!
//! The engine follows the same mental model as a browser, scaled down to a
//! sticker scene:
//!
//! 1. [`parser`] scans rules and declarations (once).
//! 2. [`selector`] matches rules against scene elements.
//! 3. [`cascade`] orders declarations, resolves variables and inheritance.
//! 4. [`computed::ComputedStyle`] exposes typed style modules used by the
//!    layout engine and the painters.

pub mod cascade;
pub mod computed;
pub mod parser;
pub mod preferences;
pub mod properties;
pub mod selector;
pub mod values;

pub use cascade::{Cascade, FontFace, MediaQuery, StyleRule, Stylesheet};
pub use computed::{
    BackgroundStyle, BorderStyleComputed, ComputedStyle, EffectsStyle, FlexStyle, GridStyle,
    TextStyle,
};
pub use preferences::{ColorScheme, DisplayMode, Hover, Pointer, Preferences};
pub use properties::{
    AlignContent, AlignItems, BackgroundPosition, BackgroundRepeat, BackgroundSize, BorderBox,
    BorderStyle, BoxSizing, ClipPath, Declarations, Display, Filter, FlexDirection, FlexWrap,
    FontStyle, GridAutoFlow, JustifyContent, LineHeight, Overflow, Position, TextAlign,
    TextOverflow, TextTransform, TrackSize, Visibility, WhiteSpace,
};
pub use selector::{Element, Selector, Specificity};
pub use values::{Axis, Color, Edges, Length, ResolveContext, Size, Unit};

/// The logical size of the output surface.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Viewport {
    pub width: f32,
    pub height: f32,
    pub scale_factor: f32,
}

impl Default for Viewport {
    fn default() -> Self {
        Self {
            width: 1920.0,
            height: 1080.0,
            scale_factor: 1.0,
        }
    }
}

impl Viewport {
    pub fn context(&self, font_size: f32, basis_width: f32, basis_height: f32) -> ResolveContext {
        ResolveContext {
            viewport_width: self.width,
            viewport_height: self.height,
            font_size,
            root_font_size: 16.0,
            basis_width,
            basis_height,
        }
    }
}
