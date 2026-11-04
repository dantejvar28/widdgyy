// SPDX-FileCopyrightText: 2026 Daniel
// SPDX-License-Identifier: GPL-3.0-or-later

//! Animations, transitions and `@keyframes` data structures.

use std::time::Duration;

use super::{Declarations, split_commas, split_ws};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TimingFunction {
    Linear,
    Ease,
    EaseIn,
    EaseOut,
    EaseInOut,
    CubicBezier(f32, f32, f32, f32),
    Steps(u32, StepPosition),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StepPosition {
    Start,
    End,
    Both,
    None,
}

impl TimingFunction {
    pub fn parse(value: &str) -> Self {
        let text = value.trim().to_ascii_lowercase();
        match text.as_str() {
            "linear" => Self::Linear,
            "ease" => Self::Ease,
            "ease-in" => Self::EaseIn,
            "ease-out" => Self::EaseOut,
            "ease-in-out" => Self::EaseInOut,
            _ if text.starts_with("cubic-bezier(") => {
                let inner = text
                    .trim_start_matches("cubic-bezier(")
                    .trim_end_matches(')');
                let values: Vec<f32> = inner
                    .split(',')
                    .filter_map(|value| value.trim().parse().ok())
                    .collect();
                if values.len() == 4 {
                    Self::CubicBezier(values[0], values[1], values[2], values[3])
                } else {
                    Self::Ease
                }
            }
            _ if text.starts_with("steps(") => {
                let inner = text.trim_start_matches("steps(").trim_end_matches(')');
                let mut parts = inner.split(',');
                let count = parts
                    .next()
                    .and_then(|value| value.trim().parse::<u32>().ok())
                    .unwrap_or(1);
                let position = match parts.next().map(str::trim) {
                    Some("start") => StepPosition::Start,
                    Some("both") => StepPosition::Both,
                    Some("none") => StepPosition::None,
                    _ => StepPosition::End,
                };
                Self::Steps(count.max(1), position)
            }
            _ => Self::Ease,
        }
    }

    /// Evaluate the easing curve at normalized progress `t` in `0..=1`.
    pub fn sample(self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Self::Linear => t,
            Self::Ease => cubic_bezier(0.25, 0.1, 0.25, 1.0, t),
            Self::EaseIn => cubic_bezier(0.42, 0.0, 1.0, 1.0, t),
            Self::EaseOut => cubic_bezier(0.0, 0.0, 0.58, 1.0, t),
            Self::EaseInOut => cubic_bezier(0.42, 0.0, 0.58, 1.0, t),
            Self::CubicBezier(x1, y1, x2, y2) => cubic_bezier(x1, y1, x2, y2, t),
            Self::Steps(count, position) => {
                let value = (t * count as f32).ceil() / count as f32;
                match position {
                    StepPosition::Start => 1.0 - value,
                    _ => value.min(1.0),
                }
            }
        }
    }
}

