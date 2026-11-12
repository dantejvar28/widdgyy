# Changelog

All notable changes to Widdgyy are documented here.

The project follows [Semantic Versioning](https://semver.org/). Before `1.0.0`,
minor versions may contain breaking changes to the config schema, CSS surface,
CLI and internal APIs.

## [Unreleased]

### Planned

- Finish B8: static/dynamic overlay caching, dynamic shadow-slot count and
  per-pass GPU frame timing.
- Layout-affecting transitions (`width`, `height`, `margin`, `padding`, `gap`,
  `top`, `left`, ...) and empty/non-empty `text-shadow` transitions.
- Fine B3 text limits: `font-variation-settings`, synthetic small caps,
  first-line wrapping for `text-indent` and a true inline formatting context.
- Fine B7 limits: live desktop-theme changes, fractional output scale and
  `prefers-contrast`/`forced-colors`/`scripting`.

## [0.2.0-alpha.2] - 2026-11-12

Layout-focused alpha: the B1 layout surface is complete, and four showcase
scenes ship with the README.

### Added

- Block formatting contexts: vertical child stacking, parent/child and sibling
  margin collapsing (including negative margins), block-level auto-width fill
  and auto horizontal margin centering.
- Real `min-content`, `max-content` and `fit-content()` sizing for text,
  media, flex items and grid tracks, including intrinsic minimum sizes for
  flex shrinking.
- `flex-basis` as the flex base size, with grow/shrink, automatic minimum
  clamps and stretched cross sizes resolved from that base.
- Grid `grid-template-areas`/`grid-area`, `grid-auto-columns`,
  `grid-auto-rows`, sparse/dense row and column auto-flow, `minmax()` floors,
  intrinsic track sizing and `justify-items`/`justify-self`/`place-*`.
- B1 Firefox conformance fixtures for block margins, flex bases and named grid
  areas.
- Four showcase scenes (`examples/showcase/`) with README screenshots and
  animated previews: Aurora Grid, Bonsai Editorial, Night Train and Neon
  Terminal.

### Changed

- The layout engine keeps the custom block/flex/grid implementation; Taffy
  was evaluated and rejected for this release.
- Text intrinsic block size now uses the natural text height rather than the
  one-word-per-line narrow-shape height, so auto-height flex columns do not
  expand when API/clock text changes.

### Fixed

- Removed leftover shadow-pass debug logging from the renderer.

## [0.2.0-alpha.1] - 2026-11-09

First tagged alpha of the Rust/Vello engine.

### Added

- Python scene DSL (`config.py`) with `Text`, `Clock`, `ApiText`, `Video`,
  `Image`, `Gif`, `HBox`, `VBox`, `Grid` and `--migrate-config` for legacy
  JSONC scenes.
- Browser-like CSS engine: selectors, cascade, `!important`, variables,
  modern colors/`color-mix`, `calc()`/`min()`/`max()`/`clamp()`, media queries,
  flex, grid, gradients, shadows, borders, transforms and clipping.
- Generated `::before`/`::after` boxes with string `content`, `border-radius:
  inherit` and absolute positioning.
- Recursive offscreen layer graph for nested `filter: blur()`/`drop-shadow()`
  under `opacity`, `clip-path`, `overflow: hidden`, `isolation` and filtered
  subtrees.
- `backdrop-filter` on the video fast path, multi-layer masks with
  `mask-mode`, and `border-image` with source/slice/width/outset/repeat and
  the shorthand.
- Text features: `line-clamp`, `text-indent`, `text-align-last`,
  direction-aware alignment, `font-stretch`, `font-feature-settings`,
  `font-variant-caps` and approximate block `vertical-align`.
- CSS transitions on style changes (hot reload) for opacity, transform,
  colors, text-shadow and custom properties.
- System media preferences: `prefers-color-scheme`, `prefers-reduced-motion`,
  `resolution`, `aspect-ratio`, `hover`/`pointer` and `display-mode`, with
  environment overrides.
- Firefox headless conformance harness without geckodriver, using Marionette
  to compare layout boxes and text line rectangles.
- GPU shadow blur cache/quantization, scissored shadow/backdrop passes and a
  reduced-resolution backdrop path.
- `--dump-layout` JSON layout dump and `scripts/perf-sample.sh` performance
  sampling.

### Changed

- The scene config is preferably `config.py`; `config.jsonc` remains supported
  as a legacy fallback.
- The project is display-only: no hover, clicks, pointer, cursor or keyboard
  handling, and the Wayland surface uses an empty input region.
- Shadow sigma is quantized to 0.5 px and blur slots are reused until the
  sigma or glyph mask changes.
- The video backdrop is rendered at 35% resolution with two blur iterations;
  overlay and shadow blurs keep four.

### Fixed

- Gradient stops now pad at 0%/100% and hard stops work around Vello's ramp
  builder.
- Shadow output scissors are scaled to the full-resolution shadow slots, so
  animated glow is no longer clipped.
- `pointer-events` is accepted as a no-op and `::before`/`::after` no longer
  produce pseudo-class warnings.

### Known limitations

See [Configuration & Styling Guide § Not implemented](docs/Configuration%20%26%20Styling%20Guide.md#not-implemented).
Highlights: layout transitions, `mask-composite`, `border-image` non-URL
sources, nested filters below `mask-image`/`mix-blend-mode`, `backdrop-filter`
outside the video path, live theme subscriptions, and the alpha-level testing
coverage described in the README.
