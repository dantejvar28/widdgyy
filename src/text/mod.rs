// SPDX-FileCopyrightText: 2026 Daniel
// SPDX-License-Identifier: GPL-3.0-or-later

//! Text shaping and caching.
//!
//! Shaping is expensive, so results are cached by content + style. The cache is
//! bounded and cleared wholesale when it grows, which keeps memory predictable
//! without a background thread. Glyph runs are produced directly for Vello, so
//! painting a static text sticker does not allocate or shape anything again.

use std::collections::HashMap;
use std::sync::Arc;

use cosmic_text::{
    Attrs, Buffer, Family, FontSystem, Metrics, Shaping, Stretch, Style, Weight, Wrap,
};
use vello::Glyph;
use vello::peniko::FontData;

use crate::css::properties::text::{OverflowWrap, WordBreak};
use crate::css::properties::{FontStyle, TextAlign, TextOverflow, TextTransform, WhiteSpace};

const CACHE_LIMIT: usize = 512;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct CacheKey {
    text_hash: u64,
    font_size: u32,
    line_height: u32,
    weight: u16,
    style: u8,
    family_hash: u64,
    letter_spacing: u32,
    width: u32,
    align: u8,
    wrap: u8,
    overflow: u8,
}

#[derive(Clone, Debug)]
pub struct ShapedRun {
    pub font: FontData,
    pub font_id: cosmic_text::fontdb::ID,
    pub font_size: f32,
    pub baseline: f32,
    pub glyphs: Vec<Glyph>,
}

#[derive(Clone, Debug, Default)]
pub struct ShapedText {
    pub width: f32,
    pub height: f32,
    pub line_count: usize,
    pub runs: Vec<ShapedRun>,
    pub lines: Vec<ShapedLine>,
}

#[derive(Clone, Debug, Default)]
pub struct ShapedLine {
    pub top: f32,
    pub baseline: f32,
    pub height: f32,
    pub width: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct ShapeRequest<'a> {
    pub text: &'a str,
    pub font_size: f32,
    pub line_height: f32,
    pub families: &'a [String],
    pub weight: u16,
    pub style: FontStyle,
    pub letter_spacing: f32,
    pub max_width: Option<f32>,
    pub align: TextAlign,
    pub white_space: WhiteSpace,
    pub overflow_wrap: OverflowWrap,
    pub word_break: WordBreak,
    pub transform: TextTransform,
    pub text_overflow: TextOverflow,
}

#[derive(Clone, Debug)]
struct ResolvedFace {
    family: String,
    weight: Weight,
    style: Style,
    stretch: Stretch,
}

/// Pick the closest real face that belongs to the requested family.
///
/// Fontconfig-style family lookup in `cosmic-text` falls back to a different
/// family when the exact weight/style is missing. That is wrong for desktop
/// fonts such as "Angelic War", which only ship a Regular face: a
/// `font-weight: light` request would silently switch to Noto Sans. Here we
/// keep the family and use its closest available weight/style instead.
fn resolve_face(
    font_system: &FontSystem,
    families: &[String],
    requested_weight: u16,
    requested_style: FontStyle,
) -> ResolvedFace {
    let requested_style = match requested_style {
        FontStyle::Normal => Style::Normal,
        FontStyle::Italic => Style::Italic,
        FontStyle::Oblique => Style::Oblique,
    };
    let fallback_family = families
        .first()
        .cloned()
        .unwrap_or_else(|| "Sans".to_string());

    for family in families {
        let mut best: Option<(u32, Weight, Style, Stretch)> = None;
        for face in font_system.db().faces() {
            if !face
                .families
                .iter()
                .any(|(name, _)| name.eq_ignore_ascii_case(family))
            {
                continue;
            }
            let weight_diff = face.weight.0.abs_diff(requested_weight) as u32;
            let style_penalty = if face.style == requested_style {
                0
            } else {
                2_000
            };
            let score = weight_diff * 2 + style_penalty;
            if best
                .as_ref()
                .is_none_or(|(best_score, ..)| score < *best_score)
            {
                best = Some((score, face.weight, face.style, face.stretch));
            }
        }
        if let Some((_, weight, style, stretch)) = best {
            return ResolvedFace {
                family: family.clone(),
                weight,
                style,
                stretch,
            };
        }
    }

    ResolvedFace {
        family: fallback_family,
        weight: Weight(requested_weight),
        style: requested_style,
        stretch: Stretch::Normal,
    }
}

