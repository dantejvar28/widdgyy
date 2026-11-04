// SPDX-FileCopyrightText: 2026 Daniel
// SPDX-License-Identifier: GPL-3.0-or-later

//! GStreamer video backend.
//!
//! A `playbin` renders into an `appsink`; each scene tick pulls the newest
//! NV12 frame. NV12 is uploaded to two GPU planes and converted by the fast
//! compositor shader, avoiding a CPU RGBA conversion for every frame. There is
//! no GTK widget, no overlay and no separate GLib loop.

use std::path::Path;
use std::sync::Arc;

use crate::media::VideoFrameData;

use anyhow::{Context, Result};
use gst::prelude::*;
use gstreamer as gst;
use gstreamer_app as gst_app;
use gstreamer_video as gst_video;

pub struct VideoAsset {
    pipeline: gst::Pipeline,
    sink: gst_app::AppSink,
    pub width: u32,
    pub height: u32,
    looping: bool,
    frame: Option<Arc<VideoFrameData>>,
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
        // NV12 system memory lets us skip the expensive RGBA color conversion
        // in GStreamer; the compositor shader converts YUV to RGB on the GPU.
        let caps = gst::Caps::builder("video/x-raw")
            .features(["memory:SystemMemory"])
            .field("format", "NV12")
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

    pub fn frame(&self) -> Option<Arc<VideoFrameData>> {
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
            if let Some(frame) = sample_to_frame(&sample) {
                self.width = frame.width;
                self.height = frame.height;
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

fn sample_to_frame(sample: &gst::Sample) -> Option<VideoFrameData> {
    let caps = sample.caps()?;
    let info = gst_video::VideoInfo::from_caps(caps).ok()?;
    if info.format() != gst_video::VideoFormat::Nv12 {
        return None;
    }
    let buffer = sample.buffer()?;
    let map = buffer.map_readable().ok()?;
    let width = info.width();
    let height = info.height();
    let y_offset = *info.offset().first()?;
    let uv_offset = *info.offset().get(1)?;
    let y_stride = info.stride().first().copied()?.max(0) as usize;
    let uv_stride = info.stride().get(1).copied()?.max(0) as usize;
    let uv_height = height.div_ceil(2) as usize;
    let source = map.as_slice();
    if y_offset + y_stride * height as usize > source.len()
        || uv_offset + uv_stride * uv_height > source.len()
    {
        return None;
    }
    Some(VideoFrameData {
        buffer: Arc::from(source),
        width,
        height,
        y_offset,
        y_stride: y_stride as u32,
        uv_offset,
        uv_stride: uv_stride as u32,
    })
}
