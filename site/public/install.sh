#!/bin/sh
set -eu

fail() { printf 'xmeta installer: %s\n' "$*" >&2; exit 1; }

download() {
  if command -v curl >/dev/null 2>&1; then
    curl -fsSL --retry 2 --connect-timeout 15 "$1" -o "$2"
  elif command -v wget >/dev/null 2>&1; then
    wget -qO "$2" --timeout=30 --tries=2 "$1"
  else
    fail 'curl or wget is required'
  fi
}

sha256_file() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{print $1}'
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$1" | awk '{print $1}'
  elif command -v openssl >/dev/null 2>&1; then
    openssl dgst -sha256 "$1" | awk '{print $NF}'
  else
    fail 'sha256sum, shasum, or openssl is required'
  fi
}

cleanup() {
  [ -z "${temporary_target:-}" ] || rm -f "$temporary_target"
  [ -z "${temporary_directory:-}" ] || rm -rf "$temporary_directory"
}

main() {
  version=${XMETA_VERSION:-}
  install_dir=${XMETA_INSTALL_DIR:-"$HOME/.local/bin"}
  latest_url=${XMETA_LATEST_URL:-https://xmeta.eidos.space/cli/latest.txt}
  download_base=${XMETA_DOWNLOAD_BASE:-https://github.com/mayneyao/xmeta/releases/download}

  while [ "$#" -gt 0 ]; do
    case "$1" in
      --version) [ "$#" -ge 2 ] || fail '--version requires a value'; version=$2; shift 2 ;;
      --install-dir) [ "$#" -ge 2 ] || fail '--install-dir requires a value'; install_dir=$2; shift 2 ;;
      -h | --help)
        printf '%s\n' 'Install xmeta: install.sh [--version VERSION] [--install-dir DIRECTORY]' \
          'Defaults: latest stable version, ~/.local/bin. No administrator privileges required.'
        return ;;
      *) fail "unknown argument: $1" ;;
    esac
  done
  case "$install_dir" in /*) ;; *) fail 'install directory must be an absolute path' ;; esac

  kernel=$(uname -s)
  machine=$(uname -m)
  case "$kernel:$machine" in
    Darwin:arm64 | Darwin:aarch64) target=aarch64-apple-darwin ;;
    Darwin:x86_64 | Darwin:amd64) target=x86_64-apple-darwin ;;
    Linux:arm64 | Linux:aarch64) target=aarch64-unknown-linux-gnu ;;
    Linux:x86_64 | Linux:amd64) target=x86_64-unknown-linux-gnu ;;
    *) fail "unsupported platform: $kernel $machine (Windows: use install.ps1)" ;;
  esac

  temporary_directory=$(mktemp -d "${TMPDIR:-/tmp}/xmeta-install.XXXXXX")
  temporary_target=
  trap cleanup 0
  trap 'exit 1' HUP INT TERM
  if [ -z "$version" ]; then
    download "$latest_url" "$temporary_directory/latest.txt"
    version=$(tr -d '\r\n' < "$temporary_directory/latest.txt")
  fi
  version=${version#v}
  case "$version" in '' | *[!0-9A-Za-z.-]*) fail "invalid version: $version" ;; esac
  printf '%s\n' "$version" | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?$' || fail "invalid version: $version"

  binary="xmeta-$target"
  release_url="$download_base/v$version"
  printf 'Installing xmeta %s (%s)...\n' "$version" "$target"
  download "$release_url/$binary" "$temporary_directory/xmeta"
  download "$release_url/SHA256SUMS" "$temporary_directory/SHA256SUMS"
  expected=$(awk -v binary="$binary" '$2 == binary { print $1 }' "$temporary_directory/SHA256SUMS" | tr 'A-F' 'a-f')
  [ "${#expected}" -eq 64 ] || fail "SHA256SUMS has no unique entry for $binary"
  actual=$(sha256_file "$temporary_directory/xmeta")
  [ "$actual" = "$expected" ] || fail "checksum mismatch for $binary"
  chmod 755 "$temporary_directory/xmeta"
  actual_version=$("$temporary_directory/xmeta" --version) || fail 'the downloaded binary cannot run on this system'
  [ "$actual_version" = "xmeta $version" ] || fail "unexpected binary version: $actual_version"

  [ ! -d "$install_dir/xmeta" ] || fail "destination is a directory: $install_dir/xmeta"
  mkdir -p "$install_dir"
  temporary_target=$(mktemp "$install_dir/.xmeta.XXXXXX")
  cp "$temporary_directory/xmeta" "$temporary_target"
  chmod 755 "$temporary_target"
  mv -f "$temporary_target" "$install_dir/xmeta"
  temporary_target=
  printf 'Installed xmeta %s to %s/xmeta\n' "$version" "$install_dir"
  case ":${PATH:-}:" in
    *":$install_dir:"*) printf '%s\n' 'Run: xmeta --help' ;;
    *) printf 'Add %s to your shell PATH, then run xmeta --help.\n' "$install_dir" ;;
  esac
}

main "$@"
