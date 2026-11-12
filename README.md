# Widdgyy

Widdgyy is an open-source desktop ricing engine for Wayland that turns your wallpaper into a living workspace.

Create clocks, images, videos, text, weather panels, system monitors and API-powered stickers that are rendered directly on the desktop background layer. The scene is described with a small Python DSL (`config.py`) or the legacy `config.jsonc`, and styled with CSS, so you can build highly customized layouts without writing a desktop extension.

## Architecture

Widdgyy is a native Rust application. There is no embedded Python runtime and no GTK dependency; `config.py` is evaluated by the system `python3` and converted to the same scene document the legacy JSONC loader uses.

| Layer | Technology | Why |
| --- | --- | --- |
| Window / desktop integration | `smithay-client-toolkit` + `wlr-layer-shell` | Native background layer, anchors, exclusive zones, no decoration |
| GPU rendering | `vello` + `wgpu` | Vector rendering on Vulkan/GL, GPU-composited transparency |
| Text | `cosmic-text` + Vello glyph runs | Shaping, fallback fonts, wrapping and cacheable glyph runs without Pango |
| Images / GIF | `image` | Decoded once, bounded cache, shared through `Arc` |
| Video | GStreamer `playbin` + `appsink` (NV12) | Y/UV planes are uploaded to wgpu and converted by a shader; no GTK widget, overlay or CPU RGBA conversion |
| Styling | Built-in CSS engine | Typed style modules, cascade, variables, shorthands, media queries, offscreen filter/mask layers |
| Scene config | Python DSL + legacy JSONC | `config.py` runs on the system `python3` with a 3 s timeout; both formats produce the same schema |

The scene is described by `config.py` (preferred) or the legacy `config.jsonc`; the CSS class model is preserved in both, so `styles.css` keeps working. Migrate with `widdgyy --migrate-config config.jsonc > config.py`.

> **Compatibility note:** the previous Python widget extension point was never a public plugin API (types were hard-coded in the loader), so this rewrite does not remove a supported extension mechanism. If a plugin system is added later, it should be designed explicitly (WASM, Lua or a stable Rust trait).

## Status: alpha

**Current development line: `0.2.0-alpha.1`.**

Widdgyy is usable as a daily wallpaper engine, but it is still **alpha**:

