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

#[test]
fn parses_mask_size_position_and_repeat() {
    let css = Stylesheet::parse(
        r#"
        .fade {
            mask-image: linear-gradient(to bottom, transparent, black);
            mask-size: 50% 100%;
            mask-position: right top;
            mask-repeat: no-repeat;
        }
        "#,
    );
    let mut scene = Scene::new(Viewport::default());
    scene
        .roots
        .push(Node::container("hbox", vec!["fade".to_string()]));
    scene.resolve_styles(&css);
    let effects = &scene.roots[0].style.effects;
    assert!(matches!(
        effects.mask_size,
        widdgyy::css::properties::BackgroundSize::Explicit(
            widdgyy::css::Length::Percent(50.0),
            widdgyy::css::Length::Percent(100.0)
        )
    ));
    assert!(matches!(
        effects.mask_position.x,
        widdgyy::css::Length::Percent(100.0)
    ));
    assert_eq!(
        effects.mask_repeat,
        widdgyy::css::properties::BackgroundRepeat::NoRepeat
    );
}
