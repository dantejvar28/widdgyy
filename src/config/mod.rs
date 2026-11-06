// SPDX-FileCopyrightText: 2026 Daniel
// SPDX-License-Identifier: GPL-3.0-or-later

//! JSON5 configuration loader.
//!
//! The JSON schema mirrors the original Python implementation, but the loader
//! emits scene nodes plus low-priority style declarations. That means a value
//! from `config.jsonc` acts as a default and `styles.css` can still override it,
//! exactly like normal CSS author styles.

use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result, bail};
use serde_json::Value;

use crate::css::parser::RawDeclaration;
use crate::scene::{Anchor, ApiSpec, MediaContent, MediaKind, Node, Scene};

pub struct ConfigLoader {
    pub path: PathBuf,
}

impl ConfigLoader {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn load(&self, viewport: crate::css::Viewport) -> Result<Scene> {
        let source = std::fs::read_to_string(&self.path)
            .with_context(|| format!("reading {}", self.path.display()))?;
        let value: Value =
            json5::from_str(&source).with_context(|| format!("parsing {}", self.path.display()))?;
        let stickers = value
            .get("stickers")
            .and_then(Value::as_array)
            .context("config must contain a `stickers` array")?;

        let mut scene = Scene::new(viewport);
        for sticker in stickers {
            scene.roots.push(self.build_node(sticker)?);
        }
        scene.roots.sort_by_key(|node| node.z_index);
        Ok(scene)
    }

