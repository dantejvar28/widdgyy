#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd -- "${SCRIPT_DIR}/.." && pwd)"
INSTALL_DIR="${HOME}/.local/bin"

install -dm755 "${INSTALL_DIR}"
install -m755 "${ROOT_DIR}/scripts/widdgyy" "${INSTALL_DIR}/widdgyy"

echo "Installed: ${INSTALL_DIR}/widdgyy"

case ":${PATH}:" in
  *":${INSTALL_DIR}:"*)
    echo "${INSTALL_DIR} is already in PATH."
    ;;
  *)
    echo "Add this to your shell config (~/.config/fish/config.fish):"
    echo "  fish_add_path ${INSTALL_DIR}"
    ;;
esac
