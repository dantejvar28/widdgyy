// SPDX-FileCopyrightText: 2026 Daniel
// SPDX-License-Identifier: GPL-3.0-or-later

//! CSS animations.
//!
//! Keyframes are compiled to typed values when the stylesheet is applied. The
//! runtime only samples numeric values and writes an overlay into each node;
//! no CSS strings are parsed per frame. Transform/opacity/color animations do
//! not trigger layout, so they work with the cached video overlay path.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::time::{Duration, Instant};

use vello::peniko::Color;

use crate::css::Stylesheet;
use crate::css::properties::Declarations;
use crate::css::properties::transition::{
    Animation, AnimationDirection, AnimationFillMode, PlayState,
};
use crate::css::values::{self, Length, Shadow, TransformOp};
use crate::scene::{AnimatedStyle, Node};

#[derive(Clone, Debug, Default, PartialEq)]
struct FrameValues {
    opacity: Option<f32>,
    transform: Option<Vec<TransformOp>>,
    color: Option<Color>,
    background_color: Option<Color>,
    text_shadows: Option<Vec<Shadow>>,
    custom_properties: Option<BTreeMap<String, String>>,
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
    text_shadows: Vec<Shadow>,
    custom_properties: BTreeMap<String, String>,
}

#[derive(Clone, Copy, Debug, Default)]
struct FrameFields {
    opacity: bool,
    transform: bool,
    color: bool,
    background_color: bool,
    text_shadows: bool,
    custom_properties: bool,
}

