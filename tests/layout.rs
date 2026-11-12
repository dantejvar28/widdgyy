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

#[test]
fn positions_absolute_and_relative_children() {
    let css = Stylesheet::parse(
        r#"
        .root { width: 500px; height: 300px; padding: 20px; position: relative; }
        .abs { position: absolute; left: 10px; top: 15px; width: 40px; height: 30px; }
        .rel { position: relative; left: 5px; top: 7px; width: 20px; height: 10px; }
        "#,
    );
    let mut scene = Scene::new(Viewport {
        width: 500.0,
        height: 300.0,
        scale_factor: 1.0,
    });
    let mut root = Node::container("vbox", vec!["root".to_string()]);
    let mut abs = Node::text("text", vec!["abs".to_string()], "a");
    abs.anchor = Anchor::TopLeft;
    let mut rel = Node::text("text", vec!["rel".to_string()], "r");
    rel.anchor = Anchor::TopLeft;
    root.children.push(abs);
    root.children.push(rel);
    scene.roots.push(root);
    let mut text = TextEngine::new();
    scene.resolve_styles(&css);
    scene.layout(&mut LayoutEngine::new(&mut text));
    let root = &scene.roots[0];
    let abs = &root.children[0];
    let rel = &root.children[1];
    assert!((abs.layout.x - 30.0).abs() < 0.5, "{}", abs.layout.x);
    assert!((abs.layout.y - 35.0).abs() < 0.5, "{}", abs.layout.y);
    assert!(rel.layout.x >= 20.0);
    assert!(rel.layout.y >= 20.0);
}

#[test]
fn flex_order_changes_layout_order() {
    let css = Stylesheet::parse(
        r#"
        .root { display: flex; width: 300px; height: 50px; }
        .a { order: 2; width: 30px; height: 20px; }
        .b { order: 1; width: 30px; height: 20px; }
        .c { order: 3; width: 30px; height: 20px; }
        "#,
    );
    let mut scene = Scene::new(Viewport {
        width: 300.0,
        height: 50.0,
        scale_factor: 1.0,
    });
    let mut root = Node::container("hbox", vec!["root".to_string()]);
    for class in ["a", "b", "c"] {
        let mut node = Node::text("text", vec![class.to_string()], class);
        node.anchor = Anchor::TopLeft;
        root.children.push(node);
    }
    scene.roots.push(root);
    let mut text = TextEngine::new();
    scene.resolve_styles(&css);
    scene.layout(&mut LayoutEngine::new(&mut text));
    let children = &scene.roots[0].children;
    assert!(children[1].layout.x < children[0].layout.x);
    assert!(children[0].layout.x < children[2].layout.x);
}

#[test]
fn flex_wrap_places_items_on_new_lines() {
    let css = Stylesheet::parse(
        r#"
        .root {
            display: flex;
            flex-wrap: wrap;
            width: 200px;
            height: 200px;
        }
        .item { width: 80px; height: 30px; }
        "#,
    );
    let mut scene = Scene::new(Viewport::default());
    let mut root = Node::container("hbox", vec!["root".to_string()]);
    for _ in 0..3 {
        root.children
            .push(Node::container("text", vec!["item".to_string()]));
    }
    scene.roots.push(root);
    let mut text = TextEngine::new();
    scene.resolve_styles(&css);
    scene.layout(&mut LayoutEngine::new(&mut text));

    let root = &scene.roots[0];
    assert_eq!(root.children[0].layout.x, 0.0);
    assert_eq!(root.children[1].layout.x, 80.0);
    assert_eq!(root.children[2].layout.x, 0.0);
    assert_eq!(root.children[2].layout.y, 30.0);
}