fn family_value(family: &str) -> Family<'_> {
    match family.to_ascii_lowercase().as_str() {
        "sans" | "sans-serif" | "system-ui" => Family::SansSerif,
        "serif" => Family::Serif,
        "monospace" | "mono" => Family::Monospace,
        "cursive" => Family::Cursive,
        "fantasy" => Family::Fantasy,
        _ => Family::Name(family),
    }
}

pub struct TextEngine {
    font_system: FontSystem,
    cache: HashMap<CacheKey, Arc<ShapedText>>,
    cache_hits: u64,
    cache_misses: u64,
}

impl Default for TextEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl TextEngine {
    pub fn new() -> Self {
        Self {
            font_system: FontSystem::new(),
            cache: HashMap::new(),
            cache_hits: 0,
            cache_misses: 0,
        }
    }

    pub fn load_font_data(&mut self, data: Vec<u8>) {
        self.font_system.db_mut().load_font_data(data);
        // New fonts invalidate any fallback decisions.
        self.cache.clear();
    }

    pub fn load_font_file(&mut self, path: impl AsRef<std::path::Path>) -> bool {
        let Ok(data) = std::fs::read(path) else {
            return false;
        };
        self.load_font_data(data);
        true
    }

    pub fn shape(&mut self, request: ShapeRequest<'_>) -> Arc<ShapedText> {
        let key = CacheKey {
            text_hash: hash_str(request.text),
            font_size: request.font_size.to_bits(),
            line_height: request.line_height.to_bits(),
            weight: request.weight,
            style: match request.style {
                FontStyle::Normal => 0,
                FontStyle::Italic => 1,
                FontStyle::Oblique => 2,
            },
            family_hash: request
                .families
                .first()
                .map(|family| hash_str(family))
                .unwrap_or(0),
            letter_spacing: request.letter_spacing.to_bits(),
            width: request.max_width.unwrap_or(-1.0).to_bits(),
            align: align_id(request.align),
            wrap: wrap_id(request),
            overflow: if request.text_overflow == TextOverflow::Ellipsis {
                1
            } else {
                0
            },
        };
        if let Some(cached) = self.cache.get(&key) {
            self.cache_hits += 1;
            return Arc::clone(cached);
        }
        self.cache_misses += 1;

        let text = apply_text_transform(request.text, request.transform);
        let metrics = Metrics::new(request.font_size.max(0.1), request.line_height.max(0.1));
        let mut buffer = Buffer::new(&mut self.font_system, metrics);
        let resolved = resolve_face(
            &self.font_system,
            request.families,
            request.weight,
            request.style,
        );
        let mut attrs = Attrs::new()
            .family(family_value(&resolved.family))
            .weight(resolved.weight)
            .style(resolved.style)
            .stretch(resolved.stretch);
        if request.letter_spacing != 0.0 {
            attrs = attrs.letter_spacing(request.letter_spacing / request.font_size.max(0.1));
        }
        buffer.set_wrap(wrap_mode(request));
        if request.text_overflow == TextOverflow::Ellipsis
            && request.max_width.is_some()
            && matches!(request.white_space, WhiteSpace::NoWrap | WhiteSpace::Pre)
        {
            buffer.set_ellipsize(cosmic_text::Ellipsize::End(
                cosmic_text::EllipsizeHeightLimit::Lines(1),
            ));
        }
        if let Some(width) = request.max_width {
            buffer.set_size(Some(width.max(0.0)), None);
        } else {
            buffer.set_size(None, None);
        }
        buffer.set_text(
            &text,
            &attrs,
            Shaping::Advanced,
            Some(cosmic_alignment(request.align)),
        );
        buffer.shape_until_scroll(&mut self.font_system, false);

        let mut shaped = ShapedText::default();
        let mut collected = Vec::new();
        let mut width: f32 = 0.0;
        let mut height: f32 = 0.0;
        let mut line_count = 0;
        for run in buffer.layout_runs() {
            line_count += 1;
            width = width.max(run.line_w);
            height = height.max(run.line_top + run.line_height);
            shaped.lines.push(ShapedLine {
                top: run.line_top,
                baseline: run.line_y,
                height: run.line_height,
                width: run.line_w,
            });
            for glyph in run.glyphs {
                collected.push((
                    glyph.font_id,
                    glyph.font_weight,
                    glyph.font_size,
                    run.line_y,
                    glyph.glyph_id,
                    glyph.x,
                    glyph.y,
                ));
            }
        }
        shaped.width = width;
        shaped.height = height;
        shaped.line_count = line_count;

        // Group consecutive glyphs by font so each Vello draw call has one font.
        let mut index = 0;
        while index < collected.len() {
            let (font_id, weight, font_size, baseline, _, _, _) = collected[index];
            let start = index;
            while index < collected.len() {
                let current = collected[index];
                if current.0 != font_id
                    || current.1 != weight
                    || (current.2 - font_size).abs() > 0.01
                    || (current.3 - baseline).abs() > 0.01
                {
                    break;
                }
                index += 1;
            }
            let Some(font) = self.font_system.get_font(font_id, weight) else {
                continue;
            };
            let font_data = font.as_peniko();
            let glyphs = collected[start..index]
                .iter()
                .map(|(_, _, _, _, id, x, y)| Glyph {
                    id: *id as u32,
                    x: *x,
                    y: *y,
                })
                .collect();
            shaped.runs.push(ShapedRun {
                font: font_data,
                font_id,
                font_size,
                baseline,
                glyphs,
            });
        }

        let shaped = Arc::new(shaped);
        if self.cache.len() >= CACHE_LIMIT {
            self.cache.clear();
        }
        self.cache.insert(key, Arc::clone(&shaped));
        shaped
    }

