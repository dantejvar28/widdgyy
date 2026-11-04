// SPDX-FileCopyrightText: 2026 Daniel
// SPDX-License-Identifier: GPL-3.0-or-later

//! Media backends. Images/GIFs live in the renderer cache; video uses
//! GStreamer's `appsink` so it never needs a GTK widget.

#[cfg(feature = "video")]
pub mod video;