    fn build_node(&self, value: &Value) -> Result<Node> {
        let object = value
            .as_object()
            .context("every sticker must be a JSON object")?;
        let sticker_type = object
            .get("type")
            .and_then(Value::as_str)
            .context("sticker is missing `type`")?
            .to_ascii_lowercase();

        let classes = object
            .get("class")
            .and_then(Value::as_str)
            .map(|classes| {
                classes
                    .split_whitespace()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        let mut node = match sticker_type.as_str() {
            "text" => {
                let text = get(object, &["text"]).and_then(Value::as_str).unwrap_or("");
                Node::text("text", classes, text)
            }
            "clock" => {
                let format = get(object, &["format"])
                    .and_then(Value::as_str)
                    .unwrap_or("%H:%M:%S");
                let text = chrono::Local::now().format(format).to_string();
                Node::clock(classes, format, text)
            }
            "api_text" => {
                let mut node = Node::text("api_text", classes, "Loading...");
                if let crate::scene::NodeKind::Text(content) = &mut node.kind {
                    content.api = Some(self.build_api_spec(object)?);
                }
                node
            }
            "media" => {
                let path = get(object, &["path"])
                    .and_then(Value::as_str)
                    .context("media sticker is missing `path`")?;
                let path = self.resolve_media_path(path);
                let kind = media_kind(&path);
                let (width, height) = match kind {
                    MediaKind::Image | MediaKind::Gif => image::image_dimensions(&path)
                        .map(|(width, height)| (width as f32, height as f32))
                        .unwrap_or((0.0, 0.0)),
                    MediaKind::Video => (0.0, 0.0),
                };
                let content = MediaContent {
                    path,
                    kind,
                    intrinsic_width: width,
                    intrinsic_height: height,
                    autoplay: get(object, &["autoplay"])
                        .and_then(Value::as_bool)
                        .unwrap_or(true),
                    looping: get(object, &["loop"])
                        .and_then(Value::as_bool)
                        .unwrap_or(true),
                    revision: 0,
                };
                Node::media(classes, content)
            }
            "hbox" | "vbox" | "grid" => {
                let tag = sticker_type.clone();
                let mut container = Node::container(tag, classes);
                let children = get(object, &["children"])
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default();
                for child in &children {
                    container.children.push(self.build_node(child)?);
                }
                container
            }
            other => bail!("unknown sticker type: {other}"),
        };

        node.id = get(object, &["id"])
            .and_then(Value::as_str)
            .map(str::to_string);
        node.anchor = Anchor::parse(
            get(object, &["anchor"])
                .and_then(Value::as_str)
                .unwrap_or("top-left"),
        );
        node.offset_x = get(object, &["offset_x", "offset-x", "x"])
            .and_then(Value::as_f64)
            .unwrap_or(0.0) as f32;
        node.offset_y = get(object, &["offset_y", "offset-y", "y"])
            .and_then(Value::as_f64)
            .unwrap_or(0.0) as f32;

        node.config_style = self.config_style(&sticker_type, object);
        if let Some(style) = get(object, &["style"]).and_then(Value::as_object) {
            node.inline_style = style
                .iter()
                .filter_map(|(name, value)| {
                    Some(RawDeclaration {
                        name: name.to_ascii_lowercase(),
                        value: json_to_css(value)?,
                        important: false,
                    })
                })
                .collect();
        }

        Ok(node)
    }

    fn build_api_spec(&self, object: &serde_json::Map<String, Value>) -> Result<ApiSpec> {
        let url = get(object, &["url"])
            .and_then(Value::as_str)
            .context("api_text sticker is missing `url`")?
            .to_string();
        let headers = get(object, &["headers"])
            .and_then(Value::as_object)
            .map(|headers| {
                headers
                    .iter()
                    .filter_map(|(name, value)| Some((name.clone(), value.as_str()?.to_string())))
                    .collect()
            })
            .unwrap_or_default();
        let fields = get(object, &["fields"])
            .and_then(Value::as_object)
            .map(|fields| {
                fields
                    .iter()
                    .filter_map(|(name, value)| Some((name.clone(), value.as_str()?.to_string())))
                    .collect()
            })
            .unwrap_or_default();
        Ok(ApiSpec {
            url,
            method: get(object, &["method"])
                .and_then(Value::as_str)
                .unwrap_or("GET")
                .to_ascii_uppercase(),
            headers,
            template: get(object, &["template"])
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            fields,
            timeout: Duration::from_secs_f64(
                get(object, &["timeout"])
                    .and_then(Value::as_f64)
                    .unwrap_or(10.0)
                    .max(0.1),
            ),
            retries: get(object, &["retries"])
                .and_then(Value::as_u64)
                .unwrap_or(1) as u32,
            update_interval: Duration::from_secs_f64(
                get(object, &["update_interval", "update-interval"])
                    .and_then(Value::as_f64)
                    .unwrap_or(10.0)
                    .max(0.1),
            ),
        })
    }

    fn config_style(
        &self,
        sticker_type: &str,
        object: &serde_json::Map<String, Value>,
    ) -> Vec<RawDeclaration> {
        let mut declarations = Vec::new();
        let mut push = |name: &str, value: String| {
            declarations.push(RawDeclaration {
                name: name.to_string(),
                value,
                important: false,
            });
        };

        if let Some(value) = get(object, &["font_size", "font-size"]).and_then(json_to_css) {
            push("font-size", value);
        }
        if let Some(value) = get(object, &["color"]).and_then(json_to_css) {
            push("color", value);
        }
        if let Some(value) = get(object, &["width"]).and_then(json_to_css) {
            push("width", value);
        }
        if let Some(value) = get(object, &["height"]).and_then(json_to_css) {
            push("height", value);
        }
        if let Some(value) = get(object, &["z_index", "z-index"]).and_then(json_to_css) {
            push("z-index", value);
        }
        if let Some(value) = get(object, &["fit"]).and_then(Value::as_str) {
            push("object-fit", value.to_string());
        }
        if let Some(value) = get(object, &["margin"]).and_then(json_to_css) {
            push("margin", value);
        }
        for (field, property) in [
            ("margin_top", "margin-top"),
            ("margin_right", "margin-right"),
            ("margin_bottom", "margin-bottom"),
            ("margin_left", "margin-left"),
            ("padding_top", "padding-top"),
            ("padding_right", "padding-right"),
            ("padding_bottom", "padding-bottom"),
            ("padding_left", "padding-left"),
        ] {
            let camel = field.replace('_', "-");
            if let Some(value) = get(object, &[field, camel.as_str()]).and_then(json_to_css) {
                push(property, value);
            }
        }
        if let Some(value) = get(object, &["padding"]).and_then(json_to_css) {
            push("padding", value);
        }
        if let Some(value) = get(object, &["spacing"]).and_then(json_to_css) {
            push("gap", value);
        }

        match sticker_type {
            "hbox" => {
                push("display", "flex".to_string());
                push("flex-direction", "row".to_string());
            }
            "vbox" => {
                push("display", "flex".to_string());
                push("flex-direction", "column".to_string());
            }
            "grid" => {
                push("display", "grid".to_string());
                let columns = get(object, &["columns"])
                    .and_then(Value::as_u64)
                    .unwrap_or(2)
                    .max(1);
                push(
                    "grid-template-columns",
                    std::iter::repeat_n("1fr", columns as usize)
                        .collect::<Vec<_>>()
                        .join(" "),
                );
            }
            _ => {}
        }

        if let Some(value) = get(object, &["justify", "align"]).and_then(Value::as_str) {
            push("justify-content", value.to_string());
        }
        if let Some(value) = get(object, &["align_items"]).and_then(Value::as_str) {
            push("align-items", value.to_string());
        }

        declarations
    }

    pub fn resolve_media_path(&self, raw: &str) -> PathBuf {
        let expanded = expand_vars(raw.trim());
        let path = PathBuf::from(&expanded);
        if path.is_absolute() {
            return path;
        }
        let mut candidates = Vec::new();
        if let Ok(cwd) = std::env::current_dir() {
            candidates.push(cwd.join(&path));
        }
        if let Some(parent) = self.path.parent() {
            candidates.push(parent.join(&path));
        }
        candidates.push(path.clone());
        candidates
            .into_iter()
            .find(|candidate| candidate.exists())
            .unwrap_or(path)
    }

    pub fn config_dir(&self) -> &Path {
        self.path.parent().unwrap_or_else(|| Path::new("."))
    }
}

fn get<'a>(object: &'a serde_json::Map<String, Value>, keys: &[&str]) -> Option<&'a Value> {
    keys.iter().find_map(|key| object.get(*key))
}

