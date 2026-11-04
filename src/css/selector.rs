// SPDX-FileCopyrightText: 2026 Daniel
// SPDX-License-Identifier: GPL-3.0-or-later

//! Selector parsing and matching.
//!
//! A deliberately small, dependency-free subset of CSS selectors that covers
//! the styling needs of a sticker engine while remaining predictable:
//!
//! * type selectors (`text`), universal (`*`)
//! * class (`.panel`), id (`#cpu`) and attribute (`[data-x]`) selectors
//! * descendant (` `) and child (`>`) combinators
//! * `:first-child`, `:last-child`, `:nth-child()`, `:not()`, `:is()`
//! * dynamic pseudo classes are parsed but never match (there is no pointer).

use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Specificity {
    pub ids: u32,
    pub classes: u32,
    pub types: u32,
}

impl Specificity {
    pub const ZERO: Self = Self {
        ids: 0,
        classes: 0,
        types: 0,
    };

    pub fn combine(self, other: Self) -> Self {
        Self {
            ids: self.ids + other.ids,
            classes: self.classes + other.classes,
            types: self.types + other.types,
        }
    }

    pub fn key(&self) -> u64 {
        ((self.ids as u64) << 40) | ((self.classes as u64) << 20) | self.types as u64
    }
}

impl Default for Specificity {
    fn default() -> Self {
        Self::ZERO
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Combinator {
    Descendant,
    Child,
    Adjacent,
    GeneralSibling,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Pseudo {
    Root,
    FirstChild,
    LastChild,
    OnlyChild,
    NthChild(isize, isize),
    Not(Vec<Selector>),
    Is(Vec<Selector>),
    Where(Vec<Selector>),
    Dynamic(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttrOp {
    Exists,
    Equals,
    Includes,
    DashMatch,
    Prefix,
    Suffix,
    Substring,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AttrSelector {
    pub name: String,
    pub op: AttrOp,
    pub value: Option<String>,
    pub case_insensitive: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Compound {
    pub tag: Option<String>,
    pub id: Option<String>,
    pub classes: Vec<String>,
    pub attributes: Vec<AttrSelector>,
    pub pseudos: Vec<Pseudo>,
}

impl Compound {
    fn specificity(&self) -> Specificity {
        let mut spec = Specificity {
            ids: u32::from(self.id.is_some()),
            classes: self.classes.len() as u32 + self.attributes.len() as u32,
            types: u32::from(self.tag.as_deref().is_some_and(|tag| tag != "*")),
        };
        for pseudo in &self.pseudos {
            spec = spec.combine(match pseudo {
                Pseudo::Where(_) => Specificity::ZERO,
                Pseudo::Not(selectors) | Pseudo::Is(selectors) => selectors
                    .iter()
                    .map(|selector| selector.specificity)
                    .max_by_key(|specificity| specificity.key())
                    .unwrap_or(Specificity::ZERO),
                Pseudo::Root
                | Pseudo::FirstChild
                | Pseudo::LastChild
                | Pseudo::OnlyChild
                | Pseudo::NthChild(..)
                | Pseudo::Dynamic(_) => Specificity {
                    classes: 1,
                    ..Specificity::ZERO
                },
            });
        }
        spec
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Selector {
    pub compounds: Vec<Compound>,
    pub combinators: Vec<Combinator>,
    pub specificity: Specificity,
}

impl Selector {
    pub fn parse(input: &str) -> Result<Self, String> {
        let tokens = tokenize(input)?;
        if tokens.is_empty() {
            return Err("empty selector".to_string());
        }

        let mut compounds = Vec::new();
        let mut combinators = Vec::new();
        let mut current = Compound::default();
        let mut has_current = false;
        let mut pending_combinator = Combinator::Descendant;
        let mut index = 0;

        while index < tokens.len() {
            match &tokens[index] {
                SelectorToken::Combinator(combinator) => {
                    if has_current {
                        compounds.push(std::mem::take(&mut current));
                        combinators.push(*combinator);
                        has_current = false;
                    } else if let Some(last) = combinators.last_mut() {
                        *last = *combinator;
                    } else {
                        return Err("selector starts with a combinator".to_string());
                    }
                    pending_combinator = *combinator;
                }
                SelectorToken::Compound(compound) => {
                    merge_compound(&mut current, compound.clone());
                    has_current = true;
                }
            }
            index += 1;
        }

        if has_current {
            compounds.push(current);
        } else if !compounds.is_empty() {
            return Err("selector ends with a combinator".to_string());
        }
        if compounds.is_empty() {
            return Err("empty selector".to_string());
        }
        let _ = pending_combinator;

        let mut specificity = Specificity::ZERO;
        for compound in &compounds {
            specificity = specificity.combine(compound.specificity());
        }

        Ok(Self {
            compounds,
            combinators,
            specificity,
        })
    }

    pub fn matches(&self, element: &Element<'_>, ancestors: &[Element<'_>]) -> bool {
        self.matches_from(self.compounds.len() - 1, element, ancestors)
    }

    fn matches_from(&self, index: usize, element: &Element<'_>, ancestors: &[Element<'_>]) -> bool {
        let compound = &self.compounds[index];
        if compound
            .pseudos
            .iter()
            .any(|pseudo| matches!(pseudo, Pseudo::Root))
            && !(index == 0 && element.index == 0 && ancestors.is_empty())
        {
            return false;
        }
        if !compound.matches(element) {
            return false;
        }
        if index == 0 {
            return true;
        }
        match self.combinators[index - 1] {
            Combinator::Child => ancestors.last().is_some_and(|parent| {
                self.matches_from(index - 1, parent, &ancestors[..ancestors.len() - 1])
            }),
            Combinator::Descendant => {
                for depth in (0..ancestors.len()).rev() {
                    if self.matches_from(index - 1, &ancestors[depth], &ancestors[..depth]) {
                        return true;
                    }
                }
                false
            }
            Combinator::Adjacent | Combinator::GeneralSibling => false,
        }
    }
}

impl fmt::Display for Selector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, compound) in self.compounds.iter().enumerate() {
            if index > 0 {
                match self.combinators.get(index - 1) {
                    Some(Combinator::Child) => write!(f, " > ")?,
                    Some(Combinator::Adjacent) => write!(f, " + ")?,
                    Some(Combinator::GeneralSibling) => write!(f, " ~ ")?,
                    _ => write!(f, " ")?,
                }
            }
            write!(f, "{compound}")?;
        }
        Ok(())
    }
}

impl fmt::Display for Compound {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(tag) = &self.tag {
            write!(f, "{tag}")?;
        }
        if let Some(id) = &self.id {
            write!(f, "#{id}")?;
        }
        for class in &self.classes {
            write!(f, ".{class}")?;
        }
        for attribute in &self.attributes {
            write!(f, "[{}", attribute.name)?;
            let operator = match attribute.op {
                AttrOp::Exists => None,
                AttrOp::Equals => Some("="),
                AttrOp::Includes => Some("~="),
                AttrOp::DashMatch => Some("|="),
                AttrOp::Prefix => Some("^="),
                AttrOp::Suffix => Some("$="),
                AttrOp::Substring => Some("*="),
            };
            if let (Some(operator), Some(value)) = (operator, &attribute.value) {
                write!(f, "{operator}\"{value}\"")?;
            }
            if attribute.case_insensitive {
                write!(f, " i")?;
            }
            write!(f, "]")?;
        }
        for pseudo in &self.pseudos {
            write!(f, ":{pseudo:?}")?;
        }
        Ok(())
    }
}

impl Compound {
    pub fn matches(&self, element: &Element<'_>) -> bool {
        if let Some(tag) = &self.tag
            && tag != "*"
            && !element.tag.eq_ignore_ascii_case(tag)
        {
            return false;
        }
        if let Some(id) = &self.id
            && Some(id.as_str()) != element.id
        {
            return false;
        }
        for class in &self.classes {
            if !element.classes.iter().any(|value| value == class) {
                return false;
            }
        }
        for attribute in &self.attributes {
            let Some(actual) = element.attribute(&attribute.name) else {
                return false;
            };
            let Some(expected) = &attribute.value else {
                continue;
            };
            let (actual, expected) = if attribute.case_insensitive {
                (actual.to_ascii_lowercase(), expected.to_ascii_lowercase())
            } else {
                (actual.to_string(), expected.clone())
            };
            let matches = match attribute.op {
                AttrOp::Exists => true,
                AttrOp::Equals => actual == expected,
                AttrOp::Includes => actual.split_whitespace().any(|part| part == expected),
                AttrOp::DashMatch => {
                    actual == expected || actual.starts_with(&format!("{expected}-"))
                }
                AttrOp::Prefix => actual.starts_with(&expected),
                AttrOp::Suffix => actual.ends_with(&expected),
                AttrOp::Substring => actual.contains(&expected),
            };
            if !matches {
                return false;
            }
        }
        for pseudo in &self.pseudos {
            if !pseudo.matches(element) {
                return false;
            }
        }
        true
    }
}

impl Pseudo {
    fn matches(&self, element: &Element<'_>) -> bool {
        match self {
            Self::Root => true,
            Self::FirstChild => element.index == 0,
            Self::LastChild => element.index + 1 >= element.sibling_count,
            Self::OnlyChild => element.sibling_count <= 1,
            Self::NthChild(a, b) => {
                let position = element.index as isize + 1;
                if *a == 0 {
                    position == *b
                } else {
                    let delta = position - *b;
                    delta % *a == 0 && delta / *a >= 0
                }
            }
            Self::Not(selectors) => !selectors
                .iter()
                .any(|selector| selector.matches(element, &[])),
            Self::Is(selectors) | Self::Where(selectors) => selectors
                .iter()
                .any(|selector| selector.matches(element, &[])),
            Self::Dynamic(_) => false,
        }
    }
}

/// A borrowed element used for selector matching.
#[derive(Clone, Copy, Debug)]
pub struct Element<'a> {
    pub tag: &'a str,
    pub id: Option<&'a str>,
    pub classes: &'a [String],
    pub attributes: &'a [(String, String)],
    pub index: usize,
    pub sibling_count: usize,
}

impl<'a> Element<'a> {
    pub fn new(
        tag: &'a str,
        id: Option<&'a str>,
        classes: &'a [String],
        index: usize,
        sibling_count: usize,
    ) -> Self {
        Self {
            tag,
            id,
            classes,
            attributes: &[],
            index,
            sibling_count,
        }
    }

    pub fn with_attributes(mut self, attributes: &'a [(String, String)]) -> Self {
        self.attributes = attributes;
        self
    }

    pub fn attribute(&self, name: &str) -> Option<&str> {
        self.attributes
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }
}

#[derive(Clone, Debug)]
enum SelectorToken {
    Compound(Compound),
    Combinator(Combinator),
}

fn merge_compound(target: &mut Compound, source: Compound) {
    if source.tag.is_some() {
        target.tag = source.tag;
    }
    if source.id.is_some() {
        target.id = source.id;
    }
    target.classes.extend(source.classes);
    target.attributes.extend(source.attributes);
    target.pseudos.extend(source.pseudos);
}

/// Parse a selector list (`a, b, c`).
pub fn parse_selector_list(input: &str) -> Result<Vec<Selector>, String> {
    split_selector_list(input)
        .into_iter()
        .map(|part| Selector::parse(part.trim()))
        .collect()
}

fn split_selector_list(input: &str) -> Vec<&str> {
    let mut result = Vec::new();
    let mut depth = 0_i32;
    let mut start = 0;
    for (index, ch) in input.char_indices() {
        match ch {
            '(' | '[' => depth += 1,
            ')' | ']' => depth -= 1,
            ',' if depth == 0 => {
                result.push(&input[start..index]);
                start = index + 1;
            }
            _ => {}
        }
    }
    result.push(&input[start..]);
    result
}

fn tokenize(input: &str) -> Result<Vec<SelectorToken>, String> {
    let mut tokens = Vec::new();
    let mut index = 0;
    let bytes = input.as_bytes();

    while index < bytes.len() {
        let ch = input[index..].chars().next().unwrap();
        if ch.is_whitespace() {
            index += ch.len_utf8();
            while index < bytes.len() {
                let c = input[index..].chars().next().unwrap();
                if !c.is_whitespace() {
                    break;
                }
                index += c.len_utf8();
            }
            if index < bytes.len() {
                let next = input[index..].chars().next().unwrap();
                if !matches!(next, '>' | '+' | '~')
                    && matches!(tokens.last(), Some(SelectorToken::Compound(_)))
                {
                    tokens.push(SelectorToken::Combinator(Combinator::Descendant));
                }
            }
            continue;
        }
        if ch == '>' || ch == '+' || ch == '~' {
            let combinator = match ch {
                '>' => Combinator::Child,
                '+' => Combinator::Adjacent,
                _ => Combinator::GeneralSibling,
            };
            if let Some(SelectorToken::Combinator(last)) = tokens.last_mut() {
                *last = combinator;
            } else {
                tokens.push(SelectorToken::Combinator(combinator));
            }
            index += 1;
            continue;
        }

        let (compound, consumed) = parse_compound(&input[index..])?;
        tokens.push(SelectorToken::Compound(compound));
        index += consumed;
    }

    while matches!(tokens.last(), Some(SelectorToken::Combinator(_))) {
        tokens.pop();
    }
    Ok(tokens)
}

fn parse_compound(input: &str) -> Result<(Compound, usize), String> {
    let mut compound = Compound::default();
    let mut index = 0;
    let mut has_any = false;

    if input.starts_with('*') {
        compound.tag = Some("*".to_string());
        index += 1;
        has_any = true;
    } else if input
        .chars()
        .next()
        .is_some_and(|ch| ch.is_ascii_alphabetic() || ch == '_' || ch == '-')
    {
        let (name, consumed) = read_identifier(&input[index..])?;
        compound.tag = Some(name);
        index += consumed;
        has_any = true;
    }

    while index < input.len() {
        let ch = input[index..].chars().next().unwrap();
        match ch {
            '.' => {
                let (name, consumed) = read_identifier(&input[index + 1..])?;
                if name.is_empty() {
                    return Err("empty class selector".to_string());
                }
                compound.classes.push(name);
                index += 1 + consumed;
                has_any = true;
            }
            '#' => {
                let (name, consumed) = read_identifier(&input[index + 1..])?;
                compound.id = Some(name);
                index += 1 + consumed;
                has_any = true;
            }
            '[' => {
                let end = find_closing(&input[index..], '[', ']')?;
                let inner = input[index + 1..index + end].trim();
                compound.attributes.push(parse_attribute_selector(inner));
                index += end + 1;
                has_any = true;
            }
            ':' => {
                let (pseudo, consumed) = parse_pseudo(&input[index..])?;
                compound.pseudos.push(pseudo);
                index += consumed;
                has_any = true;
            }
            _ => break,
        }
    }

    if !has_any {
        return Err(format!("invalid selector near: {input}"));
    }
    Ok((compound, index))
}

fn parse_attribute_selector(input: &str) -> AttrSelector {
    let mut text = input.trim();
    let case_insensitive = text
        .strip_suffix(" i")
        .or_else(|| text.strip_suffix(" I"))
        .is_some();
    if case_insensitive {
        text = text.trim_end_matches(" i").trim_end_matches(" I").trim();
    }
    for (op, op_text) in [
        (AttrOp::Includes, "~="),
        (AttrOp::DashMatch, "|="),
        (AttrOp::Prefix, "^="),
        (AttrOp::Suffix, "$="),
        (AttrOp::Substring, "*="),
        (AttrOp::Equals, "="),
    ] {
        if let Some((name, value)) = text.split_once(op_text) {
            return AttrSelector {
                name: name.trim().to_string(),
                op,
                value: Some(
                    value
                        .trim()
                        .trim_matches('"')
                        .trim_matches('\'')
                        .to_string(),
                ),
                case_insensitive,
            };
        }
    }
    AttrSelector {
        name: text.to_string(),
        op: AttrOp::Exists,
        value: None,
        case_insensitive,
    }
}

fn parse_pseudo(input: &str) -> Result<(Pseudo, usize), String> {
    let rest = &input[1..];
    let (name, consumed) = read_identifier(rest)?;
    let lower = name.to_ascii_lowercase();
    let mut total = 1 + consumed;

    if rest[consumed..].starts_with('(') {
        let end = find_closing(&rest[consumed..], '(', ')')?;
        let inner = rest[consumed + 1..consumed + end].trim();
        total += end + 1;
        return Ok((
            match lower.as_str() {
                "not" => Pseudo::Not(parse_selector_list(inner)?),
                "is" | "matches" | "any" => Pseudo::Is(parse_selector_list(inner)?),
                "where" => Pseudo::Where(parse_selector_list(inner)?),
                "nth-child" => parse_nth(inner)?,
                "nth-last-child" => parse_nth(inner)?,
                _ => Pseudo::Dynamic(lower),
            },
            total,
        ));
    }

    Ok((
        match lower.as_str() {
            "root" => Pseudo::Root,
            "first-child" => Pseudo::FirstChild,
            "last-child" => Pseudo::LastChild,
            "only-child" => Pseudo::OnlyChild,
            other => Pseudo::Dynamic(other.to_string()),
        },
        total,
    ))
}

fn parse_nth(inner: &str) -> Result<Pseudo, String> {
    let text = inner.replace(' ', "").to_ascii_lowercase();
    if text == "odd" {
        return Ok(Pseudo::NthChild(2, 1));
    }
    if text == "even" {
        return Ok(Pseudo::NthChild(2, 0));
    }
    if let Some(index) = text.find('n') {
        let a = match &text[..index] {
            "" | "+" => 1,
            "-" => -1,
            value => value
                .parse::<isize>()
                .map_err(|_| format!("invalid nth-child: {inner}"))?,
        };
        let remainder = &text[index + 1..];
        let b = if remainder.is_empty() {
            0
        } else {
            remainder
                .parse::<isize>()
                .map_err(|_| format!("invalid nth-child: {inner}"))?
        };
        Ok(Pseudo::NthChild(a, b))
    } else {
        let value = text
            .parse::<isize>()
            .map_err(|_| format!("invalid nth-child: {inner}"))?;
        Ok(Pseudo::NthChild(0, value))
    }
}

fn read_identifier(input: &str) -> Result<(String, usize), String> {
    let mut result = String::new();
    let mut consumed = 0;
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' || ch == '\\' {
            result.push(ch);
            consumed += ch.len_utf8();
        } else {
            break;
        }
    }
    Ok((result, consumed))
}

