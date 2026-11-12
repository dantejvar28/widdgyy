// SPDX-FileCopyrightText: 2026 Daniel
// SPDX-License-Identifier: GPL-3.0-or-later

//! Box model, block/flex/grid layout.
//!
//! Layout is a pure function over the scene: given a computed style and a text
//! engine, it produces pixel rectangles. It runs on load/reload/resize, not on
//! every frame.
//!
//! The implementation follows CSS formatting contexts closely enough to match
//! a browser on the supported surface:
//!
//! - block containers stack children and collapse adjacent/parent-child
//!   vertical margins;
//! - flex containers use `flex-basis` as the flex base and apply grow/shrink
//!   from that base;
//! - grid containers support named areas, explicit and implicit tracks,
//!   dense/sparse auto placement and `justify-*` alignment.

use std::collections::BTreeMap;

use crate::css::Viewport;
use crate::css::computed::ComputedStyle;
use crate::css::properties::media_object_fit::ObjectFit;
use crate::css::properties::{
    AlignContent, AlignItems, Display, FlexDirection, FlexWrap, GridArea, GridAutoFlow, GridLine,
    GridPlacement, JustifyContent, Position, TrackSize,
};
use crate::css::values::{Axis, Edges, ResolveContext, Size};
use crate::scene::{Node, NodeKind};
use crate::text::{ShapeRequest, TextEngine};

#[derive(Clone, Debug, Default)]
pub struct LayoutBox {
    /// Position relative to the parent.
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    /// Content rectangle (inside padding and border), relative to the parent.
    pub content_x: f32,
    pub content_y: f32,
    pub content_width: f32,
    pub content_height: f32,
    /// Media drawing rectangle for `object-fit`.
    pub draw_x: f32,
    pub draw_y: f32,
    pub draw_width: f32,
    pub draw_height: f32,
    pub padding: Edges<f32>,
    pub border: Edges<f32>,
    pub margin: Edges<f32>,
}

/// Min/max-content extents of a box, expressed as content sizes.
#[derive(Clone, Copy, Debug, Default)]
struct IntrinsicSize {
    min_width: f32,
    max_width: f32,
    min_height: f32,
    max_height: f32,
}

impl IntrinsicSize {
    fn axis(&self, axis: Axis, minimum: bool) -> f32 {
        match (axis, minimum) {
            (Axis::Horizontal, true) => self.min_width,
            (Axis::Horizontal, false) => self.max_width,
            (Axis::Vertical, true) => self.min_height,
            (Axis::Vertical, false) => self.max_height,
        }
    }
}

pub struct LayoutEngine<'a> {
    pub text: &'a mut TextEngine,
}

impl<'a> LayoutEngine<'a> {
    pub fn new(text: &'a mut TextEngine) -> Self {
        Self { text }
    }

    pub fn layout_node(
        &mut self,
        node: &mut Node,
        viewport: Viewport,
        available_width: f32,
        available_height: f32,
    ) {
        self.layout_node_with(
            node,
            viewport,
            available_width,
            available_height,
            None,
            None,
        );
    }

    fn layout_node_with(
        &mut self,
        node: &mut Node,
        viewport: Viewport,
        available_width: f32,
        available_height: f32,
        forced_width: Option<f32>,
        forced_height: Option<f32>,
    ) {
        let style = node.style.clone();
        let font_size = style.text.font_size_px;
        let context = viewport.context(font_size, available_width, available_height);
        let padding = style.padding.resolve(&context, Axis::Horizontal);
        let border = style.border.widths.resolve(&context, Axis::Horizontal);
        let margin = style.margin.resolve(&context, Axis::Horizontal);
        let horizontal_extra = padding.left + padding.right + border.left + border.right;
        let vertical_extra = padding.top + padding.bottom + border.top + border.bottom;

        node.layout.padding = padding.clone();
        node.layout.border = border.clone();
        node.layout.margin = margin.clone();

        let intrinsic_keyword = |size: &Size| {
            matches!(
                size,
                Size::MinContent | Size::MaxContent | Size::FitContent(_) | Size::Stretch
            )
        };
        let needs_intrinsic = forced_width.is_none() && intrinsic_keyword(&style.width)
            || forced_height.is_none() && intrinsic_keyword(&style.height)
            || intrinsic_keyword(&style.min_width)
            || intrinsic_keyword(&style.max_width)
            || intrinsic_keyword(&style.min_height)
            || intrinsic_keyword(&style.max_height);
        let intrinsic =
            needs_intrinsic.then(|| self.intrinsic_size(node, viewport, available_width));

        let content_available_width =
            (available_width - margin.left - margin.right - horizontal_extra).max(0.0);
        let content_available_height =
            (available_height - margin.top - margin.bottom - vertical_extra).max(0.0);

        let explicit_width = forced_width
            .map(|width| width.max(0.0))
            .or_else(|| {
                definite_content_size(
                    &style.width,
                    &context,
                    Axis::Horizontal,
                    horizontal_extra,
                    style.box_sizing,
                )
            })
            .or_else(|| {
                intrinsic.as_ref().and_then(|intrinsic| {
                    keyword_content_size(
                        &style.width,
                        &context,
                        Axis::Horizontal,
                        content_available_width,
                        *intrinsic,
                    )
                })
            });
        let explicit_height = forced_height
            .map(|height| height.max(0.0))
            .or_else(|| {
                definite_content_size(
                    &style.height,
                    &context,
                    Axis::Vertical,
                    vertical_extra,
                    style.box_sizing,
                )
            })
            .or_else(|| {
                intrinsic.as_ref().and_then(|intrinsic| {
                    keyword_content_size(
                        &style.height,
                        &context,
                        Axis::Vertical,
                        content_available_height,
                        *intrinsic,
                    )
                })
            });

        let inner_available_width = explicit_width.unwrap_or_else(|| {
            (available_width - margin.left - margin.right - horizontal_extra).max(0.0)
        });
        let inner_available_height = explicit_height.unwrap_or_else(|| {
            (available_height - margin.top - margin.bottom - vertical_extra).max(0.0)
        });

        let (mut content_width, mut content_height) = match &mut node.kind {
            NodeKind::Text(content) => {
                let max_width =
                    explicit_width.or_else(|| style.max_width.resolve(&context, Axis::Horizontal));
                let (text_width, text_height) =
                    self.measure_text(&style, &content.text, &context, max_width);
                let indent = style
                    .text
                    .indent
                    .resolve(&context, Axis::Horizontal)
                    .max(0.0);
                let width =
                    explicit_width.unwrap_or_else(|| max_width.unwrap_or(text_width + indent));
                let height = explicit_height.unwrap_or(text_height);
                (width.max(0.0), height.max(0.0))
            }
            NodeKind::Media(media) => {
                let intrinsic_width = media.intrinsic_width.max(1.0);
                let intrinsic_height = media.intrinsic_height.max(1.0);
                let fit = style.effects.object_fit;
                let (box_width, box_height) = match (explicit_width, explicit_height) {
                    (Some(width), Some(height)) => (width, height),
                    (Some(width), None) => (width, width * intrinsic_height / intrinsic_width),
                    (None, Some(height)) => (height * intrinsic_width / intrinsic_height, height),
                    (None, None) => (intrinsic_width, intrinsic_height),
                };
                let (draw_width, draw_height) = fit_size(
                    intrinsic_width,
                    intrinsic_height,
                    box_width.max(1.0),
                    box_height.max(1.0),
                    fit,
                );
                node.layout.draw_width = draw_width;
                node.layout.draw_height = draw_height;
                (box_width, box_height)
            }
            NodeKind::Container => {
                let display = style.display;
                let (intrinsic_width, intrinsic_height) = match display {
                    Display::Grid => self.layout_grid_children(
                        node,
                        viewport,
                        inner_available_width,
                        inner_available_height,
                        explicit_width,
                        explicit_height,
                    ),
                    Display::Flex => self.layout_flex_children(
                        node,
                        viewport,
                        inner_available_width,
                        inner_available_height,
                        explicit_width,
                        explicit_height,
                    ),
                    _ => self.layout_block_children(
                        node,
                        viewport,
                        inner_available_width,
                        inner_available_height,
                        explicit_width,
                        explicit_height,
                    ),
                };
                let width = explicit_width.unwrap_or(intrinsic_width);
                let height = explicit_height.unwrap_or(intrinsic_height);
                (width.max(0.0), height.max(0.0))
            }
        };

        if let Some(ratio) = style.aspect_ratio
            && ratio > 0.0
        {
            match (explicit_width.is_some(), explicit_height.is_some()) {
                (true, false) => content_height = content_width / ratio,
                (false, true) => content_width = content_height * ratio,
                _ => {}
            }
        }

        if let Some(intrinsic) = intrinsic.as_ref() {
            content_width = clamp_content_size(
                content_width,
                &style.min_width,
                &style.max_width,
                &context,
                Axis::Horizontal,
                inner_available_width,
                *intrinsic,
            );
            content_height = clamp_content_size(
                content_height,
                &style.min_height,
                &style.max_height,
                &context,
                Axis::Vertical,
                inner_available_height,
                *intrinsic,
            );
        } else {
            content_width =
                clamp_definite_size(content_width, &style.min_width, &style.max_width, &context);
            content_height = clamp_definite_size(
                content_height,
                &style.min_height,
                &style.max_height,
                &context,
            );
        }

        node.layout.width = content_width + horizontal_extra;
        node.layout.height = content_height + vertical_extra;
        node.layout.content_x = border.left + padding.left;
        node.layout.content_y = border.top + padding.top;
        node.layout.content_width = content_width;
        node.layout.content_height = content_height;
        node.layout.draw_x = 0.0;
        node.layout.draw_y = 0.0;
        if !matches!(node.kind, NodeKind::Media(_)) {
            node.layout.draw_width = content_width;
            node.layout.draw_height = content_height;
        }
        self.position_children(node, viewport);
    }

