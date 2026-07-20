// SPDX-FileCopyrightText: 2026 Daniel
// SPDX-License-Identifier: GPL-3.0-or-later

use gstreamer as gst;
use gst::prelude::*;

pub fn duration(
    pipeline: &gst::Pipeline,
) -> u64 {

    pipeline
        .query_duration::<gst::ClockTime>()
        .map(|d| d.mseconds())
        .unwrap_or(0)
}

pub fn position(
    pipeline: &gst::Pipeline,
) -> u64 {

    pipeline
        .query_position::<gst::ClockTime>()
        .map(|d| d.mseconds())
        .unwrap_or(0)
}