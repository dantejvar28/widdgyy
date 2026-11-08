# Widdgyy Configuration & Styling Guide

This document explains how to configure Widdgyy with `config.py` (preferred) or `config.jsonc` (legacy), plus `styles.css`, where the files live, which sticker types exist and which CSS features are supported by the Rust/Vello renderer.

## Configuration files

Widdgyy reads:

```
~/.config/widdgyy/config.py       # preferred, executed with python3
~/.config/widdgyy/config.jsonc    # legacy fallback
~/.config/widdgyy/styles.css
```

When both config files exist, `config.py` wins.

Create the directory if it does not exist:

```bash
mkdir -p ~/.config/widdgyy
```

## Config structure

### Python configuration

`config.py` builds the scene with small Python objects. Styling stays in `styles.css`: every object takes a `cls` argument and the engine applies those classes through the normal CSS cascade.

```python
from widdgyy import ApiText, Clock, HBox, Video

stickers = [
    Video("assets/wallpaper.mp4", cls="background", z=-1),
    HBox(Clock("%H:%M:%S", cls="clock"), anchor="center", offset=(0, -220)),
    ApiText(
        "https://api.open-meteo.com/v1/forecast?latitude=-34.6&longitude=-58.4&current=temperature_2m",
        template="{temp}°C",
        fields={"temp": "current.temperature_2m"},
        cls="weather",
        anchor="bottom-right",
        offset=(-40, -40),
    ),
]
```

- Node classes: `Text`, `Clock`, `ApiText`, `Video`, `Image`, `Gif`, `HBox`, `VBox`, `Grid`.
- Common arguments: `cls` (string or list), `id`, `anchor`, `offset=(x, y)`, `z`, `style={...}`, `margin`, `padding`, `gap`, `font_size`, `width`, `height`, `color`.
- Containers take children positionally: `HBox(Clock(...), Text(...), cls="row")`. `Grid(..., columns=3)`.
- Any other JSON field is accepted as a keyword argument and passed through.
- Python loops and constants are allowed to generate repeated stickers; the visual style still belongs in CSS.
- `config.py` runs with your user permissions. Do not put secrets in it.

Migrate an existing JSONC file:

```bash
widdgyy --migrate-config ~/.config/widdgyy/config.jsonc > ~/.config/widdgyy/config.py
```

The Python package is installed to `~/.local/share/widdgyy/python` (or `/usr/share/widdgyy/python` by the package). `WIDDGYY_PYTHON_DIR` can point at a development copy.

### JSONC configuration (legacy)

`config.jsonc` is JSON5, so comments and trailing commas are allowed. The top-level object must contain a `stickers` array.

```jsonc
{
  "stickers": [
    {
      "type": "text",
      "text": "Hello",
      "class": "title",
      "anchor": "top-left",
      "offset_x": 24,
      "offset_y": 24
    }
  ]
}
```

### Text shadows and glow (CSS)

`text-shadow` works like in a browser: each entry is a blurred, coloured copy of the text, and the list can be animated with `@keyframes`.

```css
@keyframes glow {
  from { text-shadow: 0 0 6px #00e5ff, 0 0 12px #00e5ff; }
  to   { text-shadow: 0 0 20px #00e5ff, 0 0 40px #00e5ff, 0 0 60px #00e5ff; }
}

.pulse {
  color: #fff;
  animation: glow 1.5s ease-in-out infinite alternate;
}
```

- Multiple shadows, offsets, colours and blur radii are supported. The first shadow is painted on top, as in CSS.
- Lists of different lengths repeat the shorter list, matching CSS.
- The blur sigma is half the blur radius, as in browsers.
- The glyph coverage is rasterized once and only re-rendered when the text changes. Blur, offset and colour are applied on the GPU, so animating `text-shadow` does not re-rasterize text.
- Shadows are blurred at half resolution and upscaled, which keeps the cost low without a visible loss.

Limits on the video path, per frame: 16 shadows in total and 4 distinct blur radii. Radii beyond the fourth reuse the nearest one. Without a video wallpaper the simpler sampled fallback is used, and its blur is approximate.

Every sticker supports:

| Field | Description |
| --- | --- |
| `type` | Sticker type (see below) |
| `class` | One or more CSS classes, space separated |
| `id` | Optional `#id` selector target |
| `anchor` | `top-left`, `top-center`, `top-right`, `center-left`, `center`, `center-right`, `bottom-left`, `bottom-center`, `bottom-right` |
| `offset_x`, `offset_y` | Pixel offsets applied after anchoring |
| `z_index` | Stacking order; negative values are useful for backgrounds |
| `style` | Optional inline style object (`"style": { "color": "#fff" }`) |

