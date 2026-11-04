// SPDX-FileCopyrightText: 2026 Daniel
// SPDX-License-Identifier: GPL-3.0-or-later

//! Flex container/child properties.

use super::{
    AlignContent, AlignItems, Declarations, FlexDirection, FlexWrap, JustifyContent, parse_lengths,
    split_ws,
};
use crate::css::values::{Length, Size};

pub fn apply(decl: &mut Declarations, name: &str, value: &str) -> bool {
    match name {
        "flex-direction" => {
            decl.flex_direction = Some(match value.to_ascii_lowercase().as_str() {
                "row" => FlexDirection::Row,
                "row-reverse" => FlexDirection::RowReverse,
                "column" => FlexDirection::Column,
                "column-reverse" => FlexDirection::ColumnReverse,
                _ => return false,
            });
            true
        }
        "flex-wrap" => {
            decl.flex_wrap = Some(match value.to_ascii_lowercase().as_str() {
                "nowrap" => FlexWrap::NoWrap,
                "wrap" => FlexWrap::Wrap,
                "wrap-reverse" => FlexWrap::WrapReverse,
                _ => return false,
            });
            true
        }
        "flex-flow" => {
            for token in split_ws(value) {
                match token.to_ascii_lowercase().as_str() {
                    "row" => decl.flex_direction = Some(FlexDirection::Row),
                    "row-reverse" => decl.flex_direction = Some(FlexDirection::RowReverse),
                    "column" => decl.flex_direction = Some(FlexDirection::Column),
                    "column-reverse" => decl.flex_direction = Some(FlexDirection::ColumnReverse),
                    "nowrap" => decl.flex_wrap = Some(FlexWrap::NoWrap),
                    "wrap" => decl.flex_wrap = Some(FlexWrap::Wrap),
                    "wrap-reverse" => decl.flex_wrap = Some(FlexWrap::WrapReverse),
                    _ => {}
                }
            }
            true
        }
        "justify-content" => {
            decl.justify_content = Some(match value.to_ascii_lowercase().as_str() {
                "start" | "flex-start" | "left" => JustifyContent::Start,
                "end" | "flex-end" | "right" => JustifyContent::End,
                "center" => JustifyContent::Center,
                "space-between" => JustifyContent::SpaceBetween,
                "space-around" => JustifyContent::SpaceAround,
                "space-evenly" => JustifyContent::SpaceEvenly,
                _ => return false,
            });
            true
        }
        "align-items" => {
            decl.align_items = Some(parse_align_items(value));
            true
        }
        "align-content" => {
            decl.align_content = Some(match value.to_ascii_lowercase().as_str() {
                "start" | "flex-start" => AlignContent::Start,
                "end" | "flex-end" => AlignContent::End,
                "center" => AlignContent::Center,
                "space-between" => AlignContent::SpaceBetween,
                "space-around" => AlignContent::SpaceAround,
                "space-evenly" => AlignContent::SpaceEvenly,
                "stretch" => AlignContent::Stretch,
                _ => return false,
            });
            true
        }
        "align-self" => {
            decl.align_self = Some(parse_align_items(value));
            true
        }
        "gap" => {
            let values = parse_lengths(value).unwrap_or_default();
            if let Some(first) = values.first() {
                decl.row_gap = Some(first.clone());
                decl.column_gap = Some(values.get(1).cloned().unwrap_or_else(|| first.clone()));
                decl.gap = Some((
                    decl.row_gap.clone().unwrap_or_default(),
                    decl.column_gap.clone().unwrap_or_default(),
                ));
            }
            true
        }
        "row-gap" | "grid-row-gap" => {
            decl.row_gap = Length::parse(value).ok();
            true
        }
        "column-gap" | "grid-column-gap" => {
            decl.column_gap = Length::parse(value).ok();
            true
        }
        "order" => {
            decl.order = value.trim().parse::<i32>().ok();
            true
        }
        "flex-grow" => {
            decl.flex_grow = value.trim().parse::<f32>().ok();
            true
        }
        "flex-shrink" => {
            decl.flex_shrink = value.trim().parse::<f32>().ok();
            true
        }
        "flex-basis" => {
            decl.flex_basis = Size::parse(value).ok();
            true
        }
        "flex" => {
            apply_flex_shorthand(decl, value);
            true
        }
        _ => false,
    }
}

fn parse_align_items(value: &str) -> AlignItems {
    match value.to_ascii_lowercase().as_str() {
        "end" | "flex-end" => AlignItems::End,
        "center" => AlignItems::Center,
        "stretch" => AlignItems::Stretch,
        "baseline" => AlignItems::Baseline,
        _ => AlignItems::Start,
    }
}

fn apply_flex_shorthand(decl: &mut Declarations, value: &str) {
    let tokens = split_ws(value);
    match tokens.as_slice() {
        [single] if single == "none" => {
            decl.flex_grow = Some(0.0);
            decl.flex_shrink = Some(0.0);
            decl.flex_basis = Some(Size::Auto);
        }
        [single] if single == "auto" => {
            decl.flex_grow = Some(1.0);
            decl.flex_shrink = Some(1.0);
            decl.flex_basis = Some(Size::Auto);
        }
        [single] if single == "initial" => {
            decl.flex_grow = Some(0.0);
            decl.flex_shrink = Some(1.0);
            decl.flex_basis = Some(Size::Auto);
        }
        [grow] => {
            if let Ok(number) = grow.parse::<f32>() {
                decl.flex_grow = Some(number);
                decl.flex_shrink = Some(1.0);
                decl.flex_basis = Some(Size::Length(Length::Percent(0.0)));
            } else if let Ok(size) = Size::parse(grow) {
                decl.flex_grow = Some(1.0);
                decl.flex_shrink = Some(1.0);
                decl.flex_basis = Some(size);
            }
        }
        [grow, shrink] => {
            decl.flex_grow = grow.parse::<f32>().ok();
            decl.flex_shrink = shrink.parse::<f32>().ok();
        }
        [grow, shrink, basis] => {
            decl.flex_grow = grow.parse::<f32>().ok();
            decl.flex_shrink = shrink.parse::<f32>().ok();
            decl.flex_basis = Size::parse(basis).ok();
        }
        _ => {}
    }
}
