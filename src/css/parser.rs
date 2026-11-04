// SPDX-FileCopyrightText: 2026 Daniel
// SPDX-License-Identifier: GPL-3.0-or-later

//! Low level stylesheet scanner.
//!
//! This is intentionally a small, allocation-friendly scanner rather than a
//! full CSS tokenizer. It understands comments, strings, nested blocks and
//! top-level declaration splitting, which is all the engine needs. Typed
//! parsing happens in [`super::properties`].

#[derive(Clone, Debug, PartialEq)]
pub struct CssBlock {
    pub prelude: String,
    pub body: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RawDeclaration {
    pub name: String,
    pub value: String,
    pub important: bool,
}

/// Remove `/* ... */` comments without touching strings.
pub fn strip_comments(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    let mut quote = None;

    while let Some(ch) = chars.next() {
        if let Some(active) = quote {
            output.push(ch);
            if ch == active {
                quote = None;
            } else if ch == '\\'
                && let Some(next) = chars.next()
            {
                output.push(next);
            }
            continue;
        }
        if ch == '/' && chars.peek() == Some(&'*') {
            chars.next();
            let mut previous = '\0';
            for inner in chars.by_ref() {
                if previous == '*' && inner == '/' {
                    break;
                }
                previous = inner;
            }
            continue;
        }
        if ch == '"' || ch == '\'' {
            quote = Some(ch);
        }
        output.push(ch);
    }
    output
}

/// Split a stylesheet body into top-level `prelude { body }` blocks.
pub fn split_blocks(input: &str) -> Vec<CssBlock> {
    let cleaned = strip_comments(input);
    let bytes = cleaned.as_bytes();
    let mut blocks = Vec::new();
    let mut index = 0;

    while index < bytes.len() {
        while index < bytes.len() && bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        if index >= bytes.len() {
            break;
        }
        // Skip stray top-level semicolons (e.g. `@import`).
        if bytes[index] == b';' {
            index += 1;
            continue;
        }

        let prelude_start = index;
        let mut quote = None;
        let mut paren = 0_i32;
        while index < bytes.len() {
            let ch = bytes[index] as char;
            if let Some(active) = quote {
                if ch == '\\' {
                    index += 2;
                    continue;
                }
                if ch == active {
                    quote = None;
                }
                index += 1;
                continue;
            }
            match ch {
                '"' | '\'' => quote = Some(ch),
                '(' => paren += 1,
                ')' => paren -= 1,
                '{' if paren == 0 => break,
                ';' if paren == 0 => break,
                _ => {}
            }
            index += 1;
        }
        if index >= bytes.len() {
            break;
        }
        let prelude = cleaned[prelude_start..index].trim().to_string();
        if bytes[index] == b';' {
            // Statement at-rule without a block.
            index += 1;
            continue;
        }

        // Find the matching closing brace.
        let body_start = index + 1;
        let mut depth = 1_i32;
        index = body_start;
        quote = None;
        while index < bytes.len() && depth > 0 {
            let ch = bytes[index] as char;
            if let Some(active) = quote {
                if ch == '\\' {
                    index += 2;
                    continue;
                }
                if ch == active {
                    quote = None;
                }
                index += 1;
                continue;
            }
            match ch {
                '"' | '\'' => quote = Some(ch),
                '{' => depth += 1,
                '}' => depth -= 1,
                _ => {}
            }
            index += 1;
        }
        let body_end = index.saturating_sub(1);
        blocks.push(CssBlock {
            prelude,
            body: cleaned[body_start..body_end].to_string(),
        });
    }

    blocks
}

/// Split a declaration body into raw declarations.
pub fn parse_raw_declarations(body: &str) -> Vec<RawDeclaration> {
    let mut declarations = Vec::new();
    let mut current = String::new();
    let mut depth = 0_i32;
    let mut quote = None;

    let mut flush = |current: &mut String| {
        let text = current.trim();
        if let Some((name, value)) = text.split_once(':') {
            let name = name.trim().to_ascii_lowercase();
            if name.is_empty() {
                return;
            }
            let mut value = value.trim().to_string();
            let important = value
                .to_ascii_lowercase()
                .trim_end()
                .ends_with("!important");
            if important {
                let lower = value.to_ascii_lowercase();
                let index = lower.rfind("!important").unwrap();
                value.truncate(index);
                value = value.trim().to_string();
            }
            if !value.is_empty() {
                declarations.push(RawDeclaration {
                    name,
                    value,
                    important,
                });
            }
        }
    };

    for ch in strip_comments(body).chars() {
        if let Some(active) = quote {
            current.push(ch);
            if ch == active {
                quote = None;
            }
            continue;
        }
        match ch {
            '"' | '\'' => {
                quote = Some(ch);
                current.push(ch);
            }
            '(' | '[' => {
                depth += 1;
                current.push(ch);
            }
            ')' | ']' => {
                depth -= 1;
                current.push(ch);
            }
            ';' if depth == 0 => {
                flush(&mut current);
                current.clear();
            }
            c => current.push(c),
        }
    }
    flush(&mut current);
    declarations
}

pub fn parse_declarations(body: &str) -> Vec<(String, String)> {
    parse_raw_declarations(body)
        .into_iter()
        .map(|declaration| (declaration.name, declaration.value))
        .collect()
}

/// Resolve every `var(--name, fallback)` in a value. Returns `None` when a
/// referenced variable is missing and has no fallback.
pub fn resolve_variables(
    value: &str,
    lookup: &mut dyn FnMut(&str) -> Option<String>,
    depth: usize,
) -> Option<String> {
    if depth > 16 {
        return None;
    }
    let mut output = String::with_capacity(value.len());
    let mut index = 0;
    let bytes = value.as_bytes();

    while index < bytes.len() {
        let rest = &value[index..];
        let lower = rest.to_ascii_lowercase();
        if lower.starts_with("var(") {
            let open = index + 3;
            let close = find_matching(value, open, '(', ')')?;
            let inner = &value[open + 1..close];
            let (name, fallback) = match split_top_level(inner, ',') {
                Some((name, fallback)) => (name.trim(), Some(fallback.trim())),
                None => (inner.trim(), None),
            };
            let resolved = lookup(name).or_else(|| {
                fallback.and_then(|fallback| resolve_variables(fallback, lookup, depth + 1))
            })?;
            output.push_str(&resolved);
            index = close + 1;
        } else {
            let ch = rest.chars().next().unwrap();
            output.push(ch);
            index += ch.len_utf8();
        }
    }

    // Resolve nested variables produced by substitutions.
    if output.to_ascii_lowercase().contains("var(") {
        return resolve_variables(&output, lookup, depth + 1);
    }
    Some(output)
}

/// Find the index of the closing delimiter matching `open_index`.
pub fn find_matching(input: &str, open_index: usize, open: char, close: char) -> Option<usize> {
    let mut depth = 0_i32;
    let mut quote = None;
    for (offset, ch) in input[open_index..].char_indices() {
        if let Some(active) = quote {
            if ch == active {
                quote = None;
            }
            continue;
        }
        match ch {
            '"' | '\'' => quote = Some(ch),
            c if c == open => depth += 1,
            c if c == close => {
                depth -= 1;
                if depth == 0 {
                    return Some(open_index + offset);
                }
            }
            _ => {}
        }
    }
    None
}

fn split_top_level(input: &str, delimiter: char) -> Option<(&str, &str)> {
    let mut depth = 0_i32;
    let mut quote = None;
    for (index, ch) in input.char_indices() {
        if let Some(active) = quote {
            if ch == active {
                quote = None;
            }
            continue;
        }
        match ch {
            '"' | '\'' => quote = Some(ch),
            '(' | '[' => depth += 1,
            ')' | ']' => depth -= 1,
            c if c == delimiter && depth == 0 => {
                return Some((&input[..index], &input[index + ch.len_utf8()..]));
            }
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_rules_and_declarations() {
        let css = r#"
            /* a comment { } */
            .title, text { color: red; font-size: 20px; }
            @media (min-width: 100px) {
                .small { width: 10px; }
            }
        "#;
        let blocks = split_blocks(css);
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].prelude, ".title, text");
        assert_eq!(
            parse_declarations(&blocks[0].body),
            vec![
                ("color".to_string(), "red".to_string()),
                ("font-size".to_string(), "20px".to_string())
            ]
        );
        assert!(blocks[1].prelude.starts_with("@media"));
    }

    #[test]
    fn respects_strings_and_important() {
        let declarations = parse_raw_declarations(
            r#"content: ";"; color: red !important; width: calc(100% - 2px)"#,
        );
        assert_eq!(declarations.len(), 3);
        assert!(declarations[1].important);
        assert_eq!(declarations[2].value, "calc(100% - 2px)");
    }

    #[test]
    fn resolves_variables() {
        let mut lookup = |name: &str| match name {
            "--accent" => Some("#ff0000".to_string()),
            "--space" => Some("8px".to_string()),
            _ => None,
        };
        assert_eq!(
            resolve_variables("var(--space) var(--accent)", &mut lookup, 0).unwrap(),
            "8px #ff0000"
        );
        assert_eq!(
            resolve_variables("var(--missing, 4px)", &mut lookup, 0).unwrap(),
            "4px"
        );
        assert!(resolve_variables("var(--missing)", &mut lookup, 0).is_none());
    }
}
