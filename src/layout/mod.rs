// SPDX-FileCopyrightText: 2026 Daniel
// SPDX-License-Identifier: GPL-3.0-or-later

//! Box model, flex and grid layout.
//!
//! Layout is a pure function over the scene: given a computed style and a text
//! engine, it produces pixel rectangles. It runs on load/reload/resize, not on
//! every frame.

use crate::css::Viewport;
use crate::css::computed::ComputedStyle;
use crate::css::properties::media_object_fit::ObjectFit;
use crate::css::properties::{
    AlignContent, AlignItems, Display, FlexDirection, FlexWrap, GridAutoFlow, JustifyContent,
    Position, TrackSize,
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

        let explicit_width = forced_width.map(|width| width.max(0.0)).or_else(|| {
            style
                .width
                .resolve(&context, Axis::Horizontal)
                .map(|width| match style.box_sizing {
                    crate::css::BoxSizing::BorderBox => (width - horizontal_extra).max(0.0),
                    crate::css::BoxSizing::ContentBox => width,
                })
        });
        let explicit_height = forced_height.map(|height| height.max(0.0)).or_else(|| {
            style
                .height
                .resolve(&context, Axis::Vertical)
                .map(|height| match style.box_sizing {
                    crate::css::BoxSizing::BorderBox => (height - vertical_extra).max(0.0),
                    crate::css::BoxSizing::ContentBox => height,
                })
        });

        let content_available_width = explicit_width.unwrap_or_else(|| {
            (available_width - margin.left - margin.right - horizontal_extra).max(0.0)
        });
        let content_available_height = explicit_height.unwrap_or_else(|| {
            (available_height - margin.top - margin.bottom - vertical_extra).max(0.0)
        });

        let (mut content_width, mut content_height) = match &mut node.kind {
            NodeKind::Text(content) => {
                let max_width =
                    explicit_width.or_else(|| style.max_width.resolve(&context, Axis::Horizontal));
                let request = ShapeRequest {
                    text: &content.text,
                    font_size,
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
                        .resolve(&context, Axis::Horizontal)
                        .max(0.0),
                    white_space: style.text.white_space,
                    overflow_wrap: style.text.overflow_wrap,
                    word_break: style.text.word_break,
                    transform: style.text.transform,
                    text_overflow: style.text.overflow,
                    line_clamp: style.text.line_clamp,
                };
                let (text_width, text_height) = self.text.measure(request);
                let width = explicit_width.unwrap_or_else(|| {
                    max_width.unwrap_or_else(|| {
                        text_width
                            + style
                                .text
                                .indent
                                .resolve(&context, Axis::Horizontal)
                                .max(0.0)
                    })
                });
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
                        content_available_width,
                        content_available_height,
                        explicit_width,
                        explicit_height,
                    ),
                    _ => self.layout_flex_children(
                        node,
                        viewport,
                        content_available_width,
                        content_available_height,
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

        let total_width = content_width + horizontal_extra;
        let total_height = content_height + vertical_extra;
        node.layout.width = total_width;
        node.layout.height = total_height;
        node.layout.padding = padding.clone();
        node.layout.border = border.clone();
        node.layout.margin = margin;
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

        // Flex items keep their measured size. Grow/shrink are not applied to
        // text yet, so explicit sizes and intrinsic widths determine a line.
        for index in &flow_indices {
            self.layout_node(
                &mut node.children[*index],
                viewport,
                available_width.max(0.0),
                available_height.max(0.0),
            );
        }

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

        let main_size = |child: &Node| {
            if is_row {
                child.layout.margin.left + child.layout.width + child.layout.margin.right
            } else {
                child.layout.margin.top + child.layout.height + child.layout.margin.bottom
            }
        };
        let cross_size = |child: &Node| {
            if is_row {
                child.layout.margin.top + child.layout.height + child.layout.margin.bottom
            } else {
                child.layout.margin.left + child.layout.width + child.layout.margin.right
            }
        };

        let container_main = explicit_content_width
            .or_else(|| {
                let available = if is_row {
                    available_width - horizontal_extra
                } else {
                    available_height - vertical_extra
                };
                Some(available.max(0.0))
            })
            .unwrap_or(0.0);

        // Group into flex lines. Without wrapping this is one line.
        let wraps = !matches!(style.flex.wrap, FlexWrap::NoWrap);
        let mut lines: Vec<Vec<usize>> = Vec::new();
        let mut line: Vec<usize> = Vec::new();
        let mut line_main = 0.0;
        for index in flow_indices {
            let size = main_size(&node.children[index]);
            let advance = size + if line.is_empty() { 0.0 } else { main_gap };
            if wraps && !line.is_empty() && line_main + advance > container_main {
                lines.push(std::mem::take(&mut line));
                line_main = 0.0;
            }
            line_main += size + if line.is_empty() { 0.0 } else { main_gap };
            line.push(index);
        }
        if !line.is_empty() {
            lines.push(line);
        }
        if matches!(style.flex.wrap, FlexWrap::WrapReverse) {
            lines.reverse();
        }

        let (explicit_main, explicit_cross) = if is_row {
            (explicit_content_width, explicit_content_height)
        } else {
            (explicit_content_height, explicit_content_width)
        };

        // Distribute free main-axis space with flex-grow, or shrink items when
        // an explicit size makes the line overflow.
        if let Some(main_limit) = explicit_main {
            for line in &lines {
                let base_total = line
                    .iter()
                    .map(|index| main_size(&node.children[*index]))
                    .sum::<f32>();
                let free = main_limit - base_total;
                if free.abs() <= 0.01 {
                    continue;
                }
                let total_grow = line
                    .iter()
                    .map(|index| node.children[*index].style.flex.grow.max(0.0))
                    .sum::<f32>();
                let total_shrink = line
                    .iter()
                    .map(|index| {
                        let child = &node.children[*index];
                        let base = if is_row {
                            child.layout.content_width
                        } else {
                            child.layout.content_height
                        };
                        child.style.flex.shrink.max(0.0) * base
                    })
                    .sum::<f32>();
                for index in line {
                    let child = &mut node.children[*index];
                    let current = if is_row {
                        child.layout.content_width
                    } else {
                        child.layout.content_height
                    };
                    let delta = if free > 0.0 && total_grow > 0.0 {
                        free * child.style.flex.grow.max(0.0) / total_grow
                    } else if free < 0.0 && total_shrink > 0.0 {
                        let base = current.max(0.0);
                        free * (child.style.flex.shrink.max(0.0) * base) / total_shrink
                    } else {
                        0.0
                    };
                    let next = (current + delta).max(0.0);
                    if is_row {
                        self.layout_node_with(
                            child,
                            viewport,
                            available_width,
                            available_height,
                            Some(next),
                            None,
                        );
                    } else {
                        self.layout_node_with(
                            child,
                            viewport,
                            available_width,
                            available_height,
                            None,
                            Some(next),
                        );
                    }
                }
            }
        }

        let line_cross: Vec<f32> = lines
            .iter()
            .map(|line| {
                line.iter()
                    .map(|index| cross_size(&node.children[*index]))
                    .fold(0.0, f32::max)
            })
            .collect();
        let lines_count = lines.len();
        let total_cross =
            line_cross.iter().sum::<f32>() + cross_gap * (lines_count.saturating_sub(1)) as f32;
        // Without an explicit size the flex container shrink-wraps to its
        // longest line. The available size is only the wrapping limit; using it
        // as the final size would make anchors position a full-viewport box.
        let max_line_main = lines
            .iter()
            .map(|line| {
                line.iter()
                    .map(|index| main_size(&node.children[*index]))
                    .sum::<f32>()
                    + main_gap * (line.len().saturating_sub(1)) as f32
            })
            .fold(0.0, f32::max);
        let content_main = explicit_main.unwrap_or(max_line_main);
        let content_cross = explicit_cross.unwrap_or(total_cross);
        let free_cross = (content_cross - total_cross).max(0.0);
        let (mut cross_cursor, line_extra) = match style.flex.align_content {
            AlignContent::Start | AlignContent::Stretch => (0.0, 0.0),
            AlignContent::End => (free_cross, 0.0),
            AlignContent::Center => (free_cross / 2.0, 0.0),
            AlignContent::SpaceBetween => {
                if lines_count > 1 {
                    (0.0, free_cross / (lines_count - 1) as f32)
                } else {
                    (0.0, 0.0)
                }
            }
            AlignContent::SpaceAround => {
                if lines_count == 0 {
                    (0.0, 0.0)
                } else {
                    let each = free_cross / lines_count as f32;
                    (each / 2.0, each)
                }
            }
            AlignContent::SpaceEvenly => {
                if lines_count == 0 {
                    (0.0, 0.0)
                } else {
                    let each = free_cross / (lines_count + 1) as f32;
                    (each, each)
                }
            }
        };

        let content_offset_x = border.left + padding.left;
        let content_offset_y = border.top + padding.top;

        for (line_index, line) in lines.iter().enumerate() {
            let line_main_total = line
                .iter()
                .map(|index| main_size(&node.children[*index]))
                .sum::<f32>()
                + main_gap * (line.len().saturating_sub(1)) as f32;
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
            for index in line {
                let child = &mut node.children[*index];
                let align = effective_align(&style, child);
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
                cursor += main_size(child) + main_gap + extra_gap;
            }
            cross_cursor += line_height + cross_gap + line_extra;
        }

        if is_row {
            (content_main, content_cross)
        } else {
            (content_cross, content_main)
        }
    }

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
        let content_available_width = available_width.max(0.0);
        let content_available_height = available_height.max(0.0);

        let flow_indices: Vec<usize> = (0..node.children.len())
            .filter(|index| !out_of_flow(&node.children[*index]))
            .collect();
        for index in &flow_indices {
            self.layout_node(
                &mut node.children[*index],
                viewport,
                content_available_width,
                content_available_height,
            );
        }

        let mut columns = style.grid.columns.len().max(1);
        let mut rows = 1usize;
        for index in &flow_indices {
            let (column, row) = (
                &node.children[*index].style.grid.column,
                &node.children[*index].style.grid.row,
            );
            if column.0 > 0 {
                columns =
                    columns.max((column.0 as usize).saturating_sub(1) + column.1.max(1) as usize);
            }
            if row.0 > 0 {
                rows = rows.max((row.0 as usize).saturating_sub(1) + row.1.max(1) as usize);
            }
        }
        if matches!(
            style.grid.auto_flow,
            GridAutoFlow::Row | GridAutoFlow::RowDense
        ) {
            rows = rows.max(flow_indices.len().div_ceil(columns));
        } else {
            columns = columns.max(flow_indices.len().div_ceil(rows));
        }

        let mut occupied = vec![vec![false; columns]; rows];
        let mut placements: Vec<Option<(usize, usize, usize, usize)>> =
            vec![None; node.children.len()];

        // Explicit placement first.
        for index in &flow_indices {
            let (column, row) = (
                &node.children[*index].style.grid.column,
                &node.children[*index].style.grid.row,
            );
            if column.0 <= 0 && row.0 <= 0 {
                continue;
            }
            let col = (column.0.max(1) as usize - 1).min(columns - 1);
            let row_index = (row.0.max(1) as usize - 1).min(rows - 1);
            let col_span = (column.1.max(1) as usize).min(columns - col);
            let row_span = (row.1.max(1) as usize).min(rows - row_index);
            placements[*index] = Some((col, row_index, col_span, row_span));
            for row in &mut occupied[row_index..row_index + row_span] {
                for cell in &mut row[col..col + col_span] {
                    *cell = true;
                }
            }
        }

        // Auto placement in row-major or column-major order.
        let column_major = matches!(
            style.grid.auto_flow,
            GridAutoFlow::Column | GridAutoFlow::ColumnDense
        );
        let mut candidates: Vec<(usize, usize)> = Vec::new();
        if column_major {
            for x in 0..columns {
                for y in 0..rows {
                    candidates.push((y, x));
                }
            }
        } else {
            for y in 0..rows {
                for x in 0..columns {
                    candidates.push((y, x));
                }
            }
        }
        for index in &flow_indices {
            if placements[*index].is_some() {
                continue;
            }
            let (column, row) = (
                &node.children[*index].style.grid.column,
                &node.children[*index].style.grid.row,
            );
            let col_span = column.1.max(1) as usize;
            let row_span = row.1.max(1) as usize;
            let Some(&(y, x)) = candidates.iter().find(|(y, x)| {
                *y + row_span <= rows
                    && *x + col_span <= columns
                    && (0..row_span).all(|dy| (0..col_span).all(|dx| !occupied[y + dy][x + dx]))
            }) else {
                continue;
            };
            placements[*index] = Some((x, y, col_span, row_span));
            for row in &mut occupied[y..y + row_span] {
                for cell in &mut row[x..x + col_span] {
                    *cell = true;
                }
            }
        }

        let mut column_widths = vec![0.0_f32; columns];
        let mut row_heights = vec![0.0_f32; rows];
        for (index, placement) in placements.iter().enumerate() {
            let Some((col, row, col_span, row_span)) = placement else {
                continue;
            };
            let child = &node.children[index];
            let width = child.layout.margin.left + child.layout.width + child.layout.margin.right;
            let height = child.layout.margin.top + child.layout.height + child.layout.margin.bottom;
            let per_column = width / *col_span as f32;
            let per_row = height / *row_span as f32;
            for width in &mut column_widths[*col..col + col_span] {
                *width = width.max(per_column);
            }
            for height in &mut row_heights[*row..row + row_span] {
                *height = height.max(per_row);
            }
        }

        let track_basis_width = explicit_content_width.unwrap_or(content_available_width);
        let track_basis_height = explicit_content_height.unwrap_or(content_available_height);
        apply_tracks(
            &style.grid.columns,
            &mut column_widths,
            track_basis_width,
            |track, basis| match track {
                TrackSize::Px(value) => Some(*value),
                TrackSize::Percent(value) => Some(basis * value / 100.0),
                _ => None,
            },
        );
        apply_tracks(
            &style.grid.rows,
            &mut row_heights,
            track_basis_height,
            |track, basis| match track {
                TrackSize::Px(value) => Some(*value),
                TrackSize::Percent(value) => Some(basis * value / 100.0),
                _ => None,
            },
        );

        let column_gap = style.flex.column_gap.resolve(&context, Axis::Horizontal);
        let row_gap = style.flex.row_gap.resolve(&context, Axis::Vertical);

        // Grid items stretch to their track by default when the axis is auto,
        // matching browser grid behaviour.
        for (index, placement) in placements.iter().enumerate() {
            let Some((column, row, col_span, row_span)) = placement else {
                continue;
            };
            let child = &mut node.children[index];
            let span_width = column_widths[*column..column + col_span]
                .iter()
                .sum::<f32>()
                + (col_span.saturating_sub(1)) as f32 * column_gap;
            let span_height = row_heights[*row..row + row_span].iter().sum::<f32>()
                + (row_span.saturating_sub(1)) as f32 * row_gap;
            let extra_x = child.layout.width - child.layout.content_width;
            let extra_y = child.layout.height - child.layout.content_height;
            let force_width = matches!(child.style.width, Size::Auto).then(|| {
                (span_width - child.layout.margin.left - child.layout.margin.right - extra_x)
                    .max(0.0)
            });
            let force_height = matches!(child.style.height, Size::Auto).then(|| {
                (span_height - child.layout.margin.top - child.layout.margin.bottom - extra_y)
                    .max(0.0)
            });
            if force_width.is_some() || force_height.is_some() {
                self.layout_node_with(
                    child,
                    viewport,
                    content_available_width,
                    content_available_height,
                    force_width,
                    force_height,
                );
            }
        }

        for (index, placement) in placements.iter().enumerate() {
            let Some((column, row, _, _)) = placement else {
                continue;
            };
            let child = &mut node.children[index];
            let x = border.left
                + padding.left
                + column_widths[..*column].iter().sum::<f32>()
                + *column as f32 * column_gap;
            let y = border.top
                + padding.top
                + row_heights[..*row].iter().sum::<f32>()
                + *row as f32 * row_gap;
            child.layout.x = x + child.layout.margin.left;
            child.layout.y = y + child.layout.margin.top;
        }

        let content_width =
            column_widths.iter().sum::<f32>() + (columns.saturating_sub(1)) as f32 * column_gap;
        let content_height =
            row_heights.iter().sum::<f32>() + (rows.saturating_sub(1)) as f32 * row_gap;
        (content_width, content_height)
    }
}