### Values from the config act as defaults

`font_size`, `padding`, `spacing`, `align_items`, etc. are converted into low-priority CSS declarations. A rule in `styles.css` overrides them, exactly like normal CSS author styles. The `style` object is inline CSS and has the highest priority (except `!important` in the stylesheet).

### Sticker types

#### `text`

```jsonc
{ "type": "text", "text": "CPU 42%", "class": "monitor" }
```

Supports `text`, `font_size`/`font-size`, `color`, margins and all common positioning fields.

#### `clock`

```jsonc
{ "type": "clock", "format": "%H:%M:%S", "class": "clock", "update_interval": 1 }
```

`format` uses chrono's `strftime` syntax. The text is regenerated whenever the formatted value changes; `update_interval` is accepted for compatibility but the clock is synchronized to the second.

#### `api_text`

```jsonc
{
  "type": "api_text",
  "class": "weather",
  "url": "https://api.example.com/weather",
  "method": "GET",
  "headers": { "Accept": "application/json" },
  "template": "{city} {temperature}°C",
  "fields": {
    "city": "location.name",
    "temperature": "current.temp_c"
  },
  "timeout": 10,
  "retries": 1,
  "update_interval": 10
}
```

- `fields` maps template names to JSON paths such as `daily.time[0]` or `location.name`.
- Inline filters are supported after `|`: `weekday_short` and `fixed_N` (fixed-point formatting, e.g. `price|fixed_2`).
- Requests run on a worker thread; the UI never blocks. While a request is in flight no duplicate request is started.

#### `media`

```jsonc
{
  "type": "media",
  "path": "assets/wallpaper.mp4",
  "width": "100%",
  "height": "100%",
  "fit": "cover",
  "autoplay": true,
  "loop": true,
  "class": "background"
}
```

- `path` supports `~`, `$VAR`, `${VAR}` and paths relative to the config directory.
- Images: `png`, `jpg`, `jpeg`, `webp`.
- Animated: `gif`.
- Video: `mp4`, `mkv`, `webm`, `avi`, `mov`, `m4v` (GStreamer must have the codec).
- `width`/`height` accept `px` and `%`.
- `fit` maps to `object-fit`: `contain`, `cover`, `stretch`/`fill`, `none`, `scale-down`.

#### `hbox` / `vbox`

```jsonc
{
  "type": "hbox",
  "class": "panel",
  "spacing": 12,
  "padding": 16,
  "justify": "center",
  "align_items": "center",
  "children": [
    { "type": "text", "text": "CPU" },
    { "type": "text", "text": "42%" }
  ]
}
```

- `hbox` = `display: flex; flex-direction: row`.
- `vbox` = `display: flex; flex-direction: column`.
- `spacing` becomes `gap`.
- `justify` maps to `justify-content`; `align_items` to `align-items`.

#### `grid`

```jsonc
{
  "type": "grid",
  "columns": 2,
  "spacing": 8,
  "padding": 12,
  "children": [
    { "type": "text", "text": "A" },
    { "type": "text", "text": "B" },
    { "type": "text", "text": "C" },
    { "type": "text", "text": "D" }
  ]
}
```

`columns` creates that many `1fr` grid tracks. You can override them in CSS:

```css
.my-grid {
  grid-template-columns: 2fr 1fr 120px;
  column-gap: 20px;
  row-gap: 6px;
}
```

## CSS basics

```css
.title {
  color: #f7f2ff;
  font-family: "Angelic War", Sans;
  font-size: 42px;
  font-weight: bold;
  text-shadow: 0 3px 12px rgba(0, 0, 0, 0.7);
}
```

### Selectors

- Type: `text`, `clock`, `media`, `hbox`, `vbox`, `grid`, `api_text`
- Class: `.panel`
- Id: `#cpu`
- Attribute: `[data-x]`, `[role="value"]`
- Descendant: `.panel text`
- Child: `.panel > text`
- `*`
- `:root`, `:first-child`, `:last-child`, `:only-child`, `:nth-child()`, `:not()`, `:is()`, `:where()`

Interaction pseudo-classes (`:hover`, `:focus`, `:active`, …) are rejected during parsing because the wallpaper is display-only. The layer uses an empty Wayland input region, so clicks pass through to the desktop.

