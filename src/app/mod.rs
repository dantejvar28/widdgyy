// SPDX-FileCopyrightText: 2026 Daniel
// SPDX-License-Identifier: GPL-3.0-or-later

//! Application runtime: configuration, styles, layout, animation and hot
//! reload.
//!
//! The platform shell owns the window and GPU surface; this module owns the
//! document. That split keeps the Wayland code small and makes the runtime
//! testable without a compositor.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use vello::Scene as VelloScene;
use vello::peniko::Mix;

use crate::animation::{self, AnimationSet};
use crate::api::ApiManager;
use crate::config::ConfigLoader;
use crate::css::properties::ClipPath;
use crate::css::{Axis, Overflow, Stylesheet, Viewport, Visibility};
use crate::layout::LayoutEngine;
use crate::render::media_painter::media_draw_rect;
use crate::render::{MediaStore, Painter, VideoFrame};
use crate::scene::{MediaKind, Node, NodeKind, Scene};
use crate::text::TextEngine;
use crate::user_config_dir;

pub struct Runtime {
    pub config: ConfigLoader,
    pub css_path: PathBuf,
    pub stylesheet: Stylesheet,
    pub scene: Scene,
    pub text: TextEngine,
    pub media: MediaStore,
    pub api: ApiManager,
    canvas: VelloScene,
    viewport: Viewport,
    last_tick: Instant,
    pending_reload: Option<Instant>,
    watcher: Option<RecommendedWatcher>,
    events: Option<Receiver<notify::Result<Event>>>,
    loaded_fonts: HashSet<String>,
    config_mtime: Option<std::time::SystemTime>,
    css_mtime: Option<std::time::SystemTime>,
    overlay_dirty: bool,
    fast_mode: bool,
    animations: HashMap<u64, AnimationSet>,
    animations_active: bool,
    dirty: bool,
}

/// What the platform should draw for the current frame.
pub struct FramePlan {
    pub fast: bool,
    pub overlay_updated: bool,
    pub video: Option<VideoFrame>,
}

impl Runtime {
    pub fn from_default_config(viewport: Viewport) -> Result<Self> {
        let dir = user_config_dir();
        Self::new(dir.join("config.jsonc"), viewport)
    }

    pub fn new(config_path: impl AsRef<Path>, viewport: Viewport) -> Result<Self> {
        let config = ConfigLoader::new(config_path.as_ref());
        let css_path = config.config_dir().join("styles.css");
        let stylesheet = load_stylesheet(&css_path);
        let scene = config
            .load(viewport)
            .with_context(|| format!("loading {}", config.path.display()))?;
        let text = TextEngine::new();
        let config_mtime = mtime(&config.path);
        let css_mtime = mtime(&css_path);
        let mut runtime = Self {
            config,
            css_path,
            stylesheet,
            scene,
            text,
            media: MediaStore::new(),
            api: ApiManager::new(),
            canvas: VelloScene::new(),
            viewport,
            last_tick: Instant::now(),
            pending_reload: None,
            watcher: None,
            events: None,
            loaded_fonts: HashSet::new(),
            config_mtime,
            css_mtime,
            overlay_dirty: true,
            fast_mode: false,
            animations: HashMap::new(),
            animations_active: false,
            dirty: true,
        };
        runtime.media.set_base_dir(runtime.config.config_dir());
        runtime.apply_styles_and_layout();
        runtime.start_watcher();
        Ok(runtime)
    }

    pub fn viewport(&self) -> Viewport {
        self.viewport
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        let width = width as f32;
        let height = height as f32;
        if (self.viewport.width - width).abs() < 0.5 && (self.viewport.height - height).abs() < 0.5
        {
            return;
        }
        self.viewport.width = width;
        self.viewport.height = height;
        self.apply_styles_and_layout();
        self.dirty = true;
    }

    fn start_watcher(&mut self) {
        let (sender, receiver) = channel();
        let dir = self.config.config_dir().to_path_buf();
        let mut watcher = match notify::recommended_watcher(move |event| {
            let _ = sender.send(event);
        }) {
            Ok(watcher) => watcher,
            Err(error) => {
                log::warn!("cannot start hot reload watcher: {error}");
                return;
            }
        };
        if let Err(error) = watcher.watch(&dir, RecursiveMode::NonRecursive) {
            log::warn!("cannot watch {}: {error}", dir.display());
            return;
        }
        self.watcher = Some(watcher);
        self.events = Some(receiver);
    }

