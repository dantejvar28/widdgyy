# Widdgyy

Python project with a GTK interface to render text and image stickers.

## License

GPL-3.0-or-later. See `LICENSE`.

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

## Local terminal command

To install a user-level `widdgyy` command (without sudo):

```bash
./scripts/install-local-cli.sh
```

After that, run:

```bash
widdgyy
```

### Publish to AUR

1. Create a dedicated AUR git repository for `widdgyy-git`.
2. Copy `PKGBUILD` and `.SRCINFO` from `packaging/aur/` to that AUR repo root.
3. Regenerate `.SRCINFO` in the AUR repo:

```bash
makepkg --printsrcinfo > .SRCINFO
```

4. Commit and push to AUR.