#[test]
fn grid_places_explicit_and_auto_items() {
    let css = Stylesheet::parse(
        r#"
        .root {
            display: grid;
            width: 200px;
            height: 200px;
            grid-template-columns: 100px 100px;
            grid-template-rows: 50px 50px;
        }
        .item { width: 20px; height: 20px; }
        .second-column { grid-column: 2; grid-row: 1; }
        .second-row { grid-column: 1; grid-row: 2; }
        "#,
    );
    let mut scene = Scene::new(Viewport::default());
    let mut root = Node::container("grid", vec!["root".to_string()]);
    for class in ["item second-column", "item second-row", "item"] {
        let classes = class.split_whitespace().map(str::to_string).collect();
        root.children.push(Node::container("text", classes));
    }
    scene.roots.push(root);
    let mut text = TextEngine::new();
    scene.resolve_styles(&css);
    scene.layout(&mut LayoutEngine::new(&mut text));

    let root = &scene.roots[0];
    println!("errors={:?}", css.errors);
    println!("rules={}", css.rules.len());
    println!(
        "display={:?} columns={:?}",
        root.style.display, root.style.grid.columns
    );
    for (index, child) in root.children.iter().enumerate() {
        println!(
            "child {index} grid={:?} layout={:?}",
            child.style.grid, child.layout
        );
    }
    assert_eq!(root.children[0].layout.x, 100.0);
    assert_eq!(root.children[0].layout.y, 0.0);
    assert_eq!(root.children[1].layout.x, 0.0);
    assert_eq!(root.children[1].layout.y, 50.0);
    assert_eq!(root.children[2].layout.x, 0.0);
    assert_eq!(root.children[2].layout.y, 0.0);
}

#[test]
fn flex_grow_distributes_free_space() {
    let css = Stylesheet::parse(
        r#"
        .root { display: flex; width: 300px; height: 100px; }
        .a { width: 50px; height: 20px; flex-grow: 1; }
        .b { width: 50px; height: 20px; flex-grow: 2; }
        "#,
    );
    let mut scene = Scene::new(Viewport::default());
    let mut root = Node::container("hbox", vec!["root".to_string()]);
    for class in ["a", "b"] {
        root.children
            .push(Node::container("text", vec![class.to_string()]));
    }
    scene.roots.push(root);
    let mut text = TextEngine::new();
    scene.resolve_styles(&css);
    scene.layout(&mut LayoutEngine::new(&mut text));

    let root = &scene.roots[0];
    assert!((root.children[0].layout.width - 116.67).abs() < 0.5);
    assert!((root.children[1].layout.width - 183.33).abs() < 0.5);
}

#[test]
fn flex_shrink_removes_overflow() {
    let css = Stylesheet::parse(
        r#"
        .root { display: flex; width: 100px; height: 100px; }
        .a { width: 80px; height: 20px; flex-shrink: 1; }
        .b { width: 80px; height: 20px; flex-shrink: 1; }
        "#,
    );
    let mut scene = Scene::new(Viewport::default());
    let mut root = Node::container("hbox", vec!["root".to_string()]);
    for class in ["a", "b"] {
        root.children
            .push(Node::container("text", vec![class.to_string()]));
    }
    scene.roots.push(root);
    let mut text = TextEngine::new();
    scene.resolve_styles(&css);
    scene.layout(&mut LayoutEngine::new(&mut text));

    let root = &scene.roots[0];
    println!(
        "widths {:?} {:?}",
        root.children[0].layout, root.children[1].layout
    );
    assert!((root.children[0].layout.width - 50.0).abs() < 0.5);
    assert!((root.children[1].layout.width - 50.0).abs() < 0.5);
}

#[test]
fn grid_items_stretch_to_their_tracks() {
    let css = Stylesheet::parse(
        r#"
        .root {
            display: grid;
            width: 200px;
            height: 100px;
            grid-template-columns: 100px 100px;
            grid-template-rows: 50px;
        }
        .item { }
        "#,
    );
    let mut scene = Scene::new(Viewport::default());
    let mut root = Node::container("grid", vec!["root".to_string()]);
    root.children
        .push(Node::container("text", vec!["item".to_string()]));
    scene.roots.push(root);
    let mut text = TextEngine::new();
    scene.resolve_styles(&css);
    scene.layout(&mut LayoutEngine::new(&mut text));

    let item = &scene.roots[0].children[0];
    assert_eq!(item.layout.width, 100.0);
    assert_eq!(item.layout.height, 50.0);
}

