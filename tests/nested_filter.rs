// SPDX-FileCopyrightText: 2026 Daniel
// SPDX-License-Identifier: GPL-3.0-or-later

use widdgyy::css::{Stylesheet, Viewport};
use widdgyy::layout::LayoutEngine;
use widdgyy::render::filter::LayerItem;
use widdgyy::render::{FilterLayer, MediaStore, Painter};
use widdgyy::scene::{Node, Scene};
use widdgyy::text::TextEngine;

fn paint(css: &str, root: Node) -> Vec<FilterLayer> {
    let sheet = Stylesheet::parse(css);
    let mut scene = Scene::new(Viewport::default());
    scene.roots.push(root);
    let mut text = TextEngine::new();
    scene.resolve_styles(&sheet);
    scene.layout(&mut LayoutEngine::new(&mut text));

    let mut media = MediaStore::new();
    let mut paint_text = TextEngine::new();
    let mut painter = Painter::new(&mut paint_text, &mut media);
    let mut layers = Vec::new();
    painter.paint_layers(&scene, &mut layers);
    layers
}

fn panel(_label: &str, classes: &[&str]) -> Node {
    Node::container("vbox", classes.iter().map(|c| c.to_string()).collect())
}

#[test]
fn nested_filter_splits_paint_layers() {
    let mut root = Node::container("hbox", vec!["root".to_string()]);
    root.children.push(panel("panel", &["panel"]));
    let layers = paint(
        r#"
        .root { display: flex; width: 200px; height: 200px; background: #000; }
        .panel { width: 100px; height: 100px; filter: blur(4px); background: #f00; }
        "#,
        root,
    );

    assert_eq!(layers.len(), 2, "expected base and filtered child");
    assert!(layers[0].filter.is_none());
    assert!(layers[0].mask.is_none());
    assert!(layers[1].filter.is_some());
    assert!(matches!(layers[1].items[0], LayerItem::Scene(_)));
}

#[test]
fn filter_under_opacity_keeps_the_group_opacity() {
    let mut root = Node::container("hbox", vec!["root".to_string()]);
    root.children.push(panel("panel", &["panel"]));
    let layers = paint(
        r#"
        .root {
            display: flex;
            width: 400px;
            height: 300px;
            background: #000;
            opacity: 0.5;
        }
        .panel { width: 200px; height: 100px; background: #f00; filter: blur(4px); background: #f00; }
        "#,
        root,
    );

    assert_eq!(layers.len(), 1, "the opacity must own one offscreen group");
    let group = &layers[0];
    assert!(group.filter.is_none());
    assert!((group.opacity - 0.5).abs() < 1e-6);
    assert_eq!(group.items.len(), 2, "root paint run plus the panel group");
    assert!(matches!(group.items[0], LayerItem::Scene(_)));
    let LayerItem::Layer(child) = &group.items[1] else {
        panic!("expected a nested filtered group");
    };
    assert!(child.filter.is_some());
}

#[test]
fn nested_filters_form_a_tree() {
    let mut root = Node::container("hbox", vec!["root".to_string()]);
    root.children.push(panel("panel", &["panel"]));
    let layers = paint(
        r#"
        .root {
            display: flex;
            width: 400px;
            height: 300px;
            background: #000;
            filter: blur(2px);
        }
        .panel { width: 200px; height: 100px; background: #0f0; filter: drop-shadow(0 0 4px #000); background: #0f0; }
        "#,
        root,
    );

    assert_eq!(layers.len(), 1);
    let group = &layers[0];
    assert!(
        group.filter.is_some(),
        "outer filter stays on the root group"
    );
    let nested = group
        .items
        .iter()
        .find_map(|item| match item {
            LayerItem::Layer(child) => Some(child),
            LayerItem::Scene(_) => None,
        })
        .expect("child filter must be a nested layer");
    assert!(nested.filter.is_some());
}

#[test]
fn clip_path_over_a_filtered_subtree_becomes_a_composite_mask() {
    let mut root = Node::container("hbox", vec!["root".to_string()]);
    root.children.push(panel("panel", &["panel"]));
    let layers = paint(
        r#"
        .root {
            display: flex;
            width: 400px;
            height: 300px;
            background: #000;
            opacity: 0.8;
            clip-path: inset(10px);
        }
        .panel { width: 200px; height: 100px; filter: blur(4px); background: #f00; }
        "#,
        root,
    );

    assert_eq!(layers.len(), 1);
    assert!(
        layers[0].mask.is_some(),
        "clip must be applied at composite"
    );
    assert!((layers[0].opacity - 0.8).abs() < 1e-6);
}

#[test]
fn overflow_hidden_over_a_filtered_subtree_becomes_a_composite_mask() {
    let mut root = Node::container("hbox", vec!["root".to_string()]);
    root.children.push(panel("panel", &["panel"]));
    let layers = paint(
        r#"
        .root {
            display: flex;
            width: 400px;
            height: 300px;
            overflow: hidden;
        }
        .panel { width: 200px; height: 100px; filter: blur(4px); background: #f00; }
        "#,
        root,
    );

    assert_eq!(layers.len(), 1);
    assert!(layers[0].mask.is_some());
}

#[test]
fn isolation_over_a_filtered_subtree_still_splits() {
    let mut root = Node::container("hbox", vec!["root".to_string()]);
    root.children.push(panel("panel", &["panel"]));
    let layers = paint(
        r#"
        .root { display: flex; width: 400px; height: 300px; isolation: isolate; }
        .panel { width: 200px; height: 100px; filter: blur(4px); background: #f00; }
        "#,
        root,
    );

    assert_eq!(layers.len(), 1);
    assert!(layers[0].is_plain() || layers[0].filter.is_none());
    let nested = layers[0]
        .items
        .iter()
        .any(|item| matches!(item, LayerItem::Layer(child) if child.filter.is_some()));
    assert!(nested, "the filtered child must keep its own group");
}
