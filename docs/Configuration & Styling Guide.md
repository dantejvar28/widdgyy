# Widdgyy Configuration & Styling Guide

Widdgyy turns a Wayland wallpaper into a display-only scene. It has three independent inputs:

1. **Scene**: `config.py` (preferred) or the legacy `config.jsonc`. It describes nodes, their classes, anchors, media and data sources.
2. **Presentation**: `styles.css`. It uses the normal CSS cascade, so the same class can style any node.
3. **Assets**: local images, GIFs, videos and fonts referenced from the config or CSS.

Everything is rendered by the built-in Rust/Vello engine. The wallpaper has no interaction: clicks, hover, focus and keyboard events are not handled, and the Wayland surface has an empty input region.

## Files and precedence

Widdgyy reads:

```
~/.config/widdgyy/config.py       # preferred, executed with python3
~/.config/widdgyy/config.jsonc    # legacy fallback
~/.config/widdgyy/styles.css
~/.config/widdgyy/assets/...      # optional assets directory
```

- If `config.py` exists, it wins over `config.jsonc`. `styles.css` is always read.
- Use `--config PATH` to point at another file. The CSS file is looked up next to it.
- Relative asset paths are resolved against the config directory, then the current directory.
- `~`, `$VAR` and `${VAR}` are expanded in media paths.
- `WIDDGYY_PYTHON_DIR` can point at a development copy of the Python package.
- Python configs require Python 3.10 or newer (the package uses modern type syntax).

Create the directory first:

```bash
mkdir -p ~/.config/widdgyy/assets
```

## Python configuration (`config.py`)

### How it runs

`config.py` is plain Python. The engine executes it in a subprocess:

```bash
python3 -m widdgyy.build ~/.config/widdgyy/config.py
```

- The script must define a top-level `stickers` list containing `Sticker` objects (usually `Text`, `Clock`, `ApiText`, `Video`, `Image`, `Gif`, `HBox`, `VBox` or `Grid`).
- The package converts the objects to the same JSON scene document used by the legacy loader, so CSS, layout, media and API behavior are identical.
- The subprocess has a **3 second timeout**. A failing or slow config keeps the previous scene alive and logs `stderr`.
- The config runs with your user permissions. There is **no sandbox**: do not put secrets in it, and treat it like any other local script.
- The Python interpreter is the system `python3`; Widdgyy does not embed a Python runtime.

Package lookup order:

1. `$WIDDGYY_PYTHON_DIR`
2. `$XDG_DATA_HOME/widdgyy/python`
3. `~/.local/share/widdgyy/python`
4. `/usr/share/widdgyy/python`
5. `python/` inside the development checkout

### Minimal example

```python
from widdgyy import ApiText, Clock, HBox, Video

WEATHER = (
    "https://api.open-meteo.com/v1/forecast"
    "?latitude=-34.6&longitude=-58.4&current=temperature_2m,relative_humidity_2m"
)

stickers = [
    Video("assets/wallpaper.mp4", cls="background", z=-1),
    HBox(
        Clock("%H:%M:%S", cls="clock"),
        anchor="center",
        offset=(0, -220),
    ),
    ApiText(
        WEATHER,
        template="{temp}°C  {hum}%",
        fields={
            "temp": "current.temperature_2m",
            "hum": "current.relative_humidity_2m",
        },
        cls="weather",
        anchor="bottom-right",
        offset=(-40, -40),
    ),
]
```

### Node classes

| Class | Constructor | Scene `type` | Notes |
| --- | --- | --- | --- |
| `Text` | `Text(text="", **fields)` | `text` | Static text. |
| `Clock` | `Clock(format="%H:%M:%S", **fields)` | `clock` | `strftime` format; updates on the second. |
| `ApiText` | `ApiText(url, template="", fields=None, **fields)` | `api_text` | Text fed by an HTTP JSON endpoint. |
| `Video` | `Video(path, **fields)` | `media` | Video file. Kind is detected by extension. |
| `Image` | `Image(path, **fields)` | `media` | Static image. |
| `Gif` | `Gif(path, **fields)` | `media` | Animated GIF. |
| `HBox` | `HBox(*children, children_list=None, **fields)` | `hbox` | `display: flex; flex-direction: row`. |
| `VBox` | `VBox(*children, children_list=None, **fields)` | `vbox` | `display: flex; flex-direction: column`. |
| `Grid` | `Grid(*children, columns=2, **fields)` | `grid` | `display: grid` with `columns` equal `1fr` tracks. |
| `Sticker` | base class | — | Subclass it and set `sticker_type` for custom builders. |

Containers accept children positionally, as a list, or both:

```python
row = HBox(Clock(cls="clock"), Text("Widdgyy", cls="brand"), cls="row")
grid = Grid(*[Text(f"{i}", cls="cell") for i in range(6)], columns=3)
```

### Common fields

Every sticker accepts these keyword arguments:

| Field | Type | Description |
| --- | --- | --- |
| `cls` | `str` or iterable of `str` | CSS classes. Lists are joined with spaces. |
| `id` | `str` | `#id` selector target. |
| `anchor` | `str` | `top-left`, `top-center`, `top-right`, `center-left`, `center`, `center-right`, `bottom-left`, `bottom-center`, `bottom-right`. |
| `offset` | `(x, y)` | Pixel offset after anchoring. |
| `offset_x`, `offset_y` | number | Alternative to `offset`. |
| `z` / `z_index` | int | Stacking order. Negative values are useful for backgrounds. |
| `style` | mapping | Inline CSS declarations, for example `style={"color": "#fff", "gap": "12px"}`. |
| `**fields` | any | Any extra keyword is forwarded to the scene document. `None` values are dropped. |

Unknown extra fields are passed through instead of rejected. This is what lets the Python DSL expose every scene option without a wrapper for each one.

### `Text`

```python
Text("CPU 42%", cls="monitor")
```

| Field | Description |
| --- | --- |
| `text` | Text content. Missing/`None` becomes an empty string. |

### `Clock`

```python
Clock("%H:%M", cls="clock")
Clock("%A %d %B", cls="date")
```

| Field | Description |
| --- | --- |
| `format` | `chrono`/`strftime` pattern. Defaults to `%H:%M:%S`. |

The text is regenerated whenever the formatted value changes. `update_interval` is accepted for compatibility, but clocks are synchronized to the second.

### `ApiText`

```python
ApiText(
    "https://api.example.com/weather",
    method="GET",
    headers={"Accept": "application/json"},
    template="{city} {temperature}°C",
    fields={
        "city": "location.name",
        "temperature": "current.temp_c",
    },
    timeout=10,
    retries=1,
    update_interval=10,
    cls="weather",
)
```

| Field | Default | Description |
| --- | --- | --- |
| `url` | required | Endpoint URL. |
| `method` | `GET` | HTTP method. |
| `headers` | `{}` | Request headers. |
| `template` | `""` | Output text; `{name}` placeholders are replaced from `fields`. |
| `fields` | `{}` | Map of placeholder name to JSON path. Supports paths such as `daily.time[0]` and `location.name`. |
| `timeout` | `10` | Request timeout in seconds. |
| `retries` | `1` | Retry count. |
| `update_interval` | `10` | Seconds between requests. |

Inline filters after a JSON path:

- `field|weekday_short` formats a weekday.
- `field|fixed_2` formats with a fixed number of decimals.

Requests run on a worker thread. While a request is in flight no duplicate request is started.

### Media: `Video`, `Image`, `Gif`

```python
Video("assets/wallpaper.mp4", cls="background", autoplay=True, loop=True)
Image("assets/logo.png", cls="badge")
Gif("assets/star.gif", cls="mascot")
```

| Field | Default | Description |
| --- | --- | --- |
| `path` | required | Local image, GIF or video path. |
| `autoplay` | `true` | Start video playback automatically. |
| `loop` | `true` | Loop video playback. |

Supported formats:

- Images: `png`, `jpg`, `jpeg`, `webp`.
- Animated: `gif`.
- Video: `mp4`, `mkv`, `webm`, `avi`, `mov`, `m4v`. GStreamer must have the codec installed.

Static images and GIFs get their intrinsic size from the file; videos are sized by CSS or config `width`/`height`. `object-fit` is controlled by `fit` or by CSS:

```python
Video("assets/wallpaper.mp4", fit="cover", cls="background")
```

`fit` accepts `contain`, `cover`, `fill`/`stretch`, `none` and `scale-down`.

### `HBox` and `VBox`

```python
HBox(
    Text("CPU", cls="label"),
    Text("42%", cls="value"),
    spacing=12,
    padding=16,
    justify="center",
    align_items="center",
    cls="row",
)
```

| Field | Description |
| --- | --- |
| positional / `children_list` | Child stickers. |
| `spacing` | Maps to `gap`. |
| `padding`, `padding_top`, `padding_right`, `padding_bottom`, `padding_left` | Padding. |
| `justify` / `align` | Maps to `justify-content`. |
| `align_items` | Maps to `align-items`. |

`HBox` sets `display: flex; flex-direction: row`; `VBox` uses `flex-direction: column`. Both are low-priority defaults, so `styles.css` can change them.

### `Grid`

```python
Grid(
    Text("A", cls="cell"),
    Text("B", cls="cell"),
    Text("C", cls="cell"),
    Text("D", cls="cell"),
    columns=2,
    spacing=8,
    padding=12,
    cls="dashboard",
)
```

`columns` (default `2`, minimum `1`) generates `grid-template-columns: 1fr 1fr ...`. Override it in CSS like any other grid:

```css
.dashboard {
  grid-template-columns: 2fr 1fr 120px;
  column-gap: 20px;
  row-gap: 6px;
}
```

### Style defaults from config

Top-level config fields are converted into **low-priority CSS declarations**. They are applied before `styles.css`, so a stylesheet rule overrides them, and an inline `style` object is the last author style (except `!important`).

