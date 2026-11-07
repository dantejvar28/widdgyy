// SPDX-FileCopyrightText: 2026 Daniel
// SPDX-License-Identifier: GPL-3.0-or-later

use widdgyy::css::{Stylesheet, Viewport};
use widdgyy::scene::{Node, Scene};

#[test]
fn parses_mask_image_gradient() {
    let css = Stylesheet::parse(
        r#"
        .fade {
            mask-image: linear-gradient(to bottom, transparent 0%, black 25%, black 75%, transparent 100%);
        }
        "#,
    );
    let mut scene = Scene::new(Viewport::default());
    scene
        .roots
        .push(Node::container("hbox", vec!["fade".to_string()]));
    scene.resolve_styles(&css);
    assert!(scene.roots[0].mask_image().is_some());
}

#[test]
fn ignores_elements_without_mask_image() {
    let css = Stylesheet::parse(".plain { color: red; }");
    let mut scene = Scene::new(Viewport::default());
    scene
        .roots
        .push(Node::container("hbox", vec!["plain".to_string()]));
    scene.resolve_styles(&css);
    assert!(scene.roots[0].mask_image().is_none());
}