#[test]
fn block_stacks_children_and_collapses_sibling_margins() {
    let css = Stylesheet::parse(
        r#"
        .root { display: block; width: 300px; }
        .a { display: block; width: 50px; height: 20px; margin-bottom: 24px; }
        .b { display: block; width: 50px; height: 20px; margin-top: 10px; }
        "#,
    );
    let mut root = Node::container("root", vec!["root".to_string()]);
    root.children
        .push(Node::container("a", vec!["a".to_string()]));
    root.children
        .push(Node::container("b", vec!["b".to_string()]));
    let mut scene = Scene::new(Viewport::default());
    scene.roots.push(root);
    let mut text = TextEngine::new();
    scene.resolve_styles(&css);
    scene.layout(&mut LayoutEngine::new(&mut text));

    let root = &scene.roots[0];
    assert_eq!(root.children[0].layout.y, 0.0);
    // 20px tall first box plus the collapsed margin max(24, 10).
    assert_eq!(root.children[1].layout.y, 44.0);
    assert_eq!(root.layout.content_height, 64.0);
}

#[test]
fn block_collapses_negative_margins() {
    let css = Stylesheet::parse(
        r#"
        .root { display: block; width: 300px; }
        .a { display: block; height: 20px; margin-bottom: -6px; }
        .b { display: block; height: 20px; margin-top: 10px; }
        .c { display: block; height: 20px; margin-top: -8px; }
        "#,
    );
    let mut root = Node::container("root", vec!["root".to_string()]);
    for class in ["a", "b", "c"] {
        root.children
            .push(Node::container(class, vec![class.to_string()]));
    }
    let mut scene = Scene::new(Viewport::default());
    scene.roots.push(root);
    let mut text = TextEngine::new();
    scene.resolve_styles(&css);
    scene.layout(&mut LayoutEngine::new(&mut text));

    let root = &scene.roots[0];
    assert_eq!(root.children[0].layout.y, 0.0);
    // max positive (10) + min negative (-6) = 4.
    assert_eq!(root.children[1].layout.y, 24.0);
    // -6 and -8 collapse to -8.
    assert_eq!(root.children[2].layout.y, 36.0);
}

#[test]
fn block_collapses_parent_child_margins() {
    let css = Stylesheet::parse(
        r#"
        .root { display: block; width: 300px; }
        .a { display: block; height: 20px; margin-top: 12px; }
        .b { display: block; height: 20px; margin-bottom: 18px; }
        "#,
    );
    let mut root = Node::container("root", vec!["root".to_string()]);
    root.children
        .push(Node::container("a", vec!["a".to_string()]));
    root.children
        .push(Node::container("b", vec!["b".to_string()]));
    let mut scene = Scene::new(Viewport::default());
    scene.roots.push(root);
    let mut text = TextEngine::new();
    scene.resolve_styles(&css);
    scene.layout(&mut LayoutEngine::new(&mut text));

    let root = &scene.roots[0];
    // The first child's top margin escapes the parent, and the last child's
    // bottom margin does too.
    assert_eq!(root.layout.margin.top, 12.0);
    assert_eq!(root.layout.margin.bottom, 18.0);
    assert_eq!(root.children[0].layout.y, 0.0);
    assert_eq!(root.layout.content_height, 40.0);
}

#[test]
fn block_level_children_fill_but_text_keeps_its_intrinsic_width() {
    let css = Stylesheet::parse(
        r#"
        .root { display: block; width: 300px; }
        .card { display: block; height: 20px; }
        .label { font-family: "DejaVu Sans"; font-size: 16px; }
        "#,
    );
    let mut root = Node::container("root", vec!["root".to_string()]);
    root.children
        .push(Node::container("card", vec!["card".to_string()]));
    root.children
        .push(Node::text("text", vec!["label".to_string()], "Hi"));
    let mut scene = Scene::new(Viewport::default());
    scene.roots.push(root);
    let mut text = TextEngine::new();
    scene.resolve_styles(&css);
    scene.layout(&mut LayoutEngine::new(&mut text));

    let root = &scene.roots[0];
    assert_eq!(root.children[0].layout.width, 300.0);
    assert!(root.children[1].layout.width < 100.0);
}