| Config field | CSS property |
| --- | --- |
| `font_size` / `font-size` | `font-size` |
| `color` | `color` |
| `width` | `width` |
| `height` | `height` |
| `z_index` / `z-index` | `z-index` |
| `fit` | `object-fit` |
| `margin`, `margin_top`, ... | `margin`, `margin-*` |
| `padding`, `padding_top`, ... | `padding`, `padding-*` |
| `spacing` | `gap` |
| `justify` / `align` | `justify-content` |
| `align_items` | `align-items` |

Numbers are treated as pixels for length properties. For example `padding=16` is equivalent to `padding: 16px`.

The `style` mapping is inline CSS:

```python
HBox(
    Text("42%", cls="value"),
    cls="panel",
    style={"background": "rgba(10, 10, 20, 0.6)", "border-radius": "24px"},
)
```

### Building layouts with Python

Because the config is ordinary Python, loops and constants are encouraged. Keep the visual styling in `styles.css`; use Python for structure and data.

```python
from widdgyy import HBox, Text, VBox, Video

DAYS = ["MON", "TUE", "WED", "THU", "FRI"]

stickers = [
    Video("assets/wallpaper.mp4", cls="background", z=-1),
    VBox(
        HBox(
            Text("WIDDGY", cls="title"),
            Text("40.7°N", cls="subtitle"),
            cls="header",
        ),
        HBox(
            *[Text(day, cls="day") for day in DAYS],
            spacing=18,
            cls="week",
        ),
        anchor="center",
        offset=(0, 40),
    ),
]
```

### Custom stickers (advanced)

The built-in classes cover the scene types, but the base class is public. A custom sticker sets `sticker_type` and fills `self.data`; `to_dict()` returns the JSON document the engine consumes.

```python
from widdgyy import Sticker

class Badge(Sticker):
    sticker_type = "text"

    def __init__(self, value: str, **fields):
        super().__init__(**fields)
        self.data["text"] = value
        self.data["class"] = "badge"
```

Extra fields passed to `super().__init__()` are forwarded as usual. `ANCHORS` is exported if you need to validate anchor names yourself.

### Migration from JSONC

```bash
widdgyy --migrate-config ~/.config/widdgyy/config.jsonc > ~/.config/widdgyy/config.py
# or, with explicit source and destination
widdgyy --migrate-config old-config.jsonc > config.py
```

The converter emits the Python DSL with `cls=` arguments, keeps `style` objects, and preserves unknown fields as keyword arguments. Presentation still belongs in `styles.css`.

## JSONC configuration (legacy)

`config.jsonc` is JSON5: comments and trailing commas are allowed. It uses the same scene schema.

```jsonc
{
  "stickers": [
    {
      "type": "media",
      "path": "assets/wallpaper.mp4",
      "class": "background",
      "z_index": -1,
      "loop": true
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

Differences from the Python DSL:

- `type` selects the node kind: `text`, `clock`, `api_text`, `media`, `hbox`, `vbox`, `grid`.
- `class` must be a string (the Python DSL also accepts a list).
- Offsets use `offset_x` / `offset_y` (aliases `offset-x`, `x` and `y` are accepted).
- Children are a `children` array.
- `style` is an object of CSS declarations.

Python is preferred for new configs. Both formats support hot reload and the same CSS.

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

Supported:

- Type: `text`, `clock`, `media`, `hbox`, `vbox`, `grid`, `api_text`
- Universal: `*`
- Class: `.panel`
- Id: `#cpu`
- Attribute: `[data-x]`, `[role="value"]`, `[class~="x"]`, `[lang|="en"]`, `[href^="https"]`, `[src$=".png"]`, `[title*="foo"]`, with the optional `i` case-insensitive flag
- Descendant: `.panel text`
- Child: `.panel > text`
- Pseudo-classes: `:root`, `:first-child`, `:last-child`, `:only-child`, `:nth-child()`, `:nth-last-child()`, `:not()`, `:is()` (also `:matches()`/`:any()`), `:where()`
- Generated boxes: `::before`, `::after` (legacy `:before`/`:after` are accepted). Other pseudo-elements are rejected.

Parsed but not matched:

- Adjacent sibling `+` and general sibling `~` are accepted by the parser but never match.

Rejected on purpose:

- Interaction pseudo-classes `:hover`, `:focus`, `:active`, `:focus-visible`, `:visited`, etc. The wallpaper is display-only.

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

Variables inherit through the scene tree. `var(--missing, 4px)` supports a fallback. Numeric, length and colour custom properties can be animated; other values switch discretely.

### Units and math

`px`, `%`, `em`, `rem`, `ex`, `ch`, `vw`, `vh`, `vmin`, `vmax`, `pt`, `pc`, `in`, `cm`, `mm`, `fr`, unitless zero, `auto`, and these functions:

- `calc(100% - 40px)`
- `min(10vw, 200px)`
- `max(12px, 2em)`
- `clamp(12px, 2vw, 20px)`

`fr` is meaningful in grid track lists and flex bases.

### Colors

Named colors, `#rgb`, `#rrggbb`, `#rrggbbaa`, `rgb()`, `rgba()`, `hsl()`, `hsla()`, `transparent`, `currentColor`, and the legacy `r, g, b, a` syntax from old Widdgyy stylesheets.

Modern color spaces are supported:

- `oklch()`, `oklab()`, `lab()`, `lch()`, `hwb()`, `color()`
- `color-mix(in srgb, red 25%, blue 75%)` with optional percentages

### At-rules

| At-rule | Behavior |
| --- | --- |
| `@media` | Parsed and applied. Supports media types (`screen`, `all`, `print`), `not`/`only`, and the features listed below. |
| `@supports` | Declaration and selector conditions, including `and`, `or`, `not`, `selector(...)`. |
| `@keyframes`, `@-webkit-keyframes` | Keyframe animations. |
| `@font-face` | Local `font-family`, `src: url(...)`, `font-weight` and `font-style`. |
| `@layer` | The block is parsed and flattened; there is no layer ordering. |
| `@import` | Rejected with a diagnostics message; inline the imported file. |
| Other at-rules | Reported by `widdgyy --check-css`. |

### Media queries

```css
@media (min-width: 1200px) and (orientation: landscape) {
  .clock { font-size: 80px; }
}
```

Supported features:

- Dimensions: `min-width`, `max-width`, `min-height`, `max-height`, `orientation` (`portrait`/`landscape`).
- Color and motion: `prefers-color-scheme` (`light`/`dark`) and `prefers-reduced-motion` (`reduce`/`no-preference`).
- Output: `resolution`, `min-resolution`, `max-resolution` (`dppx`/`x`, `dpi`, `dpcm`); `aspect-ratio`, `min-aspect-ratio`, `max-aspect-ratio` (`16/9` or a decimal).
- Input: `hover`, `any-hover` (`none`/`hover`), `pointer`, `any-pointer` (`none`/`coarse`/`fine`).
- Display: `display-mode` (`fullscreen`, `browser`, `minimal-ui`, `standalone`, `picture-in-picture`, `borderless`, `window-controls-overlay`, `tabbed`).

How preferences are detected (at load and reload):

- `prefers-color-scheme`: `WIDDGYY_COLOR_SCHEME=light|dark` overrides detection. Otherwise Widdgyy reads `gsettings org.gnome.desktop.interface color-scheme`, then `gtk-theme`, then KDE's `ColorScheme`, then `GTK_THEME`. The default is light.
- `prefers-reduced-motion`: `WIDDGYY_REDUCED_MOTION=1|0` overrides. Otherwise `gsettings ... enable-animations=false` or KDE's `AnimationDurationFactor=0` enables reduce. The default is no-preference.
- `hover`/`any-hover` and `pointer`/`any-pointer`: the wallpaper has no input, so they default to `none`. Override with `WIDDGYY_HOVER`, `WIDDGYY_ANY_HOVER`, `WIDDGYY_POINTER`, `WIDDGYY_ANY_POINTER` for testing or unusual setups.
- `display-mode`: defaults to `fullscreen` because the surface is a layer-shell background. Override with `WIDDGYY_DISPLAY_MODE`.
- `resolution`: uses the viewport scale factor (currently 1.0 until the platform reports fractional output scale), so `1dppx`/`96dpi` is the default.

Behavior notes:

- Comma-separated conditions are OR; `and` is AND; `not` negates the whole condition; `only` is accepted.
- `print` does not match; `screen` and `all` do.
- Unknown media features do not match (they are not silently ignored).
- Theme/preference changes are picked up on the next reload (touching `styles.css` or `config.py` is enough).
- `widdgyy --check-css` prints the detected preferences.
- `scripting`, `prefers-contrast`, `prefers-reduced-transparency`, `forced-colors`, `update`, `overflow-block` and similar features are not implemented.

## Supported CSS properties

