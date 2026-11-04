// SPDX-FileCopyrightText: 2026 Daniel
// SPDX-License-Identifier: GPL-3.0-or-later

use vello::peniko::Color;
use widdgyy::css::Stylesheet;
use widdgyy::css::properties::Declarations;
use widdgyy::css::selector::{Element, Selector};
use widdgyy::css::values::{Axis, Length, ResolveContext};

#[test]
fn resolves_min_max_clamp() {
    let ctx = ResolveContext {
        viewport_width: 1000.0,
        viewport_height: 1000.0,
        ..ResolveContext::default()
    };
    assert_eq!(
        Length::parse("clamp(10px, 5vw, 100px)")
            .unwrap()
            .resolve(&ctx, Axis::Horizontal),
        50.0
    );
    assert_eq!(
        Length::parse("max(20px, 5vw)")
            .unwrap()
            .resolve(&ctx, Axis::Horizontal),
        50.0
    );
    assert_eq!(
        Length::parse("min(20px, 5vw)")
            .unwrap()
            .resolve(&ctx, Axis::Horizontal),
        20.0
    );
}

#[test]
fn maps_logical_properties() {
    let color = Color::new([1.0, 1.0, 1.0, 1.0]);
    let mut declarations = Declarations::default();
    assert!(declarations.apply("margin-inline-start", "12px", color));
    assert_eq!(declarations.margin.as_ref().unwrap().left, Length::Px(12.0));
    assert!(declarations.apply("padding-block", "4px 8px", color));
    let padding = declarations.padding.as_ref().unwrap();
    assert_eq!(padding.top, Length::Px(4.0));
    assert_eq!(padding.bottom, Length::Px(8.0));
    assert!(declarations.apply("inline-size", "50%", color));
    assert!(matches!(
        declarations.width,
        Some(widdgyy::css::Size::Length(Length::Percent(50.0)))
    ));
}

#[test]
fn evaluates_supports() {
    let supported = Stylesheet::parse(r#"@supports (display: grid) { .a { color: red; } }"#);
    assert_eq!(supported.rules.len(), 1);
    let unsupported = Stylesheet::parse(r#"@supports (display: banana) { .a { color: red; } }"#);
    assert_eq!(unsupported.rules.len(), 0);
}

#[test]
fn matches_attribute_operators() {
    let classes = Vec::new();
    let attributes = vec![
        ("data-kind".to_string(), "primary-large".to_string()),
        ("role".to_string(), "value".to_string()),
    ];
    let element = Element::new("text", None, &classes, 0, 1).with_attributes(&attributes);
    for selector in [
        "[role]",
        "[role=\"value\"]",
        "[data-kind^=\"primary\"]",
        "[data-kind$=\"large\"]",
        "[data-kind*=\"-lar\"]",
        "[data-kind|=\"primary\"]",
    ] {
        assert!(
            Selector::parse(selector).unwrap().matches(&element, &[]),
            "{selector}"
        );
    }
    assert!(
        !Selector::parse("[role=\"other\"]")
            .unwrap()
            .matches(&element, &[])
    );
}

#[test]
fn parses_individual_transforms_and_filters() {
    let color = Color::new([1.0, 1.0, 1.0, 1.0]);
    let mut declarations = Declarations::default();
    assert!(declarations.apply("translate", "10px 20px", color));
    assert!(declarations.apply("rotate", "45deg", color));
    assert!(declarations.apply("scale", "2 0.5", color));
    assert!(declarations.translate.is_some());
    assert!(declarations.rotate.is_some());
    assert_eq!(declarations.scale, Some((2.0, 0.5)));

    assert!(declarations.apply("filter", "brightness(50%)", color));
    let filtered = widdgyy::css::properties::effects::apply_color_filters(
        Color::new([1.0, 0.5, 0.25, 1.0]),
        &declarations.filter.unwrap(),
    );
    assert!((filtered.components[0] - 0.5).abs() < 0.01);
    assert!((filtered.components[1] - 0.25).abs() < 0.01);
}
