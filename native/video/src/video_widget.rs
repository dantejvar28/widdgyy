// SPDX-FileCopyrightText: 2026 Daniel
// SPDX-License-Identifier: GPL-3.0-or-later

use gtk4::prelude::*;
use gtk4::{Align, MediaFile, Video};

pub struct VideoWidget {
    video: Video,
}

impl VideoWidget {
    pub fn new(media_file: &MediaFile) -> Self {
        let video = Video::for_media_stream(Some(media_file));

        video.set_hexpand(true);
        video.set_vexpand(true);
        video.set_halign(Align::Start);
        video.set_valign(Align::Start);

        Self {
            video,
        }
    }

    pub fn widget(&self) -> &Video {
        &self.video
    }

    pub fn show(&self) {
        self.video.set_visible(true);
    }

    pub fn hide(&self) {
        self.video.set_visible(false);
    }

    pub fn set_bounds(&self, x: i32, y: i32, width: u32, height: u32) {
        self.video.set_margin_start(x.max(0));
        self.video.set_margin_top(y.max(0));
        self.video.set_size_request(width as i32, height as i32);
    }
}