    fn measure_text(
        &mut self,
        style: &ComputedStyle,
        text: &str,
        context: &ResolveContext,
        max_width: Option<f32>,
    ) -> (f32, f32) {
        let request = ShapeRequest {
            text,
            font_size: style.text.font_size_px,
            line_height: style.text.line_height_px,
            families: &style.text.families,
            weight: style.text.weight,
            style: style.text.style,
            stretch: style.text.stretch,
            caps: style.text.caps,
            features: &style.text.features,
            letter_spacing: style.text.letter_spacing_px,
            word_spacing: style.text.word_spacing_px,
            max_width,
            align: style.text.align,
            align_last: style.text.align_last,
            direction: style.text.direction,
            indent: style
                .text
                .indent
                .resolve(context, Axis::Horizontal)
                .max(0.0),
            white_space: style.text.white_space,
            overflow_wrap: style.text.overflow_wrap,
            word_break: style.text.word_break,
            transform: style.text.transform,
            text_overflow: style.text.overflow,
            line_clamp: style.text.line_clamp,
        };
        self.text.measure(request)
    }

    fn position_children(&mut self, node: &mut Node, viewport: Viewport) {
        let style = node.style.clone();
        let context = viewport.context(
            style.text.font_size_px,
            node.layout.content_width,
            node.layout.content_height,
        );
        let content_width = node.layout.content_width;
        let content_height = node.layout.content_height;
        let offset_x = node.layout.content_x;
        let offset_y = node.layout.content_y;
        // Generated ::before/::after boxes use the parent's padding box as
        // their containing block, like absolutely positioned boxes in CSS.
        let padding_box_x = node.layout.border.left;
        let padding_box_y = node.layout.border.top;
        let padding_box_width =
            (node.layout.width - node.layout.border.left - node.layout.border.right).max(0.0);
        let padding_box_height =
            (node.layout.height - node.layout.border.top - node.layout.border.bottom).max(0.0);

        for child in &mut node.children {
            if child.pseudo_element.is_some()
                && matches!(child.style.position, Position::Absolute | Position::Fixed)
            {
                self.layout_node(child, viewport, padding_box_width, padding_box_height);
                let context = viewport.context(
                    child.style.text.font_size_px,
                    padding_box_width,
                    padding_box_height,
                );
                let left = child
                    .style
                    .left
                    .as_ref()
                    .map(|value| value.resolve(&context, Axis::Horizontal));
                let right = child
                    .style
                    .right
                    .as_ref()
                    .map(|value| value.resolve(&context, Axis::Horizontal));
                let top = child
                    .style
                    .top
                    .as_ref()
                    .map(|value| value.resolve(&context, Axis::Vertical));
                let bottom = child
                    .style
                    .bottom
                    .as_ref()
                    .map(|value| value.resolve(&context, Axis::Vertical));
                let mut width = child.layout.width;
                let mut height = child.layout.height;
                if child.style.width.is_auto()
                    && let (Some(left), Some(right)) = (left, right)
                {
                    width = (padding_box_width - left - right).max(0.0);
                }
                if child.style.height.is_auto()
                    && let (Some(top), Some(bottom)) = (top, bottom)
                {
                    height = (padding_box_height - top - bottom).max(0.0);
                }
                child.layout.width = width;
                child.layout.height = height;
                child.layout.content_width = (width
                    - child.layout.border.left
                    - child.layout.border.right
                    - child.layout.padding.left
                    - child.layout.padding.right)
                    .max(0.0);
                child.layout.content_height = (height
                    - child.layout.border.top
                    - child.layout.border.bottom
                    - child.layout.padding.top
                    - child.layout.padding.bottom)
                    .max(0.0);
                let x = match (left, right) {
                    (Some(left), _) => left,
                    (None, Some(right)) => padding_box_width - right - width,
                    _ => 0.0,
                };
                let y = match (top, bottom) {
                    (Some(top), _) => top,
                    (None, Some(bottom)) => padding_box_height - bottom - height,
                    _ => 0.0,
                };
                child.layout.x = padding_box_x + x;
                child.layout.y = padding_box_y + y;
                continue;
            }
            match child.style.position {
                Position::Absolute | Position::Fixed => {
                    self.layout_node(child, viewport, content_width, content_height);
                    let left = child
                        .style
                        .left
                        .as_ref()
                        .map(|value| value.resolve(&context, Axis::Horizontal));
                    let right = child
                        .style
                        .right
                        .as_ref()
                        .map(|value| value.resolve(&context, Axis::Horizontal));
                    let top = child
                        .style
                        .top
                        .as_ref()
                        .map(|value| value.resolve(&context, Axis::Vertical));
                    let bottom = child
                        .style
                        .bottom
                        .as_ref()
                        .map(|value| value.resolve(&context, Axis::Vertical));
                    let x = match (left, right) {
                        (Some(left), _) => left,
                        (None, Some(right)) => content_width - right - child.layout.width,
                        _ => 0.0,
                    };
                    let y = match (top, bottom) {
                        (Some(top), _) => top,
                        (None, Some(bottom)) => content_height - bottom - child.layout.height,
                        _ => 0.0,
                    };
                    child.layout.x = offset_x + x;
                    child.layout.y = offset_y + y;
                }
                Position::Relative => {
                    let left = child
                        .style
                        .left
                        .as_ref()
                        .map(|value| value.resolve(&context, Axis::Horizontal));
                    let right = child
                        .style
                        .right
                        .as_ref()
                        .map(|value| value.resolve(&context, Axis::Horizontal));
                    let top = child
                        .style
                        .top
                        .as_ref()
                        .map(|value| value.resolve(&context, Axis::Vertical));
                    let bottom = child
                        .style
                        .bottom
                        .as_ref()
                        .map(|value| value.resolve(&context, Axis::Vertical));
                    child.layout.x += match (left, right) {
                        (Some(left), _) => left,
                        (None, Some(right)) => -right,
                        _ => 0.0,
                    };
                    child.layout.y += match (top, bottom) {
                        (Some(top), _) => top,
                        (None, Some(bottom)) => -bottom,
                        _ => 0.0,
                    };
                }
                _ => {}
            }
        }
    }

    // -----------------------------------------------------------------
    // Block formatting context
    // -----------------------------------------------------------------

    fn layout_block_children(
        &mut self,
        node: &mut Node,
        viewport: Viewport,
        available_width: f32,
        available_height: f32,
        explicit_content_width: Option<f32>,
        explicit_content_height: Option<f32>,
    ) -> (f32, f32) {
        let style = node.style.clone();
        let context = viewport.context(style.text.font_size_px, available_width, available_height);
        let padding = style.padding.resolve(&context, Axis::Horizontal);
        let border = style.border.widths.resolve(&context, Axis::Horizontal);
        let has_top_barrier = border.top > 0.0 || padding.top > 0.0;
        let has_bottom_barrier = border.bottom > 0.0 || padding.bottom > 0.0;

        let flow_indices: Vec<usize> = (0..node.children.len())
            .filter(|index| !out_of_flow(&node.children[*index]))
            .collect();

        let content_width = explicit_content_width.unwrap_or(available_width.max(0.0));
        let content_offset_x = border.left + padding.left;
        let content_offset_y = border.top + padding.top;
        let mut cursor_y = 0.0;
        let mut previous_bottom: Option<f32> = None;
        let mut first_top = 0.0;
        let mut last_bottom = 0.0;
        let mut max_right: f32 = 0.0;

        for index in &flow_indices {
            let child = &mut node.children[*index];
            self.layout_node(child, viewport, content_width, available_height.max(0.0));

            // Block-level boxes with an automatic width fill the containing
            // block. Text and media keep their intrinsic size and behave like
            // atomic inline boxes, which is what the wallpaper DSL expects.
            let block_level = matches!(
                child.style.display,
                Display::Block | Display::Flex | Display::Grid
            ) && matches!(child.kind, NodeKind::Container);
            if block_level && child.style.width.is_auto() {
                let extra = child.layout.width - child.layout.content_width;
                let margin = &child.layout.margin;
                let forced = (content_width - margin.left - margin.right - extra).max(0.0);
                self.layout_node_with(
                    child,
                    viewport,
                    content_width,
                    available_height.max(0.0),
                    Some(forced),
                    None,
                );
            }

            let margin = child.layout.margin.clone();
            let gap = match previous_bottom {
                None => {
                    if has_top_barrier {
                        margin.top
                    } else {
                        first_top = margin.top;
                        0.0
                    }
                }
                Some(bottom) => collapse_margins(bottom, margin.top),
            };
            let x = block_horizontal_offset(
                &child.style.margin,
                &margin,
                content_width,
                child.layout.width,
            );
            child.layout.x = content_offset_x + x;
            child.layout.y = content_offset_y + cursor_y + gap;
            cursor_y += gap + child.layout.height;
            previous_bottom = Some(margin.bottom);
            last_bottom = margin.bottom;
            max_right = max_right.max(
                child.layout.x
                    + child.layout.width
                    + if margin.right > 0.0 {
                        margin.right
                    } else {
                        0.0
                    },
            );
        }

        // Margins of the first/last in-flow child collapse with the parent
        // when no border or padding separates them.
        if !has_top_barrier && !flow_indices.is_empty() {
            node.layout.margin.top = collapse_margins(node.layout.margin.top, first_top);
        }
        if !has_bottom_barrier && explicit_content_height.is_none() && !flow_indices.is_empty() {
            node.layout.margin.bottom = collapse_margins(node.layout.margin.bottom, last_bottom);
        }

        let height = explicit_content_height.unwrap_or_else(|| {
            let mut height = cursor_y;
            if has_bottom_barrier {
                height += last_bottom;
            }
            height.max(0.0)
        });
        let width = explicit_content_width.unwrap_or(max_right.max(0.0));
        (width, height)
    }

    // -----------------------------------------------------------------
    // Flex formatting context
    // -----------------------------------------------------------------

