// SPDX-FileCopyrightText: 2026 Daniel
// SPDX-License-Identifier: GPL-3.0-or-later

//! Typed, cascade-resolved style modules.
//!
//! The renderer never looks at raw declarations or property strings: it reads
//! these structures. That keeps painters free of `if property == ...` chains.

use std::collections::BTreeMap;

use vello::peniko::{Color, Mix};

use super::Viewport;
use super::properties::background::BackgroundAttachment;
use super::properties::background::BackgroundImage;
use super::properties::media_object_fit::ObjectFit;
use super::properties::text::{Direction, FontVariantCaps, OverflowWrap, VerticalAlign, WordBreak};
use super::properties::transition::{Animation, Transition};
use super::properties::{
    AlignContent, AlignItems, BackgroundPosition, BackgroundRepeat, BackgroundSize, BorderBox,
    BorderStyle, BoxSizing, ClipPath, Declarations, Display, Filter, FlexDirection, FlexWrap,
    FontStyle, GridAutoFlow, JustifyContent, LineHeight, Overflow, Position, TextAlign,
    TextDecoration, TextOverflow, TextTransform, TrackSize, Visibility, WhiteSpace,
};
use super::values::{Angle, Axis, Edges, Length, ResolveContext, Shadow, Size, TransformOp};

#[derive(Clone, Debug)]
pub struct BackgroundStyle {
    pub color: Color,
    pub images: Vec<BackgroundImage>,
    pub positions: Vec<BackgroundPosition>,
    pub sizes: Vec<BackgroundSize>,
    pub repeats: Vec<BackgroundRepeat>,
    pub clip: BorderBox,
    pub origin: BorderBox,
    pub blend_mode: Mix,
    pub attachment: BackgroundAttachment,
}

impl Default for BackgroundStyle {
    fn default() -> Self {
        Self {
            color: Color::new([0.0, 0.0, 0.0, 0.0]),
            images: Vec::new(),
            positions: Vec::new(),
            sizes: Vec::new(),
            repeats: Vec::new(),
            clip: BorderBox::BorderBox,
            origin: BorderBox::BorderBox,
            blend_mode: Mix::Normal,
            attachment: BackgroundAttachment::Scroll,
        }
    }
}

#[derive(Clone, Debug)]
pub struct BorderStyleComputed {
    pub image: Option<crate::css::properties::border::BorderImage>,
    pub widths: Edges<Length>,
    pub styles: Edges<BorderStyle>,
    pub colors: Edges<Color>,
    pub radius: Edges<Length>,
    pub outline_width: Length,
    pub outline_style: BorderStyle,
    pub outline_color: Color,
    pub outline_offset: Length,
}

impl Default for BorderStyleComputed {
    fn default() -> Self {
        Self {
            image: None,
            widths: Edges::default(),
            styles: Edges::default(),
            colors: Edges::all(Color::new([1.0, 1.0, 1.0, 1.0])),
            radius: Edges::default(),
            outline_width: Length::Zero,
            outline_style: BorderStyle::None,
            outline_color: Color::new([1.0, 1.0, 1.0, 1.0]),
            outline_offset: Length::Zero,
        }
    }
}

#[derive(Clone, Debug)]
pub struct TextStyle {
    pub color: Color,
    pub families: Vec<String>,
    pub font_size_px: f32,
    pub weight: u16,
    pub style: FontStyle,
    pub stretch: f32,
    pub caps: FontVariantCaps,
    pub line_height_px: f32,
    pub letter_spacing_px: f32,
    pub word_spacing_px: f32,
    pub align: TextAlign,
    pub align_last: TextAlign,
    pub decoration: TextDecoration,
    pub transform: TextTransform,
    pub indent: Length,
    pub white_space: WhiteSpace,
    pub overflow_wrap: OverflowWrap,
    pub word_break: WordBreak,
    pub overflow: TextOverflow,
    pub shadows: Vec<Shadow>,
    pub direction: Direction,
    pub vertical_align: VerticalAlign,
}

