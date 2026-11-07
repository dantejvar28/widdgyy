// SPDX-FileCopyrightText: 2026 Daniel
// SPDX-License-Identifier: GPL-3.0-or-later

//! Vello scene painter.
//!
//! Style families live in their own modules so each painter is short and has no
//! nested property dispatch:
//!
//! * [`box_painter`] — background, border and shadows.
//! * [`text_painter`] — text and decorations.
//! * [`effects_painter`] — opacity, transform, blend and clipping.
//! * [`media_painter`] — images, GIF and video frames.

use vello::Scene;
use vello::kurbo::Affine;
use vello::peniko::{BlendMode, Compose, Mix};

use crate::css::properties::ClipPath;
use crate::scene::{Node, NodeKind, Scene as StickerScene};
use crate::text::TextEngine;

use super::ShadowDraw;
use super::filter::{FilterLayer, root_spatial_filter};
use super::media::MediaStore;
use super::{box_painter, effects_painter, media_painter, text_painter};

pub struct Painter<'a> {
    pub text: &'a mut TextEngine,
    pub media: &'a mut MediaStore,
    /// When false the fast video path skips the sampled text shadows; those
    /// are rendered by the GPU shadow layer instead.
    pub inline_shadows: bool,
}

impl<'a> Painter<'a> {
    pub fn new(text: &'a mut TextEngine, media: &'a mut MediaStore) -> Self {
        Self {
            text,
            media,
            inline_shadows: true,
        }
    }

    pub fn paint(&mut self, sticker_scene: &StickerScene, canvas: &mut Scene) {
        self.paint_filtered(sticker_scene, canvas, None);
    }

    /// Paint the scene, optionally skipping one node (used by the fast video
    /// compositor to render only the static overlay).
    pub fn paint_filtered(
        &mut self,
        sticker_scene: &StickerScene,
        canvas: &mut Scene,
        skip_uid: Option<u64>,
    ) {
        let mut indices: Vec<usize> = (0..sticker_scene.roots.len()).collect();
        indices.sort_by_key(|index| sticker_scene.roots[*index].z_index);
        for index in indices {
            self.paint_node(
                &sticker_scene.roots[index],
                Affine::IDENTITY,
                skip_uid,
                canvas,
            );
        }
    }

    /// Paint a single root (and its subtree) into its own scene. Used by the
    /// offscreen filter path to isolate filtered roots.
    pub fn paint_root(&mut self, node: &Node, canvas: &mut Scene) {
        self.paint_node(node, Affine::IDENTITY, None, canvas);
    }

    /// Paint the glyph coverage of every text node that has a text-shadow.
    /// The result is white and unshifted; the GPU shadow pass applies blur,
    /// offset and colour per shadow.
    pub fn paint_text_masks(
        &mut self,
        sticker_scene: &StickerScene,
        canvas: &mut Scene,
        scale: f64,
    ) {
        let mut indices: Vec<usize> = (0..sticker_scene.roots.len()).collect();
        indices.sort_by_key(|index| sticker_scene.roots[*index].z_index);
        for index in indices {
            self.paint_mask_node(&sticker_scene.roots[index], Affine::scale(scale), canvas);
        }
    }

    fn paint_mask_node(&mut self, node: &Node, parent_visual: Affine, canvas: &mut Scene) {
        if !node.visible || node.style.display == crate::css::Display::None {
            return;
        }
        let width = node.layout.width.max(0.0);
        let height = node.layout.height.max(0.0);
        if width <= 0.0 || height <= 0.0 {
            return;
        }

        let context = node
            .style
            .resolve_context(crate::css::Viewport::default(), width, height);
        let local = effects_painter::node_transform(node, &context);
        let translate = Affine::translate((node.absolute_x as f64, node.absolute_y as f64));
        let base = parent_visual * translate * local;
        let children_visual = parent_visual
            * translate
            * local
            * Affine::translate((-node.absolute_x as f64, -node.absolute_y as f64));

        match &node.kind {
            NodeKind::Text(_) => {
                if !node.text_shadows().is_empty() {
                    text_painter::paint_text_mask(node, &base, canvas, self.text);
                }
            }
            NodeKind::Container => {
                let mut indices: Vec<usize> = (0..node.children.len()).collect();
                indices.sort_by_key(|index| node.children[*index].z_index);
                for index in indices {
                    self.paint_mask_node(&node.children[index], children_visual, canvas);
                }
            }
            NodeKind::Media(_) => {}
        }
    }