    fn drain_watcher(&mut self) {
        let Some(events) = &self.events else {
            return;
        };
        let mut changed = false;
        loop {
            match events.try_recv() {
                Ok(Ok(event)) => {
                    if !matches!(
                        event.kind,
                        notify::EventKind::Modify(_)
                            | notify::EventKind::Create(_)
                            | notify::EventKind::Remove(_)
                    ) {
                        continue;
                    }
                    log::debug!("watch event {:?} {:?}", event.kind, event.paths);
                    for path in &event.paths {
                        if path == &self.config.path {
                            let current = mtime(&self.config.path);
                            if current != self.config_mtime {
                                self.config_mtime = current;
                                changed = true;
                            }
                        } else if path == &self.css_path {
                            let current = mtime(&self.css_path);
                            if current != self.css_mtime {
                                self.css_mtime = current;
                                changed = true;
                            }
                        }
                    }
                }
                Ok(Err(error)) => log::warn!("hot reload watch error: {error}"),
                Err(std::sync::mpsc::TryRecvError::Empty) => break,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => break,
            }
        }
        if changed {
            self.pending_reload = Some(Instant::now() + Duration::from_millis(120));
        }
        if let Some(deadline) = self.pending_reload
            && Instant::now() >= deadline
        {
            self.pending_reload = None;
            self.reload();
        }
    }

    pub fn reload(&mut self) {
        let stylesheet = load_stylesheet(&self.css_path);
        let scene = match self.config.load(self.viewport) {
            Ok(scene) => scene,
            Err(error) => {
                log::warn!("reload failed, keeping previous scene: {error:#}");
                return;
            }
        };
        self.stylesheet = stylesheet;
        self.scene = scene;
        self.api.reset();
        self.apply_styles_and_layout();
        self.config_mtime = mtime(&self.config.path);
        self.css_mtime = mtime(&self.css_path);
        self.dirty = true;
        log::info!("configuration reloaded");
    }

    fn apply_styles_and_layout(&mut self) {
        let config_dir = self.config.config_dir().to_path_buf();
        for face in &self.stylesheet.font_faces {
            if !self.loaded_fonts.insert(face.source.clone()) {
                continue;
            }
            let path = config_dir.join(&face.source);
            if !self.text.load_font_file(&path) {
                log::warn!(
                    "cannot load font face {} from {}",
                    face.family,
                    path.display()
                );
            }
        }
        self.scene.viewport = self.viewport;
        self.scene.resolve_styles(&self.stylesheet);
        self.scene.layout(&mut LayoutEngine::new(&mut self.text));
        let paths = media_paths(&self.scene);
        self.media.retain_paths(&paths);
        let now = Instant::now();
        let (animations, active) = animation::prepare_scene(&mut self.scene, &self.stylesheet, now);
        self.animations = animations;
        self.animations_active = active;
        self.overlay_dirty = true;
        self.fast_mode = false;
        self.dirty = true;
    }

    /// Advance clocks, GIFs, API requests and hot reload. Returns `true` when
    /// the surface needs a repaint.
    pub fn tick(&mut self, now: Instant) -> bool {
        let delta = now.saturating_duration_since(self.last_tick);
        self.last_tick = now;

        self.drain_watcher();

        let mut content_changed = self.scene.tick(chrono::Local::now());
        content_changed |= self.api.tick(&mut self.scene, now);
        if content_changed {
            self.scene.layout(&mut LayoutEngine::new(&mut self.text));
            self.overlay_dirty = true;
            self.dirty = true;
        }
        let media_changes = self.media.update(delta);
        if media_changes.overlay {
            self.overlay_dirty = true;
            self.dirty = true;
        }
        if media_changes.video {
            self.dirty = true;
        }
        let (animation_changed, animation_active) =
            animation::update_scene(&mut self.scene, &self.animations, now);
        if animation_changed {
            self.overlay_dirty = true;
            self.dirty = true;
        }
        self.animations_active = animation_active;
        self.dirty
    }

    /// Prepare the next frame. Fast path videos are drawn as a single wgpu
    /// texture; the Vello overlay is only re-rendered when its content changes.
    pub fn prepare_frame(&mut self) -> FramePlan {
        let candidate = self.background_video_node().and_then(|node| {
            let media = node.media_content()?.clone();
            let (x, y, width, height) = media_draw_rect(node)?;
            Some((
                node.uid,
                media,
                node.absolute_x + x,
                node.absolute_y + y,
                width,
                height,
            ))
        });

        if let Some((uid, media, x, y, width, height)) = candidate
            && let Some(image) = self.media.video_data(&media)
        {
            if !self.fast_mode {
                self.fast_mode = true;
                self.overlay_dirty = true;
            }
            let overlay_updated = self.overlay_dirty;
            if overlay_updated {
                self.canvas.reset();
                let mut painter = Painter::new(&mut self.text, &mut self.media);
                painter.paint_filtered(&self.scene, &mut self.canvas, Some(uid));
                self.overlay_dirty = false;
            }
            self.dirty = false;
            self.scene.clear_dirty();
            return FramePlan {
                fast: true,
                overlay_updated,
                video: Some(VideoFrame {
                    data: image,
                    x,
                    y,
                    width: width.max(1.0),
                    height: height.max(1.0),
                }),
            };
        }

        self.fast_mode = false;
        self.prepare_full()
    }

