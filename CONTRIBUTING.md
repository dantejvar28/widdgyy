# Contributing to Widdgyy

Thanks for your interest in contributing.

## Quick Start

1. Fork the repository and clone your fork.
2. Create a branch from `develop`:

```bash
git checkout -b feat/my-change
```

3. Install runtime and development dependencies (without `venv`):

```bash
python3 -m pip install --user -r requirements-dev.txt
```

4. Run the project:

```bash
python3 main.py
```

5. Optional: install the local launcher:

```bash
./scripts/install-local-cli.sh
widdgyy
```

## Development Workflow

- Keep PRs focused and small.
- Write clear commit messages.
- Update docs when behavior changes.
- Add tests for bug fixes and new features when possible.

## Code Style

- Python: follow PEP 8.
- Rust: run `cargo fmt` in `native/video/`.
- Keep comments concise and technical.
- Avoid unrelated refactors in the same PR.

## Tests and Checks

Run these checks before opening a PR:

```bash
python3 -m compileall .
```

For native video code:

```bash
cd native/video
cargo fmt --check
cargo clippy -- -D warnings
cargo test
```

## Pull Requests

- Open a PR against `develop`.
- Fill in the PR template completely.
- Link related issues.
- Include screenshots or short videos for UI changes.

## Reporting Bugs

Use the bug report issue template and include:

- Linux distro and desktop session
- Python version
- Steps to reproduce
- Expected vs actual behavior
- Relevant logs or tracebacks

## License

By contributing, you agree that your contributions are licensed under GPL-3.0-or-later.
