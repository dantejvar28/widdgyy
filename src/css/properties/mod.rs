// SPDX-FileCopyrightText: 2026 Daniel
// SPDX-License-Identifier: GPL-3.0-or-later

//! Typed CSS declarations.
//!
//! Declarations are grouped by style family. Each submodule owns the parsing
//! rules for its family, which keeps the cascade and the painters free of long
//! `if name == "..."` chains. Parsing happens once per element (at load or hot
//! reload), never inside the frame loop.

pub mod background;
pub mod border;
pub mod box_model;
pub mod effects;
pub mod flex;
pub mod grid;
pub mod text;
pub mod transition;

use vello::peniko::Mix;

use super::values::{Angle, Color, Edges, Length, Shadow, Size, TransformOp};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Display {
    #[default]
    Block,
    None,
    Inline,
    InlineBlock,
    Flex,
    Grid,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Position {
    #[default]
    Static,
    Relative,
    Absolute,
    Fixed,
    Sticky,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum BoxSizing {
    #[default]
    ContentBox,
    BorderBox,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Overflow {
    #[default]
    Visible,
    Hidden,
    Clip,
    Scroll,
    Auto,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Visibility {
    #[default]
    Visible,
    Hidden,
    Collapse,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum BorderStyle {
    #[default]
    None,
    Solid,
    Dashed,
    Dotted,
    Double,
    Groove,
    Ridge,
    Inset,
    Outset,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum FontStyle {
    #[default]
    Normal,
    Italic,
    Oblique,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum TextAlign {
    #[default]
    Left,
    Right,
    Center,
    Justify,
    Start,
    End,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum TextTransform {
    #[default]
    None,
    Uppercase,
    Lowercase,
    Capitalize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum WhiteSpace {
    #[default]
    Normal,
    NoWrap,
    Pre,
    PreWrap,
    PreLine,
    BreakSpaces,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum TextOverflow {
    #[default]
    Clip,
    Ellipsis,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum FlexDirection {
    #[default]
    Row,
    RowReverse,
    Column,
    ColumnReverse,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum FlexWrap {
    #[default]
    NoWrap,
    Wrap,
    WrapReverse,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum JustifyContent {
    #[default]
    Start,
    End,
    Center,
    SpaceBetween,
    SpaceAround,
    SpaceEvenly,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum AlignItems {
    #[default]
    Start,
    End,
    Center,
    Stretch,
    Baseline,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum AlignContent {
    #[default]
    Start,
    End,
    Center,
    SpaceBetween,
    SpaceAround,
    SpaceEvenly,
    Stretch,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum BorderBox {
    #[default]
    BorderBox,
    PaddingBox,
    ContentBox,
    Text,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum BackgroundRepeat {
    #[default]
    Repeat,
    RepeatX,
    RepeatY,
    NoRepeat,
    Space,
    Round,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub enum BackgroundSize {
    #[default]
    Auto,
    Cover,
    Contain,
    Explicit(Length, Length),
}

#[derive(Clone, Debug, PartialEq)]
pub struct BackgroundPosition {
    pub x: Length,
    pub y: Length,
}

impl Default for BackgroundPosition {
    fn default() -> Self {
        Self {
            x: Length::Percent(0.0),
            y: Length::Percent(0.0),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum GridAutoFlow {
    #[default]
    Row,
    Column,
    RowDense,
    ColumnDense,
}

#[derive(Clone, Debug, PartialEq)]
pub enum TrackSize {
    Auto,
    Fr(f32),
    Px(f32),
    Percent(f32),
    MinContent,
    MaxContent,
    FitContent(Length),
    Repeat(u32, Vec<TrackSize>),
}

#[derive(Clone, Debug, PartialEq, Default)]
pub enum LineHeight {
    #[default]
    Normal,
    Number(f32),
    Length(Length),
}

#[derive(Clone, Debug, PartialEq)]
pub enum ClipPath {
    None,
    Inset(Edges<Length>, Option<Length>),
    Circle(Length, Length, Length),
    Ellipse(Length, Length, Length, Length),
    Polygon(Vec<(Length, Length)>),
    Url(String),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Filter {
    Blur(Length),
    Brightness(f32),
    Contrast(f32),
    Grayscale(f32),
    Saturate(f32),
    Sepia(f32),
    Invert(f32),
    Opacity(f32),
    HueRotate(Angle),
    DropShadow(Shadow),
}

#[derive(Clone, Debug, PartialEq)]
pub struct TextDecoration {
    pub underline: bool,
    pub overline: bool,
    pub line_through: bool,
    pub style: BorderStyle,
    pub color: Option<Color>,
    pub thickness: Option<Length>,
}

impl Default for TextDecoration {
    fn default() -> Self {
        Self {
            underline: false,
            overline: false,
            line_through: false,
            style: BorderStyle::Solid,
            color: None,
            thickness: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct TextShadow {
    pub shadows: Vec<Shadow>,
}

/// All CSS properties understood by the engine.
///
/// Every field is an `Option` so the cascade can merge rules without caring
/// about property families. Shorthands are expanded by the family parsers.
#[derive(Clone, Debug, Default)]
pub struct Declarations {
    // box model
    pub display: Option<Display>,
    pub position: Option<Position>,
    pub top: Option<Length>,
    pub right: Option<Length>,
    pub bottom: Option<Length>,
    pub left: Option<Length>,
    pub width: Option<Size>,
    pub height: Option<Size>,
    pub min_width: Option<Size>,
    pub min_height: Option<Size>,
    pub max_width: Option<Size>,
    pub max_height: Option<Size>,
    pub margin: Option<Edges<Length>>,
    pub padding: Option<Edges<Length>>,
    pub box_sizing: Option<BoxSizing>,
    pub overflow_x: Option<Overflow>,
    pub overflow_y: Option<Overflow>,
    pub aspect_ratio: Option<f32>,
    pub visibility: Option<Visibility>,

    // background
    pub background_color: Option<Color>,
    pub background_image: Option<Vec<background::BackgroundImage>>,
    pub background_position: Option<Vec<BackgroundPosition>>,
    pub background_size: Option<Vec<BackgroundSize>>,
    pub background_repeat: Option<Vec<BackgroundRepeat>>,
    pub background_clip: Option<BorderBox>,
    pub background_origin: Option<BorderBox>,
    pub background_blend_mode: Option<Mix>,
    pub background_attachment: Option<background::BackgroundAttachment>,

    // border / outline
    pub border_width: Option<Edges<Length>>,
    pub border_style: Option<Edges<BorderStyle>>,
    pub border_color: Option<Edges<Color>>,
    pub border_radius: Option<Edges<Length>>,
    pub outline_width: Option<Length>,
    pub outline_style: Option<BorderStyle>,
    pub outline_color: Option<Color>,
    pub outline_offset: Option<Length>,

    // text
    pub color: Option<Color>,
    pub font_family: Option<Vec<String>>,
    pub font_size: Option<Length>,
    pub font_weight: Option<u16>,
    pub font_style: Option<FontStyle>,
    pub font_stretch: Option<f32>,
    pub font_variant_caps: Option<text::FontVariantCaps>,
    pub line_height: Option<LineHeight>,
    pub letter_spacing: Option<Length>,
    pub word_spacing: Option<Length>,
    pub text_align: Option<TextAlign>,
    pub text_align_last: Option<TextAlign>,
    pub text_decoration: Option<TextDecoration>,
    pub text_transform: Option<TextTransform>,
    pub text_indent: Option<Length>,
    pub white_space: Option<WhiteSpace>,
    pub overflow_wrap: Option<text::OverflowWrap>,
    pub word_break: Option<text::WordBreak>,
    pub text_overflow: Option<TextOverflow>,
    pub text_shadow: Option<Vec<Shadow>>,
    pub direction: Option<text::Direction>,
    pub vertical_align: Option<text::VerticalAlign>,

    // effects
    pub opacity: Option<f32>,
    pub box_shadow: Option<Vec<Shadow>>,
    pub filter: Option<Vec<Filter>>,
    pub backdrop_filter: Option<Vec<Filter>>,
    pub mix_blend_mode: Option<Mix>,
    pub transform: Option<Vec<TransformOp>>,
    pub transform_origin: Option<(Length, Length)>,
    pub clip_path: Option<ClipPath>,
    pub isolation: Option<bool>,
    pub object_fit: Option<media_object_fit::ObjectFit>,
    pub object_position: Option<BackgroundPosition>,

    // flex
    pub flex_direction: Option<FlexDirection>,
    pub flex_wrap: Option<FlexWrap>,
    pub justify_content: Option<JustifyContent>,
    pub align_items: Option<AlignItems>,
    pub align_content: Option<AlignContent>,
    pub align_self: Option<AlignItems>,
    pub gap: Option<(Length, Length)>,
    pub row_gap: Option<Length>,
    pub column_gap: Option<Length>,
    pub order: Option<i32>,
    pub flex_grow: Option<f32>,
    pub flex_shrink: Option<f32>,
    pub flex_basis: Option<Size>,

    // grid
    pub grid_template_columns: Option<Vec<TrackSize>>,
    pub grid_template_rows: Option<Vec<TrackSize>>,
    pub grid_auto_flow: Option<GridAutoFlow>,
    pub grid_column: Option<(i32, i32)>,
    pub grid_row: Option<(i32, i32)>,

    // stacking / interaction
    pub z_index: Option<i32>,
    pub pointer_events: Option<bool>,
    pub cursor: Option<String>,

    // animation / transition
    pub animation: Option<Vec<transition::Animation>>,
    pub transition: Option<Vec<transition::Transition>>,
}

/// `object-fit` values for media widgets.
pub mod media_object_fit {
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
    pub enum ObjectFit {
        #[default]
        Fill,
        Contain,
        Cover,
        None,
        ScaleDown,
    }
}

impl Declarations {
    /// Apply one declaration. Returns `true` when the property is known.
    pub fn apply(&mut self, name: &str, value: &str, current: Color) -> bool {
        let name = name.trim().to_ascii_lowercase();
        let value = value.trim();
        if name.starts_with("--") {
            return false;
        }
        if box_model::apply(self, &name, value, current)
            || background::apply(self, &name, value, current)
            || border::apply(self, &name, value, current)
            || text::apply(self, &name, value, current)
            || effects::apply(self, &name, value, current)
            || flex::apply(self, &name, value)
            || grid::apply(self, &name, value)
            || transition::apply(self, &name, value)
            || apply_misc(self, &name, value)
        {
            return true;
        }
        log::debug!("unsupported CSS property: {name}: {value}");
        false
    }
}

fn apply_misc(decl: &mut Declarations, name: &str, value: &str) -> bool {
    match name {
        "z-index" => {
            if value.eq_ignore_ascii_case("auto") {
                decl.z_index = Some(0);
            } else if let Ok(number) = value.parse::<i32>() {
                decl.z_index = Some(number);
            }
            true
        }
        "visibility" => {
            decl.visibility = Some(match value.to_ascii_lowercase().as_str() {
                "visible" => Visibility::Visible,
                "hidden" => Visibility::Hidden,
                "collapse" => Visibility::Collapse,
                _ => return false,
            });
            true
        }
        "pointer-events" => {
            decl.pointer_events = Some(!value.eq_ignore_ascii_case("none"));
            true
        }
        "cursor" => {
            decl.cursor = Some(value.to_string());
            true
        }
        _ => false,
    }
}

pub(crate) fn split_ws(input: &str) -> Vec<String> {
    super::values::shadow::split_ws(input)
}

pub(crate) fn split_commas(input: &str) -> Vec<String> {
    super::values::shadow::split_commas(input)
}

pub(crate) fn parse_lengths(input: &str) -> Result<Vec<Length>, String> {
    split_ws(input)
        .into_iter()
        .map(|token| Length::parse(&token))
        .collect()
}

pub(crate) fn parse_color(input: &str, current: Color) -> Option<Color> {
    super::values::color::parse(input, current)
}