    fn layout_flex_children(
        &mut self,
        node: &mut Node,
        viewport: Viewport,
        available_width: f32,
        available_height: f32,
        explicit_content_width: Option<f32>,
        explicit_content_height: Option<f32>,
    ) -> (f32, f32) {
        let style = node.style.clone();
        let direction = style.flex.direction;
        let is_row = matches!(direction, FlexDirection::Row | FlexDirection::RowReverse);
        let reverse = matches!(
            direction,
            FlexDirection::RowReverse | FlexDirection::ColumnReverse
        );
        let context = viewport.context(style.text.font_size_px, available_width, available_height);
        let padding = style.padding.resolve(&context, Axis::Horizontal);
        let border = style.border.widths.resolve(&context, Axis::Horizontal);
        let horizontal_extra = padding.left + padding.right + border.left + border.right;
        let vertical_extra = padding.top + padding.bottom + border.top + border.bottom;

        let mut flow_indices: Vec<usize> = (0..node.children.len())
            .filter(|index| !out_of_flow(&node.children[*index]))
            .collect();
        flow_indices.sort_by_key(|index| node.children[*index].style.flex.order);
        if reverse {
            flow_indices.reverse();
        }

        let (explicit_main, explicit_cross) = if is_row {
            (explicit_content_width, explicit_content_height)
        } else {
            (explicit_content_height, explicit_content_width)
        };
        let main_available = explicit_main.unwrap_or_else(|| {
            let available = if is_row {
                available_width - horizontal_extra
            } else {
                available_height - vertical_extra
            };
            available.max(0.0)
        });
        let main_axis = if is_row {
            Axis::Horizontal
        } else {
            Axis::Vertical
        };
        let main_basis_context = viewport.context(
            style.text.font_size_px,
            if is_row {
                main_available
            } else {
                available_width
            },
            if is_row {
                available_height
            } else {
                main_available
            },
        );

        let main_gap = if is_row {
            style.flex.column_gap.resolve(&context, Axis::Horizontal)
        } else {
            style.flex.row_gap.resolve(&context, Axis::Vertical)
        }
        .max(0.0);
        let cross_gap = if is_row {
            style.flex.row_gap.resolve(&context, Axis::Vertical)
        } else {
            style.flex.column_gap.resolve(&context, Axis::Horizontal)
        }
        .max(0.0);

        // First pass: lay children out with their natural size so auto bases
        // and cross sizes are known.
        for index in &flow_indices {
            self.layout_node(
                &mut node.children[*index],
                viewport,
                available_width.max(0.0),
                available_height.max(0.0),
            );
        }

        #[derive(Clone, Copy)]
        struct Item {
            index: usize,
            base: f32,
            extra: f32,
            min: f32,
            max: f32,
            final_main: f32,
        }

        let mut items: Vec<Item> = Vec::with_capacity(flow_indices.len());
        for index in &flow_indices {
            let child = &node.children[*index];
            let child_context = viewport.context(
                child.style.text.font_size_px,
                available_width.max(0.0),
                available_height.max(0.0),
            );
            let extra = if is_row {
                child.layout.padding.left
                    + child.layout.padding.right
                    + child.layout.border.left
                    + child.layout.border.right
            } else {
                child.layout.padding.top
                    + child.layout.padding.bottom
                    + child.layout.border.top
                    + child.layout.border.bottom
            };
            let automatic_min = if is_row {
                child.style.min_width.is_auto()
            } else {
                child.style.min_height.is_auto()
            };
            let intrinsic = if matches!(
                child.style.flex.basis,
                Size::MinContent | Size::MaxContent | Size::FitContent(_) | Size::Stretch
            ) || automatic_min
            {
                Some(self.content_intrinsic_size(child, viewport, main_available))
            } else {
                None
            };
            let explicit_main = if is_row {
                definite_content_size(
                    &child.style.width,
                    &child_context,
                    Axis::Horizontal,
                    extra,
                    child.style.box_sizing,
                )
            } else {
                definite_content_size(
                    &child.style.height,
                    &child_context,
                    Axis::Vertical,
                    extra,
                    child.style.box_sizing,
                )
            };
            let measured_main = if is_row {
                child.layout.content_width
            } else {
                child.layout.content_height
            };
            let mut base = match &child.style.flex.basis {
                Size::Length(_) => definite_content_size(
                    &child.style.flex.basis,
                    &main_basis_context,
                    main_axis,
                    extra,
                    child.style.box_sizing,
                ),
                Size::Stretch => Some(main_available),
                Size::MinContent => intrinsic.map(|size| size.axis(main_axis, true)),
                Size::MaxContent => intrinsic.map(|size| size.axis(main_axis, false)),
                Size::FitContent(limit) => intrinsic.map(|size| {
                    let resolved = limit.resolve(&main_basis_context, main_axis).max(0.0);
                    size.axis(main_axis, false)
                        .min(resolved)
                        .max(size.axis(main_axis, true))
                }),
                Size::Auto | Size::None => None,
            }
            .or(explicit_main)
            .unwrap_or(measured_main);
            let min_size = min_content_size(
                &if is_row {
                    child.style.min_width.clone()
                } else {
                    child.style.min_height.clone()
                },
                &child_context,
                main_axis,
                intrinsic,
                true,
            );
            let max_size = max_content_size(
                &if is_row {
                    child.style.max_width.clone()
                } else {
                    child.style.max_height.clone()
                },
                &child_context,
                main_axis,
                intrinsic,
            );
            base = base.clamp(min_size, max_size.max(min_size));
            items.push(Item {
                index: *index,
                base,
                extra,
                min: min_size,
                max: max_size.max(min_size),
                final_main: base,
            });
        }

        let main_size = |child: &Node, item: &Item| {
            let margin = if is_row {
                child.layout.margin.left + child.layout.margin.right
            } else {
                child.layout.margin.top + child.layout.margin.bottom
            };
            item.base + item.extra + margin
        };
        let cross_size = |child: &Node| {
            if is_row {
                child.layout.margin.top + child.layout.height + child.layout.margin.bottom
            } else {
                child.layout.margin.left + child.layout.width + child.layout.margin.right
            }
        };

        // Group items into flex lines using the flex base size.
        let wraps = !matches!(style.flex.wrap, FlexWrap::NoWrap);
        let mut lines: Vec<Vec<usize>> = Vec::new();
        let mut line: Vec<usize> = Vec::new();
        let mut line_main = 0.0;
        for (item_index, item) in items.iter().enumerate() {
            let child = &node.children[item.index];
            let size = main_size(child, item);
            let advance = size + if line.is_empty() { 0.0 } else { main_gap };
            if wraps && !line.is_empty() && line_main + advance > main_available {
                lines.push(std::mem::take(&mut line));
                line_main = 0.0;
            }
            line_main += size + if line.is_empty() { 0.0 } else { main_gap };
            line.push(item_index);
        }
        if !line.is_empty() {
            lines.push(line);
        }
        if matches!(style.flex.wrap, FlexWrap::WrapReverse) {
            lines.reverse();
        }

        // Resolve grow/shrink from the flex base, then lay the item out again
        // with the final main size.
        if let Some(main_limit) = explicit_main {
            for line in &lines {
                let base_total = line
                    .iter()
                    .map(|index| main_size(&node.children[items[*index].index], &items[*index]))
                    .sum::<f32>()
                    + main_gap * line.len().saturating_sub(1) as f32;
                let free = main_limit - base_total;
                if free.abs() <= 0.01 {
                    continue;
                }
                let total_grow = line
                    .iter()
                    .map(|index| node.children[items[*index].index].style.flex.grow.max(0.0))
                    .sum::<f32>();
                let total_shrink = line
                    .iter()
                    .map(|index| {
                        let item = &items[*index];
                        let child = &node.children[item.index];
                        child.style.flex.shrink.max(0.0) * item.base
                    })
                    .sum::<f32>();
                for item_index in line {
                    let item = &mut items[*item_index];
                    let child = &node.children[item.index];
                    let delta = if free > 0.0 && total_grow > 0.0 {
                        free * child.style.flex.grow.max(0.0) / total_grow
                    } else if free < 0.0 && total_shrink > 0.0 {
                        free * (child.style.flex.shrink.max(0.0) * item.base) / total_shrink
                    } else {
                        0.0
                    };
                    item.final_main = (item.base + delta).clamp(item.min, item.max);
                }
            }
        }

        for item in &items {
            let child = &mut node.children[item.index];
            // A definite flex base (or explicit container size) must be
            // applied even when it equals the measured size, because
            // `flex-basis` overrides `width`/`height`.
            let has_definite_basis = !matches!(child.style.flex.basis, Size::Auto | Size::None);
            if explicit_main.is_some()
                || has_definite_basis
                || (item.final_main - item.base).abs() > 0.01
            {
                if is_row {
                    self.layout_node_with(
                        child,
                        viewport,
                        available_width,
                        available_height,
                        Some(item.final_main),
                        None,
                    );
                } else {
                    self.layout_node_with(
                        child,
                        viewport,
                        available_width,
                        available_height,
                        None,
                        Some(item.final_main),
                    );
                }
            }
        }

        let line_cross: Vec<f32> = lines
            .iter()
            .map(|line| {
                line.iter()
                    .map(|index| cross_size(&node.children[items[*index].index]))
                    .fold(0.0, f32::max)
            })
            .collect();
        let lines_count = lines.len();
        let total_cross =
            line_cross.iter().sum::<f32>() + cross_gap * lines_count.saturating_sub(1) as f32;
        let max_line_main = lines
            .iter()
            .map(|line| {
                line.iter()
                    .map(|index| main_size(&node.children[items[*index].index], &items[*index]))
                    .sum::<f32>()
                    + main_gap * line.len().saturating_sub(1) as f32
            })
            .fold(0.0, f32::max);
        let content_main = explicit_main.unwrap_or(max_line_main);
        let content_cross = explicit_cross.unwrap_or(total_cross);
        // Stretch items whose cross size is auto. This is what `align-items:
        // stretch` expects on both flex and grid paths.
        for (line_index, line) in lines.iter().enumerate() {
            for item_index in line {
                let item = &items[*item_index];
                let child = &mut node.children[item.index];
                let align = effective_flex_align(&style, child);
                let cross_auto = if is_row {
                    child.style.height.is_auto()
                } else {
                    child.style.width.is_auto()
                };
                if align == AlignItems::Stretch && cross_auto && explicit_cross.is_some() {
                    let cross_extra = if is_row {
                        child.layout.padding.top
                            + child.layout.padding.bottom
                            + child.layout.border.top
                            + child.layout.border.bottom
                    } else {
                        child.layout.padding.left
                            + child.layout.padding.right
                            + child.layout.border.left
                            + child.layout.border.right
                    };
                    let margins = if is_row {
                        child.layout.margin.top + child.layout.margin.bottom
                    } else {
                        child.layout.margin.left + child.layout.margin.right
                    };
                    let target = (line_cross[line_index] - margins - cross_extra).max(0.0);
                    if target > 0.0 {
                        if is_row {
                            self.layout_node_with(
                                child,
                                viewport,
                                available_width,
                                available_height,
                                Some(item.final_main),
                                Some(target),
                            );
                        } else {
                            self.layout_node_with(
                                child,
                                viewport,
                                available_width,
                                available_height,
                                Some(target),
                                Some(item.final_main),
                            );
                        }
                    }
                }
            }
        }

        let free_cross = (content_cross - total_cross).max(0.0);
        let (mut cross_cursor, line_extra) =
            distribution(style.flex.align_content, free_cross, lines_count);

        let content_offset_x = border.left + padding.left;
        let content_offset_y = border.top + padding.top;

        for (line_index, line) in lines.iter().enumerate() {
            let line_main_total = line
                .iter()
                .map(|index| main_size(&node.children[items[*index].index], &items[*index]))
                .sum::<f32>()
                + main_gap * line.len().saturating_sub(1) as f32;
            let free_main = (content_main - line_main_total).max(0.0);
            let (mut cursor, extra_gap) = match style.flex.justify {
                JustifyContent::Start => (0.0, 0.0),
                JustifyContent::End => (free_main, 0.0),
                JustifyContent::Center => (free_main / 2.0, 0.0),
                JustifyContent::SpaceBetween => {
                    if line.len() > 1 {
                        (0.0, free_main / (line.len() - 1) as f32)
                    } else {
                        (0.0, 0.0)
                    }
                }
                JustifyContent::SpaceAround => {
                    if line.is_empty() {
                        (0.0, 0.0)
                    } else {
                        let each = free_main / line.len() as f32;
                        (each / 2.0, each)
                    }
                }
                JustifyContent::SpaceEvenly => {
                    if line.is_empty() {
                        (0.0, 0.0)
                    } else {
                        let each = free_main / (line.len() + 1) as f32;
                        (each, each)
                    }
                }
            };

            let line_height = line_cross[line_index];
            for item_index in line {
                let item = &items[*item_index];
                let child = &mut node.children[item.index];
                let align = effective_flex_align(&style, child);
                let child_cross = cross_size(child);
                let cross_offset = match align {
                    AlignItems::Start | AlignItems::Baseline | AlignItems::Stretch => 0.0,
                    AlignItems::End => (line_height - child_cross).max(0.0),
                    AlignItems::Center => ((line_height - child_cross) / 2.0).max(0.0),
                };
                if is_row {
                    child.layout.x = content_offset_x + cursor + child.layout.margin.left;
                    child.layout.y =
                        content_offset_y + cross_cursor + cross_offset + child.layout.margin.top;
                } else {
                    child.layout.y = content_offset_y + cursor + child.layout.margin.top;
                    child.layout.x =
                        content_offset_x + cross_cursor + cross_offset + child.layout.margin.left;
                }
                cursor += main_size(child, item) + main_gap + extra_gap;
            }
            cross_cursor += line_height + cross_gap + line_extra;
        }

        if is_row {
            (content_main, content_cross)
        } else {
            (content_cross, content_main)
        }
    }

