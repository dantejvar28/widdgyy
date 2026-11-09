// SPDX-FileCopyrightText: 2026 Daniel
// SPDX-License-Identifier: GPL-3.0-or-later

//! Stylesheet storage and the cascade.

use std::collections::{BTreeMap, HashMap};

use vello::peniko::Color;

use super::Viewport;
use super::computed::ComputedStyle;
use super::parser::{self, RawDeclaration};
use super::preferences::{ColorScheme, DisplayMode, Hover, Pointer, Preferences};
use super::properties::Declarations;
use super::properties::transition::{self, Keyframes};
use super::selector::{Element, PseudoElement, Selector, Specificity, parse_selector_list};
use super::values::Length;

#[derive(Clone, Debug, PartialEq)]
pub struct FontFace {
    pub family: String,
    pub source: String,
    pub weight: u16,
    pub style: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MediaCondition {
    pub min_width: Option<f32>,
    pub max_width: Option<f32>,
    pub min_height: Option<f32>,
    pub max_height: Option<f32>,
    pub orientation: Option<Orientation>,
    pub prefers_color_scheme: Option<ColorScheme>,
    /// `true` means `reduce`, `false` means `no-preference`.
    pub prefers_reduced_motion: Option<bool>,
    pub min_resolution: Option<f32>,
    pub max_resolution: Option<f32>,
    pub min_aspect_ratio: Option<f32>,
    pub max_aspect_ratio: Option<f32>,
    pub hover: Option<Hover>,
    pub pointer: Option<Pointer>,
    pub any_hover: Option<Hover>,
    pub any_pointer: Option<Pointer>,
    pub display_mode: Option<DisplayMode>,
    pub media_type_matches: bool,
    pub negate: bool,
    pub unknown: bool,
}

impl Default for MediaCondition {
    fn default() -> Self {
        Self {
            min_width: None,
            max_width: None,
            min_height: None,
            max_height: None,
            orientation: None,
            prefers_color_scheme: None,
            prefers_reduced_motion: None,
            min_resolution: None,
            max_resolution: None,
            min_aspect_ratio: None,
            max_aspect_ratio: None,
            hover: None,
            pointer: None,
            any_hover: None,
            any_pointer: None,
            display_mode: None,
            media_type_matches: true,
            negate: false,
            unknown: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Orientation {
    Portrait,
    Landscape,
}

impl MediaCondition {
    pub fn matches(&self, viewport: Viewport, preferences: Preferences) -> bool {
        let mut result = self.media_type_matches && !self.unknown;

        if result && self.min_width.is_some_and(|value| viewport.width < value) {
            result = false;
        }
        if result && self.max_width.is_some_and(|value| viewport.width > value) {
            result = false;
        }
        if result && self.min_height.is_some_and(|value| viewport.height < value) {
            result = false;
        }
        if result && self.max_height.is_some_and(|value| viewport.height > value) {
            result = false;
        }
        if result && let Some(orientation) = self.orientation {
            let current = if viewport.width >= viewport.height {
                Orientation::Landscape
            } else {
                Orientation::Portrait
            };
            result = current == orientation;
        }
        if result && let Some(scheme) = self.prefers_color_scheme {
            result = scheme == preferences.color_scheme;
        }
        if result && let Some(reduced) = self.prefers_reduced_motion {
            result = reduced == preferences.reduced_motion;
        }
        if result {
            let resolution = viewport.scale_factor.max(0.01);
            if self.min_resolution.is_some_and(|value| resolution < value)
                || self.max_resolution.is_some_and(|value| resolution > value)
            {
                result = false;
            }
        }
        if result {
            let ratio = if viewport.height > 0.0 {
                viewport.width / viewport.height
            } else {
                0.0
            };
            if self.min_aspect_ratio.is_some_and(|value| ratio < value)
                || self.max_aspect_ratio.is_some_and(|value| ratio > value)
            {
                result = false;
            }
        }
        if result && let Some(hover) = self.hover {
            result = hover == preferences.hover;
        }
        if result && let Some(pointer) = self.pointer {
            result = pointer == preferences.pointer;
        }
        if result && let Some(hover) = self.any_hover {
            result = hover == preferences.any_hover;
        }
        if result && let Some(pointer) = self.any_pointer {
            result = pointer == preferences.any_pointer;
        }
        if result && let Some(mode) = self.display_mode {
            result = mode == preferences.display_mode;
        }

        if self.negate { !result } else { result }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct MediaQuery {
    /// A comma separated list is an OR of conditions.
    pub conditions: Vec<MediaCondition>,
}

impl MediaQuery {
    pub fn parse(input: &str) -> Self {
        let mut conditions = Vec::new();
        for part in input.split(',') {
            let mut condition = MediaCondition::default();
            let mut text = part.trim();
            if let Some(rest) = text.strip_prefix("not ") {
                condition.negate = true;
                text = rest.trim();
            }
            text = text.strip_prefix("only ").unwrap_or(text).trim();

            let type_end = text
                .find(|ch: char| ch == '(' || ch.is_whitespace())
                .unwrap_or(text.len());
            let media_type = text[..type_end].trim().to_ascii_lowercase();
            if !media_type.is_empty() {
                condition.media_type_matches = matches!(media_type.as_str(), "screen" | "all");
                let rest = text[type_end..].trim_start();
                text = rest.strip_prefix("and ").unwrap_or(rest).trim();
            }

            let mut remaining = text.to_string();
            while let Some(open) = remaining.find('(') {
                let Some(close) = remaining[open..].find(')').map(|value| value + open) else {
                    condition.unknown = true;
                    break;
                };
                let feature = remaining[open + 1..close].trim().to_ascii_lowercase();
                remaining = remaining[close + 1..].to_string();
                let (name, value) = feature
                    .split_once(':')
                    .map(|(name, value)| (name.trim(), value.trim()))
                    .unwrap_or((feature.as_str(), ""));
                match name {
                    "min-width" => match parse_px(value) {
                        Some(parsed) => condition.min_width = Some(parsed),
                        None => condition.unknown = true,
                    },
                    "max-width" => match parse_px(value) {
                        Some(parsed) => condition.max_width = Some(parsed),
                        None => condition.unknown = true,
                    },
                    "min-height" => match parse_px(value) {
                        Some(parsed) => condition.min_height = Some(parsed),
                        None => condition.unknown = true,
                    },
                    "max-height" => match parse_px(value) {
                        Some(parsed) => condition.max_height = Some(parsed),
                        None => condition.unknown = true,
                    },
                    "orientation" => {
                        condition.orientation = Some(if value == "portrait" {
                            Orientation::Portrait
                        } else if value == "landscape" {
                            Orientation::Landscape
                        } else {
                            condition.unknown = true;
                            Orientation::Landscape
                        })
                    }
                    "prefers-color-scheme" => {
                        condition.prefers_color_scheme = ColorScheme::parse(value);
                        if condition.prefers_color_scheme.is_none() {
                            condition.unknown = true;
                        }
                    }
                    "prefers-reduced-motion" => {
                        condition.prefers_reduced_motion = match value {
                            "reduce" => Some(true),
                            "no-preference" => Some(false),
                            _ => {
                                condition.unknown = true;
                                None
                            }
                        }
                    }
                    "min-resolution" => match parse_resolution(value) {
                        Some(parsed) => condition.min_resolution = Some(parsed),
                        None => condition.unknown = true,
                    },
                    "max-resolution" => match parse_resolution(value) {
                        Some(parsed) => condition.max_resolution = Some(parsed),
                        None => condition.unknown = true,
                    },
                    "resolution" => match parse_resolution(value) {
                        Some(parsed) => {
                            condition.min_resolution = Some(parsed);
                            condition.max_resolution = Some(parsed);
                        }
                        None => condition.unknown = true,
                    },
                    "min-aspect-ratio" => match parse_ratio(value) {
                        Some(parsed) => condition.min_aspect_ratio = Some(parsed),
                        None => condition.unknown = true,
                    },
                    "max-aspect-ratio" => match parse_ratio(value) {
                        Some(parsed) => condition.max_aspect_ratio = Some(parsed),
                        None => condition.unknown = true,
                    },
                    "aspect-ratio" => match parse_ratio(value) {
                        Some(parsed) => {
                            condition.min_aspect_ratio = Some(parsed);
                            condition.max_aspect_ratio = Some(parsed);
                        }
                        None => condition.unknown = true,
                    },
                    "hover" => match Hover::parse(value) {
                        Some(parsed) => condition.hover = Some(parsed),
                        None => condition.unknown = true,
                    },
                    "pointer" => match Pointer::parse(value) {
                        Some(parsed) => condition.pointer = Some(parsed),
                        None => condition.unknown = true,
                    },
                    "any-hover" => match Hover::parse(value) {
                        Some(parsed) => condition.any_hover = Some(parsed),
                        None => condition.unknown = true,
                    },
                    "any-pointer" => match Pointer::parse(value) {
                        Some(parsed) => condition.any_pointer = Some(parsed),
                        None => condition.unknown = true,
                    },
                    "display-mode" => match DisplayMode::parse(value) {
                        Some(parsed) => condition.display_mode = Some(parsed),
                        None => condition.unknown = true,
                    },
                    _ => condition.unknown = true,
                }
            }
            conditions.push(condition);
        }
        if conditions.is_empty() {
            conditions.push(MediaCondition::default());
        }
        Self { conditions }
    }

    pub fn matches(&self, viewport: Viewport, preferences: Preferences) -> bool {
        self.conditions
            .iter()
            .any(|condition| condition.matches(viewport, preferences))
    }
}

fn parse_px(value: &str) -> Option<f32> {
    Length::parse(value).ok().map(|length| {
        let context = crate::css::ResolveContext {
            viewport_width: 1920.0,
            viewport_height: 1080.0,
            font_size: 16.0,
            root_font_size: 16.0,
            basis_width: 1920.0,
            basis_height: 1080.0,
        };
        length.resolve(&context, crate::css::Axis::Horizontal)
    })
}

fn parse_resolution(value: &str) -> Option<f32> {
    let value = value.trim().to_ascii_lowercase();
    if let Some(number) = value
        .strip_suffix("dppx")
        .or_else(|| value.strip_suffix('x'))
    {
        return number.trim().parse().ok();
    }
    if let Some(number) = value.strip_suffix("dpi") {
        return number.trim().parse::<f32>().ok().map(|value| value / 96.0);
    }
    if let Some(number) = value.strip_suffix("dpcm") {
        return number
            .trim()
            .parse::<f32>()
            .ok()
            .map(|value| value * 2.54 / 96.0);
    }
    None
}

fn parse_ratio(value: &str) -> Option<f32> {
    let value = value.trim();
    if let Some((width, height)) = value.split_once('/') {
        let width = width.trim().parse::<f32>().ok()?;
        let height = height.trim().parse::<f32>().ok()?;
        (height > 0.0).then_some(width / height)
    } else {
        value.parse().ok()
    }
}

#[derive(Clone, Debug)]
pub struct StyleRule {
    pub selectors: Vec<Selector>,
    pub declarations: Vec<RawDeclaration>,
    pub media: Option<MediaQuery>,
    pub order: usize,
    pub source: String,
}

#[derive(Clone, Debug, Default)]
pub struct Stylesheet {
    pub rules: Vec<StyleRule>,
    pub keyframes: HashMap<String, Keyframes>,
    pub font_faces: Vec<FontFace>,
    pub errors: Vec<String>,
    /// Properties seen in the stylesheet that the engine does not implement.
    pub unsupported_properties: BTreeMap<String, usize>,
    /// At-rules that were skipped.
    pub unsupported_at_rules: Vec<String>,
}

impl Stylesheet {
    pub fn parse(css: &str) -> Self {
        let mut stylesheet = Self::default();
        if css.to_ascii_lowercase().contains("@import") {
            stylesheet
                .errors
                .push("@import is not supported; inline the imported file".to_string());
        }
        let mut order = 0;
        stylesheet.parse_blocks(css, None, &mut order);
        stylesheet
    }

    fn parse_blocks(&mut self, css: &str, media: Option<&MediaQuery>, order: &mut usize) {
        for block in parser::split_blocks(css) {
            let prelude = block.prelude.trim();
            if prelude.starts_with('@') {
                self.parse_at_rule(prelude, &block.body, media, order);
                continue;
            }

            let selectors = match parse_selector_list(prelude) {
                Ok(selectors) => selectors,
                Err(error) => {
                    self.errors.push(format!("{prelude}: {error}"));
                    continue;
                }
            };
            let declarations = parser::parse_raw_declarations(&block.body);
            if declarations.is_empty() {
                continue;
            }
            for declaration in &declarations {
                if declaration.name.starts_with("--") {
                    continue;
                }
                if !declaration_supported(&declaration.name, &declaration.value) {
                    *self
                        .unsupported_properties
                        .entry(declaration.name.clone())
                        .or_default() += 1;
                }
            }
            self.rules.push(StyleRule {
                selectors,
                declarations,
                media: media.cloned(),
                order: *order,
                source: prelude.to_string(),
            });
            *order += 1;
        }
    }

    fn parse_at_rule(
        &mut self,
        prelude: &str,
        body: &str,
        inherited_media: Option<&MediaQuery>,
        order: &mut usize,
    ) {
        let lower = prelude.to_ascii_lowercase();
        if let Some(condition) = lower.strip_prefix("@media") {
            let query = MediaQuery::parse(condition.trim());
            self.parse_blocks(body, Some(&query), order);
        } else if let Some(condition) = prelude.strip_prefix("@supports") {
            if supports_condition(condition) {
                self.parse_blocks(body, inherited_media, order);
            }
        } else if let Some(name) = lower
            .strip_prefix("@keyframes")
            .or_else(|| lower.strip_prefix("@-webkit-keyframes"))
        {
            let name = name.trim().trim_matches('"').trim_matches('\'');
            let frames = transition::parse_keyframes(name, body);
            self.keyframes.insert(name.to_string(), frames);
        } else if lower.starts_with("@font-face") {
            let declarations = parser::parse_declarations(body);
            let mut family = None;
            let mut source = None;
            let mut weight = 400;
            let mut style = "normal".to_string();
            for (name, value) in declarations {
                match name.as_str() {
                    "font-family" => {
                        family = Some(
                            value
                                .trim()
                                .trim_matches('"')
                                .trim_matches('\'')
                                .to_string(),
                        )
                    }
                    "src" => {
                        source = value.split(',').find_map(|part| {
                            let part = part.trim();
                            let start = part.to_ascii_lowercase().find("url(")?;
                            let rest = &part[start + 4..];
                            let end = rest.find(')')?;
                            Some(
                                rest[..end]
                                    .trim()
                                    .trim_matches('"')
                                    .trim_matches('\'')
                                    .to_string(),
                            )
                        })
                    }
                    "font-weight" => {
                        weight = value.trim().parse::<u16>().unwrap_or(400);
                    }
                    "font-style" => style = value.trim().to_ascii_lowercase(),
                    _ => {}
                }
            }
            if let (Some(family), Some(source)) = (family, source) {
                self.font_faces.push(FontFace {
                    family,
                    source,
                    weight,
                    style,
                });
            }
        } else if lower.starts_with("@import") {
            self.errors
                .push("@import is not supported; inline the imported file".to_string());
        } else if lower.starts_with("@layer") {
            self.parse_blocks(body, inherited_media, order);
        } else {
            self.unsupported_at_rules.push(prelude.to_string());
        }
    }

    pub fn rules_for<'a>(
        &'a self,
        element: &Element<'_>,
        ancestors: &[Element<'_>],
        viewport: Viewport,
        preferences: Preferences,
        pseudo: Option<PseudoElement>,
    ) -> Vec<(&'a StyleRule, Specificity)> {
        let mut matched = Vec::new();
        for rule in &self.rules {
            if let Some(media) = &rule.media
                && !media.matches(viewport, preferences)
            {
                continue;
            }
            let mut best: Option<Specificity> = None;
            for selector in &rule.selectors {
                if selector.pseudo_element() != pseudo {
                    continue;
                }
                if selector.matches(element, ancestors) {
                    best = Some(match best {
                        Some(current) if current.key() >= selector.specificity.key() => current,
                        _ => selector.specificity,
                    });
                }
            }
            if let Some(specificity) = best {
                matched.push((rule, specificity));
            }
        }
        matched
    }
}

/// Applies a [`Stylesheet`] to scene elements.
pub struct Cascade<'a> {
    pub stylesheet: &'a Stylesheet,
    pub viewport: Viewport,
    pub preferences: Preferences,
}

impl<'a> Cascade<'a> {
    /// Browser-like defaults: light, no reduced motion, no input. Used by
    /// tests and tools that do not probe the desktop.
    pub fn new(stylesheet: &'a Stylesheet, viewport: Viewport) -> Self {
        Self {
            stylesheet,
            viewport,
            preferences: Preferences::default(),
        }
    }

    pub fn with_preferences(
        stylesheet: &'a Stylesheet,
        viewport: Viewport,
        preferences: Preferences,
    ) -> Self {
        Self {
            stylesheet,
            viewport,
            preferences,
        }
    }

    pub fn compute(
        &self,
        element: &Element<'_>,
        ancestors: &[Element<'_>],
        parent: Option<&ComputedStyle>,
        inline: &[RawDeclaration],
        defaults: &[RawDeclaration],
    ) -> ComputedStyle {
        self.compute_filtered(element, ancestors, parent, inline, defaults, None)
    }

    /// Compute the style of a generated `::before`/`::after` box. Only rules
    /// whose subject targets that pseudo-element participate; inherited
    /// properties come from the originating element.
    pub fn compute_pseudo(
        &self,
        element: &Element<'_>,
        ancestors: &[Element<'_>],
        pseudo: PseudoElement,
        parent: &ComputedStyle,
    ) -> ComputedStyle {
        self.compute_filtered(element, ancestors, Some(parent), &[], &[], Some(pseudo))
    }

    #[allow(clippy::too_many_arguments)]
    fn compute_filtered(
        &self,
        element: &Element<'_>,
        ancestors: &[Element<'_>],
        parent: Option<&ComputedStyle>,
        inline: &[RawDeclaration],
        defaults: &[RawDeclaration],
        pseudo: Option<PseudoElement>,
    ) -> ComputedStyle {
        let mut matched: Vec<Matched<'_>> = self
            .stylesheet
            .rules_for(element, ancestors, self.viewport, self.preferences, pseudo)
            .into_iter()
            .flat_map(|(rule, specificity)| {
                rule.declarations.iter().map(move |declaration| Matched {
                    declaration,
                    specificity,
                    order: rule.order,
                    inline: false,
                })
            })
            .collect();

        let inline_specificity = Specificity {
            ids: u32::MAX / 4,
            classes: u32::MAX / 4,
            types: u32::MAX / 4,
        };
        for declaration in inline {
            matched.push(Matched {
                declaration,
                specificity: inline_specificity,
                order: usize::MAX / 2,
                inline: true,
            });
        }

        matched.sort_by(|a, b| {
            cascade_key(a)
                .cmp(&cascade_key(b))
                .then_with(|| a.order.cmp(&b.order))
                .then_with(|| a.declaration.name.cmp(&b.declaration.name))
        });

        let mut custom: BTreeMap<String, String> = parent
            .map(|parent| parent.custom_properties.clone())
            .unwrap_or_default();

        for declaration in defaults {
            if !declaration.name.starts_with("--") {
                continue;
            }
            let resolved = {
                let snapshot = &custom;
                parser::resolve_variables(
                    &declaration.value,
                    &mut |name| snapshot.get(name).cloned(),
                    0,
                )
            };
            if let Some(value) = resolved {
                custom.insert(declaration.name.clone(), value);
            }
        }

        for matched in &matched {
            let declaration = matched.declaration;
            if !declaration.name.starts_with("--") {
                continue;
            }
            let resolved = {
                let snapshot = &custom;
                parser::resolve_variables(
                    &declaration.value,
                    &mut |name| snapshot.get(name).cloned(),
                    0,
                )
            };
            if let Some(value) = resolved {
                custom.insert(declaration.name.clone(), value);
            }
        }

        let parent_color = parent
            .map(|parent| parent.text.color)
            .unwrap_or_else(|| Color::new([1.0, 1.0, 1.0, 1.0]));

        // Resolve `currentColor` before applying the rest of the declarations.
        let mut current_color = parent_color;
        for declaration in defaults {
            if declaration.name != "color" {
                continue;
            }
            let resolved = parser::resolve_variables(
                &declaration.value,
                &mut |name| custom.get(name).cloned(),
                0,
            );
            if let Some(value) = resolved
                && let Some(color) = super::values::color::parse(&value, parent_color)
            {
                current_color = color;
            }
        }
        for matched in &matched {
            let declaration = matched.declaration;
            if declaration.name != "color" {
                continue;
            }
            let resolved = parser::resolve_variables(
                &declaration.value,
                &mut |name| custom.get(name).cloned(),
                0,
            );
            if let Some(value) = resolved
                && let Some(color) = super::values::color::parse(&value, parent_color)
            {
                current_color = color;
            }
        }

        let mut declarations = Declarations::default();
        for declaration in defaults {
            if declaration.name.starts_with("--") {
                continue;
            }
            let resolved = parser::resolve_variables(
                &declaration.value,
                &mut |name| custom.get(name).cloned(),
                0,
            );
            if let Some(value) = resolved {
                declarations.apply(&declaration.name, &value, current_color);
            }
        }
        for matched in &matched {
            let declaration = matched.declaration;
            if declaration.name.starts_with("--") {
                continue;
            }
            let resolved = parser::resolve_variables(
                &declaration.value,
                &mut |name| custom.get(name).cloned(),
                0,
            );
            if let Some(value) = resolved {
                declarations.apply(&declaration.name, &value, current_color);
            } else {
                log::debug!(
                    "unresolved variable in {}: {}",
                    declaration.name,
                    declaration.value
                );
            }
        }

        ComputedStyle::compute(&declarations, parent, self.viewport, custom)
    }
}

struct Matched<'a> {
    declaration: &'a RawDeclaration,
    specificity: Specificity,
    order: usize,
    inline: bool,
}

fn cascade_key(matched: &Matched<'_>) -> (bool, u64, bool) {
    (
        matched.declaration.important,
        matched.specificity.key(),
        matched.inline,
    )
}

/// Parse an inline style attribute (`"color: red; font-size: 12px"`).
pub fn parse_inline_style(value: &str) -> Vec<RawDeclaration> {
    parser::parse_raw_declarations(value)
}

/// Check whether the engine knows a property/value pair.
pub fn declaration_supported(name: &str, value: &str) -> bool {
    let mut declarations = Declarations::default();
    declarations.apply(name, value, Color::new([1.0, 1.0, 1.0, 1.0]))
}

fn supports_condition(condition: &str) -> bool {
    let text = condition.trim();
    let text = text
        .strip_prefix("not ")
        .map(|rest| (true, rest.trim()))
        .or_else(|| {
            text.strip_prefix("not(")
                .map(|rest| (true, rest.trim_end_matches(')').trim()))
        })
        .unwrap_or((false, text));

    let (negate, text) = text;
    let result = if let Some((left, right)) = split_supports(text, " and ") {
        supports_condition(left) && supports_condition(right)
    } else if let Some((left, right)) = split_supports(text, " or ") {
        supports_condition(left) || supports_condition(right)
    } else if let Some(inner) = strip_outer_parens(text) {
        let inner = inner.trim();
        if let Some(selector) = inner.strip_prefix("selector(") {
            parse_selector_list(selector.trim_end_matches(')')).is_ok()
        } else if let Some((name, value)) = inner.split_once(':') {
            declaration_supported(name.trim(), value.trim())
        } else {
            true
        }
    } else {
        true
    };
    if negate { !result } else { result }
}

fn split_supports<'a>(text: &'a str, keyword: &str) -> Option<(&'a str, &'a str)> {
    let mut depth = 0_i32;
    let mut index = 0;
    while index + keyword.len() <= text.len() {
        let ch = text[index..].chars().next()?;
        match ch {
            '(' => depth += 1,
            ')' => depth -= 1,
            _ => {}
        }
        if depth == 0 && text[index..].starts_with(keyword) {
            return Some((&text[..index], &text[index + keyword.len()..]));
        }
        index += ch.len_utf8();
    }
    None
}

fn strip_outer_parens(text: &str) -> Option<&str> {
    let text = text.trim();
    if !text.starts_with('(') {
        return None;
    }
    let close = parser::find_matching(text, 0, '(', ')')?;
    if close + 1 != text.len() {
        return None;
    }
    Some(&text[1..close])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn element<'a>(tag: &'a str, classes: &'a [String]) -> Element<'a> {
        Element::new(tag, None, classes, 0, 1)
    }

    #[test]
    fn parses_media_query() {
        let query = MediaQuery::parse("screen and (min-width: 800px) and (orientation: landscape)");
        let preferences = Preferences::default();
        assert!(query.matches(
            Viewport {
                width: 1000.0,
                height: 500.0,
                ..Viewport::default()
            },
            preferences
        ));
        assert!(!query.matches(
            Viewport {
                width: 500.0,
                height: 1000.0,
                ..Viewport::default()
            },
            preferences
        ));
    }

    #[test]
    fn media_color_scheme_uses_preferences() {
        let query = MediaQuery::parse("(prefers-color-scheme: dark)");
        let viewport = Viewport::default();
        assert!(!query.matches(viewport, Preferences::default()));
        let dark = Preferences {
            color_scheme: ColorScheme::Dark,
            ..Preferences::default()
        };
        assert!(query.matches(viewport, dark));
    }

    #[test]
    fn media_reduced_motion_uses_preferences() {
        let reduce = MediaQuery::parse("(prefers-reduced-motion: reduce)");
        let no_preference = MediaQuery::parse("(prefers-reduced-motion: no-preference)");
        let viewport = Viewport::default();
        assert!(!reduce.matches(viewport, Preferences::default()));
        assert!(no_preference.matches(viewport, Preferences::default()));
        let reduced = Preferences {
            reduced_motion: true,
            ..Preferences::default()
        };
        assert!(reduce.matches(viewport, reduced));
        assert!(!no_preference.matches(viewport, reduced));
    }

    #[test]
    fn media_resolution_and_aspect_ratio() {
        let viewport = Viewport {
            width: 1600.0,
            height: 900.0,
            scale_factor: 2.0,
        };
        let preferences = Preferences::default();
        assert!(MediaQuery::parse("(min-resolution: 2dppx)").matches(viewport, preferences));
        assert!(MediaQuery::parse("(resolution: 192dpi)").matches(viewport, preferences));
        assert!(!MediaQuery::parse("(min-resolution: 3dppx)").matches(viewport, preferences));
        assert!(MediaQuery::parse("(aspect-ratio: 16/9)").matches(viewport, preferences));
        assert!(MediaQuery::parse("(min-aspect-ratio: 4/3)").matches(viewport, preferences));
        assert!(!MediaQuery::parse("(max-aspect-ratio: 3/2)").matches(viewport, preferences));
    }

    #[test]
    fn media_input_and_display_mode() {
        let viewport = Viewport::default();
        let preferences = Preferences::default();
        assert!(
            MediaQuery::parse("(hover: none) and (pointer: none) and (display-mode: fullscreen)")
                .matches(viewport, preferences)
        );
        assert!(!MediaQuery::parse("(hover: hover)").matches(viewport, preferences));
        assert!(!MediaQuery::parse("(pointer: fine)").matches(viewport, preferences));
        let fine = Preferences {
            pointer: Pointer::Fine,
            ..Preferences::default()
        };
        assert!(MediaQuery::parse("(pointer: fine)").matches(viewport, fine));
    }

    #[test]
    fn media_negation_types_and_unknown_features() {
        let viewport = Viewport::default();
        let preferences = Preferences::default();
        assert!(!MediaQuery::parse("print").matches(viewport, preferences));
        assert!(MediaQuery::parse("screen").matches(viewport, preferences));
        assert!(
            MediaQuery::parse("only screen and (min-width: 100px)").matches(viewport, preferences)
        );
        assert!(
            MediaQuery::parse("not screen and (min-width: 9999px)").matches(viewport, preferences)
        );
        assert!(
            !MediaQuery::parse("not screen and (min-width: 100px)").matches(viewport, preferences)
        );
        assert!(!MediaQuery::parse("(scripting: enabled)").matches(viewport, preferences));
    }

    #[test]
    fn cascade_applies_preference_media_queries() {
        let css = r#"
            .a { color: #000000; }
            @media (prefers-color-scheme: dark) {
                .a { color: #ffffff; }
            }
        "#;
        let sheet = Stylesheet::parse(css);
        let classes = vec!["a".to_string()];
        let element = element("text", &classes);
        let dark = Preferences {
            color_scheme: ColorScheme::Dark,
            ..Preferences::default()
        };
        let dark_cascade = Cascade::with_preferences(&sheet, Viewport::default(), dark);
        let dark_style = dark_cascade.compute(&element, &[], None, &[], &[]);
        assert!(dark_style.text.color.components[0] > 0.9);

        let light_cascade = Cascade::new(&sheet, Viewport::default());
        let light_style = light_cascade.compute(&element, &[], None, &[], &[]);
        assert!(light_style.text.color.components[0] < 0.1);
    }

    #[test]
    fn cascade_respects_specificity_and_important() {
        let css = r#"
            text { color: #ff0000; }
            .a { color: #00ff00; }
            .a { color: #0000ff !important; }
        "#;
        let sheet = Stylesheet::parse(css);
        let classes = vec!["a".to_string()];
        let element = element("text", &classes);
        let cascade = Cascade::new(&sheet, Viewport::default());
        let style = cascade.compute(&element, &[], None, &[], &[]);
        let color = style.text.color.components;
        assert!(color[2] > 0.9 && color[0] < 0.1, "{color:?}");
    }

    #[test]
    fn variables_and_inheritance_work() {
        let css = r#"
            :root { --accent: #00ff00; }
            .a { color: var(--accent); font-size: 20px; }
            .b { font-size: 2em; }
        "#;
        let sheet = Stylesheet::parse(css);
        let root_classes: Vec<String> = Vec::new();
        let root = element("root", &root_classes);
        let cascade = Cascade::new(&sheet, Viewport::default());
        let root_style = cascade.compute(&root, &[], None, &[], &[]);
        assert_eq!(
            root_style.custom_properties.get("--accent").unwrap(),
            "#00ff00"
        );

        let a_classes = vec!["a".to_string()];
        let a_ancestors = [root];
        let a = element("text", &a_classes);
        let a_style = cascade.compute(&a, &a_ancestors, Some(&root_style), &[], &[]);
        assert_eq!(a_style.text.font_size_px, 20.0);

        let b_classes = vec!["b".to_string()];
        let b_ancestors = [a];
        let b = element("text", &b_classes);
        let b_style = cascade.compute(&b, &b_ancestors, Some(&a_style), &[], &[]);
        assert_eq!(b_style.text.font_size_px, 40.0);
    }
}