The lists below are the properties the parser accepts. Unknown properties are reported by `widdgyy --check-css` and listed again in [Not implemented](#not-implemented).

### Box model and layout

- `display`: `none`, `block`, `inline`, `inline-block`, `flex`, `inline-flex`, `grid`, `inline-grid`, plus the `-webkit-box` alias used by line-clamp markup. Containers use the flex layout unless the value is `grid`; block/inline/inline-block do not create a distinct formatting context.
- `position`: `static`, `relative`, `absolute`, `fixed`. `sticky` is accepted but behaves as `static`.
- `top`, `right`, `bottom`, `left`, `inset`, `inset-inline`, `inset-inline-start`, `inset-inline-end`, `inset-block`, `inset-block-start`, `inset-block-end`
- `width`, `height`, `min-width`, `min-height`, `max-width`, `max-height` (values: `auto`, `none`, `stretch`/`-webkit-fill-available`, `min-content`, `max-content`, `fit-content()`, lengths and percentages)
- `inline-size`, `block-size`, `min-inline-size`, `min-block-size`, `max-inline-size`, `max-block-size` (aliases for width/height)
- `margin`, `margin-top`, `margin-right`, `margin-bottom`, `margin-left`
- `margin-inline`, `margin-inline-start`, `margin-inline-end`, `margin-block`, `margin-block-start`, `margin-block-end`
- `padding`, `padding-top`, `padding-right`, `padding-bottom`, `padding-left`
- `padding-inline`, `padding-inline-start`, `padding-inline-end`, `padding-block`, `padding-block-start`, `padding-block-end`
- `box-sizing`
- `overflow`, `overflow-x`, `overflow-y` (`scroll`/`auto` clip but do not scroll)
- `aspect-ratio`
- `z-index`
- `visibility`: `visible`, `hidden`, `collapse`

`position: absolute` and `position: fixed` take children out of flow and honor insets. `position: relative` shifts a child without affecting its siblings. Root placement is still controlled by the config `anchor`/`offset` fields.

### Backgrounds

- `background` (single or comma-separated layers)
- `background-color`
- `background-image`: `url()` and gradients
- `background-position`
- `background-size`: `auto`, `cover`, `contain`, explicit
- `background-repeat`: `repeat`, `repeat-x`, `repeat-y`, `no-repeat`, `space`, `round`
- `background-clip`: `border-box`, `padding-box`, `content-box` (`text` is painted as `content-box`)
- `background-origin`: `border-box`, `padding-box`, `content-box`
- `background-blend-mode`: the first value applies to every layer
- `background-attachment`: parsed, no visual effect (there is no scrolling)

Multiple background layers are painted with per-layer position, size and repeat lists. The first image is on top, as in CSS. `background-blend-mode` uses a single value for all layers.

Gradients: `linear-gradient()`, `repeating-linear-gradient()`, `radial-gradient()`, `repeating-radial-gradient()`, `conic-gradient()`. Explicit stop positions, padding before the first stop and after the last stop, and hard stops with two colours at the same position are supported.

### Borders, outlines and border images

- `border`, `border-width`, `border-style`, `border-color`
- `border-top`, `border-right`, `border-bottom`, `border-left`
- `border-top-width`, `border-right-width`, `border-bottom-width`, `border-left-width`
- `border-top-style`, `border-right-style`, `border-bottom-style`, `border-left-style`
- `border-top-color`, `border-right-color`, `border-bottom-color`, `border-left-color`
- `border-radius` (including `border-radius: inherit`), `border-top-left-radius`, `border-top-right-radius`, `border-bottom-right-radius`, `border-bottom-left-radius`
- `outline`, `outline-width`, `outline-style`, `outline-color`, `outline-offset`
- `border-image-source` (`url()` only), `border-image-slice` (including `fill`), `border-image-width`, `border-image-outset`, `border-image-repeat`, `border-image` shorthand

Notes:

- As in browsers, a border is only painted when a non-`none` border style is present. `border-width` alone does not draw a border.
- `dashed` and `dotted` are rendered. `double`, `groove`, `ridge`, `inset` and `outset` are accepted but painted as solid.
- Elliptical `border-radius: 10px / 20px` is parsed, but only the horizontal radius is used.
- `border-image` supports `stretch`, `repeat`, `round` and `space`, one value for both axes or `horizontal vertical`. Corners are stretched, edges tile as specified, and the center is only drawn with `fill`. Tiling is capped at 512 tiles per axis.
- `outline-style` is accepted; the outline is painted as a solid stroke.

### Text

- `color`
- `font`, `font-family`, `font-size`, `font-weight`, `font-style`
- `font-stretch` (keyword or percentage; selects the closest face)
- `font-variant`, `font-variant-caps` (OpenType `smcp`, `c2sc`, `pcap`, `unic`, `titl`; no synthetic caps)
- `font-feature-settings` (OpenType feature tags such as `"liga" 0` or `"ss01"`)
- `font-variation-settings`, `text-rendering` (accepted, no effect)
- `line-height`, `letter-spacing`, `word-spacing`
- `text-align`, `text-align-last`
- `text-decoration`, `text-decoration-line`, `text-decoration-color`, `text-decoration-style`, `text-decoration-thickness`
- `text-transform`
- `text-indent` (offsets the first line; wrapping still uses the full width)
- `white-space`, `text-wrap` (maps `nowrap` to `white-space: nowrap`, otherwise normal), `overflow-wrap`/`word-wrap`, `word-break`
- `text-overflow`: `clip`, `ellipsis`; `line-clamp` and `-webkit-line-clamp` (multi-line ellipsis)
- `text-shadow`
- `direction` (maps `start`/`end` alignment for RTL; bidi shaping follows the text content)
- `vertical-align` (aligns the text block inside its box for `top`/`middle`/`bottom`; `sub`/`super` shift by 0.2em; not an inline formatting context)

If a family exists but does not ship the exact weight/style (common for display fonts with only a Regular face), Widdgyy uses the closest face from that same family instead of silently switching to a different font family.

### Effects, filters and masks

- `opacity`
- `box-shadow` (outer and inset, multiple shadows)
- `text-shadow`
- `filter`: `blur()`, `drop-shadow()`, `brightness()`, `contrast()`, `grayscale()`, `saturate()`, `sepia()`, `invert()`, `opacity()`, `hue-rotate()`
- `backdrop-filter` (`blur()` only, video fast path; `-webkit-backdrop-filter` alias)
- `mix-blend-mode` (the 16 CSS blend modes; `plus-lighter` maps to normal)
- `translate`, `rotate`, `scale`
- `transform`: `translate()`, `translateX()`, `translateY()`, `scale()`, `scaleX()`, `scaleY()`, `rotate()`, `skew()`, `skewX()`, `skewY()`, `matrix()`
- `transform-origin`
- `clip-path` (`-webkit-clip-path` alias): `inset()`, `circle()`, `ellipse()`, `polygon()`, `path()`
- `isolation`
- `object-fit`, `object-position`
- `mask-image`, `mask-mode`, `mask-size`, `mask-position`, `mask-repeat`

Filter and mask behavior:

- `blur()` and `drop-shadow()` work at any depth: the painter builds a recursive offscreen layer tree. Nested filters work under `opacity`, `clip-path`, `overflow: hidden` and `isolation`, and inside another filtered subtree.
- A group's effects are applied in CSS order: spatial filter first, then clip mask, then opacity.
- Layer nesting is capped at 8 levels. Deeper groups are composited flat and a warning is logged once.
- When a node has several spatial filters, `blur()` wins over `drop-shadow()` and multiple blur radii collapse to the largest one.
- Spatial filters below `mask-image` or `mix-blend-mode` are painted inline (those properties wrap the subtree in one Vello layer); this is logged once.
- `backdrop-filter: blur(Npx)` only works with a video wallpaper on the fast path. The largest blur radius in the scene is used; the video is rendered offscreen at 35% resolution with two separable blur iterations, scissored to the union of the backdrop panels, and blended through a panel coverage mask. A scene with spatial filters uses the full layer path, where `backdrop-filter` is ignored.
- `mask-image` accepts a comma-separated list of gradients and `url()` masks. `mask-size`, `mask-position`, `mask-repeat` and `mask-mode` (`match-source`, `alpha`, `luminance`) are list-valued and match each layer; missing values repeat from the start of the list. Multiple layers accumulate additively, matching the initial `mask-composite: add`. `match-source` resolves to alpha for CSS images (rasters and gradients). Gradient stops are painted as white with their alpha; a luminance mask multiplies the image's luminance by its alpha.
- `mask-composite` operators other than additive composition are not implemented.
- `mix-blend-mode` only blends against content painted earlier in the same Vello scene/group. Blend modes are not carried across offscreen group boundaries.

`text-shadow` is rendered with a real GPU Gaussian blur on both the video and no-video paths. The glyph mask is cached, blur slots are reused until the quantized sigma changes, and the blur/composite passes are scissored to the shadow bounding box, so animated shadow colour/offset does not re-blur every frame. Scenes with any spatial filter use the offscreen layer path, which falls back to the sampled text-shadow approximation.

### Generated content (`::before` / `::after`)

```css
.markets-panel { position: relative; overflow: hidden; border-radius: 14px; }
.markets-panel::before {
  content: "";
  position: absolute;
  inset: 0;
  border-radius: inherit;
  background: linear-gradient(135deg, rgba(255,255,255,0.20), transparent 45%);
  pointer-events: none;
}
.markets-panel::after {
  content: "LIVE";
  position: absolute;
  right: 12px;
  bottom: 10px;
  color: #fff;
}
```

- `content` accepts quoted strings, `none` and `normal`. `content: none`/`normal` produces no box. `attr()`, counters, quotes, images and `content: url()` are not implemented.
- `::before` is inserted before the element's children; `::after` after them. Both are painted above the element's own background and border and below/above the other children according to source order and `z-index`.
- A generated box participates in layout as a normal box. With `position: absolute`, its containing block is the parent's padding box, so `inset: 0` fills it. `left`/`right` or `top`/`bottom` with an auto width/height stretch the box between the insets.
- `border-radius: inherit` copies the originating element's radius; this is the only property that currently supports the `inherit` keyword explicitly.
- `pointer-events` is accepted as a no-op so browser-oriented stylesheets keep loading.

### Flex

- `flex`, `flex-direction`, `flex-wrap`, `flex-flow`
- `justify-content`, `align-items`, `align-content`, `align-self`
- `gap`, `row-gap`, `column-gap` (aliases `grid-row-gap`, `grid-column-gap`)
- `order`, `flex-grow`, `flex-shrink`, `flex-basis`

`flex-wrap` creates multiple lines and respects `align-content`, per-line `justify-content` and `align-items`. `flex-grow` distributes free main-axis space and `flex-shrink` removes overflow. `flex-basis` is parsed, but the measured size is used as the base for now.

### Grid

- `grid-template-columns`, `grid-template-rows`
- `grid-auto-flow`: `row`, `column`, `row dense`, `column dense`
- `grid-column`, `grid-column-start`, `grid-column-end`
- `grid-row`, `grid-row-start`, `grid-row-end`

Track values: `auto`, `min-content`, `max-content`, `fit-content()`, `N fr`, `px`, `%`, `repeat()` and `minmax()`. Explicit placement, spans (`span N`), row/column auto-flow and track stretching are supported. Track lists accept whitespace-separated values such as `100px 1fr auto` and `repeat(2, 1fr)`.

### Animation

- `animation`, `animation-name`, `animation-duration`, `animation-delay`, `animation-timing-function`, `animation-iteration-count`, `animation-direction`, `animation-fill-mode`, `animation-play-state`
- `@keyframes` and `@-webkit-keyframes`

Animatable properties: `transform`, `opacity`, `color`, `background-color`, `text-shadow` and numeric/length/colour custom properties. Timing functions: `linear`, `ease`, `ease-in`, `ease-out`, `ease-in-out`, `steps()` and `cubic-bezier()`.

Multiple animations per node are supported. Shadow lists of different lengths are interpolated by repeating the shorter list, matching CSS. Animating `text-shadow` refreshes the offscreen shadow layer without repainting the text overlay.

`transition`, `transition-property`, `transition-duration`, `transition-delay` and `transition-timing-function` are applied when the scene styles change (hot reload of `config.py`/`styles.css`). Transitionable fields are `opacity`, `transform`, `color`, `background-color`, `text-shadow` and custom properties; `transition: all` covers them. Animations take precedence over transitions for the same field. Layout properties (`width`, `height`, `margin`, `padding`, `gap`, `top`, `left`, ...) do not transition yet, and newly created/removed nodes start at their target values.

Example:

```css
.panel {
  transition: background-color 300ms ease, opacity 200ms linear;
}
```

## Not implemented

This is the complete list of known unsupported or partially supported behavior. Unknown properties are also reported by `widdgyy --check-css`.

### Interaction and selectors

- All interaction: hover, focus, active, pointer, cursor, keyboard, clicks. The wallpaper is display-only.
- Sibling combinators `+` and `~` (parsed but never match).
- `:has()`, `:first-of-type`, `:last-of-type`, `:nth-of-type()`, `:empty`, `:target` and form pseudo-classes.
- Pseudo-elements other than `::before`/`::after`: `::marker`, `::first-line`, `::first-letter`, `::selection`, `::placeholder`, etc.
- `content` features beyond quoted strings and `none`/`normal`: `attr()`, counters, quote keywords, `content: url()` and images.

### Layout

- CSS tables (`display: table*`, `border-collapse`, `border-spacing`, table layout).
- Floats (`float`, `clear`), multi-column layout (`columns`, `column-*`).
- `display: contents`, `display: flow-root` and other display values beyond the list above.
- `position: sticky` (parsed as static).
- `justify-items`, `justify-self`, `place-items`, `place-content`, `place-self`.
- `grid-area`, `grid-template-areas`, `grid-auto-columns`, `grid-auto-rows`, `grid-template`/`grid` shorthands (accepted but ignored).
- True `min-content`/`max-content` intrinsic sizing in flex; `flex-basis` as the flex base; margin collapsing.
- Scrolling behavior for `overflow: scroll`/`auto` (content is clipped, not scrollable).
- Logical properties beyond the width/height/margin/padding/inset aliases listed above.

### Backgrounds and borders

- Multiple `background-blend-mode` values (only the first value is used).
- `background-clip: text` (painted as `content-box`).
- `background-attachment` (parsed, no effect).
- Border styles `double`, `groove`, `ridge`, `inset`, `outset` (painted as solid).
- Elliptical `border-radius` (the part after `/` is ignored).
- `border-image` sources other than `url()` (for example gradients) and `border-image-repeat` beyond the documented four values.
- `outline-style` variants (painted solid).

### Text

- `font-variation-settings` (accepted, ignored).
- Synthetic small caps: `font-variant-caps` needs a font that ships the OpenType feature.
- True inline formatting for `vertical-align` (the engine has no inline formatting context; only block alignment inside the text box is approximated).
- `text-indent` changing the wrap width of the first line (the offset is applied, but wrapping still uses the full width).
- Full bidi base-direction override for LTR text under `direction: rtl` (glyphs are shaped from the text's own bidi runs).
- `hyphens`, `writing-mode`, `text-orientation`, `unicode-bidi`, `text-emphasis`, `initial-letter`.
- Font synthesis: no artificial bold/italic faces; the closest real face is used.

### Effects, filters and masks

- Spatial filters below `mask-image` or `mix-blend-mode`.
- Group-level blending across offscreen layer boundaries.
- `mask-composite` operators other than the initial additive composition.
- `mask` and `-webkit-mask` shorthands.
- SVG external masks and `clip-path: url(...)` (not fetched).
- `filter: url(...)` and reference filters.
- `backdrop-filter` outside the video fast path, and backdrop functions other than `blur()`.
- Group-nesting effects beyond 8 levels (composited flat with a warning).

### Animation and transitions

- Layout-affecting transitions (`width`, `height`, `margin`, `padding`, `gap`, `top`, `left`, ...) and layout-affecting keyframe animations.
- Animating properties other than `transform`, `opacity`, `color`, `background-color`, `text-shadow` and custom properties.
- Scroll-driven animations, view transitions, `@starting-style`, `animation-timeline`.

### At-rules and media queries

- `@import`, `@charset`, `@namespace`, `@page`, `@container`, `@scope`, `@property`, `@counter-style`, `@font-palette-values`, `@view-transition`.
- Cascade layers: `@layer` blocks are parsed but flattened, so layer order is ignored.
- Media features beyond the supported set: `scripting`, `prefers-contrast`, `prefers-reduced-transparency`, `forced-colors`, `update`, `overflow-block`, etc.
- Automatic preference changes without a reload (the system theme is probed on load/reload).
- Fractional output-scale detection: `resolution` uses `Viewport::scale_factor`, currently 1.0.

### Values and functions

- `env()`, `attr()`, `url()` for filters/masks beyond local images, and `image-set()`.
- `subgrid`, `masonry`, container query units (`cqw`, `cqh`, ...).
- Viewport units other than `vw`, `vh`, `vmin`, `vmax`.
- `@font-face` formats other than local file URLs.

## Painting model and known limits

The renderer is a browser-like painter, not a DOM engine attempt at full CSS conformance. These are the architectural choices worth knowing:

### Offscreen filter layers

- Nodes with `filter: blur()`/`drop-shadow()`, or with `opacity`/`clip-path`/`overflow: hidden`/`isolation` over a filtered subtree, become recursive offscreen groups.
- Each group gets an accumulation texture at its nesting depth. Child groups are composited recursively, then the group's filter, clip mask and opacity are applied.
- Group nesting is capped at 8. Accumulation textures are allocated lazily, so simple scenes do not pay for unused levels.
- A clip mask is rasterized in screen coordinates and applied after the filter, before opacity, matching the CSS order.
- Vello still handles ordinary `opacity`, `clip-path`, `isolation` and masks when no nested filter needs splitting.

### Masks

- Masks are painted into a single Vello luminance mask layer for the node and its subtree.
- Multiple `mask-image` layers are accumulated additively inside that layer, equivalent to `mask-composite: add`.
- `mask-mode: alpha` with an opaque URL image uses a cached white RGB copy with the source alpha, so the result is the image alpha rather than its luminance.

### Border images

- The nine regions are scaled according to `border-image-width`, then tiled according to `border-image-repeat`.
- Corners are stretched; edges use horizontal/vertical repeat rules; the center is drawn only with `fill`.
- `repeat` centers the pattern and clips partial tiles; `round` rescales to a whole number; `space` discards partials and distributes equal gaps before, between and after tiles.
- Tiling is capped at 512 tiles per axis to bound work.

### Backdrop and video

- With a background video and no spatial filters, the fast path draws the video as a texture and caches the static overlay.
- `backdrop-filter: blur()` renders the video offscreen at 35% resolution with two blur iterations and blends it through the panel coverage mask.
- A scene with spatial filters uses the full Vello layer path instead; `backdrop-filter` is ignored there.
- Without a video wallpaper there is no backdrop to sample on a transparent layer surface, so `backdrop-filter` cannot work without compositor support.

## Conformance harness

The repository includes a Firefox-based layout harness:

```bash
./scripts/css-conformance.sh                 # all fixtures
./scripts/css-conformance.sh text-basic      # one fixture
```

It starts Firefox headless with a temporary profile, speaks the Marionette protocol directly (no geckodriver or Python packages), reads `getBoundingClientRect()` and `Range.getClientRects()` for elements marked with `data-test`/`data-lines`, and compares them with `widdgyy --dump-layout`. If Firefox is unavailable the fixtures are skipped cleanly.

Fixtures live in `tests/conformance/<name>/`:

```
config.py       widdgyy scene
styles.css      widdgyy styles
index.html      equivalent browser document
fixture.json    optional {"width", "height", "tolerance"}
```

The command used for a fixture is:

```bash
widdgyy --dump-layout --config tests/conformance/text-basic/config.py --viewport 800x526
```

`--dump-layout` prints JSON with every node's border box, content box, id/classes/tag, generated pseudo-element marker and text line rectangles. Layout comparisons use a tolerance in pixels (default 2 px, configurable per fixture); text widths normally sit within 1–3 px because Firefox and cosmic-text may select slightly different faces or apply different kerning.

## Hot reload

Widdgyy watches the config directory. Writing `config.py`, `config.jsonc` or `styles.css` triggers a debounced reload:

- the new scene replaces the old one;
- styles are re-resolved and layout is recomputed;
- Python configs are executed again (with the same 3 second timeout);
- unused media assets are pruned and their GStreamer pipelines are dropped;
- in-flight API requests from the old scene are discarded.

If the new config cannot be parsed, the previous scene is kept and the error is logged.

## Debugging

```bash
RUST_LOG=widdgyy=debug ./target/release/widdgyy
widdgyy --check-css
widdgyy --dump-layout --viewport 1280x720
./scripts/perf-sample.sh 10
```

`--check-css` prints the active config and stylesheet, counts stickers/rules/keyframes/fonts and lists:

- CSS errors,
- unsupported properties,
- unsupported at-rules.

Useful log messages: missing fonts, failed media loads, unsupported CSS properties, API failures, watcher errors, Python config failures and filter/mask fallback notices.

## Quick start

```bash
mkdir -p ~/.config/widdgyy
cp examples/config.py ~/.config/widdgyy/
cp examples/styles.css ~/.config/widdgyy/
./target/release/widdgyy
```

For the legacy format:

```bash
cp examples/config.jsonc ~/.config/widdgyy/
```
