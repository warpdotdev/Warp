#!/usr/bin/env bash
# Installs the Warp remote server binary on a remote host.
#
# Placeholders (substituted at runtime by setup.rs):
#   {download_base_url}  — e.g. https://app.warp.dev/download/cli
#   {channel}            — stable | preview | dev
#   {install_dir}        — e.g. ~/.warp/remote-server
#   {binary_name}        — e.g. oz | oz-dev | oz-preview
#   {version_query}      — e.g. &version=v0.2026... (empty when no release tag)
#   {version_suffix}     — e.g. -v0.2026...        (empty when no release tag)
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
install_dir="${install_dir/#\~/"$HOME"}"
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

curl -fSL "{download_base_url}?package=tar&os=$os_name&arch=$arch_name&channel={channel}{version_query}" \
  -o "$tmpdir/oz.tar.gz"
tar -xzf "$tmpdir/oz.tar.gz" -C "$tmpdir"

bin=$(find "$tmpdir" -type f -name 'oz*' ! -name '*.tar.gz' | head -n1)
if [ -z "$bin" ]; then echo "no binary found in tarball" >&2; exit 1; fi
chmod +x "$bin"
mv "$bin" "$install_dir/{binary_name}{version_suffix}"
