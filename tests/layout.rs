// SPDX-FileCopyrightText: 2026 Daniel
// SPDX-License-Identifier: GPL-3.0-or-later

use widdgyy::css::{Stylesheet, Viewport};
use widdgyy::layout::LayoutEngine;
use widdgyy::scene::{Anchor, Node, Scene};
use widdgyy::text::TextEngine;

#[test]
fn padding_and_centering_are_applied() {
    let css = Stylesheet::parse(
        r#"
        .root {
            display: flex;
            box-sizing: border-box;
            width: 100%;
            height: 100%;
            padding: 40px;
            justify-content: center;
            align-items: center;
        }
        .card {
            display: flex;
            flex-direction: column;
            padding: 20px;
            gap: 8px;
        }
        .label { font-size: 24px; }
        "#,
    );

    let mut scene = Scene::new(Viewport {
        width: 800.0,
        height: 600.0,
        scale_factor: 1.0,
    });
    let mut root = Node::container("vbox", vec!["root".to_string()]);
    root.anchor = Anchor::TopLeft;
    let mut card = Node::container("vbox", vec!["card".to_string()]);
    card.anchor = Anchor::TopLeft;
    card.children
        .push(Node::text("text", vec!["label".to_string()], "Hello"));
    root.children.push(card);
    scene.roots.push(root);

    let mut text = TextEngine::new();
    scene.resolve_styles(&css);
    scene.layout(&mut LayoutEngine::new(&mut text));

    let root = &scene.roots[0];
    assert_eq!(root.layout.width, 800.0);
    assert_eq!(root.layout.height, 600.0);
    assert_eq!(root.layout.content_width, 720.0);
    assert_eq!(root.layout.content_height, 520.0);

    let card = &root.children[0];
    println!("root {:?}", root.layout);
    println!("card {:?}", card.layout);
    println!(
        "root padding {:?} card padding {:?}",
        root.style.padding, card.style.padding
    );
    println!(
        "root box {:?} card box {:?}",
        root.style.box_sizing, card.style.box_sizing
    );
    // The card is centered inside the padded content box.
    let expected_x = 40.0 + (720.0 - card.layout.width) / 2.0;
    assert!(
        (card.layout.x - expected_x).abs() < 0.5,
        "{} != {}",
        card.layout.x,
        expected_x
    );
    assert!(card.layout.x >= 40.0);

    let label = &card.children[0];
    assert!(label.layout.x >= 20.0, "child ignored card padding");
    assert!(label.layout.y >= 20.0, "child ignored card padding");
    assert!(label.absolute_x > card.absolute_x);
}

#[test]
fn positions_absolute_and_relative_children() {
    let css = Stylesheet::parse(
        r#"
        .root { width: 500px; height: 300px; padding: 20px; position: relative; }
        .abs { position: absolute; left: 10px; top: 15px; width: 40px; height: 30px; }
        .rel { position: relative; left: 5px; top: 7px; width: 20px; height: 10px; }
        "#,
    );
    let mut scene = Scene::new(Viewport {
        width: 500.0,
        height: 300.0,
        scale_factor: 1.0,
    });
    let mut root = Node::container("vbox", vec!["root".to_string()]);
    let mut abs = Node::text("text", vec!["abs".to_string()], "a");
    abs.anchor = Anchor::TopLeft;
    let mut rel = Node::text("text", vec!["rel".to_string()], "r");
    rel.anchor = Anchor::TopLeft;
    root.children.push(abs);
    root.children.push(rel);
    scene.roots.push(root);
    let mut text = TextEngine::new();
    scene.resolve_styles(&css);
    scene.layout(&mut LayoutEngine::new(&mut text));
    let root = &scene.roots[0];
    let abs = &root.children[0];
    let rel = &root.children[1];
    assert!((abs.layout.x - 30.0).abs() < 0.5, "{}", abs.layout.x);
    assert!((abs.layout.y - 35.0).abs() < 0.5, "{}", abs.layout.y);
    assert!(rel.layout.x >= 20.0);
    assert!(rel.layout.y >= 20.0);
}

