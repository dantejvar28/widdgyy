// SPDX-FileCopyrightText: 2026 Daniel
// SPDX-License-Identifier: GPL-3.0-or-later

use widdgyy::css::{Stylesheet, Viewport};
use widdgyy::layout::LayoutEngine;
use widdgyy::render::{MediaStore, Painter};
use widdgyy::scene::{Node, Scene};
use widdgyy::text::TextEngine;

#[test]
fn nested_filter_splits_paint_layers() {
    let css = Stylesheet::parse(
        r#"
        .root { display: flex; width: 200px; height: 200px; }
        .panel { width: 100px; height: 100px; filter: blur(4px); }
        .label { font-size: 12px; }
        "#,
    );
    let mut scene = Scene::new(Viewport::default());
    let mut root = Node::container("hbox", vec!["root".to_string()]);
    let mut panel = Node::container("vbox", vec!["panel".to_string()]);
    panel
        .children
        .push(Node::text("text", vec!["label".to_string()], "blur"));
    root.children.push(panel);
    scene.roots.push(root);
    let mut text = TextEngine::new();
    scene.resolve_styles(&css);
    scene.layout(&mut LayoutEngine::new(&mut text));

    let mut media = MediaStore::new();
    let mut paint_text = TextEngine::new();
    let mut painter = Painter::new(&mut paint_text, &mut media);
    let mut layers = Vec::new();
    painter.paint_layers(&scene, &mut layers);

    assert_eq!(layers.len(), 3, "expected base, filtered child and tail");
    assert!(layers[0].filter.is_none());
    assert!(layers[1].filter.is_some());
    assert!(layers[2].filter.is_none());
}