    pub fn measure(&mut self, request: ShapeRequest<'_>) -> (f32, f32) {
        let shaped = self.shape(request);
        (shaped.width, shaped.height)
    }

    pub fn cache_stats(&self) -> (u64, u64, usize) {
        (self.cache_hits, self.cache_misses, self.cache.len())
    }
}

fn align_id(align: TextAlign) -> u8 {
    match align {
        TextAlign::Left | TextAlign::Start => 0,
        TextAlign::Right | TextAlign::End => 1,
        TextAlign::Center => 2,
        TextAlign::Justify => 3,
    }
}

fn wrap_id(request: ShapeRequest<'_>) -> u8 {
    if request.max_width.is_none() {
        return 0;
    }
    if matches!(request.white_space, WhiteSpace::NoWrap | WhiteSpace::Pre) {
        return 0;
    }
    match (request.overflow_wrap, request.word_break) {
        (OverflowWrap::Anywhere, _) | (_, WordBreak::BreakAll) => 2,
        (OverflowWrap::BreakWord, _) | (_, WordBreak::BreakWord) => 3,
        _ => 1,
    }
}

fn wrap_mode(request: ShapeRequest<'_>) -> Wrap {
    if request.max_width.is_none()
        || matches!(request.white_space, WhiteSpace::NoWrap | WhiteSpace::Pre)
    {
        return Wrap::None;
    }
    match (request.overflow_wrap, request.word_break) {
        (OverflowWrap::Anywhere, _) | (_, WordBreak::BreakAll) => Wrap::Glyph,
        (OverflowWrap::BreakWord, _) | (_, WordBreak::BreakWord) => Wrap::WordOrGlyph,
        _ => Wrap::Word,
    }
}

