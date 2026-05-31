from pathlib import Path


def user_config_dir() -> Path:
	return Path.home() / ".config" / "widdgyy"
