# Architecture & Memory Notes

This document records the design decisions of the Rust rewrite and the lifecycle rules that keep memory bounded.

## Pipeline

```
config.jsonc ──► ConfigLoader ──► Scene graph
                                     │
styles.css ───► Stylesheet ──► Cascade ──► ComputedStyle (typed modules)
                                     │
                                  LayoutEngine ──► LayoutBox per node
                                     │
                            Painter (Vello Scene)
                                     │
                         GpuRenderer (wgpu + Vello)
                                     │
                            wlr-layer-shell surface
```

- Styles are parsed once. The cascade runs on load, hot reload and resize only.
- Layout runs on load, hot reload, resize and when text content changes (clock/API).
- Painting runs only when the scene is dirty: a clock second, a GIF/video frame, an API result, a resize or a reload.
- The fast video path draws a background video as a single textured quad and composites a cached Vello overlay. Vello only re-rasterizes the overlay when clocks/API/text/resize change, not per video frame.
- The Wayland event loop waits 200 ms when idle and 16 ms while animated media/API work is active.

## Style modules

Each family owns its parser and painter:

| Family | Parser | Painter |
| --- | --- | --- |
| Box model | `css/properties/box_model.rs` | `layout/` |
| Background | `css/properties/background.rs` | `render/box_painter.rs` |
| Border/outline | `css/properties/border.rs` | `render/box_painter.rs` |
| Text | `css/properties/text.rs` | `render/text_painter.rs` |
| Effects | `css/properties/effects.rs` | `render/effects_painter.rs` |
| Flex | `css/properties/flex.rs` | `layout/` |
| Grid | `css/properties/grid.rs` | `layout/` |
| Animation | `css/properties/transition.rs` | runtime (parsed values ready) |

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

- `GpuRenderer` owns the surface, Vello renderer, intermediate texture and blitter.
- Resizing recreates only the intermediate texture; the surface and device stay alive.
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
| 1080p looping video + clock + API stickers | ~16% | ~475 MB, stable over 60 s |

The video path uses NVDEC/NV12, avoids the per-frame CPU RGBA conversion, and composites the static Vello overlay only when it changes. `appsink` uses `sync=true`, `max-buffers=1` and `drop=true` so the decoder does not run ahead of the display. RSS was sampled every 10 s for 60 s with no upward trend.

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

## Known limitations

- Animation interpolation is not wired to the painters yet (values and keyframes are parsed).
- General CSS filters are parsed but not painted; Vello does not expose a generic layer filter in the current integration.
- `@import` is rejected with a log message.
- External SVG clip paths are not fetched.
