---
name: warp-onboarding-walkthrough
description: Launch a single Oz cloud agent with computer use to download and install the latest stable Linux Warp build, then capture screenshots while walking through first-time onboarding until a usable terminal session is reached. Use this whenever the user asks to test, document, screenshot, or walk through the Warp first-time install/onboarding experience in a cloud Linux environment.
---

# Warp onboarding walkthrough

Use this skill to run a simple cloud-based walkthrough of the first-time Warp install and onboarding flow on Linux.

The parent agent should not perform the walkthrough locally. Launch one Oz cloud agent with computer use, have that child agent install the latest stable Warp Linux package appropriate for its platform, and ask it to capture screenshots at every visible onboarding step until Warp reaches a usable terminal session.

## Parent workflow

1. Launch exactly one remote Oz cloud agent with computer use enabled.
2. Use no environment-specific assumptions unless the user provided an environment. If no environment was provided, omit the environment ID and let Warp choose the default remote environment.
3. Give the child agent the child prompt below, filling in any user-specific details.
4. Wait for the child agent's report before summarizing results.

Use a `run_agents` call shaped like this:

```text
summary: Launching a cloud agent with computer use to install stable Warp and capture onboarding screenshots.
remote.computer_use_enabled: true
agent_run_configs: one child named "warp-onboarding-walkthrough"
base_prompt: the child prompt below
```

## Child prompt

Ask the cloud agent to do the following:

```text
You are verifying the first-time Warp install and onboarding experience on Linux.

Goal:
- Download and install the latest stable Warp Linux build appropriate for this cloud environment's distro and CPU architecture.
- Launch Warp in a fresh first-run state.
- Take a screenshot at every visible onboarding step.
- Do not create an account, log in, or use a real user identity.
- Continue only through login-free or account-free paths until Warp reaches a usable terminal session.
- Stop and report a blocker if the flow requires login or account creation with no skip/continue-without-account option.

Install requirements:
- Use official stable Warp downloads only.
- Do not use Warp Preview, Alpha, source builds, or this repository's development build.
- Detect CPU architecture with `uname -m`.
- Detect the package manager or distro before choosing the package format.
- Prefer native packages over AppImage because they install dependencies and register the app normally.

Stable Linux package mapping:
- Debian/Ubuntu with amd64 or x86_64: https://app.warp.dev/download?package=deb
- Debian/Ubuntu with arm64 or aarch64: https://app.warp.dev/download?package=deb_arm64
- Fedora/RHEL/CentOS/openSUSE with amd64 or x86_64: https://app.warp.dev/download?package=rpm
- Fedora/RHEL/CentOS/openSUSE with arm64 or aarch64: https://app.warp.dev/download?package=rpm_arm64
- Arch with amd64 or x86_64: https://app.warp.dev/download?package=pacman
- Arch with arm64 or aarch64: https://app.warp.dev/download?package=pacman_arm64
- If no native package path is available, use the AppImage fallback:
  - amd64 or x86_64: https://app.warp.dev/download?package=appimage
  - arm64 or aarch64: https://app.warp.dev/download?package=appimage_arm64

Before launch:
- Create an artifact directory such as `~/warp-onboarding-walkthrough`.
- Ensure the run starts from a fresh Warp first-run state by removing only Warp-specific config/data/cache directories for the test user, such as `~/.config/warp-terminal`, `~/.local/share/warp-terminal`, and `~/.cache/warp-terminal` if they exist.
- Do not delete unrelated user files or system directories.

Screenshot workflow:
- Take the first screenshot before interacting with the first visible Warp window.
- Take one screenshot before every user action.
- Take another screenshot after each action if the UI changes.
- Use sequential filenames such as `01-initial-window.png`, `02-skip-login.png`, and `03-terminal-ready.png`.
- Maintain a manifest file in the artifact directory with, for each screenshot:
  - filename
  - timestamp
  - what was visible
  - what action was about to happen or just happened

Onboarding behavior:
- Choose the default or most conservative option at each step unless it would require login.
- If there is a skip, "continue without account", "not now", or equivalent option, use it.
- Do not enter an email address, connect OAuth, or create credentials.
- If telemetry, shell, theme, or editor-import choices appear, use the default path and document the choice in the manifest.
- Continue until a normal terminal prompt is visible and usable.

Terminal verification:
- Once a terminal session is visible, run a harmless command such as `echo warp-onboarding-ready`.
- Capture a final screenshot showing the usable terminal and command output.

Report back:
- OS and distro detected.
- CPU architecture detected.
- Package URL and install method used.
- Launch command used.
- Whether the walkthrough reached a usable terminal session.
- Ordered screenshot list with short descriptions.
- Artifact directory path.
- Any blocker, crash, missing dependency, display problem, or step that required judgment.

Do not upload screenshots or logs to public external services. If the harness provides a built-in artifact or screenshot attachment mechanism, use that. Otherwise, leave the files in the artifact directory and report their paths.
```

## Success criteria

The walkthrough is successful when the child agent reports:

- Warp stable was installed from an official Linux package or AppImage for the detected architecture.
- Screenshots were captured for each onboarding screen and the final usable terminal.
- No login, account creation, or real user identity was used.
- A terminal session was usable enough to run `echo warp-onboarding-ready`.

## Common failure handling

- If the package manager prompts for confirmation, use the non-interactive confirmation flag supported by that package manager.
- If launching `warp-terminal` fails because of display setup, inspect the cloud environment's display variables and try launching from the desktop/app launcher if computer use provides one.
- If the app blocks on login with no skip path, stop at that screen, capture a screenshot, and report that as the terminal point for this hello-world version.
- If the native package cannot be installed because dependencies are unavailable, fall back to the matching AppImage and clearly report the fallback.
