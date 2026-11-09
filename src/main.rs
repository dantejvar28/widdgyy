// SPDX-FileCopyrightText: 2026 Daniel
// SPDX-License-Identifier: GPL-3.0-or-later

use std::path::PathBuf;

fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("widdgyy=info,warn"),
    )
    .init();

    let mut config_path = None;
    let mut check_css = false;
    let mut dump_layout = false;
    let mut viewport = widdgyy::css::Viewport::default();
    let mut migrate = false;
    let mut migrate_source = None;
    let mut args = std::env::args().skip(1).peekable();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--config" | "-c" => {
                config_path =
                    Some(PathBuf::from(args.next().ok_or_else(|| {
                        anyhow::anyhow!("--config requires a path")
                    })?));
            }
            "--check-css" => check_css = true,
            "--dump-layout" => dump_layout = true,
            "--viewport" => {
                let value = args
                    .next()
                    .ok_or_else(|| anyhow::anyhow!("--viewport requires WIDTHxHEIGHT"))?;
                let (width, height) = value.split_once(['x', 'X']).ok_or_else(|| {
                    anyhow::anyhow!("invalid viewport {value:?}; expected WIDTHxHEIGHT")
                })?;
                viewport.width = width.parse()?;
                viewport.height = height.parse()?;
            }
            "--migrate-config" => {
                migrate = true;
                if args
                    .peek()
                    .is_some_and(|argument| !argument.starts_with('-'))
                {
                    migrate_source = args.next().map(PathBuf::from);
                }
            }
            "--help" | "-h" => {
                println!(
                    "widdgyy {}\n\nUsage:\n  widdgyy [--config PATH]\n  widdgyy --check-css [--config PATH]\n  widdgyy --dump-layout [--config PATH] [--viewport WxH]\n  widdgyy --migrate-config [SOURCE]\n\nDefault config: ~/.config/widdgyy/config.py,\nfalling back to ~/.config/widdgyy/config.jsonc",
                    env!("CARGO_PKG_VERSION")
                );
                return Ok(());
            }
            other => anyhow::bail!("unknown argument: {other} (try --help)"),
        }
    }

    if migrate {
        let source = migrate_source
            .or(config_path)
            .unwrap_or_else(|| widdgyy::user_config_dir().join("config.jsonc"));
        let text = std::fs::read_to_string(&source)
            .map_err(|error| anyhow::anyhow!("reading {}: {error}", source.display()))?;
        let value: serde_json::Value = json5::from_str(&text)
            .map_err(|error| anyhow::anyhow!("parsing {}: {error}", source.display()))?;
        print!("{}", widdgyy::config::migrate::to_python(&value)?);
        return Ok(());
    }

    if dump_layout {
        return widdgyy::conformance::dump_layout(config_path, viewport);
    }

    if check_css {
        return widdgyy::diagnostics::run(config_path);
    }

    widdgyy::platform::run(config_path)
}