#[test]
fn flex_basis_is_the_flex_base_and_overrides_width() {
    let css = Stylesheet::parse(
        r#"
        .root { display: flex; width: 300px; height: 50px; }
        .a { flex: 0 0 50px; width: 200px; height: 10px; }
        .b { flex: 1 0 50px; width: 10px; height: 10px; }
        "#,
    );
    let mut root = Node::container("root", vec!["root".to_string()]);
    for class in ["a", "b"] {
        root.children
            .push(Node::container(class, vec![class.to_string()]));
    }
    let mut scene = Scene::new(Viewport::default());
    scene.roots.push(root);
    let mut text = TextEngine::new();
    scene.resolve_styles(&css);
    scene.layout(&mut LayoutEngine::new(&mut text));

    let root = &scene.roots[0];
    assert!((root.children[0].layout.width - 50.0).abs() < 0.5);
    assert!((root.children[1].layout.width - 250.0).abs() < 0.5);
}

#[test]
fn flex_shrink_uses_the_flex_basis() {
    let css = Stylesheet::parse(
        r#"
        .root { display: flex; width: 300px; height: 50px; }
        .a { flex: 0 1 200px; width: 10px; height: 10px; }
        .b { flex: 0 3 200px; width: 10px; height: 10px; }
        "#,
    );
    let mut root = Node::container("root", vec!["root".to_string()]);
    for class in ["a", "b"] {
        root.children
            .push(Node::container(class, vec![class.to_string()]));
    }
    let mut scene = Scene::new(Viewport::default());
    scene.roots.push(root);
    let mut text = TextEngine::new();
    scene.resolve_styles(&css);
    scene.layout(&mut LayoutEngine::new(&mut text));

    let root = &scene.roots[0];
    assert!((root.children[0].layout.width - 175.0).abs() < 0.5);
    assert!((root.children[1].layout.width - 125.0).abs() < 0.5);
}

#[test]
fn text_supports_min_content_max_content_and_fit_content() {
    let css = Stylesheet::parse(
        r#"
        .root { display: flex; height: 200px; }
        .min { width: min-content; font-family: "DejaVu Sans"; font-size: 20px; }
        .max { width: max-content; font-family: "DejaVu Sans"; font-size: 20px; }
        .fit { width: fit-content(100px); font-family: "DejaVu Sans"; font-size: 20px; }
        "#,
    );
    let mut root = Node::container("root", vec!["root".to_string()]);
    for (class, text) in [
        ("min", "wonderful little words"),
        ("max", "wonderful little words"),
        ("fit", "wonderful little words"),
    ] {
        root.children
            .push(Node::text("text", vec![class.to_string()], text));
    }
    let mut scene = Scene::new(Viewport::default());
    scene.roots.push(root);
    let mut text = TextEngine::new();
    scene.resolve_styles(&css);
    scene.layout(&mut LayoutEngine::new(&mut text));

    let children = &scene.roots[0].children;
    let min = children[0].layout.width;
    let max = children[1].layout.width;
    let fit = children[2].layout.width;
    assert!(min > 0.0 && min < max, "min={min} max={max}");
    assert!(fit <= 100.0 && fit >= min, "fit={fit} min={min}");
}

#[test]
fn grid_template_areas_place_items_and_span_tracks() {
    let css = Stylesheet::parse(
        r#"
        .root {
            display: grid;
            width: 300px;
            height: 100px;
            grid-template-columns: repeat(3, 100px);
            grid-template-rows: 50px 50px;
            grid-template-areas:
                "header header side"
                "main   main   side";
        }
        .header { grid-area: header; }
        .side { grid-area: side; }
        .main { grid-area: main; }
        "#,
    );
    let mut root = Node::container("root", vec!["root".to_string()]);
    for class in ["header", "side", "main"] {
        root.children
            .push(Node::container(class, vec![class.to_string()]));
    }
    let mut scene = Scene::new(Viewport::default());
    scene.roots.push(root);
    let mut text = TextEngine::new();
    scene.resolve_styles(&css);
    scene.layout(&mut LayoutEngine::new(&mut text));

    let children = &scene.roots[0].children;
    assert_eq!((children[0].layout.x, children[0].layout.y), (0.0, 0.0));
    assert_eq!(
        (children[0].layout.width, children[0].layout.height),
        (200.0, 50.0)
    );
    assert_eq!((children[1].layout.x, children[1].layout.y), (200.0, 0.0));
    assert_eq!(
        (children[1].layout.width, children[1].layout.height),
        (100.0, 100.0)
    );
    assert_eq!((children[2].layout.x, children[2].layout.y), (0.0, 50.0));
    assert_eq!(
        (children[2].layout.width, children[2].layout.height),
        (200.0, 50.0)
    );
}

