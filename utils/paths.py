# SPDX-FileCopyrightText: 2026 Daniel
# SPDX-License-Identifier: GPL-3.0-or-later

from pathlib import Path


def user_config_dir() -> Path:
	return Path.home() / ".config" / "widdgyy"