fn effective_align(style: &ComputedStyle, child: &Node) -> AlignItems {
    // `align-self: start` is also the initial value, so treat it as "auto" and
    // inherit the container value. Explicit non-start values win.
    match child.style.flex.align_self {
        AlignItems::Start => style.flex.align_items,
        other => other,
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

fn apply_tracks(
    tracks: &[TrackSize],
    sizes: &mut [f32],
    basis: f32,
    resolve: impl Fn(&TrackSize, f32) -> Option<f32> + Copy,
) {
    let mut fr_total = 0.0;
    let mut fixed_total = 0.0;
    for (index, track) in tracks.iter().enumerate() {
        if index >= sizes.len() {
            break;
        }
        match track {
            TrackSize::Fr(value) => fr_total += value.max(0.0),
            TrackSize::Auto | TrackSize::MinContent | TrackSize::MaxContent => {
                fixed_total += sizes[index];
            }
            TrackSize::FitContent(length) => {
                let context = ResolveContext {
                    basis_width: basis,
                    basis_height: basis,
                    ..ResolveContext::default()
                };
                sizes[index] = sizes[index].max(length.resolve(&context, Axis::Horizontal));
                fixed_total += sizes[index];
            }
            _ => {
                if let Some(size) = resolve(track, basis) {
                    sizes[index] = size.max(0.0);
                    fixed_total += sizes[index];
                }
            }
        }
    }
    if fr_total > 0.0 {
        let remaining = (basis - fixed_total).max(0.0);
        for (index, track) in tracks.iter().enumerate() {
            if index >= sizes.len() {
                break;
            }
            if let TrackSize::Fr(value) = track {
                sizes[index] = remaining * value.max(0.0) / fr_total;
            }
        }
    }
}