#[test]
fn grid_area_shorthand_uses_line_numbers() {
    let css = Stylesheet::parse(
        r#"
        .root {
            display: grid;
            width: 300px;
            height: 100px;
            grid-template-columns: repeat(3, 100px);
            grid-template-rows: 50px 50px;
        }
        .a { grid-area: 1 / 2 / 2 / 4; }
        "#,
    );
    let mut root = Node::container("root", vec!["root".to_string()]);
    root.children
        .push(Node::container("a", vec!["a".to_string()]));
    let mut scene = Scene::new(Viewport::default());
    scene.roots.push(root);
    let mut text = TextEngine::new();
    scene.resolve_styles(&css);
    scene.layout(&mut LayoutEngine::new(&mut text));

    let item = &scene.roots[0].children[0];
    assert_eq!((item.layout.x, item.layout.y), (100.0, 0.0));
    assert_eq!((item.layout.width, item.layout.height), (200.0, 50.0));
}

#[test]
fn grid_auto_tracks_size_implicit_tracks() {
    let css = Stylesheet::parse(
        r#"
        .root {
            display: grid;
            width: 300px;
            grid-template-columns: 100px;
            grid-auto-columns: 50px;
            grid-auto-rows: 40px;
        }
        .a { grid-column: 2; grid-row: 1; width: 20px; height: 20px; }
        .b { width: 20px; height: 20px; }
        "#,
    );
    let mut root = Node::container("root", vec!["root".to_string()]);
    for class in ["a", "b"] {
        root.children
            .push(Node::container(class, vec![class.to_string()]));
    }
    let mut scene = Scene::new(Viewport::default());
    scene.roots.push(root);
    let mut text = TextEngine::new();
    scene.resolve_styles(&css);
    scene.layout(&mut LayoutEngine::new(&mut text));

    let children = &scene.roots[0].children;
    assert_eq!((children[0].layout.x, children[0].layout.y), (100.0, 0.0));
    assert_eq!((children[1].layout.x, children[1].layout.y), (0.0, 0.0));
    assert_eq!(scene.roots[0].layout.content_height, 40.0);
}

#[test]
fn grid_dense_flow_backfills_holes() {
    fn positions(flow: &str) -> Vec<(f32, f32, f32)> {
        let css = Stylesheet::parse(&format!(
            r#"
            .root {{
                display: grid;
                width: 300px;
                grid-template-columns: repeat(3, 100px);
                grid-auto-rows: 20px;
                grid-auto-flow: {flow};
            }}
            .fixed {{ grid-column: 2; grid-row: 1; width: 20px; height: 20px; }}
            .wide {{ grid-column: span 2; width: 20px; height: 20px; }}
            .small {{ width: 20px; height: 20px; }}
            "#
        ));
        let mut root = Node::container("root", vec!["root".to_string()]);
        for class in ["fixed", "wide", "small"] {
            root.children
                .push(Node::container(class, vec![class.to_string()]));
        }
        let mut scene = Scene::new(Viewport::default());
        scene.roots.push(root);
        let mut text = TextEngine::new();
        scene.resolve_styles(&css);
        scene.layout(&mut LayoutEngine::new(&mut text));
        scene.roots[0]
            .children
            .iter()
            .map(|child| (child.layout.x, child.layout.y, child.layout.width))
            .collect()
    }

    let sparse = positions("row");
    let dense = positions("row dense");
    assert_eq!(sparse[2], (200.0, 20.0, 20.0));
    assert_eq!(dense[2], (0.0, 0.0, 20.0));
}