impl Default for TextStyle {
    fn default() -> Self {
        Self {
            color: Color::new([1.0, 1.0, 1.0, 1.0]),
            families: vec!["Sans".to_string()],
            font_size_px: 16.0,
            weight: 400,
            style: FontStyle::Normal,
            stretch: 1.0,
            caps: FontVariantCaps::Normal,
            line_height_px: 19.2,
            letter_spacing_px: 0.0,
            word_spacing_px: 0.0,
            align: TextAlign::Left,
            align_last: TextAlign::Left,
            decoration: TextDecoration::default(),
            transform: TextTransform::None,
            indent: Length::Zero,
            white_space: WhiteSpace::Normal,
            overflow_wrap: OverflowWrap::Normal,
            word_break: WordBreak::Normal,
            overflow: TextOverflow::Clip,
            shadows: Vec::new(),
            direction: Direction::Ltr,
            vertical_align: VerticalAlign::Baseline,
        }
    }
}

#[derive(Clone, Debug)]
pub struct EffectsStyle {
    pub opacity: f32,
    pub box_shadows: Vec<Shadow>,
    pub filters: Vec<Filter>,
    pub backdrop_filters: Vec<Filter>,
    pub mask_image: Option<crate::css::properties::background::BackgroundImage>,
    pub mask_size: crate::css::properties::BackgroundSize,
    pub mask_position: crate::css::properties::BackgroundPosition,
    pub mask_repeat: crate::css::properties::BackgroundRepeat,
    pub mix_blend_mode: Mix,
    pub transform: Vec<TransformOp>,
    pub translate: Option<(Length, Length)>,
    pub rotate: Option<Angle>,
    pub scale: Option<(f32, f32)>,
    pub transform_origin: (Length, Length),
    pub clip_path: ClipPath,
    pub isolation: bool,
    pub object_fit: ObjectFit,
    pub object_position: BackgroundPosition,
}