### Cascade

The cascade follows browser rules:

1. `!important` declarations win.
2. Higher specificity wins: id > class/attribute/pseudo-class > type.
3. Later rules win on a tie.
4. Inline `style` declarations beat stylesheet declarations unless the stylesheet uses `!important`.
5. Config-derived declarations are applied first and act as defaults.

### Variables

```css
:root {
  --accent: #c7a7ff;
  --panel: rgba(18, 12, 28, 0.82);
  --space: 8px;
}

.panel {
  background-color: var(--panel);
  gap: var(--space);
  color: var(--accent);
}
```

Variables inherit through the scene tree. `var(--missing, 4px)` supports a fallback.

### Units

`px`, `%`, `em`, `rem`, `ex`, `ch`, `vw`, `vh`, `vmin`, `vmax`, `pt`, `pc`, `in`, `cm`, `mm`, `fr`, unitless zero, `auto`, and `calc()` expressions such as `calc(100% - 40px)`.

### Colors

Named colors, `#rgb`, `#rrggbb`, `#rrggbbaa`, `rgb()`, `rgba()`, `hsl()`, `hsla()`, `transparent`, `currentColor`, and the legacy `r, g, b, a` syntax from old Widdgyy stylesheets.

## Supported properties

### Box model

`display` (`block`, `inline`, `inline-block`, `flex`, `grid`, `none`), `width`, `height`, `min-width`, `min-height`, `max-width`, `max-height`, `margin*`, `padding*`, `box-sizing`, `overflow`, `overflow-x`, `overflow-y`, `aspect-ratio`, `visibility`.

`position: absolute` and `position: fixed` take children out of flow and honor `top`/`right`/`bottom`/`left`/`inset`; `position: relative` shifts a child without affecting its siblings. Root placement is still controlled by the config `anchor`/`offset_*` fields.

### Background

`background`, `background-color`, `background-image` (`url()` and gradients), `background-position`, `background-size` (`auto`, `cover`, `contain`, explicit), `background-repeat`, `background-clip`, `background-origin`, `background-blend-mode`, `background-attachment`.

Gradients: `linear-gradient()`, `repeating-linear-gradient()`, `radial-gradient()`, `repeating-radial-gradient()`, `conic-gradient()`.

### Border and outline

`border`, `border-width`, `border-style`, `border-color`, `border-radius`, per-side longhands, `outline`, `outline-width`, `outline-style`, `outline-color`, `outline-offset`.

`border-image-source` (`url()`), `border-image-slice` (including `fill`), `border-image-width` and `border-image-outset` are supported with stretched slices. `border-image-repeat` and the `border-image` shorthand are not implemented yet.

As in browsers, a border is only painted when a non-`none` border style is present. Setting `border-width` alone does not draw a border; add `border-style: solid` (or use the `border` shorthand).

### Text

`color`, `font-family`, `font-size`, `font-weight`, `font-style`, `font-stretch`, `font-variant-caps`, `line-height`, `letter-spacing`, `word-spacing`, `text-align`, `text-align-last`, `text-decoration*`, `text-transform`, `text-indent`, `white-space`, `text-wrap`, `overflow-wrap`, `word-break`, `text-overflow`, `text-shadow`, `direction`, `vertical-align`, and the `font` shorthand.

Colors include named/hex/`rgb()`/`hsl()`, modern spaces (`oklch()`, `oklab()`, `lab()`, `lch()`, `hwb()`, `color()`), and `color-mix(in srgb, ...)` with optional percentages.

If a family exists but does not ship the exact weight/style (common for display fonts with only a Regular face), Widdgyy uses the closest face from that same family instead of silently switching to a different font family.

### Effects

`opacity`, `box-shadow` (outer and inset), `text-shadow`, `mix-blend-mode`, `filter` (color-matrix filters), individual `translate`/`rotate`/`scale`, `transform`, `transform-origin`, `clip-path` (inset/circle/ellipse/polygon/path), `isolation`, `object-fit`, `object-position`, `backdrop-filter` (video path) and `mask-image` (gradients).

`blur()` and `drop-shadow()` work on root nodes and on nested descendants, including inside an already-filtered subtree and under `opacity`, `clip-path`, `overflow: hidden` or `isolation`: the painter builds a recursive offscreen layer tree and applies each group's filter, clip mask and opacity at composite time.

`mix-blend-mode` and `mask-image` wrap their subtree in a single Vello layer; a spatial filter below one of them is painted inline and logged once. Blend modes are not carried across offscreen group boundaries. Layer nesting is capped at 8 levels (deeper groups lose their effects) to bound texture memory.