fn json_to_css(value: &Value) -> Option<String> {
    Some(match value {
        Value::String(value) => value.clone(),
        Value::Number(value) => {
            // Bare numbers are pixels for the sticker config.
            value.to_string()
        }
        Value::Bool(value) => value.to_string(),
        Value::Array(values) => values
            .iter()
            .filter_map(json_to_css)
            .collect::<Vec<_>>()
            .join(" "),
        Value::Null => return None,
        Value::Object(_) => return None,
    })
}

fn expand_vars(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    if let Some(rest) = input.strip_prefix("~/")
        && let Some(home) = std::env::var_os("HOME")
    {
        output.push_str(&home.to_string_lossy());
        output.push('/');
        output.push_str(rest);
        return output;
    }
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '$' {
            output.push(ch);
            continue;
        }
        if chars.peek() == Some(&'{') {
            chars.next();
            let mut name = String::new();
            for inner in chars.by_ref() {
                if inner == '}' {
                    break;
                }
                name.push(inner);
            }
            if let Ok(value) = std::env::var(&name) {
                output.push_str(&value);
            }
        } else {
            let mut name = String::new();
            while let Some(next) = chars.peek() {
                if next.is_ascii_alphanumeric() || *next == '_' {
                    name.push(*next);
                    chars.next();
                } else {
                    break;
                }
            }
            if !name.is_empty() {
                if let Ok(value) = std::env::var(&name) {
                    output.push_str(&value);
                }
            } else {
                output.push('$');
            }
        }
    }
    output
}

fn media_kind(path: &Path) -> MediaKind {
    match path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "gif" => MediaKind::Gif,
        "mp4" | "mkv" | "webm" | "avi" | "mov" | "m4v" => MediaKind::Video,
        _ => MediaKind::Image,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expands_environment_variables() {
        unsafe { std::env::set_var("WIDDGYY_TEST_ROOT", "/tmp/example") };
        assert_eq!(
            expand_vars("$WIDDGYY_TEST_ROOT/file.png"),
            "/tmp/example/file.png"
        );
        assert_eq!(
            expand_vars("${WIDDGYY_TEST_ROOT}/file.png"),
            "/tmp/example/file.png"
        );
    }

    #[test]
    fn classifies_media() {
        assert_eq!(media_kind(Path::new("a.png")), MediaKind::Image);
        assert_eq!(media_kind(Path::new("a.gif")), MediaKind::Gif);
        assert_eq!(media_kind(Path::new("a.mp4")), MediaKind::Video);
    }
}
