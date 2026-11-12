# Architecture & Memory Notes

This document records the design decisions of the Rust rewrite and the lifecycle rules that keep memory bounded.

## Pipeline

```
config.py ──► python3 -m widdgyy.build ─┐
                                        ├─► ConfigLoader ──► Scene graph
config.jsonc ───────────────────────────┘                        │
styles.css ───► Stylesheet ──► Cascade ──► ComputedStyle (typed modules)
                                                                │
                                                     LayoutEngine ──► LayoutBox per node
                                                                │
                                              Painter (Vello scenes + layer tree)
                                                                │
                                              GpuRenderer (Vello + wgpu passes)
                                                                │
                                                     wlr-layer-shell surface
```

- `config.py` is evaluated by the system `python3` in a subprocess with a 3 s timeout; its JSON output goes through the same loader as `config.jsonc`. A broken Python config keeps the previous scene.
- Media queries are evaluated against `css::Preferences`. They are detected once on load/reload from environment overrides, GNOME `gsettings`, KDE `kreadconfig`, or `GTK_THEME`; the wallpaper defaults to light, no reduced motion, no input and fullscreen display mode.
- Styles are parsed once. The cascade runs on load, hot reload and resize only.
- Layout runs on load, hot reload, resize and when text content changes (clock/API).
- `LayoutEngine` implements block, flex and grid formatting directly. Block containers stack children and collapse vertical margins; flex uses `flex-basis` plus grow/shrink and automatic min-content clamps; grid resolves named areas, explicit/implicit tracks, dense/sparse placement and intrinsic track contributions. Taffy was evaluated and rejected for this release: it would require replacing the scene node model and the existing absolute/relative positioning path for no B1 feature that the custom engine cannot now provide.
- Painting runs only when the scene is dirty: a clock second, a GIF/video frame, an API result, a resize or a reload.
- The fast video path draws a background video as a single textured quad and composites a cached Vello overlay. Vello only re-rasterizes the overlay when clocks/API/text/resize change, not per video frame.
- Scenes with spatial filters use a recursive offscreen layer tree: each group rasterizes into an accumulation texture, child groups composite into it, and the group's filter/clip/opacity is applied when it composites into its parent.
- The Wayland event loop waits 200 ms when idle and 16 ms while animated media/API work is active.

## Style modules

Each family owns its parser and painter:

| Family | Parser | Painter |
| --- | --- | --- |
| Box model | `css/properties/box_model.rs` | `layout/` |
| Background | `css/properties/background.rs` | `render/box_painter.rs` |
| Border/outline/border-image | `css/properties/border.rs` | `render/box_painter.rs` |
| Text | `css/properties/text.rs` | `render/text_painter.rs` |
| Effects/filters/masks | `css/properties/effects.rs` | `render/effects_painter.rs`, `render/filter.rs`, `render/box_painter.rs` |
| Flex | `css/properties/flex.rs` | `layout/` |
| Grid | `css/properties/grid.rs` | `layout/` |
| Animation/transitions | `css/properties/transition.rs` | `animation.rs` (sampled overlays), painters |

Painters read typed `ComputedStyle` fields. They never compare property-name strings, which keeps nesting shallow and avoids per-frame parsing.

## Memory lifecycle

### Scene and styles

- `Runtime::reload()` builds a brand-new `Scene` and replaces the old one. The old tree is dropped, including every `Node`, `ComputedStyle` and cached shaped-text handle.
- `ApiManager::reset()` drains stale results and clears in-flight tracking so workers from the previous scene cannot mutate new nodes.
- `MediaStore::retain_paths()` drops images, GIFs and video pipelines that the new scene no longer references.

### Text

- `TextEngine` caches `Arc<ShapedText>` keyed by text + style + wrap width.
- The cache is capped at 512 entries and cleared wholesale when full. This avoids an LRU dependency and keeps peak memory predictable.
- Glyph runs hold `FontData` (reference counted blobs), not copies of font files.
- System font loading happens once at startup. `@font-face` files are loaded once per path and invalidate the shaping cache.

### Images and GIFs

- Images are decoded once into `peniko::ImageData` and shared through `Arc`.
- `mask-mode: alpha` on an opaque URL image uses a cached white-RGB copy with the original alpha, so Vello's luminance mask resolves to the source alpha. This cache is bounded like the image cache.
- The image cache is capped at 64 entries and pruned on reload.
- GIFs are decoded once with hard limits: 240 frames and 64 M pixels. Playback only advances an index; it does not decode per frame.

### Video

- `VideoAsset` owns a GStreamer `playbin` and an `appsink` negotiated to NV12 system memory.
- `pump()` copies at most 4 newest samples per tick; `max-buffers=1` and `drop=true` prevent queue growth.
- The full NV12 buffer is copied once per frame (`Arc<[u8]>`). Y and UV planes are uploaded separately, then converted to RGB by the compositor shader.
- The fast path keeps only the newest frame, the Y/UV GPU textures and a cached Vello overlay texture.
- If the video has a border/radius/transform/opacity/filter, the renderer falls back to the full Vello path and converts NV12 to RGBA for that frame.
- `Drop for VideoAsset` sets the pipeline to `State::Null`, releasing decoder threads, buffers and GStreamer resources.
- `MediaStore` drops the `VideoAsset` when the path is no longer referenced after reload.

