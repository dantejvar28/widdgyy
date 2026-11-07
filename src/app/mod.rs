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
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use vello::Scene as VelloScene;
use vello::kurbo::{Affine, Rect};
use vello::peniko::{Color, Fill, Mix};

use crate::animation::{self, AnimationSet};
use crate::api::ApiManager;
use crate::config::ConfigLoader;
use crate::css::properties::ClipPath;
use crate::css::{Axis, Overflow, Stylesheet, Viewport, Visibility};
use crate::layout::LayoutEngine;
use crate::render::media_painter::media_draw_rect;
use crate::render::{
    FilterLayer, MediaStore, Painter, ShadowDraw, VideoFrame, has_spatial_filter,
    root_spatial_filter,
};
use crate::scene::{MediaKind, Node, NodeKind, Scene};
use crate::text::TextEngine;
use crate::user_config_dir;

/// Shadow blur and shadow list refresh rate while animations are running.
const SHADOW_INTERVAL: Duration = Duration::from_millis(16);

/// Shadow masks are rasterized and blurred at half resolution. A blurred
/// coverage is smooth, so upscaling it is invisible and costs a quarter of
/// the full-resolution passes.
pub const SHADOW_MASK_SCALE: f64 = 0.5;

/// Fill a scene with a full-viewport transparent rect. Vello processes every
/// tile when a shape covers it, which overwrites stale pixels from the last
/// render. A non-zero alpha stops the rect from being culled as empty.
fn clear_scene(scene: &mut VelloScene, viewport: Viewport, scale: f64) {
    let rect = Rect::new(0.0, 0.0, viewport.width as f64, viewport.height as f64);
    scene.fill(
        Fill::NonZero,
        Affine::scale(scale),
        Color::new([0.0, 0.0, 0.0, 1.0 / 255.0]),
        None,
        &rect,
    );
}

pub struct Runtime {
    pub config: ConfigLoader,
    pub css_path: PathBuf,
    pub stylesheet: Stylesheet,
    pub scene: Scene,
    pub text: TextEngine,
    pub media: MediaStore,
    pub api: ApiManager,
    canvas: VelloScene,
    layers: Vec<FilterLayer>,
    shadow_items: Vec<ShadowDraw>,
    shadow_mask: VelloScene,
    last_shadow_build: Instant,
    viewport: Viewport,
    last_tick: Instant,
    pending_reload: Option<Instant>,
    watcher: Option<RecommendedWatcher>,
    events: Option<Receiver<notify::Result<Event>>>,
    loaded_fonts: HashSet<String>,
    config_mtime: Option<std::time::SystemTime>,
    css_mtime: Option<std::time::SystemTime>,
    overlay_dirty: bool,
    shadows_dirty: bool,
    fast_mode: bool,
    animations: HashMap<u64, AnimationSet>,
    animations_active: bool,
    last_animation_sample: Instant,
    dirty: bool,
}

/// What the platform should draw for the current frame.
pub struct FramePlan {
    pub fast: bool,
    pub overlay_updated: bool,
    /// True when the shadow blur and shadow list must be refreshed.
    pub shadows_updated: bool,
    pub video: Option<VideoFrame>,
    /// True when `Runtime::render_layers()` must be used instead of the single
    /// cached overlay scene. Only set for the full (non-video) path.
    pub filtered: bool,
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
            layers: Vec::new(),
            shadow_items: Vec::new(),
            shadow_mask: VelloScene::new(),
            last_shadow_build: Instant::now() - Duration::from_secs(1),
            viewport,
            last_tick: Instant::now(),
            pending_reload: None,
            watcher: None,
            events: None,
            loaded_fonts: HashSet::new(),
            config_mtime,
            css_mtime,
            overlay_dirty: true,
            shadows_dirty: true,
            fast_mode: false,
            animations: HashMap::new(),
            animations_active: false,
            last_animation_sample: Instant::now() - Duration::from_secs(1),
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

