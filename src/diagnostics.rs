// SPDX-FileCopyrightText: 2026 Daniel
// SPDX-License-Identifier: GPL-3.0-or-later

//! `--check-css`: report which parts of a stylesheet are not supported.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::config::ConfigLoader;
use crate::css::{Preferences, Stylesheet, Viewport};
use crate::scene::{Node, Scene};

pub fn run(config_path: Option<PathBuf>) -> Result<()> {
    let path = config_path.unwrap_or_else(crate::user_config_path);
    let loader = ConfigLoader::new(&path);
    let viewport = Viewport::default();
    let scene = loader
        .load(viewport)
        .with_context(|| format!("loading configuration {}", path.display()))?;
    let css_path = loader.config_dir().join("styles.css");
    let css = std::fs::read_to_string(&css_path).unwrap_or_default();
    let stylesheet = Stylesheet::parse(&css);

    // Run the cascade so malformed selectors/variables are also reported.
    let mut scene = scene;
    let preferences = Preferences::detect();
    scene.resolve_styles_with_preferences(&stylesheet, preferences);

    println!("widdgyy CSS check");
    println!("  config:    {}", path.display());
    println!("  styles:    {}", css_path.display());
    println!("  stickers:  {}", count_nodes(&scene));
    println!("  rules:     {}", stylesheet.rules.len());
    println!("  keyframes: {}", stylesheet.keyframes.len());
    println!("  fonts:     {}", stylesheet.font_faces.len());
    println!(
        "  prefs:     color-scheme={:?}, reduced-motion={}, hover={:?}, pointer={:?}, display-mode={:?}",
        preferences.color_scheme,
        preferences.reduced_motion,
        preferences.hover,
        preferences.pointer,
        preferences.display_mode
    );
    println!();

    if stylesheet.errors.is_empty() {
        println!("CSS errors: none");
    } else {
        println!("CSS errors:");
        for error in &stylesheet.errors {
            println!("  - {error}");
        }
    }

    if stylesheet.unsupported_properties.is_empty() {
        println!("\nUnsupported properties: none");
    } else {
        println!("\nUnsupported properties:");
        for (property, count) in &stylesheet.unsupported_properties {
            println!("  - {property} ({count} declaration(s))");
        }
    }

    if stylesheet.unsupported_at_rules.is_empty() {
        println!("\nUnsupported at-rules: none");
    } else {
        println!("\nUnsupported at-rules:");
        for at_rule in &stylesheet.unsupported_at_rules {
            println!("  - {at_rule}");
        }
    }

    Ok(())
}

fn count_nodes(scene: &Scene) -> usize {
    fn count(node: &Node) -> usize {
        1 + node.children.iter().map(count).sum::<usize>()
    }
    scene.roots.iter().map(count).sum()
}

#[allow(dead_code)]
fn _path_marker(_: &Path) {}
