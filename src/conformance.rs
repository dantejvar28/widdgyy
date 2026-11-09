// SPDX-FileCopyrightText: 2026 Daniel
// SPDX-License-Identifier: GPL-3.0-or-later

//! Offline layout dump used by the Firefox conformance harness.

use std::path::PathBuf;

use anyhow::{Context, Result};
use serde_json::json;

use crate::config::ConfigLoader;
use crate::css::{Axis, Size, Stylesheet, Viewport};
use crate::layout::LayoutEngine;
use crate::scene::{Node, NodeKind};
use crate::text::{ShapeRequest, TextEngine};

pub fn dump_layout(config_path: Option<PathBuf>, viewport: Viewport) -> Result<()> {
    let path = config_path.unwrap_or_else(crate::user_config_path);
    let loader = ConfigLoader::new(&path);
    let mut scene = loader
        .load(viewport)
        .with_context(|| format!("loading configuration {}", path.display()))?;
    let css_path = loader.config_dir().join("styles.css");
    let css = std::fs::read_to_string(&css_path).unwrap_or_default();
    let stylesheet = Stylesheet::parse(&css);

    let mut text = TextEngine::new();
    for face in &stylesheet.font_faces {
        let font_path = loader.config_dir().join(&face.source);
        text.load_font_file(font_path);
    }
    scene.viewport = viewport;
    scene.resolve_styles(&stylesheet);
    scene.layout(&mut LayoutEngine::new(&mut text));

    let mut nodes = Vec::new();
    for root in &scene.roots {
        collect(root, &mut nodes, &mut text, viewport);
    }
    let document = json!({
        "viewport": {
            "width": viewport.width,
            "height": viewport.height,
            "scale_factor": viewport.scale_factor,
        },
        "nodes": nodes,
    });
    println!("{}", serde_json::to_string_pretty(&document)?);
    Ok(())
}

fn collect(
    node: &Node,
    out: &mut Vec<serde_json::Value>,
    text_engine: &mut TextEngine,
    viewport: Viewport,
) {
    let text = node
        .text_content()
        .map(|content| content.text.clone())
        .unwrap_or_default();
    let lines = node
        .text_content()
        .map(|_| shape_lines(node, text_engine, viewport))
        .unwrap_or_default();
    out.push(json!({
        "id": node.id,
        "tag": node.tag,
        "classes": node.classes,
        "pseudo": node.pseudo_element.map(|pseudo| format!("{pseudo:?}").to_ascii_lowercase()),
        "text": text,
        "x": node.absolute_x,
        "y": node.absolute_y,
        "width": node.layout.width,
        "height": node.layout.height,
        "content": {
            "x": node.absolute_x + node.layout.content_x,
            "y": node.absolute_y + node.layout.content_y,
            "width": node.layout.content_width,
            "height": node.layout.content_height,
        },
        "lines": lines,
    }));
    if matches!(node.kind, NodeKind::Container) {
        for child in &node.children {
            collect(child, out, text_engine, viewport);
        }
    }
}

fn shape_lines(
    node: &Node,
    text_engine: &mut TextEngine,
    viewport: Viewport,
) -> Vec<serde_json::Value> {
    let Some(content) = node.text_content() else {
        return Vec::new();
    };
    let style = &node.style;
    let context = style.resolve_context(viewport, node.layout.width, node.layout.height);
    let max_width = match &style.width {
        Size::Length(length) => Some(length.resolve(&context, Axis::Horizontal)),
        _ => style.max_width.resolve(&context, Axis::Horizontal),
    };
    let request = ShapeRequest {
        text: &content.text,
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
            .resolve(&context, Axis::Horizontal)
            .max(0.0),
        white_space: style.text.white_space,
        overflow_wrap: style.text.overflow_wrap,
        word_break: style.text.word_break,
        transform: style.text.transform,
        text_overflow: style.text.overflow,
        line_clamp: style.text.line_clamp,
    };
    let shaped = text_engine.shape(request);
    shaped
        .lines
        .iter()
        .map(|line| {
            json!({
                "x": node.absolute_x + node.layout.content_x + line.x,
                "y": node.absolute_y + node.layout.content_y + line.top,
                "width": line.width,
                "height": line.height,
            })
        })
        .collect()
}
