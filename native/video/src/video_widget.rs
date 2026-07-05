use gtk4::prelude::*;
use gtk4::{Align, MediaFile, Picture};

pub struct VideoWidget {
    picture: Picture,
}

impl VideoWidget {
    pub fn new(media_file: &MediaFile) -> Self {
        let picture = Picture::for_paintable(media_file);

        picture.set_hexpand(true);
        picture.set_vexpand(true);
        picture.set_halign(Align::Start);
        picture.set_valign(Align::Start);
        picture.set_can_shrink(true);

        Self {
            picture,
        }
    }

    pub fn widget(&self) -> &Picture {
        &self.picture
    }

    pub fn show(&self) {
        self.picture.set_visible(true);
    }

    pub fn hide(&self) {
        self.picture.set_visible(false);
    }

    pub fn set_bounds(&self, x: i32, y: i32, width: u32, height: u32) {
        self.picture.set_margin_start(x.max(0));
        self.picture.set_margin_top(y.max(0));
        self.picture.set_size_request(width as i32, height as i32);
    }
}