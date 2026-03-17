#!/usr/bin/env bash
set -euo pipefail

REPO="trouze/dbtp"
BINARY="dbtp"
INSTALL_DIR="${DBTP_INSTALL_DIR:-$HOME/.local/bin}"

get_latest_version() {
  curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" \
    | grep '"tag_name"' \
    | sed -E 's/.*"tag_name": *"([^"]+)".*/\1/'
}

detect_target() {
  local os arch
  os="$(uname -s)"
  arch="$(uname -m)"

  case "${os}" in
    Linux)  os="unknown-linux-gnu" ;;
    Darwin) os="apple-darwin" ;;
    *)      echo "Unsupported OS: ${os}" >&2; exit 1 ;;
  esac

  case "${arch}" in
    x86_64|amd64) arch="x86_64" ;;
    arm64|aarch64) arch="aarch64" ;;
    *)             echo "Unsupported architecture: ${arch}" >&2; exit 1 ;;
  esac

  echo "${arch}-${os}"
}

main() {
  local version target url tmpdir

  version="${1:-$(get_latest_version)}"
  target="$(detect_target)"
  url="https://github.com/${REPO}/releases/download/${version}/${BINARY}-${version}-${target}.tar.gz"

  echo "Installing ${BINARY} ${version} for ${target}..."

  tmpdir="$(mktemp -d)"
  trap 'rm -rf "${tmpdir}"' EXIT

  curl -fsSL "${url}" | tar xz -C "${tmpdir}"

  mkdir -p "${INSTALL_DIR}"
  mv "${tmpdir}/${BINARY}" "${INSTALL_DIR}/${BINARY}"
  chmod +x "${INSTALL_DIR}/${BINARY}"

  echo "Installed ${BINARY} to ${INSTALL_DIR}/${BINARY}"

  case ":${PATH}:" in
    *":${INSTALL_DIR}:"*) ;;
    *)
      echo ""
      echo "NOTE: ${INSTALL_DIR} is not in your PATH."
      echo "Add it by running:"
      echo "  export PATH=\"${INSTALL_DIR}:\$PATH\""
      echo "Or add that line to your ~/.zshrc / ~/.bashrc."
      ;;
  esac

  "${INSTALL_DIR}/${BINARY}" --version
}

main "$@"
