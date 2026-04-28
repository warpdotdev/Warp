# Platform Guidance

Use this only when the operating system, Warp version, or operating-system-specific behavior is relevant and missing.

- Resolve the execution surface first: packaged native Warp app or web session.
- Prefer the bundled helper scripts and metadata files over prose or ad hoc shell inspection.
- If the user already gave a sufficiently specific OS version or Warp version, do not ask again.
- Include both OS name and version in the `Operating system` section when available.
- Include the `Warp version` section when available, and note when the report is about a web session rather than a packaged native install.

## Operating system

Resolve the OS from the machine where the reported behavior actually happens. Do not substitute the OS of a different host, container, or remote target unless that is where the bug occurs.
Run the bundled helper script when you need to resolve OS name and version:

```bash
python3 scripts/resolve_platform.py
```

Use the script output directly when filling `Operating system`. Ask the user only if Python is unavailable or the output still does not identify the relevant environment precisely enough.

## Warp version

For packaged native Warp installs, read the bundled version metadata file directly:
The bundled version metadata file lives at `../../metadata/version.json` relative to the skill root. Read its `warp_version` field and use that value directly.

Use the file contents directly when filling `Warp version`. Ask the user only if Python is unavailable, the bundled metadata file is missing or unreadable, or the report is about a browser or web session rather than a packaged native install.

- Browser or web session with no local Warp executable: use the version or build identifier from the session URL or surrounding session metadata when present. If there is no concrete version string, record that it was a web session and leave `Warp version` as `Unknown` rather than guessing.