    /// Paint a scene into filter layers, splitting at filtered descendants.
    /// Unfiltered runs share a layer; each filtered subtree gets its own.
    pub fn paint_layers(&mut self, sticker_scene: &StickerScene, layers: &mut Vec<FilterLayer>) {
        let mut indices: Vec<usize> = (0..sticker_scene.roots.len()).collect();
        indices.sort_by_key(|index| sticker_scene.roots[*index].z_index);
        for index in indices {
            let root = &sticker_scene.roots[index];
            if let Some(filter) = root_spatial_filter(root) {
                let mut scene = Scene::new();
                self.paint_node(root, Affine::IDENTITY, None, &mut scene);
                layers.push(FilterLayer {
                    scene,
                    filter: Some(filter),
                });
                continue;
            }
            if subtree_has_filter(root) && !has_active_effects(root) {
                let mut current = Scene::new();
                self.paint_node_layers(root, Affine::IDENTITY, &mut current, layers);
                layers.push(FilterLayer {
                    scene: current,
                    filter: None,
                });
            } else {
                let mut scene = Scene::new();
                self.paint_node(root, Affine::IDENTITY, None, &mut scene);
                layers.push(FilterLayer {
                    scene,
                    filter: None,
                });
            }
        }
    }

    fn paint_node_layers(
        &mut self,
        node: &Node,
        parent_visual: Affine,
        current: &mut Scene,
        layers: &mut Vec<FilterLayer>,
    ) {
        if node.mask_image().is_some() {
            self.paint_node(node, parent_visual, None, current);
            return;
        }
        if !node.visible || node.style.display == crate::css::Display::None {
            return;
        }
        let width = node.layout.width.max(0.0);
        let height = node.layout.height.max(0.0);
        if width <= 0.0 || height <= 0.0 {
            return;
        }

        let context = node
            .style
            .resolve_context(crate::css::Viewport::default(), width, height);
        let local = effects_painter::node_transform(node, &context);
        let translate = Affine::translate((node.absolute_x as f64, node.absolute_y as f64));
        let base = parent_visual * translate * local;
        let children_visual = parent_visual
            * translate
            * local
            * Affine::translate((-node.absolute_x as f64, -node.absolute_y as f64));
        let box_rect = vello::kurbo::Rect::new(0.0, 0.0, width as f64, height as f64);
        let layer = effects_painter::begin_effects(node, &base, &box_rect, current);

        box_painter::paint_shadows(node, &base, current);
        box_painter::paint_background(node, &base, current, self.media);
        box_painter::paint_border(node, &base, current);

        match &node.kind {
            NodeKind::Text(_) => {
                text_painter::paint_text(node, &base, current, self.text, self.inline_shadows);
            }
            NodeKind::Media(media) => {
                media_painter::paint_media(node, media, &base, current, self.media);
            }
            NodeKind::Container => {
                let mut indices: Vec<usize> = (0..node.children.len()).collect();
                indices.sort_by_key(|index| node.children[*index].z_index);
                for index in indices {
                    let child = &node.children[index];
                    let child_filter = root_spatial_filter(child);
                    if let Some(filter) = child_filter {
                        if layer.is_active() {
                            // Splitting would close the parent's opacity/clip
                            // layer in the wrong scene; paint it inline.
                            self.paint_node(child, children_visual, None, current);
                        } else {
                            layers.push(FilterLayer {
                                scene: std::mem::take(current),
                                filter: None,
                            });
                            let mut scene = Scene::new();
                            self.paint_node(child, children_visual, None, &mut scene);
                            layers.push(FilterLayer {
                                scene,
                                filter: Some(filter),
                            });
                        }
                    } else if subtree_has_filter(child)
                        && !layer.is_active()
                        && child.mask_image().is_none()
                    {
                        self.paint_node_layers(child, children_visual, current, layers);
                    } else {
                        self.paint_node(child, children_visual, None, current);
                    }
                }
            }
        }

        effects_painter::end_effects(layer, current);
    }

    fn paint_node(
        &mut self,
        node: &Node,
        parent_visual: Affine,
        skip_uid: Option<u64>,
        canvas: &mut Scene,
    ) {
        if skip_uid == Some(node.uid) {
            return;
        }
        if !node.visible || node.style.display == crate::css::Display::None {
            return;
        }
        let width = node.layout.width.max(0.0);
        let height = node.layout.height.max(0.0);
        if width <= 0.0 || height <= 0.0 {
            return;
        }

        let context = node
            .style
            .resolve_context(crate::css::Viewport::default(), width, height);
        let local = effects_painter::node_transform(node, &context);
        let translate = Affine::translate((node.absolute_x as f64, node.absolute_y as f64));
        let base = parent_visual * translate * local;
        // Children receive the parent's visual transform around its own box,
        // so a transformed container moves its whole subtree.
        let children_visual = parent_visual
            * translate
            * local
            * Affine::translate((-node.absolute_x as f64, -node.absolute_y as f64));
        let box_rect = vello::kurbo::Rect::new(0.0, 0.0, width as f64, height as f64);

        let layer = effects_painter::begin_effects(node, &base, &box_rect, canvas);
        let has_mask = node.mask_image().is_some();
        if has_mask {
            canvas.push_layer(
                vello::peniko::Fill::NonZero,
                BlendMode::new(Mix::Normal, Compose::SrcOver),
                1.0,
                base,
                &box_rect,
            );
        }

        box_painter::paint_shadows(node, &base, canvas);
        box_painter::paint_background(node, &base, canvas, self.media);
        box_painter::paint_border(node, &base, canvas);

        match &node.kind {
            NodeKind::Text(_) => {
                text_painter::paint_text(node, &base, canvas, self.text, self.inline_shadows);
            }
            NodeKind::Media(media) => {
                media_painter::paint_media(node, media, &base, canvas, self.media);
            }
            NodeKind::Container => {
                let mut indices: Vec<usize> = (0..node.children.len()).collect();
                indices.sort_by_key(|index| node.children[*index].z_index);
                for index in indices {
                    self.paint_node(&node.children[index], children_visual, skip_uid, canvas);
                }
            }
        }

        if has_mask {
            canvas.push_luminance_mask_layer(vello::peniko::Fill::NonZero, 1.0, base, &box_rect);
            box_painter::paint_mask_image(node, &base, canvas);
            canvas.pop_layer();
            canvas.pop_layer();
        }

        effects_painter::end_effects(layer, canvas);
    }

