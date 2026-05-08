# WARPER-002: macOS launch privacy prompts must not repeat

## Summary

Warper must launch on macOS without repeatedly asking for access to other apps' data, Login Keychain, Desktop, Documents, or other protected locations. Any privacy permission Warper requests must be tied to a visible user action and must persist according to normal macOS behavior.

## Problem

After installing the macOS app bundle, each launch can show a system prompt that `warper.app` wants to access data from other apps. Clicking Allow does not stop the prompt from returning on later launches, which makes a normal terminal launch feel broken and suggests Warper is still touching upstream Warp-owned containers or unnecessary protected resources.

## Goals / Non-goals

- Goal: opening Warper from Finder, Dock, Spotlight, or the command line does not trigger repeated macOS privacy prompts.
- Goal: Warper does not read or write upstream Warp app-group containers or upstream Warp keychain items.
- Goal: retained OpenRouter credentials remain supported as a local user-configured feature.
- Non-goal: remove macOS permission prompts that are necessary for explicit terminal user actions, such as a user running a command that accesses Desktop or Documents.
- Non-goal: bypass macOS privacy controls or suppress legitimate system prompts.

## Behavior

1. On first launch after installing Warper, the app opens to a usable terminal without prompting for access to other apps' data.
2. On every later launch, the app continues to open without prompting for access to other apps' data.
3. Warper never accesses a Warp-branded app group, Warp-branded group container, Warp-branded app container, or Warp-branded keychain item during launch.
4. Warper never requires access to upstream Warp data to migrate, restore, or initialize local terminal state.
5. If Warper needs to store user-provided OpenRouter credentials, it uses a Warper-owned credential identity. The user-visible system prompt, if macOS shows one, identifies Warper and persists normally after the user allows it.
6. If no OpenRouter credential has been saved, launch does not read secure storage in a way that causes a keychain prompt.
7. If optional local integrations such as MCP credentials need secure storage, they do not trigger a keychain prompt at launch unless the user has configured that integration and the credential is needed for visible local behavior.
8. Launch does not request Desktop, Documents, Downloads, Photos, Contacts, Calendar, Location, Camera, Microphone, Automation, or similar protected access unless a visible user action requires that resource.
9. Warper does not scan another terminal app's preferences or data at startup unless the user has explicitly started an import flow.
10. A terminal command that accesses a protected directory behaves like the same command in a normal terminal: macOS may prompt for that command or terminal session, but Warper does not pre-request the permission at startup.
11. Restoring persisted terminal sessions does not pre-scan protected workspace folders during launch. If a restored session points into a protected folder, Warper waits for a visible user action before performing protected reads that can prompt macOS.
12. If the user denies a permission prompted by an explicit action, Warper keeps running and reports only the failed action. It does not enter a repeated prompt loop.
13. Moving `Warper.app` between folders, including `/Applications` and `/Applications/DevelopmentTools`, does not make the same launch prompt appear again on every launch.
14. Running two Warper windows or launching Warper while another instance is running does not trigger duplicate privacy prompts.
15. Startup logs do not contain repeated keychain, app-group, TCC, protected-directory, other-app-preferences, or permission-denied errors for resources Warper did not visibly need.