    // -----------------------------------------------------------------
    // Grid formatting context
    // -----------------------------------------------------------------

    fn layout_grid_children(
        &mut self,
        node: &mut Node,
        viewport: Viewport,
        available_width: f32,
        available_height: f32,
        explicit_content_width: Option<f32>,
        explicit_content_height: Option<f32>,
    ) -> (f32, f32) {
        let style = node.style.clone();
        let context = viewport.context(style.text.font_size_px, available_width, available_height);
        let padding = style.padding.resolve(&context, Axis::Horizontal);
        let border = style.border.widths.resolve(&context, Axis::Horizontal);
        let column_gap = style
            .flex
            .column_gap
            .resolve(&context, Axis::Horizontal)
            .max(0.0);
        let row_gap = style
            .flex
            .row_gap
            .resolve(&context, Axis::Vertical)
            .max(0.0);

        let mut flow_indices: Vec<usize> = (0..node.children.len())
            .filter(|index| !out_of_flow(&node.children[*index]))
            .collect();
        flow_indices.sort_by_key(|index| node.children[*index].style.flex.order);

        let explicit_columns = flatten_tracks(&style.grid.columns);
        let explicit_rows = flatten_tracks(&style.grid.rows);
        let areas = &style.grid.areas;

        // Resolve each item's explicit lines and spans.
        #[derive(Clone, Copy)]
        struct Placement {
            column: Option<usize>,
            row: Option<usize>,
            column_span: usize,
            row_span: usize,
            column_definite: bool,
            row_definite: bool,
        }

        let mut placements: Vec<Option<Placement>> = vec![None; node.children.len()];
        for index in &flow_indices {
            let child = &node.children[*index];
            let (column, column_span, column_definite) = resolve_grid_axis(
                &child.style.grid.column,
                areas,
                false,
                explicit_columns.len(),
            );
            let (row, row_span, row_definite) =
                resolve_grid_axis(&child.style.grid.row, areas, true, explicit_rows.len());
            placements[*index] = Some(Placement {
                column,
                row,
                column_span,
                row_span,
                column_definite,
                row_definite,
            });
        }

        let area_columns = areas
            .values()
            .map(|area| area.column + area.column_span)
            .max()
            .unwrap_or(0);
        let area_rows = areas
            .values()
            .map(|area| area.row + area.row_span)
            .max()
            .unwrap_or(0);

        let row_flow = matches!(
            style.grid.auto_flow,
            GridAutoFlow::Row | GridAutoFlow::RowDense
        );
        let dense = matches!(
            style.grid.auto_flow,
            GridAutoFlow::RowDense | GridAutoFlow::ColumnDense
        );

        let mut columns = explicit_columns.len().max(area_columns).max(1);
        let mut rows = explicit_rows.len().max(area_rows);
        for placement in placements.iter().flatten() {
            if let (Some(column), true) = (placement.column, placement.column_definite) {
                columns = columns.max(column + placement.column_span);
            }
            if let (Some(row), true) = (placement.row, placement.row_definite) {
                rows = rows.max(row + placement.row_span);
            }
        }
        if !flow_indices.is_empty() && rows == 0 {
            rows = 1;
        }

        let mut occupied = vec![vec![false; columns]; rows.max(1)];
        let mut resolved: Vec<Option<(usize, usize, usize, usize)>> =
            vec![None; node.children.len()];

        // Place items with a definite position first.
        for index in &flow_indices {
            let placement = placements[*index].expect("placement");
            if let (Some(column), Some(row)) = (placement.column, placement.row)
                && (placement.column_definite || placement.row_definite)
            {
                ensure_grid_capacity(
                    &mut occupied,
                    row + placement.row_span,
                    column + placement.column_span,
                );
                rows = rows.max(row + placement.row_span);
                columns = columns.max(column + placement.column_span);
                mark_grid_cell(
                    &mut occupied,
                    row,
                    column,
                    placement.row_span,
                    placement.column_span,
                );
                resolved[*index] = Some((column, row, placement.column_span, placement.row_span));
            }
        }

        // Auto placement. Sparse keeps a cursor in flow order; dense restarts
        // from the origin for every item, which fills earlier holes.
        let mut cursor_row = 0usize;
        let mut cursor_column = 0usize;
        for index in &flow_indices {
            if resolved[*index].is_some() {
                continue;
            }
            let placement = placements[*index].expect("placement");
            let column_span = placement.column_span.max(1);
            let row_span = placement.row_span.max(1);

            let (column, row) = if row_flow {
                if let Some(row) = placement.row {
                    ensure_grid_capacity(&mut occupied, row + row_span, columns.max(column_span));
                    let column =
                        find_grid_column(&occupied, row, 0, columns, column_span, row_span)
                            .unwrap_or_else(|| {
                                let column = columns;
                                columns += column_span;
                                grow_grid(&mut occupied, rows, columns);
                                column
                            });
                    (column, row)
                } else if let Some(column) = placement.column {
                    if column + column_span > columns {
                        columns = column + column_span;
                        grow_grid(&mut occupied, rows, columns);
                    }
                    let start_row = if dense { 0 } else { cursor_row };
                    let row =
                        find_grid_row(&occupied, column, start_row, rows, column_span, row_span)
                            .unwrap_or_else(|| {
                                let row = rows;
                                rows += row_span;
                                grow_grid(&mut occupied, rows, columns);
                                row
                            });
                    (column, row)
                } else {
                    let (start_row, start_column) = if dense {
                        (0, 0)
                    } else {
                        (cursor_row, cursor_column)
                    };
                    place_auto_row_flow(
                        &mut occupied,
                        &mut rows,
                        columns,
                        start_row,
                        start_column,
                        column_span,
                        row_span,
                    )
                }
            } else if let Some(column) = placement.column {
                ensure_grid_capacity(&mut occupied, rows.max(row_span), column + column_span);
                let start_row = if dense { 0 } else { cursor_row };
                let row = find_grid_row(&occupied, column, start_row, rows, column_span, row_span)
                    .unwrap_or_else(|| {
                        let row = rows;
                        rows += row_span;
                        grow_grid(&mut occupied, rows, columns);
                        row
                    });
                (column, row)
            } else if let Some(row) = placement.row {
                if row + row_span > rows {
                    rows = row + row_span;
                    grow_grid(&mut occupied, rows, columns);
                }
                let start_column = if dense { 0 } else { cursor_column };
                let column =
                    find_grid_column(&occupied, row, start_column, columns, column_span, row_span)
                        .unwrap_or_else(|| {
                            let column = columns;
                            columns += column_span;
                            grow_grid(&mut occupied, rows, columns);
                            column
                        });
                (column, row)
            } else {
                let (start_row, start_column) = if dense {
                    (0, 0)
                } else {
                    (cursor_row, cursor_column)
                };
                place_auto_column_flow(
                    &mut occupied,
                    &mut columns,
                    rows,
                    start_row,
                    start_column,
                    column_span,
                    row_span,
                )
            };

            ensure_grid_capacity(&mut occupied, row + row_span, column + column_span);
            rows = rows.max(row + row_span);
            columns = columns.max(column + column_span);
            mark_grid_cell(&mut occupied, row, column, row_span, column_span);
            resolved[*index] = Some((column, row, column_span, row_span));

            if row_flow {
                cursor_row = row;
                cursor_column = column + column_span;
                if cursor_column >= columns {
                    cursor_row = row + 1;
                    cursor_column = 0;
                }
            } else {
                cursor_column = column;
                cursor_row = row + row_span;
                if cursor_row >= rows {
                    cursor_column = column + 1;
                    cursor_row = 0;
                }
            }
        }

        rows = rows.max(1);
        columns = columns.max(1);
        let column_defs: Vec<TrackSize> = (0..columns)
            .map(|index| track_or_auto(&explicit_columns, &style.grid.auto_columns, index))
            .collect();
        let row_defs: Vec<TrackSize> = (0..rows)
            .map(|index| track_or_auto(&explicit_rows, &style.grid.auto_rows, index))
            .collect();

        // Column intrinsic contributions include margins and box extras.
        let mut column_contributions = Vec::new();
        for index in &flow_indices {
            let Some((column, _, column_span, _)) = resolved[*index] else {
                continue;
            };
            let child = &node.children[*index];
            let (min_width, max_width) =
                self.grid_item_intrinsic_width(child, viewport, available_width);
            let margins = child.style.margin.resolve(
                &viewport.context(
                    child.style.text.font_size_px,
                    available_width,
                    available_height,
                ),
                Axis::Horizontal,
            );
            column_contributions.push((
                column,
                column_span.max(1),
                min_width + margins.left + margins.right,
                max_width + margins.left + margins.right,
            ));
        }
        let column_widths = size_tracks(
            &column_defs,
            &column_contributions,
            column_gap,
            explicit_content_width,
            &context,
            Axis::Horizontal,
        );

        // Lay items out at their column spans to obtain real heights, then
        // size the rows from those heights.
        for index in &flow_indices {
            let Some((column, _, column_span, _)) = resolved[*index] else {
                continue;
            };
            let child = &mut node.children[*index];
            let span_width = column_widths[column..column + column_span]
                .iter()
                .sum::<f32>()
                + column_gap * column_span.saturating_sub(1) as f32;
            let extra = child.layout.width - child.layout.content_width;
            let forced_width = if child.style.width.is_auto() {
                Some(
                    (span_width - child.layout.margin.left - child.layout.margin.right - extra)
                        .max(0.0),
                )
            } else {
                None
            };
            self.layout_node_with(
                child,
                viewport,
                available_width,
                available_height,
                forced_width,
                None,
            );
        }

        let mut row_contributions = Vec::new();
        for index in &flow_indices {
            let Some((_, row, _, row_span)) = resolved[*index] else {
                continue;
            };
            let child = &node.children[*index];
            let height = child.layout.height + child.layout.margin.top + child.layout.margin.bottom;
            row_contributions.push((row, row_span.max(1), height, height));
        }
        let row_heights = size_tracks(
            &row_defs,
            &row_contributions,
            row_gap,
            explicit_content_height,
            &context,
            Axis::Vertical,
        );

        let content_width =
            column_widths.iter().sum::<f32>() + (columns.saturating_sub(1)) as f32 * column_gap;
        let content_height =
            row_heights.iter().sum::<f32>() + (rows.saturating_sub(1)) as f32 * row_gap;

        let free_x = explicit_content_width
            .map(|width| (width - content_width).max(0.0))
            .unwrap_or(0.0);
        let free_y = explicit_content_height
            .map(|height| (height - content_height).max(0.0))
            .unwrap_or(0.0);
        let (offset_x, extra_x) = grid_content_distribution(style.flex.justify, free_x, columns);
        let (offset_y, extra_y) = grid_align_distribution(style.flex.align_content, free_y, rows);

        let content_offset_x = border.left + padding.left + offset_x;
        let content_offset_y = border.top + padding.top + offset_y;

        for index in &flow_indices {
            let Some((column, row, column_span, row_span)) = resolved[*index] else {
                continue;
            };
            let child = &mut node.children[*index];
            let area_x = column_widths[..column].iter().sum::<f32>()
                + column as f32 * (column_gap + extra_x);
            let area_y = row_heights[..row].iter().sum::<f32>() + row as f32 * (row_gap + extra_y);
            let span_width = column_widths[column..column + column_span]
                .iter()
                .sum::<f32>()
                + (column_span.saturating_sub(1)) as f32 * column_gap;
            let span_height = row_heights[row..row + row_span].iter().sum::<f32>()
                + (row_span.saturating_sub(1)) as f32 * row_gap;

            let justify = effective_grid_justify(&style, child);
            let align = effective_grid_align(&style, child);
            let extra_x = child.layout.width - child.layout.content_width;
            let extra_y = child.layout.height - child.layout.content_height;
            let stretch_width = justify == AlignItems::Stretch && child.style.width.is_auto();
            let stretch_height = align == AlignItems::Stretch && child.style.height.is_auto();
            let forced_width = stretch_width.then(|| {
                (span_width - child.layout.margin.left - child.layout.margin.right - extra_x)
                    .max(0.0)
            });
            let forced_height = stretch_height.then(|| {
                (span_height - child.layout.margin.top - child.layout.margin.bottom - extra_y)
                    .max(0.0)
            });
            if forced_width.is_some() || forced_height.is_some() {
                self.layout_node_with(
                    child,
                    viewport,
                    available_width,
                    available_height,
                    forced_width,
                    forced_height,
                );
            }

            let margin_box_width =
                child.layout.width + child.layout.margin.left + child.layout.margin.right;
            let margin_box_height =
                child.layout.height + child.layout.margin.top + child.layout.margin.bottom;
            let justify_offset = match justify {
                AlignItems::Start | AlignItems::Stretch | AlignItems::Baseline => 0.0,
                AlignItems::End => (span_width - margin_box_width).max(0.0),
                AlignItems::Center => ((span_width - margin_box_width) / 2.0).max(0.0),
            };
            let align_offset = match align {
                AlignItems::Start | AlignItems::Stretch | AlignItems::Baseline => 0.0,
                AlignItems::End => (span_height - margin_box_height).max(0.0),
                AlignItems::Center => ((span_height - margin_box_height) / 2.0).max(0.0),
            };

            child.layout.x = content_offset_x + area_x + child.layout.margin.left + justify_offset;
            child.layout.y = content_offset_y + area_y + child.layout.margin.top + align_offset;
        }

        (content_width, content_height)
    }

