// SPDX-FileCopyrightText: 2026 Daniel
// SPDX-License-Identifier: GPL-3.0-or-later

//! Painting and GPU presentation.

pub mod box_painter;
pub mod compositor;
pub mod effects_painter;
pub mod engine;
pub mod filter;
pub mod media;
pub mod media_painter;
pub mod painter;
pub mod text_painter;

pub use compositor::{Compositor, VideoFrame};

/// One pre-rendered text glow layer. The scene contains only glyphs that opted
/// into `--glow`; the compositor blurs it with `radius` and adds the animated
/// pulse when the video overlay is composited.
pub struct GlowLayer {
    pub scene: vello::Scene,
    pub radius: f32,
}
pub use engine::GpuRenderer;
pub use filter::{FilterLayer, SpatialFilter, has_spatial_filter, root_spatial_filter};
pub use media::MediaStore;
pub use painter::Painter;
