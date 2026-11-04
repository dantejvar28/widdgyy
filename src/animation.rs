// SPDX-FileCopyrightText: 2026 Daniel
// SPDX-License-Identifier: GPL-3.0-or-later

//! CSS animations.
//!
//! Keyframes are compiled to typed values when the stylesheet is applied. The
//! runtime only samples numeric values and writes an overlay into each node;
//! no CSS strings are parsed per frame. Transform/opacity/color animations do
//! not trigger layout, so they work with the cached video overlay path.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use vello::peniko::Color;

use crate::css::Stylesheet;
use crate::css::properties::Declarations;
use crate::css::properties::transition::{
    Animation, AnimationDirection, AnimationFillMode, PlayState,
};
use crate::css::values::{self, Length, TransformOp};
use crate::scene::{AnimatedStyle, Node};

#[derive(Clone, Debug, Default, PartialEq)]
struct FrameValues {
    opacity: Option<f32>,
    transform: Option<Vec<TransformOp>>,
    color: Option<Color>,
    background_color: Option<Color>,
}

#[derive(Clone, Debug)]
struct CompiledFrame {
    offset: f32,
    values: FrameValues,
}

#[derive(Clone, Debug)]
struct BaseValues {
    opacity: f32,
    transform: Vec<TransformOp>,
    color: Color,
    background_color: Color,
}

#[derive(Clone, Copy, Debug, Default)]
struct FrameFields {
    opacity: bool,
    transform: bool,
    color: bool,
    background_color: bool,
}

struct CompiledAnimation {
    animation: Animation,
    frames: Vec<CompiledFrame>,
    fields: FrameFields,
}

pub struct AnimationSet {
    start: Instant,
    base: BaseValues,
    animations: Vec<CompiledAnimation>,
    infinite: bool,
    total: Duration,
}

impl AnimationSet {
    pub fn compile(
        stylesheet: &Stylesheet,
        style: &crate::css::ComputedStyle,
        start: Instant,
    ) -> Option<Self> {
        if style.animations.is_empty() {
            return None;
        }
        let base = BaseValues {
            opacity: style.effects.opacity,
            transform: style.effects.transform.clone(),
            color: style.text.color,
            background_color: style.background.color,
        };
        let mut animations = Vec::new();
        let mut infinite = false;
        let mut total = Duration::ZERO;
        for animation in &style.animations {
            if animation.name.is_empty() || animation.name == "none" {
                continue;
            }
            let Some(keyframes) = stylesheet.keyframes.get(&animation.name) else {
                continue;
            };
            let mut frames = Vec::new();
            for frame in &keyframes.frames {
                let mut declarations = Declarations::default();
                for (name, value) in &frame.declarations {
                    declarations.apply(name, value, base.color);
                }
                frames.push(CompiledFrame {
                    offset: frame.offset,
                    values: FrameValues {
                        opacity: declarations.opacity,
                        transform: declarations.transform,
                        color: declarations.color,
                        background_color: declarations.background_color,
                    },
                });
            }
            if frames.is_empty() {
                continue;
            }
            frames.sort_by(|a, b| a.offset.total_cmp(&b.offset));
            if frames.first().is_none_or(|frame| frame.offset > 0.0) {
                frames.insert(
                    0,
                    CompiledFrame {
                        offset: 0.0,
                        values: FrameValues::default(),
                    },
                );
            }
            if frames.last().is_none_or(|frame| frame.offset < 1.0) {
                frames.push(CompiledFrame {
                    offset: 1.0,
                    values: FrameValues::default(),
                });
            }
            if animation.iteration_count.is_none() {
                infinite = true;
            } else {
                let iterations = animation.iteration_count.unwrap_or(1.0).max(0.0);
                total += animation.delay + animation.duration.mul_f32(iterations);
            }
            let fields = FrameFields {
                opacity: frames.iter().any(|frame| frame.values.opacity.is_some()),
                transform: frames.iter().any(|frame| frame.values.transform.is_some()),
                color: frames.iter().any(|frame| frame.values.color.is_some()),
                background_color: frames
                    .iter()
                    .any(|frame| frame.values.background_color.is_some()),
            };
            animations.push(CompiledAnimation {
                animation: animation.clone(),
                frames,
                fields,
            });
        }
        if animations.is_empty() {
            return None;
        }
        Some(Self {
            start,
            base,
            animations,
            infinite,
            total,
        })
    }

    pub fn is_active(&self, now: Instant) -> bool {
        if self.infinite {
            return true;
        }
        now.saturating_duration_since(self.start) < self.total
    }

