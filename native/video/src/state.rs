// SPDX-FileCopyrightText: 2026 Daniel
// SPDX-License-Identifier: GPL-3.0-or-later

#[derive(Clone,Copy,PartialEq,Eq)]
pub enum VideoState {
    Playing,
    Paused,
    Stopped,
}