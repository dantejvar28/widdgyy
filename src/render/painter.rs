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

use std::sync::atomic::{AtomicBool, Ordering};

use vello::Scene;
use vello::kurbo::Affine;
use vello::peniko::{BlendMode, Compose, Mix};

use crate::scene::{Node, NodeKind, Scene as StickerScene};
use crate::text::TextEngine;

use super::ShadowDraw;
use super::filter::{FilterLayer, LayerItem, root_spatial_filter};
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

    /// Paint a scene into compositing layers, splitting at filtered
    /// descendants. Nodes that need an offscreen buffer become groups whose
    /// items are painted recursively.
    pub fn paint_layers(&mut self, sticker_scene: &StickerScene, layers: &mut Vec<FilterLayer>) {
        let mut indices: Vec<usize> = (0..sticker_scene.roots.len()).collect();
        indices.sort_by_key(|index| sticker_scene.roots[*index].z_index);
        for index in indices {
            let root = &sticker_scene.roots[index];
            let mut builder = LayerBuilder::default();
            self.paint_node_into(root, Affine::IDENTITY, &mut builder);
            layers.extend(builder.finish());
        }
    }

    /// Paint one node in the context of a layer group, splitting children
    /// that need their own offscreen buffer.
    fn paint_node_into(&mut self, node: &Node, parent_visual: Affine, builder: &mut LayerBuilder) {
        if !node.visible || node.style.display == crate::css::Display::None {
            return;
        }
        let width = node.layout.width.max(0.0);
        let height = node.layout.height.max(0.0);
        if width <= 0.0 || height <= 0.0 {
            return;
        }

        if needs_group(node) {
            let layer = self.build_group(node, parent_visual);
            builder.push_layer(layer);
            return;
        }

        if blocks_splitting(node) && subtree_has_filter(node) {
            warn_filter_skipped();
            self.paint_node(node, parent_visual, None, &mut builder.current);
            return;
        }

        let (base, children_visual) = visual_transforms(node, parent_visual);
        let box_rect = vello::kurbo::Rect::new(0.0, 0.0, width as f64, height as f64);
        let layer = effects_painter::begin_effects(node, &base, &box_rect, &mut builder.current);
        let has_mask = !node.masks().is_empty();
        if has_mask {
            builder.current.push_layer(
                vello::peniko::Fill::NonZero,
                BlendMode::new(Mix::Normal, Compose::SrcOver),
                1.0,
                base,
                &box_rect,
            );
        }

        self.paint_decoration(node, &base, &mut builder.current);

        if matches!(node.kind, NodeKind::Container) {
            let mut indices: Vec<usize> = (0..node.children.len()).collect();
            indices.sort_by_key(|index| node.children[*index].z_index);
            for index in indices {
                let child = &node.children[index];
                if layer.is_active() {
                    // An active Vello layer cannot span several scenes.
                    self.paint_node(child, children_visual, None, &mut builder.current);
                } else {
                    self.paint_node_into(child, children_visual, builder);
                }
            }
        }

        if has_mask {
            builder.current.push_luminance_mask_layer(
                vello::peniko::Fill::NonZero,
                1.0,
                base,
                &box_rect,
            );
            box_painter::paint_mask_image(node, &base, &mut builder.current, self.media);
            builder.current.pop_layer();
            builder.current.pop_layer();
        }

        effects_painter::end_effects(layer, &mut builder.current);
    }

    /// Build an offscreen group for a node with its own filter or with
    /// opacity/clip/isolation over a filtered subtree.
    fn build_group(&mut self, node: &Node, parent_visual: Affine) -> FilterLayer {
        let width = node.layout.width.max(0.0);
        let height = node.layout.height.max(0.0);
        let (base, children_visual) = visual_transforms(node, parent_visual);
        let box_rect = vello::kurbo::Rect::new(0.0, 0.0, width as f64, height as f64);
        let mut layer = FilterLayer {
            items: Vec::new(),
            filter: root_spatial_filter(node),
            opacity: node.opacity().clamp(0.0, 1.0),
            mask: effects_painter::clip_mask_scene(node, &base, &box_rect),
        };

        // A mask layer spans the node's whole subtree, so filters below it
        // cannot be split without applying the mask at composite time. Paint
        // the subtree as one scene and keep the node's own filter working.
        if !node.masks().is_empty() {
            if subtree_has_filter(node) {
                warn_filter_skipped();
            }
            let mut scene = Scene::new();
            scene.push_layer(
                vello::peniko::Fill::NonZero,
                BlendMode::new(Mix::Normal, Compose::SrcOver),
                1.0,
                base,
                &box_rect,
            );
            self.paint_decoration(node, &base, &mut scene);
            if matches!(node.kind, NodeKind::Container) {
                let mut indices: Vec<usize> = (0..node.children.len()).collect();
                indices.sort_by_key(|index| node.children[*index].z_index);
                for index in indices {
                    self.paint_node(&node.children[index], children_visual, None, &mut scene);
                }
            }
            scene.push_luminance_mask_layer(vello::peniko::Fill::NonZero, 1.0, base, &box_rect);
            box_painter::paint_mask_image(node, &base, &mut scene, self.media);
            scene.pop_layer();
            scene.pop_layer();
            layer.items.push(LayerItem::Scene(scene));
            return layer;
        }

        let mut builder = LayerBuilder::default();
        self.paint_decoration(node, &base, &mut builder.current);
        if matches!(node.kind, NodeKind::Container) {
            let mut indices: Vec<usize> = (0..node.children.len()).collect();
            indices.sort_by_key(|index| node.children[*index].z_index);
            for index in indices {
                self.paint_node_into(&node.children[index], children_visual, &mut builder);
            }
        }
        builder.flush();
        layer.items = builder.items;
        layer
    }

    fn paint_decoration(&mut self, node: &Node, base: &Affine, canvas: &mut Scene) {
        box_painter::paint_shadows(node, base, canvas);
        box_painter::paint_background(node, base, canvas, self.media);
        box_painter::paint_border(node, base, canvas, self.media);
        match &node.kind {
            NodeKind::Text(_) => {
                text_painter::paint_text(node, base, canvas, self.text, self.inline_shadows);
            }
            NodeKind::Media(media) => {
                media_painter::paint_media(node, media, base, canvas, self.media);
            }
            NodeKind::Container => {}
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
        let has_mask = !node.masks().is_empty();
        if has_mask {
            canvas.push_layer(
                vello::peniko::Fill::NonZero,
                BlendMode::new(Mix::Normal, Compose::SrcOver),
                1.0,
                base,
                &box_rect,
            );
        }

        self.paint_decoration(node, &base, canvas);

        if matches!(node.kind, NodeKind::Container) {
            let mut indices: Vec<usize> = (0..node.children.len()).collect();
            indices.sort_by_key(|index| node.children[*index].z_index);
            for index in indices {
                self.paint_node(&node.children[index], children_visual, skip_uid, canvas);
            }
        }

        if has_mask {
            canvas.push_luminance_mask_layer(vello::peniko::Fill::NonZero, 1.0, base, &box_rect);
            box_painter::paint_mask_image(node, &base, canvas, self.media);
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

/// Bounding box of all text that has a shadow, expanded by a conservative
/// margin so animated offsets/radii stay inside the scissor. Returns
/// `[x, y, width, height]` in screen pixels.
pub fn shadow_region(scene: &StickerScene, viewport: crate::css::Viewport) -> Option<[f32; 4]> {
    const MARGIN: f32 = 128.0;
    fn visit(node: &Node, bounds: &mut Option<[f32; 4]>) {
        if node.visible
            && node.style.display != crate::css::Display::None
            && matches!(node.kind, NodeKind::Text(_))
            && !node.text_shadows().is_empty()
        {
            let x0 = node.absolute_x - MARGIN;
            let y0 = node.absolute_y - MARGIN;
            let x1 = node.absolute_x + node.layout.width + MARGIN;
            let y1 = node.absolute_y + node.layout.height + MARGIN;
            *bounds = Some(match *bounds {
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
            visit(child, bounds);
        }
    }
    let mut bounds = None;
    for root in &scene.roots {
        visit(root, &mut bounds);
    }
    bounds.and_then(|[x, y, width, height]| {
        let x0 = x.max(0.0);
        let y0 = y.max(0.0);
        let x1 = (x + width).min(viewport.width).max(x0);
        let y1 = (y + height).min(viewport.height).max(y0);
        (x1 > x0 && y1 > y0).then_some([x0, y0, x1 - x0, y1 - y0])
    })
}

/// True when the node has a filtered descendant (its own filter is handled by
/// the caller).
pub fn subtree_has_filter(node: &Node) -> bool {
    node.children.iter().any(|child| {
        crate::render::has_spatial_filter(&child.style.effects.filters) || subtree_has_filter(child)
    })
}

/// Paint-order container: runs of ordinary Vello commands interleaved with
/// nested groups.
#[derive(Default)]
struct LayerBuilder {
    items: Vec<LayerItem>,
    current: Scene,
}

impl LayerBuilder {
    fn flush(&mut self) {
        if !self.current.encoding().is_empty() {
            self.items
                .push(LayerItem::Scene(std::mem::take(&mut self.current)));
        }
    }

    fn push_layer(&mut self, layer: FilterLayer) {
        self.flush();
        self.items.push(LayerItem::Layer(layer));
    }

    fn finish(mut self) -> Vec<FilterLayer> {
        self.flush();
        self.items
            .into_iter()
            .map(|item| match item {
                LayerItem::Scene(scene) => FilterLayer::plain(scene),
                LayerItem::Layer(layer) => layer,
            })
            .collect()
    }
}

/// Node transform relative to the parent, plus the transform inherited by its
/// children (the node's transform applied around its own box).
fn visual_transforms(node: &Node, parent_visual: Affine) -> (Affine, Affine) {
    let width = node.layout.width.max(0.0);
    let height = node.layout.height.max(0.0);
    let context = node
        .style
        .resolve_context(crate::css::Viewport::default(), width, height);
    let local = effects_painter::node_transform(node, &context);
    let translate = Affine::translate((node.absolute_x as f64, node.absolute_y as f64));
    let base = parent_visual * translate * local;
    let children_visual =
        base * Affine::translate((-node.absolute_x as f64, -node.absolute_y as f64));
    (base, children_visual)
}

/// A node needs its own offscreen buffer when it is filtered, or when it
/// groups opacity/clip/isolation over a filtered descendant.
fn needs_group(node: &Node) -> bool {
    if crate::render::has_spatial_filter(&node.style.effects.filters) {
        return true;
    }
    if !subtree_has_filter(node) {
        return false;
    }
    node.opacity() < 0.999 || node.style.effects.isolation || effects_painter::has_clip(node)
}

/// These effects wrap the whole subtree in one Vello layer, so a filter cannot
/// be split out of them. They stay as documented limitations of the layer
/// graph.
fn blocks_splitting(node: &Node) -> bool {
    !node.masks().is_empty() || node.style.effects.mix_blend_mode != Mix::Normal
}

static SKIPPED_FILTER_WARNED: AtomicBool = AtomicBool::new(false);

fn warn_filter_skipped() {
    if !SKIPPED_FILTER_WARNED.swap(true, Ordering::Relaxed) {
        log::warn!(
            "filter inside mask-image or mix-blend-mode is painted inline; \
             filtering that subtree is not supported yet"
        );
    }
}
