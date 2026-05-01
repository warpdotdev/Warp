# OSC 8 hyperlinks (clickable terminal anchors)

Source issue: [warpdotdev/warp#6393](https://github.com/warpdotdev/warp/issues/6393)

## Summary

Output that uses the OSC 8 hyperlink escape sequence (`ESC ] 8 ; params ; URI ESC \`) renders as a clickable link in the terminal block, opening the URL on Cmd+click (macOS) / Ctrl+click (Linux/Windows). Today, programs like `wizcli` that print "Click to view scan report" via OSC 8 are visually plain and unclickable in Warp; this spec brings Warp in line with iTerm2, kitty, GNOME Terminal, Windows Terminal, and other terminals that already support the sequence.

## Problem

Modern CLIs use OSC 8 to attach a URL to arbitrary visible text. The visible text is often not the URL itself (e.g. "Click to view scan report", "Open dashboard", "Issue #1234"), so Warp's existing regex-based URL auto-detection cannot recover the destination — the URL only exists in the escape sequence. As a result, users have to manually copy the URL from another tool or context, which the issue reporter calls out as a workflow blocker.

Figma: none provided.

## Behavior

1. **Recognition.** Output containing the OSC 8 sequence `ESC ] 8 ; params ; URI ST` (where ST is either `BEL` / `\x07` or `ESC \` / `\x1b\\`) marks the cells written between the opening sequence and a corresponding closing sequence (`ESC ] 8 ; ; ST` — same sequence with an empty URI) as a single hyperlink span pointing to `URI`.

2. **Closing.** A new opening sequence implicitly closes any previous open hyperlink — there is never more than one active hyperlink at a time. An explicit close (`ESC ] 8 ; ; ST`) clears the active hyperlink.

3. **Params.** The `params` field is a colon-separated list of `key=value` pairs. The `id` key is honored: hyperlinks that share the same non-empty `id` are treated as one logical link for hover and click, even when the on-screen cells are not contiguous (e.g. wrapped across two lines, or split by other output). Unknown keys are ignored, never cause the sequence to be discarded.

4. **Visible text.** The visible characters between the opening and closing sequence render exactly as they would without OSC 8 — same characters, same SGR styling (color, bold, italic, underline, etc.). The hyperlink does not change the rendered glyphs or insert any visible decoration of its own beyond what (5) describes.

5. **Hover affordance.** When the cursor moves over any cell in a hyperlink span:
   - The mouse cursor changes to the same pointer/hand shape used today for auto-detected URLs.
   - The full URI is shown in the same hover tooltip currently used for auto-detected URLs ("Open link"), with the URI itself displayed so the user can see where they would be navigating before clicking.
   - When the hyperlink span is not contiguous (same `id` across non-adjacent cells), hovering any cell highlights every cell in the span, not just the hovered run.

6. **Click to open.** Cmd+click (macOS) / Ctrl+click (Linux/Windows) on any cell in a hyperlink span opens the URI in the user's default browser, using the same `open_url` path used today for auto-detected URL links — so default-browser routing, telemetry, and "open with" behavior match. Plain (un-modified) click on a hyperlinked cell behaves like a plain click on any other terminal cell (selection / cursor placement); the modifier is required, matching today's URL link UX.

7. **Right-click context menu.** Right-clicking on a hyperlink span shows the same context menu Warp shows today for auto-detected URL links, with at minimum: "Open link", "Copy link" (copies the URI, not the visible text), and the standard text-selection items. The visible text is still selectable and copyable as text via the regular text selection actions.

8. **Selection and copy.** Selecting text that overlaps a hyperlink span and copying produces the visible text by default, not the URI — the same behavior as iTerm2 and kitty. An explicit "Copy link address" affordance (context menu, see (7)) is the way to get the URI.

9. **Coexistence with auto-detected URLs.** When a cell is part of an OSC 8 hyperlink, the OSC 8 URI takes precedence over any URL the auto-detector might find in the visible text. The two link surfaces never both light up on the same cell.
   - When OSC 8 spans only part of a wider auto-detected URL run (rare but possible), the OSC 8 span wins for the cells it covers and the auto-detected link continues to apply only to the cells outside the OSC 8 span.

10. **Block boundaries and reflow.**
    - A hyperlink span that crosses a soft wrap (line wrap inside one block) stays one logical span. Resizing the pane and reflowing the block preserves clickability.
    - A hyperlink span that is still open when a new prompt / block boundary arrives is implicitly closed at the block boundary — open hyperlinks do not bleed into the next command's output.
    - A new shell session, `clear`, or any `reset` sequence clears any active hyperlink state.

11. **Streaming.** When command output arrives incrementally, cells become hyperlinked the moment the opening OSC 8 is parsed; subsequent characters are part of the span until a close (or implicit close per (2) or (10)) is parsed. Hovering or clicking a still-streaming hyperlink works as soon as the opening sequence has been received — the user does not have to wait for the close.

12. **Scrollback and history.**
    - Hyperlinks remain clickable in scrollback for the lifetime of the block in the current session.
    - Restoring a session from history / Warp Drive / shared session preserves the URI on hyperlink spans so they remain clickable in the restored block.
    - Searching within a block matches against the visible text, not the URI.

13. **Sharing a block.** When a block is shared (Warp Drive, shared link, copy as markdown), hyperlink spans are preserved as markdown-style links — `[visible text](URI)` — so the recipient sees a clickable link with the same destination. Plain "copy block" preserves the original output bytes (including the OSC 8 sequences) so a paste into another OSC-8-aware terminal renders the same link.

14. **AI / agent context.** Block content fed to the AI assistant or agents includes the URI for any OSC 8 hyperlink span, in a form the model can use (e.g. inline as `visible text (URI)` or markdown link), so an agent reading the output of `wizcli` can act on the URI rather than only seeing "Click to view scan report".

15. **Robustness against malformed input.** None of the following may crash, hang, scramble subsequent output, or leave a permanently-open hyperlink:
    - Opening sequence with no closing sequence before EOF or block boundary (treated as implicit close per (10)).
    - Closing sequence with no prior opening sequence (no-op).
    - Unknown or duplicate `key=value` params (unknown keys ignored; duplicate `id` — last-wins).
    - Empty URI in an opening sequence (treated as a close).
    - URI containing characters that the spec does not strictly allow but real-world emitters use (e.g. spaces); Warp parses the URI permissively for hover/display, but only opens it on click if it parses as a valid URL — otherwise the click is a no-op and the hover tooltip shows the literal URI so the user can copy it manually.
    - Non-UTF-8 bytes in the URI: the sequence is dropped (no hyperlink span is created); the visible text continues to render normally.
    - Sequences exceeding the VTE OSC parser's buffer: dropped silently, same way other oversized OSC sequences are handled today.

16. **Security: scheme allow-list.** Click-to-open only follows URIs whose scheme is in Warp's existing allow-list for opened URLs (at least `http`, `https`, `mailto`, `ftp`, plus any schemes already permitted for auto-detected URL clicks). For a hyperlink with a disallowed scheme (`javascript:`, `data:`, `file:` outside the allow-list, etc.), the visible text is not click-activated; the hover tooltip shows the literal URI and a short warning ("scheme not allowed") so the user understands why the click is inert.

17. **Consistency with adjacent surfaces.** OSC 8 hyperlinks render and behave identically wherever terminal block output is shown today — main terminal blocks, alt-screen apps that emit OSC 8, the agent's terminal panel, and any other surface that runs the existing ANSI processor. The hover, click, copy, and context-menu behaviors above hold in every such surface.

18. **No regression for existing surfaces.**
    - Auto-detected URL links, file path links, and rich-content links continue to work as they do today, with the same hover, click, copy, and context-menu behaviors.
    - Output that does not contain OSC 8 sequences renders byte-for-byte identically before and after this feature ships.
    - Terminals receiving Warp's *outgoing* PTY traffic (e.g. when Warp itself is the program inside another terminal) see the same bytes they see today; this feature is purely about how Warp interprets OSC 8 bytes coming *from* the PTY.
