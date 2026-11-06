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

/// One pre-rendered text-shadow layer. The scene contains shadow glyphs for a
/// single blur radius; the compositor blurs it and composites it behind the
/// crisp overlay text.
pub struct GlowLayer {
    pub scene: vello::Scene,
    pub radius: f32,
}
pub use engine::GpuRenderer;
pub use filter::{FilterLayer, SpatialFilter, has_spatial_filter, root_spatial_filter};
pub use media::MediaStore;
pub use painter::Painter;
