// SPDX-FileCopyrightText: 2026 Daniel
// SPDX-License-Identifier: GPL-3.0-or-later

use widdgyy::css::selector::PseudoElement;
use widdgyy::css::{Length, Position, Selector, Stylesheet, Viewport};
use widdgyy::layout::LayoutEngine;
use widdgyy::scene::{Node, NodeKind, Scene};
use widdgyy::text::TextEngine;

fn build(css: &str) -> Scene {
    let sheet = Stylesheet::parse(css);
    let mut scene = Scene::new(Viewport::default());
    let mut root = Node::container("vbox", vec!["root".to_string()]);
    root.children
        .push(Node::container("vbox", vec!["panel".to_string()]));
    scene.roots.push(root);
    let mut text = TextEngine::new();
    scene.resolve_styles(&sheet);
    scene.layout(&mut LayoutEngine::new(&mut text));
    scene
}

#[test]
fn parses_before_and_after_selectors() {
    let before = Selector::parse(".panel::before").unwrap();
    assert_eq!(before.pseudo_element(), Some(PseudoElement::Before));
    let after = Selector::parse(".panel:after").unwrap();
    assert_eq!(after.pseudo_element(), Some(PseudoElement::After));
    assert!(Selector::parse(".panel::marker").is_err());
}

#[test]
fn generates_before_and_after_boxes() {
    let scene = build(
        r#"
        .root { width: 400px; height: 300px; }
        .panel {
            width: 200px;
            height: 100px;
            border: 1px solid #fff;
            border-radius: 20px;
            background: #00f;
        }
        .panel::before {
            content: "";
            position: absolute;
            inset: 0;
            border-radius: inherit;
            background: #f00;
            pointer-events: none;
        }
        .panel::after {
            content: "AFTER";
            position: absolute;
            right: 4px;
            bottom: 4px;
            width: 40px;
            height: 20px;
        }
        "#,
    );
    let panel = &scene.roots[0].children[0];
    assert_eq!(panel.children.len(), 2);

    let before = &panel.children[0];
    assert_eq!(before.pseudo_element, Some(PseudoElement::Before));
    assert!(matches!(before.kind, NodeKind::Container));
    assert_eq!(before.style.position, Position::Absolute);
    assert!(matches!(
        before.style.border.radius.left,
        Length::Px(value) if (value - 20.0).abs() < 1e-6
    ));
    let background = before.style.background.color.components;
    assert!(background[0] > 0.9 && background[1] < 0.1 && background[3] > 0.9);
    // Padding box: the 200px content box, positioned inside the 1px border.
    assert!((before.layout.width - 200.0).abs() < 1e-3);
    assert!((before.layout.height - 100.0).abs() < 1e-3);
    assert!((before.layout.x - 1.0).abs() < 1e-3);
    assert!((before.layout.y - 1.0).abs() < 1e-3);

    let after = panel.children.last().unwrap();
    assert_eq!(after.pseudo_element, Some(PseudoElement::After));
    assert!(matches!(after.kind, NodeKind::Text(_)));
    assert!(matches!(
        after.text_content().map(|content| content.text.as_str()),
        Some("AFTER")
    ));
    assert!((after.layout.width - 40.0).abs() < 1e-3);
    assert!((after.layout.height - 20.0).abs() < 1e-3);
    assert!((after.layout.x - 157.0).abs() < 1e-3);
    assert!((after.layout.y - 77.0).abs() < 1e-3);
}

#[test]
fn pseudo_elements_do_not_reset_first_child_indices() {
    let sheet = Stylesheet::parse(
        r#"
        .root { width: 100px; height: 100px; }
        .panel::before { content: ""; }
        .item:first-child { color: #ff0000; }
        "#,
    );
    let mut scene = Scene::new(Viewport::default());
    let mut root = Node::container("vbox", vec!["root".to_string()]);
    let mut panel = Node::container("vbox", vec!["panel".to_string()]);
    panel
        .children
        .push(Node::container("vbox", vec!["item".to_string()]));
    root.children.push(panel);
    scene.roots.push(root);
    let mut text = TextEngine::new();
    scene.resolve_styles(&sheet);
    scene.layout(&mut LayoutEngine::new(&mut text));

    let item = &scene.roots[0].children[0].children[1];
    assert_eq!(item.pseudo_element, None);
    let color = item.style.text.color.components;
    assert!(color[0] > 0.9 && color[1] < 0.1);
}

#[test]
fn content_none_generates_no_box() {
    let scene = build(
        r#"
        .root { width: 100px; height: 100px; }
        .panel { width: 100px; height: 100px; }
        .panel::before { content: none; }
        .panel::after { content: normal; }
        "#,
    );
    assert!(scene.roots[0].children[0].children.is_empty());
}

#[test]
fn accepts_pointer_events_and_pseudo_properties() {
    let sheet = Stylesheet::parse(
        r#"
        .panel::before {
            content: "";
            position: absolute;
            inset: 0;
            border-radius: inherit;
            pointer-events: none;
        }
        "#,
    );
    assert!(sheet.errors.is_empty(), "{:?}", sheet.errors);
    assert!(!sheet.unsupported_properties.contains_key("content"));
    assert!(!sheet.unsupported_properties.contains_key("pointer-events"));
    assert!(!sheet.unsupported_properties.contains_key("border-radius"));
}