    fn grid_item_intrinsic_width(
        &mut self,
        child: &Node,
        viewport: Viewport,
        available_width: f32,
    ) -> (f32, f32) {
        let intrinsic = self.intrinsic_size(child, viewport, available_width);
        let context = viewport.context(
            child.style.text.font_size_px,
            available_width,
            viewport.height,
        );
        let horizontal = child.style.padding.resolve(&context, Axis::Horizontal);
        let border = child
            .style
            .border
            .widths
            .resolve(&context, Axis::Horizontal);
        let extras = horizontal.left + horizontal.right + border.left + border.right;
        let definite = definite_content_size(
            &child.style.width,
            &context,
            Axis::Horizontal,
            extras,
            child.style.box_sizing,
        )
        .map(|width| width + extras);
        let (min_width, max_width) = match &child.style.width {
            Size::Length(_) => (
                definite.unwrap_or(intrinsic.min_width + extras),
                definite.unwrap_or(intrinsic.max_width + extras),
            ),
            Size::Stretch => (available_width, available_width),
            Size::MinContent => (intrinsic.min_width + extras, intrinsic.min_width + extras),
            Size::MaxContent | Size::FitContent(_) | Size::Auto | Size::None => {
                (intrinsic.min_width + extras, intrinsic.max_width + extras)
            }
        };
        (min_width, max_width)
    }

    // -----------------------------------------------------------------
    // Intrinsic sizing
    // -----------------------------------------------------------------

    fn intrinsic_size(
        &mut self,
        node: &Node,
        viewport: Viewport,
        available_width: f32,
    ) -> IntrinsicSize {
        self.intrinsic_size_inner(node, viewport, available_width, true)
    }

    /// Content-based intrinsic size: the node's own explicit width/height is
    /// ignored, but fixed child sizes still contribute. This is what
    /// `min-width: auto` needs for flex items.
    fn content_intrinsic_size(
        &mut self,
        node: &Node,
        viewport: Viewport,
        available_width: f32,
    ) -> IntrinsicSize {
        self.intrinsic_size_inner(node, viewport, available_width, false)
    }

    fn intrinsic_size_inner(
        &mut self,
        node: &Node,
        viewport: Viewport,
        available_width: f32,
        apply_own_size: bool,
    ) -> IntrinsicSize {
        let style = &node.style;
        let context = viewport.context(style.text.font_size_px, available_width, viewport.height);
        let horizontal = style.padding.resolve(&context, Axis::Horizontal);
        let vertical = style.padding.resolve(&context, Axis::Vertical);
        let border = style.border.widths.resolve(&context, Axis::Horizontal);
        let vertical_border = style.border.widths.resolve(&context, Axis::Vertical);
        let horizontal_extra = horizontal.left + horizontal.right + border.left + border.right;
        let vertical_extra =
            vertical.top + vertical.bottom + vertical_border.top + vertical_border.bottom;

        let mut result = match &node.kind {
            NodeKind::Text(content) => {
                let (max_width, max_height) =
                    self.measure_text(style, &content.text, &context, None);
                let indent = style
                    .text
                    .indent
                    .resolve(&context, Axis::Horizontal)
                    .max(0.0);
                let nowrap = matches!(
                    style.text.white_space,
                    crate::css::properties::WhiteSpace::NoWrap
                        | crate::css::properties::WhiteSpace::Pre
                );
                let min_width = if nowrap {
                    max_width
                } else {
                    self.measure_text(style, &content.text, &context, Some(0.0))
                        .0
                };
                // The narrow shape is only used for the min-content width.
                // Its height reflects one word per line, which is not the
                // min-content block size; the natural height is.
                IntrinsicSize {
                    min_width: (min_width + indent).max(0.0) + horizontal_extra,
                    max_width: (max_width + indent).max(0.0) + horizontal_extra,
                    min_height: max_height + vertical_extra,
                    max_height: max_height + vertical_extra,
                }
            }
            NodeKind::Media(media) => IntrinsicSize {
                min_width: media.intrinsic_width + horizontal_extra,
                max_width: media.intrinsic_width + horizontal_extra,
                min_height: media.intrinsic_height + vertical_extra,
                max_height: media.intrinsic_height + vertical_extra,
            },
            NodeKind::Container => match style.display {
                Display::Grid => self.grid_intrinsic(node, viewport, available_width),
                Display::Flex => self.flex_intrinsic(node, viewport, available_width),
                _ => self.block_intrinsic(node, viewport, available_width),
            },
        };

        if apply_own_size {
            if let Some(width) = definite_content_size(
                &style.width,
                &context,
                Axis::Horizontal,
                horizontal_extra,
                style.box_sizing,
            ) {
                result.min_width = width + horizontal_extra;
                result.max_width = width + horizontal_extra;
            }
            if let Some(height) = definite_content_size(
                &style.height,
                &context,
                Axis::Vertical,
                vertical_extra,
                style.box_sizing,
            ) {
                result.min_height = height + vertical_extra;
                result.max_height = height + vertical_extra;
            }
        }
        let min_width = min_content_size(
            &style.min_width,
            &context,
            Axis::Horizontal,
            Some(result),
            false,
        );
        let max_width =
            max_content_size(&style.max_width, &context, Axis::Horizontal, Some(result));
        let min_height = min_content_size(
            &style.min_height,
            &context,
            Axis::Vertical,
            Some(result),
            false,
        );
        let max_height =
            max_content_size(&style.max_height, &context, Axis::Vertical, Some(result));
        result.min_width = result.min_width.max(min_width).min(max_width);
        result.max_width = result.max_width.max(result.min_width).min(max_width);
        result.min_height = result.min_height.max(min_height).min(max_height);
        result.max_height = result.max_height.max(result.min_height).min(max_height);
        result
    }

