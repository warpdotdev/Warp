#!/usr/bin/env python3

from __future__ import annotations

import json
from pathlib import Path
import platform
import subprocess
import sys


def linux_info() -> dict[str, str]:
    fields: dict[str, str] = {}
    os_release = Path("/etc/os-release")
    if os_release.exists():
        for line in os_release.read_text(encoding="utf-8", errors="replace").splitlines():
            if "=" not in line or line.startswith("#"):
                continue
            key, value = line.split("=", 1)
            fields[key] = value.strip().strip(chr(34))

    return {
        "os": fields.get("PRETTY_NAME") or fields.get("NAME") or "Linux",
        "os_version": fields.get("VERSION_ID") or fields.get("VERSION") or platform.release(),
        "kernel": platform.release(),
    }


def mac_info() -> dict[str, str]:
    build = None
    result = subprocess.run(
        ["sw_vers", "-buildVersion"],
        capture_output=True,
        text=True,
        check=False,
    )
    if result.returncode == 0:
        build = result.stdout.strip() or None

    return {
        "os": "macOS",
        "os_version": platform.mac_ver()[0] or None,
        "os_build": build,
    }


def windows_info() -> dict[str, str]:
    version = sys.getwindowsversion()
    return {
        "os": "Windows",
        "os_version": f"{version.major}.{version.minor}.{version.build}",
    }


def main() -> int:
    system = platform.system()
    if system == "Darwin":
        info = mac_info()
    elif system == "Linux":
        info = linux_info()
    elif system == "Windows":
        info = windows_info()
    else:
        info = {
            "os": system or "Unknown",
            "os_version": platform.version() or None,
        }

    json.dump({k: v for k, v in info.items() if v}, sys.stdout, sort_keys=True)
    sys.stdout.write("\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
