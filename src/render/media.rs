// SPDX-FileCopyrightText: 2026 Daniel
// SPDX-License-Identifier: GPL-3.0-or-later

//! Media asset cache.
//!
//! Images are decoded once and shared through `Arc`. GIF frames are decoded
//! once with a hard frame/pixel budget and advance from the scene clock. The
//! cache is bounded so a long-running desktop session cannot grow without
//! limit.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use image::AnimationDecoder;
use vello::peniko::{Blob, ImageAlphaType, ImageData, ImageFormat};

use crate::scene::{MediaContent, MediaKind};

const IMAGE_CACHE_LIMIT: usize = 64;
const GIF_FRAME_LIMIT: usize = 240;
const GIF_PIXEL_LIMIT: u64 = 64 * 1024 * 1024;

#[derive(Clone)]
pub struct GifAsset {
    pub frames: Vec<Arc<ImageData>>,
    pub durations: Vec<Duration>,
    pub elapsed: Duration,
    pub current: usize,
}

impl GifAsset {
    pub fn advance(&mut self, delta: Duration) -> bool {
        if self.frames.is_empty() {
            return false;
        }
        self.elapsed += delta;
        let mut changed = false;
        loop {
            let duration = self
                .durations
                .get(self.current)
                .copied()
                .unwrap_or(Duration::from_millis(100));
            if self.elapsed < duration {
                break;
            }
            self.elapsed -= duration;
            self.current = (self.current + 1) % self.frames.len();
            changed = true;
        }
        changed
    }

    pub fn image(&self) -> Option<Arc<ImageData>> {
        self.frames.get(self.current).cloned()
    }
}

#[derive(Default)]
pub struct MediaStore {
    base_dir: PathBuf,
    images: HashMap<PathBuf, Arc<ImageData>>,
    gifs: HashMap<PathBuf, GifAsset>,
    #[cfg(feature = "video")]
    videos: HashMap<PathBuf, crate::media::video::VideoAsset>,
    failed: HashMap<PathBuf, u64>,
    pub dirty: bool,
}