    pub fn sample(&self, now: Instant) -> (AnimatedStyle, bool) {
        let elapsed = now.saturating_duration_since(self.start);
        let mut overlay = AnimatedStyle::default();
        let mut active = false;

        for compiled in &self.animations {
            let animation = &compiled.animation;
            if animation.play_state == PlayState::Paused {
                continue;
            }
            let delay = animation.delay;
            let duration = if animation.duration.is_zero() {
                Duration::from_millis(1)
            } else {
                animation.duration
            };
            let iterations = animation.iteration_count.unwrap_or(f32::INFINITY);
            if elapsed < delay {
                active = true;
                if matches!(
                    animation.fill_mode,
                    AnimationFillMode::Backwards | AnimationFillMode::Both
                ) {
                    apply_values(&mut overlay, &sample_at(compiled, 0.0, &self.base));
                }
                continue;
            }

            let local = elapsed - delay;
            let iteration = if duration.is_zero() {
                0
            } else {
                (local.as_secs_f64() / duration.as_secs_f64()) as u32
            };
            let finished = iterations.is_finite() && iteration as f32 >= iterations;
            if finished {
                if matches!(
                    animation.fill_mode,
                    AnimationFillMode::Forwards | AnimationFillMode::Both
                ) {
                    apply_values(&mut overlay, &sample_at(compiled, 1.0, &self.base));
                }
                continue;
            }

            active = true;
            let progress = if duration.is_zero() {
                1.0
            } else {
                (local.as_secs_f64() % duration.as_secs_f64()) / duration.as_secs_f64()
            } as f32;
            let progress = direction_progress(animation.direction, iteration, progress);
            let values = sample_at(compiled, progress, &self.base);
            apply_values(&mut overlay, &values);
        }

        (overlay, active)
    }
}

fn direction_progress(direction: AnimationDirection, iteration: u32, progress: f32) -> f32 {
    match direction {
        AnimationDirection::Normal => progress,
        AnimationDirection::Reverse => 1.0 - progress,
        AnimationDirection::Alternate => {
            if iteration.is_multiple_of(2) {
                progress
            } else {
                1.0 - progress
            }
        }
        AnimationDirection::AlternateReverse => {
            if iteration.is_multiple_of(2) {
                1.0 - progress
            } else {
                progress
            }
        }
    }
}

fn sample_at(compiled: &CompiledAnimation, progress: f32, base: &BaseValues) -> FrameValues {
    let progress = progress.clamp(0.0, 1.0);
    let mut before = &compiled.frames[0];
    let mut after = compiled.frames.last().unwrap();
    for frame in &compiled.frames {
        if frame.offset <= progress {
            before = frame;
        }
        if frame.offset >= progress {
            after = frame;
            break;
        }
    }
    if before.offset >= after.offset {
        return resolve_frame(before, base, compiled.fields);
    }
    let local = (progress - before.offset) / (after.offset - before.offset);
    let easing = compiled.animation.timing.sample(local);
    let a = resolve_frame(before, base, compiled.fields);
    let b = resolve_frame(after, base, compiled.fields);
    FrameValues {
        opacity: lerp_option_f32(a.opacity, b.opacity, easing),
        transform: match (&a.transform, &b.transform) {
            (Some(a), Some(b)) => lerp_transforms(a, b, easing),
            _ => a.transform.or(b.transform),
        },
        color: lerp_option_color(a.color, b.color, easing),
        background_color: lerp_option_color(a.background_color, b.background_color, easing),
    }
}

fn resolve_frame(frame: &CompiledFrame, base: &BaseValues, fields: FrameFields) -> FrameValues {
    FrameValues {
        opacity: fields
            .opacity
            .then(|| frame.values.opacity.unwrap_or(base.opacity)),
        transform: fields.transform.then(|| {
            frame
                .values
                .transform
                .clone()
                .unwrap_or_else(|| base.transform.clone())
        }),
        color: fields
            .color
            .then(|| frame.values.color.unwrap_or(base.color)),
        background_color: fields.background_color.then(|| {
            frame
                .values
                .background_color
                .unwrap_or(base.background_color)
        }),
    }
}

fn apply_values(target: &mut AnimatedStyle, values: &FrameValues) {
    if let Some(opacity) = values.opacity {
        target.opacity = Some(opacity);
    }
    if let Some(transform) = &values.transform {
        target.transform = Some(transform.clone());
    }
    if let Some(color) = values.color {
        target.color = Some(color);
    }
    if let Some(color) = values.background_color {
        target.background_color = Some(color);
    }
}

fn lerp_option_f32(a: Option<f32>, b: Option<f32>, t: f32) -> Option<f32> {
    Some(match (a, b) {
        (Some(a), Some(b)) => a + (b - a) * t,
        (Some(value), None) | (None, Some(value)) => value,
        (None, None) => return None,
    })
}

fn lerp_option_color(a: Option<Color>, b: Option<Color>, t: f32) -> Option<Color> {
    Some(match (a, b) {
        (Some(a), Some(b)) => values::color::mix(a, b, t),
        (Some(value), None) | (None, Some(value)) => value,
        (None, None) => return None,
    })
}

