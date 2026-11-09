// SPDX-FileCopyrightText: 2026 Daniel
// SPDX-License-Identifier: GPL-3.0-or-later

//! Widdgyy: a GPU accelerated desktop ricing engine for Wayland.
//!
//! The crate is intentionally split into small modules:
//!
//! * [`css`] — parsing, cascade and typed style modules.
//! * [`layout`] — box model and flex/grid layout.
//! * [`scene`] — runtime scene graph.
//! * [`text`] — shaping and glyph cache.
//! * [`api`] — API sticker worker.
//! * [`render`] — Vello/wgpu painting.
//! * [`platform`] — Wayland layer-shell host.
//! * [`config`] — JSON5 configuration loader.

pub mod animation;
pub mod api;
pub mod app;
pub mod config;
pub mod conformance;
pub mod css;
pub mod diagnostics;
pub mod layout;
pub mod media;
pub mod platform;
pub mod render;
pub mod scene;
pub mod text;

use std::path::PathBuf;

/// Default XDG configuration directory.
/// Active configuration: `config.py` wins when present, otherwise JSONC.
pub fn user_config_path() -> PathBuf {
    let dir = user_config_dir();
    let python = dir.join("config.py");
    if python.is_file() {
        python
    } else {
        dir.join("config.jsonc")
    }
}

pub fn user_config_dir() -> PathBuf {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    home.join(".config").join("widdgyy")
}
