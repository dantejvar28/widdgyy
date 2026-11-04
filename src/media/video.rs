// SPDX-FileCopyrightText: 2026 Daniel
// SPDX-License-Identifier: GPL-3.0-or-later

//! GStreamer video backend.
//!
//! A `playbin` renders into an `appsink`; each scene tick pulls the newest RGBA
//! frame and publishes it as a Vello image. There is no GTK widget, no overlay
//! and no separate GLib loop: GStreamer's streaming threads fill the sink and
//! the main loop only copies the latest frame when it needs one.

use std::path::Path;
use std::sync::Arc;

use anyhow::{Context, Result};
use gst::prelude::*;
use gstreamer as gst;
use gstreamer_app as gst_app;
use gstreamer_video as gst_video;
use vello::peniko::{Blob, ImageAlphaType, ImageData, ImageFormat};

pub struct VideoAsset {
    pipeline: gst::Pipeline,
    sink: gst_app::AppSink,
    pub width: u32,
    pub height: u32,
    looping: bool,
    frame: Option<Arc<ImageData>>,
    revision: u64,
    playing: bool,
}

impl VideoAsset {
    pub fn new(path: &Path, autoplay: bool, looping: bool) -> Result<Self> {
        gst::init().context("initializing GStreamer")?;

        let uri = gst::glib::filename_to_uri(path, None)
            .map_err(|error| anyhow::anyhow!("invalid media path: {error}"))?;
        let pipeline = gst::Pipeline::new();
        let playbin = gst::ElementFactory::make("playbin")
            .property("uri", uri)
            .build()
            .context("creating playbin")?;
        // Force a single packed RGBA format. Without explicit caps playbin may
        // negotiate NV12/YUV, which cannot be copied as `width * 4` rows.
        let caps = gst::Caps::builder("video/x-raw")
            .field("format", "RGBA")
            .build();
        let sink = gst::ElementFactory::make("appsink")
            .property("sync", true)
            .property("max-buffers", 1u32)
            .property("drop", true)
            .property("emit-signals", false)
            .property("caps", caps)
            .build()
            .context("creating appsink")?
            .downcast::<gst_app::AppSink>()
            .map_err(|_| anyhow::anyhow!("appsink downcast failed"))?;
        playbin.set_property("video-sink", &sink);
        playbin.set_property(
            "audio-sink",
            gst::ElementFactory::make("fakesink").build().ok(),
        );
        pipeline.add(&playbin).context("adding playbin")?;

        pipeline
            .set_state(if autoplay {
                gst::State::Playing
            } else {
                gst::State::Paused
            })
            .context("starting video pipeline")?;

        Ok(Self {
            pipeline,
            sink,
            width: 0,
            height: 0,
            looping,
            frame: None,
            revision: 0,
            playing: autoplay,
        })
    }

    pub fn play(&mut self) {
        let _ = self.pipeline.set_state(gst::State::Playing);
        self.playing = true;
    }

    pub fn pause(&mut self) {
        let _ = self.pipeline.set_state(gst::State::Paused);
        self.playing = false;
    }

    pub fn frame(&self) -> Option<Arc<ImageData>> {
        self.frame.clone()
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Pull the newest available frame. Returns `true` when the frame changed.
    pub fn pump(&mut self) -> bool {
        let mut changed = false;
        // Cap the amount of work per tick; the sink drops old frames anyway.
        for _ in 0..4 {
            let Some(sample) = self.sink.try_pull_sample(gst::ClockTime::ZERO) else {
                break;
            };
            if let Some(frame) = sample_to_image(&sample) {
                if let Some((width, height)) = frame_dimensions(&frame) {
                    self.width = width;
                    self.height = height;
                }
                self.frame = Some(Arc::new(frame));
                self.revision = self.revision.wrapping_add(1);
                changed = true;
            }
        }

        if !changed && self.looping && self.playing {
            self.restart_at_end();
        }
        changed
    }

    fn restart_at_end(&self) {
        let Some(position) = self.pipeline.query_position::<gst::ClockTime>() else {
            return;
        };
        let Some(duration) = self.pipeline.query_duration::<gst::ClockTime>() else {
            return;
        };
        if duration == gst::ClockTime::ZERO {
            return;
        }
        if position + gst::ClockTime::from_mseconds(120) >= duration {
            let _ = self.pipeline.seek_simple(
                gst::SeekFlags::FLUSH | gst::SeekFlags::KEY_UNIT,
                gst::ClockTime::ZERO,
            );
            let _ = self.pipeline.set_state(gst::State::Playing);
        }
    }
}

impl Drop for VideoAsset {
    fn drop(&mut self) {
        let _ = self.pipeline.set_state(gst::State::Null);
    }
}

fn frame_dimensions(image: &ImageData) -> Option<(u32, u32)> {
    (image.width > 0 && image.height > 0).then_some((image.width, image.height))
}

fn sample_to_image(sample: &gst::Sample) -> Option<ImageData> {
    let caps = sample.caps()?;
    let info = gst_video::VideoInfo::from_caps(caps).ok()?;
    if info.format() != gst_video::VideoFormat::Rgba {
        return None;
    }
    let buffer = sample.buffer()?;
    let map = buffer.map_readable().ok()?;
    let width = info.width();
    let height = info.height();
    let stride = info.stride().first().copied().unwrap_or(0).max(0) as usize;
    let source = map.as_slice();
    let row_bytes = width as usize * 4;
    let mut rgba = Vec::with_capacity(row_bytes * height as usize);
    for row in 0..height as usize {
        let start = row * stride;
        if start + row_bytes > source.len() {
            return None;
        }
        rgba.extend_from_slice(&source[start..start + row_bytes]);
    }
    Some(ImageData {
        data: Blob::from(rgba),
        format: ImageFormat::Rgba8,
        alpha_type: ImageAlphaType::Alpha,
        width,
        height,
    })
}
