// SPDX-FileCopyrightText: 2026 Daniel
// SPDX-License-Identifier: GPL-3.0-or-later

//! Runtime scene graph.
//!
//! Nodes own their computed style and their media metadata; the renderer only
//! borrows them. Styles and layouts are recomputed only when the scene changes,
//! never once per frame.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::css::parser::RawDeclaration;
use crate::css::selector::Element;
use crate::css::{Cascade, ComputedStyle, Viewport};
use crate::layout::{LayoutBox, LayoutEngine};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Anchor {
    #[default]
    TopLeft,
    TopCenter,
    TopRight,
    CenterLeft,
    Center,
    CenterRight,
    BottomLeft,
    BottomCenter,
    BottomRight,
}

impl Anchor {
    pub fn parse(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "top-center" | "top" => Self::TopCenter,
            "top-right" => Self::TopRight,
            "center-left" | "left" => Self::CenterLeft,
            "center" => Self::Center,
            "center-right" | "right" => Self::CenterRight,
            "bottom-left" => Self::BottomLeft,
            "bottom-center" | "bottom" => Self::BottomCenter,
            "bottom-right" => Self::BottomRight,
            _ => Self::TopLeft,
        }
    }

    pub fn position(
        self,
        viewport: Viewport,
        width: f32,
        height: f32,
        offset_x: f32,
        offset_y: f32,
    ) -> (f32, f32) {
        let (x, y) = match self {
            Self::TopLeft => (0.0, 0.0),
            Self::TopCenter => ((viewport.width - width) / 2.0, 0.0),
            Self::TopRight => (viewport.width - width, 0.0),
            Self::CenterLeft => (0.0, (viewport.height - height) / 2.0),
            Self::Center => (
                (viewport.width - width) / 2.0,
                (viewport.height - height) / 2.0,
            ),
            Self::CenterRight => (viewport.width - width, (viewport.height - height) / 2.0),
            Self::BottomLeft => (0.0, viewport.height - height),
            Self::BottomCenter => ((viewport.width - width) / 2.0, viewport.height - height),
            Self::BottomRight => (viewport.width - width, viewport.height - height),
        };
        (x + offset_x, y + offset_y)
    }
}

#[derive(Clone, Debug)]
pub struct ApiSpec {
    pub url: String,
    pub method: String,
    pub headers: Vec<(String, String)>,
    pub template: String,
    pub fields: Vec<(String, String)>,
    pub timeout: std::time::Duration,
    pub retries: u32,
    pub update_interval: std::time::Duration,
}

#[derive(Clone, Debug)]
pub struct TextContent {
    pub text: String,
    /// `Some` for clock stickers: the text is regenerated on every tick.
    pub clock_format: Option<String>,
    /// `Some` for API powered stickers.
    pub api: Option<ApiSpec>,
    /// Updated by the API sticker worker.
    pub loading: bool,
    pub revision: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MediaKind {
    Image,
    Gif,
    Video,
}

#[derive(Clone, Debug)]
pub struct MediaContent {
    pub path: PathBuf,
    pub kind: MediaKind,
    pub intrinsic_width: f32,
    pub intrinsic_height: f32,
    pub autoplay: bool,
    pub looping: bool,
    pub revision: u64,
}

#[derive(Clone, Debug)]
pub enum NodeKind {
    Container,
    Text(TextContent),
    Media(MediaContent),
}

static NEXT_UID: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Debug)]
pub struct Node {
    /// Stable runtime identifier used by async workers and hot reload.
    pub uid: u64,
    /// CSS type selector: `text`, `clock`, `media`, `hbox`, `vbox`, `grid`.
    pub tag: String,
    pub id: Option<String>,
    pub classes: Vec<String>,
    pub kind: NodeKind,
    pub children: Vec<Node>,
    pub style: ComputedStyle,
    pub layout: LayoutBox,
    pub anchor: Anchor,
    pub offset_x: f32,
    pub offset_y: f32,
    /// Inline declarations from the JSON config (`"style": { ... }`).
    pub inline_style: Vec<RawDeclaration>,
    /// Low priority declarations derived from JSON fields (`font_size`,
    /// `padding`, `spacing`, ...). User CSS always overrides these.
    pub config_style: Vec<RawDeclaration>,
    /// Set when the content changed and the scene needs a repaint.
    pub dirty: bool,
    pub z_index: i32,
    /// Absolute position in viewport coordinates, filled by layout.
    pub absolute_x: f32,
    pub absolute_y: f32,
    pub visible: bool,
}

