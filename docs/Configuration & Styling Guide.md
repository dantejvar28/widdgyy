# Widdgyy Configuration & Styling Guide

This document explains how to configure Widdgyy with `config.jsonc` and `styles.css`, where the files live, which sticker types exist and which CSS features are supported by the Rust/Vello renderer.

## Configuration files

Widdgyy reads:

```
~/.config/widdgyy/config.jsonc
~/.config/widdgyy/styles.css
```

Create the directory if it does not exist:

```bash
mkdir -p ~/.config/widdgyy
```

## Config structure

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
- Inline filters are supported after `|`; currently `weekday_short` is available.
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

Dynamic pseudo classes (`:hover`, etc.) are parsed but never match because the background layer does not receive pointer input.

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

`position` and `top`/`right`/`bottom`/`left`/`inset` are parsed, but root placement is controlled by the JSON `anchor`/`offset_*` fields. Absolute-positioned children are not yet laid out out-of-flow.

### Background

`background`, `background-color`, `background-image` (`url()` and gradients), `background-position`, `background-size` (`auto`, `cover`, `contain`, explicit), `background-repeat`, `background-clip`, `background-origin`, `background-blend-mode`, `background-attachment`.

Gradients: `linear-gradient()`, `repeating-linear-gradient()`, `radial-gradient()`, `repeating-radial-gradient()`, `conic-gradient()`.

### Border and outline

`border`, `border-width`, `border-style`, `border-color`, `border-radius`, per-side longhands, `outline`, `outline-width`, `outline-style`, `outline-color`, `outline-offset`.

As in browsers, a border is only painted when a non-`none` border style is present. Setting `border-width` alone does not draw a border; add `border-style: solid` (or use the `border` shorthand).

### Text

`color`, `font-family`, `font-size`, `font-weight`, `font-style`, `font-stretch`, `font-variant-caps`, `line-height`, `letter-spacing`, `word-spacing`, `text-align`, `text-align-last`, `text-decoration*`, `text-transform`, `text-indent`, `white-space`, `text-wrap`, `overflow-wrap`, `word-break`, `text-overflow`, `text-shadow`, `direction`, `vertical-align`, and the `font` shorthand.

If a family exists but does not ship the exact weight/style (common for display fonts with only a Regular face), Widdgyy uses the closest face from that same family instead of silently switching to a different font family.

### Effects

`opacity`, `box-shadow`, `text-shadow`, `mix-blend-mode`, `transform`, `transform-origin`, `clip-path`, `isolation`, `object-fit`, `object-position`.

`filter` and `backdrop-filter` are parsed but still need the layer/filter pipeline.

`text-shadow` blur is approximated with a sampled glyph disk (Vello has no arbitrary glyph blur). Larger blur radii visibly feather further; the pass count is capped for animated scenes.

### Flex

`flex`, `flex-direction`, `flex-wrap`, `flex-flow`, `justify-content`, `align-items`, `align-content`, `align-self`, `gap`, `row-gap`, `column-gap`, `order`, `flex-grow`, `flex-shrink`, `flex-basis`.

`flex-wrap` is parsed but the current layout engine places children on a single line. `flex-grow`/`flex-shrink` are parsed; explicit sizes/margins determine the current box sizes.

### Grid

`grid-template-columns`, `grid-template-rows`, `grid-auto-flow`, `grid-column`, `grid-row`, `grid-column-start`, `grid-column-end`, `grid-row-start`, `grid-row-end`.

Track values: `auto`, `min-content`, `max-content`, `fit-content()`, `N fr`, `px`, `%`, `repeat()` and `minmax()`.

Explicit `grid-column`/`grid-row` placement is parsed, but the layout engine currently auto-places children in row-major order.

### Animation and transitions

`animation*` and `@keyframes` are applied at runtime for `transform`, `opacity`, `color` and `background-color`, including duration, delay, timing functions, direction, iteration count and fill mode. Multiple animations per node are supported.

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

- General `filter` / `backdrop-filter` effects (the values are parsed; only `mix-blend-mode`, opacity and shadows are rendered).
- SVG external clip paths (`clip-path: url(...)`).
- `@import`.
- Animation property interpolation/transitions.
- Pointer/hover/focus pseudo classes (the surface uses `KeyboardInteractivity::None`).

## Hot reload

Widdgyy watches the config directory. Writing `config.jsonc` or `styles.css` triggers a debounced reload:

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
cp examples/config.jsonc ~/.config/widdgyy/
cp examples/styles.css ~/.config/widdgyy/
./target/release/widdgyy
```