    fn block_intrinsic(
        &mut self,
        node: &Node,
        viewport: Viewport,
        available_width: f32,
    ) -> IntrinsicSize {
        let mut result = IntrinsicSize::default();
        for child in &node.children {
            if out_of_flow(child) {
                continue;
            }
            let intrinsic = self.intrinsic_size(child, viewport, available_width);
            let context = viewport.context(
                child.style.text.font_size_px,
                available_width,
                viewport.height,
            );
            let margin = child.style.margin.resolve(&context, Axis::Horizontal);
            result.max_width = result
                .max_width
                .max(intrinsic.max_width + margin.left + margin.right);
            result.min_width = result
                .min_width
                .max(intrinsic.min_width + margin.left + margin.right);
            result.max_height += intrinsic.max_height + margin.top + margin.bottom;
            result.min_height += intrinsic.min_height + margin.top + margin.bottom;
        }
        result
    }

    fn flex_intrinsic(
        &mut self,
        node: &Node,
        viewport: Viewport,
        available_width: f32,
    ) -> IntrinsicSize {
        let style = &node.style;
        let is_row = matches!(
            style.flex.direction,
            FlexDirection::Row | FlexDirection::RowReverse
        );
        let context = viewport.context(style.text.font_size_px, available_width, viewport.height);
        let gap = if is_row {
            style.flex.column_gap.resolve(&context, Axis::Horizontal)
        } else {
            style.flex.row_gap.resolve(&context, Axis::Vertical)
        }
        .max(0.0);
        let wrap = !matches!(style.flex.wrap, FlexWrap::NoWrap);
        let mut min_main: f32 = 0.0;
        let mut max_main: f32 = 0.0;
        let mut min_cross: f32 = 0.0;
        let mut max_cross: f32 = 0.0;
        let mut count = 0usize;
        for child in &node.children {
            if out_of_flow(child) {
                continue;
            }
            count += 1;
            let intrinsic = self.intrinsic_size(child, viewport, available_width);
            let content_intrinsic = self.content_intrinsic_size(child, viewport, available_width);
            let child_context = viewport.context(
                child.style.text.font_size_px,
                available_width,
                viewport.height,
            );
            let margin = child.style.margin.resolve(&child_context, Axis::Horizontal);
            let (main_axis, cross_axis) = if is_row {
                (Axis::Horizontal, Axis::Vertical)
            } else {
                (Axis::Vertical, Axis::Horizontal)
            };
            let padding = child.style.padding.resolve(&child_context, main_axis);
            let border = child.style.border.widths.resolve(&child_context, main_axis);
            let extra = if is_row {
                padding.left + padding.right + border.left + border.right
            } else {
                padding.top + padding.bottom + border.top + border.bottom
            };
            let explicit_main = definite_content_size(
                if is_row {
                    &child.style.width
                } else {
                    &child.style.height
                },
                &child_context,
                main_axis,
                extra,
                child.style.box_sizing,
            );
            let basis = match &child.style.flex.basis {
                Size::Length(_) => definite_content_size(
                    &child.style.flex.basis,
                    &child_context,
                    main_axis,
                    extra,
                    child.style.box_sizing,
                ),
                Size::Stretch => Some(if is_row {
                    available_width
                } else {
                    viewport.height
                }),
                Size::MinContent => Some(content_intrinsic.axis(main_axis, true)),
                Size::MaxContent => Some(content_intrinsic.axis(main_axis, false)),
                Size::FitContent(limit) => Some(
                    content_intrinsic
                        .axis(main_axis, false)
                        .min(limit.resolve(&child_context, main_axis).max(0.0))
                        .max(content_intrinsic.axis(main_axis, true)),
                ),
                Size::Auto | Size::None => explicit_main,
            };
            let main_min = content_intrinsic.axis(main_axis, true) + extra;
            let main_max = basis
                .map(|value| value + extra)
                .unwrap_or(intrinsic.axis(main_axis, false));
            let cross_min = intrinsic.axis(cross_axis, true);
            let cross_max = intrinsic.axis(cross_axis, false);
            let (main_min, main_max, cross_min, cross_max) = if is_row {
                (
                    main_min + margin.left + margin.right,
                    main_max + margin.left + margin.right,
                    cross_min + margin.top + margin.bottom,
                    cross_max + margin.top + margin.bottom,
                )
            } else {
                (
                    main_min + margin.top + margin.bottom,
                    main_max + margin.top + margin.bottom,
                    cross_min + margin.left + margin.right,
                    cross_max + margin.left + margin.right,
                )
            };
            min_main = if wrap {
                min_main.max(main_min)
            } else {
                min_main + main_min
            };
            max_main += main_max;
            min_cross = min_cross.max(cross_min);
            max_cross = max_cross.max(cross_max);
        }
        let gap_total = gap * count.saturating_sub(1) as f32;
        max_main += gap_total;
        if !wrap {
            min_main += gap_total;
        }
        if is_row {
            IntrinsicSize {
                min_width: min_main,
                max_width: max_main,
                min_height: min_cross,
                max_height: max_cross,
            }
        } else {
            IntrinsicSize {
                min_width: min_cross,
                max_width: max_cross,
                min_height: min_main,
                max_height: max_main,
            }
        }
    }

    fn grid_intrinsic(
        &mut self,
        node: &Node,
        viewport: Viewport,
        available_width: f32,
    ) -> IntrinsicSize {
        let style = &node.style;
        let explicit_columns = flatten_tracks(&style.grid.columns);
        let explicit_rows = flatten_tracks(&style.grid.rows);
        let area_columns = style
            .grid
            .areas
            .values()
            .map(|area| area.column + area.column_span)
            .max()
            .unwrap_or(0);
        let area_rows = style
            .grid
            .areas
            .values()
            .map(|area| area.row + area.row_span)
            .max()
            .unwrap_or(0);
        let columns = explicit_columns.len().max(area_columns).max(1);
        let rows = explicit_rows.len().max(area_rows).max(1);
        let mut column_min = vec![0.0f32; columns];
        let mut column_max = vec![0.0f32; columns];
        let mut row_min = vec![0.0f32; rows];
        let mut row_max = vec![0.0f32; rows];
        let mut cursor_column = 0usize;
        let mut cursor_row = 0usize;
        for child in &node.children {
            if out_of_flow(child) {
                continue;
            }
            let intrinsic = self.intrinsic_size(child, viewport, available_width);
            let context = viewport.context(
                child.style.text.font_size_px,
                available_width,
                viewport.height,
            );
            let margin = child.style.margin.resolve(&context, Axis::Horizontal);
            let outer_min = intrinsic.min_width + margin.left + margin.right;
            let outer_max = intrinsic.max_width + margin.left + margin.right;
            let (column, row) = if let Some((column, row, _, _)) =
                intrinsic_grid_cell(child, style, columns, rows)
            {
                (column, row)
            } else {
                let column = cursor_column;
                let row = cursor_row;
                cursor_column += 1;
                if cursor_column >= columns {
                    cursor_column = 0;
                    cursor_row += 1;
                }
                (column.min(columns - 1), row.min(rows - 1))
            };
            column_min[column] = column_min[column].max(outer_min);
            column_max[column] = column_max[column].max(outer_max);
            row_min[row] = row_min[row].max(intrinsic.min_height);
            row_max[row] = row_max[row].max(intrinsic.max_height);
        }
        let mut width_min = 0.0;
        let mut width_max = 0.0;
        for index in 0..columns {
            let def = track_or_auto(&explicit_columns, &style.grid.auto_columns, index);
            let (min, max) = intrinsic_track_extent(&def, column_min[index], column_max[index]);
            width_min += min;
            width_max += max;
        }
        let mut height_min = 0.0;
        let mut height_max = 0.0;
        for index in 0..rows {
            let def = track_or_auto(&explicit_rows, &style.grid.auto_rows, index);
            let (min, max) = intrinsic_track_extent(&def, row_min[index], row_max[index]);
            height_min += min;
            height_max += max;
        }
        IntrinsicSize {
            min_width: width_min,
            max_width: width_max,
            min_height: height_min,
            max_height: height_max,
        }
    }
}

// ---------------------------------------------------------------------
// Size helpers
// ---------------------------------------------------------------------

fn definite_content_size(
    size: &Size,
    context: &ResolveContext,
    axis: Axis,
    extra: f32,
    box_sizing: crate::css::BoxSizing,
) -> Option<f32> {
    let Size::Length(length) = size else {
        return None;
    };
    if length.is_auto() {
        return None;
    }
    let value = length.resolve(context, axis).max(0.0);
    Some(match box_sizing {
        crate::css::BoxSizing::BorderBox => (value - extra).max(0.0),
        crate::css::BoxSizing::ContentBox => value,
    })
}

fn keyword_content_size(
    size: &Size,
    context: &ResolveContext,
    axis: Axis,
    available: f32,
    intrinsic: IntrinsicSize,
) -> Option<f32> {
    match size {
        Size::MinContent => Some(intrinsic.axis(axis, true)),
        Size::MaxContent => Some(intrinsic.axis(axis, false)),
        Size::FitContent(limit) => {
            let limit = limit.resolve(context, axis).max(0.0);
            Some(
                intrinsic
                    .axis(axis, false)
                    .min(limit)
                    .max(intrinsic.axis(axis, true)),
            )
        }
        Size::Stretch => Some(available.max(0.0)),
        _ => None,
    }
}

