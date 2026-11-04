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
  local    Build the Rust binary and install it at ~/.local/bin/widdgyy (default)
  package  Build and install the AUR package from packaging/aur

Examples:
  ./scripts/install-local-cli.sh
  ./scripts/install-local-cli.sh local
  ./scripts/install-local-cli.sh package
  ./scripts/install-local-cli.sh package --noconfirm
EOF
}

install_local_launcher() {
  echo "Building widdgyy (release)..."
  (cd "${ROOT_DIR}" && cargo build --release --locked)

  install -dm755 "${INSTALL_DIR}"
  install -Dm755 "${ROOT_DIR}/target/release/widdgyy" "${INSTALL_DIR}/widdgyy"

  echo "Installed: ${INSTALL_DIR}/widdgyy"

  case ":${PATH}:" in
    *":${INSTALL_DIR}:"*)
      echo "${INSTALL_DIR} is already in PATH."
      ;;
    *)
      echo "Add this to your shell config:"
      echo "  fish_add_path ${INSTALL_DIR}"
      echo "  # or: export PATH=\"${INSTALL_DIR}:\$PATH\""
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

  local build_dir="${XDG_CACHE_HOME:-${HOME}/.cache}/widdgyy-makepkg"
  rm -rf "${build_dir}"
  mkdir -p "${build_dir}"

  local version
  version="$(sed -n 's/^version = "\(.*\)"/\1/p' "${ROOT_DIR}/Cargo.toml" | head -1)"
  version="${version:-0.0.0}"

  echo "Creating local package from the working tree..."
  tar -C "${ROOT_DIR}" \
    --exclude=./target \
    --exclude=./.git \
    --exclude=./packaging/aur/src \
    --exclude=./packaging/aur/pkg \
    --exclude=./packaging/aur/widdgyy-git \
    -czf "${build_dir}/widdgyy-local.tar.gz" .

  cat > "${build_dir}/PKGBUILD" <<'PKGEOF'
pkgname=widdgyy
pkgver=0.0.0
pkgrel=1
pkgdesc="GPU-accelerated Wayland layer-shell desktop ricing engine (local build)"
arch=('x86_64')
url="https://github.com/dantejvar28/widdgyy"
license=('GPL-3.0-or-later')
depends=(
  'gstreamer'
  'gst-plugins-base'
  'gst-plugins-good'
  'wayland'
  'vulkan-icd-loader'
)
optdepends=(
  'gst-plugins-bad: extra video codecs'
  'gst-libav: H.264/H.265 decoding'
)
makedepends=('cargo' 'rust')
conflicts=('widdgyy-git' 'widdgyy-local')
replaces=('widdgyy-git')
source=('widdgyy-local.tar.gz')
sha256sums=('SKIP')

build() {
  unset CFLAGS CXXFLAGS LDFLAGS RUSTFLAGS
  export CARGO_TARGET_DIR="${srcdir}/../target"
  cd "${srcdir}"
  cargo build --release --locked
}

package() {
  cd "${srcdir}"
  install -Dm755 "${srcdir}/../target/release/widdgyy" "${pkgdir}/usr/bin/widdgyy"
  install -dm755 "${pkgdir}/usr/share/widdgyy"
  if [[ -d assets ]]; then
    cp -r assets "${pkgdir}/usr/share/widdgyy/"
  fi
  install -m644 README.md "${pkgdir}/usr/share/widdgyy/README.md"
  install -m644 "docs/Configuration & Styling Guide.md" \
    "${pkgdir}/usr/share/widdgyy/Configuration & Styling Guide.md"
  install -Dm644 LICENSE "${pkgdir}/usr/share/licenses/widdgyy/LICENSE"
}
PKGEOF

  sed -i "s/^pkgver=.*/pkgver=${version}/" "${build_dir}/PKGBUILD"

  echo "Package build directory: ${build_dir}"
  echo "Installing to /usr/bin/widdgyy via pacman..."
  cd "${build_dir}"
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