impl Node {
    pub fn container(tag: impl Into<String>, classes: Vec<String>) -> Self {
        Self {
            uid: NEXT_UID.fetch_add(1, Ordering::Relaxed),
            tag: tag.into(),
            id: None,
            classes,
            kind: NodeKind::Container,
            children: Vec::new(),
            style: ComputedStyle::default(),
            layout: LayoutBox::default(),
            anchor: Anchor::TopLeft,
            offset_x: 0.0,
            offset_y: 0.0,
            inline_style: Vec::new(),
            config_style: Vec::new(),
            dirty: true,
            z_index: 0,
            absolute_x: 0.0,
            absolute_y: 0.0,
            visible: true,
        }
    }

    pub fn text(tag: impl Into<String>, classes: Vec<String>, text: impl Into<String>) -> Self {
        let mut node = Self::container(tag, classes);
        node.kind = NodeKind::Text(TextContent {
            text: text.into(),
            clock_format: None,
            api: None,
            loading: false,
            revision: 0,
        });
        node
    }

    pub fn clock(classes: Vec<String>, format: impl Into<String>, text: impl Into<String>) -> Self {
        let mut node = Self::text("clock", classes, text);
        if let NodeKind::Text(content) = &mut node.kind {
            content.clock_format = Some(format.into());
        }
        node
    }

    pub fn media(classes: Vec<String>, content: MediaContent) -> Self {
        let mut node = Self::container("media", classes);
        node.kind = NodeKind::Media(content);
        node
    }

    pub fn text_content(&self) -> Option<&TextContent> {
        match &self.kind {
            NodeKind::Text(content) => Some(content),
            _ => None,
        }
    }

    pub fn text_content_mut(&mut self) -> Option<&mut TextContent> {
        match &mut self.kind {
            NodeKind::Text(content) => Some(content),
            _ => None,
        }
    }

    pub fn media_content(&self) -> Option<&MediaContent> {
        match &self.kind {
            NodeKind::Media(content) => Some(content),
            _ => None,
        }
    }

    /// Recursively recompute styles from the stylesheet.
    pub fn resolve_styles(
        &mut self,
        cascade: &Cascade<'_>,
        ancestors: &mut Vec<ElementDesc>,
        parent: Option<&ComputedStyle>,
        index: usize,
        sibling_count: usize,
    ) {
        let desc = ElementDesc {
            tag: self.tag.clone(),
            id: self.id.clone(),
            classes: self.classes.clone(),
            index,
            sibling_count,
        };
        let ancestor_elements: Vec<Element<'_>> =
            ancestors.iter().map(ElementDesc::element).collect();
        let element = desc.element();
        self.style = cascade.compute(
            &element,
            &ancestor_elements,
            parent,
            &self.inline_style,
            &self.config_style,
        );
        self.z_index = self.style.z_index;
        self.visible = self.style.visibility != crate::css::Visibility::Hidden
            && self.style.visibility != crate::css::Visibility::Collapse
            && self.style.display != crate::css::Display::None;

        let parent_style = self.style.clone();
        ancestors.push(desc);
        let total = self.children.len();
        for (index, child) in self.children.iter_mut().enumerate() {
            child.resolve_styles(cascade, ancestors, Some(&parent_style), index, total);
        }
        ancestors.pop();
    }

    /// Recompute layout for the whole subtree.
    pub fn layout_tree(
        &mut self,
        engine: &mut LayoutEngine<'_>,
        viewport: Viewport,
        containing_width: f32,
        containing_height: f32,
    ) {
        engine.layout_node(self, viewport, containing_width, containing_height);
        let (x, y) = self.anchor.position(
            viewport,
            self.layout.width,
            self.layout.height,
            self.offset_x,
            self.offset_y,
        );
        self.absolute_x = x;
        self.absolute_y = y;
        self.arrange_absolute(x, y);
    }

    fn arrange_absolute(&mut self, x: f32, y: f32) {
        self.absolute_x = x;
        self.absolute_y = y;
        for child in &mut self.children {
            let child_x = x + child.layout.x;
            let child_y = y + child.layout.y;
            child.arrange_absolute(child_x, child_y);
        }
    }

