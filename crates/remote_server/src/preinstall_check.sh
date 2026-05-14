#!/usr/bin/env bash
# OpenWarp remote-server 二进制的预安装检查。
#
# stdout 输出结构化 key=value 摘要。退出码 0 表示探测完成;
# 非 0 表示探测过程失败,客户端会按 `status=unknown` 处理并 fail open。

set -u

# OpenWarp Linux remote-server 由 openwarp_release.yml 的 ubuntu-22.04
# job 构建,glibc floor 是 2.35。runner 镜像升级时同步调整。
required_glibc="2.35"
echo "required_glibc=${required_glibc}"

# 1. 识别 libc family,并在 glibc 场景下识别版本。
libc_family="unknown"
libc_version=""

if version=$(getconf GNU_LIBC_VERSION 2>/dev/null); then
    # 输出形如: "glibc 2.35"
    libc_family="glibc"
    libc_version="${version##* }"
elif ldd_out=$(ldd --version 2>&1 | head -n1); then
    case "$ldd_out" in
        *musl*)   libc_family="musl"   ;;
        *uClibc*) libc_family="uclibc" ;;
        *)
            v=$(printf '%s\n' "$ldd_out" | grep -oE '[0-9]+\.[0-9]+' | head -n1)
            if [ -n "$v" ]; then
                libc_family="glibc"
                libc_version="$v"
            fi
            ;;
    esac
fi

echo "libc_family=${libc_family}"
[ -n "$libc_version" ] && echo "libc_version=${libc_version}"

# 2. 根据探测结果判断支持状态。
status="unknown"
reason=""

if [ "$libc_family" = "glibc" ] && [ -n "$libc_version" ]; then
    have_major="${libc_version%%.*}"
    have_minor="${libc_version#*.}"
    have_minor="${have_minor%%.*}"
    req_major="${required_glibc%%.*}"
    req_minor="${required_glibc#*.}"
    if [ "$have_major" -gt "$req_major" ] \
       || { [ "$have_major" -eq "$req_major" ] && [ "$have_minor" -ge "$req_minor" ]; }; then
        status="supported"
    else
        status="unsupported"
        reason="glibc_too_old"
    fi
elif [ "$libc_family" = "musl" ] || [ "$libc_family" = "bionic" ] || [ "$libc_family" = "uclibc" ]; then
    status="unsupported"
    reason="non_glibc"
fi

echo "status=${status}"
[ -n "$reason" ] && echo "reason=${reason}"