#[test]
fn flex_order_changes_layout_order() {
    let css = Stylesheet::parse(
        r#"
        .root { display: flex; width: 300px; height: 50px; }
        .a { order: 2; width: 30px; height: 20px; }
        .b { order: 1; width: 30px; height: 20px; }
        .c { order: 3; width: 30px; height: 20px; }
        "#,
    );
    let mut scene = Scene::new(Viewport {
        width: 300.0,
        height: 50.0,
        scale_factor: 1.0,
    });
    let mut root = Node::container("hbox", vec!["root".to_string()]);
    for class in ["a", "b", "c"] {
        let mut node = Node::text("text", vec![class.to_string()], class);
        node.anchor = Anchor::TopLeft;
        root.children.push(node);
    }
    scene.roots.push(root);
    let mut text = TextEngine::new();
    scene.resolve_styles(&css);
    scene.layout(&mut LayoutEngine::new(&mut text));
    let children = &scene.roots[0].children;
    assert!(children[1].layout.x < children[0].layout.x);
    assert!(children[0].layout.x < children[2].layout.x);
}

#[test]
fn flex_wrap_places_items_on_new_lines() {
    let css = Stylesheet::parse(
        r#"
        .root {
            display: flex;
            flex-wrap: wrap;
            width: 200px;
            height: 200px;
        }
        .item { width: 80px; height: 30px; }
        "#,
    );
    let mut scene = Scene::new(Viewport::default());
    let mut root = Node::container("hbox", vec!["root".to_string()]);
    for _ in 0..3 {
        root.children
            .push(Node::container("text", vec!["item".to_string()]));
    }
    scene.roots.push(root);
    let mut text = TextEngine::new();
    scene.resolve_styles(&css);
    scene.layout(&mut LayoutEngine::new(&mut text));

    let root = &scene.roots[0];
    assert_eq!(root.children[0].layout.x, 0.0);
    assert_eq!(root.children[1].layout.x, 80.0);
    assert_eq!(root.children[2].layout.x, 0.0);
    assert_eq!(root.children[2].layout.y, 30.0);
}

#[test]
fn grid_places_explicit_and_auto_items() {
    let css = Stylesheet::parse(
        r#"
        .root {
            display: grid;
            width: 200px;
            height: 200px;
            grid-template-columns: 100px 100px;
            grid-template-rows: 50px 50px;
        }
        .item { width: 20px; height: 20px; }
        .second-column { grid-column: 2; grid-row: 1; }
        .second-row { grid-column: 1; grid-row: 2; }
        "#,
    );
    let mut scene = Scene::new(Viewport::default());
    let mut root = Node::container("grid", vec!["root".to_string()]);
    for class in ["item second-column", "item second-row", "item"] {
        let classes = class.split_whitespace().map(str::to_string).collect();
        root.children.push(Node::container("text", classes));
    }
    scene.roots.push(root);
    let mut text = TextEngine::new();
    scene.resolve_styles(&css);
    scene.layout(&mut LayoutEngine::new(&mut text));

    let root = &scene.roots[0];
    println!("errors={:?}", css.errors);
    println!("rules={}", css.rules.len());
    println!(
        "display={:?} columns={:?}",
        root.style.display, root.style.grid.columns
    );
    for (index, child) in root.children.iter().enumerate() {
        println!(
            "child {index} grid={:?} layout={:?}",
            child.style.grid, child.layout
        );
    }
    assert_eq!(root.children[0].layout.x, 100.0);
    assert_eq!(root.children[0].layout.y, 0.0);
    assert_eq!(root.children[1].layout.x, 0.0);
    assert_eq!(root.children[1].layout.y, 50.0);
    assert_eq!(root.children[2].layout.x, 0.0);
    assert_eq!(root.children[2].layout.y, 0.0);
}
