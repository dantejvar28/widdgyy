// SPDX-FileCopyrightText: 2026 Daniel
// SPDX-License-Identifier: GPL-3.0-or-later

use widdgyy::css::{Stylesheet, Viewport};
use widdgyy::scene::{Node, Scene};

#[test]
fn resolves_backdrop_blur_radius() {
    let css = Stylesheet::parse(
        r#"
        .glass { backdrop-filter: blur(24px); }
        "#,
    );
    let mut scene = Scene::new(Viewport::default());
    scene
        .roots
        .push(Node::container("hbox", vec!["glass".to_string()]));
    scene.resolve_styles(&css);
    assert_eq!(scene.roots[0].backdrop_blur(), Some(24.0));
}

#[test]
fn ignores_nodes_without_backdrop_filter() {
    let css = Stylesheet::parse(".plain { opacity: 0.5; }");
    let mut scene = Scene::new(Viewport::default());
    scene
        .roots
        .push(Node::container("hbox", vec!["plain".to_string()]));
    scene.resolve_styles(&css);
    assert_eq!(scene.roots[0].backdrop_blur(), None);
}