`backdrop-filter: blur(Npx)` works on the video fast path: the video is rendered offscreen at half resolution, blurred, and blended behind each panel using a coverage mask. The largest blur radius in the scene is used. Without a video wallpaper the backdrop is ignored.

`mask-image` supports linear, radial and conic gradients, plus `url()` masks. Gradient stops are treated as alpha (the RGB is converted to white), so `transparent → black → transparent` fades the element and its children. `mask-size`, `mask-position` and `mask-repeat` apply to the mask image. `mask-mode` and multiple mask layers are not implemented yet.

`text-shadow` is rendered with a real GPU Gaussian blur on both the video and the no-video paths (see above). Scenes with any spatial filter use the offscreen layer path, which falls back to the older sampled text-shadow approximation.

### Flex

`flex`, `flex-direction`, `flex-wrap`, `flex-flow`, `justify-content`, `align-items`, `align-content`, `align-self`, `gap`, `row-gap`, `column-gap`, `order`, `flex-grow`, `flex-shrink`, `flex-basis`.

`flex-wrap` creates multiple lines and respects `align-content`, `justify-content` per line and `align-items`. `flex-grow` distributes free main-axis space and `flex-shrink` removes overflow; `flex-basis` is parsed but the measured size is used as the base for now.

### Grid

`grid-template-columns`, `grid-template-rows`, `grid-auto-flow`, `grid-column`, `grid-row`, `grid-column-start`, `grid-column-end`, `grid-row-start`, `grid-row-end`.

Track values: `auto`, `min-content`, `max-content`, `fit-content()`, `N fr`, `px`, `%`, `repeat()` and `minmax()`.

Explicit `grid-column`/`grid-row` placement (including spans and `span N`) and automatic placement in row-major or column-major order are supported. Track lists accept whitespace-separated values such as `100px 1fr auto` and `repeat(2, 1fr)`. Items with an `auto` axis stretch to their track, like browser grid.

### Animation and transitions

`animation*` and `@keyframes` are applied at runtime for `transform`, `opacity`, `color`, `background-color`, `text-shadow` and custom properties (`--name`), including duration, delay, timing functions, direction, iteration count and fill mode. Multiple animations per node are supported. Shadow lists of different lengths are interpolated by repeating the shorter list, matching CSS. Numeric, length and colour custom-property values are interpolated; other values switch discretely. Animating `text-shadow` refreshes the offscreen shadow layer without repainting the text overlay.

`transition*` is parsed but not wired to style changes yet. Layout-affecting animations (width/height/margin/padding) are also pending.

### Media queries

```css
@media (min-width: 1200px) and (orientation: landscape) {
  .clock { font-size: 80px; }
}
```

Supported features: `min-width`, `max-width`, `min-height`, `max-height`, `orientation`, `prefers-reduced-motion`, `prefers-color-scheme` (the engine renders the light/default palette; dark queries do not match).

## Not implemented

These are parsed where possible but not painted yet, and are documented so they do not fail silently:

- Filters below `mask-image` or `mix-blend-mode`, and group-level blending across offscreen layers.
- `mask-mode` and multiple mask layers.
- `border-image-repeat` and the `border-image` shorthand.
- Multiple background layers.
- `backdrop-filter` outside the video fast path.
- SVG external clip paths (`clip-path: url(...)`).
- `@import`.
- Animation property interpolation/transitions.
- (Interaction pseudo-classes are intentionally unsupported; the wallpaper is display-only.)

## Hot reload

Widdgyy watches the config directory. Writing `config.py`, `config.jsonc` or `styles.css` triggers a debounced reload. Creating `config.py` switches the active source without a restart:

- the new scene replaces the old one;
- styles are re-resolved and layout is recomputed;
- unused media assets are pruned and their GStreamer pipelines are dropped;
- in-flight API requests from the old scene are discarded.

If the new config cannot be parsed, the previous scene is kept and the error is logged.

## Debugging

```bash
RUST_LOG=widdgyy=debug ./target/release/widdgyy
widdgyy --check-css
```

Useful messages: missing fonts, failed media loads, unsupported CSS properties, API failures, watcher errors.

## Quick start

```bash
mkdir -p ~/.config/widdgyy
cp examples/config.py ~/.config/widdgyy/
cp examples/styles.css ~/.config/widdgyy/
./target/release/widdgyy
```