/// Approximate a cubic-bezier by solving x(t) with Newton iterations.
fn cubic_bezier(x1: f32, y1: f32, x2: f32, y2: f32, x: f32) -> f32 {
    let bezier = |t: f32, p1: f32, p2: f32| {
        let mt = 1.0 - t;
        3.0 * mt * mt * t * p1 + 3.0 * mt * t * t * p2 + t * t * t
    };
    let mut t = x;
    for _ in 0..8 {
        let current = bezier(t, x1, x2);
        let delta = current - x;
        if delta.abs() < 0.0001 {
            break;
        }
        let slope = 3.0 * (1.0 - t) * (1.0 - t) * x1
            + 6.0 * (1.0 - t) * t * (x2 - x1)
            + 3.0 * t * t * (1.0 - x2);
        if slope.abs() < 0.0001 {
            break;
        }
        t -= delta / slope;
        t = t.clamp(0.0, 1.0);
    }
    bezier(t, y1, y2)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnimationDirection {
    Normal,
    Reverse,
    Alternate,
    AlternateReverse,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnimationFillMode {
    None,
    Forwards,
    Backwards,
    Both,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlayState {
    Running,
    Paused,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Animation {
    pub name: String,
    pub duration: Duration,
    pub delay: Duration,
    pub timing: TimingFunction,
    pub iteration_count: Option<f32>,
    pub direction: AnimationDirection,
    pub fill_mode: AnimationFillMode,
    pub play_state: PlayState,
}

impl Default for Animation {
    fn default() -> Self {
        Self {
            name: String::new(),
            duration: Duration::ZERO,
            delay: Duration::ZERO,
            timing: TimingFunction::Ease,
            iteration_count: Some(1.0),
            direction: AnimationDirection::Normal,
            fill_mode: AnimationFillMode::None,
            play_state: PlayState::Running,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Transition {
    pub property: String,
    pub duration: Duration,
    pub delay: Duration,
    pub timing: TimingFunction,
}

impl Default for Transition {
    fn default() -> Self {
        Self {
            property: "all".to_string(),
            duration: Duration::ZERO,
            delay: Duration::ZERO,
            timing: TimingFunction::Ease,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Keyframe {
    pub offset: f32,
    pub declarations: Vec<(String, String)>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Keyframes {
    pub name: String,
    pub frames: Vec<Keyframe>,
}

pub fn apply(decl: &mut Declarations, name: &str, value: &str) -> bool {
    match name {
        "animation" => {
            decl.animation = Some(parse_animation_list(value));
            true
        }
        "animation-name" => {
            let animations = decl.animation.get_or_insert_with(Vec::new);
            ensure_animation(animations);
            if let Some(animation) = animations.first_mut() {
                animation.name = value.trim().to_string();
            }
            true
        }
        "animation-duration" => {
            let times = parse_time_list(value);
            let animations = decl.animation.get_or_insert_with(Vec::new);
            ensure_animation(animations);
            for (animation, time) in animations.iter_mut().zip(times) {
                animation.duration = time;
            }
            true
        }
        "animation-delay" => {
            let times = parse_time_list(value);
            let animations = decl.animation.get_or_insert_with(Vec::new);
            ensure_animation(animations);
            for (animation, time) in animations.iter_mut().zip(times) {
                animation.delay = time;
            }
            true
        }
        "animation-timing-function" => {
            let functions: Vec<TimingFunction> = split_commas(value)
                .iter()
                .map(|value| TimingFunction::parse(value))
                .collect();
            let animations = decl.animation.get_or_insert_with(Vec::new);
            ensure_animation(animations);
            for (animation, function) in animations.iter_mut().zip(functions) {
                animation.timing = function;
            }
            true
        }
        "animation-iteration-count" => {
            let counts: Vec<Option<f32>> = split_commas(value)
                .iter()
                .map(|value| {
                    if value.trim().eq_ignore_ascii_case("infinite") {
                        None
                    } else {
                        value.trim().parse::<f32>().ok()
                    }
                })
                .collect();
            let animations = decl.animation.get_or_insert_with(Vec::new);
            ensure_animation(animations);
            for (animation, count) in animations.iter_mut().zip(counts) {
                animation.iteration_count = count;
            }
            true
        }
        "animation-direction" => {
            let directions: Vec<AnimationDirection> = split_commas(value)
                .iter()
                .map(|value| match value.trim().to_ascii_lowercase().as_str() {
                    "reverse" => AnimationDirection::Reverse,
                    "alternate" => AnimationDirection::Alternate,
                    "alternate-reverse" => AnimationDirection::AlternateReverse,
                    _ => AnimationDirection::Normal,
                })
                .collect();
            let animations = decl.animation.get_or_insert_with(Vec::new);
            ensure_animation(animations);
            for (animation, direction) in animations.iter_mut().zip(directions) {
                animation.direction = direction;
            }
            true
        }
        "animation-fill-mode" => {
            let modes: Vec<AnimationFillMode> = split_commas(value)
                .iter()
                .map(|value| match value.trim().to_ascii_lowercase().as_str() {
                    "forwards" => AnimationFillMode::Forwards,
                    "backwards" => AnimationFillMode::Backwards,
                    "both" => AnimationFillMode::Both,
                    _ => AnimationFillMode::None,
                })
                .collect();
            let animations = decl.animation.get_or_insert_with(Vec::new);
            ensure_animation(animations);
            for (animation, mode) in animations.iter_mut().zip(modes) {
                animation.fill_mode = mode;
            }
            true
        }
        "animation-play-state" => {
            let states: Vec<PlayState> = split_commas(value)
                .iter()
                .map(|value| {
                    if value.trim().eq_ignore_ascii_case("paused") {
                        PlayState::Paused
                    } else {
                        PlayState::Running
                    }
                })
                .collect();
            let animations = decl.animation.get_or_insert_with(Vec::new);
            ensure_animation(animations);
            for (animation, state) in animations.iter_mut().zip(states) {
                animation.play_state = state;
            }
            true
        }
        "transition" => {
            decl.transition = Some(parse_transition_list(value));
            true
        }
        "transition-property" => {
            let properties = split_commas(value);
            let transitions = decl.transition.get_or_insert_with(Vec::new);
            ensure_transition(transitions);
            for (transition, property) in transitions.iter_mut().zip(properties) {
                transition.property = property.trim().to_string();
            }
            true
        }
        "transition-duration" => {
            let times = parse_time_list(value);
            let transitions = decl.transition.get_or_insert_with(Vec::new);
            ensure_transition(transitions);
            for (transition, time) in transitions.iter_mut().zip(times) {
                transition.duration = time;
            }
            true
        }
        "transition-delay" => {
            let times = parse_time_list(value);
            let transitions = decl.transition.get_or_insert_with(Vec::new);
            ensure_transition(transitions);
            for (transition, time) in transitions.iter_mut().zip(times) {
                transition.delay = time;
            }
            true
        }
        "transition-timing-function" => {
            let functions: Vec<TimingFunction> = split_commas(value)
                .iter()
                .map(|value| TimingFunction::parse(value))
                .collect();
            let transitions = decl.transition.get_or_insert_with(Vec::new);
            ensure_transition(transitions);
            for (transition, function) in transitions.iter_mut().zip(functions) {
                transition.timing = function;
            }
            true
        }
        _ => false,
    }
}

fn ensure_animation(animations: &mut Vec<Animation>) {
    if animations.is_empty() {
        animations.push(Animation::default());
    }
}

fn ensure_transition(transitions: &mut Vec<Transition>) {
    if transitions.is_empty() {
        transitions.push(Transition::default());
    }
}

pub fn parse_time(value: &str) -> Option<Duration> {
    let text = value.trim().to_ascii_lowercase();
    if let Some(ms) = text.strip_suffix("ms") {
        return ms
            .trim()
            .parse::<f64>()
            .ok()
            .map(|value| Duration::from_secs_f64(value / 1000.0));
    }
    if let Some(seconds) = text.strip_suffix('s') {
        return seconds
            .trim()
            .parse::<f64>()
            .ok()
            .map(Duration::from_secs_f64);
    }
    text.parse::<f64>().ok().map(Duration::from_secs_f64)
}

fn parse_time_list(value: &str) -> Vec<Duration> {
    split_commas(value)
        .iter()
        .filter_map(|value| parse_time(value))
        .collect()
}

fn parse_animation_list(value: &str) -> Vec<Animation> {
    let mut animations = Vec::new();
    for part in split_commas(value) {
        let mut animation = Animation::default();
        let mut name_set = false;
        for token in split_ws(&part) {
            let lower = token.to_ascii_lowercase();
            if let Some(time) = parse_time(&lower) {
                if animation.duration.is_zero() && !lower.ends_with("ms") {
                    animation.duration = time;
                } else if animation.delay.is_zero() {
                    animation.delay = time;
                } else {
                    animation.duration = time;
                }
            } else if lower == "infinite" {
                animation.iteration_count = None;
            } else if let Ok(count) = lower.parse::<f32>() {
                animation.iteration_count = Some(count);
            } else if matches!(
                lower.as_str(),
                "linear" | "ease" | "ease-in" | "ease-out" | "ease-in-out"
            ) || lower.starts_with("cubic-bezier(")
                || lower.starts_with("steps(")
            {
                animation.timing = TimingFunction::parse(&lower);
            } else if matches!(
                lower.as_str(),
                "normal" | "reverse" | "alternate" | "alternate-reverse"
            ) {
                animation.direction = match lower.as_str() {
                    "reverse" => AnimationDirection::Reverse,
                    "alternate" => AnimationDirection::Alternate,
                    "alternate-reverse" => AnimationDirection::AlternateReverse,
                    _ => AnimationDirection::Normal,
                };
            } else if matches!(lower.as_str(), "forwards" | "backwards" | "both") {
                animation.fill_mode = match lower.as_str() {
                    "forwards" => AnimationFillMode::Forwards,
                    "backwards" => AnimationFillMode::Backwards,
                    _ => AnimationFillMode::Both,
                };
            } else if matches!(lower.as_str(), "running" | "paused") {
                animation.play_state = if lower == "paused" {
                    PlayState::Paused
                } else {
                    PlayState::Running
                };
            } else if lower != "none" && !name_set {
                animation.name = token;
                name_set = true;
            }
        }
        if !animation.name.is_empty() {
            animations.push(animation);
        }
    }
    animations
}

fn parse_transition_list(value: &str) -> Vec<Transition> {
    let mut transitions = Vec::new();
    for part in split_commas(value) {
        let mut transition = Transition::default();
        let mut property_set = false;
        for token in split_ws(&part) {
            let lower = token.to_ascii_lowercase();
            if let Some(time) = parse_time(&lower) {
                if transition.duration.is_zero() {
                    transition.duration = time;
                } else {
                    transition.delay = time;
                }
            } else if matches!(
                lower.as_str(),
                "linear" | "ease" | "ease-in" | "ease-out" | "ease-in-out"
            ) || lower.starts_with("cubic-bezier(")
                || lower.starts_with("steps(")
            {
                transition.timing = TimingFunction::parse(&lower);
            } else if !property_set {
                transition.property = token;
                property_set = true;
            }
        }
        transitions.push(transition);
    }
    transitions
}

/// Parse a raw `@keyframes` block body into offsets and declarations.
pub fn parse_keyframes(name: &str, body: &str) -> Keyframes {
    let mut frames = Vec::new();
    for block in crate::css::parser::split_blocks(body) {
        let offsets: Vec<f32> = block
            .prelude
            .split(',')
            .filter_map(|value| {
                let value = value.trim();
                if value.eq_ignore_ascii_case("from") {
                    Some(0.0)
                } else if value.eq_ignore_ascii_case("to") {
                    Some(1.0)
                } else {
                    value
                        .trim_end_matches('%')
                        .parse::<f32>()
                        .ok()
                        .map(|value| value / 100.0)
                }
            })
            .collect();
        for offset in offsets {
            frames.push(Keyframe {
                offset: offset.clamp(0.0, 1.0),
                declarations: crate::css::parser::parse_declarations(&block.body),
            });
        }
    }
    frames.sort_by(|a, b| a.offset.total_cmp(&b.offset));
    Keyframes {
        name: name.to_string(),
        frames,
    }
}