fn find_closing(input: &str, open: char, close: char) -> Result<usize, String> {
    let mut depth = 0_i32;
    let mut in_string = None;
    for (index, ch) in input.char_indices() {
        if let Some(quote) = in_string {
            if ch == quote {
                in_string = None;
            }
            continue;
        }
        match ch {
            '"' | '\'' => in_string = Some(ch),
            c if c == open => depth += 1,
            c if c == close => {
                depth -= 1;
                if depth == 0 {
                    return Ok(index);
                }
            }
            _ => {}
        }
    }
    Err(format!("unclosed {open}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn element<'a>(
        tag: &'a str,
        id: Option<&'a str>,
        classes: &'a [String],
        index: usize,
        siblings: usize,
    ) -> Element<'a> {
        Element::new(tag, id, classes, index, siblings)
    }

    #[test]
    fn parses_compound_and_specificity() {
        let selector = Selector::parse("hbox > text.cpu#value").unwrap();
        assert_eq!(selector.compounds.len(), 2);
        assert_eq!(selector.combinators, vec![Combinator::Child]);
        assert_eq!(selector.specificity.ids, 1);
        assert_eq!(selector.specificity.classes, 1);
        assert_eq!(selector.specificity.types, 2);
    }

    #[test]
    fn matches_class_and_ancestor() {
        let root_classes = vec!["panel".to_string()];
        let child_classes = vec!["value".to_string(), "big".to_string()];
        let root = element("vbox", None, &root_classes, 0, 1);
        let ancestors = [root];
        let child = element("text", Some("cpu"), &child_classes, 0, 1);
        assert!(
            Selector::parse(".panel text")
                .unwrap()
                .matches(&child, &ancestors)
        );
        assert!(
            Selector::parse("vbox > .value.big")
                .unwrap()
                .matches(&child, &ancestors)
        );
        assert!(
            !Selector::parse("hbox > text")
                .unwrap()
                .matches(&child, &ancestors)
        );
    }

    #[test]
    fn matches_pseudo() {
        let classes = vec!["item".to_string()];
        let element = element("text", None, &classes, 2, 5);
        assert!(
            Selector::parse(".item:nth-child(3)")
                .unwrap()
                .matches(&element, &[])
        );
        assert!(
            !Selector::parse(".item:first-child")
                .unwrap()
                .matches(&element, &[])
        );
        assert!(
            Selector::parse(":not(.other)")
                .unwrap()
                .matches(&element, &[])
        );
    }
}
