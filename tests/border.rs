// SPDX-FileCopyrightText: 2026 Daniel
// SPDX-License-Identifier: GPL-3.0-or-later

use widdgyy::css::{Stylesheet, Viewport};
use widdgyy::scene::{Node, Scene};

#[test]
fn parses_border_image_longhands() {
    let css = Stylesheet::parse(
        r#"
        .frame {
            border-style: solid;
            border-width: 10px;
            border-image-source: url(assets/tree.png);
            border-image-slice: 30 fill;
            border-image-width: 20px;
        }
        "#,
    );
    let mut scene = Scene::new(Viewport::default());
    scene
        .roots
        .push(Node::container("hbox", vec!["frame".to_string()]));
    scene.resolve_styles(&css);
    let image = scene.roots[0]
        .style
        .border
        .image
        .as_ref()
        .expect("border image parsed");
    assert_eq!(image.source, "assets/tree.png");
    assert!(image.fill);
    assert!(matches!(
        image.width.left,
        widdgyy::css::Length::Px(value) if (value - 20.0).abs() < f32::EPSILON
    ));
}

#[test]
fn parses_border_image_outset() {
    let css = Stylesheet::parse(
        r#"
        .frame {
            border-image-source: url(assets/tree.png);
            border-image-slice: 30;
            border-image-outset: 4px;
        }
        "#,
    );
    let mut scene = Scene::new(Viewport::default());
    scene
        .roots
        .push(Node::container("hbox", vec!["frame".to_string()]));
    scene.resolve_styles(&css);
    let image = scene.roots[0].style.border.image.as_ref().unwrap();
    assert!(matches!(
        image.outset.left,
        widdgyy::css::Length::Px(value) if (value - 4.0).abs() < f32::EPSILON
    ));
}
