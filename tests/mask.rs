// SPDX-FileCopyrightText: 2026 Daniel
// SPDX-License-Identifier: GPL-3.0-or-later

use widdgyy::css::{Stylesheet, Viewport};
use widdgyy::scene::{Node, Scene};

fn resolve(css: &str) -> Scene {
    let sheet = Stylesheet::parse(css);
    let mut scene = Scene::new(Viewport::default());
    scene
        .roots
        .push(Node::container("hbox", vec!["fade".to_string()]));
    scene.resolve_styles(&sheet);
    scene
}

#[test]
fn parses_mask_image_gradient() {
    let scene = resolve(
        r#"
        .fade {
            mask-image: linear-gradient(to bottom, transparent 0%, black 25%, black 75%, transparent 100%);
        }
        "#,
    );
    assert_eq!(scene.roots[0].masks().len(), 1);
}

#[test]
fn ignores_elements_without_mask_image() {
    let scene = resolve(".plain { color: red; }");
    assert!(scene.roots[0].masks().is_empty());
}

#[test]
fn parses_mask_size_position_and_repeat() {
    let scene = resolve(
        r#"
        .fade {
            mask-image: linear-gradient(to bottom, transparent, black);
            mask-size: 50% 100%;
            mask-position: right top;
            mask-repeat: no-repeat;
        }
        "#,
    );
    let mask = &scene.roots[0].masks()[0];
    assert!(matches!(
        mask.size,
        widdgyy::css::properties::BackgroundSize::Explicit(
            widdgyy::css::Length::Percent(50.0),
            widdgyy::css::Length::Percent(100.0)
        )
    ));
    assert!(matches!(
        mask.position.x,
        widdgyy::css::Length::Percent(100.0)
    ));
    assert_eq!(
        mask.repeat,
        widdgyy::css::properties::BackgroundRepeat::NoRepeat
    );
}

#[test]
fn parses_multiple_mask_layers_and_modes() {
    let scene = resolve(
        r#"
        .fade {
            mask-image:
                linear-gradient(black, transparent),
                url(assets/star.png);
            mask-mode: alpha, luminance;
            mask-size: 50% 50%;
            mask-position: left top, right bottom;
            mask-repeat: no-repeat;
        }
        "#,
    );
    let masks = scene.roots[0].masks();
    assert_eq!(masks.len(), 2);
    assert_eq!(masks[0].mode, widdgyy::css::properties::MaskMode::Alpha);
    assert_eq!(masks[1].mode, widdgyy::css::properties::MaskMode::Luminance);
    assert!(matches!(
        masks[0].image,
        widdgyy::css::properties::background::BackgroundImage::Gradient(_)
    ));
    assert!(matches!(
        masks[1].image,
        widdgyy::css::properties::background::BackgroundImage::Url(_)
    ));
    // The size list repeats for the second layer.
    assert!(matches!(
        masks[1].size,
        widdgyy::css::properties::BackgroundSize::Explicit(_, _)
    ));
    assert!(matches!(
        masks[1].position.x,
        widdgyy::css::Length::Percent(100.0)
    ));
    assert_eq!(
        masks[1].repeat,
        widdgyy::css::properties::BackgroundRepeat::NoRepeat
    );
}

#[test]
fn mask_none_clears_the_layers() {
    let scene = resolve(".fade { mask-image: none; }");
    assert!(scene.roots[0].masks().is_empty());
}
