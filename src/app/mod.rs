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
use vello::kurbo::{Affine, Rect};
use vello::peniko::{Color, Fill, Mix};

use crate::animation::{self, AnimationSet};
use crate::api::ApiManager;
use crate::config::ConfigLoader;
use crate::css::properties::ClipPath;
use crate::css::{Axis, Overflow, Preferences, Stylesheet, Viewport, Visibility};
use crate::layout::LayoutEngine;
use crate::render::media_painter::media_draw_rect;
use crate::render::{
    FilterLayer, MediaStore, Painter, ShadowDraw, VideoFrame, root_spatial_filter,
};
use crate::scene::{MediaKind, Node, NodeKind, Scene};
use crate::text::TextEngine;
use crate::user_config_path;

/// Shadow blur and shadow list refresh rate while animations are running.
const SHADOW_INTERVAL: Duration = Duration::from_millis(16);

/// Shadow masks are rasterized and blurred at half resolution. A blurred
/// coverage is smooth, so upscaling it is invisible and costs a quarter of
/// the full-resolution passes.
pub const SHADOW_MASK_SCALE: f64 = 0.5;

/// Backdrop masks and blurred video run at half resolution. Glass is soft, so
/// the upscale is invisible and the blur cost drops to a quarter.
pub const BACKDROP_MASK_SCALE: f64 = 0.35;

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
    shadow_region: Option<[f32; 4]>,
    backdrop_mask: VelloScene,
    backdrop_radius: Option<f32>,
    backdrop_region: Option<[f32; 4]>,
    last_shadow_build: Instant,
    viewport: Viewport,
    last_tick: Instant,
    pending_reload: Option<Instant>,
    watcher: Option<RecommendedWatcher>,
    events: Option<Receiver<notify::Result<Event>>>,
    loaded_fonts: HashSet<String>,
    config_mtime: Option<std::time::SystemTime>,
    css_mtime: Option<std::time::SystemTime>,
    preferences: Preferences,
    overlay_dirty: bool,
    shadows_dirty: bool,
    fast_mode: bool,
    animations: HashMap<u64, AnimationSet>,
    transitions: HashMap<u64, crate::animation::TransitionSet>,
    animations_active: bool,
    last_animation_sample: Instant,
    auto_config: bool,
    dirty: bool,
}

/// What the platform should draw for the current frame.
pub struct FramePlan {
    pub fast: bool,
    pub overlay_updated: bool,
    /// True when the shadow blur and shadow list must be refreshed.
    pub shadows_updated: bool,
    /// Render the cached overlay and GPU shadows through the video compositor
    /// even without a video (the no-filter full path).
    pub compose: bool,
    pub video: Option<VideoFrame>,
    /// True when `Runtime::render_layers()` must be used instead of the single
    /// cached overlay scene. Only set for the full (non-video) path.
    pub filtered: bool,
}

