// SPDX-FileCopyrightText: 2026 Daniel
// SPDX-License-Identifier: GPL-3.0-or-later

use std::path::PathBuf;

fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("widdgyy=info,warn"),
    )
    .init();

    let mut config_path = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--config" | "-c" => {
                config_path =
                    Some(PathBuf::from(args.next().ok_or_else(|| {
                        anyhow::anyhow!("--config requires a path")
                    })?));
            }
            "--help" | "-h" => {
                println!(
                    "widdgyy {}\n\nUsage: widdgyy [--config PATH]\n\nDefault config: ~/.config/widdgyy/config.jsonc",
                    env!("CARGO_PKG_VERSION")
                );
                return Ok(());
            }
            other => anyhow::bail!("unknown argument: {other} (try --help)"),
        }
    }

    widdgyy::platform::run(config_path)
}
