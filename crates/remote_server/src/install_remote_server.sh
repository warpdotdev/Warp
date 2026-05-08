#!/usr/bin/env bash
# Installs the Warp remote server binary on a remote host.
#
# Placeholders (substituted at runtime by setup.rs):
#   {download_base_url}         — e.g. https://app.warp.dev/download/cli
#   {channel}                   — stable | preview | dev
#   {install_dir}               — e.g. ~/.warp/remote-server
#   {binary_name}               — e.g. oz | oz-dev | oz-preview
#   {version_query}             — e.g. &version=v0.2026... (empty when no release tag)
#   {version_suffix}            — e.g. -v0.2026...        (empty when no release tag)
#   {no_http_client_exit_code}  — exit code when neither curl nor wget is available
#   {download_failed_exit_code} — exit code when both curl and wget fail to download
#   {no_tar_exit_code}          — exit code when tar is not available
#   {staging_tarball_path}      — path to a pre-uploaded tarball (SCP fallback; empty normally)
set -e

arch=$(uname -m)
case "$arch" in
  x86_64)        arch_name=x86_64 ;;
  aarch64|arm64) arch_name=aarch64 ;;
  *) echo "unsupported arch: $arch" >&2; exit 2 ;;
esac

os_kernel=$(uname -s)
case "$os_kernel" in
  Darwin) os_name=macos ;;
  Linux)  os_name=linux ;;
  *) echo "unsupported OS: $os_kernel" >&2; exit 2 ;;
esac

install_dir="{install_dir}"
# Avoid `${var/pattern/replacement}` for tilde expansion. Two
# interpreter quirks make it dangerous in this script:
#   1. bash 3.2 (macOS /bin/bash) keeps inner double-quotes around the
#      replacement literal, so `"$HOME"` ends up as 6 literal
#      characters and the install lands under a directory tree
#      literally named `"`.
#   2. bash 5.2+ enables `patsub_replacement` by default, which makes
#      `&` in the replacement expand to the matched pattern, so a
#      `$HOME` containing `&` resolves to a `~`-substituted path.
# Use `case` + `${var#\~}` instead — works on bash 3.2 and bash 5.2+
# without surprises.
case "$install_dir" in
  "~"|"~/"*) install_dir="${HOME}${install_dir#\~}" ;;
esac
mkdir -p "$install_dir"

tmpdir=$(mktemp -d "$install_dir/.install.XXXXXX")
# Best-effort cleanup of the staging directory. A failure here (e.g.
# EBUSY or "Directory not empty" races on some filesystems/mounts)
# must not fail the install: by the time this fires the binary has
# either already been moved into its final location, or the script
# has already failed for an unrelated reason that we want to surface
# instead of clobbering with the cleanup's exit code.
cleanup() {
  rm -rf "$tmpdir" 2>/dev/null || true
}
trap cleanup EXIT

staging_tarball_path="{staging_tarball_path}"
if [ -n "$staging_tarball_path" ]; then
  # SCP fallback: tarball already uploaded by the client.
  # Same tilde-expansion caveat as install_dir above.
  case "$staging_tarball_path" in
    "~"|"~/"*) staging_tarball_path="${HOME}${staging_tarball_path#\~}" ;;
  esac
  mv "$staging_tarball_path" "$tmpdir/oz.tar.gz"
else
  # Normal path: download via curl or wget, with retry using the
  # alternate client if the primary fails.
  url="{download_base_url}?package=tar&os=$os_name&arch=$arch_name&channel={channel}{version_query}"

  has_curl=false
  has_wget=false
  command -v curl >/dev/null 2>&1 && has_curl=true
  command -v wget >/dev/null 2>&1 && has_wget=true

  if [ "$has_curl" = false ] && [ "$has_wget" = false ]; then
    echo "error: neither curl nor wget is available" >&2
    exit {no_http_client_exit_code}
  fi

  download_ok=false
  download_err=""

  # Try primary client, then retry with alternate on failure.
  if [ "$has_curl" = true ]; then
    if curl -fSL --connect-timeout 15 --retry 1 "$url" -o "$tmpdir/oz.tar.gz" 2>/dev/null; then
      download_ok=true
    else
      download_err="curl failed (exit $?)"
    fi
  fi

  if [ "$download_ok" = false ] && [ "$has_wget" = true ]; then
    if wget -q --timeout=15 -O "$tmpdir/oz.tar.gz" "$url" 2>/dev/null; then
      download_ok=true
    else
      download_err="${download_err:+$download_err; }wget failed (exit $?)"
    fi
  fi

  if [ "$download_ok" = false ]; then
    echo "error: remote download failed: $download_err" >&2
    exit {download_failed_exit_code}
  fi
fi

# Verify tar is available before attempting extraction.
if ! command -v tar >/dev/null 2>&1; then
  echo "error: tar is not available" >&2
  exit {no_tar_exit_code}
fi

tar -xzf "$tmpdir/oz.tar.gz" -C "$tmpdir"

bin=$(find "$tmpdir" -type f -name 'oz*' ! -name '*.tar.gz' | head -n1)
if [ -z "$bin" ]; then echo "no binary found in tarball" >&2; exit 1; fi
chmod +x "$bin"
mv "$bin" "$install_dir/{binary_name}{version_suffix}"
