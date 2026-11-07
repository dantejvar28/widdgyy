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

use crate::scene::{Node, NodeKind, Scene as StickerScene};
use crate::text::TextEngine;

use super::ShadowDraw;
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

        effects_painter::end_effects(layer, canvas);
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