    /// Text shadows in paint order for the current frame.
    pub fn shadow_items(&self) -> &[ShadowDraw] {
        &self.shadow_items
    }

    /// Glyph coverage of all shadowed text. Re-rendered only when the text
    /// changes, not when a shadow animates.
    pub fn shadow_mask(&self) -> &VelloScene {
        &self.shadow_mask
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
        self.last_animation_sample = Instant::now() - Duration::from_secs(1);
        self.rebuild_shadows();
        self.layers.clear();
        self.overlay_dirty = true;
        self.shadows_dirty = true;
        self.last_shadow_build = Instant::now() - Duration::from_secs(1);
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
            self.shadows_dirty = true;
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
        // The Wayland source can wake the loop far above video frame rate.
        // Sampling CSS animations that often is wasted work, so cap it at
        // roughly 60 Hz.
        let animation_due = self.animations_active
            && now.saturating_duration_since(self.last_animation_sample)
                >= Duration::from_millis(16);
        if animation_due {
            self.last_animation_sample = now;
            let (animation_changed, glow_changed, animation_active) =
                animation::update_scene(&mut self.scene, &self.animations, now);
            if animation_changed {
                self.overlay_dirty = true;
                self.shadows_dirty = true;
                self.dirty = true;
            }
            if glow_changed {
                // Without the video compositor text shadows still live in the
                // overlay, so a shadow-only animation must repaint it.
                if !self.fast_mode {
                    self.overlay_dirty = true;
                }
                self.shadows_dirty = true;
                self.dirty = true;
            }
            self.animations_active = animation_active;
        }
        self.dirty
    }

    /// Rebuild the CPU-side shadow list from the current scene. This is cheap
    /// and runs whenever a shadow value changes.
    fn rebuild_shadows(&mut self) {
        self.shadow_items = crate::render::painter::collect_shadow_draws(&self.scene);
    }

    /// Rasterize the glyph coverage of all shadowed text into the mask scene.
    /// Skipped entirely when no text has a shadow.
    fn rebuild_shadow_mask(&mut self) {
        self.shadow_mask = VelloScene::new();
        if self.shadow_items.is_empty() {
            return;
        }
        clear_scene(&mut self.shadow_mask, self.viewport, SHADOW_MASK_SCALE);
        let mut painter = Painter::new(&mut self.text, &mut self.media);
        painter.paint_text_masks(&self.scene, &mut self.shadow_mask, SHADOW_MASK_SCALE);
    }

    /// Prepare the next frame. Fast path videos are drawn as a single wgpu
    /// texture; the Vello overlay is only re-rendered when its content changes.
    pub fn prepare_frame(&mut self) -> FramePlan {
        log_nested_spatial_filters(&self.scene);
        // Spatial filters need offscreen layers, which the fast video
        // compositor cannot provide. Fall back to the full Vello path for such
        // scenes instead of regressing video playback for everyone else.
        let has_filtered_root = self
            .scene
            .roots
            .iter()
            .any(|root| root_spatial_filter(root).is_some());
        self.layers.clear();

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

        if !has_filtered_root
            && let Some((uid, media, x, y, width, height)) = candidate
            && let Some(image) = self.media.video_data(&media)
        {
            if !self.fast_mode {
                self.fast_mode = true;
                self.overlay_dirty = true;
            }
            let overlay_updated = self.overlay_dirty;
            // Shadow animation only re-blurs the cached mask, so cap that work
            // at the frame interval while video keeps compositing.
            let shadows_due =
                self.shadows_dirty && self.last_shadow_build.elapsed() >= SHADOW_INTERVAL;
            let shadows_updated = overlay_updated || shadows_due;
            if overlay_updated {
                self.canvas.reset();
                clear_scene(&mut self.canvas, self.viewport, 1.0);
                let mut painter = Painter::new(&mut self.text, &mut self.media);
                painter.inline_shadows = false;
                painter.paint_filtered(&self.scene, &mut self.canvas, Some(uid));
                self.overlay_dirty = false;
            }
            if shadows_updated {
                self.rebuild_shadows();
                if overlay_updated {
                    self.rebuild_shadow_mask();
                }
                self.shadows_dirty = false;
                self.last_shadow_build = Instant::now();
            }
            self.dirty = false;
            self.scene.clear_dirty();
            return FramePlan {
                fast: true,
                overlay_updated,
                shadows_updated,
                video: Some(VideoFrame {
                    data: image,
                    x,
                    y,
                    width: width.max(1.0),
                    height: height.max(1.0),
                }),
                filtered: false,
            };
        }

        self.fast_mode = false;
        self.prepare_full(has_filtered_root)
    }