    /// Update clock text if needed. Returns `true` when content changed.
    pub fn tick(&mut self, now: chrono::DateTime<chrono::Local>) -> bool {
        let mut dirty = false;
        match &mut self.kind {
            NodeKind::Text(content) => {
                if let Some(format) = &content.clock_format {
                    let value = now.format(format).to_string();
                    if value != content.text {
                        content.text = value;
                        content.revision = content.revision.wrapping_add(1);
                        self.dirty = true;
                        dirty = true;
                    }
                }
            }
            NodeKind::Container => {}
            NodeKind::Media(content) => {
                if let MediaKind::Gif = content.kind {
                    // GIF playback is driven by the media store, which bumps the
                    // revision; bubbling the flag keeps the app repainting.
                    let _ = content.revision;
                }
            }
        }
        for child in &mut self.children {
            dirty |= child.tick(now);
        }
        dirty
    }

    pub fn set_text(&mut self, text: impl Into<String>) -> bool {
        if let NodeKind::Text(content) = &mut self.kind {
            let text = text.into();
            if text != content.text {
                content.text = text;
                content.revision = content.revision.wrapping_add(1);
                self.dirty = true;
                return true;
            }
        }
        false
    }

    pub fn find_by_uid_mut(&mut self, uid: u64) -> Option<&mut Node> {
        if self.uid == uid {
            return Some(self);
        }
        for child in &mut self.children {
            if let Some(found) = child.find_by_uid_mut(uid) {
                return Some(found);
            }
        }
        None
    }

    pub fn find_mut(&mut self, predicate: &mut dyn FnMut(&Node) -> bool) -> Option<&mut Node> {
        if predicate(self) {
            return Some(self);
        }
        for child in &mut self.children {
            if let Some(found) = child.find_mut(predicate) {
                return Some(found);
            }
        }
        None
    }

    pub fn descendants<'a>(&'a self, out: &mut Vec<&'a Node>) {
        out.push(self);
        for child in &self.children {
            child.descendants(out);
        }
    }

    pub fn clear_dirty(&mut self) {
        self.dirty = false;
        for child in &mut self.children {
            child.clear_dirty();
        }
    }

    pub fn has_dirty(&self) -> bool {
        self.dirty || self.children.iter().any(Node::has_dirty)
    }
}

/// Owned element data used while resolving the cascade.
#[derive(Clone, Debug)]
pub struct ElementDesc {
    pub tag: String,
    pub id: Option<String>,
    pub classes: Vec<String>,
    pub index: usize,
    pub sibling_count: usize,
}

impl ElementDesc {
    pub fn element(&self) -> Element<'_> {
        Element::new(
            &self.tag,
            self.id.as_deref(),
            &self.classes,
            self.index,
            self.sibling_count,
        )
    }
}

#[derive(Clone, Debug, Default)]
pub struct Scene {
    pub roots: Vec<Node>,
    pub viewport: Viewport,
}

impl Scene {
    pub fn new(viewport: Viewport) -> Self {
        Self {
            roots: Vec::new(),
            viewport,
        }
    }

    pub fn resolve_styles(&mut self, stylesheet: &crate::css::Stylesheet) {
        let cascade = Cascade::new(stylesheet, self.viewport);
        let total = self.roots.len();
        for (index, root) in self.roots.iter_mut().enumerate() {
            let mut ancestors = Vec::new();
            root.resolve_styles(&cascade, &mut ancestors, None, index, total);
        }
        self.roots.sort_by_key(|node| node.z_index);
    }

    pub fn layout(&mut self, engine: &mut LayoutEngine<'_>) {
        let viewport = self.viewport;
        let total = self.roots.len();
        for (index, root) in self.roots.iter_mut().enumerate() {
            let _ = index;
            let _ = total;
            root.layout_tree(engine, viewport, viewport.width, viewport.height);
        }
    }

    pub fn tick(&mut self, now: chrono::DateTime<chrono::Local>) -> bool {
        let mut dirty = false;
        for root in &mut self.roots {
            dirty |= root.tick(now);
        }
        dirty
    }

    pub fn has_dirty(&self) -> bool {
        self.roots.iter().any(Node::has_dirty)
    }

    pub fn clear_dirty(&mut self) {
        for root in &mut self.roots {
            root.clear_dirty();
        }
    }

    pub fn find_by_uid_mut(&mut self, uid: u64) -> Option<&mut Node> {
        for root in &mut self.roots {
            if let Some(found) = root.find_by_uid_mut(uid) {
                return Some(found);
            }
        }
        None
    }

    pub fn find_mut(&mut self, mut predicate: impl FnMut(&Node) -> bool) -> Option<&mut Node> {
        for root in &mut self.roots {
            if let Some(found) = root.find_mut(&mut predicate) {
                return Some(found);
            }
        }
        None
    }
}