fn cosmic_alignment(align: TextAlign) -> cosmic_text::Align {
    match align {
        TextAlign::Left | TextAlign::Start => cosmic_text::Align::Left,
        TextAlign::Right => cosmic_text::Align::Right,
        TextAlign::Center => cosmic_text::Align::Center,
        TextAlign::Justify => cosmic_text::Align::Justified,
        TextAlign::End => cosmic_text::Align::End,
    }
}

pub fn apply_text_transform(text: &str, transform: TextTransform) -> String {
    match transform {
        TextTransform::None => text.to_string(),
        TextTransform::Uppercase => text.to_uppercase(),
        TextTransform::Lowercase => text.to_lowercase(),
        TextTransform::Capitalize => text
            .split_inclusive(char::is_whitespace)
            .map(|word| {
                let mut chars = word.chars();
                match chars.next() {
                    Some(first) => {
                        let mut result = first.to_uppercase().collect::<String>();
                        result.push_str(chars.as_str());
                        result
                    }
                    None => String::new(),
                }
            })
            .collect(),
    }
}

fn hash_str(value: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transforms_capitalize_words() {
        assert_eq!(
            apply_text_transform("hello world", TextTransform::Capitalize),
            "Hello World"
        );
    }

    #[test]
    fn shapes_and_caches_text() {
        let mut engine = TextEngine::new();
        let families = vec!["Sans".to_string()];
        let request = ShapeRequest {
            text: "Hello",
            font_size: 20.0,
            line_height: 24.0,
            families: &families,
            weight: 400,
            style: FontStyle::Normal,
            letter_spacing: 0.0,
            max_width: None,
            align: TextAlign::Left,
            white_space: WhiteSpace::Normal,
            overflow_wrap: OverflowWrap::Normal,
            word_break: WordBreak::Normal,
            transform: TextTransform::None,
            text_overflow: TextOverflow::Clip,
        };
        let first = engine.shape(request);
        assert!(first.width > 0.0);
        assert!(!first.runs.is_empty());
        let second = engine.shape(request);
        assert!(Arc::ptr_eq(&first, &second));
        assert_eq!(engine.cache_stats().0, 1);
    }
}

#[cfg(test)]
mod face_resolution_tests {
    use super::*;

    #[test]
    fn keeps_family_when_weight_is_missing() {
        let font_system = FontSystem::new();
        let families = vec!["Angelic War".to_string()];
        let has_family = font_system
            .db()
            .faces()
            .any(|face| face.families.iter().any(|(name, _)| name == "Angelic War"));
        if !has_family {
            return;
        }
        let resolved = resolve_face(&font_system, &families, 300, FontStyle::Normal);
        assert_eq!(resolved.family, "Angelic War");
        assert_eq!(resolved.weight.0, 400);

        let mut engine = TextEngine::new();
        let shaped = engine.shape(ShapeRequest {
            text: "17.9°C",
            font_size: 32.0,
            line_height: 38.0,
            families: &families,
            weight: 300,
            style: FontStyle::Normal,
            letter_spacing: 0.0,
            max_width: None,
            align: TextAlign::Left,
            white_space: WhiteSpace::Normal,
            overflow_wrap: OverflowWrap::Normal,
            word_break: WordBreak::Normal,
            transform: TextTransform::None,
            text_overflow: TextOverflow::Clip,
        });
        let face = engine
            .font_system
            .db()
            .face(shaped.runs[0].font_id)
            .expect("shaped face");
        assert!(
            face.families.iter().any(|(name, _)| name == "Angelic War"),
            "{:?}",
            face.families
        );
    }
}
