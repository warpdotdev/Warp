# Cross-Window Tab Drag — Product Spec

## What the user sees

When dragging a tab from one Warp window into another:

1. **Floating tab chip** — a small tab-shaped element (title text + icon, same visual as a real tab) appears in the **target window** and follows the cursor in real-time as the user moves over the tab bar. It looks and moves exactly like dragging a tab within the same window.

2. **Insertion slot** — an empty space with an accented background appears in the target tab bar at the position where the tab will land if dropped. This matches exactly what same-window drag shows for the dragged tab's origin slot.

3. **Drop** — the tab transfers into the target window at the indicated position. The source window loses that tab (and closes if it was the only one).

## What the user does NOT see

- The entire source window overlaid on top of the target window.
- A static non-moving placeholder stuck at one position in the tab list.
- Any visual flicker or stutter during hover.

## Key constraint

The floating chip is a **pure visual overlay** rendered inside the target window. It represents the tab being dragged (title + icon) but does NOT require moving the actual pane group (terminals, editors, etc.) into the target window during hover. The real data transfer only happens on drop.

This means: zero view-tree transfers during hover → no performance cost.
