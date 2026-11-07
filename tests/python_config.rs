// SPDX-FileCopyrightText: 2026 Daniel
// SPDX-License-Identifier: GPL-3.0-or-later

//! Integration test for the Python configuration DSL.

use std::path::PathBuf;
use std::process::Command;

use widdgyy::config::ConfigLoader;
use widdgyy::css::Viewport;
use widdgyy::scene::NodeKind;

fn python_available() -> bool {
    Command::new("python3")
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success())
}

fn write_config(name: &str, source: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("widdgyy-python-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(name);
    std::fs::write(&path, source).unwrap();
    path
}

#[test]
fn loads_python_configuration() {
    if !python_available() {
        eprintln!("python3 not available; skipping");
        return;
    }
    let path = write_config(
        "config.py",
        r#"
from widdgyy import ApiText, Clock, HBox, Video

WEATHER = "https://example.com/weather"

stickers = [
    Video("assets/wall.mp4", cls="background", z=-1),
    HBox(
        Clock("%H:%M", cls="clock"),
        anchor="center",
        offset=(0, -100),
    ),
    ApiText(WEATHER, template="{t}", fields={"t": "a.b"}, cls="weather"),
]
"#,
    );
    let scene = ConfigLoader::new(&path)
        .load(Viewport::default())
        .expect("loading python config");
    assert_eq!(scene.roots.len(), 3);
    assert_eq!(scene.roots[0].classes, vec!["background"]);
    assert!(matches!(scene.roots[0].kind, NodeKind::Media(_)));
    assert!(scene.roots[1].classes.is_empty());
    assert_eq!(scene.roots[1].children[0].classes, vec!["clock"]);
    assert_eq!(scene.roots[2].classes, vec!["weather"]);
}

#[test]
fn migrated_python_round_trips() {
    if !python_available() {
        eprintln!("python3 not available; skipping");
        return;
    }
    let value: serde_json::Value = json5::from_str(
        r#"{
            stickers: [
                { type: "media", path: "assets/wall.mp4", class: "background", z_index: -1 },
                { type: "hbox", class: "row", children: [
                    { type: "clock", format: "%H:%M" },
                    { type: "text", text: "hi", class: "note" }
                ] }
            ]
        }"#,
    )
    .unwrap();
    let python = widdgyy::config::migrate::to_python(&value).unwrap();
    let path = write_config("migrated.py", &python);
    let scene = ConfigLoader::new(&path)
        .load(Viewport::default())
        .expect("loading migrated config");
    assert_eq!(scene.roots.len(), 2);
    assert_eq!(scene.roots[1].classes, vec!["row"]);
    assert_eq!(scene.roots[1].children.len(), 2);
}