struct CompiledAnimation {
    animation: Animation,
    frames: Vec<CompiledFrame>,
    fields: FrameFields,
    custom_keys: BTreeSet<String>,
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
            text_shadows: style.text.shadows.clone(),
            custom_properties: style.custom_properties.clone(),
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
                let mut custom = BTreeMap::new();
                for (name, value) in &frame.declarations {
                    let name = name.trim();
                    if name.starts_with("--") {
                        let mut lookup =
                            |variable: &str| style.custom_properties.get(variable).cloned();
                        let value = crate::css::parser::resolve_variables(value, &mut lookup, 0)
                            .unwrap_or_else(|| value.clone());
                        custom.insert(name.to_string(), value);
                        continue;
                    }
                    declarations.apply(name, value, base.color);
                }
                frames.push(CompiledFrame {
                    offset: frame.offset,
                    values: FrameValues {
                        opacity: declarations.opacity,
                        transform: declarations.transform,
                        color: declarations.color,
                        background_color: declarations.background_color,
                        text_shadows: declarations.text_shadow,
                        custom_properties: (!custom.is_empty()).then_some(custom),
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
            let custom_keys: BTreeSet<String> = frames
                .iter()
                .filter_map(|frame| frame.values.custom_properties.as_ref())
                .flat_map(|properties| properties.keys().cloned())
                .collect();
            let fields = FrameFields {
                opacity: frames.iter().any(|frame| frame.values.opacity.is_some()),
                transform: frames.iter().any(|frame| frame.values.transform.is_some()),
                color: frames.iter().any(|frame| frame.values.color.is_some()),
                background_color: frames
                    .iter()
                    .any(|frame| frame.values.background_color.is_some()),
                text_shadows: frames
                    .iter()
                    .any(|frame| frame.values.text_shadows.is_some()),
                custom_properties: !custom_keys.is_empty(),
            };
            animations.push(CompiledAnimation {
                animation: animation.clone(),
                frames,
                fields,
                custom_keys,
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
        return resolve_frame(before, base, compiled.fields, &compiled.custom_keys);
    }
    let local = (progress - before.offset) / (after.offset - before.offset);
    let easing = compiled.animation.timing.sample(local);
    let a = resolve_frame(before, base, compiled.fields, &compiled.custom_keys);
    let b = resolve_frame(after, base, compiled.fields, &compiled.custom_keys);
    FrameValues {
        opacity: lerp_option_f32(a.opacity, b.opacity, easing),
        transform: match (&a.transform, &b.transform) {
            (Some(a), Some(b)) => lerp_transforms(a, b, easing),
            _ => a.transform.or(b.transform),
        },
        color: lerp_option_color(a.color, b.color, easing),
        background_color: lerp_option_color(a.background_color, b.background_color, easing),
        text_shadows: match (&a.text_shadows, &b.text_shadows) {
            (Some(a), Some(b)) => lerp_shadows(a, b, easing),
            _ => a.text_shadows.or(b.text_shadows),
        },
        custom_properties: lerp_custom_properties(
            a.custom_properties.as_ref(),
            b.custom_properties.as_ref(),
            easing,
        ),
    }
}

fn resolve_frame(
    frame: &CompiledFrame,
    base: &BaseValues,
    fields: FrameFields,
    custom_keys: &BTreeSet<String>,
) -> FrameValues {
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
        text_shadows: fields.text_shadows.then(|| {
            frame
                .values
                .text_shadows
                .clone()
                .unwrap_or_else(|| base.text_shadows.clone())
        }),
        custom_properties: fields.custom_properties.then(|| {
            custom_keys
                .iter()
                .filter_map(|key| {
                    frame
                        .values
                        .custom_properties
                        .as_ref()
                        .and_then(|properties| properties.get(key))
                        .or_else(|| base.custom_properties.get(key))
                        .map(|value| (key.clone(), value.clone()))
                })
                .collect()
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
    if let Some(shadows) = &values.text_shadows {
        target.text_shadows = Some(shadows.clone());
    }
    if let Some(properties) = &values.custom_properties {
        target.custom_properties = Some(properties.clone());
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

/// Interpolate two custom-property maps key by key. Keys that only exist on
/// one side stay constant, which is the closest match to CSS's discrete
/// fallback for values that cannot be interpolated.
fn lerp_custom_properties(
    a: Option<&BTreeMap<String, String>>,
    b: Option<&BTreeMap<String, String>>,
    t: f32,
) -> Option<BTreeMap<String, String>> {
    match (a, b) {
        (None, None) => None,
        (Some(a), None) => Some(a.clone()),
        (None, Some(b)) => Some(b.clone()),
        (Some(a), Some(b)) => {
            let keys: BTreeSet<&String> = a.keys().chain(b.keys()).collect();
            Some(
                keys.into_iter()
                    .filter_map(|key| match (a.get(key), b.get(key)) {
                        (Some(a), Some(b)) => Some((key.clone(), lerp_custom_value(a, b, t))),
                        (Some(value), None) | (None, Some(value)) => {
                            Some((key.clone(), value.clone()))
                        }
                        (None, None) => None,
                    })
                    .collect(),
            )
        }
    }
}

fn lerp_custom_value(a: &str, b: &str, t: f32) -> String {
    let t = t.clamp(0.0, 1.0);
    if let (Ok(a), Ok(b)) = (a.trim().parse::<f32>(), b.trim().parse::<f32>()) {
        return format!("{:.4}", a + (b - a) * t);
    }
    if let (Ok(a), Ok(b)) = (Length::parse(a), Length::parse(b))
        && let Some(value) = lerp_length_value(&a, &b, t)
    {
        return value;
    }
    if let (Some(a), Some(b)) = (
        values::color::parse(a, Color::BLACK),
        values::color::parse(b, Color::BLACK),
    ) {
        let color = values::color::mix(a, b, t);
        let [r, g, b, a] = color.components;
        return format!(
            "rgba({:.0}, {:.0}, {:.0}, {:.3})",
            r * 255.0,
            g * 255.0,
            b * 255.0,
            a
        );
    }
    if t < 0.5 {
        a.to_string()
    } else {
        b.to_string()
    }
}

fn lerp_length_value(a: &Length, b: &Length, t: f32) -> Option<String> {
    let px = |a: f32, b: f32| format!("{:.2}px", a + (b - a) * t);
    let percent = |a: f32, b: f32| format!("{:.2}%", a + (b - a) * t);
    Some(match (a, b) {
        (Length::Zero, Length::Zero) => "0".to_string(),
        (Length::Zero, Length::Px(b)) => px(0.0, *b),
        (Length::Px(a), Length::Zero) => px(*a, 0.0),
        (Length::Px(a), Length::Px(b)) => px(*a, *b),
        (Length::Percent(a), Length::Percent(b)) => percent(*a, *b),
        (Length::Em(a), Length::Em(b)) => format!("{:.2}em", a + (b - a) * t),
        (Length::Rem(a), Length::Rem(b)) => format!("{:.2}rem", a + (b - a) * t),
        (Length::Vw(a), Length::Vw(b)) => format!("{:.2}vw", a + (b - a) * t),
        (Length::Vh(a), Length::Vh(b)) => format!("{:.2}vh", a + (b - a) * t),
        _ => return None,
    })
}

fn lerp_shadows(a: &[Shadow], b: &[Shadow], t: f32) -> Option<Vec<Shadow>> {
    if a.is_empty() || b.is_empty() {
        return None;
    }
    // CSS repeats the shorter shadow list when lengths differ. This is what
    // makes a single soft shadow animate smoothly into a three-layer glow.
    let length = a.len().max(b.len());
    Some(
        (0..length)
            .map(|index| {
                let a = &a[index % a.len()];
                let b = &b[index % b.len()];
                Shadow {
                    offset_x: lerp_length(&a.offset_x, &b.offset_x, t),
                    offset_y: lerp_length(&a.offset_y, &b.offset_y, t),
                    blur: lerp_length(&a.blur, &b.blur, t),
                    spread: lerp_length(&a.spread, &b.spread, t),
                    color: values::color::mix(a.color, b.color, t),
                    inset: if t < 0.5 { a.inset } else { b.inset },
                }
            })
            .collect(),
    )
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

/// Update animated overlays. Returns `(overlay_changed, glow_changed, active)`.
/// Custom-property changes only affect the composited glow mask, so they do
/// not force the text overlay to be repainted.
pub fn update_scene(
    scene: &mut crate::scene::Scene,
    sets: &HashMap<u64, AnimationSet>,
    now: Instant,
) -> (bool, bool, bool) {
    let mut changed = false;
    let mut glow_changed = false;
    let mut active = false;
    for root in &mut scene.roots {
        update_node(
            root,
            sets,
            now,
            &mut changed,
            &mut glow_changed,
            &mut active,
        );
    }
    (changed, glow_changed, active)
}

fn update_node(
    node: &mut Node,
    sets: &HashMap<u64, AnimationSet>,
    now: Instant,
    changed: &mut bool,
    glow_changed: &mut bool,
    active: &mut bool,
) {
    if let Some(set) = sets.get(&node.uid) {
        let (overlay, is_active) = set.sample(now);
        *active |= is_active;
        if node.animated != overlay {
            if glow_only_change(&node.animated, &overlay) {
                *glow_changed = true;
            } else {
                *changed = true;
            }
            node.animated = overlay;
        }
    }
    for child in &mut node.children {
        update_node(child, sets, now, changed, glow_changed, active);
    }
}

fn glow_only_change(current: &AnimatedStyle, next: &AnimatedStyle) -> bool {
    current.opacity == next.opacity
        && current.transform == next.transform
        && current.color == next.color
        && current.background_color == next.background_color
        && (current.text_shadows != next.text_shadows
            || current.custom_properties != next.custom_properties)
}
