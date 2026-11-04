# Contributing to Widdgyy

Thanks for your interest in contributing.

## Quick Start

1. Fork the repository and clone your fork.
2. Create a branch from `develop`:

```bash
git checkout -b feat/my-change
```

3. Install Rust 1.89+ and the system dependencies listed in the README.
4. Build and run:

```bash
cargo build
./target/debug/widdgyy
```

5. Optional: install the local binary:

```bash
./scripts/install-local-cli.sh
widdgyy
```

## Development Workflow

- Keep PRs focused and small.
- Write clear commit messages.
- Update docs when behavior changes.
- Add tests for bug fixes and new features.

## Code Style

- Run `cargo fmt` before committing.
- Run `cargo clippy --all-targets -- -D warnings` when possible.
- Keep the CSS layer typed: parse once, resolve at cascade time, never parse property strings inside a painter.
- Keep style families in their own modules (`css/properties/*`, `render/*_painter.rs`) instead of adding long `if property == ...` chains.
- Treat media/assets lifecycle explicitly: caches must stay bounded and `Drop` must release GPU/media resources.

## Tests and Checks

```bash
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
```

`tests/offscreen_render.rs` renders a scene to a wgpu texture and reads the pixels back. It skips the GPU assertion when no adapter is available, so it still works in headless CI.

## Pull Requests

- Open a PR against `develop`.
- Fill in the PR template completely.
- Link related issues.
- Include screenshots or short videos for UI changes.
- Mention which CSS properties or sticker types are affected.

## Reporting Bugs

Use the bug report issue template and include:

- Linux distribution, compositor and GPU driver
- Rust version (`rustc --version`)
- Widdgyy version / commit
- Whether the issue happens with the sample config or a custom one
- Steps to reproduce
- Expected vs actual behavior
- Relevant logs (`RUST_LOG=widdgyy=debug`)

## License

By contributing, you agree that your contributions are licensed under GPL-3.0-or-later.
