// SPDX-FileCopyrightText: 2026 Daniel
// SPDX-License-Identifier: GPL-3.0-or-later

//! Grid template and placement properties.

use super::{Declarations, GridAutoFlow, TrackSize, split_commas, split_ws};
use crate::css::values::Length;

pub fn apply(decl: &mut Declarations, name: &str, value: &str) -> bool {
    match name {
        "grid-template-columns" | "grid-template-rows" => {
            let tracks = parse_tracks(value);
            if name == "grid-template-columns" {
                decl.grid_template_columns = Some(tracks);
            } else {
                decl.grid_template_rows = Some(tracks);
            }
            true
        }
        "grid-auto-flow" => {
            decl.grid_auto_flow = Some(match value.to_ascii_lowercase().as_str() {
                "row" => GridAutoFlow::Row,
                "column" => GridAutoFlow::Column,
                "row dense" | "dense" => GridAutoFlow::RowDense,
                "column dense" => GridAutoFlow::ColumnDense,
                _ => return false,
            });
            true
        }
        "grid-column" | "grid-column-start" | "grid-column-end" => {
            let (start, span) = parse_placement(value);
            let current = decl.grid_column.unwrap_or((1, 1));
            if name == "grid-column-start" {
                decl.grid_column = Some((start, current.1));
            } else if name == "grid-column-end" {
                decl.grid_column = Some((current.0, span.max(1)));
            } else {
                decl.grid_column = Some((start, span));
            }
            true
        }
        "grid-row" | "grid-row-start" | "grid-row-end" => {
            let (start, span) = parse_placement(value);
            let current = decl.grid_row.unwrap_or((1, 1));
            if name == "grid-row-start" {
                decl.grid_row = Some((start, current.1));
            } else if name == "grid-row-end" {
                decl.grid_row = Some((current.0, span.max(1)));
            } else {
                decl.grid_row = Some((start, span));
            }
            true
        }
        "grid-template" | "grid" => true,
        _ => false,
    }
}

fn parse_tracks(value: &str) -> Vec<TrackSize> {
    let mut tracks = Vec::new();
    for part in split_commas(value) {
        let part = part.trim();
        let lower = part.to_ascii_lowercase();
        if lower.starts_with("repeat(") && lower.ends_with(')') {
            let inner = &part[7..part.len() - 1];
            let mut split = inner.splitn(2, ',');
            let count = split
                .next()
                .and_then(|value| value.trim().parse::<u32>().ok())
                .unwrap_or(1);
            let rest = split.next().unwrap_or("");
            tracks.push(TrackSize::Repeat(count, parse_tracks(rest)));
        } else if lower.starts_with("minmax(") && lower.ends_with(')') {
            let inner = &part[7..part.len() - 1];
            let mut split = inner.splitn(2, ',');
            let min = split.next().unwrap_or("auto").trim();
            let max = split.next().unwrap_or("auto").trim();
            tracks.push(
                parse_track(min)
                    .or_else(|| parse_track(max))
                    .unwrap_or(TrackSize::Auto),
            );
        } else if let Some(track) = parse_track(part) {
            tracks.push(track);
        }
    }
    tracks
}

fn parse_track(value: &str) -> Option<TrackSize> {
    let text = value.trim();
    Some(match text.to_ascii_lowercase().as_str() {
        "auto" => TrackSize::Auto,
        "min-content" => TrackSize::MinContent,
        "max-content" => TrackSize::MaxContent,
        _ => {
            if let Some(inner) = text
                .strip_prefix("fit-content(")
                .and_then(|value| value.strip_suffix(')'))
            {
                return Some(TrackSize::FitContent(Length::parse(inner).ok()?));
            }
            if let Some(fr) = text.strip_suffix("fr") {
                return Some(TrackSize::Fr(fr.trim().parse().ok()?));
            }
            if let Some(percent) = text.strip_suffix('%') {
                return Some(TrackSize::Percent(percent.trim().parse().ok()?));
            }
            if let Some(px) = text.strip_suffix("px") {
                return Some(TrackSize::Px(px.trim().parse().ok()?));
            }
            let number = text.parse::<f32>().ok()?;
            if number == 0.0 {
                TrackSize::Px(0.0)
            } else {
                TrackSize::Fr(number)
            }
        }
    })
}

fn parse_placement(value: &str) -> (i32, i32) {
    let tokens = split_ws(value);
    let start = tokens
        .first()
        .and_then(|token| token.parse::<i32>().ok())
        .unwrap_or(1);
    let mut span = 1;
    if let Some(second) = tokens.get(1) {
        if let Some(number) = second.strip_prefix("span ") {
            span = number.trim().parse::<i32>().unwrap_or(1).max(1);
        } else if let Ok(end) = second.parse::<i32>() {
            span = (end - start).max(1);
        }
    } else if let Some(after_span) = value.to_ascii_lowercase().strip_prefix("span ") {
        span = after_span.trim().parse::<i32>().unwrap_or(1).max(1);
    }
    (start, span)
}