    fn prepare_full(&mut self, has_filtered_root: bool) -> FramePlan {
        self.canvas.reset();
        clear_scene(&mut self.canvas, self.viewport, 1.0);

        if !has_filtered_root {
            let mut painter = Painter::new(&mut self.text, &mut self.media);
            painter.paint(&self.scene, &mut self.canvas);
            self.overlay_dirty = false;
            self.dirty = false;
            self.scene.clear_dirty();
            return FramePlan {
                fast: false,
                overlay_updated: true,
                shadows_updated: false,
                video: None,
                filtered: false,
            };
        }

        // Keep paint order: consecutive unfiltered roots share a scene, while
        // each filtered root gets its own layer.
        let mut indices: Vec<usize> = (0..self.scene.roots.len()).collect();
        indices.sort_by_key(|index| self.scene.roots[*index].z_index);
        let mut layers = Vec::with_capacity(indices.len());
        let mut pending: Option<VelloScene> = None;
        {
            let mut painter = Painter::new(&mut self.text, &mut self.media);
            for index in indices {
                let root = &self.scene.roots[index];
                if let Some(filter) = root_spatial_filter(root) {
                    if let Some(scene) = pending.take() {
                        layers.push(FilterLayer {
                            scene,
                            filter: None,
                        });
                    }
                    let mut scene = VelloScene::new();
                    painter.paint_root(root, &mut scene);
                    layers.push(FilterLayer {
                        scene,
                        filter: Some(filter),
                    });
                } else {
                    let scene = pending.get_or_insert_with(VelloScene::new);
                    painter.paint_root(root, scene);
                }
            }
        }
        if let Some(scene) = pending.take() {
            layers.push(FilterLayer {
                scene,
                filter: None,
            });
        }

        self.layers = layers;
        self.overlay_dirty = false;
        self.dirty = false;
        self.scene.clear_dirty();
        FramePlan {
            fast: false,
            overlay_updated: true,
            shadows_updated: false,
            video: None,
            filtered: true,
        }
    }

    pub fn overlay_scene(&self) -> &VelloScene {
        &self.canvas
    }

    /// Root layers prepared by the last `prepare_frame` call. Only meaningful
    /// when `FramePlan::filtered` is true.
    pub fn render_layers(&self) -> &[FilterLayer] {
        &self.layers
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
        || style.effects.translate.is_some()
        || style.effects.rotate.is_some()
        || style.effects.scale.is_some()
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

/// Nested spatial filters are not rendered offscreen yet. Warn once so a
/// stylesheet using them does not fail silently.
fn log_nested_spatial_filters(scene: &Scene) {
    static CHECKED: AtomicBool = AtomicBool::new(false);
    if CHECKED.swap(true, Ordering::Relaxed) {
        return;
    }
    fn nested(node: &Node) -> bool {
        node.children
            .iter()
            .any(|child| has_spatial_filter(&child.style.effects.filters) || nested(child))
    }
    if scene.roots.iter().any(nested) {
        log::debug!(
            "nested CSS blur()/drop-shadow() filters are unsupported; \
             only root scene nodes are filtered"
        );
    }
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