fn clamp_content_size(
    value: f32,
    min: &Size,
    max: &Size,
    context: &ResolveContext,
    axis: Axis,
    _available: f32,
    intrinsic: IntrinsicSize,
) -> f32 {
    let minimum = min_content_size(min, context, axis, Some(intrinsic), false);
    let maximum = max_content_size(max, context, axis, Some(intrinsic));
    value.max(minimum).min(maximum.max(minimum)).max(0.0)
}

fn clamp_definite_size(value: f32, min: &Size, max: &Size, context: &ResolveContext) -> f32 {
    let minimum = match min {
        Size::Length(length) => length.resolve(context, Axis::Horizontal).max(0.0),
        _ => 0.0,
    };
    let maximum = match max {
        Size::Length(length) => length.resolve(context, Axis::Horizontal).max(0.0),
        _ => f32::INFINITY,
    };
    value.clamp(minimum, maximum.max(minimum))
}

fn min_content_size(
    size: &Size,
    context: &ResolveContext,
    axis: Axis,
    intrinsic: Option<IntrinsicSize>,
    automatic_min: bool,
) -> f32 {
    match size {
        Size::Length(length) => length.resolve(context, axis).max(0.0),
        Size::MinContent => intrinsic.map(|size| size.axis(axis, true)).unwrap_or(0.0),
        Size::MaxContent => intrinsic.map(|size| size.axis(axis, false)).unwrap_or(0.0),
        Size::FitContent(limit) => {
            let limit = limit.resolve(context, axis).max(0.0);
            intrinsic
                .map(|size| size.axis(axis, false).min(limit).max(size.axis(axis, true)))
                .unwrap_or(limit)
        }
        Size::Stretch => intrinsic.map(|size| size.axis(axis, false)).unwrap_or(0.0),
        Size::Auto | Size::None => {
            if automatic_min {
                intrinsic.map(|size| size.axis(axis, true)).unwrap_or(0.0)
            } else {
                0.0
            }
        }
    }
}

fn max_content_size(
    size: &Size,
    context: &ResolveContext,
    axis: Axis,
    intrinsic: Option<IntrinsicSize>,
) -> f32 {
    match size {
        Size::Length(length) => length.resolve(context, axis).max(0.0),
        Size::MinContent => intrinsic.map(|size| size.axis(axis, true)).unwrap_or(0.0),
        Size::MaxContent => intrinsic.map(|size| size.axis(axis, false)).unwrap_or(0.0),
        Size::FitContent(limit) => {
            let limit = limit.resolve(context, axis).max(0.0);
            intrinsic
                .map(|size| size.axis(axis, false).min(limit).max(size.axis(axis, true)))
                .unwrap_or(limit)
        }
        Size::Auto | Size::None | Size::Stretch => f32::INFINITY,
    }
}

fn block_horizontal_offset(
    style_margin: &Edges<crate::css::values::Length>,
    resolved: &Edges<f32>,
    available: f32,
    width: f32,
) -> f32 {
    let auto_left = style_margin.left.is_auto();
    let auto_right = style_margin.right.is_auto();
    match (auto_left, auto_right) {
        (true, true) => ((available - width) / 2.0).max(0.0),
        (true, false) => (available - resolved.right - width).max(0.0),
        _ => resolved.left,
    }
}

fn collapse_margins(first: f32, second: f32) -> f32 {
    if first >= 0.0 && second >= 0.0 {
        first.max(second)
    } else if first <= 0.0 && second <= 0.0 {
        first.min(second)
    } else {
        first + second
    }
}

fn effective_flex_align(style: &ComputedStyle, child: &Node) -> AlignItems {
    child
        .style
        .flex
        .align_self
        .unwrap_or(style.flex.align_items)
}

fn effective_grid_justify(style: &ComputedStyle, child: &Node) -> AlignItems {
    child
        .style
        .grid
        .justify_self
        .unwrap_or(if style.grid.justify_items_set {
            style.grid.justify_items
        } else {
            AlignItems::Stretch
        })
}

fn effective_grid_align(style: &ComputedStyle, child: &Node) -> AlignItems {
    child
        .style
        .flex
        .align_self
        .unwrap_or(if style.flex.align_items_set {
            style.flex.align_items
        } else {
            AlignItems::Stretch
        })
}

fn distribution(content: AlignContent, free: f32, count: usize) -> (f32, f32) {
    match content {
        AlignContent::Start | AlignContent::Stretch => (0.0, 0.0),
        AlignContent::End => (free, 0.0),
        AlignContent::Center => (free / 2.0, 0.0),
        AlignContent::SpaceBetween => {
            if count > 1 {
                (0.0, free / (count - 1) as f32)
            } else {
                (0.0, 0.0)
            }
        }
        AlignContent::SpaceAround => {
            if count == 0 {
                (0.0, 0.0)
            } else {
                let each = free / count as f32;
                (each / 2.0, each)
            }
        }
        AlignContent::SpaceEvenly => {
            if count == 0 {
                (0.0, 0.0)
            } else {
                let each = free / (count + 1) as f32;
                (each, each)
            }
        }
    }
}

fn grid_content_distribution(content: JustifyContent, free: f32, count: usize) -> (f32, f32) {
    match content {
        JustifyContent::Start => (0.0, 0.0),
        JustifyContent::End => (free, 0.0),
        JustifyContent::Center => (free / 2.0, 0.0),
        JustifyContent::SpaceBetween => {
            if count > 1 {
                (0.0, free / (count - 1) as f32)
            } else {
                (0.0, 0.0)
            }
        }
        JustifyContent::SpaceAround => {
            if count == 0 {
                (0.0, 0.0)
            } else {
                let each = free / count as f32;
                (each / 2.0, each)
            }
        }
        JustifyContent::SpaceEvenly => {
            if count == 0 {
                (0.0, 0.0)
            } else {
                let each = free / (count + 1) as f32;
                (each, each)
            }
        }
    }
}

fn grid_align_distribution(content: AlignContent, free: f32, count: usize) -> (f32, f32) {
    match content {
        AlignContent::Start | AlignContent::Stretch => (0.0, 0.0),
        AlignContent::End => (free, 0.0),
        AlignContent::Center => (free / 2.0, 0.0),
        AlignContent::SpaceBetween => {
            if count > 1 {
                (0.0, free / (count - 1) as f32)
            } else {
                (0.0, 0.0)
            }
        }
        AlignContent::SpaceAround => {
            if count == 0 {
                (0.0, 0.0)
            } else {
                let each = free / count as f32;
                (each / 2.0, each)
            }
        }
        AlignContent::SpaceEvenly => {
            if count == 0 {
                (0.0, 0.0)
            } else {
                let each = free / (count + 1) as f32;
                (each, each)
            }
        }
    }
}

fn out_of_flow(node: &Node) -> bool {
    matches!(node.style.position, Position::Absolute | Position::Fixed)
}

fn fit_size(
    intrinsic_width: f32,
    intrinsic_height: f32,
    box_width: f32,
    box_height: f32,
    fit: ObjectFit,
) -> (f32, f32) {
    match fit {
        ObjectFit::Fill => (box_width, box_height),
        ObjectFit::None => (intrinsic_width, intrinsic_height),
        ObjectFit::Contain | ObjectFit::ScaleDown => {
            let scale = (box_width / intrinsic_width)
                .min(box_height / intrinsic_height)
                .min(if fit == ObjectFit::ScaleDown {
                    1.0
                } else {
                    f32::MAX
                });
            (intrinsic_width * scale, intrinsic_height * scale)
        }
        ObjectFit::Cover => {
            let scale = (box_width / intrinsic_width).max(box_height / intrinsic_height);
            (intrinsic_width * scale, intrinsic_height * scale)
        }
    }
}

// ---------------------------------------------------------------------
// Grid helpers
// ---------------------------------------------------------------------

fn flatten_tracks(tracks: &[TrackSize]) -> Vec<TrackSize> {
    let mut flat = Vec::new();
    fn push(track: &TrackSize, out: &mut Vec<TrackSize>) {
        match track {
            TrackSize::Repeat(count, tracks) => {
                let count = (*count).min(1000) as usize;
                for _ in 0..count {
                    for track in tracks {
                        push(track, out);
                        if out.len() >= 1000 {
                            return;
                        }
                    }
                }
            }
            other => out.push(other.clone()),
        }
    }
    for track in tracks {
        push(track, &mut flat);
        if flat.len() >= 1000 {
            break;
        }
    }
    flat
}

fn track_or_auto(explicit: &[TrackSize], auto: &[TrackSize], index: usize) -> TrackSize {
    if let Some(track) = explicit.get(index) {
        return track.clone();
    }
    if auto.is_empty() {
        return TrackSize::Auto;
    }
    let offset = index.saturating_sub(explicit.len());
    auto[offset % auto.len()].clone()
}

fn resolve_grid_axis(
    placement: &GridPlacement,
    areas: &BTreeMap<String, GridArea>,
    row_axis: bool,
    explicit_tracks: usize,
) -> (Option<usize>, usize, bool) {
    let area_line = |line: &GridLine, end: bool| -> Option<usize> {
        let GridLine::Name(name) = line else {
            return None;
        };
        let area = areas.get(name)?;
        Some(if row_axis {
            if end {
                area.row + area.row_span
            } else {
                area.row
            }
        } else if end {
            area.column + area.column_span
        } else {
            area.column
        })
    };
    let line_position = |line: &GridLine, is_end: bool| -> Option<usize> {
        match line {
            GridLine::Line(number) => {
                if *number > 0 {
                    Some(*number as usize - 1)
                } else if *number < 0 {
                    Some((explicit_tracks as i32 + *number).max(0) as usize)
                } else {
                    None
                }
            }
            GridLine::Name(_) => area_line(line, is_end),
            GridLine::Auto | GridLine::Span(_) => None,
        }
    };

    let start = line_position(&placement.start, false);
    let end = line_position(&placement.end, true);
    let start_area_span = match &placement.start {
        GridLine::Name(name) => areas.get(name).map(|area| {
            if row_axis {
                area.row_span.max(1)
            } else {
                area.column_span.max(1)
            }
        }),
        _ => None,
    };
    let (start, span) = match (&placement.start, &placement.end, end) {
        (GridLine::Span(span), _, Some(end)) => {
            let span = (*span).max(1) as usize;
            (Some(end.saturating_sub(span)), span)
        }
        (GridLine::Span(span), _, _) => (None, (*span).max(1) as usize),
        (GridLine::Name(_), GridLine::Auto, _) => (start, start_area_span.unwrap_or(1)),
        (_, GridLine::Span(span), _) => (start, (*span).max(1) as usize),
        (_, GridLine::Name(_), Some(end)) => {
            let start = start.unwrap_or(end.saturating_sub(1));
            (Some(start), end.saturating_sub(start).max(1))
        }
        (_, GridLine::Line(_), Some(end)) => {
            let start = start.unwrap_or(end.saturating_sub(1));
            (Some(start), end.saturating_sub(start).max(1))
        }
        _ => (start, 1),
    };
    let definite = start.is_some();
    (start, span, definite)
}

