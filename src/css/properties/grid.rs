// SPDX-FileCopyrightText: 2026 Daniel
// SPDX-License-Identifier: GPL-3.0-or-later

//! Grid template, placement and auto-track properties.

use std::collections::BTreeMap;

use super::{Declarations, GridArea, GridAutoFlow, GridLine, GridPlacement, TrackSize, split_ws};
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
        "grid-auto-columns" => {
            decl.grid_auto_columns = Some(parse_tracks(value));
            true
        }
        "grid-auto-rows" => {
            decl.grid_auto_rows = Some(parse_tracks(value));
            true
        }
        "grid-template-areas" => {
            decl.grid_template_areas = Some(parse_areas(value));
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
            let placement = decl.grid_column.clone().unwrap_or_default();
            decl.grid_column = Some(apply_placement_property(placement, name, value, "column"));
            true
        }
        "grid-row" | "grid-row-start" | "grid-row-end" => {
            let placement = decl.grid_row.clone().unwrap_or_default();
            decl.grid_row = Some(apply_placement_property(placement, name, value, "row"));
            true
        }
        "grid-area" => {
            let parts: Vec<&str> = split_slash(value);
            let mut row = GridPlacement::default();
            let mut column = GridPlacement::default();
            match parts.as_slice() {
                [single] => {
                    let line = parse_line(single);
                    if let GridLine::Name(name) = &line {
                        row.start = GridLine::Name(name.clone());
                        row.end = GridLine::Name(name.clone());
                        column.start = GridLine::Name(name.clone());
                        column.end = GridLine::Name(name.clone());
                    } else {
                        row.start = line;
                    }
                }
                [row_start, column_start] => {
                    row.start = parse_line(row_start);
                    column.start = parse_line(column_start);
                }
                [row_start, column_start, row_end] => {
                    row.start = parse_line(row_start);
                    column.start = parse_line(column_start);
                    row.end = parse_line(row_end);
                }
                [row_start, column_start, row_end, column_end, ..] => {
                    row.start = parse_line(row_start);
                    column.start = parse_line(column_start);
                    row.end = parse_line(row_end);
                    column.end = parse_line(column_end);
                }
                [] => {}
            }
            decl.grid_row = Some(row);
            decl.grid_column = Some(column);
            true
        }
        "grid-template" | "grid" => true,
        _ => false,
    }
}

fn apply_placement_property(
    mut placement: GridPlacement,
    property: &str,
    value: &str,
    _axis: &str,
) -> GridPlacement {
    let parts: Vec<&str> = split_slash(value);
    match property {
        "grid-column-start" | "grid-row-start" => {
            if let Some(part) = parts.first() {
                placement.start = parse_line(part);
            }
        }
        "grid-column-end" | "grid-row-end" => {
            if let Some(part) = parts.first() {
                placement.end = parse_line(part);
            }
        }
        _ => {
            placement.start = parts
                .first()
                .map(|part| parse_line(part))
                .unwrap_or(GridLine::Auto);
            placement.end = parts
                .get(1)
                .map(|part| parse_line(part))
                .unwrap_or(GridLine::Auto);
        }
    }
    placement
}

fn split_slash(value: &str) -> Vec<&str> {
    value
        .split('/')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect()
}

fn parse_line(value: &str) -> GridLine {
    let text = value.trim();
    let lower = text.to_ascii_lowercase();
    if lower == "auto" {
        return GridLine::Auto;
    }
    if let Some(rest) = lower.strip_prefix("span ") {
        if let Ok(number) = rest.trim().parse::<i32>() {
            return GridLine::Span(number.max(1));
        }
        // Named spans are accepted; the name is ignored and the span falls
        // back to one track.
        return GridLine::Span(1);
    }
    if let Ok(number) = text.parse::<i32>() {
        return GridLine::Line(number);
    }
    GridLine::Name(text.to_string())
}

fn parse_areas(value: &str) -> BTreeMap<String, GridArea> {
    let mut rows: Vec<Vec<String>> = Vec::new();
    for area in quoted_strings(value) {
        let cells: Vec<String> = split_ws(&area);
        if !cells.is_empty() {
            rows.push(cells);
        }
    }
    let mut map: BTreeMap<String, GridArea> = BTreeMap::new();
    for (row, cells) in rows.iter().enumerate() {
        for (column, name) in cells.iter().enumerate() {
            if name == "." || name.eq_ignore_ascii_case("none") {
                continue;
            }
            map.entry(name.clone())
                .and_modify(|area| {
                    let end_row = row + 1;
                    let end_column = column + 1;
                    area.row = area.row.min(row);
                    area.column = area.column.min(column);
                    area.row_span = area.row_span.max(end_row - area.row);
                    area.column_span = area.column_span.max(end_column - area.column);
                })
                .or_insert(GridArea {
                    row,
                    column,
                    row_span: 1,
                    column_span: 1,
                });
        }
    }
    map
}

fn quoted_strings(value: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut chars = value.chars().peekable();
    while let Some(character) = chars.next() {
        if character == '"' || character == '\'' {
            let quote = character;
            let mut current = String::new();
            for next in chars.by_ref() {
                if next == quote {
                    break;
                }
                current.push(next);
            }
            result.push(current);
        }
    }
    result
}

fn parse_tracks(value: &str) -> Vec<TrackSize> {
    let mut tracks = Vec::new();
    for part in split_tracks(value) {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
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
            tracks.push(TrackSize::MinMax(
                Box::new(parse_track(min).unwrap_or(TrackSize::Auto)),
                Box::new(parse_track(max).unwrap_or(TrackSize::Auto)),
            ));
        } else if let Some(track) = parse_track(part) {
            tracks.push(track);
        }
    }
    tracks
}

/// Split a track list on top-level whitespace. Commas inside `repeat()` or
/// `minmax()` stay part of their function.
fn split_tracks(value: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0usize;
    let mut start = None;
    for (index, character) in value.char_indices() {
        match character {
            '(' => {
                depth += 1;
                start.get_or_insert(index);
            }
            ')' => {
                depth = depth.saturating_sub(1);
            }
            character if character.is_whitespace() && depth == 0 => {
                if let Some(begin) = start.take() {
                    parts.push(&value[begin..index]);
                }
            }
            _ => {
                start.get_or_insert(index);
            }
        }
    }
    if let Some(begin) = start {
        parts.push(&value[begin..]);
    }
    parts
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
