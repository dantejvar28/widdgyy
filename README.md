# Widdgyy

Widdgyy is an open-source desktop ricing engine for Wayland that turns your wallpaper into a living workspace.

Create clocks, images, videos, text, weather panels, system monitors and API-powered stickers that are rendered directly on the desktop background layer. Everything is configured with JSON and styled with CSS, so you can build highly customized layouts without writing a desktop extension.

## Architecture

Widdgyy is a native Rust application. There is no Python runtime and no GTK dependency.

| Layer | Technology | Why |
| --- | --- | --- |
| Window / desktop integration | `smithay-client-toolkit` + `wlr-layer-shell` | Native background layer, anchors, exclusive zones, no decoration |
| GPU rendering | `vello` + `wgpu` | Vector rendering on Vulkan/GL, GPU-composited transparency |
| Text | `cosmic-text` + Vello glyph runs | Shaping, fallback fonts, wrapping and cacheable glyph runs without Pango |
| Images / GIF | `image` | Decoded once, bounded cache, shared through `Arc` |
| Video | GStreamer `playbin` + `appsink` (NV12) | Y/UV planes are uploaded to wgpu and converted by a shader; no GTK widget, overlay or CPU RGBA conversion |
| Styling | Built-in CSS engine | Typed style modules, cascade, variables, shorthands, media queries |

The old Python implementation was removed. The JSON schema and the CSS class model are preserved, so existing `config.jsonc` / `styles.css` files keep working.

> **Compatibility note:** the previous Python widget extension point was never a public plugin API (types were hard-coded in the loader), so this rewrite does not remove a supported extension mechanism. If a plugin system is added later, it should be designed explicitly (WASM, Lua or a stable Rust trait).

## Features

- **CSS, not ad-hoc properties:** selectors, specificity, `!important`, inheritance, custom properties (`--accent` / `var()`), `calc()`, `min()/max()/clamp()`, media queries, shorthands, gradients, shadows, borders, transforms, clipping, flex and grid.
- **Static CSS parsed once:** the frame loop never parses colors, lengths or selectors.
- **Bounded caches:** text shaping and media assets have hard limits; reloading drops the old scene and releases its assets.
- **Animations:** `@keyframes` for transform, opacity, colors and background-color, with delay/duration/direction/iteration/fill-mode.
- **Diagnostics:** `widdgyy --check-css` lists unsupported properties and at-rules.
- **Hot reload:** editing `config.jsonc` or `styles.css` rebuilds the scene without restarting.
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
./target/release/widdgyy --config /path/to/config.jsonc
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

- `~/.config/widdgyy/config.jsonc`
- `~/.config/widdgyy/styles.css`

A minimal example:

```jsonc
{
  "stickers": [
    {
      "type": "media",
      "path": "assets/wallpaper.mp4",
      "width": "100%",
      "height": "100%",
      "z_index": -1,
      "loop": true,
      "class": "background"
    },
    {
      "type": "vbox",
      "class": "panel",
      "anchor": "center",
      "padding": 24,
      "spacing": 12,
      "align_items": "center",
      "children": [
        { "type": "clock", "class": "clock", "format": "%H:%M" },
        { "type": "text", "class": "subtitle", "text": "Widdgyy" }
      ]
    }
  ]
}
```