fn intrinsic_grid_cell(
    child: &Node,
    style: &ComputedStyle,
    columns: usize,
    rows: usize,
) -> Option<(usize, usize, usize, usize)> {
    let (column, column_span, _) =
        resolve_grid_axis(&child.style.grid.column, &style.grid.areas, false, columns);
    let (row, row_span, _) =
        resolve_grid_axis(&child.style.grid.row, &style.grid.areas, true, rows);
    let column = column?;
    let row = row?;
    Some((
        column.min(columns.saturating_sub(1)),
        row.min(rows.saturating_sub(1)),
        column_span.max(1),
        row_span.max(1),
    ))
}

fn intrinsic_track_extent(def: &TrackSize, min_content: f32, max_content: f32) -> (f32, f32) {
    match def {
        TrackSize::Px(value) => (*value, *value),
        TrackSize::Percent(_) => (min_content, max_content),
        TrackSize::Fr(_) | TrackSize::Auto | TrackSize::MaxContent => (max_content, max_content),
        TrackSize::MinContent => (min_content, min_content),
        TrackSize::FitContent(len) => {
            let limit = match len {
                crate::css::values::Length::Px(value) => *value,
                _ => max_content,
            };
            (min_content.min(limit), max_content.min(limit))
        }
        TrackSize::MinMax(min, max) => {
            let (min_min, _) = intrinsic_track_extent(min, min_content, max_content);
            let (_, max_max) = intrinsic_track_extent(max, min_content, max_content);
            (min_min, max_max.max(min_min))
        }
        TrackSize::Repeat(_, _) => (min_content, max_content),
    }
}

fn ensure_grid_capacity(occupied: &mut Vec<Vec<bool>>, rows: usize, columns: usize) {
    if occupied.len() < rows {
        occupied.resize(rows, Vec::new());
    }
    for row in occupied.iter_mut() {
        if row.len() < columns {
            row.resize(columns, false);
        }
    }
}

fn grow_grid(occupied: &mut Vec<Vec<bool>>, rows: usize, columns: usize) {
    ensure_grid_capacity(occupied, rows.max(1), columns.max(1));
}

fn mark_grid_cell(
    occupied: &mut [Vec<bool>],
    row: usize,
    column: usize,
    row_span: usize,
    column_span: usize,
) {
    for y in row..(row + row_span).min(occupied.len()) {
        for x in column..(column + column_span).min(occupied[y].len()) {
            occupied[y][x] = true;
        }
    }
}

fn grid_area_free(
    occupied: &[Vec<bool>],
    row: usize,
    column: usize,
    row_span: usize,
    column_span: usize,
) -> bool {
    if row + row_span > occupied.len() {
        return false;
    }
    for cells in occupied.iter().skip(row).take(row_span) {
        if column + column_span > cells.len() {
            return false;
        }
        if cells[column..column + column_span].iter().any(|cell| *cell) {
            return false;
        }
    }
    true
}

fn find_grid_column(
    occupied: &[Vec<bool>],
    row: usize,
    start: usize,
    columns: usize,
    column_span: usize,
    row_span: usize,
) -> Option<usize> {
    (start..columns).find(|column| {
        column + column_span <= columns
            && grid_area_free(occupied, row, *column, row_span, column_span)
    })
}

fn find_grid_row(
    occupied: &[Vec<bool>],
    column: usize,
    start: usize,
    rows: usize,
    column_span: usize,
    row_span: usize,
) -> Option<usize> {
    (start..rows).find(|row| {
        row + row_span <= rows && grid_area_free(occupied, *row, column, row_span, column_span)
    })
}

fn place_auto_row_flow(
    occupied: &mut Vec<Vec<bool>>,
    rows: &mut usize,
    columns: usize,
    start_row: usize,
    start_column: usize,
    column_span: usize,
    row_span: usize,
) -> (usize, usize) {
    let mut row = start_row;
    let mut column = start_column.min(columns.saturating_sub(1));
    loop {
        if row + row_span > occupied.len() {
            *rows = (*rows).max(row + row_span);
            ensure_grid_capacity(occupied, *rows, columns);
        }
        if column + column_span <= columns
            && grid_area_free(occupied, row, column, row_span, column_span)
        {
            return (column, row);
        }
        column += 1;
        if column + column_span > columns {
            column = 0;
            row += 1;
        }
    }
}

fn place_auto_column_flow(
    occupied: &mut Vec<Vec<bool>>,
    columns: &mut usize,
    rows: usize,
    start_row: usize,
    start_column: usize,
    column_span: usize,
    row_span: usize,
) -> (usize, usize) {
    let mut row = start_row.min(rows.saturating_sub(1));
    let mut column = start_column;
    loop {
        if column + column_span > occupied.first().map(Vec::len).unwrap_or(0) {
            *columns = (*columns).max(column + column_span);
            ensure_grid_capacity(occupied, rows, *columns);
        }
        if row + row_span <= rows && grid_area_free(occupied, row, column, row_span, column_span) {
            return (column, row);
        }
        row += 1;
        if row + row_span > rows {
            row = 0;
            column += 1;
        }
    }
}

fn size_tracks(
    defs: &[TrackSize],
    contributions: &[(usize, usize, f32, f32)],
    gap: f32,
    basis: Option<f32>,
    context: &ResolveContext,
    axis: Axis,
) -> Vec<f32> {
    let count = defs.len().max(1);
    let mut sizes = vec![0.0f32; count];
    let mut floors = vec![0.0f32; count];
    let mut is_fr = vec![false; count];
    let mut fr = vec![0.0f32; count];
    let mut min_content = vec![0.0f32; count];
    let mut max_content = vec![0.0f32; count];
    let mut content_sized = vec![false; count];

    for (index, def) in defs.iter().enumerate().take(count) {
        match def {
            TrackSize::Px(value) => sizes[index] = *value,
            TrackSize::Percent(percent) => {
                sizes[index] = basis.unwrap_or(0.0) * percent / 100.0;
            }
            TrackSize::Fr(value) => {
                is_fr[index] = true;
                fr[index] = value.max(0.0);
                content_sized[index] = true;
            }
            TrackSize::MinContent => {
                content_sized[index] = true;
                floors[index] = 0.0;
            }
            TrackSize::MaxContent | TrackSize::Auto => {
                content_sized[index] = true;
            }
            TrackSize::FitContent(limit) => {
                let limit = limit.resolve(context, axis).max(0.0);
                sizes[index] = limit;
                content_sized[index] = true;
            }
            TrackSize::MinMax(min, max) => {
                let min_size = match &**min {
                    TrackSize::Px(value) => *value,
                    TrackSize::Percent(percent) => basis.unwrap_or(0.0) * percent / 100.0,
                    _ => 0.0,
                };
                floors[index] = min_size;
                match &**max {
                    TrackSize::Fr(value) => {
                        is_fr[index] = true;
                        fr[index] = value.max(0.0);
                        content_sized[index] = true;
                        sizes[index] = min_size;
                    }
                    TrackSize::Px(value) => sizes[index] = (*value).max(min_size),
                    TrackSize::Percent(percent) => {
                        sizes[index] = (basis.unwrap_or(0.0) * percent / 100.0).max(min_size)
                    }
                    _ => {
                        sizes[index] = min_size;
                        content_sized[index] = true;
                    }
                }
            }
            TrackSize::Repeat(_, _) => {
                content_sized[index] = true;
            }
        }
    }

    for (start, span, min, max) in contributions {
        if *span <= 1 {
            let index = (*start).min(count - 1);
            min_content[index] = min_content[index].max(*min);
            max_content[index] = max_content[index].max(*max);
        }
    }

    for index in 0..count {
        if content_sized[index] && !is_fr[index] {
            let def = &defs[index.min(defs.len().saturating_sub(1))];
            sizes[index] = match def {
                TrackSize::MinContent => min_content[index].max(floors[index]),
                TrackSize::FitContent(limit) => {
                    let limit = limit.resolve(context, axis).max(0.0);
                    max_content[index]
                        .min(limit)
                        .max(min_content[index])
                        .max(floors[index])
                }
                _ => max_content[index].max(floors[index]),
            };
        }
    }

    // Spanning items: distribute their contribution across the content-sized
    // tracks they cover.
    for (start, span, _, max) in contributions {
        if *span <= 1 {
            continue;
        }
        let start = (*start).min(count - 1);
        let end = (start + *span).min(count);
        let current: f32 =
            sizes[start..end].iter().sum::<f32>() + gap * (end - start).saturating_sub(1) as f32;
        if *max <= current {
            continue;
        }
        let flexible: Vec<usize> = (start..end).filter(|index| content_sized[*index]).collect();
        if flexible.is_empty() {
            sizes[end - 1] += *max - current;
        } else {
            let each = (*max - current) / flexible.len() as f32;
            for index in flexible {
                sizes[index] += each;
            }
        }
    }

    if is_fr.iter().any(|value| *value) {
        if let Some(basis) = basis {
            let fixed_total = sizes
                .iter()
                .enumerate()
                .filter(|(index, _)| !is_fr[*index])
                .map(|(_, size)| *size)
                .sum::<f32>()
                + gap * count.saturating_sub(1) as f32;
            let remaining = (basis - fixed_total).max(0.0);
            let total_fr: f32 = fr.iter().sum();
            if total_fr > 0.0 {
                for index in 0..count {
                    if is_fr[index] {
                        sizes[index] = (remaining * fr[index] / total_fr).max(floors[index]);
                    }
                }
            }
        } else {
            for index in 0..count {
                if is_fr[index] {
                    sizes[index] = max_content[index]
                        .max(min_content[index])
                        .max(floors[index]);
                }
            }
        }
    }

    sizes
}
