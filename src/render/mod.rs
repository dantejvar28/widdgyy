// SPDX-FileCopyrightText: 2026 Daniel
// SPDX-License-Identifier: GPL-3.0-or-later

//! Painting and GPU presentation.

pub mod box_painter;
pub mod effects_painter;
pub mod engine;
pub mod media;
pub mod media_painter;
pub mod painter;
pub mod text_painter;

pub use engine::GpuRenderer;
pub use media::MediaStore;
pub use painter::Painter;