### GPU

- `GpuRenderer` owns the surface, Vello renderer, intermediate texture, blitter, filter compositor and layer textures.
- Filter layers reuse a scratch Vello target and one set of blur textures. Each nesting depth lazily allocates one full-screen accumulation texture (and a shared clip-mask scratch), so simple scenes do not pay for the eight-level cap.
- Shadow and backdrop filters use separate `FilterCompositor` instances. Shadow masks run at half resolution with four blur iterations; the backdrop runs at 35% resolution with two. Shadow sigma is quantized to 0.5 px and each slot's blur is cached until the sigma or the glyph mask changes.
- Filter passes accept an optional scissor. Backdrop blur/composite is limited to the union of panel boxes plus blur margin, and the video downsample pass is scissored to the same region; shadow blur/composite is limited to the union of shadowed text plus a conservative animation margin. Shadow blur passes use mask-resolution coordinates while the final shadow composite uses full-resolution slot coordinates.
- Resizing recreates the intermediate, scratch, blur, shadow and backdrop textures and drops the accumulation pool; the surface and device stay alive.
- The renderer is dropped before the layer surface (`Drop for WaylandState`), matching the required Wayland/wgpu destruction order.
- Vello's image atlas is internal and bounded by the images referenced in the current scene.

### Hot reload

- `notify` watches only the config directory, non-recursively.
- Only `Modify`/`Create`/`Remove` events are considered. `Access` events are ignored; otherwise the app would reload itself by reading its own config.
- Reloads are debounced (120 ms). File modification times are compared to avoid duplicate events.
- Parse errors keep the previous scene instead of leaving a half-built tree.

## Measured behavior (development machine)

Release build, 1920x1080, real config from `~/.config/widdgyy`:

| Scenario | CPU (one core) | RSS after warm-up |
| --- | --- | --- |
| Text/clock only (`--config` test fixture) | ~0.2% | ~165 MB |
| 1080p looping video + glow + market panel/backdrop | ~20–28% | ~500–515 MB, stable |

The video path uses NVDEC/NV12, avoids the per-frame CPU RGBA conversion, and composites the static Vello overlay only when it changes. `appsink` uses `sync=true`, `max-buffers=1` and `drop=true` so the decoder does not run ahead of the display. Shadow blur slots are cached/quantized and backdrop/shadows use region scissors; total GPU utilization on the development GTX 1650 dropped from roughly 37% to 25–32% including the desktop baseline (~22%). Readings are noisy because `nvidia-smi` reports the whole GPU; use `scripts/perf-sample.sh` for same-machine comparisons.

## Removed leak sources from the Python version

| Old issue | Rust fix |
| --- | --- |
| `GLib.timeout_add` never removed | Single calloop event loop, no global timeout handles |
| `add_tick_callback` never removed | No tick callbacks; event loop timeout |
| `scene.clear()` left video pipelines and native widgets alive | `Scene` replacement + `Drop` for `VideoAsset` + media pruning |
| Per-frame `sorted()`/`set()` allocations | Paint only on dirty; ordering computed when needed |
| Pango layout recreated per frame | `TextEngine` shaping cache |
| Colors/lengths re-parsed per frame | Typed `ComputedStyle` |
| CSS styles never cleared on reload | New `Stylesheet` replaces the old one |
| API thread per interval with no coalescing | Bounded in-flight set per sticker |
| Image pixbufs retained forever | Bounded `Arc` image cache, pruned on reload |

## Conformance harness

- `widdgyy --dump-layout --viewport WxH` resolves styles and layout without opening a window and prints JSON boxes, generated pseudo boxes, text-shadow counts and text line rectangles.
- `scripts/css-conformance.sh` runs each `tests/conformance/<name>` fixture against Firefox headless through Marionette (no geckodriver), comparing boxes and line rects with per-fixture tolerances.
- The harness is optional: it skips cleanly when Firefox or Marionette is unavailable, so the normal test suite has no browser dependency.

## Known limitations

- Nested filters are supported through the offscreen layer graph, but filters below `mask-image`/`mix-blend-mode` are painted inline. `mask-composite` only supports additive composition.
- Group nesting is capped at 8 levels; deeper groups are composited flat with a warning.
- `backdrop-filter` requires the video fast path; a transparent layer surface cannot sample the desktop without compositor support.
- Transitions animate opacity, transform, colors, text-shadow and custom properties on style changes; layout-affecting transitions and animations are pending.
- `@import` is rejected with a log message; `@layer` blocks are flattened without ordering.
- External SVG clip paths and `clip-path: url(...)` are not fetched.
- The CSS support matrix (including parsed-only properties) is maintained in the Configuration & Styling Guide.
- Media preference changes are picked up on reload; there is no live desktop-theme subscription. Output scale factor is still fixed at 1.0, so `resolution` reports 1dppx.
- GPU text shadows are composited behind the whole overlay texture. An opaque element background painted in the overlay can hide them; without video, a full-screen opaque page background does so.
- B8 leftovers: overlay static/dynamic caching, dynamic shadow-slot count, and per-pass GPU frame timing.
