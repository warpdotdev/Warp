#!/usr/bin/env bash
# 在远端主机安装 OpenWarp CLI 二进制,用于 remote-server-proxy。
#
# setup.rs 会在运行时替换这些占位符:
#   {download_base_url}     - 例如 https://github.com/zerx-lab/warp/releases/latest/download
#   {install_dir}           - 例如 ~/.openwarp/remote-server
#   {binary_name}           - 例如 warp-oss
#   {version_suffix}        - 例如 -v0.2026...,没有 release tag 时为空
#   {staging_tarball_path}  - SCP fallback 预上传 tarball 路径,常规下载路径为空
set -e

arch=$(uname -m)
case "$arch" in
  x86_64|amd64)  arch_name=x86_64 ;;
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
case "$install_dir" in
  "~"|"~/"*) install_dir="${HOME}${install_dir#\~}" ;;
esac
mkdir -p "$install_dir"

tmpdir=$(mktemp -d "$install_dir/.install.XXXXXX")
# 尽力清理 staging 目录。这里失败不能覆盖真正的安装结果:
# trap 触发时二进制要么已经移动到最终路径,要么脚本已经因为
# 其他原因失败,后者的错误更值得暴露给调用方。
cleanup() {
  rm -rf "$tmpdir" 2>/dev/null || true
}
trap cleanup EXIT

staging_tarball_path="{staging_tarball_path}"
if [ -n "$staging_tarball_path" ]; then
  case "$staging_tarball_path" in
    "~"|"~/"*) staging_tarball_path="${HOME}${staging_tarball_path#\~}" ;;
  esac
  mv "$staging_tarball_path" "$tmpdir/openwarp.tar.gz"
else
  url="{download_base_url}/openwarp-$os_name-$arch_name.tar.gz"
  if command -v curl >/dev/null 2>&1; then
    curl -fSL --connect-timeout 15 "$url" -o "$tmpdir/openwarp.tar.gz"
  elif command -v wget >/dev/null 2>&1; then
    wget -q -O "$tmpdir/openwarp.tar.gz" "$url"
  else
    echo "error: neither curl nor wget is available" >&2
    exit 3
  fi
fi

tar -xzf "$tmpdir/openwarp.tar.gz" -C "$tmpdir"

bin="$tmpdir/{binary_name}"
if [ ! -f "$bin" ]; then
  bin=$(find "$tmpdir" -type f \( -name 'warp-oss' -o -name 'oz*' \) ! -path "$tmpdir/resources/*" ! -name '*.tar.gz' | head -n1)
fi
if [ -z "$bin" ]; then echo "no binary found in tarball" >&2; exit 1; fi
chmod +x "$bin"
mv "$bin" "$install_dir/{binary_name}{version_suffix}"
