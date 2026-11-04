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
    AlignItems, Display, FlexDirection, JustifyContent, Position, TrackSize,
};
use crate::css::values::{Axis, Edges, ResolveContext};
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
        let style = node.style.clone();
        let font_size = style.text.font_size_px;
        let context = viewport.context(font_size, available_width, available_height);
        let padding = style.padding.resolve(&context, Axis::Horizontal);
        let border = style.border.widths.resolve(&context, Axis::Horizontal);
        let margin = style.margin.resolve(&context, Axis::Horizontal);
        let horizontal_extra = padding.left + padding.right + border.left + border.right;
        let vertical_extra = padding.top + padding.bottom + border.top + border.bottom;

        let explicit_width =
            style
                .width
                .resolve(&context, Axis::Horizontal)
                .map(|width| match style.box_sizing {
                    crate::css::BoxSizing::BorderBox => (width - horizontal_extra).max(0.0),
                    crate::css::BoxSizing::ContentBox => width,
                });
        let explicit_height = style
            .height
            .resolve(&context, Axis::Vertical)
            .map(|height| match style.box_sizing {
                crate::css::BoxSizing::BorderBox => (height - vertical_extra).max(0.0),
                crate::css::BoxSizing::ContentBox => height,
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
                    letter_spacing: style.text.letter_spacing_px,
                    word_spacing: style.text.word_spacing_px,
                    max_width,
                    align: style.text.align,
                    white_space: style.text.white_space,
                    overflow_wrap: style.text.overflow_wrap,
                    word_break: style.text.word_break,
                    transform: style.text.transform,
                    text_overflow: style.text.overflow,
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

        for child in &mut node.children {
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
        let horizontal_padding = style.padding.resolve(&context, Axis::Horizontal);
        let vertical_padding = style.padding.resolve(&context, Axis::Vertical);
        let horizontal_border = style.border.widths.resolve(&context, Axis::Horizontal);
        let vertical_border = style.border.widths.resolve(&context, Axis::Vertical);

        let child_available_width = available_width.max(0.0);
        let child_available_height = available_height.max(0.0);

        let mut flow_indices: Vec<usize> = (0..node.children.len())
            .filter(|index| !out_of_flow(&node.children[*index]))
            .collect();
        flow_indices.sort_by_key(|index| node.children[*index].style.flex.order);
        if reverse {
            flow_indices.reverse();
        }

        for index in &flow_indices {
            self.layout_node(
                &mut node.children[*index],
                viewport,
                child_available_width,
                child_available_height,
            );
        }

        let gap = if is_row {
            style.flex.column_gap.resolve(&context, Axis::Horizontal)
        } else {
            style.flex.row_gap.resolve(&context, Axis::Vertical)
        }
        .max(0.0);
        let gap_total = gap * (flow_indices.len().saturating_sub(1)) as f32;
        let flow_children: Vec<&Node> = flow_indices
            .iter()
            .map(|index| &node.children[*index])
            .collect();

        let (intrinsic_main, intrinsic_cross) = if is_row {
            let main = flow_children
                .iter()
                .map(|child| {
                    child.layout.margin.left + child.layout.width + child.layout.margin.right
                })
                .sum::<f32>()
                + gap_total;
            let cross = flow_children
                .iter()
                .map(|child| {
                    child.layout.margin.top + child.layout.height + child.layout.margin.bottom
                })
                .fold(0.0, f32::max);
            (main, cross)
        } else {
            let main = flow_children
                .iter()
                .map(|child| {
                    child.layout.margin.top + child.layout.height + child.layout.margin.bottom
                })
                .sum::<f32>()
                + gap_total;
            let cross = flow_children
                .iter()
                .map(|child| {
                    child.layout.margin.left + child.layout.width + child.layout.margin.right
                })
                .fold(0.0, f32::max);
            (main, cross)
        };

        let content_main = if is_row {
            explicit_content_width.unwrap_or(intrinsic_main)
        } else {
            explicit_content_height.unwrap_or(intrinsic_main)
        };
        let content_cross = if is_row {
            explicit_content_height.unwrap_or(intrinsic_cross)
        } else {
            explicit_content_width.unwrap_or(intrinsic_cross)
        };

        let free = (content_main - intrinsic_main).max(0.0);
        let (mut cursor, extra_gap) = match style.flex.justify {
            JustifyContent::Start => (0.0, 0.0),
            JustifyContent::End => (free, 0.0),
            JustifyContent::Center => (free / 2.0, 0.0),
            JustifyContent::SpaceBetween => {
                if flow_indices.len() > 1 {
                    (0.0, free / (flow_indices.len() - 1) as f32)
                } else {
                    (0.0, 0.0)
                }
            }
            JustifyContent::SpaceAround => {
                if flow_indices.is_empty() {
                    (0.0, 0.0)
                } else {
                    let each = free / flow_indices.len() as f32;
                    (each / 2.0, each)
                }
            }
            JustifyContent::SpaceEvenly => {
                if flow_indices.is_empty() {
                    (0.0, 0.0)
                } else {
                    let each = free / (flow_indices.len() + 1) as f32;
                    (each, each)
                }
            }
        };

        let content_offset_x = horizontal_border.left + horizontal_padding.left;
        let content_offset_y = vertical_border.top + vertical_padding.top;

        for index in flow_indices {
            let child = &mut node.children[index];
            let cross_start = child.layout.margin.top_or(is_row);
            let cross_size = if is_row {
                child.layout.height
            } else {
                child.layout.width
            };
            let align = effective_align(&style, child);
            let cross_offset = match align {
                AlignItems::Start | AlignItems::Baseline => 0.0,
                AlignItems::End => (content_cross - cross_size).max(0.0),
                AlignItems::Center => ((content_cross - cross_size) / 2.0).max(0.0),
                AlignItems::Stretch => 0.0,
            };
            if is_row {
                child.layout.x = content_offset_x + cursor + child.layout.margin.left;
                child.layout.y = content_offset_y + cross_start + cross_offset;
            } else {
                child.layout.y = content_offset_y + cursor + child.layout.margin.top;
                child.layout.x = content_offset_x + cross_start + cross_offset;
            }
            let advance = if is_row {
                child.layout.margin.left + child.layout.width + child.layout.margin.right
            } else {
                child.layout.margin.top + child.layout.height + child.layout.margin.bottom
            };
            cursor += advance + gap + extra_gap;
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

        let columns = style.grid.columns.len().max(1);
        let rows = node.children.len().div_ceil(columns);

        for child in &mut node.children {
            self.layout_node(
                child,
                viewport,
                content_available_width,
                content_available_height,
            );
        }

        let mut column_widths = vec![0.0_f32; columns];
        let mut row_heights = vec![0.0_f32; rows];
        for (index, child) in node.children.iter().enumerate() {
            let column = index % columns;
            let row = index / columns;
            column_widths[column] = column_widths[column]
                .max(child.layout.margin.left + child.layout.width + child.layout.margin.right);
            row_heights[row] = row_heights[row]
                .max(child.layout.margin.top + child.layout.height + child.layout.margin.bottom);
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

        for (index, child) in node.children.iter_mut().enumerate() {
            let column = index % columns;
            let row = index / columns;
            let x = border.left
                + padding.left
                + column_widths[..column].iter().sum::<f32>()
                + column as f32 * column_gap;
            let y = border.top
                + padding.top
                + row_heights[..row].iter().sum::<f32>()
                + row as f32 * row_gap;
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

trait MarginStart {
    fn top_or(&self, is_row: bool) -> f32;
}

impl MarginStart for Edges<f32> {
    fn top_or(&self, is_row: bool) -> f32 {
        if is_row { self.top } else { self.left }
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