    /// Paint the boxes of nodes with `backdrop-filter` into a white coverage
    /// mask. The compositor blends the blurred video through this alpha.
    pub fn paint_backdrop_masks(
        &mut self,
        sticker_scene: &StickerScene,
        canvas: &mut Scene,
        scale: f64,
    ) {
        let mut indices: Vec<usize> = (0..sticker_scene.roots.len()).collect();
        indices.sort_by_key(|index| sticker_scene.roots[*index].z_index);
        for index in indices {
            self.paint_backdrop_node(&sticker_scene.roots[index], Affine::scale(scale), canvas);
        }
    }

    fn paint_backdrop_node(&mut self, node: &Node, parent_visual: Affine, canvas: &mut Scene) {
        if !node.visible || node.style.display == crate::css::Display::None {
            return;
        }
        let width = node.layout.width.max(0.0);
        let height = node.layout.height.max(0.0);
        if width <= 0.0 || height <= 0.0 {
            return;
        }
        let context = node
            .style
            .resolve_context(crate::css::Viewport::default(), width, height);
        let local = effects_painter::node_transform(node, &context);
        let translate = Affine::translate((node.absolute_x as f64, node.absolute_y as f64));
        let base = parent_visual * translate * local;
        let children_visual = parent_visual
            * translate
            * local
            * Affine::translate((-node.absolute_x as f64, -node.absolute_y as f64));

        if node.backdrop_blur().is_some() {
            let radius = box_painter::radius(&node.style.border.radius, &context, width, height);
            let rect = vello::kurbo::Rect::new(0.0, 0.0, width as f64, height as f64);
            canvas.fill(
                vello::peniko::Fill::NonZero,
                base,
                vello::peniko::Color::new([1.0, 1.0, 1.0, 1.0]),
                None,
                &vello::kurbo::RoundedRect::from_rect(rect, radius),
            );
        }

        let mut indices: Vec<usize> = (0..node.children.len()).collect();
        indices.sort_by_key(|index| node.children[*index].z_index);
        for index in indices {
            self.paint_backdrop_node(&node.children[index], children_visual, canvas);
        }
    }
}

/// Collect text shadows in paint order. Within one text node the last CSS
/// shadow comes first, because CSS paints the first shadow on top.
pub fn collect_shadow_draws(sticker_scene: &StickerScene) -> Vec<ShadowDraw> {
    let mut draws = Vec::new();
    let mut indices: Vec<usize> = (0..sticker_scene.roots.len()).collect();
    indices.sort_by_key(|index| sticker_scene.roots[*index].z_index);
    for index in indices {
        collect_node_shadows(&sticker_scene.roots[index], &mut draws);
    }
    draws
}

fn collect_node_shadows(node: &Node, draws: &mut Vec<ShadowDraw>) {
    if !node.visible || node.style.display == crate::css::Display::None {
        return;
    }
    match &node.kind {
        NodeKind::Text(_) => {
            for shadow in node.text_shadows().iter().rev() {
                let color = node.filter_color(shadow.color).components;
                if color[3] <= 0.0 {
                    continue;
                }
                let (dx, dy, blur) = node.shadow_metrics(shadow);
                draws.push(ShadowDraw {
                    sigma: blur * 0.5,
                    offset: [dx, dy],
                    color,
                });
            }
        }
        NodeKind::Container => {
            let mut indices: Vec<usize> = (0..node.children.len()).collect();
            indices.sort_by_key(|index| node.children[*index].z_index);
            for index in indices {
                collect_node_shadows(&node.children[index], draws);
            }
        }
        NodeKind::Media(_) => {}
    }
}

/// True when the node has a filtered descendant (its own filter is handled by
/// the caller).
pub fn subtree_has_filter(node: &Node) -> bool {
    node.children.iter().any(|child| {
        crate::render::has_spatial_filter(&child.style.effects.filters) || subtree_has_filter(child)
    })
}

fn has_active_effects(node: &Node) -> bool {
    node.opacity() < 0.999
        || node.style.effects.mix_blend_mode != Mix::Normal
        || node.style.effects.isolation
        || !matches!(node.style.effects.clip_path, ClipPath::None)
}
