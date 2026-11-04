// SPDX-FileCopyrightText: 2026 Daniel
// SPDX-License-Identifier: GPL-3.0-or-later

//! Stylesheet storage and the cascade.

use std::collections::{BTreeMap, HashMap};

use vello::peniko::Color;

use super::Viewport;
use super::computed::ComputedStyle;
use super::parser::{self, RawDeclaration};
use super::properties::Declarations;
use super::properties::transition::{self, Keyframes};
use super::selector::{Element, Selector, Specificity, parse_selector_list};
use super::values::Length;

#[derive(Clone, Debug, PartialEq)]
pub struct FontFace {
    pub family: String,
    pub source: String,
    pub weight: u16,
    pub style: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct MediaCondition {
    pub min_width: Option<f32>,
    pub max_width: Option<f32>,
    pub min_height: Option<f32>,
    pub max_height: Option<f32>,
    pub orientation: Option<Orientation>,
    pub prefers_dark: Option<bool>,
    pub prefers_reduced_motion: Option<bool>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Orientation {
    Portrait,
    Landscape,
}

impl MediaCondition {
    pub fn matches(&self, viewport: Viewport) -> bool {
        if let Some(value) = self.min_width
            && viewport.width < value
        {
            return false;
        }
        if let Some(value) = self.max_width
            && viewport.width > value
        {
            return false;
        }
        if let Some(value) = self.min_height
            && viewport.height < value
        {
            return false;
        }
        if let Some(value) = self.max_height
            && viewport.height > value
        {
            return false;
        }
        if let Some(orientation) = self.orientation {
            let current = if viewport.width >= viewport.height {
                Orientation::Landscape
            } else {
                Orientation::Portrait
            };
            if current != orientation {
                return false;
            }
        }
        // The engine always renders the default light palette; media queries
        // are still parsed so stylesheets remain portable.
        if let Some(dark) = self.prefers_dark
            && dark
        {
            return false;
        }
        if let Some(reduced) = self.prefers_reduced_motion
            && !reduced
        {
            return false;
        }
        true
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
            let text = part
                .trim()
                .trim_start_matches("only ")
                .trim_start_matches("screen and ")
                .trim_start_matches("all and ")
                .trim();
            let mut remaining = text.to_string();
            while let Some(open) = remaining.find('(') {
                let Some(close) = remaining[open..].find(')').map(|value| value + open) else {
                    break;
                };
                let feature = remaining[open + 1..close].trim().to_ascii_lowercase();
                remaining = remaining[close + 1..].to_string();
                let (name, value) = feature
                    .split_once(':')
                    .map(|(name, value)| (name.trim(), value.trim()))
                    .unwrap_or((feature.as_str(), ""));
                match name {
                    "min-width" => condition.min_width = parse_px(value),
                    "max-width" => condition.max_width = parse_px(value),
                    "min-height" => condition.min_height = parse_px(value),
                    "max-height" => condition.max_height = parse_px(value),
                    "orientation" => {
                        condition.orientation = Some(if value == "portrait" {
                            Orientation::Portrait
                        } else {
                            Orientation::Landscape
                        })
                    }
                    "prefers-color-scheme" => condition.prefers_dark = Some(value == "dark"),
                    "prefers-reduced-motion" => {
                        condition.prefers_reduced_motion = Some(value != "reduce")
                    }
                    _ => {}
                }
            }
            conditions.push(condition);
        }
        if conditions.is_empty() {
            conditions.push(MediaCondition::default());
        }
        Self { conditions }
    }

    pub fn matches(&self, viewport: Viewport) -> bool {
        self.conditions
            .iter()
            .any(|condition| condition.matches(viewport))
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
    ) -> Vec<(&'a StyleRule, Specificity)> {
        let mut matched = Vec::new();
        for rule in &self.rules {
            if let Some(media) = &rule.media
                && !media.matches(viewport)
            {
                continue;
            }
            let mut best: Option<Specificity> = None;
            for selector in &rule.selectors {
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
}

impl<'a> Cascade<'a> {
    pub fn new(stylesheet: &'a Stylesheet, viewport: Viewport) -> Self {
        Self {
            stylesheet,
            viewport,
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
        let mut matched: Vec<Matched<'_>> = self
            .stylesheet
            .rules_for(element, ancestors, self.viewport)
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
        assert!(query.matches(Viewport {
            width: 1000.0,
            height: 500.0,
            ..Viewport::default()
        }));
        assert!(!query.matches(Viewport {
            width: 500.0,
            height: 1000.0,
            ..Viewport::default()
        }));
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