#[test]
fn grid_justify_items_and_place_content_align_items() {
    let css = Stylesheet::parse(
        r#"
        .root {
            display: grid;
            width: 300px;
            height: 200px;
            grid-template-columns: 100px;
            grid-template-rows: 50px;
            grid-auto-rows: 10px;
            justify-items: center;
            align-items: end;
            place-content: center;
        }
        .a { width: 20px; height: 10px; }
        .b { width: 20px; height: 10px; justify-self: end; align-self: start; }
        "#,
    );
    let mut root = Node::container("root", vec!["root".to_string()]);
    for class in ["a", "b"] {
        root.children
            .push(Node::container(class, vec![class.to_string()]));
    }
    let mut scene = Scene::new(Viewport::default());
    scene.roots.push(root);
    let mut text = TextEngine::new();
    scene.resolve_styles(&css);
    scene.layout(&mut LayoutEngine::new(&mut text));

    let children = &scene.roots[0].children;
    assert_eq!(children[0].layout.x, 100.0 + 40.0);
    assert_eq!(children[0].layout.y, 70.0 + 40.0);
    assert_eq!(children[1].layout.x, 100.0 + 80.0);
    assert_eq!(children[1].layout.y, 120.0);
}

#[test]
fn grid_max_content_track_sizes_to_the_text() {
    let css = Stylesheet::parse(
        r#"
        .root {
            display: grid;
            width: 400px;
            grid-template-columns: max-content 1fr;
            font-family: "DejaVu Sans";
            font-size: 20px;
        }
        .label { }
        .body { }
        "#,
    );
    let mut root = Node::container("root", vec!["root".to_string()]);
    root.children.push(Node::text(
        "text",
        vec!["label".to_string()],
        "a fairly long label",
    ));
    root.children
        .push(Node::text("text", vec!["body".to_string()], "body"));
    let mut scene = Scene::new(Viewport::default());
    scene.roots.push(root);
    let mut text = TextEngine::new();
    scene.resolve_styles(&css);
    scene.layout(&mut LayoutEngine::new(&mut text));

    let children = &scene.roots[0].children;
    assert!(children[0].layout.width > 100.0);
    assert!((children[1].layout.x - children[0].layout.width).abs() < 0.5);
    assert!((children[1].layout.width + children[0].layout.width - 400.0).abs() < 0.5);
}

#[test]
fn grid_column_flow_places_items_down_then_across() {
    let css = Stylesheet::parse(
        r#"
        .root {
            display: grid;
            width: 300px;
            grid-template-rows: 50px 50px;
            grid-auto-columns: 100px;
            grid-auto-flow: column;
        }
        .a, .b, .c { width: 20px; height: 20px; }
        "#,
    );
    let mut root = Node::container("root", vec!["root".to_string()]);
    for class in ["a", "b", "c"] {
        root.children
            .push(Node::container(class, vec![class.to_string()]));
    }
    let mut scene = Scene::new(Viewport::default());
    scene.roots.push(root);
    let mut text = TextEngine::new();
    scene.resolve_styles(&css);
    scene.layout(&mut LayoutEngine::new(&mut text));

    let children = &scene.roots[0].children;
    assert_eq!((children[0].layout.x, children[0].layout.y), (0.0, 0.0));
    assert_eq!((children[1].layout.x, children[1].layout.y), (0.0, 50.0));
    assert_eq!((children[2].layout.x, children[2].layout.y), (100.0, 0.0));
}

#[test]
fn grid_fr_tracks_split_the_free_space_with_gaps() {
    let css = Stylesheet::parse(
        r#"
        .root {
            display: grid;
            width: 300px;
            height: 20px;
            grid-template-columns: 1fr 1fr;
            column-gap: 20px;
        }
        .a, .b { height: 20px; }
        "#,
    );
    let mut root = Node::container("root", vec!["root".to_string()]);
    for class in ["a", "b"] {
        root.children
            .push(Node::container(class, vec![class.to_string()]));
    }
    let mut scene = Scene::new(Viewport::default());
    scene.roots.push(root);
    let mut text = TextEngine::new();
    scene.resolve_styles(&css);
    scene.layout(&mut LayoutEngine::new(&mut text));

    let children = &scene.roots[0].children;
    assert!((children[0].layout.width - 140.0).abs() < 0.5);
    assert!((children[1].layout.width - 140.0).abs() < 0.5);
    assert!((children[1].layout.x - 160.0).abs() < 0.5);
}

