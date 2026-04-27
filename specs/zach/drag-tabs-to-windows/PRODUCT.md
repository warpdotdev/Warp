# Drag Tabs to Windows

## Summary

Define the expected user-facing behavior for dragging tabs within a window, detaching tabs into new windows, and attaching dragged tabs into existing windows. The desired outcome is a continuous, Chrome-like drag interaction that preserves tab identity, keeps windows visually stable, and behaves predictably across repeated attach and detach cycles.

## Problem

Tab drag-and-drop crosses several user-visible states: in-window reordering, detach into a standalone window, attach into an existing window, continued dragging after attach, and final drop. If any transition behaves inconsistently, the interaction feels broken: tabs can appear duplicated, windows can flash transparent, close-confirmation dialogs can appear during harmless transfers, or the drag can feel like it unexpectedly ends mid-gesture.

Warp needs one coherent product definition for this interaction so implementers and reviewers can preserve the same mental model across all drag states. From the user's perspective, dragging a tab should feel like one uninterrupted gesture even when the tab changes windows multiple times before mouse-up.

## Goals

- Match the core interaction model of Chrome-style tab dragging.
- Support continuous drag gestures across reordering, detach, attach, and re-detach without requiring the user to release the mouse.
- Make single-tab and multi-tab windows behave consistently from the user's perspective.
- Make drop targeting and z-order behavior predictable when windows overlap.
- Keep the UI visually stable during the drag lifecycle: no transparent windows, no misplaced hover overlays, and no placeholder states that diverge from the eventual drop result.
- Preserve user trust by avoiding data-loss prompts or duplicate tabs during tab transfer.
- Leave the resulting active tab ready for immediate typing when the drag ends.

## Non-goals

- Adding new drop targets outside a window's tab bar.
- Changing non-tab window drag behavior outside the tab drag lifecycle.
- Redesigning the visual style of tabs, tab bars, or window chrome.
- Changing process lifetime or shell semantics during tab transfer; the feature only changes where the tab is hosted.

## Figma / design references

Figma: none provided

## User experience

### Reference model

The intended interaction should feel like Google Chrome tab dragging. The same drag gesture can pass through multiple states without interruption, and every state change should feel like a direct consequence of the cursor position.

### Default behavior

- A drag begins from the tab itself.
- If the tab remains within a multi-tab window's tab bar and moves horizontally, the interaction is an in-window reorder.
- If the tab moves vertically past the detach threshold, the interaction becomes a detached-window drag.
- While detached, moving over an eligible window tab bar attaches the real tab into that window at the current landing position.
- The drag remains active until mouse-up, even if the tab changes host windows one or more times before the drag ends.

### Single-tab windows

- If a window has exactly one tab, dragging anywhere in that tab drags the entire window.
- The tab is not treated as detached just because the drag began in a single-tab window.
- If that drag begins from the tab itself, the tab title/text and in-window rendering must remain visually stable while the window follows the cursor. The interaction must not introduce text jitter, shimmer, or a second drag preview layered over the moving window.
- Once that tab is attached into another window during the same gesture, it behaves like any other dragged tab: the user may continue dragging it, detach it again, attach it to a different window, or drop it as a standalone window without releasing the mouse first.

### Multi-tab reordering

- If a window has more than one tab, dragging a tab left or right reorders it within that window's tab bar.
- Reordering feedback updates continuously as the cursor moves.

### Detaching a tab

- Dragging a tab vertically beyond a small detach threshold removes it from its current window and creates a new window containing only that tab.
- The detached tab appears in the first position in the new window's tab bar.
- The new window is positioned so the cursor keeps the same relative offset within the dragged tab that it had at the moment of detachment.
- After detachment, the new window continues following the cursor for the remainder of the drag unless the tab is attached somewhere else or the user drops it.

### Dropping without a target window

- If the user releases the mouse while the detached tab is not over any eligible window tab bar, the detached window remains where it was dropped.
- The dropped result is immediately a normal, usable single-tab window.

### Attaching to a window

- While still dragging, the user may move the detached tab over the tab bar of either the original window or any other existing window.
- Attaching to the original window and attaching to a different window behave the same way.
- When a window becomes the current drop target:
  - that target window comes to the front, behind only the dragged preview window
  - the full tab is inserted at the exact position where it would land if the user released immediately
  - the tab is rendered as its real tab, with its real title, content, and styling
- There is no placeholder, ghost slot, or temporary empty state in the target tab bar.
- If the user released the mouse at that moment, there would be no additional visual jump because the tab is already shown in its final position.

### State transitions after attach

- Attaching a tab does not end the drag gesture.
- After attaching, the user can continue dragging the same tab without releasing the mouse.
- Continuing the drag may:
  - reorder the tab within the target window
  - detach it back out into its own window
  - attach it into yet another window
  - end the gesture by dropping it wherever it currently is
- This cycle may repeat any number of times in a single uninterrupted drag.
- Reversing direction must behave predictably. If the user drags a tab into a target window and then back out again, the tab transfers back out cleanly with no duplicate entries left behind.

### Reordering after attach

- Once a dragged tab is attached into a target window, horizontal movement within that target window's tab bar reorders it in real time just like a normal in-window reorder.
- This must feel the same regardless of whether the tab came from:
  - another multi-tab window
  - a single-tab window
  - the same window it is currently in

### No close confirmation during transfer

- If a window closes as part of the tab drag lifecycle, Warp must not show the "Close window?" confirmation dialog during that transfer.
- This applies when:
  - a source window loses its last tab
  - a temporary preview or transfer window is cleaned up after a handoff