impl Runtime {
    pub fn from_default_config(viewport: Viewport) -> Result<Self> {
        let mut runtime = Self::new(user_config_path(), viewport)?;
        runtime.auto_config = true;
        Ok(runtime)
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
        let preferences = Preferences::detect();
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
            shadow_region: None,
            backdrop_mask: VelloScene::new(),
            backdrop_radius: None,
            backdrop_region: None,
            last_shadow_build: Instant::now() - Duration::from_secs(1),
            viewport,
            last_tick: Instant::now(),
            pending_reload: None,
            watcher: None,
            events: None,
            loaded_fonts: HashSet::new(),
            config_mtime,
            css_mtime,
            preferences,
            overlay_dirty: true,
            shadows_dirty: true,
            fast_mode: false,
            animations: HashMap::new(),
            transitions: HashMap::new(),
            animations_active: false,
            last_animation_sample: Instant::now() - Duration::from_secs(1),
            auto_config: false,
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

    /// White coverage of the panels that request `backdrop-filter`.
    pub fn backdrop_mask(&self) -> &VelloScene {
        &self.backdrop_mask
    }

    /// Largest `backdrop-filter: blur()` radius in the scene, if any.
    pub fn backdrop_radius(&self) -> Option<f32> {
        self.backdrop_radius
    }

    pub fn shadow_region(&self) -> Option<[f32; 4]> {
        self.shadow_region
    }

    pub fn backdrop_region(&self) -> Option<[f32; 4]> {
        self.backdrop_region
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
                        // With the default config, creating config.py switches
                        // the active source without a restart.
                        if self.auto_config
                            && matches!(
                                path.file_name().and_then(|name| name.to_str()),
                                Some("config.py") | Some("config.jsonc")
                            )
                        {
                            let preferred = user_config_path();
                            if preferred != self.config.path {
                                self.config.path = preferred;
                                self.config_mtime = mtime(&self.config.path);
                                changed = true;
                            }
                        }
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
        // Capture what is currently on screen before replacing the tree so
        // style changes can transition from the old values.
        let previous = animation::capture_values(&self.scene);
        self.stylesheet = stylesheet;
        self.scene = scene;
        self.preferences = Preferences::detect();
        self.api.reset();
        self.apply_styles_and_layout();

        let now = Instant::now();
        self.transitions.clear();
        let (transitions, transition_active) =
            animation::start_transitions(&mut self.scene, &previous, now);
        self.transitions = transitions;
        self.animations_active |= transition_active;
        let (changed, glow_changed, overlay_active) =
            animation::update_scene(&mut self.scene, &self.animations, &self.transitions, now);
        self.animations_active |= overlay_active;
        if changed {
            self.overlay_dirty = true;
            self.shadows_dirty = true;
            self.dirty = true;
        }
        if glow_changed {
            self.shadows_dirty = true;
            self.dirty = true;
        }

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
        self.scene
            .resolve_styles_with_preferences(&self.stylesheet, self.preferences);
        self.scene.layout(&mut LayoutEngine::new(&mut self.text));
        let paths = media_paths(&self.scene);
        self.media.retain_paths(&paths);
        let now = Instant::now();
        let (animations, active) = animation::prepare_scene(&mut self.scene, &self.stylesheet, now);
        self.animations = animations;
        self.animations_active = active;
        self.last_animation_sample = Instant::now() - Duration::from_secs(1);
        self.rebuild_shadows();
        self.rebuild_backdrop_mask();
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
                animation::update_scene(&mut self.scene, &self.animations, &self.transitions, now);
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
        eprintln!(
            "DEBUG shadows items={} region={:?}",
            self.shadow_items.len(),
            self.shadow_region
        );
        self.shadow_region = crate::render::painter::shadow_region(&self.scene, self.viewport);
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
        eprintln!(
            "DEBUG mask empty={} items={}",
            self.shadow_mask.encoding().is_empty(),
            self.shadow_items.len()
        );
    }

    /// Rasterize the boxes of backdrop-filter panels. The mask is static until
    /// the layout or styles change.
    fn rebuild_backdrop_mask(&mut self) {
        self.backdrop_mask = VelloScene::new();
        self.backdrop_radius = None;
        self.backdrop_region = None;
        let mut radius: Option<f32> = None;
        let mut region: Option<[f32; 4]> = None;
        fn visit(node: &Node, radius: &mut Option<f32>, region: &mut Option<[f32; 4]>) {
            if let Some(blur) = node.backdrop_blur() {
                *radius = Some(radius.map_or(blur, |current: f32| current.max(blur)));
                let margin = blur * 2.5 + 8.0;
                let x0 = node.absolute_x - margin;
                let y0 = node.absolute_y - margin;
                let x1 = node.absolute_x + node.layout.width + margin;
                let y1 = node.absolute_y + node.layout.height + margin;
                *region = Some(match *region {
                    Some([bx, by, bw, bh]) => [
                        bx.min(x0),
                        by.min(y0),
                        (bx + bw).max(x1) - bx.min(x0),
                        (by + bh).max(y1) - by.min(y0),
                    ],
                    None => [x0, y0, x1 - x0, y1 - y0],
                });
            }
            for child in &node.children {
                visit(child, radius, region);
            }
        }
        for root in &self.scene.roots {
            visit(root, &mut radius, &mut region);
        }
        self.backdrop_radius = radius;
        self.backdrop_region = region.and_then(|[x, y, width, height]| {
            let x0 = x.max(0.0);
            let y0 = y.max(0.0);
            let x1 = (x + width).min(self.viewport.width).max(x0);
            let y1 = (y + height).min(self.viewport.height).max(y0);
            (x1 > x0 && y1 > y0).then_some([x0, y0, x1 - x0, y1 - y0])
        });
        if radius.is_none() {
            return;
        }
        clear_scene(&mut self.backdrop_mask, self.viewport, BACKDROP_MASK_SCALE);
        let mut painter = Painter::new(&mut self.text, &mut self.media);
        painter.paint_backdrop_masks(&self.scene, &mut self.backdrop_mask, BACKDROP_MASK_SCALE);
    }

    /// Prepare the next frame. Fast path videos are drawn as a single wgpu
    /// texture; the Vello overlay is only re-rendered when its content changes.
    pub fn prepare_frame(&mut self) -> FramePlan {
        // Spatial filters need offscreen layers, which the fast video
        // compositor cannot provide. Fall back to the full Vello path for such
        // scenes instead of regressing video playback for everyone else.
        let has_filtered_root = self.scene.roots.iter().any(|root| {
            root_spatial_filter(root).is_some() || crate::render::painter::subtree_has_filter(root)
        });
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
                    self.rebuild_backdrop_mask();
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
                compose: false,
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
            painter.inline_shadows = false;
            painter.paint(&self.scene, &mut self.canvas);
            self.rebuild_shadows();
            self.rebuild_shadow_mask();
            self.rebuild_backdrop_mask();
            self.overlay_dirty = false;
            self.shadows_dirty = false;
            self.last_shadow_build = Instant::now();
            self.dirty = false;
            self.scene.clear_dirty();
            return FramePlan {
                fast: false,
                overlay_updated: true,
                shadows_updated: true,
                compose: true,
                video: None,
                filtered: false,
            };
        }

        let mut layers = Vec::with_capacity(self.scene.roots.len());
        let mut painter = Painter::new(&mut self.text, &mut self.media);
        painter.paint_layers(&self.scene, &mut layers);
        self.layers = layers;
        self.overlay_dirty = false;
        self.dirty = false;
        self.scene.clear_dirty();
        FramePlan {
            fast: false,
            overlay_updated: true,
            shadows_updated: false,
            compose: false,
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
