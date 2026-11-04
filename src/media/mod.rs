// SPDX-FileCopyrightText: 2026 Daniel
// SPDX-License-Identifier: GPL-3.0-or-later

//! Media backends. Images/GIFs live in the renderer cache; video uses
//! GStreamer's `appsink` so it never needs a GTK widget.

use std::sync::Arc;

/// One decoded NV12 frame. Kept outside the optional video module so the
/// renderer/compositor types compile in `--no-default-features` builds too.
#[derive(Clone, Debug)]
pub struct VideoFrameData {
    pub buffer: Arc<[u8]>,
    pub width: u32,
    pub height: u32,
    pub y_offset: usize,
    pub y_stride: u32,
    pub uv_offset: usize,
    pub uv_stride: u32,
}

#[cfg(feature = "video")]
pub mod video;
