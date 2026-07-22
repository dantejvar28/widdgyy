# Widdgyy Configuration & Styling Guide

This document explains how to configure Widdgyy using `config.jsonc` and `styles.css`, where these files should be stored, and which CSS properties are currently supported.

## Configuration Files Location

Widdgyy looks for the following files:

`~/.config/widdgyy/config.jsonc`

`~/.config/widdgyy/styles.css`

If the directory does not exist, create it before running the application:

```bash
mkdir -p ~/.config/widdgyy
```

---

## Creating the Configuration Files

1. Create the `~/.config/widdgyy` directory.
2. Create `config.jsonc` to describe your stickers and layouts.
3. Create `styles.css` to define styles for each class.
4. Make sure the `class` value of every sticker matches a CSS class defined in `styles.css`.

`config.jsonc` uses `commentjson`, so comments are supported.

---

## Basic `config.jsonc` Structure

The configuration file must contain a top-level `stickers` key with an array of sticker objects.

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

---

## Available Sticker Types

### `text`

Main fields:

- `text`
- `font_size` or `font-size`
- `class`
- `anchor`
- `offset_x` or `offset-x`
- `offset_y` or `offset-y`
- `margin`, `margin_top`, `margin_right`, `margin_bottom`, `margin_left`

### `clock`

Main fields:

- `format` (default: `%H:%M:%S`)
- `update_interval`
- `font_size` or `font-size`
- All common positioning and margin fields

`clock` inherits from `text`, so it also supports text styling.

### `api_text`

Main fields:

- `url`
- `method`
- `headers`
- `template`
- `fields`
- `timeout`
- `retries`
- `update_interval`
- `font_size` or `font-size`
- All common positioning and margin fields

### `media`

Main fields:

- `path`
- `width`
- `height`
- `fit` (`contain`, `cover`, or `stretch`)
- `autoplay`
- `loop`
- `z_index`
- `class`

**Notes:**

- `image` is deprecated; use `media` instead.
- `width` and `height` support both `px` and `%`.
- If only one dimension is specified, the other one attempts to preserve the original aspect ratio.

### `hbox`

Main fields:

- `children`
- `spacing`
- `padding` or `padding_top` / `padding_right` / `padding_bottom` / `padding_left`
- `justify` (`start`, `center`, `end`)
- `align_items` (`start`, `center`, `end`)
- `anchor`, `offset_x`, `offset_y`

### `vbox`

Main fields:

- `children`
- `spacing`
- `padding` or `padding_*`
- `justify` (`start`, `center`, `end`)
- `align_items` (`start`, `center`, `end`)
- `anchor`, `offset_x`, `offset_y`

### `grid`

Main fields:

- `children`
- `columns`
- `spacing`
- `padding`
- `anchor`, `offset_x`, `offset_y`

---

## Example `config.jsonc`

```jsonc
{
  "stickers": [
    {
      "type": "text",
      "text": "Widdgyy",
      "class": "title",
      "font_size": 42,
      "anchor": "top-left",
      "offset_x": 40,
      "offset_y": 32
    },
    {
      "type": "clock",
      "class": "clock",
      "format": "%H:%M",
      "font_size": 28,
      "anchor": "top-left",
      "offset_x": 40,
      "offset_y": 90
    },
    {
      "type": "media",
      "class": "media-card",
      "path": "~/Pictures/wallpaper.png",
      "width": "30%",
      "height": "30%",
      "fit": "cover",
      "anchor": "center"
    },
    {
      "type": "hbox",
      "class": "panel",
      "anchor": "bottom-left",
      "offset_x": 32,
      "offset_y": -32,
      "padding": 16,
      "spacing": 12,
      "justify": "start",
      "align_items": "center",
      "children": [
        {
          "type": "text",
          "text": "CPU",
          "class": "panel-label"
        },
        {
          "type": "text",
          "text": "42%",
          "class": "panel-value"
        }
      ]
    }
  ]
}
```

---

## Writing `styles.css`

Styles are defined by class using a simple CSS-like syntax:

```css
.title {
  color: 255, 255, 255, 1;
  font-size: 42;
  font-family: Sans;
  font-weight: bold;
  text-shadow: 0 2px 8px rgba(0, 0, 0, 0.65);
}
```

### Important Rules

- Each block begins with `.class-name { ... }`.
- Properties are separated with `;`.
- The parser normalizes `-` into `_`, so `background-color` and `background_color` become the same internal key.
- In practice, you can use standard hyphenated CSS property names.
- Text color currently accepts only comma-separated numeric values, for example `255, 255, 255, 1` or `1, 1, 1, 1`.
- A future parser will be added to support hexadecimal colors and `rgba()` syntax.

---

## Currently Supported CSS Properties

### Box & Background

- `background-color` or `background`
- `border-color`
- `border-width`
- `border-radius`
- `box-shadow`

### Text

- `color`
- `font-family`
- `font-size`
- `font-weight`
- `font-style`
- `text-align`
- `text-wrap`
- `text-overflow`
- `text-shadow`

### Text or Media Sizing

- `width`
- `max-width`

**Note:** text color parsing is still numeric-only for now; hex and `rgba()` support will be added later.

---

## Example `styles.css`

```css
.title {
  color: 255, 255, 255, 1;
  font-family: Sans;
  font-size: 42;
  font-weight: bold;
  text-shadow: 0 3px 10px rgba(0,0,0,0.6);
}

.clock {
  color: 255, 221, 136, 1;
  font-family: Sans;
  font-size: 28;
}

.media-card {
  background-color: rgba(20, 20, 20, 0.35);
  border-radius: 18;
  border-width: 2;
  border-color: rgba(255,255,255,0.12);
  box-shadow: 0 10px 30px rgba(0,0,0,0.25);
}

.panel {
  background-color: rgba(15, 15, 20, 0.55);
  border-radius: 16;
  border-width: 1;
  border-color: rgba(255,255,255,0.10);
}

.panel-label {
  color: 200, 200, 200, 1;
  font-size: 18;
}

.panel-value {
  color: 255, 255, 255, 1;
  font-size: 18;
  font-weight: bold;
}
```

---

## How Styles Are Applied

The styling process works as follows:

1. `config.jsonc` creates each sticker.
2. Each sticker receives its `class` value.
3. `styles.css` defines a style block for that class.
4. On startup and during hot reload, Widdgyy loads the stylesheet and applies it to every sticker.

---

## Practical Tips

- If a style is not applied, make sure the sticker's `class` exactly matches the CSS class name.
- When using `media`, define either `width` or `height` using `px` or `%` to control its size.
- For text stickers, you can define `font_size` in the configuration as a default value, then override it in CSS if you prefer to centralize styling.
- If you're migrating from an older configuration, replace `type: "image"` with `type: "media"`.

---

## Quick Start

```bash
mkdir -p ~/.config/widdgyy
cp config.jsonc ~/.config/widdgyy/config.jsonc
cp styles.css ~/.config/widdgyy/styles.css
python3 /home/daniel/Documentos/widdgyy/main.py
```