fn lerp_length(a: &Length, b: &Length, t: f32) -> Length {
    match (a, b) {
        (Length::Px(a), Length::Px(b)) => Length::Px(a + (b - a) * t),
        (Length::Zero, Length::Px(b)) => Length::Px(*b * t),
        (Length::Px(a), Length::Zero) => Length::Px(*a * (1.0 - t)),
        (Length::Percent(a), Length::Percent(b)) => Length::Percent(a + (b - a) * t),
        (Length::Em(a), Length::Em(b)) => Length::Em(a + (b - a) * t),
        (Length::Rem(a), Length::Rem(b)) => Length::Rem(a + (b - a) * t),
        (Length::Auto, Length::Auto) | (Length::Zero, Length::Zero) => Length::Zero,
        _ => Length::Px(
            a.resolve(&Default::default(), values::Axis::Horizontal) * (1.0 - t)
                + b.resolve(&Default::default(), values::Axis::Horizontal) * t,
        ),
    }
}

fn lerp_transforms(a: &[TransformOp], b: &[TransformOp], t: f32) -> Option<Vec<TransformOp>> {
    if a.len() != b.len() {
        return None;
    }
    let mut output = Vec::with_capacity(a.len());
    for (a, b) in a.iter().zip(b) {
        output.push(match (a, b) {
            (TransformOp::Translate(ax, ay), TransformOp::Translate(bx, by)) => {
                TransformOp::Translate(lerp_length(ax, bx, t), lerp_length(ay, by, t))
            }
            (TransformOp::TranslateX(ax), TransformOp::TranslateX(bx)) => {
                TransformOp::TranslateX(lerp_length(ax, bx, t))
            }
            (TransformOp::TranslateY(ay), TransformOp::TranslateY(by)) => {
                TransformOp::TranslateY(lerp_length(ay, by, t))
            }
            (TransformOp::Scale(ax, ay), TransformOp::Scale(bx, by)) => {
                TransformOp::Scale(ax + (bx - ax) * t, ay + (by - ay) * t)
            }
            (TransformOp::ScaleX(ax), TransformOp::ScaleX(bx)) => {
                TransformOp::ScaleX(ax + (bx - ax) * t)
            }
            (TransformOp::ScaleY(ay), TransformOp::ScaleY(by)) => {
                TransformOp::ScaleY(ay + (by - ay) * t)
            }
            (TransformOp::Rotate(a), TransformOp::Rotate(b)) => {
                TransformOp::Rotate(crate::css::values::Angle(a.0 + (b.0 - a.0) * t))
            }
            (TransformOp::Skew(ax, ay), TransformOp::Skew(bx, by)) => TransformOp::Skew(
                crate::css::values::Angle(ax.0 + (bx.0 - ax.0) * t),
                crate::css::values::Angle(ay.0 + (by.0 - ay.0) * t),
            ),
            (TransformOp::SkewX(ax), TransformOp::SkewX(bx)) => {
                TransformOp::SkewX(crate::css::values::Angle(ax.0 + (bx.0 - ax.0) * t))
            }
            (TransformOp::SkewY(ay), TransformOp::SkewY(by)) => {
                TransformOp::SkewY(crate::css::values::Angle(ay.0 + (by.0 - ay.0) * t))
            }
            (TransformOp::Matrix(a), TransformOp::Matrix(b)) => {
                let mut matrix = [0.0; 6];
                for index in 0..6 {
                    matrix[index] = a[index] + (b[index] - a[index]) * t;
                }
                TransformOp::Matrix(matrix)
            }
            _ => return None,
        });
    }
    Some(output)
}

/// Compile every animated node in the scene and reset its overlay.
pub fn prepare_scene(
    scene: &mut crate::scene::Scene,
    stylesheet: &Stylesheet,
    now: Instant,
) -> (HashMap<u64, AnimationSet>, bool) {
    let mut sets = HashMap::new();
    let mut active = false;
    for root in &mut scene.roots {
        prepare_node(root, stylesheet, now, &mut sets, &mut active);
    }
    (sets, active)
}

fn prepare_node(
    node: &mut Node,
    stylesheet: &Stylesheet,
    now: Instant,
    sets: &mut HashMap<u64, AnimationSet>,
    active: &mut bool,
) {
    node.animated = AnimatedStyle::default();
    if let Some(set) = AnimationSet::compile(stylesheet, &node.style, now) {
        *active |= set.is_active(now);
        sets.insert(node.uid, set);
    }
    for child in &mut node.children {
        prepare_node(child, stylesheet, now, sets, active);
    }
}

/// Update animated overlays. Returns `(changed, any_active)`.
pub fn update_scene(
    scene: &mut crate::scene::Scene,
    sets: &HashMap<u64, AnimationSet>,
    now: Instant,
) -> (bool, bool) {
    let mut changed = false;
    let mut active = false;
    for root in &mut scene.roots {
        update_node(root, sets, now, &mut changed, &mut active);
    }
    (changed, active)
}

fn update_node(
    node: &mut Node,
    sets: &HashMap<u64, AnimationSet>,
    now: Instant,
    changed: &mut bool,
    active: &mut bool,
) {
    if let Some(set) = sets.get(&node.uid) {
        let (overlay, is_active) = set.sample(now);
        *active |= is_active;
        if node.animated != overlay {
            node.animated = overlay;
            *changed = true;
        }
    }
    for child in &mut node.children {
        update_node(child, sets, now, changed, active);
    }
}