- Running processes continue in the tab's new home window. The transfer should not be treated as user intent to close those processes.

### Single-tab handoff behavior

- When a tab that started in a single-tab window is attached into another window, the drag must continue seamlessly.
- The user must be able to drag that same tab back out again, attach it elsewhere, or drop it as a standalone window, all within the same gesture.
- The transfer must not cause visible duplicate windows, broken drag tracking, or any interruption that makes the user release and restart the drag.

### Hover behavior during drag

- While any tab drag is in progress, hover-only overlays inside tabs must not appear.
- This includes elements such as:
  - tab tooltips
  - close-button hover treatments
- During drag, the dragged tab must not show overlays that appear mispositioned relative to the moving tab.

### Z-order and drop targeting

- A dragged tab may only attach into a window whose tab bar is actually reachable based on the current z-order.
- The valid drop target is the window directly below the dragged preview window in z-order.
- If another window sits in front of a potential target and occludes its tab bar, that front window blocks the drop.
- The dragged tab must not pass through an intervening window to attach to a window behind it.

### Window opacity and content readiness

- No visible window may become transparent or appear see-through during any part of the drag sequence.
- When a tab is detached from a multi-tab window, the source window must immediately show another valid tab so it continues rendering normal content.
- The new detached preview window must not appear onscreen until its tab content is ready to render.
- Users should never observe a transparent flash or blank window caused by the new window appearing before its content is ready.

### Input focus after drag ends

- When the drag completes, the resulting active tab must have terminal input focus.
- This applies whether the tab ended as:
  - a standalone detached window
  - a tab attached into another window
  - a tab attached back into its original window
- The user should be able to type immediately without clicking again.

### Invariants

- The dragged tab always represents one real tab, never a duplicate.
- The tab shown in a target window during attach is the real tab in its live landing position, not a placeholder.
- The total number of tab instances across all windows remains constant throughout the drag lifecycle.
- No visible window should ever flash transparent, blank, or otherwise appear incomplete during transfer.
- Any window that closes solely because of tab transfer closes silently, without treating the transfer as destructive user intent.
- The resulting active tab is always usable immediately after mouse-up.

## Success criteria

1. Dragging the only tab in a window moves the whole window until that tab is attached into another window, and starting that drag from the tab itself does not cause visible tab-text jitter or layered-preview artifacts while the window moves.
2. Dragging a tab horizontally inside a multi-tab window reorders it in real time.
3. Dragging a tab vertically past the detach threshold creates a new single-tab window that follows the cursor while preserving the cursor's relative grab position within the tab.
4. Releasing a detached tab when it is not over an eligible tab bar leaves it as a normal standalone single-tab window.
5. Moving a detached tab over an eligible target window inserts the real tab into the exact landing position before mouse release, with no placeholder or ghost state.
6. After an attach, the user can keep dragging without releasing and can reorder, detach, or attach the same tab again in one continuous gesture.
7. Tabs attached into a target window reorder continuously inside that target window as the cursor moves left or right.
8. During tab transfer, Warp never shows a close-confirmation dialog for windows that close only because the tab changed host windows.
9. Single-tab-origin drags behave continuously across attach and detach cycles without interruption or duplicate windows.
10. No hover-only tab overlays appear while a drag is active.
11. Drop targeting respects z-order: an occluded window cannot receive the drop through a window in front of it.
12. No visible window becomes transparent, flashes blank, or shows invalid content during detach or handoff.
13. When the drag ends, the resulting active tab is focused and ready to accept typing immediately.
14. The total tab count across all windows stays constant throughout repeated attach/detach cycles.

## Validation

- **Single-tab drag**: Start with a window that has one tab. Drag the tab and verify the whole window moves. When the drag begins from the tab itself, verify the tab text/content stays visually stable with no jitter, shimmer, or duplicate-looking overlay as the window follows the cursor. Attach it into another window, then drag it back out again without releasing first and verify the gesture continues normally.
- **In-window reorder**: In a multi-tab window, drag a tab left and right and verify the order updates continuously.
- **Detach behavior**: Drag a tab vertically out of a multi-tab window and verify a new single-tab window appears, follows the cursor, and preserves the original cursor grab offset within the tab.
- **Drop with no target**: Detach a tab and release it over empty space. Verify it remains as a usable standalone window at the dropped location.
- **Attach behavior**: Detach a tab and drag it over another window's tab bar. Verify the real tab appears in its landing position before release and that releasing causes no extra visual jump.
- **Repeated attach/detach cycle**: During one uninterrupted drag, attach a tab into a target window, reorder it there, drag it back out, and attach it into a different window. Verify the drag remains continuous and no duplicate tabs appear.
- **Single-tab handoff**: Repeat the previous validation starting from a single-tab window. Verify the same continuity and no duplicate or stuck windows.
- **No close dialog**: Perform transfers involving tabs with running processes and verify no "Close window?" dialog appears as transfer windows close.
- **Hover suppression**: While dragging a tab, move across other tabs and verify tooltips and hover-only close affordances do not appear.
- **Z-order targeting**: Arrange overlapping windows so one tab bar is visually blocked by another window. Verify the blocked window cannot receive the dragged tab until it is actually the reachable top target.
- **No transparency**: During detach and handoff, verify that neither the source window nor the detached window flashes transparent or blank.
- **Focus after drop**: Complete the drag in each final state (standalone, other window, original window) and verify the terminal input is focused with a blinking cursor and accepts typing immediately.
- **Tab count stability**: Repeatedly drag a tab between windows and back out again, then verify the total number of tabs across windows matches the starting count.

## Open questions

None.
