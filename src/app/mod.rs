// SPDX-FileCopyrightText: 2026 Daniel
// SPDX-License-Identifier: GPL-3.0-or-later

//! Application runtime: configuration, styles, layout, animation and hot
//! reload.
//!
//! The platform shell owns the window and GPU surface; this module owns the
//! document. That split keeps the Wayland code small and makes the runtime
//! testable without a compositor.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use vello::Scene as VelloScene;

use crate::api::ApiManager;
use crate::config::ConfigLoader;
use crate::css::{Stylesheet, Viewport};
use crate::layout::LayoutEngine;
use crate::render::{MediaStore, Painter};
use crate::scene::{MediaKind, NodeKind, Scene};
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
    dirty: bool,
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
            self.dirty = true;
        }
        if self.media.update(delta) {
            self.dirty = true;
        }
        self.dirty
    }

    pub fn paint(&mut self) -> &VelloScene {
        self.canvas.reset();
        let mut painter = Painter::new(&mut self.text, &mut self.media);
        painter.paint(&self.scene, &mut self.canvas);
        self.dirty = false;
        self.scene.clear_dirty();
        &self.canvas
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
        self.scene.roots.iter().any(node_animated)
    }

    pub fn invalidate(&mut self) {
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