- The `config.py`/`config.jsonc` schema, the supported CSS surface, CLI flags and rendering internals may change between minor releases.
- Keep a backup of `~/.config/widdgyy` and use `widdgyy --migrate-config` when moving a legacy JSONC config.
- It is tested primarily on Hyprland with an NVIDIA GTX 1650; other compositors, GPU vendors and fractional-scale setups may need fixes.
- Known CSS limitations are tracked in the [Configuration & Styling Guide](docs/Configuration%20%26%20Styling%20Guide.md#not-implemented); they are not silent failures.
- There is no stable API/ABI guarantee yet.

Beta criteria: freeze the config schema and CSS scope for a release, add a CI matrix (distros, compositors, GPU/fallback), run a 24 h video/API soak test, and publish tagged packages with release notes. Until then, pin a commit if you need a reproducible setup and report issues with your distro, compositor, GPU driver and `RUST_LOG=widdgyy=debug`.

See [CHANGELOG.md](CHANGELOG.md) for release notes and known changes.

## Features

- **Python scene DSL:** `config.py` with `Text`, `Clock`, `ApiText`, `Video`, `Image`, `Gif`, `HBox`, `VBox`, `Grid`; loops and constants are allowed, styling stays in CSS.
- **CSS, not ad-hoc properties:** selectors, specificity, `!important`, inheritance, custom properties (`--accent` / `var()`), `calc()`/`min()`/`max()`/`clamp()`, media queries, shorthands, gradients, shadows, borders, transforms, clipping, flex and grid.
- **Browser-like effects:** recursive offscreen layers for nested `filter: blur()`/`drop-shadow()`, `backdrop-filter` on video, multi-layer masks with `mask-mode`, and `border-image` with slice/width/outset/repeat.
- **Static CSS parsed once:** the frame loop never parses colors, lengths or selectors.
- **Bounded caches:** text shaping and media assets have hard limits; reloading drops the old scene and releases its assets.
- **Animations:** `@keyframes` for transform, opacity, colors, background-color, text-shadow and custom properties, with delay/duration/direction/iteration/fill-mode.
- **Diagnostics:** `widdgyy --check-css` lists unsupported properties and at-rules.
- **Hot reload:** editing `config.py`, `config.jsonc` or `styles.css` rebuilds the scene without restarting.
- **API stickers:** requests run on worker threads and report through a channel; the render loop never blocks on the network.
- **Animated media:** GIF and video frames update through the same clock as widgets.

## Requirements

- A Wayland compositor with `wlr-layer-shell` (Hyprland, Sway, river, Wayfire, etc.).
- Rust 1.89 or newer.
- GStreamer 1.20+ with the codecs you need (`gst-plugins-base`, `gst-plugins-good`, optional `gst-plugins-bad` / `gst-libav`).
- A Vulkan or OpenGL driver.

### Arch Linux

```bash
sudo pacman -S --needed rust cargo wayland vulkan-icd-loader \
  gstreamer gst-plugins-base gst-plugins-good
```

### Debian / Ubuntu

```bash
sudo apt install -y build-essential pkg-config \
  libwayland-dev libxkbcommon-dev libvulkan-dev \
  libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev \
  gstreamer1.0-plugins-base gstreamer1.0-plugins-good
```

## Build and run

```bash
cargo build --release
./target/release/widdgyy
# optional: use another config directory/file
./target/release/widdgyy --config /path/to/config.py
# audit unsupported CSS without opening a window
./target/release/widdgyy --check-css

# build without GStreamer/video support
cargo build --release --no-default-features
```

Or use the launcher, which builds on demand:

```bash
./scripts/widdgyy
```

Install the binary in `~/.local/bin`:

```bash
./scripts/install-local-cli.sh
```

Build and install the Arch package (`/usr/bin/widdgyy`, assets in `/usr/share/widdgyy`):

```bash
./scripts/install-local-cli.sh package --noconfirm
```

The package build runs in `~/.cache/widdgyy-makepkg`, not in the cloned repository. The installed executable is `/usr/bin/widdgyy`; you can then remove the clone/build directory if you want.

## Configuration

Widdgyy reads:

- `~/.config/widdgyy/config.py` (preferred)
- `~/.config/widdgyy/config.jsonc` (legacy fallback)
- `~/.config/widdgyy/styles.css`

A minimal Python config:

```python
from widdgyy import ApiText, Clock, HBox, Video

stickers = [
    Video("assets/wallpaper.mp4", cls="background", z=-1),
    HBox(
        Clock("%H:%M", cls="clock"),
        anchor="center",
    ),
]
```

`config.py` is executed with the system `python3` (3 s timeout) and must define a top-level `stickers` list. Every node takes `cls=` for CSS; containers take children positionally:

```python
HBox(Clock("%H:%M", cls="clock"), Text("Widdgyy", cls="brand"), cls="panel")
```

The legacy `config.jsonc` uses the same scene schema and still works:

```jsonc
{
  "stickers": [
    { "type": "media", "path": "assets/wallpaper.mp4", "class": "background", "z_index": -1 },
    { "type": "clock", "class": "clock", "anchor": "center", "format": "%H:%M" }
  ]
}
```

```css
.panel {
  display: flex;
  flex-direction: column;
  gap: 12px;
  padding: 24px;
  background: linear-gradient(135deg, rgba(20, 12, 35, 0.85), rgba(70, 25, 90, 0.75));
  border: 1px solid rgba(255, 255, 255, 0.18);
  border-radius: 28px;
  box-shadow: 0 18px 48px rgba(0, 0, 0, 0.55);
}

.clock {
  font-family: "Angelic War", Sans;
  font-size: 64px;
  font-weight: bold;
  color: var(--accent);
  text-shadow: 0 4px 18px rgba(0, 0, 0, 0.8);
}
```

See [docs/Configuration & Styling Guide.md](docs/Configuration%20%26%20Styling%20Guide.md) for the complete DSL reference (every node type and field), the exhaustive CSS property list, and the complete list of unsupported features. See [docs/Architecture & Memory.md](docs/Architecture%20%26%20Memory.md) for the lifecycle and memory rules.

## CSS support

Implemented in the renderer:

- Colors: named, `#rgb`, `#rrggbb`, `#rrggbbaa`, `rgb()`, `rgba()`, `hsl()`, `hsla()`, `oklch()`, `oklab()`, `lab()`, `lch()`, `hwb()`, `color()`, `color-mix()`, `transparent`, `currentColor`, legacy `r, g, b, a`.
- Lengths: `px`, `%`, `em`, `rem`, `ex`, `ch`, `vw`, `vh`, `vmin`, `vmax`, `pt`, `pc`, `in`, `cm`, `mm`, `fr`, `auto`, `calc()`, `min()`, `max()`, `clamp()`.
- Box: `display`, width/height/min/max, margins, padding, `box-sizing`, `overflow`, `aspect-ratio`, `position`/insets, `z-index`, `visibility`.
- Background: color, multiple image layers, `url()`, linear/radial/conic gradients, position, size, repeat, clip/origin, blend mode.
- Border/outline: width, style, color, radius, outline, `border-image` with source/slice/width/outset/repeat and the shorthand.
- Text: family, size, weight, style, stretch, live OpenType features, line-height, letter/word spacing, alignment/last-line alignment, indent, direction, decoration, transform, white-space, wrapping, overflow/ellipsis, multi-line `line-clamp`, shadows, `font` shorthand.
- Effects: opacity, box-shadow (including inset), text-shadow, color filters, nested `blur()`/`drop-shadow()`, individual translate/rotate/scale, transform/transform-origin, clip-path shapes and `path()`, isolation, object-fit/object-position, multi-layer masks with `mask-mode`, `backdrop-filter` on video.
- Layout: block formatting with margin collapsing, flex (direction, wrap, justify, align/place-*, gap, order, grow/shrink and `flex-basis` as the flex base) and CSS grid (explicit/implicit tracks, `repeat()`/`minmax()`, named areas, `grid-area`, spans, sparse/dense auto-flow, intrinsic track sizing and item/content alignment).
- Animations: `@keyframes` applied at runtime for transform, opacity, color, background-color, text-shadow and custom properties; timing functions, delay, direction, iteration and fill modes.
- Transitions: hot-reload style changes animate opacity, transform, colors, text-shadow and custom properties with duration/delay/easing; layout transitions are still pending.
- Media queries: width/height/aspect-ratio, orientation, resolution, `prefers-color-scheme`/`prefers-reduced-motion` from the desktop, `hover`/`pointer`, display mode, media types and `not`/`only`.
- Selectors: type, `*`, `.class`, `#id`, attribute selectors with `=`, `~=`, `|=`, `^=`, `$=`, `*=`, descendant/child combinators, `:root`, `:first-child`, `:last-child`, `:only-child`, `:nth-child()`, `:nth-last-child()`, `:not()`, `:is()`, `:where()`, and generated `::before`/`::after` boxes with string `content`.
- Verification: a Firefox headless harness compares layout boxes and text line rectangles against the real browser (`./scripts/css-conformance.sh`).

The exhaustive property-by-property reference and the complete list of unsupported features live in the [Configuration & Styling Guide](docs/Configuration%20%26%20Styling%20Guide.md#supported-css-properties). Highlights of the limits:

- Sibling combinators `+`/`~` never match; interaction pseudo-classes are rejected (display-only).
- `border-image` supports local `url()` sources only; `double`/`groove`/`ridge`/`inset`/`outset` borders paint as solid.
- Layout-affecting transitions and animations (`width`, `height`, `margin`, `padding`, `gap`, ...) are pending.
- Media features such as `scripting`, `prefers-contrast` and `forced-colors` are not implemented; unknown features do not match.
- `font-variation-settings` is parsed but not applied; synthetic small caps are not generated.
- `vertical-align` only aligns the text block inside its box (no inline formatting context); `text-indent` does not change first-line wrapping.
- `backdrop-filter` requires the video fast path; `mask-composite` supports only additive composition.
- GPU text shadows sit behind the overlay texture, so an opaque element/page background can hide them.
- Spatial filters below `mask-image`/`mix-blend-mode` and blend modes across offscreen groups are not supported.
- Pseudo-elements other than `::before`/`::after`; generated content is limited to string `content` (no `attr()`, counters or images).
- `@import` is rejected; `@layer` blocks are flattened without layer ordering.

## Performance and memory

The render loop is event-driven:

- The Wayland event loop waits with a 16 ms timeout only when a GIF/video/API sticker is active; otherwise it wakes every 200 ms for the clock.
- Background video uses a fast compositor: the newest NV12 frame is uploaded to two GPU planes and converted to RGB in a shader, while the static Vello overlay is cached and only re-rasterized when text/API content changes. Video playback no longer re-runs Vello over the whole 1080p scene.
- Text is shaped once per content/style pair and cached (bounded to 512 entries). Painting reuses the shaped glyph runs.
- Text-shadow blur slots are cached and the animated sigma is quantized, so a moving glow only re-blurs when the radius crosses a step instead of every frame.
- The backdrop filter renders the video at 35% resolution with two separable blur iterations, scissored to the panel union; overlay and shadow blurs keep four.
- Shadow and backdrop blur passes are scissored to their content region instead of the full screen, the backdrop video downsample is scissored to the same region, and the shadow blur slots are cached.
- `./scripts/perf-sample.sh 10` samples CPU, RSS and total GPU utilization of a running instance for before/after comparisons (GPU readings include the desktop and are noisy).
- Styles are resolved and layout runs only on load, reload and resize, not per frame.
- Images are decoded once and shared with `Arc`; the image cache is bounded to 64 entries and pruned on reload.
- GIFs are bounded to 240 frames / 64 M pixels. Video keeps a single latest frame.
- Dropping the old scene on hot reload drops its media cache entries and GStreamer pipelines (`Drop` sets the pipeline to `State::Null`).
- There are no global tick callbacks or dangling GLib timeouts.

Measured with the real 1080p30 video + glow + backdrop config on the development machine (GTX 1650): ~20–28 % of one CPU core and ~500–515 MB RSS at steady state. Total GPU utilization dropped from ~37 % to ~25–32 % (the desktop baseline without widdgyy is ~22 %; readings include Firefox/desktop and are noisy, so compare on the same machine). Most of the saving comes from cached/quantized shadow blurs and the scissored backdrop; the remaining GPU time is dominated by the video path. Idle text/clock remains ~0.6 % of one core and ~169 MB.

## Project layout

```
src/
  api.rs                 API sticker worker
  app/                   runtime, hot reload, scheduling
  config/                Python DSL runner + JSON5 loader + migration
  css/
    values/              color, length, angle, shadow, gradient, transform
    properties/          box-model, background, border, text, effects, flex, grid, transitions
    selector.rs          selector matching
    parser.rs            stylesheet scanner
    cascade.rs           specificity, variables, inheritance
    computed.rs          typed style modules
  layout/                box model, block/flex/grid formatting
  media/                 GStreamer video backend
  platform/              Wayland layer-shell host
  render/                Vello painters, filter/mask layers, wgpu surface
  scene/                 scene graph
  text/                  shaping + cache
python/widdgyy/          Python config DSL (nodes, build)
tests/                   layout, paint, filters, masks, config tests
```

## Contributing

- Read [CONTRIBUTING.md](CONTRIBUTING.md) for the development workflow.
- Follow [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md).
- Use the issue and pull request templates under `.github/`.

## License

GPL-3.0-or-later. See [LICENSE](LICENSE).
