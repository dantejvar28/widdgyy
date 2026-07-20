// SPDX-FileCopyrightText: 2026 Daniel
// SPDX-License-Identifier: GPL-3.0-or-later

use gstreamer as gst;
use gst::prelude::*;

pub fn play(pipeline: &gst::Pipeline){
    let _ = pipeline.set_state(gst::State::Playing);
}

pub fn pause(pipeline: &gst::Pipeline){
    let _ = pipeline.set_state(gst::State::Paused);
}

pub fn stop(pipeline: &gst::Pipeline){
    let _ = pipeline.set_state(gst::State::Ready);

    let _ = pipeline.seek_simple(
        gst::SeekFlags::FLUSH,
        gst::ClockTime::ZERO,
    );
}

pub fn seek (
    pipeline: &gst::Pipeline,
    ms: u64,
){
    let _ = pipeline.seek_simple(
        gst::SeekFlags::FLUSH,
        gst::ClockTime::from_mseconds(ms),
    );
}