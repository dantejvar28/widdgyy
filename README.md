# Widdgyy

Widdgyy is an open-source desktop ricing engine for Linux that turns your wallpaper into a living workspace.

Create clocks, images, text, weather panels, system monitors, and API-powered stickers that are rendered directly on the desktop using GTK. Everything is configured with simple JSON and styled with CSS, making it easy to build highly customized layouts without writing a full desktop extension.

Unlike traditional widget tools, Widdgyy focuses on a modern, hackable workflow:

- JSON for layout
- CSS for styling
- Python for rapid widget development
- GTK 4 for native Wayland/Linux integration
# Why Python? 

Widdgyy was intentionally started in Python to optimize for experimentation and community contribution. The goal is to make creating a new widget feel as easy as writing a small Python class, not compiling an entire desktop application.

This allows contributors to prototype ideas quickly:

Spotify widgets
weather integrations
calendar panels
custom API dashboards
animated stickers
productivity overlays
Performance-first architecture

The long-term vision is a hybrid architecture: keep the widget ecosystem accessible in Python while moving performance-critical components (such as video rendering, animation, and low-level rendering pipelines) to Rust.

This approach aims to provide the best of both worlds:

Python → fast iteration, plugins, accessibility
Rust → low CPU usage, smooth rendering, memory safety
The goal

Widdgyy is not just a widget app.

It is an attempt to build a next-generation desktop ricing framework: lightweight, extensible, Wayland-friendly, and enjoyable for both artists and developers who want their desktop to be programmable, beautiful, and alive.

## License

GPL-3.0-or-later. See `LICENSE`.

## Contributing

- Read `CONTRIBUTING.md` for development setup and pull request workflow.
- Follow `CODE_OF_CONDUCT.md` for community standards.
- Use issue and pull request templates under `.github/`.

## Pip packages

Install in your environment:

pip install commentjson pycairo PyGObject

## System dependencies (Linux)

To make GI/GTK work correctly, also install:

sudo apt update
sudo apt install -y python3-gi gir1.2-gtk-4.0 gir1.2-gtk4-layer-shell-1.0

## Run

python3 main.py

## AUR packaging layout

This repository now includes an AUR-ready packaging scaffold in `packaging/aur/`:

- `packaging/aur/PKGBUILD`
- `packaging/aur/.SRCINFO`
- `packaging/aur/widdgyy.sh` (launcher installed as `/usr/bin/widdgyy`)

### Build locally with makepkg

From `packaging/aur/`:

```bash
makepkg -si
```

## Install helper script

The repository includes `scripts/install-local-cli.sh` with two install modes:

- Local launcher mode (default): installs `~/.local/bin/widdgyy` pointing to your current clone.
- Package mode: builds and installs from `packaging/aur/` using `makepkg -si`.

### 1) Local launcher (no package build)

```bash
./scripts/install-local-cli.sh
# or
./scripts/install-local-cli.sh local
```

This installs only a launcher in `~/.local/bin/widdgyy`. It does not copy the full repository into `~/.local/bin`.

Then run:

```bash
widdgyy
```

### 2) Build and install package (Arch/CachyOS)

```bash
./scripts/install-local-cli.sh package
```

This runs `makepkg -si` inside `packaging/aur/` and installs the built package on the machine.

### Publish to AUR

1. Create a dedicated AUR git repository for `widdgyy-git`.
2. Copy `PKGBUILD` and `.SRCINFO` from `packaging/aur/` to that AUR repo root.
3. Regenerate `.SRCINFO` in the AUR repo:

```bash
makepkg --printsrcinfo > .SRCINFO
```

4. Commit and push to AUR.