impl MediaStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_base_dir(&mut self, dir: impl Into<PathBuf>) {
        self.base_dir = dir.into();
        self.images.clear();
        self.gifs.clear();
        #[cfg(feature = "video")]
        self.videos.clear();
    }

    fn resolve_path(&self, path: &Path) -> PathBuf {
        if path.is_absolute() {
            return path.to_path_buf();
        }
        let candidate = self.base_dir.join(path);
        if candidate.exists() {
            candidate
        } else {
            path.to_path_buf()
        }
    }

    pub fn image(&mut self, media: &MediaContent) -> Option<Arc<ImageData>> {
        match media.kind {
            MediaKind::Image => self.image_for(&media.path),
            MediaKind::Gif => self.gif_image(&media.path),
            MediaKind::Video => self.video_frame(media),
        }
    }

    pub fn image_for(&mut self, path: &Path) -> Option<Arc<ImageData>> {
        let key = self.resolve_path(path);
        if let Some(image) = self.images.get(&key) {
            return Some(Arc::clone(image));
        }
        match load_image(&key) {
            Ok(image) => {
                let image = Arc::new(image);
                if self.images.len() >= IMAGE_CACHE_LIMIT {
                    self.images.clear();
                }
                self.images.insert(key, Arc::clone(&image));
                Some(image)
            }
            Err(error) => {
                log::warn!("failed to load image {}: {error:#}", key.display());
                self.failed.insert(key, 0);
                None
            }
        }
    }

    fn gif_image(&mut self, path: &Path) -> Option<Arc<ImageData>> {
        let key = self.resolve_path(path);
        if !self.gifs.contains_key(&key) {
            match load_gif(&key) {
                Ok(asset) => {
                    self.gifs.insert(key.clone(), asset);
                }
                Err(error) => {
                    log::warn!("failed to load gif {}: {error:#}", key.display());
                    self.failed.insert(key, 0);
                    return None;
                }
            }
        }
        self.gifs.get(&key).and_then(GifAsset::image)
    }

    #[cfg(feature = "video")]
    fn video_frame(&mut self, media: &MediaContent) -> Option<Arc<ImageData>> {
        let key = media.path.clone();
        if !self.videos.contains_key(&key) {
            match crate::media::video::VideoAsset::new(&media.path, media.autoplay, media.looping) {
                Ok(asset) => {
                    self.videos.insert(key.clone(), asset);
                }
                Err(error) => {
                    log::warn!("failed to open video {}: {error:#}", media.path.display());
                    self.failed.insert(key, 0);
                    return None;
                }
            }
        }
        self.videos.get(&key).and_then(|video| video.frame())
    }

    #[cfg(not(feature = "video"))]
    fn video_frame(&mut self, _media: &MediaContent) -> Option<Arc<ImageData>> {
        None
    }

    /// Advance animated assets. Returns `true` when a frame changed.
    pub fn update(&mut self, delta: Duration) -> bool {
        let mut changed = false;
        for gif in self.gifs.values_mut() {
            changed |= gif.advance(delta);
        }
        #[cfg(feature = "video")]
        for video in self.videos.values_mut() {
            changed |= video.pump();
        }
        self.dirty = changed;
        changed
    }

    pub fn has_video(&self) -> bool {
        #[cfg(feature = "video")]
        {
            !self.videos.is_empty()
        }
        #[cfg(not(feature = "video"))]
        {
            false
        }
    }

    /// Drop assets that are no longer referenced by the scene.
    pub fn retain_paths(&mut self, paths: &std::collections::HashSet<PathBuf>) {
        self.images.retain(|path, _| paths.contains(path));
        self.gifs.retain(|path, _| paths.contains(path));
        #[cfg(feature = "video")]
        self.videos.retain(|path, _| paths.contains(path));
        self.failed.retain(|path, _| paths.contains(path));
    }
}

fn load_image(path: &Path) -> Result<ImageData> {
    let image = image::open(path)
        .with_context(|| format!("decoding {}", path.display()))?
        .to_rgba8();
    let (width, height) = image.dimensions();
    Ok(ImageData {
        data: Blob::from(image.into_raw()),
        format: ImageFormat::Rgba8,
        alpha_type: ImageAlphaType::Alpha,
        width,
        height,
    })
}

fn load_gif(path: &Path) -> Result<GifAsset> {
    let file = std::fs::File::open(path).with_context(|| format!("opening {}", path.display()))?;
    let decoder = image::codecs::gif::GifDecoder::new(std::io::BufReader::new(file))
        .with_context(|| format!("reading gif {}", path.display()))?;
    let frames = decoder
        .into_frames()
        .collect_frames()
        .with_context(|| format!("decoding gif frames {}", path.display()))?;

    let mut total_pixels = 0_u64;
    let mut assets = Vec::new();
    let mut durations = Vec::new();
    for frame in frames.into_iter().take(GIF_FRAME_LIMIT) {
        let (numerator, denominator) = frame.delay().numer_denom_ms();
        let delay = numerator
            .checked_div(denominator)
            .map(|value| Duration::from_millis(value.max(20) as u64))
            .unwrap_or(Duration::from_millis(100));
        let image = frame.into_buffer();
        let (width, height) = image.dimensions();
        total_pixels += u64::from(width) * u64::from(height);
        if total_pixels > GIF_PIXEL_LIMIT {
            break;
        }
        assets.push(Arc::new(ImageData {
            data: Blob::from(image.into_raw()),
            format: ImageFormat::Rgba8,
            alpha_type: ImageAlphaType::Alpha,
            width,
            height,
        }));
        durations.push(delay);
    }
    if assets.is_empty() {
        anyhow::bail!("gif has no frames");
    }
    Ok(GifAsset {
        frames: assets,
        durations,
        elapsed: Duration::ZERO,
        current: 0,
    })
}