#[test]
fn place_items_and_place_self_override_alignment() {
    let css = Stylesheet::parse(
        r#"
        .root {
            display: grid;
            width: 300px;
            height: 100px;
            grid-template-columns: 100px;
            grid-template-rows: 50px;
            place-items: center;
        }
        .a { width: 20px; height: 10px; }
        .b { width: 20px; height: 10px; place-self: end start; }
        "#,
    );
    let mut root = Node::container("root", vec!["root".to_string()]);
    for class in ["a", "b"] {
        root.children
            .push(Node::container(class, vec![class.to_string()]));
    }
    let mut scene = Scene::new(Viewport::default());
    scene.roots.push(root);
    let mut text = TextEngine::new();
    scene.resolve_styles(&css);
    scene.layout(&mut LayoutEngine::new(&mut text));

    let children = &scene.roots[0].children;
    assert_eq!(children[0].layout.x, 40.0);
    assert_eq!(children[0].layout.y, 20.0);
    assert_eq!(children[1].layout.x, 0.0);
    assert_eq!(children[1].layout.y, 50.0);
}

#[test]
fn block_auto_horizontal_margins_center_the_box() {
    let css = Stylesheet::parse(
        r#"
        .root { display: block; width: 300px; }
        .card { display: block; width: 100px; height: 20px; margin: 0 auto; }
        "#,
    );
    let mut root = Node::container("root", vec!["root".to_string()]);
    root.children
        .push(Node::container("card", vec!["card".to_string()]));
    let mut scene = Scene::new(Viewport::default());
    scene.roots.push(root);
    let mut text = TextEngine::new();
    scene.resolve_styles(&css);
    scene.layout(&mut LayoutEngine::new(&mut text));

    let card = &scene.roots[0].children[0];
    assert_eq!(card.layout.x, 100.0);
}

#[test]
fn auto_width_flex_container_uses_the_flex_bases() {
    let css = Stylesheet::parse(
        r#"
        .root { display: flex; }
        .a { flex: 0 0 120px; height: 10px; }
        .b { flex: 0 0 80px; height: 10px; }
        "#,
    );
    let mut root = Node::container("root", vec!["root".to_string()]);
    for class in ["a", "b"] {
        root.children
            .push(Node::container(class, vec![class.to_string()]));
    }
    let mut scene = Scene::new(Viewport::default());
    scene.roots.push(root);
    let mut text = TextEngine::new();
    scene.resolve_styles(&css);
    scene.layout(&mut LayoutEngine::new(&mut text));

    let root = &scene.roots[0];
    assert_eq!(root.layout.width, 200.0);
    assert_eq!(root.children[0].layout.width, 120.0);
    assert_eq!(root.children[1].layout.width, 80.0);
}

#[test]
fn flex_column_auto_min_height_uses_the_natural_text_height() {
    let css = Stylesheet::parse(
        r#"
        .root { display: flex; flex-direction: column; width: 600px; }
        .label { font-family: "DejaVu Sans"; font-size: 20px; line-height: 24px; }
        "#,
    );
    let mut root = Node::container("root", vec!["root".to_string()]);
    root.children.push(Node::text(
        "text",
        vec!["label".to_string()],
        "one two three four five six seven eight",
    ));
    let mut scene = Scene::new(Viewport::default());
    scene.roots.push(root);
    let mut text = TextEngine::new();
    scene.resolve_styles(&css);
    scene.layout(&mut LayoutEngine::new(&mut text));

    let root = &scene.roots[0];
    assert!(
        root.layout.height < 60.0,
        "auto minimum height used the one-word-per-line text height: {}",
        root.layout.height
    );
}
