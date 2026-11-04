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
