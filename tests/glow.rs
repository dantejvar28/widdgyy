// SPDX-FileCopyrightText: 2026 Daniel
// SPDX-License-Identifier: GPL-3.0-or-later

use widdgyy::css::{Stylesheet, Viewport};
use widdgyy::scene::{Node, Scene};

#[test]
fn css_custom_properties_configure_text_glow() {
    let css = Stylesheet::parse(
        r#"
        * {
            --glow-radius: 9px;
            --glow-speed: 1.5;
        }
        .glow {
            --glow: 1;
            --glow-color: #ff6b6b;
            --glow-strength: 1.4;
            --glow-radius: 18px;
        }
        "#,
    );
    let mut scene = Scene::new(Viewport::default());
    scene
        .roots
        .push(Node::text("text", vec!["glow".into()], "Glow"));
    scene
        .roots
        .push(Node::text("text", vec!["plain".into()], "Plain"));
    scene.resolve_styles(&css);

    let glow = &scene.roots[0];
    assert!(glow.glow_enabled());
    assert!((glow.glow_radius() - 18.0).abs() < f32::EPSILON);
    assert!((glow.glow_strength() - 1.4).abs() < f32::EPSILON);
    assert!((glow.glow_speed() - 1.5).abs() < f32::EPSILON);
    assert!((glow.glow_color().components[0] - 1.0).abs() < 0.05);

    let plain = &scene.roots[1];
    assert!(!plain.glow_enabled());
}
