// SPDX-FileCopyrightText: 2026 Daniel
// SPDX-License-Identifier: GPL-3.0-or-later

use widdgyy::css::values::Length;
use widdgyy::css::{Color, Viewport};
use widdgyy::scene::{Node, Scene};

#[test]
fn resolves_text_shadow_metrics() {
    let mut node = Node::text("text", vec!["shadow".into()], "Shadow");
    let shadow = widdgyy::css::values::Shadow {
        offset_x: Length::px(2.0),
        offset_y: Length::px(-3.0),
        blur: Length::px(18.0),
        spread: Length::px(0.0),
        color: Color::new([0.0, 1.0, 1.0, 0.8]),
        inset: false,
    };
    node.style.text.shadows = vec![shadow.clone()];

    let (dx, dy, blur) = node.shadow_metrics(&shadow);
    assert!((dx - 2.0).abs() < f32::EPSILON);
    assert!((dy + 3.0).abs() < f32::EPSILON);
    assert!((blur - 18.0).abs() < f32::EPSILON);
}

#[test]
fn scene_without_css_has_no_shadow_layers() {
    let mut scene = Scene::new(Viewport::default());
    scene
        .roots
        .push(Node::text("text", vec!["plain".into()], "No shadow"));
    assert!(scene.roots[0].text_shadows().is_empty());
}

#[test]
fn collects_shadows_in_paint_order() {
    let mut scene = Scene::new(Viewport::default());
    let mut node = Node::text("text", vec!["shadow".into()], "Shadow");
    let red = Color::new([1.0, 0.0, 0.0, 1.0]);
    let blue = Color::new([0.0, 0.0, 1.0, 1.0]);
    node.style.text.shadows = vec![
        widdgyy::css::values::Shadow {
            offset_x: Length::px(1.0),
            offset_y: Length::px(0.0),
            blur: Length::px(4.0),
            spread: Length::px(0.0),
            color: red,
            inset: false,
        },
        widdgyy::css::values::Shadow {
            offset_x: Length::px(0.0),
            offset_y: Length::px(2.0),
            blur: Length::px(10.0),
            spread: Length::px(0.0),
            color: blue,
            inset: false,
        },
    ];
    scene.roots.push(node);

    let draws = widdgyy::render::painter::collect_shadow_draws(&scene);
    assert_eq!(draws.len(), 2);
    // CSS paints the first shadow on top, so it is the last item.
    assert!((draws[0].sigma - 5.0).abs() < f32::EPSILON);
    assert_eq!(draws[0].color, blue.components);
    assert!((draws[1].sigma - 2.0).abs() < f32::EPSILON);
    assert_eq!(draws[1].offset, [1.0, 0.0]);
}