```css
:root {
  --accent: #c7a7ff;
}

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

See [docs/Configuration & Styling Guide.md](docs/Configuration%20%26%20Styling%20Guide.md) for the full schema and property reference, and [docs/Architecture & Memory.md](docs/Architecture%20%26%20Memory.md) for the lifecycle and memory rules.

## CSS support

Implemented in the renderer:

- Colors: named, `#rgb`, `#rrggbb`, `#rrggbbaa`, `rgb()`, `rgba()`, `hsl()`, `hsla()`, `transparent`, `currentColor`, legacy `r, g, b, a`.
- Lengths: `px`, `%`, `em`, `rem`, `ex`, `ch`, `vw`, `vh`, `vmin`, `vmax`, `pt`, `pc`, `in`, `cm`, `mm`, `fr`, `auto`, `calc()`.
- Box: `display`, width/height/min/max, margins, padding, `box-sizing`, `overflow`, `aspect-ratio`. `position`/insets are parsed; sticker anchoring is still controlled by the JSON `anchor` field.
- Background: color, images (`url()`), linear/radial/conic gradients, position, size, repeat, clip/origin, blend mode.
- Border/outline: width, style (solid/dashed/dotted), color, radius, outline.
- Text: family, size, weight, style, line-height, letter/word spacing, alignment, decoration, transform, white-space, wrapping, overflow/ellipsis, shadows. `font-stretch`, `font-variant-caps` and `vertical-align` are parsed but not applied by the shaper yet.
- Effects: opacity, box-shadow (including inset), text-shadow, color filters, individual translate/rotate/scale, transform/transform-origin, clip-path shapes, isolation, object-fit/object-position.
- Layout: flex (direction, justify, align-*, gap, order, grow/shrink/basis) and CSS grid templates/auto-flow. `flex-wrap` and explicit `grid-column`/`grid-row` placement are parsed; layout currently uses a single flex line and auto-placement.
- Animations: `@keyframes` applied at runtime for `transform`, `opacity`, `color` and `background-color`; timing functions, delay, direction, iteration and fill modes. Transitions and layout-affecting animations are still pending.
- Media queries: width/height, orientation, `prefers-reduced-motion`, `prefers-color-scheme` parsing.
- Selectors: type, `*`, `.class`, `#id`, `[attr]`, descendant/child combinators, `:root`, `:first-child`, `:last-child`, `:only-child`, `:nth-child()`, `:not()`, `:is()`, `:where()`.

Not implemented yet (tracked explicitly rather than silently ignored):

- Spatial filters (`blur`, `drop-shadow`, `backdrop-filter`) and masks/border-image still need the offscreen layer pipeline. Color-matrix filters are applied to solid colors/text.
- SVG `clip-path: url(...)`.
- `@import` (inline the file instead).
- Animation property interpolation and transitions.

## Performance and memory

The render loop is event-driven:

- The Wayland event loop waits with a 16 ms timeout only when a GIF/video/API sticker is active; otherwise it wakes every 200 ms for the clock.
- Background video uses a fast compositor: the newest NV12 frame is uploaded to two GPU planes and converted to RGB in a shader, while the static Vello overlay is cached and only re-rasterized when text/API content changes. Video playback no longer re-runs Vello over the whole 1080p scene.
- Text is shaped once per content/style pair and cached (bounded to 512 entries). Painting reuses the shaped glyph runs.
- Styles are resolved and layout runs only on load, reload and resize, not per frame.
- Images are decoded once and shared with `Arc`; the image cache is bounded to 64 entries and pruned on reload.
- GIFs are bounded to 240 frames / 64 M pixels. Video keeps a single latest frame.
- Dropping the old scene on hot reload drops its media cache entries and GStreamer pipelines (`Drop` sets the pipeline to `State::Null`).
- There are no global tick callbacks or dangling GLib timeouts.

Measured with the real 1080p30 video config on the development machine (GTX 1650): ~16 % of one CPU core and ~475 MB RSS at steady state, versus ~66 %/~515 MB before the fast compositor. Idle text/clock remains ~0.6 % of one core and ~169 MB.

## Project layout

```
src/
  api.rs                 API sticker worker
  app/                   runtime, hot reload, scheduling
  config/                JSON5 loader
  css/
    values/              color, length, angle, shadow, gradient, transform
    properties/          box-model, background, border, text, effects, flex, grid, transitions
    selector.rs          selector matching
    parser.rs            stylesheet scanner
    cascade.rs           specificity, variables, inheritance
    computed.rs          typed style modules
  layout/                box model, flex, grid
  media/                 GStreamer video backend
  platform/              Wayland layer-shell host
  render/                Vello painters + wgpu surface
  scene/                 scene graph
  text/                  shaping + cache
tests/offscreen_render.rs
```

## Contributing

- Read [CONTRIBUTING.md](CONTRIBUTING.md) for the development workflow.
- Follow [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md).
- Use the issue and pull request templates under `.github/`.

## License

GPL-3.0-or-later. See [LICENSE](LICENSE).