impl Default for EffectsStyle {
    fn default() -> Self {
        Self {
            opacity: 1.0,
            box_shadows: Vec::new(),
            filters: Vec::new(),
            backdrop_filters: Vec::new(),
            mask_image: None,
            mask_size: crate::css::properties::BackgroundSize::Auto,
            mask_position: crate::css::properties::BackgroundPosition::default(),
            mask_repeat: crate::css::properties::BackgroundRepeat::Repeat,
            mix_blend_mode: Mix::Normal,
            transform: Vec::new(),
            translate: None,
            rotate: None,
            scale: None,
            transform_origin: (Length::Percent(50.0), Length::Percent(50.0)),
            clip_path: ClipPath::None,
            isolation: false,
            object_fit: ObjectFit::Fill,
            object_position: BackgroundPosition::default(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct FlexStyle {
    pub direction: FlexDirection,
    pub wrap: FlexWrap,
    pub justify: JustifyContent,
    pub align_items: AlignItems,
    pub align_content: AlignContent,
    pub align_self: AlignItems,
    pub row_gap: Length,
    pub column_gap: Length,
    pub order: i32,
    pub grow: f32,
    pub shrink: f32,
    pub basis: Size,
}

impl Default for FlexStyle {
    fn default() -> Self {
        Self {
            direction: FlexDirection::Row,
            wrap: FlexWrap::NoWrap,
            justify: JustifyContent::Start,
            align_items: AlignItems::Start,
            align_content: AlignContent::Start,
            align_self: AlignItems::Start,
            row_gap: Length::Zero,
            column_gap: Length::Zero,
            order: 0,
            grow: 0.0,
            shrink: 1.0,
            basis: Size::Auto,
        }
    }
}

#[derive(Clone, Debug)]
pub struct GridStyle {
    pub columns: Vec<TrackSize>,
    pub rows: Vec<TrackSize>,
    pub auto_flow: GridAutoFlow,
    pub column: (i32, i32),
    pub row: (i32, i32),
}

impl Default for GridStyle {
    fn default() -> Self {
        Self {
            columns: Vec::new(),
            rows: Vec::new(),
            auto_flow: GridAutoFlow::Row,
            column: (0, 0),
            row: (0, 0),
        }
    }
}

/// Fully resolved style for one scene element.
#[derive(Clone, Debug)]
pub struct ComputedStyle {
    pub display: Display,
    pub position: Position,
    pub top: Option<Length>,
    pub right: Option<Length>,
    pub bottom: Option<Length>,
    pub left: Option<Length>,
    pub width: Size,
    pub height: Size,
    pub min_width: Size,
    pub min_height: Size,
    pub max_width: Size,
    pub max_height: Size,
    pub margin: Edges<Length>,
    pub padding: Edges<Length>,
    pub box_sizing: BoxSizing,
    pub overflow_x: Overflow,
    pub overflow_y: Overflow,
    pub aspect_ratio: Option<f32>,
    pub visibility: Visibility,

    pub background: BackgroundStyle,
    pub border: BorderStyleComputed,
    pub text: TextStyle,
    pub effects: EffectsStyle,
    pub flex: FlexStyle,
    pub grid: GridStyle,

    pub z_index: i32,
    pub animations: Vec<Animation>,
    pub transitions: Vec<Transition>,
    pub custom_properties: BTreeMap<String, String>,
}

impl Default for ComputedStyle {
    fn default() -> Self {
        Self {
            display: Display::Block,
            position: Position::Static,
            top: None,
            right: None,
            bottom: None,
            left: None,
            width: Size::Auto,
            height: Size::Auto,
            min_width: Size::Auto,
            min_height: Size::Auto,
            max_width: Size::None,
            max_height: Size::None,
            margin: Edges::default(),
            padding: Edges::default(),
            box_sizing: BoxSizing::ContentBox,
            overflow_x: Overflow::Visible,
            overflow_y: Overflow::Visible,
            aspect_ratio: None,
            visibility: Visibility::Visible,
            background: BackgroundStyle::default(),
            border: BorderStyleComputed {
                colors: Edges::all(Color::new([1.0, 1.0, 1.0, 1.0])),
                ..BorderStyleComputed::default()
            },
            text: TextStyle::default(),
            effects: EffectsStyle::default(),
            flex: FlexStyle::default(),
            grid: GridStyle::default(),
            z_index: 0,
            animations: Vec::new(),
            transitions: Vec::new(),
            custom_properties: BTreeMap::new(),
        }
    }
}

impl ComputedStyle {
    /// Resolve declarations on top of an optional parent style.
    pub fn compute(
        declarations: &Declarations,
        parent: Option<&ComputedStyle>,
        viewport: Viewport,
        custom_properties: BTreeMap<String, String>,
    ) -> Self {
        let mut style = match parent {
            Some(parent) => Self {
                // Structural properties are not inherited.
                display: Display::Block,
                position: Position::Static,
                top: None,
                right: None,
                bottom: None,
                left: None,
                width: Size::Auto,
                height: Size::Auto,
                min_width: Size::Auto,
                min_height: Size::Auto,
                max_width: Size::None,
                max_height: Size::None,
                margin: Edges::default(),
                padding: Edges::default(),
                box_sizing: BoxSizing::ContentBox,
                overflow_x: Overflow::Visible,
                overflow_y: Overflow::Visible,
                aspect_ratio: None,
                background: BackgroundStyle::default(),
                border: BorderStyleComputed {
                    colors: Edges::all(parent.text.color),
                    ..BorderStyleComputed::default()
                },
                effects: EffectsStyle::default(),
                flex: FlexStyle::default(),
                grid: GridStyle::default(),
                z_index: 0,
                animations: Vec::new(),
                transitions: Vec::new(),
                custom_properties: parent.custom_properties.clone(),
                // Inherited modules start from the parent.
                visibility: parent.visibility,
                text: parent.text.clone(),
            },
            None => Self::default(),
        };
        style.custom_properties = custom_properties;

        style.display = declarations.display.unwrap_or(style.display);
        style.position = declarations.position.unwrap_or(style.position);
        style.top = declarations.top.clone();
        style.right = declarations.right.clone();
        style.bottom = declarations.bottom.clone();
        style.left = declarations.left.clone();
        style.width = declarations.width.clone().unwrap_or(style.width);
        style.height = declarations.height.clone().unwrap_or(style.height);
        style.min_width = declarations.min_width.clone().unwrap_or(style.min_width);
        style.min_height = declarations.min_height.clone().unwrap_or(style.min_height);
        style.max_width = declarations.max_width.clone().unwrap_or(style.max_width);
        style.max_height = declarations.max_height.clone().unwrap_or(style.max_height);
        style.margin = declarations.margin.clone().unwrap_or_default();
        style.padding = declarations.padding.clone().unwrap_or_default();
        style.box_sizing = declarations.box_sizing.unwrap_or(style.box_sizing);
        style.overflow_x = declarations.overflow_x.unwrap_or(style.overflow_x);
        style.overflow_y = declarations.overflow_y.unwrap_or(style.overflow_y);
        style.aspect_ratio = declarations.aspect_ratio;
        style.visibility = declarations.visibility.unwrap_or(style.visibility);
        style.z_index = declarations.z_index.unwrap_or(0);
        if let Some(animations) = &declarations.animation {
            style.animations = animations.clone();
        }
        if let Some(transitions) = &declarations.transition {
            style.transitions = transitions.clone();
        }

        // ---- text -------------------------------------------------------
        if let Some(color) = declarations.color {
            style.text.color = color;
        }
        if let Some(families) = &declarations.font_family {
            style.text.families = families.clone();
        }
        let parent_font_size = parent
            .map(|parent| parent.text.font_size_px)
            .unwrap_or(16.0);
        if let Some(size) = &declarations.font_size {
            style.text.font_size_px = match size {
                Length::Percent(percent) => parent_font_size * percent / 100.0,
                other => {
                    let context = ResolveContext {
                        viewport_width: viewport.width,
                        viewport_height: viewport.height,
                        font_size: parent_font_size,
                        root_font_size: 16.0,
                        basis_width: viewport.width,
                        basis_height: viewport.height,
                    };
                    other.resolve(&context, Axis::Horizontal)
                }
            }
            .max(0.0);
        }
        if let Some(weight) = declarations.font_weight {
            style.text.weight = weight;
        }
        if let Some(font_style) = declarations.font_style {
            style.text.style = font_style;
        }
        if let Some(stretch) = declarations.font_stretch {
            style.text.stretch = stretch;
        }
        if let Some(caps) = declarations.font_variant_caps {
            style.text.caps = caps;
        }
        style.text.line_height_px = match &declarations.line_height {
            Some(LineHeight::Normal) | None => style.text.font_size_px * 1.2,
            Some(LineHeight::Number(number)) => style.text.font_size_px * number,
            Some(LineHeight::Length(length)) => {
                let context = crate::css::Viewport {
                    width: viewport.width,
                    height: viewport.height,
                    scale_factor: viewport.scale_factor,
                }
                .context(style.text.font_size_px, viewport.width, viewport.height);
                length.resolve(&context, Axis::Horizontal)
            }
        };

        let text_context =
            viewport.context(style.text.font_size_px, viewport.width, viewport.height);
        if let Some(spacing) = &declarations.letter_spacing {
            style.text.letter_spacing_px = spacing.resolve(&text_context, Axis::Horizontal);
        }
        if let Some(spacing) = &declarations.word_spacing {
            style.text.word_spacing_px = spacing.resolve(&text_context, Axis::Horizontal);
        }
        if let Some(align) = declarations.text_align {
            style.text.align = align;
        }
        if let Some(align) = declarations.text_align_last {
            style.text.align_last = align;
        }
        if let Some(decoration) = &declarations.text_decoration {
            style.text.decoration = decoration.clone();
        }
        if let Some(transform) = declarations.text_transform {
            style.text.transform = transform;
        }
        if let Some(indent) = &declarations.text_indent {
            style.text.indent = indent.clone();
        }
        if let Some(white_space) = declarations.white_space {
            style.text.white_space = white_space;
        }
        if let Some(overflow_wrap) = declarations.overflow_wrap {
            style.text.overflow_wrap = overflow_wrap;
        }
        if let Some(word_break) = declarations.word_break {
            style.text.word_break = word_break;
        }
        if let Some(overflow) = declarations.text_overflow {
            style.text.overflow = overflow;
        }
        if let Some(shadows) = &declarations.text_shadow {
            style.text.shadows = shadows.clone();
        }
        if let Some(direction) = declarations.direction {
            style.text.direction = direction;
        }
        if let Some(align) = declarations.vertical_align {
            style.text.vertical_align = align;
        }

        // ---- background -------------------------------------------------
        if let Some(color) = declarations.background_color {
            style.background.color = color;
        }
        if let Some(images) = &declarations.background_image {
            style.background.images = images.clone();
        }
        if let Some(positions) = &declarations.background_position {
            style.background.positions = positions.clone();
        }
        if let Some(sizes) = &declarations.background_size {
            style.background.sizes = sizes.clone();
        }
        if let Some(repeats) = &declarations.background_repeat {
            style.background.repeats = repeats.clone();
        }
        if let Some(clip) = declarations.background_clip {
            style.background.clip = clip;
        }
        if let Some(origin) = declarations.background_origin {
            style.background.origin = origin;
        }
        if let Some(blend) = declarations.background_blend_mode {
            style.background.blend_mode = blend;
        }
        if let Some(attachment) = declarations.background_attachment {
            style.background.attachment = attachment;
        }

        // ---- border -----------------------------------------------------
        if let Some(widths) = &declarations.border_width {
            style.border.widths = widths.clone();
        }
        if let Some(styles) = &declarations.border_style {
            style.border.styles = styles.clone();
        }
        if let Some(colors) = &declarations.border_color {
            style.border.colors = colors.clone();
        }
        if let Some(radius) = &declarations.border_radius {
            style.border.radius = radius.clone();
        }
        if let Some(width) = &declarations.outline_width {
            style.border.outline_width = width.clone();
        }
        if let Some(outline_style) = declarations.outline_style {
            style.border.outline_style = outline_style;
        }
        if let Some(color) = declarations.outline_color {
            style.border.outline_color = color;
        }
        if let Some(offset) = &declarations.outline_offset {
            style.border.outline_offset = offset.clone();
        }

        // ---- effects ----------------------------------------------------
        if let Some(opacity) = declarations.opacity {
            style.effects.opacity = opacity;
        }
        if let Some(shadows) = &declarations.box_shadow {
            style.effects.box_shadows = shadows.clone();
        }
        if let Some(filters) = &declarations.filter {
            style.effects.filters = filters.clone();
        }
        if let Some(filters) = &declarations.backdrop_filter {
            style.effects.backdrop_filters = filters.clone();
        }
        if let Some(mask) = &declarations.mask_image {
            style.effects.mask_image = Some(mask.clone());
        }
        if let Some(size) = &declarations.mask_size {
            style.effects.mask_size = size.clone();
        }
        if let Some(position) = declarations.mask_position.clone() {
            style.effects.mask_position = position;
        }
        if let Some(repeat) = declarations.mask_repeat {
            style.effects.mask_repeat = repeat;
        }
        if let Some(blend) = declarations.mix_blend_mode {
            style.effects.mix_blend_mode = blend;
        }
        if let Some(transform) = &declarations.transform {
            style.effects.transform = transform.clone();
        }
        style.effects.translate = declarations.translate.clone();
        style.effects.rotate = declarations.rotate;
        style.effects.scale = declarations.scale;
        if let Some(origin) = &declarations.transform_origin {
            style.effects.transform_origin = origin.clone();
        }
        if let Some(clip) = &declarations.clip_path {
            style.effects.clip_path = clip.clone();
        }
        if let Some(isolation) = declarations.isolation {
            style.effects.isolation = isolation;
        }
        if let Some(fit) = declarations.object_fit {
            style.effects.object_fit = fit;
        }
        if let Some(position) = &declarations.object_position {
            style.effects.object_position = position.clone();
        }

        // ---- flex -------------------------------------------------------
        if let Some(direction) = declarations.flex_direction {
            style.flex.direction = direction;
            if matches!(
                direction,
                FlexDirection::Column | FlexDirection::ColumnReverse
            ) && declarations.display.is_none()
            {
                style.display = Display::Flex;
            }
        }
        if let Some(wrap) = declarations.flex_wrap {
            style.flex.wrap = wrap;
        }
        if let Some(justify) = declarations.justify_content {
            style.flex.justify = justify;
            if declarations.display.is_none() {
                style.display = Display::Flex;
            }
        }
        if let Some(align) = declarations.align_items {
            style.flex.align_items = align;
        }
        if let Some(align) = declarations.align_content {
            style.flex.align_content = align;
        }
        if let Some(align) = declarations.align_self {
            style.flex.align_self = align;
        }
        if let Some((row, column)) = &declarations.gap {
            style.flex.row_gap = row.clone();
            style.flex.column_gap = column.clone();
        }
        if let Some(gap) = &declarations.row_gap {
            style.flex.row_gap = gap.clone();
        }
        if let Some(gap) = &declarations.column_gap {
            style.flex.column_gap = gap.clone();
        }
        if let Some(order) = declarations.order {
            style.flex.order = order;
        }
        if let Some(grow) = declarations.flex_grow {
            style.flex.grow = grow;
        }
        if let Some(shrink) = declarations.flex_shrink {
            style.flex.shrink = shrink;
        }
        if let Some(basis) = &declarations.flex_basis {
            style.flex.basis = basis.clone();
        }

        if let Some(source) = &declarations.border_image_source {
            let (slice, fill) = declarations
                .border_image_slice
                .clone()
                .unwrap_or_else(|| (Edges::all(Length::Percent(30.0)), false));
            let width = declarations
                .border_image_width
                .clone()
                .unwrap_or_else(|| style.border.widths.clone());
            let outset = declarations
                .border_image_outset
                .clone()
                .unwrap_or_else(|| Edges::all(Length::Zero));
            style.border.image = Some(crate::css::properties::border::BorderImage {
                source: source.clone(),
                slice,
                width,
                outset,
                fill,
            });
        }

        // ---- grid -------------------------------------------------------
        if let Some(columns) = &declarations.grid_template_columns {
            style.grid.columns = columns.clone();
            if declarations.display.is_none() {
                style.display = Display::Grid;
            }
        }
        if let Some(rows) = &declarations.grid_template_rows {
            style.grid.rows = rows.clone();
        }
        if let Some(flow) = declarations.grid_auto_flow {
            style.grid.auto_flow = flow;
        }
        if let Some(column) = declarations.grid_column {
            style.grid.column = column;
        }
        if let Some(row) = declarations.grid_row {
            style.grid.row = row;
        }

        style
    }

    /// Convenience helper for painters that need the element's own font
    /// relative resolve context.
    pub fn resolve_context(&self, viewport: Viewport, width: f32, height: f32) -> ResolveContext {
        viewport.context(self.text.font_size_px, width, height)
    }
}
