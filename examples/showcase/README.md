# Showcase scenes

Four self-contained scenes used for the screenshots and GIFs in the project
README. They are intentionally small: each one is a single `config.py` plus a
`styles.css`. Bonsai and Night Train ship their own local media (`tree.png`
and an 8 s 1080p clip) so the scenes work in a fresh clone.

| Scene | What it demonstrates |
| --- | --- |
| [`aurora`](aurora) | No-video scene made of CSS grid areas, layered gradients, generated `::before`/`::after` decoration, and a drifting transform animation. |
| [`bonsai`](bonsai) | Transparent PNG wallpaper, paper-like background gradients, serif editorial type, named grid areas, `place-self` alignment, and a sway animation. |
| [`night-train`](night-train) | 4K video wallpaper through the fast video path, cinematic overlay, text shadows, and a looping glow/pulse. |
| [`neon`](neon) | Type-only cyberpunk HUD: nested grids, `repeating-linear-gradient` scanlines, opacity/transform keyframes, and `::after` data bars. |

Run any scene from the repository root:

```bash
./target/release/widdgyy --config examples/showcase/aurora/config.py
./target/release/widdgyy --config examples/showcase/bonsai/config.py
./target/release/widdgyy --config examples/showcase/night-train/config.py
./target/release/widdgyy --config examples/showcase/neon/config.py
```

The screenshots were captured at 1920x1080 on Hyprland. The GIFs are 900x506
at 10 fps; use them as a starting point, not as a fixed art direction.
