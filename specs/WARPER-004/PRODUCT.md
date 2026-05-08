# WARPER-004: complete Warper app branding

## Summary

Warper must present itself as Warper across app identity, menus, settings, about pages, icons, logos, prompts, and user-visible product copy. Upstream Warp branding must not remain on surfaces that identify the installed forked app.

## Problem

Visible app surfaces still contain upstream Warp identity, including about-page logos/copyright, menu descriptions, app icons, and other product labels. Warper now has its own icon source assets, but those assets still need to replace every upstream Warp icon use-case in the app bundle, Dock tile plugin, in-app icon picker, and generated app icon outputs.

## Goals / Non-goals

- Goal: replace app-identity surfaces that say or visually show Warp with Warper.
- Goal: ship Warper-specific app icons generated from the new Warper source PNGs.
- Goal: propagate the new Warper icon set to every app-icon use-case that currently uses upstream Warp icons.
- Goal: preserve transparent margins for Dock tile plugin resources and use no-margin icons for generated app icons.
- Goal: keep compatibility names only where changing them would break external terminal integrations.
- Non-goal: remove all occurrences of the string `warp` from internal code, package names, protocol compatibility values, or historical comments.
- Non-goal: redesign the entire visual system.

## Behavior

1. Finder, Dock, app switcher, Force Quit, macOS privacy prompts, and system dialogs identify the app as `Warper`.

2. The default macOS app icon is Warper-specific. It is generated from the new Warper no-margin source PNGs under `app/assets/bundled/icons`, resized or transformed for the app-icon use-case. It is not the upstream Warp icon, a lightly renamed Warp icon, or an upstream Warp alternate icon.

3. The Warper icon variants are `beaver`, `classic`, `dark`, `grunge`, `light`, `space`, `swiss`, and `vostok`. Every retained app-icon choice uses one of these Warper variants or another explicitly Warper-owned variant.

4. Dock tile plugin icon resources come from the margin-preserving Warper PNGs under `app/DockTilePlugin/Resources`. These resources keep transparent margins suitable for Dock rendering and replace the previous upstream Warp Dock tile resources.

5. Generated app icon files, channel icons, no-padding icons, packaged bundle icons, and any icon files used to create `.app` or installer icons come from the no-margin Warper PNGs under `app/assets/bundled/icons`. They are resized, padded, converted, or transformed only as required by the target format.

6. No upstream Warp app icon source remains in any visible app-icon path. Existing upstream icon names may remain only as compatibility aliases that resolve to Warper artwork and do not display Warp artwork.

7. App icon settings, if retained, offer Warper-owned icon choices using Warper labels and Warper artwork. Upstream Warp alternate icon names and images are absent from the user-visible picker.

8. Existing persisted icon preferences from an upstream Warp install do not cause Warper to display upstream Warp artwork. A stale upstream icon preference either maps to a Warper icon variant or falls back to the Warper default icon.

9. In-app logo assets used on About, Welcome, Get Started, empty states, and similar app-identity surfaces are Warper-specific.

10. Settings -> About shows Warper branding. It does not show the upstream Warp logo, `Copyright ... Warp`, or `About Warp`.

11. App menus and keybinding descriptions that identify the app use `Warper`, including About, help, settings, logs, and product-level commands.

12. Terminal prompts, onboarding copy, empty states, and AI input placeholders use `Warper` when addressing the installed app or local agent.

13. Compatibility values that external shells or integrations depend on may remain unchanged when required for behavior. Examples include terminal compatibility environment values or protocol names. These values are not shown as product branding unless necessary for compatibility.

14. User-visible links, dialogs, and messages do not refer to downloading, opening, or installing `Warp Desktop`.

15. Search, command palette, settings search, and menu search return Warper-branded labels for app-identity actions.

16. When outbound networking is blocked, all branding surfaces render from local assets without attempting to fetch upstream Warp images or metadata.
