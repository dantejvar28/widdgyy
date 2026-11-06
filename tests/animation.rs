// SPDX-FileCopyrightText: 2026 Daniel
// SPDX-License-Identifier: GPL-3.0-or-later

use std::time::{Duration, Instant};

use widdgyy::animation;
use widdgyy::css::{Stylesheet, Viewport};
use widdgyy::scene::{Node, Scene};

#[test]
fn samples_rotation_and_opacity() {
    let css = Stylesheet::parse(
        r#"
        @keyframes spin {
            from { opacity: 0; transform: rotate(0deg); }
            to   { opacity: 1; transform: rotate(360deg); }
        }
        .spin { animation: spin 1s linear infinite; }
        "#,
    );
    let mut scene = Scene::new(Viewport::default());
    scene
        .roots
        .push(Node::text("text", vec!["spin".to_string()], "x"));
    scene.resolve_styles(&css);

    let start = Instant::now();
    let (sets, active) = animation::prepare_scene(&mut scene, &css, start);
    assert!(active);
    let set = sets.values().next().expect("compiled animation");

    let (half, active) = set.sample(start + Duration::from_millis(500));
    assert!(active);
    assert!((half.opacity.unwrap() - 0.5).abs() < 0.01);
    match &half.transform.unwrap()[0] {
        widdgyy::css::values::TransformOp::Rotate(angle) => {
            assert!((angle.0 - 180.0).abs() < 0.5, "{}", angle.0);
        }
        other => panic!("unexpected transform: {other:?}"),
    }

    let (end, _) = set.sample(start + Duration::from_millis(1000));
    match &end.transform.unwrap()[0] {
        widdgyy::css::values::TransformOp::Rotate(angle) => {
            assert!(angle.0.abs() < 0.5 || (angle.0 - 360.0).abs() < 0.5);
        }
        other => panic!("unexpected transform: {other:?}"),
    }
}

#[test]
fn samples_colors() {
    let css = Stylesheet::parse(
        r#"
        @keyframes fade {
            from { background-color: #000000; }
            to   { background-color: #ffffff; }
        }
        .fade { animation: fade 2s linear infinite; }
        "#,
    );
    let mut scene = Scene::new(Viewport::default());
    scene
        .roots
        .push(Node::text("text", vec!["fade".to_string()], "x"));
    scene.resolve_styles(&css);
    let start = Instant::now();
    let (sets, _) = animation::prepare_scene(&mut scene, &css, start);
    let set = sets.values().next().unwrap();
    let (half, _) = set.sample(start + Duration::from_secs(1));
    let color = half.background_color.unwrap();
    assert!((color.components[0] - 0.5).abs() < 0.05, "{color:?}");
}

#[test]
fn interpolates_text_shadow_glow() {
    let css = Stylesheet::parse(
        r#"
        @keyframes glow {
            from { text-shadow: 0 0 4px rgba(255,255,255,0.3), 0 0 10px rgba(150,80,255,0.2); }
            50% { text-shadow: 0 0 12px rgba(255,255,255,1), 0 0 30px rgba(150,80,255,0.8); }
            to { text-shadow: 0 0 4px rgba(255,255,255,0.3), 0 0 10px rgba(150,80,255,0.2); }
        }
        .glow { animation: glow 2s ease-in-out infinite; }
        "#,
    );
    let mut scene = Scene::new(Viewport::default());
    scene
        .roots
        .push(Node::text("text", vec!["glow".to_string()], "Glow"));
    scene.resolve_styles(&css);
    let start = Instant::now();
    let (sets, active) = animation::prepare_scene(&mut scene, &css, start);
    assert!(active);
    let set = sets.values().next().expect("compiled glow animation");
    let (mid, _) = set.sample(start + Duration::from_secs(1));
    let shadows = mid.text_shadows.expect("animated text shadows");
    assert_eq!(shadows.len(), 2);
    assert!(matches!(shadows[0].blur, widdgyy::css::Length::Px(value) if value > 4.0));
    assert!(shadows[0].color.components[3] > 0.3);
}

#[test]
fn interpolates_custom_properties() {
    let css = Stylesheet::parse(
        r#"
        @keyframes glow {
            from {
                --glow-strength: 0.5;
                --glow-radius: 8px;
                --glow-color: #000000;
            }
            to {
                --glow-strength: 1.5;
                --glow-radius: 20px;
                --glow-color: #ffffff;
            }
        }
        .glow {
            --glow: 1;
            animation: glow 2s linear infinite;
        }
        "#,
    );
    let mut scene = Scene::new(Viewport::default());
    scene
        .roots
        .push(Node::text("text", vec!["glow".to_string()], "Glow"));
    scene.resolve_styles(&css);

    let start = Instant::now();
    let (sets, active) = animation::prepare_scene(&mut scene, &css, start);
    assert!(active);
    let set = sets.values().next().expect("compiled glow animation");
    let (half, _) = set.sample(start + Duration::from_secs(1));
    let properties = half.custom_properties.expect("animated custom properties");
    assert!(!properties.contains_key("--glow"));
    assert_eq!(
        properties.get("--glow-strength").map(String::as_str),
        Some("1.0000")
    );
    assert_eq!(
        properties.get("--glow-radius").map(String::as_str),
        Some("14.00px")
    );
    assert_eq!(
        properties.get("--glow-color").map(String::as_str),
        Some("rgba(128, 128, 128, 1.000)")
    );
}