    fn prepare_full(&mut self) -> FramePlan {
        self.canvas.reset();
        let mut painter = Painter::new(&mut self.text, &mut self.media);
        painter.paint(&self.scene, &mut self.canvas);
        self.overlay_dirty = false;
        self.dirty = false;
        self.scene.clear_dirty();
        FramePlan {
            fast: false,
            overlay_updated: true,
            video: None,
        }
    }

    pub fn overlay_scene(&self) -> &VelloScene {
        &self.canvas
    }

    fn background_video_node(&self) -> Option<&Node> {
        let mut video = None;
        for root in &self.scene.roots {
            if let NodeKind::Media(media) = &root.kind
                && media.kind == MediaKind::Video
            {
                if video.is_some() {
                    return None;
                }
                video = Some(root);
            }
        }
        let video = video?;
        if !fast_style_ok(video) {
            return None;
        }
        for root in &self.scene.roots {
            if root.uid != video.uid && root.z_index <= video.z_index {
                return None;
            }
        }
        if self
            .scene
            .roots
            .iter()
            .any(|root| has_video(root, Some(video.uid)))
        {
            return None;
        }
        Some(video)
    }

    pub fn needs_animation(&self) -> bool {
        fn node_animated(node: &crate::scene::Node) -> bool {
            match &node.kind {
                NodeKind::Media(media) => {
                    matches!(media.kind, MediaKind::Gif | MediaKind::Video)
                }
                // Clocks and API results only need the 200 ms idle cadence.
                NodeKind::Text(_) => false,
                NodeKind::Container => node.children.iter().any(node_animated),
            }
        }
        self.scene.roots.iter().any(node_animated) || self.animations_active
    }

    pub fn invalidate(&mut self) {
        self.overlay_dirty = true;
        self.dirty = true;
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Look up parsed `@keyframes` data for a named animation.
    pub fn keyframes(&self, name: &str) -> Option<crate::css::properties::transition::Keyframes> {
        self.stylesheet.keyframes.get(name).cloned()
    }
}

fn has_video(node: &Node, skip_uid: Option<u64>) -> bool {
    if skip_uid == Some(node.uid) {
        return false;
    }
    if let NodeKind::Media(media) = &node.kind
        && media.kind == MediaKind::Video
    {
        return true;
    }
    node.children.iter().any(|child| has_video(child, skip_uid))
}

fn fast_style_ok(node: &Node) -> bool {
    let style = &node.style;
    if !node.visible
        || style.display == crate::css::Display::None
        || style.visibility != Visibility::Visible
    {
        return false;
    }
    if style.effects.opacity < 0.999
        || !style.effects.transform.is_empty()
        || style.effects.clip_path != ClipPath::None
        || !style.effects.box_shadows.is_empty()
        || !style.effects.filters.is_empty()
        || !style.effects.backdrop_filters.is_empty()
        || style.effects.mix_blend_mode != Mix::Normal
        || !style.animations.is_empty()
    {
        return false;
    }
    if !style.background.images.is_empty() || style.background.color.components[3] > 0.0 {
        return false;
    }
    let context = style.resolve_context(Viewport::default(), node.layout.width, node.layout.height);
    let border = style.border.widths.resolve(&context, Axis::Horizontal);
    if border.top > 0.0 || border.right > 0.0 || border.bottom > 0.0 || border.left > 0.0 {
        return false;
    }
    let radius = style.border.radius.resolve(&context, Axis::Horizontal);
    if radius.top > 0.0 || radius.right > 0.0 || radius.bottom > 0.0 || radius.left > 0.0 {
        return false;
    }
    if style
        .border
        .outline_width
        .resolve(&context, Axis::Horizontal)
        > 0.0
    {
        return false;
    }
    style.overflow_x == Overflow::Visible && style.overflow_y == Overflow::Visible
}

fn load_stylesheet(path: &Path) -> Stylesheet {
    match std::fs::read_to_string(path) {
        Ok(css) => {
            let stylesheet = Stylesheet::parse(&css);
            for error in &stylesheet.errors {
                log::warn!("CSS: {error}");
            }
            stylesheet
        }
        Err(error) => {
            log::warn!("cannot read {}: {error}", path.display());
            Stylesheet::default()
        }
    }
}

fn mtime(path: &Path) -> Option<std::time::SystemTime> {
    std::fs::metadata(path)
        .ok()
        .and_then(|metadata| metadata.modified().ok())
}

fn media_paths(scene: &Scene) -> HashSet<PathBuf> {
    let mut paths = HashSet::new();
    collect_media_paths(&scene.roots, &mut paths);
    paths
}

fn collect_media_paths(nodes: &[crate::scene::Node], paths: &mut HashSet<PathBuf>) {
    for node in nodes {
        if let NodeKind::Media(media) = &node.kind {
            paths.insert(media.path.clone());
        }
        collect_media_paths(&node.children, paths);
    }
}
