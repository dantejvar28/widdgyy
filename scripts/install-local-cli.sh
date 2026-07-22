#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd -- "${SCRIPT_DIR}/.." && pwd)"
INSTALL_DIR="${HOME}/.local/bin"

usage() {
  cat <<'EOF'
Usage:
  ./scripts/install-local-cli.sh [local]
  ./scripts/install-local-cli.sh package [makepkg_args...]

Modes:
  local    Install user launcher at ~/.local/bin/widdgyy (default)
  package  Build and install package from packaging/aur with makepkg -si

Examples:
  ./scripts/install-local-cli.sh
  ./scripts/install-local-cli.sh local
  ./scripts/install-local-cli.sh package
  ./scripts/install-local-cli.sh package --noconfirm
EOF
}

install_local_launcher() {
  install -dm755 "${INSTALL_DIR}"

  # Install a launcher bound to this clone path so it works from ~/.local/bin.
  ROOT_DIR_ESCAPED="$(printf '%q' "${ROOT_DIR}")"
  cat > "${INSTALL_DIR}/widdgyy" <<EOF
#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR=${ROOT_DIR_ESCAPED}
ROOT_DIR="\${WIDDGYY_ROOT:-\${ROOT_DIR}}"

if [[ ! -f "\${ROOT_DIR}/main.py" ]]; then
  echo "widdgyy launcher error: main.py not found at \${ROOT_DIR}" >&2
  echo "Set WIDDGYY_ROOT to your clone path or reinstall from the right repository." >&2
  exit 1
fi

export PYTHONPATH="\${ROOT_DIR}\${PYTHONPATH:+:\${PYTHONPATH}}"
exec python3 "\${ROOT_DIR}/main.py" "\$@"
EOF

  chmod 755 "${INSTALL_DIR}/widdgyy"

  echo "Installed launcher: ${INSTALL_DIR}/widdgyy"

  case ":${PATH}:" in
    *":${INSTALL_DIR}:"*)
      echo "${INSTALL_DIR} is already in PATH."
      ;;
    *)
      echo "Add this to your shell config (~/.config/fish/config.fish):"
      echo "  fish_add_path ${INSTALL_DIR}"
      ;;
  esac
}

install_aur_package() {
  local -a makepkg_args
  makepkg_args=("$@")

  if ! command -v makepkg >/dev/null 2>&1; then
    echo "Error: makepkg not found. This mode requires an Arch-based system." >&2
    exit 1
  fi

  cd "${ROOT_DIR}/packaging/aur"
  echo "Building and installing package with makepkg -si ${makepkg_args[*]}"
  makepkg -si "${makepkg_args[@]}"
}

main() {
  local mode
  mode="${1:-local}"

  case "${mode}" in
    -h|--help|help)
      usage
      ;;
    local)
      if [[ $# -gt 1 ]]; then
        echo "Error: local mode does not accept extra arguments." >&2
        usage
        exit 1
      fi
      install_local_launcher
      ;;
    package)
      shift
      install_aur_package "$@"
      ;;
    *)
      echo "Error: unknown mode '${mode}'." >&2
      usage
      exit 1
      ;;
  esac
}

main "$@"
